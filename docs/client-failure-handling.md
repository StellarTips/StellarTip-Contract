# Client-Side Failure-Handling Recommendation Spec

> **Scope**: This document is addressed to wallet developers, SDK authors, and
> any off-chain client that submits transactions to the StellarTip contract.
> The contract itself requires **no code changes** — all failure-handling logic
> lives entirely on the client side.

---

## Background

Soroban's transaction model rolls back every storage write *and* every emitted
event when a call ends in `panic_with_error!`. The only observable signal of
failure from the Soroban RPC is:

1. The **error code** embedded in the `TransactionResult` (`resultCode`
   `txFAILED` → `opINVOKE_HOST_FUNCTION_TRAPPED`).
2. The **host error message** returned by `simulateTransaction` or
   `sendTransaction` (a stringified `#N` code such as `#6` for
   `InvalidAmount`).

Because events are rolled back on panic, an indexer **cannot** detect why a
tip failed by watching on-chain events alone. Clients must implement the
pre-flight and recovery flows described here.

---

## TipError Variant Reference

The contract defines **16 `TipError` variants** mapped to Soroban error codes
`#1` – `#16`. They are split into three retry classes:

### Never-retry variants (deterministic, user/state-driven)

Re-submitting the same transaction without first resolving the underlying
condition will always produce the same failure.

| Code | Variant | Triggering condition | Recommended client action |
|------|---------|----------------------|---------------------------|
| `#1` | `CreatorAlreadyExists` | `register()` called for an address that already has a profile | Inform the user they are already registered; skip re-registration |
| `#3` | `UsernameTaken` | `register()` called with a username already claimed by another address | Prompt the user to choose a different username |
| `#6` | `InvalidAmount` | `tip()` called with `amount ≤ 0` | Validate `amount > 0` before submission; surface a UI error |
| `#8` | `NotInitialized` | Any call before `init()` has been executed | Contact the platform operator; the contract has not been set up |
| `#9` | `AlreadyInitialized` | `init()` called a second time | No action needed; contract is already live |
| `#11` | `NotAuthorized` | Caller is not the admin for an admin-gated function | Never expose admin functions to non-admin users |
| `#12` | `InvalidInput` | `display_name` or `bio` exceeds maximum byte length | Enforce length limits in the UI before submission |
| `#13` | `BalanceNotEmpty` | `unregister()` called while the creator still holds a non-zero balance | Direct the user to withdraw all token balances first |
| `#15` | `FeeRecipientNotSet` | A tip is attempted but `fee_bps > 0` and no `FeeRecipient` is configured | Contact the platform operator; this is a contract misconfiguration |

### Retry-with-backoff variants (condition may lift)

These failures reflect transient or correctable state. Clients should wait for
the underlying condition to be resolved before retrying.

| Code | Variant | Triggering condition | Recommended client action |
|------|---------|----------------------|---------------------------|
| `#2` | `CreatorNotFound` | `tip()` or profile lookup for an address that has no registered profile | Verify the recipient address or username; re-fetch profile before retrying |
| `#4` | `InsufficientBalance` | `withdraw()` called for more than the creator's recorded balance | Re-read the balance via `get_balance()` and adjust the amount |
| `#5` | `TransferFailed` | The underlying SAC `transfer()` call failed (insufficient user funds, frozen asset, etc.) | Check user wallet balance; surface a wallet-level error; retry after user tops up |
| `#10` | `Paused` | Contract is in emergency-pause state | Poll `is_paused()` with exponential backoff; notify the user and retry once unpaused |
| `#14` | `CapExceeded` | `MaxCreators` or `MaxTipsPerCreator` cap has been reached | Notify the user; retry only after the admin raises the cap |
| `#16` | `BelowMinimum` | `tip()` called with `amount > 0` but below the configured `MinTipAmount` | Re-quote the amount using `get_min_tip_amount()`; **do not** blindly retry with the same amount |

> **Note on `#16` (`BelowMinimum`)**: This requires **re-quoting**, not a
> blind retry. Fetch the current minimum via `get_min_tip_amount()` and
> present the updated floor to the user before re-submitting.

### Currently unreachable (dead variant per audit)

| Code | Variant | Notes |
|------|---------|-------|
| `#7` | `NoTips` | Defined in the enum but never raised by the current implementation. Treat as an unexpected error if encountered. |

---

## Pre-flight Simulation Pattern

Always simulate a transaction before broadcasting. Soroban's
`simulateTransaction` RPC method runs the contract logic off-chain and returns:

- The **estimated resource fees** (CPU instructions, memory, ledger reads/writes).
- Any **contract error** that would be raised, before any funds are deducted.

### Recommended pre-flight checklist before `tip()`

```typescript
// 1. Resolve the creator address (if using username lookup)
const creatorAddress = await contract.get_creator_from_username({ username });

// 2. Check the contract is live and unpaused
const paused = await contract.is_paused();
if (paused) throw new Error("Contract is paused. Please try again later.");

// 3. Check the minimum tip amount
const minTip = await contract.get_min_tip_amount();
if (amount < minTip) throw new Error(`Tip must be at least ${minTip} base units.`);

// 4. Simulate the transaction — catches most failure modes before submission
const sim = await server.simulateTransaction(tipTransaction);
if (SorobanRpc.Api.isSimulationError(sim)) {
  // Parse the error code from sim.error (e.g. "#6", "#16")
  handleContractError(sim.error);
  return;
}

// 5. Submit only if simulation succeeded
const result = await server.sendTransaction(assembleTransaction(sim));
await pollForConfirmation(result.hash);
```

---

## Transaction Submission and RPC Error Handling

After `sendTransaction`, poll `getTransaction` until the status is `SUCCESS` or
`FAILED`. The canonical Stellar Horizon / Soroban RPC error-code mapping is:

| Soroban RPC status | Meaning | Client action |
|--------------------|---------|---------------|
| `SUCCESS` | Transaction confirmed | Update UI; observe emitted events |
| `FAILED` | Contract panicked or host error | Parse `resultXdr` for `#N` code; apply retry rules above |
| `NOT_FOUND` | Transaction not yet in a ledger | Retry `getTransaction` with exponential backoff (max ~30 s) |
| `PENDING` | In the transaction queue | Continue polling |

### Parsing the error code

The host error is returned in the `resultXdr` as a `contractError` with an
`int32` value equal to the `TipError` discriminant. Most SDKs surface this as
a string like `"Error(Contract, #6)"`. Extract the integer and map it to the
table above.

```typescript
function handleContractError(errorString: string): never {
  const match = errorString.match(/#(\d+)/);
  const code = match ? parseInt(match[1], 10) : -1;

  switch (code) {
    case 2:  throw new CreatorNotFoundError();
    case 6:  throw new InvalidAmountError("Amount must be greater than 0");
    case 10: throw new ContractPausedError("Contract is paused; please try again later");
    case 14: throw new CapExceededError("Creator capacity reached; contact support");
    case 16: throw new BelowMinimumError("Tip is below the minimum; please increase the amount");
    default: throw new UnknownContractError(`Unexpected contract error: ${errorString}`);
  }
}
```

---

## Worked Example: Full Pre-flight → Submit → Error Recovery Flow

```typescript
async function sendTip(
  creatorUsername: string,
  tokenAddress: string,
  amount: bigint,
  message: string
): Promise<string> {

  // --- Step 1: pre-flight checks ---
  const paused = await contract.is_paused();
  if (paused) throw new ContractPausedError();

  const minTip = await contract.get_min_tip_amount();
  if (amount < BigInt(minTip)) {
    throw new BelowMinimumError(`Minimum tip is ${minTip}`);
  }

  const creatorAddress = await contract.get_creator_from_username({
    username: creatorUsername,
  });
  if (!creatorAddress) throw new CreatorNotFoundError(creatorUsername);

  // --- Step 2: build transaction ---
  const tx = await buildTipTransaction({
    creator: creatorAddress,
    token: tokenAddress,
    amount,
    message,
  });

  // --- Step 3: simulate ---
  const sim = await rpc.simulateTransaction(tx);
  if (SorobanRpc.Api.isSimulationError(sim)) {
    handleContractError(sim.error); // throws
  }

  // --- Step 4: sign and submit ---
  const signed = await wallet.sign(assembleTransaction(sim));
  const { hash } = await rpc.sendTransaction(signed);

  // --- Step 5: poll for confirmation (max 30 s, 5-second intervals) ---
  for (let i = 0; i < 6; i++) {
    await sleep(5_000);
    const result = await rpc.getTransaction(hash);
    if (result.status === "SUCCESS") return hash;
    if (result.status === "FAILED") {
      handleContractError(result.resultXdr ?? "unknown"); // throws
    }
  }
  throw new TransactionTimeoutError(hash);
}
```

---

## Retry Rules Summary

| Variant | Retry? | Notes |
|---------|--------|-------|
| `CreatorAlreadyExists` (#1) | Never | Deterministic |
| `CreatorNotFound` (#2) | After resolving | Re-fetch profile |
| `UsernameTaken` (#3) | Never | Choose different username |
| `InsufficientBalance` (#4) | After balance check | Re-read balance |
| `TransferFailed` (#5) | After wallet check | User must top up |
| `InvalidAmount` (#6) | Never | Fix amount in UI |
| `NoTips` (#7) | N/A | Dead code path |
| `NotInitialized` (#8) | Never | Operator issue |
| `AlreadyInitialized` (#9) | Never | Already live |
| `Paused` (#10) | After unpause | Poll `is_paused()` |
| `NotAuthorized` (#11) | Never | Auth issue |
| `InvalidInput` (#12) | Never | Fix input in UI |
| `BalanceNotEmpty` (#13) | Never | Withdraw first |
| `CapExceeded` (#14) | After cap raised | Admin action needed |
| `FeeRecipientNotSet` (#15) | Never | Operator misconfiguration |
| `BelowMinimum` (#16) | After re-quoting | Fetch new minimum first |

---

## Cross-references

- [API Reference](./API_REFERENCE.md) — complete public function signatures
- [Architecture](./ARCHITECTURE.md) — storage layout and state lifecycle
- [Soroban RPC documentation](https://developers.stellar.org/docs/data/rpc) — official RPC spec
- [`src/lib.rs`](../src/lib.rs) — `TipError` enum definition
