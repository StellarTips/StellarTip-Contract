#![cfg(test)]
extern crate std;

use proptest::prelude::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger, LedgerInfo},
    token,
    token::StellarAssetClient,
    Address, Env, String, Symbol, Vec,
};

use crate::{TipContract, TipContractClient};

/// A minimal deterministic pseudo-random number generator so that
/// fuzz seeds produce reproducible sequences of operations.
struct SimpleRng {
    state: u64,
}

impl SimpleRng {
    fn new(seed: u64) -> Self {
        Self { state: seed.wrapping_add(1) }
    }

    fn next(&mut self) -> u64 {
        // SplitMix64
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
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
        sac.mint(&admin, &1_000_000_000_000_000_000);

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

    // -------------------------------------------------------------------
    // Issue #90 — i128 boundary amount handling
    // -------------------------------------------------------------------

    #[test]
    fn test_i128_boundary_amount_no_overflow(
        amount in prop_oneof![
            9 => 1i128..1_000_000_000_001i128,
            1 => Just(1i128),
            1 => Just(1_000_000_000_000i128),
        ],
    ) {
        let t = FuzzEnv::new(0);

        let creator = Address::generate(&t.env);
        t.tip_client().register(
            &creator,
            &Symbol::new(&t.env, "creator"),
            &s(&t.env, "Creator"),
            &s(&t.env, "Bio"),
        );

        let tipper = Address::generate(&t.env);
        t.stellar_client().mint(&tipper, &amount);
        t.tip_client().tip(&tipper, &creator, &t.token_id, &amount, &s(&t.env, "ok"));

        prop_assert_eq!(
            t.tip_client().get_balance(&creator, &t.token_id),
            amount,
            "amount boundary: balance mismatch after tip"
        );
    }

    // -------------------------------------------------------------------
    // Issue #95 — full token-conservation invariant
    // -------------------------------------------------------------------

    #[test]
    fn test_token_conservation_invariant(
        fee_bps in 0..10_000u32,
        num_creators in 1usize..6,
        ops in prop::collection::vec(
            (0..10usize, 0..3u8, 1i128..100_000_000i128),
            1..6,
        ),
    ) {
        let t = FuzzEnv::new(fee_bps);

        let syms = [
            Symbol::new(&t.env, "a"),
            Symbol::new(&t.env, "b"),
            Symbol::new(&t.env, "c"),
            Symbol::new(&t.env, "d"),
            Symbol::new(&t.env, "e"),
        ];
        let display = s(&t.env, "C");
        let mut creators = Vec::new(&t.env);
        for i in 0..num_creators {
            let c = Address::generate(&t.env);
            t.tip_client().register(&c, &syms[i], &display, &s(&t.env, ""));
            creators.push_back(c);
        }

        for (ci_raw, is_wd, amt) in &ops {
            let ci = ci_raw % (creators.len() as usize);
            let creator = creators.get(ci as u32).unwrap();

            if *is_wd == 0 {
                let tipper = Address::generate(&t.env);
                t.stellar_client().mint(&tipper, amt);
                t.tip_client().tip(&tipper, &creator, &t.token_id, amt, &s(&t.env, ""));
            } else {
                let bal = t.tip_client().get_balance(&creator, &t.token_id);
                if bal > 0 {
                    let wd = (bal * amt / 100_000_000i128).max(1);
                    t.tip_client().withdraw(&creator, &t.token_id, &wd);
                }
            }
        }

        let contract_balance = t.token_client().balance(&t.contract_id);
        let mut sum_internal: i128 = 0;
        for i in 0..creators.len() {
            let c = creators.get(i as u32).unwrap();
            sum_internal += t.tip_client().get_balance(&c, &t.token_id);
        }

        prop_assert_eq!(
            contract_balance, sum_internal,
            "conservation violated: contract balance mismatch for fee_bps setting"
        );
    }

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

        t.tip_client().tip(&tipper, &creator, &t.token_id, &amount, &s(&t.env, "Thanks!"));

        let fee = (amount * (fee_bps as i128)) / 10000;
        let expected_creator_balance = amount - fee;

        let internal_balance = t.tip_client().get_balance(&creator, &t.token_id);
        prop_assert_eq!(internal_balance, expected_creator_balance);

        let fee_recipient_balance_after = t.token_client().balance(&t.fee_recipient);
        prop_assert_eq!(fee_recipient_balance_after - fee_recipient_balance_before, fee);

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
    // Fuzz: paused-mid-batch scenarios (issue #92)
    // -----------------------------------------------------------------------
    //
    // Randomly interleave admin pause/unpause with user tips and assert
    // no state corruption.  The property: sum of internal balances for
    // every creator == contract's token balance (modulo fees forwarded
    // to the fee recipient).

    #[test]
    fn test_paused_mid_batch_no_state_corruption(
        seed in 0..u64::MAX,
        num_ops in 3..30usize,
        fee_bps in 0..2_500u32,
    ) {
        let t = FuzzEnv::new(fee_bps);

        // Pre-register two creators.
        let c1 = Address::generate(&t.env);
        let c2 = Address::generate(&t.env);
        t.tip_client().register(&c1, &Symbol::new(&t.env, "creator1"), &s(&t.env, "C1"), &s(&t.env, ""));
        t.tip_client().register(&c2, &Symbol::new(&t.env, "creator2"), &s(&t.env, "C2"), &s(&t.env, ""));

        let mut rng = SimpleRng::new(seed);
        let mut expected_balance1: i128 = 0;
        let mut expected_balance2: i128 = 0;
        let mut cumulative_fee: i128 = 0;

        for _ in 0..num_ops {
            let op = rng.next() % 10;

            match op {
                0 => {
                    // Pause (admin only).
                    t.tip_client().pause(&t.admin);
                }
                1 => {
                    // Unpause (admin only).
                    t.tip_client().unpause(&t.admin);
                }
                2..=4 => {
                    // Tip to c1.
                    let is_paused = t.tip_client().is_paused();
                    let tip_amount = (rng.next() % 10_000_000) as i128 + 1;
                    let tipper = Address::generate(&t.env);
                    t.stellar_client().mint(&tipper, &tip_amount);

                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        t.tip_client().tip(&tipper, &c1, &t.token_id, &tip_amount, &s(&t.env, "fuzz"));
                    }));

                    match result {
                        Ok(_) => {
                            prop_assert!(!is_paused, "tip to c1 succeeded but contract is paused");
                            let fee = (tip_amount * (fee_bps as i128)) / 10000;
                            expected_balance1 += tip_amount - fee;
                            cumulative_fee += fee;
                        }
                        Err(_) => {
                            prop_assert!(is_paused, "tip to c1 panicked but contract is not paused");
                        }
                    }
                }
                5..=7 => {
                    // Tip to c2.
                    let is_paused = t.tip_client().is_paused();
                    let tip_amount = (rng.next() % 10_000_000) as i128 + 1;
                    let tipper = Address::generate(&t.env);
                    t.stellar_client().mint(&tipper, &tip_amount);

                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        t.tip_client().tip(&tipper, &c2, &t.token_id, &tip_amount, &s(&t.env, "fuzz"));
                    }));

                    match result {
                        Ok(_) => {
                            prop_assert!(!is_paused, "tip to c2 succeeded but contract is paused");
                            let fee = (tip_amount * (fee_bps as i128)) / 10000;
                            expected_balance2 += tip_amount - fee;
                            cumulative_fee += fee;
                        }
                        Err(_) => {
                            prop_assert!(is_paused, "tip to c2 panicked but contract is not paused");
                        }
                    }
                }
                8 => {
                    // Withdraw from c1 — only if not paused.
                    let is_paused = t.tip_client().is_paused();
                    if !is_paused && expected_balance1 > 0 {
                        let wd = (rng.next() % (expected_balance1 as u64 + 1)) as i128;
                        if wd > 0 {
                            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                t.tip_client().withdraw(&c1, &t.token_id, &wd);
                            }));
                            if result.is_ok() {
                                expected_balance1 -= wd;
                            }
                        }
                    }
                }
                9 => {
                    // Withdraw from c2 — only if not paused.
                    let is_paused = t.tip_client().is_paused();
                    if !is_paused && expected_balance2 > 0 {
                        let wd = (rng.next() % (expected_balance2 as u64 + 1)) as i128;
                        if wd > 0 {
                            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                t.tip_client().withdraw(&c2, &t.token_id, &wd);
                            }));
                            if result.is_ok() {
                                expected_balance2 -= wd;
                            }
                        }
                    }
                }
                _ => unreachable!("op = rng.next() % 10 is always 0..=9"),
            }
        }

        // Final invariant: internal balances must match actual state.
        prop_assert_eq!(t.tip_client().get_balance(&c1, &t.token_id), expected_balance1);
        prop_assert_eq!(t.tip_client().get_balance(&c2, &t.token_id), expected_balance2);

        // Contract balance = sum of creator internal balances (fees already forwarded).
        let contract_balance = t.token_client().balance(&t.contract_id);
        prop_assert_eq!(contract_balance, expected_balance1 + expected_balance2);

        // Fee recipient holds all forwarded fees.
        let fee_recipient_balance = t.token_client().balance(&t.fee_recipient);
        prop_assert_eq!(fee_recipient_balance, cumulative_fee);
    }
}
