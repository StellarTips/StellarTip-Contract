# StellarTip Contracts

Soroban smart contract for a decentralized micro-tipping platform on Stellar.

## Overview

StellarTip allows creators to register profiles and receive instant micro-payments
(tips) from supporters using any Stellar asset (XLM, USDC, etc.). Tips are held in
the contract until creators withdraw them, and every tip is recorded on-chain for
transparency.

## Features

- **Creator Profiles** – register with a unique username, display name, and bio
- **Username-based lookup** – find any creator by their username
- **Multi-token tips** – supporters can tip in any Stellar asset
- **On-chain history** – every tip is permanently recorded
- **Self-custody withdrawal** – creators withdraw tips at any time
- **Events** – all actions emit standard Soroban events for indexing

## Contract Interface

### Write Functions

| Function | Description |
|----------|-------------|
| `register(username, display_name, bio)` | Register as a creator |
| `tip(creator, token, amount, message)` | Send a tip to a creator |
| `withdraw(token, amount)` | Withdraw accumulated tips for a token |

### View Functions

| Function | Description |
|----------|-------------|
| `get_profile(address)` | Get a creator's profile |
| `get_creator_from_username(username)` | Resolve a username to an address |
| `get_balance(creator, token)` | Check a creator's balance for a token |
| `get_tip_count(creator)` | Get total tips received |
| `get_tip(creator, index)` | Get a specific tip record |
| `is_creator(address)` | Check if an address is registered |
| `is_username_taken(username)` | Check if a username is claimed |

## Getting Started

### Prerequisites

- Rust (nightly) – <https://rustup.rs>
- Soroban CLI – `cargo install soroban-cli`
- Stellar CLI – `cargo install stellar-cli`

### Build

```bash
cargo build --release
```

### Test

```bash
cargo test
```

### Deploy (testnet)

```bash
stellar contract deploy \
  --wasm target/wasm32-unknown-unknown/release/stellar_tip.wasm \
  --network testnet \
  --source <your-identity>
```

## Contract Architecture

```
User (supporter)        TipContract          Token Contract
      │                      │                     │
      │── tip(creator,amt) ──│                     │
      │                      │── transfer(from) ──│
      │                      │←────── ok ─────────│
      │←────── tip index ────│                     │
      │                      │                     │
      │── withdraw(token,amt)│                     │
      │                      │── transfer(creator)│
      │←────── tokens ───────│                     │
```

Tip flow:
1. Supporter calls `tip()` – the Stellar wallet prompts them to sign the
   authorization
2. The contract calls `transfer()` on the **Stellar Asset Contract** (SAC) to
   pull tokens from the supporter into the contract
3. The creator's internal balance is updated and the tip is recorded
4. Later, the creator calls `withdraw()` – the contract sends the accumulated
   tokens back to the creator

## Project Structure

```
├── Cargo.toml          # Rust / Soroban dependencies
├── src/
│   ├── lib.rs          # Contract logic
│   └── test.rs         # Unit tests
├── .gitignore
└── README.md
```

## License

MIT
