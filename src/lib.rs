#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, panic_with_error, token, Address, Env, String, Symbol};

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

/// Storage keys used for persistent and instance storage.
#[derive(Clone)]
#[contracttype]
pub enum DataKey {
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
    }
}

use error::TipError;

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

/// Emitted when a new creator registers.
const EVENT_CREATOR_REGISTERED: Symbol = soroban_sdk::symbol_short!("CREG");

/// Emitted when a tip is sent.
const EVENT_TIP_SENT: Symbol = soroban_sdk::symbol_short!("TIP");

/// Emitted when a creator withdraws tokens.
const EVENT_WITHDRAW: Symbol = soroban_sdk::symbol_short!("WDRW");

// ---------------------------------------------------------------------------
// Contract
// ---------------------------------------------------------------------------

#[contract]
pub struct TipContract;

#[contractimpl]
impl TipContract {
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
        env.storage().instance().set(&DataKey::TipCount(caller.clone()), &0u64);

        env.events().publish(
            (EVENT_CREATOR_REGISTERED, caller),
            (profile.username, profile.registered_at),
        );
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
