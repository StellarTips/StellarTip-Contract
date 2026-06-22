#![cfg(test)]

use proptest::prelude::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger, LedgerInfo},
    token,
    token::StellarAssetClient,
    Address, Env, String, Symbol,
};

use crate::{TipContract, TipContractClient};

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
        let contract_id = env.register(TipContract, ());

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
}
