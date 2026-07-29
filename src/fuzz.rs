#![cfg(test)]

use proptest::prelude::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger, LedgerInfo},
    token,
    token::StellarAssetClient,
    Address, Env, String, Symbol,
};

use crate::{TipContract, TipContractClient};

/// Extract a string message from a caught panic payload.
fn panic_message(e: Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = e.downcast_ref::<String>() {
        s.clone()
    } else if let Some(s) = e.downcast_ref::<&str>() {
        s.to_string()
    } else {
        "<unknown panic payload>".to_string()
    }
}

/// Convenience: create a `String` from a `&str`.
fn s(env: &Env, text: &str) -> String {
    String::from_str(env, text)
}

struct FuzzEnv {
    env: Env,
    contract_id: Address,
    admin: Address,
    fee_recipient: Address,
    token_id: Address,
}

impl FuzzEnv {
    fn new(fee_bps: u32) -> Self {
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
        let contract_id = env.register_contract(None, TipContract);

        let token_admin = Address::generate(&env);
        let token_contract = env.register_stellar_asset_contract_v2(token_admin.clone());
        let token_id = token_contract.address();

        let sac = StellarAssetClient::new(&env, &token_id);
        sac.mint(&admin, &1_000_000_000_000_000_000); // large amount of tokens

        let t = FuzzEnv { env, contract_id, admin, fee_recipient, token_id };
        t.tip_client().init(&t.admin, &t.fee_recipient, &fee_bps, &0u32, &0u32, &0i128);
        t
    }

    fn tip_client(&self) -> TipContractClient<'_> {
        TipContractClient::new(&self.env, &self.contract_id)
    }

    fn token_client(&self) -> token::Client<'_> {
        token::Client::new(&self.env, &self.token_id)
    }

    fn stellar_client(&self) -> StellarAssetClient<'_> {
        StellarAssetClient::new(&self.env, &self.token_id)
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10000))]

    #[test]
    fn test_tip_balance_invariant(
        amount in 1..1_000_000_000_000i128,
        fee_bps in 0..10_000u32,
    ) {
        let t = FuzzEnv::new(fee_bps);

        let creator = Address::generate(&t.env);
        t.tip_client().register(&creator, &Symbol::new(&t.env, "creator"), &s(&t.env, "Creator"), &s(&t.env, "Bio"));

        let tipper = Address::generate(&t.env);
        t.stellar_client().mint(&tipper, &amount);

        let fee_recipient_balance_before = t.token_client().balance(&t.fee_recipient);

        // Tip
        t.tip_client().tip(&tipper, &creator, &t.token_id, &amount, &s(&t.env, "Thanks!"));

        // Verifications
        let fee = (amount * (fee_bps as i128)) / 10000;
        let expected_creator_balance = amount - fee;

        // Verify internal creator balance
        let internal_balance = t.tip_client().get_balance(&creator, &t.token_id);
        prop_assert_eq!(internal_balance, expected_creator_balance);

        // Verify fee recipient received the fee
        let fee_recipient_balance_after = t.token_client().balance(&t.fee_recipient);
        prop_assert_eq!(fee_recipient_balance_after - fee_recipient_balance_before, fee);

        // Verify contract token balance
        let contract_balance = t.token_client().balance(&t.contract_id);
        prop_assert_eq!(contract_balance, expected_creator_balance);
    }

    #[test]
    fn test_withdraw_balance_invariant(
        amount in 1..1_000_000_000_000i128,
        withdraw_amount in 1..1_000_000_000_000i128,
        fee_bps in 0..10_000u32,
    ) {
        let t = FuzzEnv::new(fee_bps);

        let creator = Address::generate(&t.env);
        t.tip_client().register(&creator, &Symbol::new(&t.env, "creator"), &s(&t.env, "Creator"), &s(&t.env, "Bio"));

        let tipper = Address::generate(&t.env);
        t.stellar_client().mint(&tipper, &amount);

        t.tip_client().tip(&tipper, &creator, &t.token_id, &amount, &s(&t.env, "Thanks!"));

        let fee = (amount * (fee_bps as i128)) / 10000;
        let expected_creator_balance = amount - fee;

        let withdraw_amount = withdraw_amount.min(expected_creator_balance);

        prop_assume!(withdraw_amount > 0);

        let creator_token_balance_before = t.token_client().balance(&creator);
        let contract_token_balance_before = t.token_client().balance(&t.contract_id);

        t.tip_client().withdraw(&creator, &t.token_id, &withdraw_amount);

        let creator_token_balance_after = t.token_client().balance(&creator);
        let contract_token_balance_after = t.token_client().balance(&t.contract_id);
        let internal_balance_after = t.tip_client().get_balance(&creator, &t.token_id);

        prop_assert_eq!(creator_token_balance_after - creator_token_balance_before, withdraw_amount);
        prop_assert_eq!(contract_token_balance_before - contract_token_balance_after, withdraw_amount);
        prop_assert_eq!(internal_balance_after, expected_creator_balance - withdraw_amount);
    }

    #[test]
    fn test_combined_flow(
        tip1 in 1..1_000_000_000_000i128,
        tip2 in 1..1_000_000_000_000i128,
        tip3 in 1..1_000_000_000_000i128,
        fee_bps in 0..10_000u32,
        withdraw_pct in 0..100u8,
    ) {
        let t = FuzzEnv::new(fee_bps);

        let creator = Address::generate(&t.env);
        t.tip_client().register(&creator, &Symbol::new(&t.env, "creator"), &s(&t.env, "Creator"), &s(&t.env, "Bio"));

        let tipper = Address::generate(&t.env);
        let total_tip = tip1 + tip2 + tip3;
        t.stellar_client().mint(&tipper, &total_tip);

        t.tip_client().tip(&tipper, &creator, &t.token_id, &tip1, &s(&t.env, "T1"));
        t.tip_client().tip(&tipper, &creator, &t.token_id, &tip2, &s(&t.env, "T2"));
        t.tip_client().tip(&tipper, &creator, &t.token_id, &tip3, &s(&t.env, "T3"));

        let fee1 = (tip1 * (fee_bps as i128)) / 10000;
        let fee2 = (tip2 * (fee_bps as i128)) / 10000;
        let fee3 = (tip3 * (fee_bps as i128)) / 10000;

        let expected_internal = (tip1 - fee1) + (tip2 - fee2) + (tip3 - fee3);
        prop_assert_eq!(t.tip_client().get_balance(&creator, &t.token_id), expected_internal);
        prop_assert_eq!(t.token_client().balance(&t.contract_id), expected_internal);

        if expected_internal > 0 {
            let partial_withdraw = (expected_internal * (withdraw_pct as i128)) / 100;
            if partial_withdraw > 0 {
                t.tip_client().withdraw(&creator, &t.token_id, &partial_withdraw);
                prop_assert_eq!(t.tip_client().get_balance(&creator, &t.token_id), expected_internal - partial_withdraw);
                prop_assert_eq!(t.token_client().balance(&t.contract_id), expected_internal - partial_withdraw);
            }

            let remaining = t.tip_client().get_balance(&creator, &t.token_id);
            if remaining > 0 {
                t.tip_client().withdraw(&creator, &t.token_id, &remaining);
            }

            prop_assert_eq!(t.tip_client().get_balance(&creator, &t.token_id), 0);
            prop_assert_eq!(t.token_client().balance(&t.contract_id), 0);
        }
    }

    // -----------------------------------------------------------------------
    // Property: pause invariant (issue #96)
    // -----------------------------------------------------------------------
    //
    // Once the contract is paused, every state-changing user operation
    // (register, tip, withdraw, update_profile, unregister) must panic
    // with Paused (#10).  The admin-only unpause function is excluded
    // because it is the intended escape hatch.

    #[test]
    fn test_pause_invariant_all_user_ops_rejected(
        amount in 1..100_000_000i128,
        fee_bps in 0..2_500u32,
    ) {
        let t = FuzzEnv::new(fee_bps);

        // Register a creator and send one tip — the creator needs a balance
        // so that withdraw/unregister tests are meaningful after pause.
        let creator = Address::generate(&t.env);
        t.tip_client().register(&creator, &Symbol::new(&t.env, "creator"), &s(&t.env, "Creator"), &s(&t.env, "Bio"));

        let tipper = Address::generate(&t.env);
        t.stellar_client().mint(&tipper, &amount);
        t.tip_client().tip(&tipper, &creator, &t.token_id, &amount, &s(&t.env, "pre-pause tip"));

        // Capture pre-pause state for later comparison.
        let balance_before = t.tip_client().get_balance(&creator, &t.token_id);
        let creator_count_before = t.tip_client().get_creator_count();

        // Pause the contract.
        t.tip_client().pause(&t.admin);
        prop_assert!(t.tip_client().is_paused());

        // ----- register -----
        let new_creator = Address::generate(&t.env);
        let reg_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            t.tip_client().register(&new_creator, &Symbol::new(&t.env, "newbie"), &s(&t.env, "New"), &s(&t.env, ""));
        }));
        prop_assert!(reg_result.is_err(), "register should panic when paused");
        prop_assert!(
            panic_message(reg_result.unwrap_err()).contains("#10"),
            "register should panic with Paused (#10)"
        );

        // ----- tip -----
        let tip_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            t.tip_client().tip(&tipper, &creator, &t.token_id, &1, &s(&t.env, "tip-while-paused"));
        }));
        prop_assert!(tip_result.is_err(), "tip should panic when paused");
        prop_assert!(
            panic_message(tip_result.unwrap_err()).contains("#10"),
            "tip should panic with Paused (#10)"
        );

        // ----- withdraw -----
        if balance_before > 0 {
            let wd_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                t.tip_client().withdraw(&creator, &t.token_id, &1);
            }));
            prop_assert!(wd_result.is_err(), "withdraw should panic when paused");
            prop_assert!(
                panic_message(wd_result.unwrap_err()).contains("#10"),
                "withdraw should panic with Paused (#10)"
            );
        }

        // ----- update_profile -----
        let up_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            t.tip_client().update_profile(&creator, &s(&t.env, "NewName"), &s(&t.env, "NewBio"));
        }));
        prop_assert!(up_result.is_err(), "update_profile should panic when paused");
        prop_assert!(
            panic_message(up_result.unwrap_err()).contains("#10"),
            "update_profile should panic with Paused (#10)"
        );

        // ----- unregister -----
        let unreg_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            t.tip_client().unregister(&creator);
        }));
        prop_assert!(unreg_result.is_err(), "unregister should panic when paused");
        prop_assert!(
            panic_message(unreg_result.unwrap_err()).contains("#10"),
            "unregister should panic with Paused (#10)"
        );

        // State must be unchanged after all failed ops.
        prop_assert_eq!(t.tip_client().get_balance(&creator, &t.token_id), balance_before);
        prop_assert_eq!(t.tip_client().get_creator_count(), creator_count_before);
        prop_assert!(t.tip_client().is_creator(&creator));

        // Unpause, then verify operations succeed again.
        t.tip_client().unpause(&t.admin);
        prop_assert!(!t.tip_client().is_paused());

        // After unpause: tip must succeed.
        let tip2_amount = 10i128.min(amount);
        t.stellar_client().mint(&tipper, &tip2_amount);
        t.tip_client().tip(&tipper, &creator, &t.token_id, &tip2_amount, &s(&t.env, "post-unpause tip"));
        let expected_after = balance_before + tip2_amount - (tip2_amount * (fee_bps as i128)) / 10000;
        prop_assert_eq!(t.tip_client().get_balance(&creator, &t.token_id), expected_after);
    }
}
