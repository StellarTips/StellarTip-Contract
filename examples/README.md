# StellarTip SDK Examples

Runnable, end-to-end client examples for invoking the StellarTip contract.
Each example walks through the same **register → tip → read back** workflow,
but from a different integration perspective so you can copy-paste whichever
fits your stack:

| Path                              | Stack                                                | Use case                                                |
|-----------------------------------|------------------------------------------------------|---------------------------------------------------------|
| `typescript/register-and-tip.ts`  | [`@stellar/stellar-sdk`](https://www.npmjs.com/package/@stellar/stellar-sdk) | Browser / Node wallet dApp                              |
| `python/register_and_tip.py`      | [`stellar-sdk`](https://pypi.org/project/stellar-sdk/) (>= 9)              | Server-side tip orchestration (e.g. moderation bot)     |
| `rust/src/lib.rs` (+ `rust/Cargo.toml`) | [`soroban-sdk`](https://crates.io/crates/soroban-sdk) (=21.7.7)      | Cross-contract integration from another Soroban contract |

> **Prerequisite:** a StellarTip contract has been deployed and initialized
> on a Stellar network. Follow [`docs/tutorial.md`](../docs/tutorial.md)
> through **Step 4** (Initialize the Contract) once before running any
> example below.

## Environment variables

Every example below reads its configuration from environment variables so the
source stays self-contained:

| Variable          | Meaning                                                                          |
|-------------------|----------------------------------------------------------------------------------|
| `CONTRACT_ID`     | StellarTip contract ID (`C…`); printed by `scripts/deploy.sh` or `stellar contract deploy`. |
| `SECRET_KEY`      | Signer secret key (`S…`) for the actor invoking the contract. **Never commit a real secret.** |
| `PUBLIC_KEY`      | Signer public key (`G…`); the value passed as `caller`/`from`.                   |
| `XLM_TOKEN`       | Native XLM **Stellar Asset Contract** address on the target network.             |
| `SOROBAN_RPC_URL` | Soroban RPC endpoint, e.g. `https://soroban-testnet.stellar.org` (testnet).      |

Testnet values:

* `XLM_TOKEN` = `CAS3J7GYLGXMF6TDJBBYYSE3HQ6BBSMLNUQ34T6TZMYMW2EVH34XOWMA`
* `SOROBAN_RPC_URL` = `https://soroban-testnet.stellar.org`

See [`docs/tutorial.md`](../docs/tutorial.md) for how to look up the values
for other networks (futurenet / mainnet).

## Running

### TypeScript (Node)

```bash
cd examples/typescript
npm install @stellar/stellar-sdk ts-node typescript
export CONTRACT_ID=... SECRET_KEY=... PUBLIC_KEY=... XLM_TOKEN=... SOROBAN_RPC_URL=https://soroban-testnet.stellar.org
npx ts-node register-and-tip.ts
```

### Python

```bash
cd examples/python
python3 -m pip install 'stellar-sdk>=9'
export CONTRACT_ID=... SECRET_KEY=... PUBLIC_KEY=... XLM_TOKEN=... SOROBAN_RPC_URL=https://soroban-testnet.stellar.org
python3 register_and_tip.py
```

### Rust (cross-contract)

The Rust example is a standalone (`cdylib`) Soroban contract crate under
`examples/rust/` that calls StellarTip's methods via
`soroban_sdk::contractimport!`. To try it:

```bash
# 1. From the repo root, build the StellarTip WASM first; the contractimport!
#    macro reads the exported method shapes from the artifact on disk.
make wasm-build

# 2. Then compile the orchestrator crate.
cargo build \
  --release \
  --target wasm32-unknown-unknown \
  --manifest-path examples/rust/Cargo.toml
```

The orchestrator wraps StellarTip's `tip()` and `register()` so a single
host call lands the configured fee, credits the creator, and emits the
same `TIP` event as a direct invocation. Deploy the orchestrator somehow
(soroban-cli / stellar-cli, or your own pipeline) and invoke with
`stellar contract invoke` — a representative invocation is in the
comment block at the bottom of `examples/rust/src/lib.rs`.

## Verifying outputs

Once each example completes, you can verify on-chain state with the Stellar
CLI (matches the tutorial):

```bash
stellar contract invoke --id "$CONTRACT_ID" --network testnet --source alice -- \
  get_tip_count --creator "$PUBLIC_KEY"
# → 1   (after the TS/Python examples have run once)

stellar contract invoke --id "$CONTRACT_ID" --network testnet --source alice -- \
  get_profile --address "$PUBLIC_KEY"
# → { username: 'alice', display_name: 'Alice', bio: 'Digital artist', registered_at: <ledger_ts> }
```
