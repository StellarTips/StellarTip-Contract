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
| `init(admin, fee_recipient, fee_bps, max_creators, max_tips_per_creator)` | Initialize the contract (one-time). `max_creators` and `max_tips_per_creator` are optional caps — pass `0` for unlimited. |
| `set_admin(caller, new_admin)` | Transfer admin privileges |
| `pause(caller)` | Pause the contract (emergency stop) |
| `unpause(caller)` | Unpause the contract |
| `set_fee_percentage(caller, fee_bps)` | Set platform fee (0–10_000 bps) |
| `set_fee_recipient(caller, address)` | Set the fee recipient address |
| `set_max_creators(caller, max_creators)` | Update the creator cap (`0` = unlimited) |
| `set_max_tips_per_creator(caller, max_tips)` | Update the per-creator tip-history cap (`0` = unlimited) |

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
| `get_max_creators()` | Get the configured creator cap (`0` = unlimited) |
| `get_max_tips_per_creator()` | Get the configured per-creator tip cap (`0` = unlimited) |
| `get_creator_count()` | Get the current number of registered creators |

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

## Storage Caps (v2)

To protect the contract against storage bloat and DoS-style \"dust\" attacks,
admin-configurable caps are enforced on every `register()` and `tip()` call:

| Constant | Default | Description |
|----------|---------|-------------|
| `DEFAULT_MAX_CREATORS` | `10_000` | Maximum number of registered creators |
| `DEFAULT_MAX_TIPS_PER_CREATOR` | `10_000` | Maximum tip-history length per creator |

- Pass `0` for either cap (in `init()` or the corresponding setter) to
  disable the cap entirely ("unlimited").
- When `MaxCreators` or `MaxTipsPerCreator` is reached, new calls fail with
  `TipError::CapExceeded` (`#14`). Existing creators and tip history are
  never retroactively evicted — lowering a cap only blocks future activity
  until the admin raises it again (or `unregister()` frees a creator slot).
- `CreatorCount` is tracked alongside `MaxCreators` so enforcement is O(1).

`get_max_creators()`, `get_max_tips_per_creator()`, and `get_creator_count()`
expose the current configuration.

> **Migration note (v1 → v2):** `init()`'s signature was extended with
> `max_creators` and `max_tips_per_creator`. Soroban contracts are
> non-upgradable, so existing v1 deployments must redeploy using the v2
> WASM. Once redeployed the previous profile/balance data is no longer
> reachable through the v2 contract entrypoint; in practice this is fine
> because v1 never shipped to mainnet.

## Project Status

![CI](https://github.com/StellarTips/StellarTip-Contract/actions/workflows/ci.yml/badge.svg)

Automated CI runs format checks, clippy lints, tests, WASM builds, and **Scout static analysis** on every push and pull request. Deployments are automated via GitHub Actions on version tags.

## License

MIT
