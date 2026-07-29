//! # Soroban fork tests against Stellar testnet (issue #88)
//!
//! This module runs the same set of invariants as the unit tests in
//! `src/test.rs`, but against a **forked** testnet state rather than an
//! isolated `Env::default()` host.
//!
//! ## Why fork tests matter
//!
//! Soroban's emulated host is deterministic but not exhaustive. Fork tests
//! catch issues that only surface against real RPC behaviour, host-side
//! authorization verification, actual `symbol_short` truncations, and real
//! reclaim gas costs.
//!
//! ## Prerequisites
//!
//! Fork testing requires soroban-sdk ≥ 22.x (`test_snapshot`
//! infrastructure). This project currently pins soroban-sdk 21.7.7 for
//! stability (see `Cargo.toml`). **The fork tests are gated behind the
//! `fork` feature and ignored by default.**
//!
//! When the SDK is upgraded to 22.x (tracked in issue #88), replace the
//! stubs below with real fork-mode assertions.
//!
//! ## Running (post-upgrade)
//!
//! ```bash
//! export STELLAR_RPC_URL=https://soroban-testnet.stellar.org
//! cargo test --features fork -- --ignored
//! ```

// ---------------------------------------------------------------------------
// Stubs — TODO: replace with real fork tests after SDK 22.x upgrade
// ---------------------------------------------------------------------------

/// Entry point: validate the deployed contract WASM against forked testnet
/// state.  All concrete assertions live in the helper modules below.
#[test]
#[ignore = "Fork tests require soroban-sdk ≥ 22.x (see module-level docs)"]
#[cfg(feature = "fork")]
fn fork_invariants() {
    // TODO(#88): Replace this stub with a real fork-mode test.
    //
    // Expected structure (soroban-sdk 22.x):
    // ```rust,ignore
    // let env = Env::from_snapshot_file("testnet-snapshot.json")?;
    // env.register_contract_wasm_from_file("target/wasm32-unknown-unknown/release/stellar_tip.wasm");
    // // Replay the full unit-test invariant suite against forked state.
    // ```
    //
    // Until then, this test is `#[ignore]`-d so CI passes without a
    // testnet RPC.
}

/// Fork-mode test that replays registration invariants against testnet
/// state.
#[test]
#[ignore = "Fork tests require soroban-sdk ≥ 22.x"]
#[cfg(feature = "fork")]
fn fork_registration_invariants() {
    // TODO(#88): register → assert profile exists → assert creator count.
}

/// Fork-mode test that replays tipping invariants against testnet state.
#[test]
#[ignore = "Fork tests require soroban-sdk ≥ 22.x"]
#[cfg(feature = "fork")]
fn fork_tipping_invariants() {
    // TODO(#88): tip → assert balance updated → assert tip recorded.
}

/// Fork-mode test that replays withdrawal invariants against testnet
/// state.
#[test]
#[ignore = "Fork tests require soroban-sdk ≥ 22.x"]
#[cfg(feature = "fork")]
fn fork_withdrawal_invariants() {
    // TODO(#88): withdraw → assert tokens transferred → assert balance zero.
}

/// Fork-mode test that replays admin-rotation invariants against testnet
/// state.
#[test]
#[ignore = "Fork tests require soroban-sdk ≥ 22.x"]
#[cfg(feature = "fork")]
fn fork_admin_invariants() {
    // TODO(#88): pause → rotate admin → set fee recipient → unpause → tip.
}
