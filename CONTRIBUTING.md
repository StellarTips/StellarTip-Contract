# Contributing to StellarTip

Thank you for your interest in contributing! We welcome all contributions.

## Getting Started

1. **Fork and clone** the repository
2. **Install prerequisites:**
   - Rust nightly with wasm32 target: `rustup toolchain install nightly --target wasm32-unknown-unknown`
   - The `rust-toolchain.toml` file will set this up automatically
3. **Build:** `cargo build --release --target wasm32-unknown-unknown`
4. **Test:** `cargo test`

## Development Workflow

```bash
# Build for WASM
make build

# Run tests
make test

# Format code
make fmt

# Run lints (clippy)
make lint

# Build + test + lint (CI check)
make check
```

## Pull Request Process

1. Create a feature branch from `main`
2. Make your changes, including tests if applicable
3. Run `make check` to ensure everything passes
4. Update the README if the public API has changed
5. Submit a PR with a clear description following the PR template

## Code Style

- We use `rustfmt` with the `.rustfmt.toml` config in the repo root
- Run `cargo fmt` before committing
- We use `cargo clippy` with `-D warnings`
- Follow existing patterns in the codebase

## Commit Convention

We follow [Conventional Commits](https://www.conventionalcommits.org/):

- `feat:` for new features
- `fix:` for bug fixes
- `chore:` for maintenance tasks
- `docs:` for documentation changes
- `test:` for test changes
- `refactor:` for code restructuring

## Testing

- All new features must include tests
- Bug fixes should include a regression test
- Run `cargo test` to verify all tests pass

## Contract Guidelines

- The contract uses `#![no_std]` — do not import from `std`
- All state changes must emit Soroban events
- Use `panic_with_error!` with typed errors rather than `panic!`
- Prefer `persistent` storage for long-lived data; use `instance` for counters and indexes

## Questions?

Open a [GitHub Discussion](https://github.com/StellarTips/StellarTip-Contract/discussions) or ask in an issue.
