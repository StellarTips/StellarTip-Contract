# StellarTip Contracts

Soroban smart contract for a decentralized micro-tipping platform on Stellar.

## Overview

StellarTip allows creators to register profiles and receive instant micro-payments
(tips) from supporters using any Stellar asset (XLM, USDC, etc.). Tips are held in
the contract until creators withdraw them, and every tip is recorded on-chain for
transparency.

## Features

- **Creator Profiles** – register with a unique username, display name, and bio
- **Profile Updates** – creators can update their display name and bio
- **Account Deletion** – creators can unregister when all balances are zero
- **Username-based lookup** – find any creator by their username
- **Multi-token tips** – supporters can tip in any Stellar asset
- **On-chain history** – every tip is permanently recorded with pagination
- **Self-custody withdrawal** – creators withdraw tips at any time
- **Platform Fee** – configurable fee deducted from each tip
- **Emergency Pause** – admin can pause and unpause the contract
- **Events** – all actions emit standard Soroban events for indexing
- **Persistent TTL** – automatic storage TTL extension prevents data loss
- **CI Static Analysis** – Scout (Soroban-compatible) detects vulnerabilities in CI

## Contract Interface

### Admin Functions

| Function | Description |
|----------|-------------|
| `init(admin, fee_recipient, fee_bps)` | Initialize the contract (one-time) |
| `set_admin(caller, new_admin)` | Transfer admin privileges |
| `pause(caller)` | Pause the contract (emergency stop) |
| `unpause(caller)` | Unpause the contract |
| `set_fee_percentage(caller, fee_bps)` | Set platform fee (0–10_000 bps) |
| `set_fee_recipient(caller, address)` | Set the fee recipient address |

### Creator Functions

| Function | Description |
|----------|-------------|
| `register(username, display_name, bio)` | Register as a creator |
| `update_profile(display_name, bio)` | Update your display name and bio |
| `unregister(caller)` | Delete your profile (requires zero balance) |

### Tipping & Withdrawal

| Function | Description |
|----------|-------------|
| `tip(creator, token, amount, message)` | Send a tip to a creator |
| `withdraw(token, amount)` | Withdraw accumulated tips for a token |

### View Functions

| Function | Description |
|----------|-------------|
| `get_profile(address)` | Get a creator's profile |
| `get_creator_from_username(username)` | Resolve a username to an address |
| `get_profile_by_username(username)` | Get profile directly by username |
| `get_balance(creator, token)` | Check a creator's balance for a token |
| `get_tip_count(creator)` | Get total tips received |
| `get_tip(creator, index)` | Get a specific tip record |
| `get_tips(creator, start, limit)` | Paginated tip history |
| `get_all_tokens(creator)` | List all tokens a creator has received |
| `is_creator(address)` | Check if an address is registered |
| `is_username_taken(username)` | Check if a username is claimed |
| `get_contract_version()` | Get the contract version |
| `get_admin()` | Get the current admin address |
| `is_paused()` | Check if the contract is paused |
| `get_fee_percentage()` | Get current platform fee in bps |
| `get_fee_recipient()` | Get the fee recipient address |

## Getting Started

### Prerequisites

- Rust (nightly) – <https://rustup.rs>
- Soroban CLI – `cargo install soroban-cli`
- Stellar CLI – `cargo install stellar-cli`

### Build

```bash
cargo build --release --target wasm32-unknown-unknown
```

### Test

```bash
cargo test
```

### Format & Lint

```bash
make fmt
make lint
make check      # fmt + lint + test + wasm-build
```

### Deploy (testnet)

```bash
make deploy-testnet
# or directly:
./scripts/deploy.sh testnet
```

### Deploy (mainnet)

```bash
make deploy-mainnet
# or directly:
./scripts/deploy.sh mainnet
```

### Makefile Targets

| Target            | Description                        |
|-------------------|------------------------------------|
| `make build`      | Build the contract (release)       |
| `make wasm-build` | Build for WASM target              |
| `make test`       | Run all tests                      |
| `make fmt`        | Format code with rustfmt           |
| `make lint`       | Run clippy lints                   |
| `make scout`      | Run Scout static analysis          |
| `make check`      | CI-style check (fmt + lint + scout + test + build) |
| `make clean`      | Remove build artifacts             |
| `make deploy-testnet` | Deploy to Stellar testnet      |
| `make deploy-mainnet` | Deploy to Stellar mainnet      |

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
3. A platform fee (if configured) is forwarded to the fee recipient
4. The creator's internal balance is updated and the tip is recorded
5. Later, the creator calls `withdraw()` – the contract sends the accumulated
   tokens back to the creator

## Project Structure

```
├── Cargo.toml                  # Rust / Soroban dependencies
├── src/
│   ├── lib.rs                  # Contract logic
│   └── test.rs                 # Unit tests (34 tests)
├── docs/
│   └── static-analysis-findings.md  # Known static analysis findings
├── .github/
│   └── workflows/
│       └── ci.yml              # CI pipeline (fmt, lint, test, build, static analysis)
├── .gitignore
├── scripts/
│   └── deploy.sh               # Deployment script
├── Makefile
└── README.md
```

## Project Status

![CI](https://github.com/StellarTips/StellarTip-Contract/actions/workflows/ci.yml/badge.svg)

Automated CI runs format checks, clippy lints, tests, WASM builds, and **Scout static analysis** on every push and pull request. Deployments are automated via GitHub Actions on version tags.

## License

MIT
