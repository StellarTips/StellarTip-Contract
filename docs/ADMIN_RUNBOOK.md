# StellarTip administrator runbook

This runbook is for operators responding to an administrator or platform
incident. It covers the actions exposed by the current contract; it does not
replace an incident-response policy or key-custody procedure.

> [!CAUTION]
> Never paste a secret key or seed phrase into a ticket, chat, shell history,
> environment variable, or this repository. The values named `ADMIN_ID` and
> `NEW_ADMIN_ID` below are local Stellar CLI identity aliases, not secrets.
> Prefer a hardware-backed or appropriately controlled production signer.

## Command setup and safety gates

Run all write operations from a clean, access-controlled workstation with a
current [Stellar CLI](https://developers.stellar.org/docs/tools/cli/install-cli).
Set these values for the affected deployment:

```bash
export NETWORK=testnet                 # use mainnet only after rehearsal
export CONTRACT_ID=C...                # affected StellarTip contract
export ADMIN_ID=stellar-tip-admin      # current admin's local identity alias
export ADMIN_ADDRESS="$(stellar keys public-key "$ADMIN_ID")"
```

Expected output from `stellar keys public-key` is the current admin's `G...`
public address. It must not print or request an `S...` secret key.

Before any write:

1. Open an incident log and record the UTC time, network, contract ID, current
   ledger, operator, reason, and intended change.
2. Have a second operator verify `NETWORK` and `CONTRACT_ID` against the
   deployment inventory or block explorer.
3. Read the live state. Stop if `get_admin` does not equal `ADMIN_ADDRESS`.

```bash
stellar contract invoke \
  --id "$CONTRACT_ID" \
  --network "$NETWORK" \
  --source-account "$ADMIN_ID" \
  -- \
  get_admin

stellar contract invoke \
  --id "$CONTRACT_ID" \
  --network "$NETWORK" \
  --source-account "$ADMIN_ID" \
  -- \
  is_paused

stellar contract invoke \
  --id "$CONTRACT_ID" \
  --network "$NETWORK" \
  --source-account "$ADMIN_ID" \
  -- \
  get_fee_percentage

stellar contract invoke \
  --id "$CONTRACT_ID" \
  --network "$NETWORK" \
  --source-account "$ADMIN_ID" \
  -- \
  get_fee_recipient
```

Expected outputs, in order, are the current admin `G...` address, `true` or
`false`, an integer from `0` through `10000` (basis points), and the fee
recipient `G...` address. Save the outputs in the incident log.

All successful admin writes below return successfully (the functions return no
value). Treat the subsequent read command—not an absent error message—as the
proof that the new state is live. A failed authorization or contract error means
the change did not complete; do not repeatedly resubmit without diagnosing it.

## Scenario 1: administrator key compromised

### Contain

If the current admin signer is still under operator control, pause immediately:

```bash
stellar contract invoke \
  --id "$CONTRACT_ID" \
  --network "$NETWORK" \
  --source-account "$ADMIN_ID" \
  -- \
  pause \
  --caller "$ADMIN_ADDRESS"

stellar contract invoke \
  --id "$CONTRACT_ID" \
  --network "$NETWORK" \
  --source-account "$ADMIN_ID" \
  -- \
  is_paused
```

Expected verification output: `true`. Pause blocks `register`, `tip`, and
`withdraw`; it does **not** block admin setters, so key rotation can continue
while the contract is paused.

Create or select a replacement signer through the approved custody process.
Only its public address is needed here:

```bash
export NEW_ADMIN_ID=stellar-tip-admin-next
export NEW_ADMIN_ADDRESS="$(stellar keys public-key "$NEW_ADMIN_ID")"
```

Expected output: a different `G...` address. A second operator must compare it
with the custody record before rotation.

### Rotate and verify

```bash
stellar contract invoke \
  --id "$CONTRACT_ID" \
  --network "$NETWORK" \
  --source-account "$ADMIN_ID" \
  -- \
  set_admin \
  --caller "$ADMIN_ADDRESS" \
  --new_admin "$NEW_ADMIN_ADDRESS"

stellar contract invoke \
  --id "$CONTRACT_ID" \
  --network "$NETWORK" \
  --source-account "$NEW_ADMIN_ID" \
  -- \
  get_admin
```

Expected verification output: `NEW_ADMIN_ADDRESS`. From this point on, set
`ADMIN_ID="$NEW_ADMIN_ID"` and `ADMIN_ADDRESS="$NEW_ADMIN_ADDRESS"`; the old
admin can no longer authorize admin calls.

Before unpausing, inspect the fee recipient and fee percentage for attacker
changes. Restore approved values if necessary using the fee-spike procedure.
Revoke the compromised signer from custody systems, rotate related credentials,
and retain relevant logs.

```bash
stellar contract invoke \
  --id "$CONTRACT_ID" \
  --network "$NETWORK" \
  --source-account "$ADMIN_ID" \
  -- \
  unpause \
  --caller "$ADMIN_ADDRESS"

stellar contract invoke \
  --id "$CONTRACT_ID" \
  --network "$NETWORK" \
  --source-account "$ADMIN_ID" \
  -- \
  is_paused
```

Expected verification output after an approved recovery: `false`.

### If the current admin is no longer controllable

Do not claim that rotation or pause succeeded. The contract has no secondary
admin, recovery key, timelock, or upgrade/recovery entrypoint. If `get_admin`
shows an attacker-controlled address or the current signer cannot authorize:

1. Mark the deployment compromised and stop frontends/indexers from presenting
   it as safe.
2. Notify maintainers, integrators, and users through verified channels.
3. Preserve the last known state and transaction/event evidence.
4. Prepare a replacement deployment, independent review, and explicit migration
   plan. Never ask users to send funds to a replacement until its contract ID is
   published through verified project channels.

## Scenario 2: unexpected fee spike

A value of `100` is 1%; `10000` is 100%. First verify the live value and
recipient. If the fee is unexpected, pause before changing configuration so no
new tips are processed at the wrong rate.

```bash
stellar contract invoke \
  --id "$CONTRACT_ID" \
  --network "$NETWORK" \
  --source-account "$ADMIN_ID" \
  -- \
  get_fee_percentage

stellar contract invoke \
  --id "$CONTRACT_ID" \
  --network "$NETWORK" \
  --source-account "$ADMIN_ID" \
  -- \
  get_fee_recipient

stellar contract invoke \
  --id "$CONTRACT_ID" \
  --network "$NETWORK" \
  --source-account "$ADMIN_ID" \
  -- \
  pause \
  --caller "$ADMIN_ADDRESS"
```

Expected outputs: the observed basis-point value, current recipient address,
and then a successful pause. Confirm `is_paused` returns `true` as shown above.

Set a conservative containment fee of zero, or replace `0` with the fee rate
approved in the incident record:

```bash
export APPROVED_FEE_BPS=0

stellar contract invoke \
  --id "$CONTRACT_ID" \
  --network "$NETWORK" \
  --source-account "$ADMIN_ID" \
  -- \
  set_fee_percentage \
  --caller "$ADMIN_ADDRESS" \
  --fee_bps "$APPROVED_FEE_BPS"

stellar contract invoke \
  --id "$CONTRACT_ID" \
  --network "$NETWORK" \
  --source-account "$ADMIN_ID" \
  -- \
  get_fee_percentage
```

Expected verification output: `0` (or the approved integer). Values greater
than `10000` are rejected with `TipError::InvalidInput` and must never be used as
a retry target.

If the recipient was also changed, restore the verified public address:

```bash
export APPROVED_FEE_RECIPIENT=G...

stellar contract invoke \
  --id "$CONTRACT_ID" \
  --network "$NETWORK" \
  --source-account "$ADMIN_ID" \
  -- \
  set_fee_recipient \
  --caller "$ADMIN_ADDRESS" \
  --fee_recipient "$APPROVED_FEE_RECIPIENT"

stellar contract invoke \
  --id "$CONTRACT_ID" \
  --network "$NETWORK" \
  --source-account "$ADMIN_ID" \
  -- \
  get_fee_recipient
```

Expected verification output: `APPROVED_FEE_RECIPIENT`. Investigate how the
configuration changed, account for already processed fees, and obtain incident
lead approval before unpausing. Changing the fee does not reverse past tips.

## Scenario 3: new administrator onboarding

Use this for a planned handover, not an active compromise.

1. Provision the new signer using the production custody policy. Test backup,
   recovery, access removal, and transaction approval outside the incident.
2. Give the operator the contract ID, network, this runbook, fee policy, and
   verified deployment inventory—never the old admin's secret.
3. Rehearse the exact sequence against a testnet deployment first.
4. Schedule a two-operator production change window and announce it internally.

Verify both addresses before the handover:

```bash
export NEW_ADMIN_ID=stellar-tip-admin-next
export NEW_ADMIN_ADDRESS="$(stellar keys public-key "$NEW_ADMIN_ID")"

stellar contract invoke \
  --id "$CONTRACT_ID" \
  --network "$NETWORK" \
  --source-account "$ADMIN_ID" \
  -- \
  get_admin
```

Expected outputs: `NEW_ADMIN_ADDRESS` is a `G...` public address and `get_admin`
still returns `ADMIN_ADDRESS`. Stop if the two addresses are identical.

Execute `set_admin` using the rotation command in Scenario 1. Expected output
from the follow-up `get_admin` call is `NEW_ADMIN_ADDRESS`. The new operator must
then prove control with a no-op state transition that does not interrupt users.
Copy the verified integer from `get_fee_percentage` into `CURRENT_FEE_BPS`; do
not use command output that has not been reviewed:

```bash
export CURRENT_FEE_BPS=0  # replace with the verified current integer

stellar contract invoke \
  --id "$CONTRACT_ID" \
  --network "$NETWORK" \
  --source-account "$NEW_ADMIN_ID" \
  -- \
  set_fee_percentage \
  --caller "$NEW_ADMIN_ADDRESS" \
  --fee_bps "$CURRENT_FEE_BPS"
```

Expected result: successful authorization with the existing fee unchanged.
Archive the transaction hashes and approvals, remove the old operator's access,
and update the deployment inventory and escalation contacts.

## Scenario 4: platform incident response

Use this checklist for application compromise, bad releases, RPC/indexer
failures, abnormal tip/withdraw activity, or an incident that has not yet been
classified.

### Triage

1. Declare severity and appoint an incident lead, chain operator, communicator,
   and recorder. Use an out-of-band channel if the platform may be compromised.
2. Verify the report on-chain or through an independent RPC/explorer. Record
   transaction hashes, ledgers, timestamps, contract ID, frontend release, and
   affected accounts. Do not copy secret material into the log.
3. Run the four read-only preflight commands and compare their outputs with the
   approved deployment inventory.
4. If continued `register`, `tip`, or `withdraw` calls could cause harm, invoke
   `pause` and verify `is_paused` returns `true`.

### Stabilize and recover

- Suspected admin compromise: follow Scenario 1.
- Unexpected fee or recipient: follow Scenario 2.
- Frontend-only issue: disable the affected UI path, but do not state that the
  contract is paused unless the on-chain query returns `true`.
- RPC/indexer issue: compare at least two trusted sources; do not mutate contract
  state merely to repair stale off-chain data.
- Contract defect without a safe admin mitigation: keep the deployment paused,
  prepare a reviewed replacement deployment and migration plan, and communicate
  the new contract ID only through verified channels.

Before recovery, document the root cause or explicit risk acceptance, validate
the admin/fee/recipient state, test the fix on testnet, and obtain incident-lead
approval. Then unpause and verify `is_paused` returns `false`. Closely monitor the
first post-recovery transactions and publish a post-incident review without
secrets or exploitable details.

## Fire-drill tests

The repository already has focused unit tests for every admin action used by
this runbook. They do not contact a network or expose a real key.

```bash
cargo test test_pause_and_unpause
cargo test test_set_admin
cargo test test_set_fee_percentage
cargo test test_set_fee_recipient
cargo test unauthorized
```

Expected result for each command: exit status `0` and a final line containing
`test result: ok`. The `unauthorized` filter must run the rejection tests and
still end in `ok`; those tests pass only when unauthorized calls are rejected.

Before merging changes to this runbook or contract, run the repository checks:

```bash
cargo fmt --all -- --check
cargo clippy --target wasm32-unknown-unknown --release -- -D warnings
cargo test
cargo build --release --target wasm32-unknown-unknown
```

Expected result: all four commands exit `0`; the complete test run ends with
`test result: ok`, and the build creates
`target/wasm32-unknown-unknown/release/stellar_tip.wasm`.
