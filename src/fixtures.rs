#![cfg(test)]
//! Test-only token fixtures.
//!
//! # `RebasingToken`
//!
//! A minimal but *conforming* SEP-41 token whose holder balances change
//! without any transfer — the defining property of stETH, Aave aTokens and
//! friends. Holders own **shares**; the nominal balance reported by
//! `balance()` is `shares * num / den`, and `rebase(num, den)` rescales every
//! holder at once without moving a single share.
//!
//! It exists to characterise what `TipContract` does when the invariant
//!
//! ```text
//! sum(internal balances for token T) == T.balance(contract)
//! ```
//!
//! stops holding. `TipContract` credits `Balance(creator, token)` straight
//! from the `amount` argument and never reads `token.balance()`, so a rebase
//! silently desynchronises the two ledgers. See the rebasing-token section of
//! `src/test.rs` and the Residual Risks entry in `SECURITY.md`.
//!
//! ## Why the full `TokenInterface` trait
//!
//! The claim being pinned down is that *a rebasing token is a legal token*.
//! An inherent impl would assert that in a comment; implementing the trait
//! asserts it to the compiler — this fixture does not build unless it presents
//! a complete conforming interface. The five methods `TipContract` never calls
//! panic `NotSupported`, which doubles as a standing audit of the contract's
//! actual token surface: today it uses `transfer` and nothing else.
//!
//! ## Constraints callers must respect
//!
//! - **Use only the exact rebase factors `(1,1)`, `(2,1)`, `(1,2)` and
//!   `(0,1)`, with amounts that are multiples of 2.** Every arithmetic step is
//!   then exact and no assertion depends on truncation. Picking something like
//!   `(3,7)` will produce confusing off-by-one results that are artifacts of
//!   this mock, not of `TipContract`.
//! - **Keep amounts small (≤ 10_000).** `amount * den` is unguarded and can
//!   overflow `i128` for adversarial inputs. Guarding it would add arithmetic
//!   that the tests do not exercise.
//!
//! Error codes start at 900 so they can never collide with a `TipError`
//! (1–16). A `#[should_panic(expected = "#900")]` therefore *positively
//! proves* a revert came from the token layer, after `TipContract`'s own
//! guards passed.

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, panic_with_error, token::TokenInterface,
    Address, Env, String,
};

/// Storage keys for `RebasingToken`.
#[contracttype]
#[derive(Clone)]
pub enum RebasingKey {
    /// Shares held by an address. Nominal balance is `shares * num / den`.
    Shares(Address),
    /// Rebase numerator.
    Num,
    /// Rebase denominator.
    Den,
}

/// Failures raised by `RebasingToken`. Numbered from 900 to stay clear of
/// `TipError` (1–16) so tests can attribute a panic to the token layer.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum RebasingTokenError {
    /// Holder does not own enough shares to cover the transfer.
    InsufficientBalance = 900,
    /// A conforming method that this fixture deliberately does not implement.
    /// Reaching it means `TipContract` grew a new token dependency.
    NotSupported = 901,
    /// `den <= 0`, or `num < 0`, or a share conversion attempted while
    /// `num == 0` (a wiped-out token).
    InvalidRebaseFactor = 902,
}

#[contract]
pub struct RebasingToken;

/// Test knobs. Kept in their own `impl` block so the conforming SEP-41
/// surface below stays visually separate from the fixture's controls.
#[contractimpl]
impl RebasingToken {
    /// Credit `to` with `amount` *nominal* units at the current rebase factor.
    pub fn mint(env: Env, to: Address, amount: i128) {
        let shares = nominal_to_shares(&env, amount);
        let current = shares_of(&env, &to);
        env.storage().persistent().set(&RebasingKey::Shares(to), &(current + shares));
    }

    /// Rescale every holder's nominal balance to `shares * num / den` without
    /// moving any shares. `rebase(0, 1)` wipes the token out.
    pub fn rebase(env: Env, num: i128, den: i128) {
        if num < 0 || den <= 0 {
            panic_with_error!(&env, RebasingTokenError::InvalidRebaseFactor);
        }
        env.storage().instance().set(&RebasingKey::Num, &num);
        env.storage().instance().set(&RebasingKey::Den, &den);
    }
}

#[contractimpl]
impl TokenInterface for RebasingToken {
    fn balance(env: Env, id: Address) -> i128 {
        let (num, den) = factor(&env);
        shares_of(&env, &id) * num / den
    }

    /// Moves `amount` **nominal** units, converting to shares internally —
    /// exactly what a real rebasing token does. Moving shares instead would
    /// mean `tip(1000)` credited 1000 internally while transferring some other
    /// nominal amount, and the divergence under test would be an artifact of a
    /// wrong mock rather than of rebasing.
    fn transfer(env: Env, from: Address, to: Address, amount: i128) {
        from.require_auth();

        // Convert once and compare in share-space. Checking sufficiency in
        // nominal-space and deducting in share-space is two independent
        // truncations, and inputs exist where the check passes but the
        // deduction underflows.
        let shares = nominal_to_shares(&env, amount);
        let from_shares = shares_of(&env, &from);
        if from_shares < shares {
            panic_with_error!(&env, RebasingTokenError::InsufficientBalance);
        }

        let to_shares = shares_of(&env, &to);
        env.storage().persistent().set(&RebasingKey::Shares(from), &(from_shares - shares));
        env.storage().persistent().set(&RebasingKey::Shares(to), &(to_shares + shares));
    }

    fn decimals(_env: Env) -> u32 {
        7
    }

    fn name(env: Env) -> String {
        String::from_str(&env, "Rebasing")
    }

    fn symbol(env: Env) -> String {
        String::from_str(&env, "REBASE")
    }

    // -----------------------------------------------------------------------
    // Unsupported: `TipContract` calls none of these. Leave them panicking —
    // if one ever fires, the contract's token dependency has widened.
    // -----------------------------------------------------------------------

    fn allowance(env: Env, _from: Address, _spender: Address) -> i128 {
        panic_with_error!(&env, RebasingTokenError::NotSupported)
    }

    fn approve(env: Env, _from: Address, _spender: Address, _amount: i128, _expiration: u32) {
        panic_with_error!(&env, RebasingTokenError::NotSupported)
    }

    fn transfer_from(env: Env, _spender: Address, _from: Address, _to: Address, _amount: i128) {
        panic_with_error!(&env, RebasingTokenError::NotSupported)
    }

    fn burn(env: Env, _from: Address, _amount: i128) {
        panic_with_error!(&env, RebasingTokenError::NotSupported)
    }

    fn burn_from(env: Env, _spender: Address, _from: Address, _amount: i128) {
        panic_with_error!(&env, RebasingTokenError::NotSupported)
    }
}

/// Current `(num, den)`. Defaults to `(1, 1)` — a freshly deployed token has
/// not rebased, so nominal balance equals shares.
fn factor(env: &Env) -> (i128, i128) {
    let num: i128 = env.storage().instance().get(&RebasingKey::Num).unwrap_or(1);
    let den: i128 = env.storage().instance().get(&RebasingKey::Den).unwrap_or(1);
    (num, den)
}

fn shares_of(env: &Env, id: &Address) -> i128 {
    env.storage().persistent().get(&RebasingKey::Shares(id.clone())).unwrap_or(0)
}

/// `amount` nominal units expressed in shares. Guards `num == 0` so a wipeout
/// rebase raises `InvalidRebaseFactor` rather than a raw host divide-by-zero.
fn nominal_to_shares(env: &Env, amount: i128) -> i128 {
    let (num, den) = factor(env);
    if num == 0 {
        panic_with_error!(env, RebasingTokenError::InvalidRebaseFactor);
    }
    amount * den / num
}
