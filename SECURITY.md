# Security Policy

## Supported Versions

Security updates are applied to the default branch and the latest released contract version.

| Version | Supported |
| --- | --- |
| `main` | Yes |
| Latest release | Yes |
| Older releases | No |

## Reporting a Vulnerability

Please report suspected vulnerabilities privately through GitHub Security Advisories for this repository. Do not open a public issue or pull request that includes exploit details, private keys, account data, or reproduction steps that could put users at risk.

When possible, include:

- A clear description of the vulnerability and affected code path.
- Minimal reproduction steps or a proof of concept.
- Potential impact, including affected assets or permissions.
- Suggested remediation, if you already have one.

If GitHub Security Advisories are unavailable, open a GitHub Discussion or issue that asks for a private security contact without including sensitive details.

## Disclosure Timeline

The maintainers aim to:

1. Acknowledge valid reports within 3 business days.
2. Triage severity and affected versions within 7 business days.
3. Prepare and test a fix before public disclosure.
4. Credit reporters in release notes when requested and appropriate.

Public disclosure should wait until a fix is available or the maintainers agree on a coordinated disclosure date.

## Scope

Reports are in scope when they affect the StellarTip contract, deployment scripts, CI, or repository configuration in a way that could compromise contract funds, admin controls, user balances, privacy, or release integrity.

Out-of-scope reports include spam, social engineering, denial-of-service against third-party services, and vulnerabilities that require already-compromised maintainer credentials.

## Threat Model

This section documents a STRIDE threat model for the current contract (`CONTRACT_VERSION = 3`, the `init()` signature that includes `min_tip_amount`). Each mitigation links to the issue(s) that introduced it; protections that predate issue tracking are labelled "initial design (v0.1.0)".

### System Overview

**Assets**

- Creator internal balances (`Balance(creator, token)` persistent entries) and the tokens the contract holds in custody on their behalf.
- The admin role (pause control, fee configuration, caps, admin rotation).
- The profile and username registry (`Profile`, `UsernameToAddress`).
- Tip history (`Tip`, `TipCount` persistent entries) as an auditable record.

**Actors**

- **Supporter** — any wallet calling `tip()`.
- **Creator** — a registered profile owner who can `withdraw()`, `update_profile()`, and `unregister()`.
- **Admin** — the address set at `init()` (or via `set_admin()`); controls pause, fees, and caps.
- **Deployer** — uploads the WASM and is expected to call `init()` immediately.
- **Anonymous caller** — anyone invoking view functions; reads are unauthenticated by design.

**Trust boundaries**

- Wallet authorization: every state-changing function calls `require_auth()` on the acting address; the contract performs no signature verification of its own.
- External calls: token movements go through Stellar Asset Contract (SAC) `token::Client` transfers — the token contracts are outside this contract's trust domain, and the contract assumes only that a transfer of `amount` moves `amount`; it does not verify this and does not reconcile its internal ledger against token balances (see Residual Risks).
- Storage tiers: instance storage (shared config) vs. persistent storage (per-creator data) have independent TTL lifecycles.

### STRIDE Table

| Threat | STRIDE category | Surface | Mitigation | Mitigating issue(s) |
| --- | --- | --- | --- | --- |
| Acting as another wallet (tipper, creator, or admin) | Spoofing | All state-changing functions | `require_auth()` on the acting address in every state-changing function | Initial design (v0.1.0) |
| Claiming an identity already registered | Spoofing | `register()` | Username uniqueness (`UsernameTaken`) and one-profile-per-address (`CreatorAlreadyExists`) checks | Initial design (v0.1.0) |
| Non-admin mutating fees, caps, pause state, or admin key | Tampering | Admin functions | Stored-admin equality check; mismatch panics `NotAuthorized` | [#17](https://github.com/StellarTips/StellarTip-Contract/issues/17), [#18](https://github.com/StellarTips/StellarTip-Contract/issues/18) |
| Fee arithmetic overflow or precision manipulation | Tampering | `tip()` fee calculation | `overflow-checks = true` in the release profile, `i128` arithmetic, multiply-before-divide, and fuzz-tested invariants (10,000 proptest cases) | [#43](https://github.com/StellarTips/StellarTip-Contract/issues/43) |
| Tip proceeds routed against corrupted fee state (fee set, recipient missing) | Tampering | `tip()` | Fail-fast `FeeRecipientNotSet` guard evaluated before any external transfer | [#28](https://github.com/StellarTips/StellarTip-Contract/issues/28) (PR [#46](https://github.com/StellarTips/StellarTip-Contract/pull/46)) |
| Withdrawing more than the accrued balance | Tampering | `withdraw()` | `current_balance >= amount` check before any SAC call; violation panics `InsufficientBalance` | Initial design (v0.1.0) |
| Non-standard token breaking the custody invariant (rebasing, fee-on-transfer, clawback) | Tampering | `tip()` and `withdraw()` SAC calls | **Accepted, not mitigated.** Internal `Balance(creator, token)` bookkeeping is authoritative and is never reconciled against `token.balance()`; divergence is confined to the offending token. Characterised by a rebasing-token fixture in `src/fixtures.rs` | [#85](https://github.com/StellarTips/StellarTip-Contract/issues/85) |
| State changes without an audit trail | Repudiation | All state-changing functions | Every state-changing function emits a typed event (`EVENT_*`); the missing `init()` event was fixed | [#31](https://github.com/StellarTips/StellarTip-Contract/issues/31) (PR [#47](https://github.com/StellarTips/StellarTip-Contract/pull/47)) |
| Disputing that a tip occurred | Repudiation | `tip()` | Tip records are written to persistent storage as permanent on-chain history | Initial design (v0.1.0) |
| Reading contract state (balances, profiles, history) | Information disclosure | All storage | All contract state is public on-chain **by design**; no secrets are stored. Accepted property, not a gap | — |
| Storage bloat via mass creator registration | Denial of service | `register()` | `MaxCreators` cap (`CapExceeded`) | [#36](https://github.com/StellarTips/StellarTip-Contract/issues/36) (PR [#45](https://github.com/StellarTips/StellarTip-Contract/pull/45)) |
| Storage bloat via dust-tip spam | Denial of service | `tip()` | `MaxTipsPerCreator` cap and configurable `MinTipAmount` (`BelowMinimum`) | [#36](https://github.com/StellarTips/StellarTip-Contract/issues/36) (PR [#45](https://github.com/StellarTips/StellarTip-Contract/pull/45)), [#42](https://github.com/StellarTips/StellarTip-Contract/issues/42) (PR [#55](https://github.com/StellarTips/StellarTip-Contract/pull/55)) |
| Instruction exhaustion from linear token-set scans | Denial of service | `tip()`, `withdraw()`, `unregister()` | `CreatorTokens` uses `Map<Address, ()>` with O(log n) insert/remove instead of a `Vec` linear scan | [#38](https://github.com/StellarTips/StellarTip-Contract/issues/38) (PR [#48](https://github.com/StellarTips/StellarTip-Contract/pull/48)) |
| State eviction via TTL expiry (data loss) | Denial of service | Persistent storage | TTL extended on every persistent read/write (15-day threshold, 30-day target), only after all guards pass | [#19](https://github.com/StellarTips/StellarTip-Contract/issues/19) |
| Dirty TTL bumps from read-only calls | Denial of service | `get_tips()` | TTL-extension side effect removed from the view function | [#44](https://github.com/StellarTips/StellarTip-Contract/issues/44) |
| Ongoing exploitation of a discovered vulnerability | Denial of service (containment) | All state-changing functions | Emergency `pause()` / `unpause()` gate checked by every state-changing function | [#18](https://github.com/StellarTips/StellarTip-Contract/issues/18) |
| Regressions introducing new vulnerable patterns | Denial of service (prevention) | CI | Scout static analysis runs in CI with `fail_on_error: true` | [#57](https://github.com/StellarTips/StellarTip-Contract/issues/57) |
| Non-admin invoking admin-only functions | Elevation of privilege | Admin functions | Stored-admin equality check panics `NotAuthorized` | [#17](https://github.com/StellarTips/StellarTip-Contract/issues/17), [#18](https://github.com/StellarTips/StellarTip-Contract/issues/18) |
| Re-initializing the contract to seize the admin role | Elevation of privilege | `init()` | One-time guard on the `Admin` key; second call panics `AlreadyInitialized` | Introduced with `init()` (no tracked issue) |

### Residual and Accepted Risks

- **Init front-running.** The first caller of `init()` after deployment becomes admin. Mitigation is procedural — deploy and initialize atomically (see `scripts/deploy.sh`). No in-contract protection; no tracked issue.
- **Single admin key.** There is no multisig or timelock. A compromised admin key enables pause abuse and fee redirection. Fee redirection is bounded: `fee_bps` is capped at 10,000 (100%) and applies only to **future** tips — existing creator balances can only ever be withdrawn by the creator.
- **Unbounded `limit` in `get_tips()`.** Documented as M-2 in [`docs/static-analysis-findings.md`](docs/static-analysis-findings.md). Impact is limited to instruction-budget exhaustion of the caller's own read; no state risk.
- **`DEFAULT_MIN_TIP_AMOUNT = 1` provides no real dust protection.** It is equivalent to the `amount > 0` guard. Admins must raise it via `set_min_tip_amount` post-deploy for meaningful protection.
- **The custody invariant is implicit and unenforced.** The contract relies on `sum(internal balances for token T) == T.balance(contract)` but never checks it: `tip()` credits the `amount` argument and `withdraw()` debits it, and neither reads `token.balance()`. Any token that moves holder balances outside a transfer breaks the invariant — **rebasing** tokens (stETH, aTokens), **fee-on-transfer** tokens (the contract receives less than `amount` but credits the full `amount`), and tokens with **clawback** enabled. All three are legal SEP-41 tokens.

  Consequences are bounded to the offending token and are published behaviour, not latent bugs. An upward rebase strands surplus in the contract permanently — there is no sweep function. A downward rebase leaves the contract under-collateralised: withdrawals revert *inside the token contract* rather than with a `TipError`, and the last creators to withdraw absorb the shortfall. A wipeout leaves `unregister()` blocked by `BalanceNotEmpty`, since internal credit survives a token balance going to zero.

  Choosing which tokens to accept is the supporter's and the creator's responsibility. The rebasing case is characterised by the fixture in `src/fixtures.rs` and the tests in `src/test.rs`; a fee-on-transfer fixture is worth a follow-up issue.

### Review Cadence

This threat model describes `CONTRACT_VERSION = 3`. Because deployed Soroban WASM is immutable, any change to the contract surface (new functions, storage keys, or an `init()` signature change) ships as a redeployment — and **must** be accompanied by a review of this section in the same pull request. Reviewers should treat a `CONTRACT_VERSION` bump without a threat-model update as a blocking omission.
