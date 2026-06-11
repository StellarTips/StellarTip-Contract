#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, panic_with_error, token, Address, Env, String, Symbol};

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

/// Storage keys used for persistent and instance storage.
#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    /// Admin address with privileged access.
    Admin,
    /// Pause flag for emergency stop.
    Paused,
    /// Platform fee in basis points (0–10_000).
    FeeBps,
    /// Address that receives platform fees.
    FeeRecipient,
    /// Creator profile keyed by the creator's `Address`.
    Profile(Address),
    /// Reverse lookup: `Symbol` (username) → `Address` (creator).
    UsernameToAddress(Symbol),
    /// Balance of a given token held for a creator.
    /// Encoded as `(creator Address, token Address)`.
    Balance(Address, Address),
    /// Total number of tips ever received by a creator.
    TipCount(Address),
    /// Single tip record identified by `(creator Address, index)`.
    Tip(Address, u64),
    /// List of tokens a creator has received tips in.
    CreatorTokens(Address),
}

/// Public profile information for a creator.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct CreatorProfile {
    pub username: Symbol,
    pub display_name: String,
    pub bio: String,
    pub registered_at: u64,
}

/// A single tip that has been sent to a creator.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct Tip {
    pub from: Address,
    pub token: Address,
    pub amount: i128,
    pub message: String,
    pub timestamp: u64,
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

mod error {
    use soroban_sdk::contracterror;

    #[contracterror]
    #[derive(Copy, Clone, Debug, Eq, PartialEq)]
    pub enum TipError {
        CreatorAlreadyExists = 1,
        CreatorNotFound = 2,
        UsernameTaken = 3,
        InsufficientBalance = 4,
        TransferFailed = 5,
        InvalidAmount = 6,
        NoTips = 7,
        NotInitialized = 8,
        AlreadyInitialized = 9,
        Paused = 10,
        NotAuthorized = 11,
        InvalidInput = 12,
        BalanceNotEmpty = 13,
    }
}

use error::TipError;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Current contract version for client compatibility.
pub const CONTRACT_VERSION: u32 = 1;

/// Maximum platform fee in basis points (100% = 10_000 bps).
const MAX_FEE_BPS: u32 = 10_000;

/// Display name max length in bytes.
const MAX_DISPLAY_NAME_LEN: u32 = 64;
/// Bio max length in bytes.
const MAX_BIO_LEN: u32 = 256;

/// TTL threshold (ledgers) before extension is triggered.
/// ~17_280 ledgers per day; 15 days.
const TTL_THRESHOLD: u32 = 17_280 * 15;
/// TTL extension target (ledgers).
/// ~30 days.
const TTL_EXTEND: u32 = 17_280 * 30;

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

/// Emitted when a new creator registers.
const EVENT_CREATOR_REGISTERED: Symbol = soroban_sdk::symbol_short!("CREG");

/// Emitted when a tip is sent.
const EVENT_TIP_SENT: Symbol = soroban_sdk::symbol_short!("TIP");

/// Emitted when a creator withdraws tokens.
const EVENT_WITHDRAW: Symbol = soroban_sdk::symbol_short!("WDRW");

/// Emitted when a creator updates their profile.
const EVENT_PROFILE_UPDATED: Symbol = soroban_sdk::symbol_short!("PUPD");

/// Emitted when a creator unregisters.
const EVENT_CREATOR_UNREGISTERED: Symbol = soroban_sdk::symbol_short!("UREG");

/// Emitted when the contract is paused.
const EVENT_PAUSED: Symbol = soroban_sdk::symbol_short!("PAUS");

/// Emitted when the contract is unpaused.
const EVENT_UNPAUSED: Symbol = soroban_sdk::symbol_short!("UNPA");

/// Emitted when the platform fee is changed.
const EVENT_FEE_CHANGED: Symbol = soroban_sdk::symbol_short!("FEEC");

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Extend the TTL of the contract instance storage.
fn extend_instance_ttl(env: &Env) {
    env.storage().instance().extend_ttl(TTL_THRESHOLD, TTL_EXTEND);
}

/// Extend the TTL of a persistent storage entry.
fn extend_persistent_ttl(env: &Env, key: &DataKey) {
    env.storage().persistent().extend_ttl(key, TTL_THRESHOLD, TTL_EXTEND);
}

/// Verify the contract is initialized and not paused.
fn check_initialized_and_not_paused(env: &Env) {
    if !env.storage().instance().has(&DataKey::Admin) {
        panic_with_error!(env, TipError::NotInitialized);
    }
    let is_paused: bool = env
        .storage()
        .instance()
        .get(&DataKey::Paused)
        .unwrap_or(false);
    if is_paused {
        panic_with_error!(env, TipError::Paused);
    }
    extend_instance_ttl(env);
}

/// Validate string length constraints.
fn validate_input(env: &Env, _username: Option<Symbol>, display_name: &String, bio: &String) {
    // Username is a Symbol which is already limited by the Soroban SDK
    // to ScSymbol's max length (32 bytes), so we skip an explicit check here.
    let _ = _username;
    if display_name.len() > MAX_DISPLAY_NAME_LEN {
        panic_with_error!(env, TipError::InvalidInput);
    }
    if bio.len() > MAX_BIO_LEN {
        panic_with_error!(env, TipError::InvalidInput);
    }
}

// ---------------------------------------------------------------------------
// Contract
// ---------------------------------------------------------------------------

#[contract]
pub struct TipContract;

#[contractimpl]
impl TipContract {
    // -----------------------------------------------------------------------
    // Initialization
    // -----------------------------------------------------------------------

    /// Initialize the contract with an admin, fee recipient, and platform fee.
    ///
    /// # Arguments
    /// * `admin` – Address with privileged control (pause, fee settings).
    /// * `fee_recipient` – Address that receives platform fees.
    /// * `fee_bps` – Platform fee in basis points (0–10_000).
    pub fn init(env: Env, admin: Address, fee_recipient: Address, fee_bps: u32) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic_with_error!(env, TipError::AlreadyInitialized);
        }
        if fee_bps > MAX_FEE_BPS {
            panic_with_error!(env, TipError::InvalidInput);
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::FeeRecipient, &fee_recipient);
        env.storage().instance().set(&DataKey::FeeBps, &fee_bps);
        env.storage().instance().set(&DataKey::Paused, &false);
        extend_instance_ttl(&env);
    }

    // -----------------------------------------------------------------------
    // Admin functions
    // -----------------------------------------------------------------------

    /// Transfer admin privileges to a new address.
    pub fn set_admin(env: Env, caller: Address, new_admin: Address) {
        caller.require_auth();
        let current_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| panic_with_error!(env, TipError::NotInitialized));
        if caller != current_admin {
            panic_with_error!(env, TipError::NotAuthorized);
        }
        env.storage().instance().set(&DataKey::Admin, &new_admin);
        extend_instance_ttl(&env);
    }

    /// Pause the contract (emergency stop). Only admin can call.
    pub fn pause(env: Env, caller: Address) {
        caller.require_auth();
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| panic_with_error!(env, TipError::NotInitialized));
        if caller != admin {
            panic_with_error!(env, TipError::NotAuthorized);
        }
        env.storage().instance().set(&DataKey::Paused, &true);
        extend_instance_ttl(&env);
        env.events().publish((EVENT_PAUSED, caller), ());
    }

    /// Unpause the contract. Only admin can call.
    pub fn unpause(env: Env, caller: Address) {
        caller.require_auth();
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| panic_with_error!(env, TipError::NotInitialized));
        if caller != admin {
            panic_with_error!(env, TipError::NotAuthorized);
        }
        env.storage().instance().set(&DataKey::Paused, &false);
        extend_instance_ttl(&env);
        env.events().publish((EVENT_UNPAUSED, caller), ());
    }

    /// Set the platform fee percentage. Only admin can call.
    pub fn set_fee_percentage(env: Env, caller: Address, fee_bps: u32) {
        caller.require_auth();
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| panic_with_error!(env, TipError::NotInitialized));
        if caller != admin {
            panic_with_error!(env, TipError::NotAuthorized);
        }
        if fee_bps > MAX_FEE_BPS {
            panic_with_error!(env, TipError::InvalidInput);
        }
        env.storage().instance().set(&DataKey::FeeBps, &fee_bps);
        extend_instance_ttl(&env);
        env.events().publish((EVENT_FEE_CHANGED, caller), fee_bps);
    }

    /// Set the fee recipient address. Only admin can call.
    pub fn set_fee_recipient(env: Env, caller: Address, fee_recipient: Address) {
        caller.require_auth();
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| panic_with_error!(env, TipError::NotInitialized));
        if caller != admin {
            panic_with_error!(env, TipError::NotAuthorized);
        }
        env.storage().instance().set(&DataKey::FeeRecipient, &fee_recipient);
        extend_instance_ttl(&env);
    }

    // -----------------------------------------------------------------------
    // Registration
    // -----------------------------------------------------------------------

    /// Register a new creator profile.
    ///
    /// # Arguments
    /// * `username` – A unique `Symbol` identifier (e.g. `"jane"`).
    /// * `display_name` – Human-readable display name.
    /// * `bio` – Short biography text.
    pub fn register(
        env: Env,
        caller: Address,
        username: Symbol,
        display_name: String,
        bio: String,
    ) {
        caller.require_auth();
        check_initialized_and_not_paused(&env);
        validate_input(&env, Some(username.clone()), &display_name, &bio);

        // Each address can only register once.
        if env.storage().instance().has(&DataKey::Profile(caller.clone())) {
            panic_with_error!(env, TipError::CreatorAlreadyExists);
        }

        // Each username must be unique.
        if env.storage().instance().has(&DataKey::UsernameToAddress(username.clone())) {
            panic_with_error!(env, TipError::UsernameTaken);
        }

        let profile = CreatorProfile {
            username: username.clone(),
            display_name,
            bio,
            registered_at: env.ledger().timestamp(),
        };

        env.storage().instance().set(&DataKey::Profile(caller.clone()), &profile);
        env.storage().instance().set(&DataKey::UsernameToAddress(username), &caller);
        // TipCount moved to persistent storage for durability.
        env.storage().persistent().set(&DataKey::TipCount(caller.clone()), &0u64);
        extend_persistent_ttl(&env, &DataKey::TipCount(caller.clone()));

        env.events().publish(
            (EVENT_CREATOR_REGISTERED, caller),
            (profile.username, profile.registered_at),
        );
    }

    /// Update a creator's display name and bio.
    pub fn update_profile(env: Env, caller: Address, display_name: String, bio: String) {
        caller.require_auth();
        check_initialized_and_not_paused(&env);
        validate_input(&env, None, &display_name, &bio);

        let mut profile: CreatorProfile = env
            .storage()
            .instance()
            .get(&DataKey::Profile(caller.clone()))
            .unwrap_or_else(|| panic_with_error!(env, TipError::CreatorNotFound));

        profile.display_name = display_name;
        profile.bio = bio;

        env.storage().instance().set(&DataKey::Profile(caller.clone()), &profile);
        extend_instance_ttl(&env);

        env.events().publish(
            (EVENT_PROFILE_UPDATED, caller),
            (profile.username, profile.display_name.clone()),
        );
    }

    /// Unregister a creator. Requires all token balances to be zero.
    pub fn unregister(env: Env, caller: Address) {
        caller.require_auth();
        check_initialized_and_not_paused(&env);

        let profile: CreatorProfile = env
            .storage()
            .instance()
            .get(&DataKey::Profile(caller.clone()))
            .unwrap_or_else(|| panic_with_error!(env, TipError::CreatorNotFound));

        // Ensure all balances are zero.
        let tokens_key = DataKey::CreatorTokens(caller.clone());
        if let Some(tokens) = env
            .storage()
            .persistent()
            .get::<_, Vec<Address>>(&tokens_key)
        {
            for token in tokens.iter() {
                let balance = env
                    .storage()
                    .persistent()
                    .get::<_, i128>(&DataKey::Balance(caller.clone(), token))
                    .unwrap_or(0);
                if balance > 0 {
                    panic_with_error!(env, TipError::BalanceNotEmpty);
                }
            }
            env.storage().persistent().remove(&tokens_key);
        }

        let tip_count_key = DataKey::TipCount(caller.clone());
        env.storage().persistent().remove(&tip_count_key);

        env.storage().instance().remove(&DataKey::UsernameToAddress(profile.username));
        env.storage().instance().remove(&DataKey::Profile(caller.clone()));

        env.events().publish((EVENT_CREATOR_UNREGISTERED, caller), ());
    }

    // -----------------------------------------------------------------------
    // Tipping
    // -----------------------------------------------------------------------

    /// Send a tip to a registered creator.
    ///
    /// The caller authorises a transfer of `amount` of `token` to this
    /// contract, which credits the creator's internal balance and records the
    /// tip for history purposes.
    ///
    /// Returns the index of the newly created `Tip` record.
    pub fn tip(
        env: Env,
        from: Address,
        creator: Address,
        token: Address,
        amount: i128,
        message: String,
    ) -> u64 {
        from.require_auth();

        if amount <= 0 {
            panic_with_error!(env, TipError::InvalidAmount);
        }

        // Verify the creator exists.
        if !env.storage().instance().has(&DataKey::Profile(creator.clone())) {
            panic_with_error!(env, TipError::CreatorNotFound);
        }

        // 1. Transfer tokens from sender → this contract.
        let token_client = token::Client::new(&env, &token);
        token_client.transfer(&from, &env.current_contract_address(), &amount);

        // 2. Credit the creator's internal balance.
        let balance_key = DataKey::Balance(creator.clone(), token.clone());
        let current_balance: i128 = env
            .storage()
            .persistent()
            .get(&balance_key)
            .unwrap_or(0_i128);
        env.storage()
            .persistent()
            .set(&balance_key, &(current_balance + amount));

        // 3. Record the tip.
        let tip_count_key = DataKey::TipCount(creator.clone());
        let index: u64 = env.storage().instance().get(&tip_count_key).unwrap_or(0);
        let tip = Tip {
            from: from.clone(),
            token: token.clone(),
            amount,
            message,
            timestamp: env.ledger().timestamp(),
        };
        env.storage()
            .persistent()
            .set(&DataKey::Tip(creator.clone(), index), &tip);
        env.storage()
            .instance()
            .set(&tip_count_key, &(index + 1));

        // 4. Emit event.
        env.events().publish(
            (EVENT_TIP_SENT, from.clone()),
            (creator, token, amount, index),
        );

        index
    }

    // -----------------------------------------------------------------------
    // Withdrawal
    // -----------------------------------------------------------------------

    /// Withdraw a given amount of a specific token from the caller's
    /// accumulated tips.  The caller must be a registered creator.
    pub fn withdraw(env: Env, caller: Address, token: Address, amount: i128) {
        caller.require_auth();

        if amount <= 0 {
            panic_with_error!(env, TipError::InvalidAmount);
        }

        let balance_key = DataKey::Balance(caller.clone(), token.clone());
        let current_balance: i128 = env
            .storage()
            .persistent()
            .get(&balance_key)
            .unwrap_or(0);

        if current_balance < amount {
            panic_with_error!(env, TipError::InsufficientBalance);
        }

        // Transfer tokens from this contract to the creator.
        let token_client = token::Client::new(&env, &token);
        token_client.transfer(&env.current_contract_address(), &caller, &amount);

        // Update balance.
        let remaining = current_balance - amount;
        if remaining > 0 {
            env.storage()
                .persistent()
                .set(&balance_key, &remaining);
        } else {
            env.storage().persistent().remove(&balance_key);
        }

        // Emit event.
        env.events().publish(
            (EVENT_WITHDRAW, caller.clone()),
            (token, amount),
        );
    }



    // -----------------------------------------------------------------------
    // View functions
    // -----------------------------------------------------------------------

    /// Return the `CreatorProfile` for the given address, or `None` if the
    /// address is not yet registered.
    pub fn get_profile(env: Env, address: Address) -> Option<CreatorProfile> {
        env.storage().instance().get(&DataKey::Profile(address))
    }

    /// Return the creator `Address` that owns the given username, or `None`.
    pub fn get_creator_from_username(env: Env, username: Symbol) -> Option<Address> {
        env.storage()
            .instance()
            .get(&DataKey::UsernameToAddress(username))
    }

    /// Return the current balance of a specific token held for a creator.
    pub fn get_balance(env: Env, creator: Address, token: Address) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::Balance(creator, token))
            .unwrap_or(0)
    }

    /// Return the total number of tips a creator has ever received.
    pub fn get_tip_count(env: Env, creator: Address) -> u64 {
        env.storage()
            .instance()
            .get(&DataKey::TipCount(creator))
            .unwrap_or(0)
    }

    /// Return a specific `Tip` record by its index.
    pub fn get_tip(env: Env, creator: Address, index: u64) -> Option<Tip> {
        env.storage()
            .persistent()
            .get(&DataKey::Tip(creator, index))
    }

    /// Return whether the given address is a registered creator.
    pub fn is_creator(env: Env, address: Address) -> bool {
        env.storage().instance().has(&DataKey::Profile(address))
    }

    /// Return whether a given username has already been taken.
    pub fn is_username_taken(env: Env, username: Symbol) -> bool {
        env.storage()
            .instance()
            .has(&DataKey::UsernameToAddress(username))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod test;
