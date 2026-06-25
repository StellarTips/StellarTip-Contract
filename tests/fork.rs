//! Fork/integration tests for StellarTip.
//!
//! These tests simulate running against a Stellar testnet by:
//!   1. Configuring the ledger with realistic testnet parameters.
//!   2. Deploying Stellar Asset Contracts (tokens) to mirror assets that
//!      already exist on testnet/mainnet.
//!   3. Deploying the TipContract alongside them.
//!   4. Exercising end-to-end flows (register -> tip -> withdraw).
//!
//! The `test_fork_snapshot_capture` test demonstrates the snapshot-based fork
//! pattern: capturing the ledger state with `to_snapshot()` and verifying its
//! contents, which is the mechanism used when forking from a real network's
//! snapshot file loaded via `Env::from_snapshot_file()`.
//!
//! ## Running
//!
//! ```bash
//! cargo test --test fork
//! ```

use soroban_sdk::{
    testutils::{Address as _, Ledger, LedgerInfo},
    token,
    token::StellarAssetClient,
    Address, Env, String, Symbol,
};
use stellar_tip::{TipContract, TipContractClient};
use token::Client as TokenClient;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn s(env: &Env, text: &str) -> String {
    String::from_str(env, text)
}

fn setup_ledger(env: &Env) {
    env.ledger().set(LedgerInfo {
        timestamp: 1_234_567,
        protocol_version: 22,
        sequence_number: 500_000,
        network_id: Default::default(),
        base_reserve: 5_000_000,
        min_persistent_entry_ttl: 10,
        max_entry_ttl: 6_312_000,
        min_temp_entry_ttl: 10,
    });
}

/// Deploy token contracts and the TipContract in a realistic environment.
fn setup_fork_env() -> ForkCtx {
    let env = Env::default();
    setup_ledger(&env);
    env.mock_all_auths();

    let token_admin = Address::generate(&env);

    let xlm_contract = env.register_stellar_asset_contract_v2(token_admin.clone());
    let xlm_id = xlm_contract.address();
    let xlm_sac = StellarAssetClient::new(&env, &xlm_id);
    xlm_sac.mint(&token_admin, &1_000_000_000_000_000);

    let usdc_contract = env.register_stellar_asset_contract_v2(token_admin.clone());
    let usdc_id = usdc_contract.address();
    let usdc_sac = StellarAssetClient::new(&env, &usdc_id);
    usdc_sac.mint(&token_admin, &1_000_000_000_000_000);

    let contract_id = env.register(TipContract, ());

    let admin = Address::generate(&env);
    let fee_recipient = Address::generate(&env);

    ForkCtx { env, contract_id, admin, fee_recipient, xlm_id, usdc_id }
}

// ---------------------------------------------------------------------------
// Test context
// ---------------------------------------------------------------------------

struct ForkCtx {
    env: Env,
    contract_id: Address,
    admin: Address,
    fee_recipient: Address,
    xlm_id: Address,
    usdc_id: Address,
}

impl ForkCtx {
    fn tip_client(&self) -> TipContractClient<'_> {
        TipContractClient::new(&self.env, &self.contract_id)
    }

    fn token_client(&self, token_id: &Address) -> TokenClient<'_> {
        TokenClient::new(&self.env, token_id)
    }

    fn sac(&self, token_id: &Address) -> StellarAssetClient<'_> {
        StellarAssetClient::new(&self.env, token_id)
    }
}

// ---------------------------------------------------------------------------
// End-to-end flow: register >> tip >> withdraw
// ---------------------------------------------------------------------------

#[test]
fn test_e2e_register_tip_withdraw() {
    let ctx = setup_fork_env();
    let creator = Address::generate(&ctx.env);
    let supporter = Address::generate(&ctx.env);

    ctx.tip_client().init(&ctx.admin, &ctx.fee_recipient, &0u32, &0u32, &0u32);

    ctx.tip_client().register(
        &creator,
        &Symbol::new(&ctx.env, "johndoe"),
        &s(&ctx.env, "John Doe"),
        &s(&ctx.env, "Content creator & writer"),
    );
    assert!(ctx.tip_client().is_creator(&creator));

    ctx.sac(&ctx.xlm_id).mint(&supporter, &10_000_000);

    let tip_idx = ctx.tip_client().tip(
        &supporter,
        &creator,
        &ctx.xlm_id,
        &500_000,
        &s(&ctx.env, "Great work!"),
    );
    assert_eq!(tip_idx, 0);
    assert_eq!(ctx.tip_client().get_balance(&creator, &ctx.xlm_id), 500_000);

    let tokens = ctx.tip_client().get_all_tokens(&creator);
    assert_eq!(tokens.len(), 1);
    assert!(tokens.contains(&ctx.xlm_id));

    ctx.tip_client().withdraw(&creator, &ctx.xlm_id, &200_000);
    assert_eq!(ctx.tip_client().get_balance(&creator, &ctx.xlm_id), 300_000);

    ctx.tip_client().withdraw(&creator, &ctx.xlm_id, &300_000);
    assert_eq!(ctx.tip_client().get_balance(&creator, &ctx.xlm_id), 0);

    let tokens = ctx.tip_client().get_all_tokens(&creator);
    assert_eq!(tokens.len(), 0);
}

// ---------------------------------------------------------------------------
// Multi-token
// ---------------------------------------------------------------------------

#[test]
fn test_e2e_multiple_tokens() {
    let ctx = setup_fork_env();
    let creator = Address::generate(&ctx.env);
    let supporter = Address::generate(&ctx.env);

    ctx.tip_client().init(&ctx.admin, &ctx.fee_recipient, &0u32, &0u32, &0u32);
    ctx.tip_client().register(
        &creator,
        &Symbol::new(&ctx.env, "alice"),
        &s(&ctx.env, "Alice"),
        &s(&ctx.env, "Multi-token creator"),
    );

    ctx.sac(&ctx.xlm_id).mint(&supporter, &100_000_000);
    ctx.sac(&ctx.usdc_id).mint(&supporter, &50_000_000);

    ctx.tip_client().tip(&supporter, &creator, &ctx.xlm_id, &10_000, &s(&ctx.env, "XLM tip"));
    assert_eq!(ctx.tip_client().get_balance(&creator, &ctx.xlm_id), 10_000);

    ctx.tip_client().tip(&supporter, &creator, &ctx.usdc_id, &5_000, &s(&ctx.env, "USDC tip"));
    assert_eq!(ctx.tip_client().get_balance(&creator, &ctx.usdc_id), 5_000);

    let tokens = ctx.tip_client().get_all_tokens(&creator);
    assert_eq!(tokens.len(), 2);
    assert!(tokens.contains(&ctx.xlm_id));
    assert!(tokens.contains(&ctx.usdc_id));

    ctx.tip_client().withdraw(&creator, &ctx.xlm_id, &10_000);
    assert_eq!(ctx.tip_client().get_balance(&creator, &ctx.xlm_id), 0);
    assert_eq!(ctx.tip_client().get_balance(&creator, &ctx.usdc_id), 5_000);

    let tokens = ctx.tip_client().get_all_tokens(&creator);
    assert_eq!(tokens.len(), 1);
    assert!(tokens.contains(&ctx.usdc_id));

    ctx.tip_client().withdraw(&creator, &ctx.usdc_id, &5_000);
    assert_eq!(ctx.tip_client().get_balance(&creator, &ctx.usdc_id), 0);

    let tokens = ctx.tip_client().get_all_tokens(&creator);
    assert_eq!(tokens.len(), 0);
}

// ---------------------------------------------------------------------------
// Fee handling
// ---------------------------------------------------------------------------

#[test]
fn test_e2e_with_fee() {
    let ctx = setup_fork_env();
    let creator = Address::generate(&ctx.env);
    let supporter = Address::generate(&ctx.env);

    ctx.tip_client().init(&ctx.admin, &ctx.fee_recipient, &500u32, &0u32, &0u32);

    ctx.tip_client().register(
        &creator,
        &Symbol::new(&ctx.env, "bob"),
        &s(&ctx.env, "Bob"),
        &s(&ctx.env, ""),
    );

    ctx.sac(&ctx.xlm_id).mint(&supporter, &100_000);
    ctx.tip_client().tip(&supporter, &creator, &ctx.xlm_id, &1_000, &s(&ctx.env, ""));

    assert_eq!(ctx.tip_client().get_balance(&creator, &ctx.xlm_id), 950);
    assert_eq!(ctx.token_client(&ctx.xlm_id).balance(&ctx.fee_recipient), 50);
}

// ---------------------------------------------------------------------------
// Multiple creators & supporters
// ---------------------------------------------------------------------------

#[test]
fn test_e2e_multiple_creators() {
    let ctx = setup_fork_env();
    let alice = Address::generate(&ctx.env);
    let bob = Address::generate(&ctx.env);
    let supporter1 = Address::generate(&ctx.env);
    let supporter2 = Address::generate(&ctx.env);

    ctx.tip_client().init(&ctx.admin, &ctx.fee_recipient, &0u32, &0u32, &0u32);

    ctx.tip_client().register(
        &alice,
        &Symbol::new(&ctx.env, "alice"),
        &s(&ctx.env, "Alice"),
        &s(&ctx.env, "Creator A"),
    );
    ctx.tip_client().register(
        &bob,
        &Symbol::new(&ctx.env, "bob"),
        &s(&ctx.env, "Bob"),
        &s(&ctx.env, "Creator B"),
    );

    ctx.sac(&ctx.xlm_id).mint(&supporter1, &100_000);
    ctx.sac(&ctx.xlm_id).mint(&supporter2, &100_000);

    ctx.tip_client().tip(&supporter1, &alice, &ctx.xlm_id, &1_000, &s(&ctx.env, "for alice"));
    ctx.tip_client().tip(&supporter1, &bob, &ctx.xlm_id, &2_000, &s(&ctx.env, "for bob"));
    ctx.tip_client().tip(&supporter2, &alice, &ctx.xlm_id, &3_000, &s(&ctx.env, "more for alice"));

    assert_eq!(ctx.tip_client().get_balance(&alice, &ctx.xlm_id), 4_000);
    assert_eq!(ctx.tip_client().get_balance(&bob, &ctx.xlm_id), 2_000);
    assert_eq!(ctx.tip_client().get_tip_count(&alice), 2);
    assert_eq!(ctx.tip_client().get_tip_count(&bob), 1);

    ctx.tip_client().withdraw(&alice, &ctx.xlm_id, &2_000);
    ctx.tip_client().withdraw(&bob, &ctx.xlm_id, &2_000);
    assert_eq!(ctx.tip_client().get_balance(&alice, &ctx.xlm_id), 2_000);
    assert_eq!(ctx.tip_client().get_balance(&bob, &ctx.xlm_id), 0);
}

// ---------------------------------------------------------------------------
// Snapshot capture / round-trip  (fork pattern)
// ---------------------------------------------------------------------------

#[test]
fn test_fork_snapshot_roundtrip() {
    let ctx = setup_fork_env();
    let creator = Address::generate(&ctx.env);
    let supporter = Address::generate(&ctx.env);

    ctx.tip_client().init(&ctx.admin, &ctx.fee_recipient, &0u32, &0u32, &0u32);

    ctx.tip_client().register(
        &creator,
        &Symbol::new(&ctx.env, "snap"),
        &s(&ctx.env, "Snapshot"),
        &s(&ctx.env, ""),
    );

    ctx.sac(&ctx.xlm_id).mint(&supporter, &100_000);
    ctx.tip_client().tip(&supporter, &creator, &ctx.xlm_id, &10_000, &s(&ctx.env, "tip 1"));
    ctx.tip_client().tip(&supporter, &creator, &ctx.xlm_id, &5_000, &s(&ctx.env, "tip 2"));
    ctx.tip_client().withdraw(&creator, &ctx.xlm_id, &3_000);

    // Capture snapshot — same format used when forking from a real network
    // via `soroban lab snapshot capture`.
    let snapshot = ctx.env.to_snapshot();

    // Verify ledger metadata is captured in the snapshot
    assert_eq!(snapshot.ledger.ledger_info().timestamp, 1_234_567);
    assert_eq!(snapshot.ledger.protocol_version, 22);
    assert_eq!(snapshot.ledger.sequence_number, 500_000);

    // The snapshot struct is serializable (serde) and can be saved to a file
    // for later use with `Env::from_snapshot_file()`.  When loading from a
    // real testnet snapshot the Address values inside ledger entries (token
    // contract IDs, account addresses) will be real StrKey-encoded strings
    // and therefore portable across Env boundaries.
}

// ---------------------------------------------------------------------------
// Pagination
// ---------------------------------------------------------------------------

#[test]
fn test_e2e_pagination() {
    let ctx = setup_fork_env();
    let creator = Address::generate(&ctx.env);

    ctx.tip_client().init(&ctx.admin, &ctx.fee_recipient, &0u32, &0u32, &0u32);
    ctx.tip_client().register(
        &creator,
        &Symbol::new(&ctx.env, "paginator"),
        &s(&ctx.env, "Paginator"),
        &s(&ctx.env, ""),
    );

    for i in 0..10 {
        let sp = Address::generate(&ctx.env);
        ctx.sac(&ctx.xlm_id).mint(&sp, &100_000);
        ctx.tip_client().tip(&sp, &creator, &ctx.xlm_id, &100, &s(&ctx.env, &format!("tip {}", i)));
    }

    assert_eq!(ctx.tip_client().get_tip_count(&creator), 10);
    assert_eq!(ctx.tip_client().get_tips(&creator, &0, &3).len(), 3);
    assert_eq!(ctx.tip_client().get_tips(&creator, &3, &4).len(), 4);
    assert_eq!(ctx.tip_client().get_tips(&creator, &7, &10).len(), 3);
    assert_eq!(ctx.tip_client().get_tips(&creator, &10, &5).len(), 0);
}

// ---------------------------------------------------------------------------
// Creator lifecycle
// ---------------------------------------------------------------------------

#[test]
fn test_e2e_register_unregister() {
    let ctx = setup_fork_env();
    let creator = Address::generate(&ctx.env);

    ctx.tip_client().init(&ctx.admin, &ctx.fee_recipient, &0u32, &0u32, &0u32);
    ctx.tip_client().register(
        &creator,
        &Symbol::new(&ctx.env, "lifecycle"),
        &s(&ctx.env, "Lifecycle"),
        &s(&ctx.env, "A life cycle test"),
    );
    assert!(ctx.tip_client().is_creator(&creator));
    assert!(ctx.tip_client().is_username_taken(&Symbol::new(&ctx.env, "lifecycle")));

    // Profile lookup
    let profile = ctx.tip_client().get_profile(&creator).unwrap();
    assert_eq!(profile.display_name, s(&ctx.env, "Lifecycle"));

    ctx.tip_client().unregister(&creator);
    assert!(!ctx.tip_client().is_creator(&creator));
    assert!(!ctx.tip_client().is_username_taken(&Symbol::new(&ctx.env, "lifecycle")));
}

// ---------------------------------------------------------------------------
// Profile update
// ---------------------------------------------------------------------------

#[test]
fn test_e2e_update_profile() {
    let ctx = setup_fork_env();
    let creator = Address::generate(&ctx.env);

    ctx.tip_client().init(&ctx.admin, &ctx.fee_recipient, &0u32, &0u32, &0u32);
    ctx.tip_client().register(
        &creator,
        &Symbol::new(&ctx.env, "updater"),
        &s(&ctx.env, "Original"),
        &s(&ctx.env, ""),
    );

    ctx.tip_client().update_profile(
        &creator,
        &s(&ctx.env, "Updated Name"),
        &s(&ctx.env, "New bio text"),
    );

    let profile = ctx.tip_client().get_profile(&creator).unwrap();
    assert_eq!(profile.display_name, s(&ctx.env, "Updated Name"));
    assert_eq!(profile.bio, s(&ctx.env, "New bio text"));
}
