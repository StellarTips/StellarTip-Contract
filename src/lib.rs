#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype, panic_with_error, token, Address, Env, Map, String,
    Symbol, Vec,
};

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
    /// Maximum number of creators that can register. `0` disables the limit.
    MaxCreators,
    /// Maximum number of tips per creator that will be recorded. `0`
    /// disables the limit.
    MaxTipsPerCreator,
    /// Minimum tip amount in token base units. `0` disables the minimum.
    MinTipAmount,
    /// Total number of currently-registered creators. Maintained alongside
    /// `MaxCreators` for efficient cap enforcement.
    CreatorCount,
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
    /// Set of tokens a creator has received tips in.
    /// Stored as a `Map<Address, ()>` so that per-token membership checks,
    /// inserts, and removals run in O(log n) instead of the O(n) linear
    /// scan required by a `Vec<Address>`.
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
        /// Raised when an admin-configured cap (`MaxCreators` or
        /// `MaxTipsPerCreator`) has been reached.
        CapExceeded = 14,
        /// Returned when a non-zero `fee_bps` is configured but the
        /// `FeeRecipient` storage key is missing (e.g. corrupted state).
        FeeRecipientNotSet = 15,
        /// Returned when `amount` is positive but below the configured
        /// `MinTipAmount`.
        BelowMinimum = 16,
    }
}

use error::TipError;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Current contract version for client compatibility.
///
/// v3 introduces a configurable minimum tip amount (`MinTipAmount`) to prevent
/// dust attacks, along with `set_min_tip_amount()`, `get_min_tip_amount()`, and
/// a new `init()` parameter. Clients should use `get_contract_version()` to
/// detect the deployed shape.
pub const CONTRACT_VERSION: u32 = 3;

/// Maximum platform fee in basis points (100% = 10_000 bps).
const MAX_FEE_BPS: u32 = 10_000;

/// Display name max length in bytes.
const MAX_DISPLAY_NAME_LEN: u32 = 64;
/// Bio max length in bytes.
const MAX_BIO_LEN: u32 = 256;

/// Default cap on total registered creators when one isn't provided by the
/// admin at initialization. Sized to give plenty of headroom for early growth
/// while protecting against unbounded instance-storage bloat.
pub const DEFAULT_MAX_CREATORS: u32 = 10_000;

/// Default cap on tip history length per creator when one isn't provided.
/// Bounds the per-creator persistent-storage footprint.
pub const DEFAULT_MAX_TIPS_PER_CREATOR: u32 = 10_000;

/// Default minimum tip amount in token base units.
///
/// A value of `1` is equivalent to the existing `amount > 0` guard and
/// preserves all prior behaviour. This default provides no meaningful dust
/// protection on its own — admins should raise it to a token-appropriate
/// threshold after deployment.  `0` disables the minimum entirely.
pub const DEFAULT_MIN_TIP_AMOUNT: i128 = 1;

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

/// Emitted when the admin is changed.
const EVENT_ADMIN_CHANGED: Symbol = soroban_sdk::symbol_short!("ADMC");

/// Emitted when the fee recipient is changed.
const EVENT_FEE_RECIPIENT_CHANGED: Symbol = soroban_sdk::symbol_short!("FERC");

/// Emitted when the admin-configured creator cap is updated.
const EVENT_MAX_CREATORS_CHANGED: Symbol = soroban_sdk::symbol_short!("CAPMC");

/// Emitted when the admin-configured per-creator tip cap is updated.
const EVENT_MAX_TIPS_CHANGED: Symbol = soroban_sdk::symbol_short!("CAPMT");
/// Emitted when the admin-configured minimum tip amount is updated.
const EVENT_MIN_TIP_CHANGED: Symbol = soroban_sdk::symbol_short!("MINTC");
/// Emitted when the contract is initialized.
const EVENT_INIT: Symbol = soroban_sdk::symbol_short!("INIT");

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
    let is_paused: bool = env.storage().instance().get(&DataKey::Paused).unwrap_or(false);
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

    /// Initialize the contract with an admin, fee recipient, platform fee,
    /// storage-bloat caps, and a minimum tip amount.
    ///
    /// `max_creators` and `max_tips_per_creator` are the hard caps enforced on
    /// `register()` and `tip()` respectively. A value of `0` disables the
    /// corresponding cap ("unlimited"). The defaults
    /// [`DEFAULT_MAX_CREATORS`] and [`DEFAULT_MAX_TIPS_PER_CREATOR`] are
    /// sensible starting points when no admin preference is known.
    ///
    /// `min_tip_amount` is the minimum token amount accepted by `tip()`.
    /// A value of `0` disables the minimum. [`DEFAULT_MIN_TIP_AMOUNT`] (= 1)
    /// preserves prior behaviour; raise it post-deployment for meaningful dust
    /// protection.
    ///
    /// # Arguments
    /// * `caller` – Address that becomes the admin (must authorize).
    /// * `fee_recipient` – Address that receives platform fees.
    /// * `fee_bps` – Platform fee in basis points (0–10_000).
    /// * `max_creators` – Cap on total registered creators (`0` = unlimited).
    /// * `max_tips_per_creator` – Cap on tip history per creator (`0` = unlimited).
    /// * `min_tip_amount` – Minimum tip in token base units (`0` = no minimum).
    pub fn init(
        env: Env,
        caller: Address,
        fee_recipient: Address,
        fee_bps: u32,
        max_creators: u32,
        max_tips_per_creator: u32,
        min_tip_amount: i128,
    ) {
        caller.require_auth();
        if env.storage().instance().has(&DataKey::Admin) {
            panic_with_error!(env, TipError::AlreadyInitialized);
        }
        if fee_bps > MAX_FEE_BPS {
            panic_with_error!(env, TipError::InvalidInput);
        }
        if min_tip_amount < 0 {
            panic_with_error!(env, TipError::InvalidInput);
        }
        if fee_recipient == env.current_contract_address() {
            panic_with_error!(env, TipError::InvalidInput);
        }
        env.storage().instance().set(&DataKey::Admin, &caller);
        env.storage().instance().set(&DataKey::FeeRecipient, &fee_recipient);
        env.storage().instance().set(&DataKey::FeeBps, &fee_bps);
        env.storage().instance().set(&DataKey::Paused, &false);
        env.storage().instance().set(&DataKey::MaxCreators, &max_creators);
        env.storage().instance().set(&DataKey::MaxTipsPerCreator, &max_tips_per_creator);
        env.storage().instance().set(&DataKey::MinTipAmount, &min_tip_amount);
        env.storage().instance().set(&DataKey::CreatorCount, &0u32);
        extend_instance_ttl(&env);
        env.events().publish((EVENT_INIT, caller), (fee_recipient, fee_bps));
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
        env.events().publish((EVENT_ADMIN_CHANGED, caller), new_admin);
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
        if fee_recipient == env.current_contract_address() {
            panic_with_error!(env, TipError::InvalidInput);
        }
        env.storage().instance().set(&DataKey::FeeRecipient, &fee_recipient);
        extend_instance_ttl(&env);
        env.events().publish((EVENT_FEE_RECIPIENT_CHANGED, caller), fee_recipient);
    }

    /// Update the maximum number of creators that can register. A value of
    /// `0` disables the cap (unlimited). Only admin can call.
    ///
    /// Lowering the cap below the current creator count does **not**
    /// retroactively remove any creator; it only blocks new registrations
    /// until the count drops (via `unregister`) back below the cap.
    pub fn set_max_creators(env: Env, caller: Address, max_creators: u32) {
        caller.require_auth();
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| panic_with_error!(env, TipError::NotInitialized));
        if caller != admin {
            panic_with_error!(env, TipError::NotAuthorized);
        }
        env.storage().instance().set(&DataKey::MaxCreators, &max_creators);
        extend_instance_ttl(&env);
        env.events().publish((EVENT_MAX_CREATORS_CHANGED, caller), max_creators);
    }

    /// Update the maximum number of tips recorded per creator. A value of
    /// `0` disables the cap (unlimited). Only admin can call.
    ///
    /// Lowering the cap below the current tip count for any creator does
    /// **not** delete historical tips; it only blocks new tips for that
    /// creator until the admin raises the cap again.
    pub fn set_max_tips_per_creator(env: Env, caller: Address, max_tips: u32) {
        caller.require_auth();
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| panic_with_error!(env, TipError::NotInitialized));
        if caller != admin {
            panic_with_error!(env, TipError::NotAuthorized);
        }
        env.storage().instance().set(&DataKey::MaxTipsPerCreator, &max_tips);
        extend_instance_ttl(&env);
        env.events().publish((EVENT_MAX_TIPS_CHANGED, caller), max_tips);
    }

    /// Update the minimum tip amount in token base units. A value of `0`
    /// disables the minimum entirely. Only admin can call.
    ///
    /// Raising the minimum does **not** affect existing tip records; it only
    /// gates future calls to `tip()`.
    pub fn set_min_tip_amount(env: Env, caller: Address, min_tip_amount: i128) {
        caller.require_auth();
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| panic_with_error!(env, TipError::NotInitialized));
        if caller != admin {
            panic_with_error!(env, TipError::NotAuthorized);
        }
        if min_tip_amount < 0 {
            panic_with_error!(env, TipError::InvalidInput);
        }
        env.storage().instance().set(&DataKey::MinTipAmount, &min_tip_amount);
        extend_instance_ttl(&env);
        env.events().publish((EVENT_MIN_TIP_CHANGED, caller), min_tip_amount);
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

        // Per-call checks first so the caller sees the most specific error
        // (e.g. `CreatorAlreadyExists`) before any global cap is consulted.
        // Each address can only register once.
        if env.storage().instance().has(&DataKey::Profile(caller.clone())) {
            panic_with_error!(env, TipError::CreatorAlreadyExists);
        }
        // Each username must be unique.
        if env.storage().instance().has(&DataKey::UsernameToAddress(username.clone())) {
            panic_with_error!(env, TipError::UsernameTaken);
        }

        // Enforce the global creator cap (skip if `MaxCreators == 0`).
        let max_creators: u32 = env.storage().instance().get(&DataKey::MaxCreators).unwrap_or(0);
        let creator_count: u32 = env.storage().instance().get(&DataKey::CreatorCount).unwrap_or(0);
        if max_creators > 0 && creator_count >= max_creators {
            panic_with_error!(env, TipError::CapExceeded);
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

        // Bump the global creator count last, after every other check has
        // succeeded, so we never dirty it on a failed registration.
        env.storage().instance().set(&DataKey::CreatorCount, &(creator_count + 1));

        env.events()
            .publish((EVENT_CREATOR_REGISTERED, caller), (profile.username, profile.registered_at));
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
        if let Some(tokens) = env.storage().persistent().get::<_, Map<Address, ()>>(&tokens_key) {
            for token in tokens.keys() {
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
        env.storage().instance().remove(&DataKey::Profile(caller.clone())); // Decrement the global creator count now that the profile is gone.
        let current_count: u32 = env.storage().instance().get(&DataKey::CreatorCount).unwrap_or(0);
        if current_count > 0 {
            env.storage().instance().set(&DataKey::CreatorCount, &(current_count - 1));
        }

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
        check_initialized_and_not_paused(&env);

        if amount <= 0 {
            panic_with_error!(env, TipError::InvalidAmount);
        }

        let min_tip_amount: i128 =
            env.storage().instance().get(&DataKey::MinTipAmount).unwrap_or(0);
        if min_tip_amount > 0 && amount < min_tip_amount {
            panic_with_error!(env, TipError::BelowMinimum);
        }

        // Verify the creator exists.
        if !env.storage().instance().has(&DataKey::Profile(creator.clone())) {
            panic_with_error!(env, TipError::CreatorNotFound);
        }

        // Enforce the per-creator tip-history cap. The `TipCount` value
        // already lives in persistent storage, so we read it once and use
        // it both for the cap check and as the new tip's index below.
        let max_tips: u32 = env.storage().instance().get(&DataKey::MaxTipsPerCreator).unwrap_or(0);
        let tip_count_key = DataKey::TipCount(creator.clone());
        let index: u64 = env.storage().persistent().get(&tip_count_key).unwrap_or(0);
        if max_tips > 0 && index >= max_tips as u64 {
            panic_with_error!(env, TipError::CapExceeded);
        }

        let fee_bps: u32 = env.storage().instance().get(&DataKey::FeeBps).unwrap_or(0);
        let fee = (amount * (fee_bps as i128)) / (MAX_FEE_BPS as i128);
        let creator_amount = amount - fee;

        // Fail-fast: if a non-zero fee is configured but the fee recipient is
        // not configured (corrupted / unset storage), abort before touching
        // external token contracts.
        let opt_fee_recipient: Option<Address> =
            env.storage().instance().get(&DataKey::FeeRecipient);
        if fee_bps > 0 && opt_fee_recipient.is_none() {
            panic_with_error!(env, TipError::FeeRecipientNotSet);
        }

        // 1. Transfer tokens from sender → this contract.
        let token_client = token::Client::new(&env, &token);
        token_client.transfer(&from, &env.current_contract_address(), &amount);

        // 2. Forward fee to recipient.
        if fee > 0 {
            // Safe to unwrap: validated above when fee_bps > 0. Use
            // `unwrap_or_else` defensively to surface a clean contract error
            // rather than a raw panic if storage ever goes missing between
            // the check and here.
            let fee_recipient: Address = opt_fee_recipient
                .unwrap_or_else(|| panic_with_error!(env, TipError::FeeRecipientNotSet));
            token_client.transfer(&env.current_contract_address(), &fee_recipient, &fee);
        }

        // 3. Credit the creator's internal balance.
        let balance_key = DataKey::Balance(creator.clone(), token.clone());
        let current_balance: i128 = env.storage().persistent().get(&balance_key).unwrap_or(0_i128);
        env.storage().persistent().set(&balance_key, &(current_balance + creator_amount));
        extend_persistent_ttl(&env, &balance_key);

        // 4. Track token for creator. The token set is stored as a
        //    `Map<Address, ()>` so membership checks and inserts run in
        //    O(log n) regardless of how many tokens the creator already has.
        let tokens_key = DataKey::CreatorTokens(creator.clone());
        let mut tokens: Map<Address, ()> =
            env.storage().persistent().get(&tokens_key).unwrap_or_else(|| Map::new(&env));
        if !tokens.contains_key(token.clone()) {
            tokens.set(token.clone(), ());
            env.storage().persistent().set(&tokens_key, &tokens);
        }
        extend_persistent_ttl(&env, &tokens_key);

        // 5. Record the tip. (Note: `index` and `tip_count_key` were read
        // above so we can enforce the per-creator tip cap before any token
        // transfer or storage write occurs.)
        let tip = Tip {
            from: from.clone(),
            token: token.clone(),
            amount,
            message,
            timestamp: env.ledger().timestamp(),
        };
        env.storage().persistent().set(&DataKey::Tip(creator.clone(), index), &tip);
        extend_persistent_ttl(&env, &DataKey::Tip(creator.clone(), index));
        env.storage().persistent().set(&tip_count_key, &(index + 1));
        extend_persistent_ttl(&env, &tip_count_key);

        // 6. Emit event.
        env.events().publish((EVENT_TIP_SENT, from.clone()), (creator, token, amount, fee, index));

        index
    }

    // -----------------------------------------------------------------------
    // Withdrawal
    // -----------------------------------------------------------------------

    /// Withdraw a given amount of a specific token from the caller's
    /// accumulated tips.  The caller must be a registered creator.
    pub fn withdraw(env: Env, caller: Address, token: Address, amount: i128) {
        caller.require_auth();
        check_initialized_and_not_paused(&env);

        // Verify caller is a registered creator.
        if !env.storage().instance().has(&DataKey::Profile(caller.clone())) {
            panic_with_error!(env, TipError::CreatorNotFound);
        }

        if amount <= 0 {
            panic_with_error!(env, TipError::InvalidAmount);
        }

        let balance_key = DataKey::Balance(caller.clone(), token.clone());
        let current_balance: i128 = env.storage().persistent().get(&balance_key).unwrap_or(0);

        if current_balance < amount {
            panic_with_error!(env, TipError::InsufficientBalance);
        }

        // Transfer tokens from this contract to the creator.
        let token_client = token::Client::new(&env, &token);
        token_client.transfer(&env.current_contract_address(), &caller, &amount);

        // Update balance.
        let remaining = current_balance - amount;
        let tokens_key = DataKey::CreatorTokens(caller.clone());
        if remaining > 0 {
            env.storage().persistent().set(&balance_key, &remaining);
            extend_persistent_ttl(&env, &balance_key);
            extend_persistent_ttl(&env, &tokens_key);
        } else {
            env.storage().persistent().remove(&balance_key);
            // Remove the token from the creator's tracked token set in
            // O(log n). The host-backed `Map` performs this lookup and
            // removal directly without scanning every entry.
            let mut tokens: Map<Address, ()> =
                env.storage().persistent().get(&tokens_key).unwrap_or_else(|| Map::new(&env));
            if tokens.remove(token.clone()).is_some() {
                if tokens.is_empty() {
                    env.storage().persistent().remove(&tokens_key);
                } else {
                    env.storage().persistent().set(&tokens_key, &tokens);
                    extend_persistent_ttl(&env, &tokens_key);
                }
            }
        }

        // Emit event.
        env.events().publish((EVENT_WITHDRAW, caller.clone()), (token, amount));
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
        env.storage().instance().get(&DataKey::UsernameToAddress(username))
    }

    /// Return the current balance of a specific token held for a creator.
    pub fn get_balance(env: Env, creator: Address, token: Address) -> i128 {
        env.storage().persistent().get(&DataKey::Balance(creator, token)).unwrap_or(0)
    }

    /// Return the total number of tips a creator has ever received.
    pub fn get_tip_count(env: Env, creator: Address) -> u64 {
        env.storage().persistent().get(&DataKey::TipCount(creator)).unwrap_or(0)
    }

    /// Return a specific `Tip` record by its index.
    pub fn get_tip(env: Env, creator: Address, index: u64) -> Option<Tip> {
        env.storage().persistent().get(&DataKey::Tip(creator, index))
    }

    /// Return a paginated list of tips for a creator.
    pub fn get_tips(env: Env, creator: Address, start: u64, limit: u64) -> Vec<Tip> {
        let mut results = Vec::new(&env);
        let count: u64 =
            env.storage().persistent().get(&DataKey::TipCount(creator.clone())).unwrap_or(0);
        let end = (start + limit).min(count);
        for i in start..end {
            let key = DataKey::Tip(creator.clone(), i);
            if let Some(tip) = env.storage().persistent().get(&key) {
                results.push_back(tip);
            }
        }
        results
    }

    /// Return the `CreatorProfile` for a given username.
    pub fn get_profile_by_username(env: Env, username: Symbol) -> Option<CreatorProfile> {
        if let Some(addr) = env.storage().instance().get(&DataKey::UsernameToAddress(username)) {
            env.storage().instance().get(&DataKey::Profile(addr))
        } else {
            None
        }
    }

    /// Return whether the given address is a registered creator.
    pub fn is_creator(env: Env, address: Address) -> bool {
        env.storage().instance().has(&DataKey::Profile(address))
    }

    /// Return whether a given username has already been taken.
    pub fn is_username_taken(env: Env, username: Symbol) -> bool {
        env.storage().instance().has(&DataKey::UsernameToAddress(username))
    }

    /// Return the list of tokens a creator has received tips in.
    pub fn get_all_tokens(env: Env, creator: Address) -> Vec<Address> {
        match env
            .storage()
            .persistent()
            .get::<_, Map<Address, ()>>(&DataKey::CreatorTokens(creator))
        {
            Some(tokens) => tokens.keys(),
            None => Vec::new(&env),
        }
    }

    /// Return the contract version.
    pub fn get_contract_version(env: Env) -> u32 {
        let _ = env; // suppress unused warning when version is a const
        CONTRACT_VERSION
    }

    /// Return the admin address.
    pub fn get_admin(env: Env) -> Option<Address> {
        env.storage().instance().get(&DataKey::Admin)
    }

    /// Return whether the contract is paused.
    pub fn is_paused(env: Env) -> bool {
        env.storage().instance().get(&DataKey::Paused).unwrap_or(false)
    }

    /// Return the current platform fee in basis points.
    pub fn get_fee_percentage(env: Env) -> u32 {
        env.storage().instance().get(&DataKey::FeeBps).unwrap_or(0)
    }

    /// Return the fee recipient address.
    pub fn get_fee_recipient(env: Env) -> Option<Address> {
        env.storage().instance().get(&DataKey::FeeRecipient)
    }

    /// Return the configured creator cap. `0` means the cap is disabled
    /// (unlimited registrations allowed).
    pub fn get_max_creators(env: Env) -> u32 {
        env.storage().instance().get(&DataKey::MaxCreators).unwrap_or(0)
    }

    /// Return the configured per-creator tip-history cap. `0` means the cap
    /// is disabled (unlimited tips per creator).
    pub fn get_max_tips_per_creator(env: Env) -> u32 {
        env.storage().instance().get(&DataKey::MaxTipsPerCreator).unwrap_or(0)
    }

    /// Return the configured minimum tip amount in token base units. `0` means
    /// the minimum is disabled and any positive amount is accepted.
    pub fn get_min_tip_amount(env: Env) -> i128 {
        env.storage().instance().get(&DataKey::MinTipAmount).unwrap_or(0)
    }

    /// Return the current count of registered creators. Tracked alongside
    /// `MaxCreators` so cap enforcement is O(1).
    pub fn get_creator_count(env: Env) -> u32 {
        env.storage().instance().get(&DataKey::CreatorCount).unwrap_or(0)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod test;

#[cfg(test)]
mod fuzz;
