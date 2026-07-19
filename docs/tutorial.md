# StellarTip Tutorial

A complete, end-to-end walkthrough of the StellarTip contract on **Stellar testnet**.
By the end you will have:

1. Deployed the contract
2. Initialized it as admin
3. Registered a creator profile
4. Sent a tip from a supporter to the creator
5. Withdrawn the tip to the creator's wallet

Every command is paste-able into your terminal. Expected outputs are shown in
fenced blocks.

---

## Prerequisites

| Tool | Install |
|------|---------|
| **Rust (nightly)** | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` then `rustup toolchain install nightly --target wasm32-unknown-unknown` |
| **Stellar CLI** | `cargo install --locked stellar-cli@22.8.2` (or latest 22.x) |

> **Tip:** The `rust-toolchain.toml` in the repo root pins nightly and the
> wasm32 target automatically when you run `cargo build` inside the repo.

Verify both tools are installed:

```bash
rustc --version
stellar --version
```

---

## Step 1 — Clone & Build

```bash
git clone https://github.com/StellarTips/StellarTip-Contract.git
cd StellarTip-Contract
```

Build the WASM artifact:

```bash
make wasm-build
```

```
   Compiling stellar-tip v0.1.0
    Finished release [optimized] target(s) in 12.34s
```

Confirm the artifact exists:

```bash
ls -lh target/wasm32-unknown-unknown/release/stellar_tip.wasm
```

```
-rwxr-xr-x  1 user  staff  18K Jul 18 12:00 target/wasm32-unknown-unknown/release/stellar_tip.wasm
```

---

## Step 2 — Create Testnet Identities

We need two identities: one for the **admin** (deploys and configures the
contract) and one for the **supporter** (sends tips).

```bash
# Generate keys (skip if you already have identities)
stellar keys generate admin --network testnet --fund
stellar keys generate supporter --network testnet --fund
```

The `--fund` flag automatically requests testnet XLM from the friendbot.

Verify both accounts are funded:

```bash
stellar keys fund admin --network testnet
stellar keys fund supporter --network testnet
```

You can also check balances:

```bash
stellar keys address admin
# → e.g. GA6QYEZ...admin...address
stellar keys address supporter
# → e.g. GB3KJET...supporter...address
```

---

## Step 3 — Deploy the Contract

Deploy using the included script (or manually with `stellar contract deploy`):

```bash
./scripts/deploy.sh testnet admin
```

```
Deploying StellarTip contract to 'testnet' using identity 'admin'...
WASM: target/wasm32-unknown-unknown/release/stellar_tip.wasm (18K)

CCONTRACT_ID: CXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX

Deploy complete. Copy the contract ID printed above for use in your application.
```

> **Important:** Copy the contract ID (`CXXX...`) printed by the deploy command.
> Every subsequent command uses it. Export it as a shell variable for convenience:

```bash
export CONTRACT_ID="CXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
```

---

## Step 4 — Initialize the Contract

The contract must be initialized **once** by the admin. This sets the admin
address, fee recipient, platform fee, and storage caps.

```bash
stellar contract invoke \
  --id "$CONTRACT_ID" \
  --network testnet \
  --source admin \
  -- \
  init \
  --caller "$(stellar keys address admin)" \
  --fee_recipient "$(stellar keys address admin)" \
  --fee_bps 250 \
  --max_creators 0 \
  --max_tips_per_creator 0 \
  --min_tip_amount 1
```

| Parameter | Meaning |
|-----------|---------|
| `fee_bps 250` | 2.5 % platform fee (250 basis points). Use `0` for no fee. |
| `max_creators 0` | Unlimited creator registrations (`0` = no cap). |
| `max_tips_per_creator 0` | Unlimited tip history per creator. |
| `min_tip_amount 1` | Minimum tip is 1 stroop (the smallest XLM unit). |

Verify initialization:

```bash
stellar contract invoke \
  --id "$CONTRACT_ID" \
  --network testnet \
  --source admin \
  -- \
  get_admin
```

```
"GA6QYEZ...admin...address"
```

```bash
stellar contract invoke \
  --id "$CONTRACT_ID" \
  --network testnet \
  --source admin \
  -- \
  get_fee_percentage
```

```
250
```

```bash
stellar contract invoke \
  --id "$CONTRACT_ID" \
  --network testnet \
  --source admin \
  -- \
  get_contract_version
```

```
3
```

---

## Step 5 — Register a Creator

We'll register a creator profile. For this tutorial, the **admin** account also
acts as the creator (you could use a separate identity).

```bash
stellar contract invoke \
  --id "$CONTRACT_ID" \
  --network testnet \
  --source admin \
  -- \
  register \
  --caller "$(stellar keys address admin)" \
  --username "alice" \
  --display_name "Alice" \
  --bio "Digital artist"
```

No output means success. Verify the profile was created:

```bash
stellar contract invoke \
  --id "$CONTRACT_ID" \
  --network testnet \
  --source admin \
  -- \
  get_profile \
  --address "$(stellar keys address admin)"
```

```json
{
  "username": "alice",
  "display_name": "Alice",
  "bio": "Digital artist",
  "registered_at": 1234567890
}
```

You can also look up by username:

```bash
stellar contract invoke \
  --id "$CONTRACT_ID" \
  --network testnet \
  --source admin \
  -- \
  is_username_taken \
  --username "alice"
```

```
true
```

---

## Step 6 — Get the Native Token Address (XLM SAC)

To tip in XLM we need the address of the **Stellar Asset Contract** (SAC) for
the native asset on testnet.

```bash
export XLM_TOKEN="CAS3J7GYLGXMF6TDJBBYYSE3HQ6BBSMLNUQ34T6TZMYMW2EVH34XOWMA"
```

> **Note:** The native XLM SAC address above is for **testnet**. On mainnet the
> address is different. Check the
> [Stellar docs](https://developers.stellar.org/docs/tokens/stellar-asset-contract)
> for the correct SAC address for your network.

---

## Step 7 — Tip the Creator

The **supporter** sends 10 XLM (10 000 000 stroops) to the creator "alice":

```bash
stellar contract invoke \
  --id "$CONTRACT_ID" \
  --network testnet \
  --source supporter \
  -- \
  tip \
  --from "$(stellar keys address supporter)" \
  --creator "$(stellar keys address admin)" \
  --token "$XLM_TOKEN" \
  --amount 10000000 \
  --message "Love your art!"
```

The return value is the tip's **index** in the creator's tip history:

```
0
```

---

## Step 8 — Check the Creator's Balance

```bash
stellar contract invoke \
  --id "$CONTRACT_ID" \
  --network testnet \
  --source admin \
  -- \
  get_balance \
  --creator "$(stellar keys address admin)" \
  --token "$XLM_TOKEN"
```

```
9750000
```

> The balance is **9 750 000 stroops** (9.75 XLM) because the contract
> deducts the 2.5 % platform fee (250 bps) from every tip:
>
> - Tip amount: 10 000 000 stroops
> - Fee (2.5 %): 250 000 stroops → sent to fee recipient
> - Creator credit: 9 750 000 stroops

---

## Step 9 — View Tip History

```bash
stellar contract invoke \
  --id "$CONTRACT_ID" \
  --network testnet \
  --source admin \
  -- \
  get_tip_count \
  --creator "$(stellar keys address admin)"
```

```
1
```

```bash
stellar contract invoke \
  --id "$CONTRACT_ID" \
  --network testnet \
  --source admin \
  -- \
  get_tip \
  --creator "$(stellar keys address admin)" \
  --index 0
```

```json
{
  "from": "GB3KJET...supporter...address",
  "token": "CAS3J7GYLGXMF6TDJBBYYSE3HQ6BBSMLNUQ34T6TZMYMW2EVH34XOWMA",
  "amount": "10000000",
  "message": "Love your art!",
  "timestamp": 1234567890
}
```

---

## Step 10 — Withdraw Tips

The creator withdraws their accumulated XLM balance to their own wallet:

```bash
stellar contract invoke \
  --id "$CONTRACT_ID" \
  --network testnet \
  --source admin \
  -- \
  withdraw \
  --caller "$(stellar keys address admin)" \
  --token "$XLM_TOKEN" \
  --amount 9750000
```

No output means success. Verify the internal balance is now zero:

```bash
stellar contract invoke \
  --id "$CONTRACT_ID" \
  --network testnet \
  --source admin \
  -- \
  get_balance \
  --creator "$(stellar keys address admin)" \
  --token "$XLM_TOKEN"
```

```
0
```

---

## Step 11 — Verify the Tokens Arrived

Check the creator's wallet balance on the network:

```bash
stellar account show --network testnet --source admin
```

You should see that the native XLM balance reflects the withdrawn amount
(minus any base reserves and transaction fees).

---

## Summary of the Flow

```
Admin (deployer)               Supporter                  StellarTip Contract
      │                            │                            │
      │── deploy ──────────────────│────────────────────────────>│
      │── init(admin, fee 2.5%) ──>│                            │
      │── register(alice) ────────>│                            │
      │                            │                            │
      │                            │── tip(10 XLM, "Love!") ──>│
      │                            │                            │── fee → admin
      │                            │                            │── credit alice
      │                            │                            │
      │<── withdraw(9.75 XLM) ────│                            │
      │                            │                            │── send XLM → alice
```

---

## Next Steps

- **Multi-token tipping:** Tip with USDC or any Stellar asset by passing
  its SAC address as the `--token` argument.
- **Multiple creators:** Register additional addresses with unique usernames.
- **Admin tuning:** Adjust fees, caps, and minimums with
  `set_fee_percentage`, `set_max_creators`, `set_max_tips_per_creator`,
  and `set_min_tip_amount`.
- **Read the full API:** See the [Contract Interface](../README.md#contract-interface)
  table in the README for every available function.

---

## Troubleshooting

| Error | Cause | Fix |
|-------|-------|-----|
| `#9` AlreadyInitialized | Contract was already initialized | You can only call `init` once. Redeploy the contract if you need a fresh start. |
| `#1` CreatorAlreadyExists | Address already registered | Each address can only register once. Use `unregister` first (requires zero balances). |
| `#3` UsernameTaken | Username already claimed | Choose a different username. |
| `#14` CapExceeded | Creator or tip cap reached | Ask the admin to raise the cap with `set_max_creators` / `set_max_tips_per_creator`, or pass `0` to disable. |
| `#10` Paused | Contract is paused | Wait for the admin to call `unpause`. |
| `#4` InsufficientBalance | Withdrawal exceeds internal balance | Check `get_balance` before withdrawing. |
| `#16` BelowMinimum | Tip amount below configured minimum | Increase the tip amount or ask the admin to lower `set_min_tip_amount`. |
