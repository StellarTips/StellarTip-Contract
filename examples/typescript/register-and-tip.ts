/**
 * StellarTip — TypeScript example (wallet dApp)
 *
 * Registers a creator profile and then sends a tip from a supporter to that
 * creator, end-to-end via Soroban RPC. Mirrors docs/tutorial.md but from a
 * TypeScript client instead of the `stellar` CLI.
 *
 * Prereqs (from the parent README):
 *
 *   npm install @stellar/stellar-sdk ts-node typescript
 *   export CONTRACT_ID=... PUBLIC_KEY=... SECRET_KEY=... XLM_TOKEN=...
 *   export SOROBAN_RPC_URL=https://soroban-testnet.stellar.org
 *   npx ts-node register-and-tip.ts
 *
 * SDK target: `@stellar/stellar-sdk` >= 11 (unified SDK combining the classic
 * `stellar-sdk` core with Soroban support; the older `soroban-client` package
 * is deprecated).
 */

import {
  Keypair,
  Networks,
  TransactionBuilder,
  SorobanRpc,
  Contract,
  Address,
  nativeToScVal,
  scValToNative,
  BASE_FEE,
} from "@stellar/stellar-sdk";

// ---------------------------------------------------------------------------
// 0. Pull config from the environment so the source is portable.
// ---------------------------------------------------------------------------

const CONTRACT_ID = required("CONTRACT_ID");
const PUBLIC_KEY = required("PUBLIC_KEY");
const SECRET_KEY = required("SECRET_KEY");
const XLM_TOKEN = required("XLM_TOKEN");
const RPC_URL = required("SOROBAN_RPC_URL");

// Stellar testnet passphrase. Swap to Networks.PUBLIC for mainnet.
const NETWORK_PASSPHRASE = Networks.TESTNET;

// One XLM = 10_000_000 stroops (the token base unit for native XLM).
const TIP_AMOUNT_STROOPS = BigInt(10_000_000);

const rpc = new SorobanRpc.Server(RPC_URL, { allowHttp: false });
const contract = new Contract(CONTRACT_ID);
const keypair = Keypair.fromSecret(SECRET_KEY);
const actor = keypair.publicKey();

// Sanity: the public key we will pass as `caller` / `from` must match the
// signer. Catching this early avoids opaque `require_auth` failures on-chain.
if (actor !== PUBLIC_KEY) {
  throw new Error(
    `SECRET_KEY derives ${actor} which does not match PUBLIC_KEY ${PUBLIC_KEY}. ` +
      `Make sure both come from the same Stellar identity.`,
  );
}

main().catch((err) => {
  console.error("example failed:", err);
  process.exit(1);
});

async function main() {
  // -------------------------------------------------------------------------
  // 1. `register(caller, username, display_name, bio)` — registers the public
  //    key as a creator profile on the contract.
  //    Errors: CreatorAlreadyExists / UsernameTaken / CapExceeded / InvalidInput.
  // -------------------------------------------------------------------------
  console.log(`[1] Registering creator "${actor}" as username 'alice'...`);
  await invoke(
    "register",
    [
      new Address(actor).toScVal(),
      nativeToScVal("alice", { type: "symbol" }),
      nativeToScVal("Alice", { type: "string" }),
      nativeToScVal("Digital artist", { type: "string" }),
    ],
  );
  console.log("[1] Registration succeeded.");

  // -------------------------------------------------------------------------
  // 2. `get_profile(address)` — read back the newly created profile.
  // -------------------------------------------------------------------------
  console.log(`[2] Reading profile for ${actor}...`);
  const profile = await read("get_profile", [new Address(actor).toScVal()]);
  console.log("[2] Profile:", JSON.stringify(profile, null, 2));

  // -------------------------------------------------------------------------
  // 3. `tip(from, creator, token, amount, message)` — supporter -> creator.
  //    The contract charges `fee_bps` off the top and credits the rest.
  //    Errors: CreatorNotFound / InvalidAmount / BelowMinimum / CapExceeded /
  //            FeeRecipientNotSet / TransferFailed.
  // -------------------------------------------------------------------------
  console.log(`[3] Sending ${TIP_AMOUNT_STROOPS} stroops (1 XLM) to creator...`);
  const tipIndex = await invoke("tip", [
    new Address(actor).toScVal(), // from
    new Address(actor).toScVal(), // creator (self-tip for demo simplicity)
    new Address(XLM_TOKEN).toScVal(),
    nativeToScVal(TIP_AMOUNT_STROOPS, { type: "i128" }),
    nativeToScVal("Love your art!", { type: "string" }),
  ]);
  console.log(`[3] Tip recorded at index ${tipIndex}.`);

  // -------------------------------------------------------------------------
  // 4. `get_tip_count(creator)` and `get_balance(creator, token)` — sanity.
  //    With init-time `fee_bps = 250` the creator credit is 9_750_000 stroops.
  // -------------------------------------------------------------------------
  const tipCount = await read("get_tip_count", [new Address(actor).toScVal()]);
  const balance = await read("get_balance", [
    new Address(actor).toScVal(),
    new Address(XLM_TOKEN).toScVal(),
  ]);
  console.log(
    `[4] On-chain state: tip_count=${tipCount}, creator balance=${balance}`,
  );
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Pulls the latest account snapshot from the network and returns it plus
 * a freshly-initialized `TransactionBuilder`. We fetch per-call because the
 * sequence number advances with every successful submission.
 */
async function freshBuilder() {
  const account = await rpc.getAccount(actor);
  return new TransactionBuilder(account, {
    fee: BASE_FEE,
    networkPassphrase: NETWORK_PASSPHRASE,
  });
}

/**
 * Read-only helper: rounds tx through `simulateTransaction` (view methods
 * never call `require_auth`, so no auth entries are needed) and returns the
 * native-decoded return value.
 */
async function read(fn: string, params: any[]): Promise<unknown> {
  let tx = (await freshBuilder()).addOperation(contract.call(fn, ...params));
  tx = tx.setTimeout(30).build();
  const sim = await rpc.simulateTransaction(tx);
  if (SorobanRpc.Api.isError(sim)) {
    throw new Error(`simulateTransaction failed for ${fn}: ${sim.error}`);
  }
  return scValToNative(sim.result.retval);
}

/**
 * Write helper: builds an invoke tx, simulates it (which attaches the
 * correct Soroban auth entries that satisfy `require_auth` calls inside the
 * contract), signs, submits, polls for confirmation, and returns the
 * decoded return value.
 */
async function invoke(fn: string, params: any[]): Promise<unknown> {
  let tx = (await freshBuilder()).addOperation(contract.call(fn, ...params));
  tx = tx.setTimeout(30).build();

  const sim = await rpc.simulateTransaction(tx);
  if (SorobanRpc.Api.isError(sim)) {
    throw new Error(`simulateTransaction failed for ${fn}: ${sim.error}`);
  }
  // assembleTransaction parses the simulation result and:
  //   - pays the fee in the native asset at the right amount (resources),
  //   - embeds the Soroban transaction data (footprint + auth).
  // Without this step the network will reject the transaction with a
  // `sorobanNotSupported` error.
  const prepared = SorobanRpc.assembleTransaction(tx, sim).build();
  prepared.sign(keypair);
  const send = await rpc.sendTransaction(prepared);
  if (send.status === "ERROR") {
    throw new Error(`sendTransaction returned error status for ${fn}`);
  }

  // Poll for confirmation so the caller sees the final return value.
  const hash = send.hash;
  let result: SorobanRpc.GetTransactionResponse | undefined;
  for (let i = 0; i < 30; i++) {
    result = await rpc.getTransaction(hash);
    if (result.status !== "NOT_FOUND") break;
    await new Promise((r) => setTimeout(r, 1000));
  }
  if (!result || result.status === "NOT_FOUND") {
    throw new Error(`Transaction ${hash} not found after polling.`);
  }
  if (result.status !== "SUCCESS") {
    throw new Error(`Transaction ${hash} failed: ${result.status}`);
  }
  return scValToNative(result.returnValue);
}

function required(name: string): string {
  const value = process.env[name];
  if (!value) {
    throw new Error(
      `Missing required env var ${name}. See examples/README.md for the full list.`,
    );
  }
  return value;
}

/*
 * ===== EXPECTED OUTPUT =====================================================
 *
 * Step values shown are illustrative — exact ledger values come from the
 * network at run time.
 *
 *   [1] Registering creator "GABC...USER" as username 'alice'...
 *   [1] Registration succeeded.
 *   [2] Reading profile for GABC...USER...
 *   [2] Profile: {
 *     "username": "alice",
 *     "display_name": "Alice",
 *     "bio": "Digital artist",
 *     "registered_at": 1718901234
 *   }
 *   [3] Sending 10000000 stroops (1 XLM) to creator...
 *   [3] Tip recorded at index 0.
 *   [4] On-chain state: tip_count=1, creator balance=9750000
 *
 * The creator balance is 9 750 000 stroops, not 10 000 000, because the
 * contract deducts the 2.5 % platform fee (init-time `fee_bps = 250`):
 *
 *   fee = (10_000_000 * 250) / 10_000 = 250_000
 *   creator credit = 10_000_000 - 250_000 = 9_750_000
 *
 * If `fee_bps = 0` was used at init time, the result would be `10000000`
 * (zero fee).
 *
 * ============================================================================
 */
