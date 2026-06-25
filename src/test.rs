#![cfg(test)]

use soroban_sdk::{
    testutils::{Address as _, Events, Ledger, LedgerInfo},
    token,
    token::StellarAssetClient,
    Address, Env, String, Symbol, TryIntoVal, Vec,
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
        t.tip_client().set_max_creators(&t.admin, &max_creators);
        t.tip_client().set_max_tips_per_creator(&t.admin, &max_tips);
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

    /// Deploy `count` additional Stellar asset tokens and mint the admin a
    /// large balance on each. Used by tests that need many distinct tokens.
    fn deploy_many_tokens(&self, count: usize) -> Vec<Address> {
        let mut out: Vec<Address> = Vec::new(&self.env);
        for _ in 0..count {
            let token_admin = Address::generate(&self.env);
            let token_contract = self.env.register_stellar_asset_contract_v2(token_admin);
            let id = token_contract.address();
            let sac = StellarAssetClient::new(&self.env, &id);
            sac.mint(&self.admin, &1_000_000_000);
            out.push_back(id);
        }
        out
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
    assert_eq!(t.tip_client().get_max_tips_per_creator(), crate::DEFAULT_MAX_TIPS_PER_CREATOR);
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
fn test_init_emits_event() {
    // Use a dedicated env so we can inspect emitted events directly,
    // independent of the shared TestEnv construction path.
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

    // Non-default fee bps so the payload cannot accidentally be zero.
    // Use 0 for both caps (unlimited) since this test is about the init event.
    client.init(&admin, &fee_recipient, &500u32, &0u32, &0u32);

    let events = env.events().all();
    let expected_name = Symbol::new(&env, "INIT");

    // Exactly one EVENT_INIT event should be published, with the admin as
    // topic[1] and (fee_recipient, fee_bps = 500) as the payload.
    let mut total: u32 = 0;
    let mut init_event = None;
    for event in &events {
        let topics = &event.1;
        if topics.len() < 2 {
            continue;
        }
        if let Some(topic0) = topics.get(0) {
            let sym: Symbol = topic0.try_into_val(&env).unwrap();
            if sym == expected_name {
                total += 1;
                init_event = Some(event);
            }
        }
    }
    assert_eq!(total, 1, "expected exactly one EVENT_INIT, found {total}");

    let event = init_event.unwrap();
    assert_eq!(event.0, contract_id);

    let topic_admin: Address = event.1.get(1).unwrap().try_into_val(&env).unwrap();
    assert_eq!(topic_admin, admin);

    let (payload_recipient, payload_bps): (Address, u32) = event.2.try_into_val(&env).unwrap();
    assert_eq!(payload_recipient, fee_recipient);
    assert_eq!(payload_bps, 500);
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

    // Step 1: current admin nominates new admin
    t.tip_client().set_admin(&t.admin, &new_admin);
    // Admin should not have changed yet — clone to avoid moves
    let admin_addr = t.admin.clone();
    assert_eq!(t.tip_client().get_admin(), Some(admin_addr));
    // Pending admin should be set
    let pending = new_admin.clone();
    assert_eq!(t.tip_client().get_pending_admin(), Some(pending));

    // Step 2: nominated admin accepts
    t.tip_client().accept_admin(&new_admin);
    let new_admin_addr = new_admin.clone();
    assert_eq!(t.tip_client().get_admin(), Some(new_admin_addr));
    assert_eq!(t.tip_client().get_pending_admin(), None);
}

#[test]
#[should_panic(expected = "#16")]
fn test_accept_admin_without_pending_fails() {
    let t = TestEnv::new();
    let rando = Address::generate(&t.env);
    t.tip_client().accept_admin(&rando);
}

#[test]
#[should_panic(expected = "#11")]
fn test_accept_admin_unauthorized_fails() {
    let t = TestEnv::new();
    let new_admin = Address::generate(&t.env);
    let rando = Address::generate(&t.env);
    t.tip_client().set_admin(&t.admin, &new_admin);
    // Only the nominated address can accept.
    t.tip_client().accept_admin(&rando);
}

#[test]
fn test_cancel_admin_transfer() {
    let t = TestEnv::new();
    let new_admin = Address::generate(&t.env);

    t.tip_client().set_admin(&t.admin, &new_admin);
    assert_eq!(t.tip_client().get_pending_admin(), Some(new_admin));

    // Cancel the pending transfer.
    t.tip_client().cancel_admin_transfer(&t.admin);
    assert_eq!(t.tip_client().get_pending_admin(), None);
    // Admin stays the same.
    assert_eq!(t.tip_client().get_admin(), Some(t.admin));
}

#[test]
#[should_panic(expected = "#11")]
fn test_cancel_admin_transfer_unauthorized_fails() {
    let t = TestEnv::new();
    let new_admin = Address::generate(&t.env);
    let rando = Address::generate(&t.env);
    t.tip_client().set_admin(&t.admin, &new_admin);
    // Only the current admin can cancel.
    t.tip_client().cancel_admin_transfer(&rando);
}

#[test]
#[should_panic(expected = "#16")]
fn test_cancel_admin_transfer_no_pending_fails() {
    let t = TestEnv::new();
    // No pending transfer to cancel.
    t.tip_client().cancel_admin_transfer(&t.admin);
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

// Regression test for issue #28:
// when `fee_bps > 0` is configured but `FeeRecipient` is unset in instance
// storage, `tip()` must surface a clean `FeeRecipientNotSet` error rather
// than an unrecoverable panic that would DoS tipping.
#[test]
#[should_panic(expected = "#15")]
fn test_tip_with_fee_recipient_unset_fails() {
    let t = TestEnv::new();

    // Configure a non-zero fee (this normally requires the recipient to be
    // set; we deliberately invalidate storage below to simulate a corrupted
    // / missing recipient state).
    t.tip_client().set_fee_percentage(&t.admin, &500u32);

    let alice = Address::generate(&t.env);
    t.tip_client().register(
        &alice,
        &Symbol::new(&t.env, "alice"),
        &s(&t.env, "Alice"),
        &s(&t.env, ""),
    );

    // Remove the `FeeRecipient` key from the contract's instance storage to
    // reproduce the failing precondition.
    let contract_id = t.contract_id.clone();
    t.env.as_contract(&contract_id, || {
        t.env.storage().instance().remove(&crate::DataKey::FeeRecipient);
    });

    let bob = Address::generate(&t.env);
    t.stellar_client().mint(&bob, &10_000);
    t.tip_client().tip(&bob, &alice, &t.token_id, &1_000, &s(&t.env, ""));
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

    // The 7-creator cap is enforced exactly as supplied. The 8th
    // registration is asserted separately in
    // `test_init_supplied_caps_rejects_8th_creator`.
    let cap_names: [&str; 7] = ["c0", "c1", "c2", "c3", "c4", "c5", "c6"];
    for name in cap_names.iter() {
        let creator = Address::generate(&env);
        client.register(&creator, &Symbol::new(&env, name), &s(&env, "Name"), &s(&env, ""));
    }
    assert_eq!(client.get_creator_count(), 7);
}

#[test]
#[should_panic(expected = "#14")]
fn test_init_supplied_caps_rejects_8th_creator() {
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

    // Cap of 7, then fill, then attempt an 8th → CapExceeded.
    client.init(&admin, &fee_recipient, &0u32, &7u32, &4u32);
    let cap_names: [&str; 7] = ["c7_0", "c7_1", "c7_2", "c7_3", "c7_4", "c7_5", "c7_6"];
    for name in cap_names.iter() {
        let creator = Address::generate(&env);
        client.register(&creator, &Symbol::new(&env, name), &s(&env, "Name"), &s(&env, ""));
    }
    assert_eq!(client.get_creator_count(), 7);
    let extra = Address::generate(&env);
    client.register(&extra, &Symbol::new(&env, "overflow"), &s(&env, "X"), &s(&env, ""));
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

    // Registering more than DEFAULT_MAX_CREATORS should be fine.
    let names: [&str; 12] = [
        "creator_00",
        "creator_01",
        "creator_02",
        "creator_03",
        "creator_04",
        "creator_05",
        "creator_06",
        "creator_07",
        "creator_08",
        "creator_09",
        "creator_10",
        "creator_11",
    ];
    for name in names.iter() {
        let creator = Address::generate(&env);
        client.register(&creator, &Symbol::new(&env, name), &s(&env, "Name"), &s(&env, ""));
    }
    assert_eq!(client.get_creator_count(), 12);
}

#[test]
#[should_panic(expected = "#14")]
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
    t.tip_client().register(
        &charlie,
        &Symbol::new(&t.env, "charlie"),
        &s(&t.env, "C"),
        &s(&t.env, ""),
    );
}

#[test]
#[should_panic(expected = "#14")]
fn test_register_blocked_when_creator_cap_reached() {
    let t = TestEnv::new_with_caps(0, 1, 0);

    let alice = Address::generate(&t.env);
    t.tip_client().register(&alice, &Symbol::new(&t.env, "alice"), &s(&t.env, "A"), &s(&t.env, ""));

    let bob = Address::generate(&t.env);
    t.tip_client().register(&bob, &Symbol::new(&t.env, "bob"), &s(&t.env, "B"), &s(&t.env, ""));
}

#[test]
fn test_unregister_frees_creator_slot_for_reuse() {
    let t = TestEnv::new_with_caps(0, 1, 0);

    let alice = Address::generate(&t.env);
    t.tip_client().register(&alice, &Symbol::new(&t.env, "alice"), &s(&t.env, "A"), &s(&t.env, ""));
    assert_eq!(t.tip_client().get_creator_count(), 1);

    t.tip_client().unregister(&alice);
    assert_eq!(t.tip_client().get_creator_count(), 0);

    let bob = Address::generate(&t.env);
    t.tip_client().register(&bob, &Symbol::new(&t.env, "bob"), &s(&t.env, "B"), &s(&t.env, ""));
    assert_eq!(t.tip_client().get_creator_count(), 1);
}

#[test]
fn test_creator_count_tracks_register_and_unregister() {
    let t = TestEnv::new();
    assert_eq!(t.tip_client().get_creator_count(), 0);

    let a0 = Address::generate(&t.env);
    let a1 = Address::generate(&t.env);
    let a2 = Address::generate(&t.env);
    let a3 = Address::generate(&t.env);
    let a4 = Address::generate(&t.env);
    let names: [&str; 5] = ["user_0", "user_1", "user_2", "user_3", "user_4"];
    let creators: [&Address; 5] = [&a0, &a1, &a2, &a3, &a4];
    for (creator, name) in creators.iter().zip(names.iter()) {
        t.tip_client().register(
            creator,
            &Symbol::new(&t.env, name),
            &s(&t.env, "Name"),
            &s(&t.env, ""),
        );
    }
    assert_eq!(t.tip_client().get_creator_count(), 5);

    t.tip_client().unregister(&a0);
    assert_eq!(t.tip_client().get_creator_count(), 4);
    t.tip_client().unregister(&a2);
    t.tip_client().unregister(&a4);
    assert_eq!(t.tip_client().get_creator_count(), 2);
    assert!(t.tip_client().is_creator(&a1));
    assert!(t.tip_client().is_creator(&a3));
}

#[test]
#[should_panic(expected = "#14")]
fn test_tip_fails_when_tip_cap_reached() {
    let t = TestEnv::new_with_caps(0, 0, 3);
    let alice = Address::generate(&t.env);
    t.tip_client().register(&alice, &Symbol::new(&t.env, "alice"), &s(&t.env, "A"), &s(&t.env, ""));

    for _ in 0..3 {
        let supporter = Address::generate(&t.env);
        t.stellar_client().mint(&supporter, &10_000);
        t.tip_client().tip(&supporter, &alice, &t.token_id, &10, &s(&t.env, ""));
    }

    let tipper = Address::generate(&t.env);
    t.stellar_client().mint(&tipper, &10_000);
    t.tip_client().tip(&tipper, &alice, &t.token_id, &10, &s(&t.env, ""));
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
    assert_eq!(t.tip_client().get_max_tips_per_creator(), crate::DEFAULT_MAX_TIPS_PER_CREATOR);

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
fn test_lowering_creator_cap_keeps_existing_creators_active() {
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
}

#[test]
#[should_panic(expected = "#14")]
fn test_lowering_creator_cap_rejects_new_registrations() {
    let t = TestEnv::new_with_caps(0, 100, 100);
    let alice = Address::generate(&t.env);
    t.tip_client().register(&alice, &Symbol::new(&t.env, "alice"), &s(&t.env, "A"), &s(&t.env, ""));
    let bob = Address::generate(&t.env);
    t.tip_client().register(&bob, &Symbol::new(&t.env, "bob"), &s(&t.env, "B"), &s(&t.env, ""));

    // Lower the creator cap below the current count.
    t.tip_client().set_max_creators(&t.admin, &1u32);

    // New registration with full cap must be rejected.
    let charlie = Address::generate(&t.env);
    t.tip_client().register(
        &charlie,
        &Symbol::new(&t.env, "charlie"),
        &s(&t.env, "C"),
        &s(&t.env, ""),
    );
}

#[test]
fn test_raising_creator_cap_restores_capacity() {
    let t = TestEnv::new_with_caps(0, 100, 100);
    let alice = Address::generate(&t.env);
    t.tip_client().register(&alice, &Symbol::new(&t.env, "alice"), &s(&t.env, "A"), &s(&t.env, ""));
    let bob = Address::generate(&t.env);
    t.tip_client().register(&bob, &Symbol::new(&t.env, "bob"), &s(&t.env, "B"), &s(&t.env, ""));

    t.tip_client().set_max_creators(&t.admin, &1u32);

    // Admin raises the cap again to restore capacity.
    t.tip_client().set_max_creators(&t.admin, &10u32);

    let charlie = Address::generate(&t.env);
    t.tip_client().register(
        &charlie,
        &Symbol::new(&t.env, "charlie"),
        &s(&t.env, "C"),
        &s(&t.env, ""),
    );
    assert_eq!(t.tip_client().get_creator_count(), 3);
}

#[test]
#[should_panic(expected = "#14")]
fn test_lowering_tip_cap_blocks_further_tips_for_existing() {
    let t = TestEnv::new_with_caps(0, 0, 5);
    let alice = Address::generate(&t.env);
    t.tip_client().register(&alice, &Symbol::new(&t.env, "alice"), &s(&t.env, "A"), &s(&t.env, ""));

    for _ in 0..5 {
        let supporter = Address::generate(&t.env);
        t.stellar_client().mint(&supporter, &10_000);
        t.tip_client().tip(&supporter, &alice, &t.token_id, &10, &s(&t.env, ""));
    }

    // Lower the per-creator tip cap below 5 and attempt to send another
    // tip — must be rejected with CapExceeded.
    t.tip_client().set_max_tips_per_creator(&t.admin, &2u32);
    let next_tipper = Address::generate(&t.env);
    t.stellar_client().mint(&next_tipper, &10_000);
    t.tip_client().tip(&next_tipper, &alice, &t.token_id, &10, &s(&t.env, ""));
}

#[test]
fn test_lowering_tip_cap_preserves_existing_history() {
    let t = TestEnv::new_with_caps(0, 0, 5);
    let alice = Address::generate(&t.env);
    t.tip_client().register(&alice, &Symbol::new(&t.env, "alice"), &s(&t.env, "A"), &s(&t.env, ""));

    for _ in 0..5 {
        let supporter = Address::generate(&t.env);
        t.stellar_client().mint(&supporter, &10_000);
        t.tip_client().tip(&supporter, &alice, &t.token_id, &10, &s(&t.env, ""));
    }
    assert_eq!(t.tip_client().get_tip_count(&alice), 5);

    // Lower the cap; no new tips are recorded but the 5 historical entries
    // are still readable.
    t.tip_client().set_max_tips_per_creator(&t.admin, &2u32);
    let history = t.tip_client().get_tips(&alice, &0u64, &10u64);
    assert_eq!(history.len(), 5);
}

// ---------------------------------------------------------------------------
// Multi-token withdraw optimization tests (issue #38)
// ---------------------------------------------------------------------------
//
// These tests confirm that the contract can manage many distinct tokens
// for a single creator and that withdrawing each token's full balance
// removes it from the tracked token set (now backed by Map<Address, ()>)
// without relying on a linear scan.

/// Register `alice` and tip them once with each of `count` distinct
/// Stellar asset tokens. Returns the tracked token addresses in
/// insertion order. Each supporting funder has enough balance to tip 1_000
/// of their token.
fn seed_creator_with_many_tokens(t: &TestEnv, alice: &Address, count: usize) -> Vec<Address> {
    let tokens = t.deploy_many_tokens(count);
    let n: u32 = count as u32;
    for i in 0..n {
        let token = tokens.get(i).unwrap();
        let supporter = Address::generate(&t.env);
        let sac = StellarAssetClient::new(&t.env, &token);
        sac.mint(&supporter, &10_000);
        t.tip_client().tip(&supporter, alice, &token, &1_000, &s(&t.env, ""));
    }
    tokens
}

#[test]
fn test_withdraw_many_tokens_full_withdraw_clears_set() {
    // Withdraw 12 distinct tokens in a non-sequential order; the
    // map-backed CreatorTokens must drop each one in O(log n).
    const N: u32 = 12;

    let t = TestEnv::new();
    let alice = Address::generate(&t.env);
    t.tip_client().register(
        &alice,
        &Symbol::new(&t.env, "alice"),
        &s(&t.env, "Alice"),
        &s(&t.env, ""),
    );

    let token_addrs = seed_creator_with_many_tokens(&t, &alice, N as usize);
    assert_eq!(token_addrs.len(), N);

    // Sanity check: every token is tracked and each balance is 1_000.
    let tracked = t.tip_client().get_all_tokens(&alice);
    assert_eq!(tracked.len(), N);
    for i in 0..N {
        let token = token_addrs.get(i).unwrap();
        assert!(tracked.contains(&token), "tokens seeded should be tracked by the contract");
        assert_eq!(t.tip_client().get_balance(&alice, &token), 1_000);
    }

    // Withdraw in non-sequential order so the map-backed removal is
    // exercised from every position.
    let order: [u32; 12] = [7, 0, 11, 3, 1, 9, 4, 6, 2, 10, 5, 8];
    for &i in order.iter() {
        let token = token_addrs.get(i).unwrap();
        t.tip_client().withdraw(&alice, &token, &1_000);

        // Removed token: gone from the tracked set and zero-balanced.
        let remaining = t.tip_client().get_all_tokens(&alice);
        assert!(!remaining.contains(&token), "token #{i} should be removed after full withdraw");
        assert_eq!(t.tip_client().get_balance(&alice, &token), 0);
    }

    // After withdrawing every token fully, the tracked set is empty and the
    // creator can unregister (which requires all balances to be zero).
    assert_eq!(t.tip_client().get_all_tokens(&alice).len(), 0);
    t.tip_client().unregister(&alice);
    assert!(!t.tip_client().is_creator(&alice));
}

#[test]
fn test_withdraw_many_tokens_partial_keeps_remaining() {
    // A partial withdrawal of one of many tokens must NOT remove that
    // token from the tracked set.
    const N: u32 = 10;
    const PARTIAL_IDX: u32 = 3;
    const PARTIAL_AMOUNT: i128 = 400;

    let t = TestEnv::new();
    let alice = Address::generate(&t.env);
    t.tip_client().register(
        &alice,
        &Symbol::new(&t.env, "alice"),
        &s(&t.env, "Alice"),
        &s(&t.env, ""),
    );

    let token_addrs = seed_creator_with_many_tokens(&t, &alice, N as usize);
    assert_eq!(token_addrs.len(), N);

    // Partial withdraw on token #PARTIAL_IDX must leave the token present.
    let partial_token = token_addrs.get(PARTIAL_IDX).unwrap();
    t.tip_client().withdraw(&alice, &partial_token, &PARTIAL_AMOUNT);

    let tracked = t.tip_client().get_all_tokens(&alice);
    assert_eq!(tracked.len(), N);
    for i in 0..N {
        let token = token_addrs.get(i).unwrap();
        let expected = if i == PARTIAL_IDX { 1_000 - PARTIAL_AMOUNT } else { 1_000 };
        assert_eq!(t.tip_client().get_balance(&alice, &token), expected);
        assert!(
            tracked.contains(&token),
            "partial withdraw must not remove token #{i} from the set"
        );
    }
}

#[test]
fn test_unregister_after_withdrawing_all_many_tokens() {
    // After fully withdrawing 11 distinct tokens, the storage entry backing
    // the tracked token set should be removed; indirectly verified by
    // `unregister` succeeding, since it iterates the tracked token set.
    const N: u32 = 11;

    let t = TestEnv::new();
    let alice = Address::generate(&t.env);
    t.tip_client().register(
        &alice,
        &Symbol::new(&t.env, "alice"),
        &s(&t.env, "Alice"),
        &s(&t.env, ""),
    );

    let token_addrs = seed_creator_with_many_tokens(&t, &alice, N as usize);
    assert_eq!(token_addrs.len(), N);

    for i in 0..N {
        let token = token_addrs.get(i).unwrap();
        t.tip_client().withdraw(&alice, &token, &1_000);
    }

    assert_eq!(t.tip_client().get_all_tokens(&alice).len(), 0);

    // `unregister` iterates the tracked token set to verify all balances
    // are zero. If the map-backed removal left an inconsistent state this
    // would panic with BalanceNotEmpty (#13).
    t.tip_client().unregister(&alice);
    assert!(!t.tip_client().is_creator(&alice));
}
