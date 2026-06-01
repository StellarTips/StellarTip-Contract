#![cfg(test)]

use soroban_sdk::{
    testutils::{Address as _, Ledger, LedgerInfo},
    token, token::StellarAssetClient, Address, Env, String, Symbol,
};

use token::Client as TokenClient;

use crate::TipContract;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Convenience: create a `String` from a `&str`.
fn s(env: &Env, text: &str) -> String {
    String::from_str(env, text)
}

/// Deploy the TipContract and a Stellar token so we can test real token
/// transfers.
struct TestEnv {
    env: Env,
    contract_id: Address,
    /// Admin / deployer address.
    admin: Address,
    /// Token contract that represents XLM / USDC etc.
    token_id: Address,
}

impl TestEnv {
    fn new() -> Self {
        let env: Env = Env::default();
        env.mock_all_auths();

        // Advance the ledger so timestamps are > 0.
        env.ledger().set(LedgerInfo {
            timestamp: 1000,
            protocol_version: 22,
            sequence_number: 100,
            network_id: Default::default(),
            base_reserve: 10,
            min_persistent_entry_ttl: 10,
            max_entry_ttl: 100_000,
            min_temp_entry_ttl: 10,
        });

        let admin = Address::generate(&env);
        let contract_id = env.register(TipContract, ());

        // Deploy a Stellar Asset Contract (token) using the modern API.
        let token_admin = Address::generate(&env);
        let token_contract = env.register_stellar_asset_contract_v2(token_admin.clone());
        let token_id = token_contract.address();

        // Use StellarAssetClient for minting.
        let sac = StellarAssetClient::new(&env, &token_id);
        sac.mint(&admin, &1_000_000_000);

        TestEnv {
            env,
            contract_id,
            admin,
            token_id,
        }
    }

    fn tip_client(&self) -> crate::TipContractClient {
        crate::TipContractClient::new(&self.env, &self.contract_id)
    }

    fn token_client(&self) -> token::Client {
        token::Client::new(&self.env, &self.token_id)
    }

    fn stellar_client(&self) -> StellarAssetClient {
        StellarAssetClient::new(&self.env, &self.token_id)
    }

    /// Deploy a second token for multi-token testing.
    fn deploy_second_token(&self) -> (Address, TokenClient, StellarAssetClient) {
        let token_admin = Address::generate(&self.env);
        let token_contract = self.env.register_stellar_asset_contract_v2(token_admin.clone());
        let id = token_contract.address();
        let sac = StellarAssetClient::new(&self.env, &id);
        sac.mint(&self.admin, &1_000_000_000);
        let client = TokenClient::new(&self.env, &id);
        (id, client, sac)
    }
}

// ---------------------------------------------------------------------------
// Registration tests
// ---------------------------------------------------------------------------

#[test]
fn test_register_creates_profile() {
    let t = TestEnv::new();
    let alice = Address::generate(&t.env);

    t.tip_client().register(
        &alice,
        &Symbol::new(&t.env, "alice"),
        &s(&t.env, "Alice"),
        &s(&t.env, "Writer"),
    );

    let profile = t.tip_client().get_profile(&alice).unwrap();
    assert_eq!(profile.username, Symbol::new(&t.env, "alice"));
    assert_eq!(profile.display_name, s(&t.env, "Alice"));
    assert_eq!(profile.bio, s(&t.env, "Writer"));
    assert_eq!(profile.registered_at, 1000);

    assert!(t.tip_client().is_creator(&alice));
    assert!(t
        .tip_client()
        .is_username_taken(&Symbol::new(&t.env, "alice")));

    let resolved = t
        .tip_client()
        .get_creator_from_username(&Symbol::new(&t.env, "alice"));
    assert_eq!(resolved, Some(alice));
}

#[test]
#[should_panic(expected = "#1")]
fn test_register_twice_fails() {
    let t = TestEnv::new();
    let alice = Address::generate(&t.env);

    t.tip_client().register(
        &alice,
        &Symbol::new(&t.env, "alice"),
        &s(&t.env, "Alice"),
        &s(&t.env, ""),
    );

    // Second registration for the same address should panic.
    t.tip_client().register(
        &alice,
        &Symbol::new(&t.env, "alice2"),
        &s(&t.env, "A"),
        &s(&t.env, ""),
    );
}

#[test]
#[should_panic(expected = "#3")]
fn test_register_duplicate_username_fails() {
    let t = TestEnv::new();
    let alice = Address::generate(&t.env);
    let bob = Address::generate(&t.env);

    t.tip_client().register(
        &alice,
        &Symbol::new(&t.env, "popstar"),
        &s(&t.env, "A"),
        &s(&t.env, ""),
    );

    // Bob tries to take "popstar".
    t.tip_client().register(
        &bob,
        &Symbol::new(&t.env, "popstar"),
        &s(&t.env, "B"),
        &s(&t.env, ""),
    );
}

// ---------------------------------------------------------------------------
// Tipping tests
// ---------------------------------------------------------------------------

#[test]
fn test_tip_transfers_tokens() {
    let t = TestEnv::new();
    let alice = Address::generate(&t.env);
    let bob = Address::generate(&t.env);

    // Register Alice as a creator.
    t.tip_client().register(
        &alice,
        &Symbol::new(&t.env, "alice"),
        &s(&t.env, "Alice"),
        &s(&t.env, ""),
    );

    // Fund Bob with some tokens.
    t.stellar_client().mint(&bob, &10_000);

    // Bob tips Alice 500 tokens.
    let bob_balance_before = t.token_client().balance(&bob);
    let contract_balance_before = t.token_client().balance(&t.contract_id);

    t.tip_client().tip(
        &bob,
        &alice,
        &t.token_id,
        &500,
        &s(&t.env, "Great work!"),
    );

    // Verify tokens moved: Bob → Contract.
    assert_eq!(
        t.token_client().balance(&bob),
        bob_balance_before - 500
    );
    assert_eq!(
        t.token_client().balance(&t.contract_id),
        contract_balance_before + 500
    );

    // Verify the creator's internal balance.
    let balance = t.tip_client().get_balance(&alice, &t.token_id);
    assert_eq!(balance, 500);
}

#[test]
fn test_tip_records_history() {
    let t = TestEnv::new();
    let alice = Address::generate(&t.env);
    let bob = Address::generate(&t.env);

    t.tip_client().register(
        &alice,
        &Symbol::new(&t.env, "alice"),
        &s(&t.env, "Alice"),
        &s(&t.env, "Writer"),
    );

    t.stellar_client().mint(&bob, &10_000);

    let index = t.tip_client().tip(
        &bob,
        &alice,
        &t.token_id,
        &300,
        &s(&t.env, "💜"),
    );

    assert_eq!(index, 0);
    assert_eq!(t.tip_client().get_tip_count(&alice), 1);

    let tip = t.tip_client().get_tip(&alice, &0).unwrap();
    assert_eq!(tip.from, bob);
    assert_eq!(tip.token, t.token_id);
    assert_eq!(tip.amount, 300);
    assert_eq!(tip.message, s(&t.env, "💜"));
    assert_eq!(tip.timestamp, 1000);

    // Second tip.
    let charlie = Address::generate(&t.env);
    t.stellar_client().mint(&charlie, &10_000);

    let index2 = t.tip_client().tip(
        &charlie,
        &alice,
        &t.token_id,
        &200,
        &s(&t.env, ""),
    );
    assert_eq!(index2, 1);
    assert_eq!(t.tip_client().get_tip_count(&alice), 2);
}

#[test]
#[should_panic(expected = "#2")]
fn test_tip_to_unregistered_creator_fails() {
    let t = TestEnv::new();
    let bob = Address::generate(&t.env);
    let stranger = Address::generate(&t.env);

    t.stellar_client().mint(&bob, &10_000);

    t.tip_client().tip(&bob, &stranger, &t.token_id, &100, &s(&t.env, ""));
}

#[test]
#[should_panic(expected = "#6")]
fn test_tip_zero_amount_fails() {
    let t = TestEnv::new();
    let alice = Address::generate(&t.env);
    let bob = Address::generate(&t.env);

    t.tip_client().register(
        &alice,
        &Symbol::new(&t.env, "alice"),
        &s(&t.env, "A"),
        &s(&t.env, ""),
    );

    t.tip_client().tip(&bob, &alice, &t.token_id, &0, &s(&t.env, ""));
}

// ---------------------------------------------------------------------------
// Withdrawal tests
// ---------------------------------------------------------------------------

#[test]
fn test_withdraw_transfers_tokens_to_creator() {
    let t = TestEnv::new();
    let alice = Address::generate(&t.env);
    let bob = Address::generate(&t.env);

    t.tip_client().register(
        &alice,
        &Symbol::new(&t.env, "alice"),
        &s(&t.env, "Alice"),
        &s(&t.env, ""),
    );

    // Bob tips Alice 1 000 tokens.
    t.stellar_client().mint(&bob, &10_000);
    t.tip_client()
        .tip(&bob, &alice, &t.token_id, &1_000, &s(&t.env, ""));

    // Alice withdraws 400 tokens.
    let alice_balance_before = t.token_client().balance(&alice);

    t.tip_client().withdraw(&alice, &t.token_id, &400);

    assert_eq!(
        t.token_client().balance(&alice),
        alice_balance_before + 400
    );

    // Internal balance reduced.
    assert_eq!(
        t.tip_client().get_balance(&alice, &t.token_id),
        600
    );
}

#[test]
fn test_withdraw_full_balance() {
    let t = TestEnv::new();
    let alice = Address::generate(&t.env);

    t.tip_client().register(
        &alice,
        &Symbol::new(&t.env, "alice"),
        &s(&t.env, "Alice"),
        &s(&t.env, ""),
    );

    let bob = Address::generate(&t.env);
    t.stellar_client().mint(&bob, &10_000);
    t.tip_client()
        .tip(&bob, &alice, &t.token_id, &777, &s(&t.env, ""));

    // Withdraw exactly the balance.
    t.tip_client().withdraw(&alice, &t.token_id, &777);
    assert_eq!(
        t.tip_client().get_balance(&alice, &t.token_id),
        0
    );
}

#[test]
#[should_panic(expected = "#4")]
fn test_withdraw_more_than_balance_fails() {
    let t = TestEnv::new();
    let alice = Address::generate(&t.env);

    t.tip_client().register(
        &alice,
        &Symbol::new(&t.env, "alice"),
        &s(&t.env, "Alice"),
        &s(&t.env, ""),
    );

    t.tip_client().withdraw(&alice, &t.token_id, &100);
}

#[test]
#[should_panic(expected = "#6")]
fn test_withdraw_zero_fails() {
    let t = TestEnv::new();
    let alice = Address::generate(&t.env);

    t.tip_client().register(
        &alice,
        &Symbol::new(&t.env, "alice"),
        &s(&t.env, "A"),
        &s(&t.env, ""),
    );

    t.tip_client().withdraw(&alice, &t.token_id, &0);
}

// ---------------------------------------------------------------------------
// Edge-case: tipping with multiple tokens
// ---------------------------------------------------------------------------

#[test]
fn test_multiple_token_balances() {
    let t = TestEnv::new();

    // Deploy a second token (e.g. USDC).
    let (token2_id, _, _) = t.deploy_second_token();

    let alice = Address::generate(&t.env);
    t.tip_client().register(
        &alice,
        &Symbol::new(&t.env, "alice"),
        &s(&t.env, "Alice"),
        &s(&t.env, ""),
    );

    // Fund a supporter with both tokens.
    let bob = Address::generate(&t.env);
    t.stellar_client().mint(&bob, &100_000);
    let t2_sac = StellarAssetClient::new(&t.env, &token2_id);
    t2_sac.mint(&bob, &50_000);

    // Tip in token 1.
    t.tip_client()
        .tip(&bob, &alice, &t.token_id, &1_000, &s(&t.env, ""));

    // Tip in token 2.
    t.tip_client()
        .tip(&bob, &alice, &token2_id, &500, &s(&t.env, ""));

    assert_eq!(
        t.tip_client().get_balance(&alice, &t.token_id),
        1_000
    );
    assert_eq!(
        t.tip_client().get_balance(&alice, &token2_id),
        500
    );

    // Withdraw from token 2 only.
    t.tip_client().withdraw(&alice, &token2_id, &200);
    assert_eq!(
        t.tip_client().get_balance(&alice, &token2_id),
        300
    );
    assert_eq!(
        t.tip_client().get_balance(&alice, &t.token_id),
        1_000
    );
}
