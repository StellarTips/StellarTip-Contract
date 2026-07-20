# Front-Running Protection Analysis

> **Status:** RFC — Design analysis for Issue #120
> **Scope:** `register` function and admin setter functions
> **Date:** 2026-07-20

## Problem Statement

On Stellar, Soroban contract invocations are submitted as plaintext
transactions to the RPC submission pool. A bot monitoring the pool can
observe a pending transaction pre-confirmation and submit a competing
transaction with higher fees or a better position to pre-empt the
original.

### Attack Surface

| Function | Front-Run Vector | Impact |
|----------|-----------------|--------|
| `register(caller, username, …)` | Attacker observes `username` and submits the same `username` first | Legitimate user's tx reverts with `UsernameTaken`; attacker squats the name |
| `set_admin(…, new_admin)` | Attacker front-runs a handover | Could cause confusion but requires caller auth; limited practical risk |
| `set_fee_percentage(…, fee_bps)` | Attacker front-runs a fee decrease | If a tip tx is in-flight, attacker can cause it to be processed under old fee |
| `pause` / `unpause` | Attacker front-runs an unpause | Minimal — admin can re-issue |

The highest-severity vector is `register` because the username is a
scarce, user-chosen resource with an off-chain brand value.

---

## Soroban Constraints

The following Stellar / Soroban properties bound any design choice:

1. **No mempool encryption.** Soroban RPC exposes full transaction
   arguments pre-confirmation. There is no built-in encryption or
   trusted execution.
2. **Fee model is CPU-resource priced.** Transaction fees on Stellar
   are based on resource consumption (instructions, memory, ledger
   entries), not a competitive gas auction. A front-runner cannot
   simply out-bid the victim by paying more.
3. **Sequence numbers are account-level.** Each Stellar account has a
   sequence number. Transactions from the same account must be
   submitted in order. However, different accounts have independent
   sequence numbers, so an attacker with a different account cannot be
   blocked via sequence-number gating.
4. **`require_auth()` is mandatory.** All mutating functions already
   call `caller.require_auth()`. This prevents third-party griefing but
   does **not** prevent front-running because the attacker calls from
   their own account.

---

## Option Analysis

### Option A: No Protection (Off-Chain Reservation)

**Description:** Rely on the application layer: the front-end reserves
a username off-chain before the user submits the on-chain transaction.
The contract is unchanged.

**Pros:**
- Zero contract changes; no engineering cost.
- No added friction or gas overhead for legitimate users.
- Off-chain reservation logic can be iterated independently.

**Cons:**
- No on-chain enforcement. If the reservation backend is compromised
  or the user bypasses it, the race remains.
- Users who interact directly with the RPC (e.g. via CLI or custom
  front-end) have no protection.
- Does not address the integrity concern described in the issue.

**Soroban Fit:** Works today but punts on the problem.

---

### Option B: Commit-Reveal for Registration

**Description:** Two-phase registration:
1. `commit(username_hash: Bytes32, salt: Bytes32)` — stores a hash
   on-chain. Only the caller knows the pre-image.
2. `register(username: Symbol, salt: Bytes32)` — reveals the username
   and salt; contract verifies `hash(username, salt)` matches a stored
   commitment before proceeding.

The commit and reveal are separate transactions. By the time the
username is visible, the commitment is already on-ledger and the
username cannot be stolen because the attacker does not know the salt.

**Pros:**
- Strong protection: a front-runner cannot forge a valid commitment.
- Well-understood cryptographic primitive.
- No off-chain infrastructure required.

**Cons:**
- Two transactions instead of one: worse UX and ~2x the fee cost.
- The committer must remember their salt (or store it off-chain).
- Commitment storage accumulates — requires a cleanup mechanism for
  abandoned commitments (e.g. expiry window).
- Does not apply to admin setters (different threat model).

**Soroban Fit:** Feasible. Store commitments in persistent storage with
a TTL. A commit expiry (e.g. 7 days) prevents abandoned-commitment
bloat. The `Bytes32` type maps to `BytesN<32>` in Soroban. Estimated
~2-3 KiB of new contract code.

---

### Option C: Submission-Fee-Based Ordering

**Description:** Admin configures a `commitment_fee` (in XLM or a
specified token). A `register` call must include a non-refundable fee
to the fee recipient. The fee disincentivises mass name-squatting and
makes front-running economically unattractive.

**Pros:**
- Simple to implement: add a fee parameter to `register`.
- Economically aligns with the platform fee model the contract already
  has.
- One transaction, no extra UX complexity.

**Cons:**
- Does **not** prevent front-running by a motivated attacker who is
  willing to pay the fee.
- A fixed fee may be too low to deter bots but too high for legitimate
  users.
- Does not apply to admin setters.
- Requires a token transfer in the registration flow, adding complexity
  and a new error path.

**Soroban Fit:** Feasible but weak. Since Stellar fees are
CPU-resource-based rather than a gas auction, the economic
disincentive is limited to a flat surcharge. A determined bot can
still observe and beat the user.

---

### Option D: Stellar Sequence-Number Gating Per Username

**Description:** Before registering, a user must first establish a
sequence-number lock by creating an account or using a sub-account
whose sequence number gates the registration. Not practical for a
generic contract.

**Verdict:** Eliminated. Stellar's account model does not support
per-resource sequence numbers, and burning an account per registration
is excessive.

---

## Recommendation

**Go: Option B (Commit-Reveal) for `register`, and accept the status
quo for admin setters.**

### Reasoning

1. `register` is the only function where front-running causes
   irreversible harm (loss of a preferred username). Admin setter
   front-running is a nuisance at worst — the legitimate admin
   re-issues the transaction and the front-runner's state change is
   also immediately reversible by the admin.

2. Commit-reveal is the only option that provides cryptographic
   protection against front-running on Stellar today. Without mempool
   encryption, economic disincentives (Option C) are insufficient
   against a determined bot.

3. The UX cost (two transactions) is acceptable for registration, which
   is a one-time action per creator. Commitments can have a 7-day TTL,
   after which they are automatically garbage-collected by Soroban's
   storage rent mechanism.

4. Admin setter front-running is a low-severity, low-likelihood attack:
   - All admin functions require `caller.require_auth()`, so only the
     current admin can call them.
   - A front-runner cannot change admin parameters; they could only
     cause their own (harmless) transaction to execute between the
     admin's submission and confirmation.
   - The admin simply re-issues the intended transaction.

### Recommended Contract Changes

If the decision is to implement, the following changes would be needed:

1. **New data types:**
   - `Commitment(address, BytesN<32>)` stored keyed by `(caller,
     hash)`.
   - A `commit_expiry` constant (e.g. `604_800` seconds = 7 days).

2. **New functions:**
   - `commit(env, caller: Address, hash: BytesN<32>)` — stores
     `Commitment { caller, hash }` with a timestamp.
   - `reveal_and_register(env, caller: Address, username: Symbol,
     salt: BytesN<32>, display_name, bio)` — verifies the hash match,
     clears the commitment, and proceeds with registration.

3. **Modified functions:**
   - `register` can remain as a direct path for non-sensitive
     registrations, or be removed in favour of the two-phase flow.

4. **No changes** to admin setters (`set_admin`, `set_fee_percentage`,
   `set_fee_recipient`, `set_max_creators`, `set_max_tips_per_creator`,
   `set_min_tip_amount`, `pause`, `unpause`).

### Follow-Up Issues

If the commit-reveal approach is adopted, the following issues should
be filed:

1. **Implementation: Commit function**
   - Add `BytesN<32>` type and `DataKey::Commitment` storage variant.
   - Implement `commit()` with TTL-based expiry.

2. **Implementation: Reveal-and-register function**
   - Implement `reveal_and_register()`.
   - Add validation for hash + salt match and commitment expiry.

3. **Gas cost audit**
   - Measure the additional resource cost of the two-phase flow vs.
     direct `register`.

4. **Backward compatibility**
   - Decide whether to keep the direct `register` path for
     non-sensitive names or remove it entirely.

---

## Decision Record

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-07-20 | **Adopt Option B** for `register`; no change for admin setters | See reasoning above |

*This section is updated when the RFC is accepted and implementation
begins.*
