# Soroban Protocol Compatibility

> **Audience:** operators, integrators, and auditors verifying that the
> compiled WASM does not silently depend on host functions introduced in
> newer protocol versions.
>
> **Companion documents:**
> - [API Reference](API_REFERENCE.md) — public method surface.
> - [Architecture](ARCHITECTURE.md) — storage layout, invariants, and
>   cross-cutting guarantees.
> - [Administrator Runbook](ADMIN_RUNBOOK.md) — operator response procedures.

---

## 1. Supported Protocol Versions

| Protocol | Minimum SDK     | Current Pinned SDK | Status    |
|----------|-----------------|--------------------|-----------|
| 21       | `soroban-sdk = "=21.7.0"` | `soroban-sdk = "=21.7.7"` | **Supported & CI-enforced** |

> **Note on protocol 22:** Contract tests run with `protocol_version: 22` in
> the Soroban test environment, confirming forward compatibility. The
> **compile-time** compatibility matrix, however, targets protocol 21 because
> the pinned SDK (`21.7.7`) is the 21.x line. Protocol 21 is currently the
> most widely deployed protocol version on Stellar mainnet and testnet.

---

## 2. CI Enforcement

The CI pipeline (`sdk-matrix` job in [`.github/workflows/ci.yml`](../.github/workflows/ci.yml))
validates that the contract **compiles** without changes against **both**:

1. **Minimum supported SDK (`=21.7.0`)** — proves no host functions
   introduced in protocol 21 patch releases 21.7.1+ have leaked into the
   contract source.
2. **Current pinned SDK (`=21.7.7`)** — the exact version used in
   production builds, defined in the root [`Cargo.toml`](../Cargo.toml).

This means: if `cargo check --target wasm32-unknown-unknown` passes for
`=21.7.0`, deploying the WASM to a network running protocol 21 **will not**
fail due to a missing host function import. The subset of host functions
verified is narrower than it would be with the original 21.0.0 baseline,
but 21.0.0 was yanked from crates.io and earlier 21.x releases do not
compile on current nightly Rust without a dependency workaround.

### Why not run tests across the matrix?

Soroban SDK test utilities (`testutils` feature) are tightly coupled to the
`soroban-env-host` version and can encounter dependency compatibility issues
(such as `ed25519-dalek` trait reshuffles) when the SDK version is swapped.
Compile-time validation via `cargo check` sidesteps these blockers while
still catching all host-function footguns, because Soroban host functions are
imported at compile time and missing imports produce link-time errors.

The full test suite remains pinned to `=21.7.7` and runs in the `test` CI
job.

---

## 3. Upgrading the Pinned SDK

When upgrading the pinned `soroban-sdk` version in `Cargo.toml`:

1. Update both `[dependencies]` and `[dev-dependencies]` entries to the new
   version.
2. Update the `sdk-matrix` in [`ci.yml`](../.github/workflows/ci.yml):
   - Set the "current" matrix entry to the new pinned version.
   - Update the two `sed` patterns in the `Swap SDK version` step to
     match the new version (otherwise the "current" matrix leg silently
     keeps testing the old version).
   - Update the "minimum" entry to the earliest stable release in the
     **same major line** (e.g., if upgrading to `22.3.1`, set minimum to
     `=22.0.0`).
3. Update the [Protocol Compatibility Matrix](#1-supported-protocol-versions)
   table above to reflect the new status.
4. Run `cargo check --target wasm32-unknown-unknown` locally after swapping
   to the minimum version to catch any issues before pushing.
5. Review the [Soroban protocol release notes](https://soroban.stellar.org/docs/reference/releases)
   for new host functions and verify the contract source does not depend on
   them unless the minimum protocol is also bumped.

---

## 4. Cross-Reference: Issue #84

The protocol compatibility matrix tracked in this document was designed
alongside the **Soroban protocol compatibility matrix** (issue
[#84](https://github.com/StellarTips/StellarTip-Contract/issues/84)). That
issue tracks the broader question of which Stellar networks (futurenet,
testnet, mainnet) run which protocol versions and how the StellarTip
contract maps to them.

- **Issue #84** — tracks the network-level protocol version mapping.
- **Issue #103** — implements the CI matrix that enforces SDK-level
  compatibility on every push and pull request.

Together, #84 and #103 ensure that no deployment will silently fail because
of a mismatch between the compiled WASM and the target network's protocol
version.

---

## 5. Contract Version vs. Protocol Version

The `CONTRACT_VERSION` constant (`src/lib.rs`, currently `3`) is an
application-level version number for the StellarTip contract's public API
shape. It is **independent** of the Soroban protocol version.

- **`CONTRACT_VERSION`** — bumped on backwards-incompatible WASM shape
  changes (new `init()` parameters, storage key layout changes, etc.).
- **Protocol version** — determined by the Stellar network; the SDK
  compiles against a specific protocol's host-function surface.

See [`API_REFERENCE.md`](API_REFERENCE.md) for the current `CONTRACT_VERSION`
and the full public method surface.
