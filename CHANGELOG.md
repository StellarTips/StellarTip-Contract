# Changelog

All notable changes to the StellarTip contract will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- `init(admin, fee_recipient, fee_bps)` – one-time contract initialization
- `update_profile()` – allows creators to update their display name and bio
- `unregister()` – allows creators to delete their profile when balances are zero
- `get_tips(start, limit)` – paginated function for fetching tip history in ranges
- `get_profile_by_username()` – convenience view function to resolve username → full profile
- `get_all_tokens()` – list all tokens a creator has received tips in
- `get_contract_version()` – returns `CONTRACT_VERSION` for client compatibility
- `get_admin()`, `is_paused()`, `get_fee_percentage()`, `get_fee_recipient()` – admin view functions
- `set_admin()`, `pause()`, `unpause()`, `set_fee_percentage()`, `set_fee_recipient()` – admin controls
- Input validation: display_name max 64 chars, bio max 256 chars
- Contract version constant (`CONTRACT_VERSION = 1`)
- Platform fee mechanism: configurable basis-points fee deducted from each tip
- Emergency pause mechanism: admin can pause and unpause all state-changing functions
- Creator verification in `withdraw()` to prevent non-creators from withdrawing
- Persistent storage TTL extension for all long-lived data entries
- `TipCount` and tip records moved to persistent storage for durability
- `CreatorTokens` tracking to support multi-token balance checking on unregister
- CI: format check, clippy lints, and WASM optimization step
- Dependabot configuration for automated dependency updates
- Issue templates (bug report and feature request)
- Pull request template
- CODEOWNERS file
- `.editorconfig` for consistent editor settings
- `.rustfmt.toml` for consistent code formatting
- `rust-toolchain.toml` to pin the Rust toolchain
- `CONTRIBUTING.md` with development workflow
- `LICENSE` (MIT)

### Changed
- `register()` now requires the contract to be initialized and not paused
- `tip()` now deducts a platform fee (if configured) and forwards it to the fee recipient
- `withdraw()` now verifies the caller is a registered creator before processing
- `tip()` and `withdraw()` now extend the TTL of persistent storage entries they touch
- CI workflow now builds for `wasm32-unknown-unknown` target
- Deploy workflow includes WASM optimization via `soroban contract optimize`
- Deploy workflow correctly handles pre-release versions
- `Makefile` expanded with `fmt`, `lint`, `check`, and `wasm-build` targets
- `deploy.sh` improved with better validation and deploy identity support
- `README.md` updated with new API documentation and architecture details

### Fixed
- Issue #19: Persistent Storage TTL Not Extended — Risk of Data Loss
  - All persistent storage reads and writes now call `extend_ttl`
  - `TipCount` moved from instance to persistent storage for durability
- Issue #18: Pause / Emergency Stop Mechanism
  - Added `pause()`, `unpause()`, and `is_paused()`
  - All state-changing functions check `Paused` flag before executing
- Issue #17: Platform Fee Mechanism
  - Added configurable fee basis points and fee recipient
  - Fee is deducted from each tip and sent to the recipient immediately
- Issue #20: Profile Updates & Account Deletion
  - Added `update_profile()` for in-place profile edits
  - Added `unregister()` that requires zero balance across all tokens
- CI build step previously did not target wasm32, causing WASM artifact verification to fail
- Whitespace and formatting in `lib.rs` cleaned up
- GitHub release concurrency group added to prevent race conditions on deploy

## [0.1.0] - 2025-01-15

### Added
- Initial release of StellarTip smart contract
- Creator registration with unique usernames
- Multi-token tipping with on-chain history
- Self-custody withdrawal for creators
- View functions for profiles, balances, and tip history
- Soroban event emission for all state changes
- Unit test suite with 12 tests
- CI pipeline with test and WASM verification
- Deploy pipeline via GitHub Actions
