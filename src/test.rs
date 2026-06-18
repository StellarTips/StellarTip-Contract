#![cfg(test)]

use soroban_sdk::{
    testutils::{Address as _, Ledger, LedgerInfo},
    token,
    token::StellarAssetClient,
    Address, Env, String, Symbol,
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
    /// Fee recipient address.
    fee_recipient: Address,
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
            max_entry_ttl: 1_000_000,
            min_temp_entry_ttl: 10,
        });

        let admin = Address::generate(&env);
        let fee_recipient = Address::generate(&env);
        let contract_id = env.register(TipContract, ());

        // Deploy a Stellar Asset Contract (token) using the modern API.
        let token_admin = Address::generate(&env);
        let token_contract = env.register_stellar_asset_contract_v2(token_admin.clone());
        let token_id = token_contract.address();

        // Use StellarAssetClient for minting.
        let sac = StellarAssetClient::new(&env, &token_id);
        sac.mint(&admin, &1_000_000_000);

        let t = TestEnv { env, contract_id, admin, fee_recipient, token_id };

        // Initialize contract with the library-default caps and no fee.
        t.tip_client().init(
            &t.admin,
            &t.fee_recipient,
            &0u32,
            &crate::DEFAULT_MAX_CREATORS,
            &crate::DEFAULT_MAX_TIPS_PER_CREATOR,
        );

        t
    }

    fn new_with_fee(fee_bps: u32) -> Self {
        let env: Env = Env::default();
        env.mock_all_auths();

        env.ledger().set(LedgerInfo {
            timestamp: 1000,
            protocol_version: 22,
            sequence_number: 100,
            network_id: Default::default(),
            base_reserve: 10,
            min_persistent_entry_ttl: 10,
            max_entry_ttl: 1_000_000,
            min_temp_entry_ttl: 10,
        });

        let admin = Address::generate(&env);
        let fee_recipient = Address::generate(&env);
        let contract_id = env.register(TipContract, ());

        let token_admin = Address::generate(&env);
        let token_contract = env.register_stellar_asset_contract_v2(token_admin.clone());
        let token_id = token_contract.address();

        let sac = StellarAssetClient::new(&env, &token_id);
        sac.mint(&admin, &1_000_000_000);

        let t = TestEnv { env, contract_id, admin, fee_recipient, token_id };

        t.tip_client().init(
            &t.admin,
            &t.fee_recipient,
            &fee_bps,
            &crate::DEFAULT_MAX_CREATORS,
            &crate::DEFAULT_MAX_TIPS_PER_CREATOR,
        );
        t
    }

    /// Build a `TestEnv` from an arbitrary cap configuration so cap-related
    /// tests can exercise low/unlimited values without having to factor out a
    /// custom ledger setup.
    fn new_with_caps(fee_bps: u32, max_creators: u32, max_tips: u32) -> Self {
        let t = Self::new_with_fee(fee_bps);
        t.tip_client()
            .set_max_creators(&t.admin, &max_creators);
        t.tip_client()
            .set_max_tips_per_creator(&t.admin, &max_tips);
        t
    }

    fn tip_client(&self) -> crate::TipContractClient<'_> {
        crate::TipContractClient::new(&self.env, &self.contract_id)
    }

    fn token_client(&self) -> token::Client<'_> {
        token::Client::new(&self.env, &self.token_id)
    }

    fn stellar_client(&self) -> StellarAssetClient<'_> {
        StellarAssetClient::new(&self.env, &self.token_id)
    }

    /// Deploy a second token for multi-token testing.
    fn deploy_second_token(&self) -> (Address, TokenClient<'_>, StellarAssetClient<'_>) {
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
// Initialization tests
// ---------------------------------------------------------------------------

#[test]
fn test_init_sets_admin_and_fee() {
    let t = TestEnv::new();
    let admin = t.tip_client().get_admin().unwrap();
    assert!(admin == t.admin);
    let fee_recipient = t.tip_client().get_fee_recipient().unwrap();
    assert!(fee_recipient == t.fee_recipient);
    assert_eq!(t.tip_client().get_fee_percentage(), 0);
    assert_eq!(t.tip_client().get_contract_version(), 2);
    assert!(!t.tip_client().is_paused());
    assert_eq!(t.tip_client().get_max_creators(), crate::DEFAULT_MAX_CREATORS);
    assert_eq!(
        t.tip_client().get_max_tips_per_creator(),
        crate::DEFAULT_MAX_TIPS_PER_CREATOR
    );
    assert_eq!(t.tip_client().get_creator_count(), 0);
}

#[test]
#[should_panic(expected = "#9")]
fn test_init_twice_fails() {
    let t = TestEnv::new();
    t.tip_client().init(
        &t.admin,
        &t.fee_recipient,
        &0u32,
        &crate::DEFAULT_MAX_CREATORS,
        &crate::DEFAULT_MAX_TIPS_PER_CREATOR,
    );
}

#[test]
#[should_panic(expected = "#12")]
fn test_init_fee_too_high_fails() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let fee_recipient = Address::generate(&env);
    let contract_id = env.register(TipContract, ());
    let client = crate::TipContractClient::new(&env, &contract_id);
    client.init(
        &admin,
        &fee_recipient,
        &10_001u32,
        &crate::DEFAULT_MAX_CREATORS,
        &crate::DEFAULT_MAX_TIPS_PER_CREATOR,
    );
}

// ---------------------------------------------------------------------------
// Pause tests
// ---------------------------------------------------------------------------

#[test]
fn test_pause_and_unpause() {
    let t = TestEnv::new();
    t.tip_client().pause(&t.admin);
    assert!(t.tip_client().is_paused());
    t.tip_client().unpause(&t.admin);
    assert!(!t.tip_client().is_paused());
}

#[test]
#[should_panic(expected = "#11")]
fn test_pause_unauthorized_fails() {
    let t = TestEnv::new();
    let rando = Address::generate(&t.env);
    t.tip_client().pause(&rando);
}

#[test]
#[should_panic(expected = "#10")]
fn test_register_when_paused_fails() {
    let t = TestEnv::new();
    t.tip_client().pause(&t.admin);
    let alice = Address::generate(&t.env);
    t.tip_client().register(
        &alice,
        &Symbol::new(&t.env, "alice"),
        &s(&t.env, "Alice"),
        &s(&t.env, ""),
    );
}

#[test]
#[should_panic(expected = "#10")]
fn test_tip_when_paused_fails() {
    let t = TestEnv::new();
    let alice = Address::generate(&t.env);
    t.tip_client().register(
        &alice,
        &Symbol::new(&t.env, "alice"),
        &s(&t.env, "Alice"),
        &s(&t.env, ""),
    );
    t.tip_client().pause(&t.admin);
    let bob = Address::generate(&t.env);
    t.stellar_client().mint(&bob, &10_000);
    t.tip_client().tip(&bob, &alice, &t.token_id, &100, &s(&t.env, ""));
}

#[test]
#[should_panic(expected = "#10")]
fn test_withdraw_when_paused_fails() {
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
    t.tip_client().tip(&bob, &alice, &t.token_id, &1_000, &s(&t.env, ""));
    t.tip_client().pause(&t.admin);
    t.tip_client().withdraw(&alice, &t.token_id, &100);
}

// ---------------------------------------------------------------------------
// Admin tests
// ---------------------------------------------------------------------------

#[test]
fn test_set_admin() {
    let t = TestEnv::new();
    let new_admin = Address::generate(&t.env);
    t.tip_client().set_admin(&t.admin, &new_admin);
    assert_eq!(t.tip_client().get_admin(), Some(new_admin));
}

#[test]
fn test_set_fee_percentage() {
    let t = TestEnv::new();
    t.tip_client().set_fee_percentage(&t.admin, &500u32);
    assert_eq!(t.tip_client().get_fee_percentage(), 500);
}

#[test]
fn test_set_fee_recipient() {
    let t = TestEnv::new();
    let new_recipient = Address::generate(&t.env);
    t.tip_client().set_fee_recipient(&t.admin, &new_recipient);
    assert_eq!(t.tip_client().get_fee_recipient(), Some(new_recipient));
}

#[test]
#[should_panic(expected = "#11")]
fn test_set_admin_unauthorized_fails() {
    let t = TestEnv::new();
    let rando = Address::generate(&t.env);
    let new_admin = Address::generate(&t.env);
    t.tip_client().set_admin(&rando, &new_admin);
}

#[test]
#[should_panic(expected = "#11")]
fn test_set_fee_recipient_unauthorized_fails() {
    let t = TestEnv::new();
    let rando = Address::generate(&t.env);
    let new_recipient = Address::generate(&t.env);
    t.tip_client().set_fee_recipient(&rando, &new_recipient);
}

#[test]
#[should_panic(expected = "#11")]
fn test_set_fee_unauthorized_fails() {
    let t = TestEnv::new();
    let rando = Address::generate(&t.env);
    t.tip_client().set_fee_percentage(&rando, &100u32);
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
    assert!(t.tip_client().is_username_taken(&Symbol::new(&t.env, "alice")));

    let resolved = t.tip_client().get_creator_from_username(&Symbol::new(&t.env, "alice"));
    assert_eq!(resolved, Some(alice));

    // get_profile_by_username convenience
    let by_username = t.tip_client().get_profile_by_username(&Symbol::new(&t.env, "alice"));
    assert_eq!(by_username, Some(profile));
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

    t.tip_client().register(&bob, &Symbol::new(&t.env, "popstar"), &s(&t.env, "B"), &s(&t.env, ""));
}

#[test]
#[should_panic(expected = "#12")]
fn test_register_display_name_too_long_fails() {
    let t = TestEnv::new();
    let alice = Address::generate(&t.env);
    let long_name = s(&t.env, &"a".repeat(65));
    t.tip_client().register(&alice, &Symbol::new(&t.env, "alice"), &long_name, &s(&t.env, ""));
}

#[test]
#[should_panic(expected = "#12")]
fn test_register_bio_too_long_fails() {
    let t = TestEnv::new();
    let alice = Address::generate(&t.env);
    let long_bio = s(&t.env, &"a".repeat(257));
    t.tip_client().register(&alice, &Symbol::new(&t.env, "alice"), &s(&t.env, "Alice"), &long_bio);
}

// ---------------------------------------------------------------------------
// Update profile tests
// ---------------------------------------------------------------------------

#[test]
fn test_update_profile() {
    let t = TestEnv::new();
    let alice = Address::generate(&t.env);
    t.tip_client().register(
        &alice,
        &Symbol::new(&t.env, "alice"),
        &s(&t.env, "Alice"),
        &s(&t.env, "Writer"),
    );

    t.tip_client().update_profile(
        &alice,
        &s(&t.env, "Alice Updated"),
        &s(&t.env, "Author and poet"),
    );

    let profile = t.tip_client().get_profile(&alice).unwrap();
    assert_eq!(profile.display_name, s(&t.env, "Alice Updated"));
    assert_eq!(profile.bio, s(&t.env, "Author and poet"));
}

#[test]
#[should_panic(expected = "#2")]
fn test_update_profile_not_creator_fails() {
    let t = TestEnv::new();
    let rando = Address::generate(&t.env);
    t.tip_client().update_profile(&rando, &s(&t.env, "X"), &s(&t.env, ""));
}

// ---------------------------------------------------------------------------
// Unregister tests
// ---------------------------------------------------------------------------

#[test]
fn test_unregister_removes_profile() {
    let t = TestEnv::new();
    let alice = Address::generate(&t.env);
    t.tip_client().register(
        &alice,
        &Symbol::new(&t.env, "alice"),
        &s(&t.env, "Alice"),
        &s(&t.env, ""),
    );

    t.tip_client().unregister(&alice);

    assert!(!t.tip_client().is_creator(&alice));
    assert!(!t.tip_client().is_username_taken(&Symbol::new(&t.env, "alice")));
    assert_eq!(t.tip_client().get_profile(&alice), None);
    assert_eq!(t.tip_client().get_tip_count(&alice), 0);
}

#[test]
#[should_panic(expected = "#13")]
fn test_unregister_with_balance_fails() {
    let t = TestEnv::new();
    let alice = Address::generate(&t.env);
    let bob = Address::generate(&t.env);
    t.tip_client().register(
        &alice,
        &Symbol::new(&t.env, "alice"),
        &s(&t.env, "Alice"),
        &s(&t.env, ""),
    );
    t.stellar_client().mint(&bob, &10_000);
    t.tip_client().tip(&bob, &alice, &t.token_id, &1_000, &s(&t.env, ""));
    t.tip_client().unregister(&alice);
}

#[test]
fn test_unregister_after_full_withdraw() {
    let t = TestEnv::new();
    let alice = Address::generate(&t.env);
    let bob = Address::generate(&t.env);
    t.tip_client().register(
        &alice,
        &Symbol::new(&t.env, "alice"),
        &s(&t.env, "Alice"),
        &s(&t.env, ""),
    );
    t.stellar_client().mint(&bob, &10_000);
    t.tip_client().tip(&bob, &alice, &t.token_id, &1_000, &s(&t.env, ""));
    t.tip_client().withdraw(&alice, &t.token_id, &1_000);
    assert_eq!(t.tip_client().get_balance(&alice, &t.token_id), 0);
    assert_eq!(t.tip_client().get_all_tokens(&alice).len(), 0);
    t.tip_client().unregister(&alice);
    assert!(!t.tip_client().is_creator(&alice));
}

// ---------------------------------------------------------------------------
// Tipping tests
// ---------------------------------------------------------------------------

#[test]
fn test_tip_transfers_tokens() {
    let t = TestEnv::new();
    let alice = Address::generate(&t.env);
    let bob = Address::generate(&t.env);

    t.tip_client().register(
        &alice,
        &Symbol::new(&t.env, "alice"),
        &s(&t.env, "Alice"),
        &s(&t.env, ""),
    );

    t.stellar_client().mint(&bob, &10_000);

    let bob_balance_before = t.token_client().balance(&bob);
    let contract_balance_before = t.token_client().balance(&t.contract_id);

    t.tip_client().tip(&bob, &alice, &t.token_id, &500, &s(&t.env, "Great work!"));

    assert_eq!(t.token_client().balance(&bob), bob_balance_before - 500);
    assert_eq!(t.token_client().balance(&t.contract_id), contract_balance_before + 500);

    let balance = t.tip_client().get_balance(&alice, &t.token_id);
    assert_eq!(balance, 500);

    let tokens = t.tip_client().get_all_tokens(&alice);
    assert_eq!(tokens.len(), 1);
    assert!(tokens.contains(&t.token_id));
}

#[test]
fn test_tip_with_fee() {
    let t = TestEnv::new_with_fee(500); // 5% fee
    let alice = Address::generate(&t.env);
    let bob = Address::generate(&t.env);

    t.tip_client().register(
        &alice,
        &Symbol::new(&t.env, "alice"),
        &s(&t.env, "Alice"),
        &s(&t.env, ""),
    );

    t.stellar_client().mint(&bob, &10_000);

    t.tip_client().tip(&bob, &alice, &t.token_id, &1_000, &s(&t.env, ""));

    // Creator gets 950 (1000 - 5% fee = 50)
    assert_eq!(t.tip_client().get_balance(&alice, &t.token_id), 950);

    // Fee recipient gets 50
    assert_eq!(t.token_client().balance(&t.fee_recipient), 50);
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

    let index = t.tip_client().tip(&bob, &alice, &t.token_id, &300, &s(&t.env, "💜"));

    assert_eq!(index, 0);
    assert_eq!(t.tip_client().get_tip_count(&alice), 1);

    let tip = t.tip_client().get_tip(&alice, &0).unwrap();
    assert_eq!(tip.from, bob);
    assert_eq!(tip.token, t.token_id);
    assert_eq!(tip.amount, 300);
    assert_eq!(tip.message, s(&t.env, "💜"));
    assert_eq!(tip.timestamp, 1000);

    let charlie = Address::generate(&t.env);
    t.stellar_client().mint(&charlie, &10_000);

    let index2 = t.tip_client().tip(&charlie, &alice, &t.token_id, &200, &s(&t.env, ""));
    assert_eq!(index2, 1);
    assert_eq!(t.tip_client().get_tip_count(&alice), 2);
}

#[test]
fn test_get_tips_pagination() {
    let t = TestEnv::new();
    let alice = Address::generate(&t.env);

    t.tip_client().register(
        &alice,
        &Symbol::new(&t.env, "alice"),
        &s(&t.env, "Alice"),
        &s(&t.env, ""),
    );

    for _ in 0..5 {
        let supporter = Address::generate(&t.env);
        t.stellar_client().mint(&supporter, &10_000);
        t.tip_client().tip(&supporter, &alice, &t.token_id, &100, &s(&t.env, "tip"));
    }

    assert_eq!(t.tip_client().get_tip_count(&alice), 5);

    let page1 = t.tip_client().get_tips(&alice, &0, &2);
    assert_eq!(page1.len(), 2);
    assert_eq!(page1.get(0).unwrap().amount, 100);

    let page2 = t.tip_client().get_tips(&alice, &2, &2);
    assert_eq!(page2.len(), 2);

    let page3 = t.tip_client().get_tips(&alice, &4, &10);
    assert_eq!(page3.len(), 1);

    let empty = t.tip_client().get_tips(&alice, &10, &10);
    assert_eq!(empty.len(), 0);
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

    t.tip_client().register(&alice, &Symbol::new(&t.env, "alice"), &s(&t.env, "A"), &s(&t.env, ""));

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

    t.stellar_client().mint(&bob, &10_000);
    t.tip_client().tip(&bob, &alice, &t.token_id, &1_000, &s(&t.env, ""));

    let alice_balance_before = t.token_client().balance(&alice);

    t.tip_client().withdraw(&alice, &t.token_id, &400);

    assert_eq!(t.token_client().balance(&alice), alice_balance_before + 400);

    assert_eq!(t.tip_client().get_balance(&alice, &t.token_id), 600);
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
    t.tip_client().tip(&bob, &alice, &t.token_id, &777, &s(&t.env, ""));

    t.tip_client().withdraw(&alice, &t.token_id, &777);
    assert_eq!(t.tip_client().get_balance(&alice, &t.token_id), 0);
    let tokens = t.tip_client().get_all_tokens(&alice);
    assert_eq!(tokens.len(), 0);
}

#[test]
#[should_panic(expected = "#2")]
fn test_withdraw_not_creator_fails() {
    let t = TestEnv::new();
    let alice = Address::generate(&t.env);
    let rando = Address::generate(&t.env);

    t.tip_client().register(
        &alice,
        &Symbol::new(&t.env, "alice"),
        &s(&t.env, "Alice"),
        &s(&t.env, ""),
    );

    let bob = Address::generate(&t.env);
    t.stellar_client().mint(&bob, &10_000);
    t.tip_client().tip(&bob, &alice, &t.token_id, &1_000, &s(&t.env, ""));

    t.tip_client().withdraw(&rando, &t.token_id, &100);
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

    t.tip_client().register(&alice, &Symbol::new(&t.env, "alice"), &s(&t.env, "A"), &s(&t.env, ""));

    t.tip_client().withdraw(&alice, &t.token_id, &0);
}

// ---------------------------------------------------------------------------
// Edge-case: tipping with multiple tokens
// ---------------------------------------------------------------------------

#[test]
fn test_multiple_token_balances() {
    let t = TestEnv::new();

    let (token2_id, _, t2_sac) = t.deploy_second_token();

    let alice = Address::generate(&t.env);
    t.tip_client().register(
        &alice,
        &Symbol::new(&t.env, "alice"),
        &s(&t.env, "Alice"),
        &s(&t.env, ""),
    );

    let bob = Address::generate(&t.env);
    t.stellar_client().mint(&bob, &100_000);
    t2_sac.mint(&bob, &50_000);

    t.tip_client().tip(&bob, &alice, &t.token_id, &1_000, &s(&t.env, ""));

    t.tip_client().tip(&bob, &alice, &token2_id, &500, &s(&t.env, ""));

    assert_eq!(t.tip_client().get_balance(&alice, &t.token_id), 1_000);
    assert_eq!(t.tip_client().get_balance(&alice, &token2_id), 500);

    let tokens = t.tip_client().get_all_tokens(&alice);
    assert!(tokens.contains(&t.token_id));
    assert!(tokens.contains(&token2_id));

    t.tip_client().withdraw(&alice, &token2_id, &200);
    assert_eq!(t.tip_client().get_balance(&alice, &token2_id), 300);
    assert_eq!(t.tip_client().get_balance(&alice, &t.token_id), 1_000);

    let tokens_after = t.tip_client().get_all_tokens(&alice);
    assert!(tokens_after.contains(&token2_id));
    assert!(tokens_after.contains(&t.token_id));
}

// ---------------------------------------------------------------------------
// Creator verification tests
// ---------------------------------------------------------------------------

#[test]
#[should_panic(expected = "#2")]
fn test_withdraw_requires_creator_verification() {
    let t = TestEnv::new();
    let alice = Address::generate(&t.env);
    let bob = Address::generate(&t.env);

    t.tip_client().register(
        &alice,
        &Symbol::new(&t.env, "alice"),
        &s(&t.env, "Alice"),
        &s(&t.env, ""),
    );

    t.stellar_client().mint(&bob, &10_000);
    t.tip_client().tip(&bob, &alice, &t.token_id, &1_000, &s(&t.env, ""));

    // Alice should be able to withdraw.
    t.tip_client().withdraw(&alice, &t.token_id, &100);

    // Bob is not a creator — this should panic.
    t.tip_client().withdraw(&bob, &t.token_id, &100);
}

// ---------------------------------------------------------------------------
// Storage bloat cap tests (issue #36)
// ---------------------------------------------------------------------------

#[test]
fn test_init_persists_supplied_caps() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set(LedgerInfo {
        timestamp: 1000,
        protocol_version: 22,
        sequence_number: 100,
        network_id: Default::default(),
        base_reserve: 10,
        min_persistent_entry_ttl: 10,
        max_entry_ttl: 1_000_000,
        min_temp_entry_ttl: 10,
    });
    let admin = Address::generate(&env);
    let fee_recipient = Address::generate(&env);
    let contract_id = env.register(TipContract, ());
    let client = crate::TipContractClient::new(&env, &contract_id);

    // Custom caps (not the defaults) are written to storage on init.
    client.init(&admin, &fee_recipient, &0u32, &7u32, &4u32);
    assert_eq!(client.get_max_creators(), 7);
    assert_eq!(client.get_max_tips_per_creator(), 4);
    assert_eq!(client.get_creator_count(), 0);

    // The 7-creator cap is enforced exactly as supplied.
    for i in 0..7 {
        let creator = Address::generate(&env);
        client.register(
            &creator,
            &Symbol::new(&env, &format!("c{i}")),
            &s(&env, "Name"),
            &s(&env, ""),
        );
    }
    assert_eq!(client.get_creator_count(), 7);

    let extra = Address::generate(&env);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.register(
            &extra,
            &Symbol::new(&env, "overflow"),
            &s(&env, "X"),
            &s(&env, ""),
        );
    }));
    assert!(result.is_err(), "8th creator must be rejected");
    assert_eq!(client.get_creator_count(), 7);
}

#[test]
fn test_init_with_zero_caps_means_unlimited() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set(LedgerInfo {
        timestamp: 1000,
        protocol_version: 22,
        sequence_number: 100,
        network_id: Default::default(),
        base_reserve: 10,
        min_persistent_entry_ttl: 10,
        max_entry_ttl: 1_000_000,
        min_temp_entry_ttl: 10,
    });
    let admin = Address::generate(&env);
    let fee_recipient = Address::generate(&env);
    let contract_id = env.register(TipContract, ());
    let client = crate::TipContractClient::new(&env, &contract_id);

    // `0` = unlimited for both caps.
    client.init(&admin, &fee_recipient, &0u32, &0u32, &0u32);
    assert_eq!(client.get_max_creators(), 0);
    assert_eq!(client.get_max_tips_per_creator(), 0);

    // Registering way more than DEFAULT_MAX_CREATORS should be fine.
    for i in 0..25 {
        let creator = Address::generate(&env);
        client.register(
            &creator,
            &Symbol::new(&env, &format!("creator_{i:02}")),
            &s(&env, "Name"),
            &s(&env, ""),
        );
    }
    assert_eq!(client.get_creator_count(), 25);
}

#[test]
fn test_register_fails_when_creator_cap_reached() {
    let t = TestEnv::new_with_caps(0, 2, 0);
    assert_eq!(t.tip_client().get_max_creators(), 2);
    assert_eq!(t.tip_client().get_creator_count(), 0);

    let alice = Address::generate(&t.env);
    let bob = Address::generate(&t.env);
    t.tip_client().register(&alice, &Symbol::new(&t.env, "alice"), &s(&t.env, "A"), &s(&t.env, ""));
    t.tip_client().register(&bob, &Symbol::new(&t.env, "bob"), &s(&t.env, "B"), &s(&t.env, ""));
    assert_eq!(t.tip_client().get_creator_count(), 2);

    let charlie = Address::generate(&t.env);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        t.tip_client().register(
            &charlie,
            &Symbol::new(&t.env, "charlie"),
            &s(&t.env, "C"),
            &s(&t.env, ""),
        );
    }));
    assert!(result.is_err(), "register should have panicked with CapExceeded");
    // Counter must remain unchanged after a rejected registration.
    assert_eq!(t.tip_client().get_creator_count(), 2);
}

#[test]
fn test_register_succeeds_after_unregister_frees_slot() {
    let t = TestEnv::new_with_caps(0, 1, 0);

    let alice = Address::generate(&t.env);
    t.tip_client().register(&alice, &Symbol::new(&t.env, "alice"), &s(&t.env, "A"), &s(&t.env, ""));
    assert_eq!(t.tip_client().get_creator_count(), 1);

    let bob = Address::generate(&t.env);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        t.tip_client().register(&bob, &Symbol::new(&t.env, "bob"), &s(&t.env, "B"), &s(&t.env, ""));
    }));
    assert!(result.is_err(), "second register should have been rejected");

    // Free the slot and try again.
    t.tip_client().unregister(&alice);
    assert_eq!(t.tip_client().get_creator_count(), 0);

    t.tip_client().register(&bob, &Symbol::new(&t.env, "bob"), &s(&t.env, "B"), &s(&t.env, ""));
    assert_eq!(t.tip_client().get_creator_count(), 1);
}

#[test]
fn test_creator_count_tracks_register_and_unregister() {
    let t = TestEnv::new();
    assert_eq!(t.tip_client().get_creator_count(), 0);

    let mut creators: Vec<Address> = Vec::new();
    for i in 0..5 {
        let a = Address::generate(&t.env);
        t.tip_client().register(
            &a,
            &Symbol::new(&t.env, &format!("user_{i}")),
            &s(&t.env, "Name"),
            &s(&t.env, ""),
        );
        creators.push(a);
    }
    assert_eq!(t.tip_client().get_creator_count(), 5);

    t.tip_client().unregister(&creators[0]);
    assert_eq!(t.tip_client().get_creator_count(), 4);
    t.tip_client().unregister(&creators[2]);
    t.tip_client().unregister(&creators[4]);
    assert_eq!(t.tip_client().get_creator_count(), 2);
}

#[test]
fn test_tip_fails_when_tip_cap_reached() {
    let t = TestEnv::new_with_caps(0, 0, 3);
    let alice = Address::generate(&t.env);
    t.tip_client().register(&alice, &Symbol::new(&t.env, "alice"), &s(&t.env, "A"), &s(&t.env, ""));

    for _ in 0..3 {
        let supporter = Address::generate(&t.env);
        t.stellar_client().mint(&supporter, &10_000);
        t.tip_client().tip(&supporter, &alice, &t.token_id, &10, &s(&t.env, ""));
    }
    assert_eq!(t.tip_client().get_tip_count(&alice), 3);

    let tipper = Address::generate(&t.env);
    t.stellar_client().mint(&tipper, &10_000);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        t.tip_client().tip(&tipper, &alice, &t.token_id, &10, &s(&t.env, ""));
    }));
    assert!(result.is_err(), "tip should have panicked with CapExceeded");
    assert_eq!(t.tip_client().get_tip_count(&alice), 3);
}

#[test]
fn test_set_max_creators_admin_authorized() {
    let t = TestEnv::new();
    // Initial defaults.
    assert_eq!(t.tip_client().get_max_creators(), crate::DEFAULT_MAX_CREATORS);

    // Admin can lower the cap.
    t.tip_client().set_max_creators(&t.admin, &500u32);
    assert_eq!(t.tip_client().get_max_creators(), 500);

    // Admin can disable the cap with 0.
    t.tip_client().set_max_creators(&t.admin, &0u32);
    assert_eq!(t.tip_client().get_max_creators(), 0);
}

#[test]
#[should_panic(expected = "#11")]
fn test_set_max_creators_unauthorized_fails() {
    let t = TestEnv::new();
    let rando = Address::generate(&t.env);
    t.tip_client().set_max_creators(&rando, &100u32);
}

#[test]
fn test_set_max_tips_per_creator_admin_authorized() {
    let t = TestEnv::new();
    assert_eq!(
        t.tip_client().get_max_tips_per_creator(),
        crate::DEFAULT_MAX_TIPS_PER_CREATOR
    );

    t.tip_client().set_max_tips_per_creator(&t.admin, &250u32);
    assert_eq!(t.tip_client().get_max_tips_per_creator(), 250);

    t.tip_client().set_max_tips_per_creator(&t.admin, &0u32);
    assert_eq!(t.tip_client().get_max_tips_per_creator(), 0);
}

#[test]
#[should_panic(expected = "#11")]
fn test_set_max_tips_per_creator_unauthorized_fails() {
    let t = TestEnv::new();
    let rando = Address::generate(&t.env);
    t.tip_client().set_max_tips_per_creator(&rando, &100u32);
}

#[test]
fn test_lowering_cap_does_not_retroactively_evict() {
    // Initial cap is generous; register several creators and tip a bunch.
    let t = TestEnv::new_with_caps(0, 100, 100);
    let alice = Address::generate(&t.env);
    t.tip_client().register(&alice, &Symbol::new(&t.env, "alice"), &s(&t.env, "A"), &s(&t.env, ""));
    let bob = Address::generate(&t.env);
    t.tip_client().register(&bob, &Symbol::new(&t.env, "bob"), &s(&t.env, "B"), &s(&t.env, ""));
    assert_eq!(t.tip_client().get_creator_count(), 2);

    // Admin lowers the creator cap below the current count.
    t.tip_client().set_max_creators(&t.admin, &1u32);
    assert_eq!(t.tip_client().get_max_creators(), 1);

    // Existing creators can still operate normally.
    assert!(t.tip_client().is_creator(&alice));
    assert!(t.tip_client().is_creator(&bob));
    let supporter = Address::generate(&t.env);
    t.stellar_client().mint(&supporter, &10_000);
    t.tip_client().tip(&supporter, &alice, &t.token_id, &50, &s(&t.env, ""));
    assert_eq!(t.tip_client().get_balance(&alice, &t.token_id), 50);

    // ...but a new registration must be rejected with CapExceeded.
    let charlie = Address::generate(&t.env);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        t.tip_client().register(
            &charlie,
            &Symbol::new(&t.env, "charlie"),
            &s(&t.env, "C"),
            &s(&t.env, ""),
        );
    }));
    assert!(result.is_err(), "register should have been rejected");
    assert_eq!(t.tip_client().get_creator_count(), 2);

    // Admin can raise the cap again to restore capacity.
    t.tip_client().set_max_creators(&t.admin, &10u32);
    t.tip_client().register(
        &charlie,
        &Symbol::new(&t.env, "charlie"),
        &s(&t.env, "C"),
        &s(&t.env, ""),
    );
    assert_eq!(t.tip_client().get_creator_count(), 3);
}

#[test]
fn test_lowering_tip_cap_blocks_further_tips_for_existing() {
    let t = TestEnv::new_with_caps(0, 0, 5);
    let alice = Address::generate(&t.env);
    t.tip_client().register(&alice, &Symbol::new(&t.env, "alice"), &s(&t.env, "A"), &s(&t.env, ""));

    for _ in 0..5 {
        let supporter = Address::generate(&t.env);
        t.stellar_client().mint(&supporter, &10_000);
        t.tip_client().tip(&supporter, &alice, &t.token_id, &10, &s(&t.env, ""));
    }
    assert_eq!(t.tip_client().get_tip_count(&alice), 5);

    // Lower the per-creator tip cap below 5. Existing tips remain; new tips
    // must be rejected.
    t.tip_client().set_max_tips_per_creator(&t.admin, &2u32);
    let next_tipper = Address::generate(&t.env);
    t.stellar_client().mint(&next_tipper, &10_000);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        t.tip_client()
            .tip(&next_tipper, &alice, &t.token_id, &10, &s(&t.env, ""));
    }));
    assert!(result.is_err(), "tip should have been rejected with CapExceeded");
    assert_eq!(t.tip_client().get_tip_count(&alice), 5);

    // History is still readable.
    let history = t.tip_client().get_tips(&alice, &0u64, &10u64);
    assert_eq!(history.len(), 5);
}
