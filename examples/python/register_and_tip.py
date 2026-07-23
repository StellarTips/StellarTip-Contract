"""
StellarTip — Python example (server-side tip orchestration)

Registers a creator profile and sends a tip, end-to-end via Soroban RPC.
This is the same flow as `docs/tutorial.md` but expressed as a runnable
Python script suitable for a backend that needs to call the contract on
behalf of users (e.g. a moderation bot, a scheduled payout, a batch
air-drop job).

Prereqs (from the parent README):

    python3 -m pip install 'stellar-sdk>=9'
    export CONTRACT_ID=... PUBLIC_KEY=... SECRET_KEY=... XLM_TOKEN=...
    export SOROBAN_RPC_URL=https://soroban-testnet.stellar.org
    python3 register_and_tip.py

SDK target: `stellar-sdk >= 9`. (Versions <= 7 have only experimental Soroban
support via `TransactionBuilder.append_invoke_contract_function_op`; v9
moved to the modern `prepare_transaction` flow shown below.)
"""

from __future__ import annotations

import os
import sys
import time
from typing import Optional

from stellar_sdk import (
    Keypair,
    Network,
    SorobanServer,
    TransactionBuilder,
    Contract,
    Address,
    scval,
)
from stellar_sdk.exceptions import NotFoundError


# ---------------------------------------------------------------------------
# 0. Configuration from environment.
# ---------------------------------------------------------------------------

CONTRACT_ID: str = require_env("CONTRACT_ID")
PUBLIC_KEY: str = require_env("PUBLIC_KEY")
SECRET_KEY: str = require_env("SECRET_KEY")
XLM_TOKEN: str = require_env("XLM_TOKEN")
RPC_URL: str = require_env("SOROBAN_RPC_URL")

NETWORK_PASSPHRASE: str = Network.TESTNET_NETWORK_PASSPHRASE

# One XLM = 10_000_000 stroops (the token base unit for native XLM).
TIP_AMOUNT_STROOPS: int = 10_000_000


def main() -> int:
    soroban = SorobanServer(RPC_URL)
    contract = Contract(CONTRACT_ID)
    keypair = Keypair.from_secret(SECRET_KEY)
    actor = keypair.public_key

    # Sanity: the public key we pass as `caller` / `from` must match the
    # signer. Catching it locally avoids opaque `require_auth` failures
    # on-chain.
    if actor != PUBLIC_KEY:
        raise RuntimeError(
            f"SECRET_KEY derives {actor} which does not match "
            f"PUBLIC_KEY {PUBLIC_KEY}. Make sure both come from the same identity."
        )

    # Fetch once and reuse for any view calls (auth is not required for
    # views, so the same account snapshot is fine).
    view_source = soroban.load_account(actor)

    # -----------------------------------------------------------------------
    # 1. `register(caller, username, display_name, bio)`
    #    Errors: CreatorAlreadyExists / UsernameTaken / CapExceeded / InvalidInput.
    # -----------------------------------------------------------------------
    print(f"[1] Registering creator {actor} as username 'alice'...")
    submit_invoke(
        soroban=soroban,
        contract=contract,
        keypair=keypair,
        fn="register",
        parameters=[
            scval.to_address(actor),
            scval.to_symbol("alice"),
            scval.to_string("Alice"),
            scval.to_string("Digital artist"),
        ],
    )
    print("[1] Registration succeeded.")

    # -----------------------------------------------------------------------
    # 2. `get_profile(address)` — read back the profile. View calls use the
    #    same invoke path but the simulation result already carries the
    #    return value, so we don't need to poll for confirmation.
    # -----------------------------------------------------------------------
    print(f"[2] Reading profile for {actor}...")
    profile = call_view(
        soroban=soroban,
        source=view_source,
        contract=contract,
        fn="get_profile",
        parameters=[scval.to_address(actor)],
    )
    print("[2] Profile:", profile)

    # -----------------------------------------------------------------------
    # 3. `tip(from, creator, token, amount, message)` — supporter -> creator.
    #    The contract takes `fee_bps` off the top and credits the rest.
    #    Errors: CreatorNotFound / InvalidAmount / BelowMinimum /
    #            CapExceeded / FeeRecipientNotSet / TransferFailed.
    # -----------------------------------------------------------------------
    print(f"[3] Sending {TIP_AMOUNT_STROOPS} stroops (1 XLM) to creator...")
    tip_index = submit_invoke(
        soroban=soroban,
        contract=contract,
        keypair=keypair,
        fn="tip",
        parameters=[
            scval.to_address(actor),  # from
            scval.to_address(actor),  # creator (self-tip for demo)
            scval.to_address(XLM_TOKEN),
            scval.to_int128(TIP_AMOUNT_STROOPS),
            scval.to_string("Love your art!"),
        ],
        expect_return=True,
    )
    print(f"[3] Tip recorded at index {tip_index}.")

    # -----------------------------------------------------------------------
    # 4. Read back `get_tip_count(creator)` and `get_balance(creator, token)`.
    #    With `fee_bps = 250` the creator credit is 9_750_000 stroops.
    # -----------------------------------------------------------------------
    tip_count = call_view(
        soroban=soroban,
        source=view_source,
        contract=contract,
        fn="get_tip_count",
        parameters=[scval.to_address(actor)],
    )
    balance = call_view(
        soroban=soroban,
        source=view_source,
        contract=contract,
        fn="get_balance",
        parameters=[scval.to_address(actor), scval.to_address(XLM_TOKEN)],
    )
    print(f"[4] On-chain state: tip_count={tip_count}, creator balance={balance}")
    return 0


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def build_invoke_tx(
    source,
    contract: Contract,
    fn: str,
    parameters: list,
    base_fee: int = 100,
    timeout: int = 30,
):
    """Build a Soroban invoke transaction on top of a pre-fetched account.

    `append_invoke_contract_function_op` is the modern (>= 9) helper for
    Soroban contract calls; it wires the `invokeHostFunction` envelope and
    footprint automatically once `prepare_transaction` runs simulation.
    """
    return (
        TransactionBuilder(
            source_account=source,
            network_passphrase=NETWORK_PASSPHRASE,
            base_fee=base_fee,
        )
        .append_invoke_contract_function_op(
            contract=contract,
            function_name=fn,
            parameters=parameters,
        )
        .set_timeout(timeout)
        .build()
    )


def submit_invoke(
    *,
    soroban: SorobanServer,
    contract: Contract,
    keypair: Keypair,
    fn: str,
    parameters: list,
    expect_return: bool = False,
) -> Optional[object]:
    """Simulate, sign, submit, poll, and return the decoded return value.

    `prepare_transaction` is the magic step: it parses the simulation
    result and (a) embeds the Soroban auth entries that satisfy every
    `require_auth` / `require_auth_for_args` call inside the contract, and
    (b) updates resource fees / footprint. Skipping this step gets a
    `TransactionResultCode.sorobanNotSupported` from the network.
    """
    # Refresh the account snapshot per submission; sequence advances
    # with every successful write.
    source = soroban.load_account(keypair.public_key)
    tx = build_invoke_tx(source, contract, fn, parameters)
    prepared = soroban.prepare_transaction(tx)
    prepared.sign(keypair)

    send = soroban.send_transaction(prepared)
    if send.status != "PENDING":
        raise RuntimeError(f"send_transaction rejected for {fn}: {send.status}")

    # Poll until the transaction lands or errors.
    result = None
    for _ in range(30):
        try:
            result = soroban.get_transaction(send.hash)
        except NotFoundError:
            time.sleep(1)
            continue
        if result.status in {"SUCCESS", "FAILED"}:
            break
    if result is None or result.status == "FAILED":
        raise RuntimeError(
            f"transaction {send.hash} did not confirm "
            f"(status={getattr(result, 'status', 'UNKNOWN')})"
        )

    if expect_return:
        # `result.return_value` is an XdrScVal; `scval.to_native` decodes
        # it into the corresponding Python primitive (int for u64/i128,
        # str for Symbol/String, dict for composite types, etc.).
        return scval.to_native(result.return_value)
    return None


def call_view(
    *,
    soroban: SorobanServer,
    source,
    contract: Contract,
    fn: str,
    parameters: list,
) -> object:
    """Simulate a view call and return the native-decoded value.

    View methods do not call `require_auth`, so we don't need signing;
    the simulation result already carries the return value.
    """
    tx = build_invoke_tx(source, contract, fn, parameters)
    sim = soroban.simulate_transaction(tx)
    if "error" in sim and sim["error"]:
        raise RuntimeError(f"simulate_transaction failed for {fn}: {sim}")
    # Symmetric to the write path: `scval.to_native` decodes ScVal to Python.
    return scval.to_native(sim["result"]["retval"])


def require_env(name: str) -> str:
    value = os.environ.get(name)
    if not value:
        raise RuntimeError(
            f"Missing required env var {name}. See examples/README.md "
            f"for the full list."
        )
    return value


if __name__ == "__main__":
    sys.exit(main())


"""
===== EXPECTED OUTPUT ========================================================

Numeric values for `registered_at` are illustrative; everything else is
deterministic.

    [1] Registering creator GABC...USER as username 'alice'...
    [1] Registration succeeded.
    [2] Reading profile for GABC...USER...
    [2] Profile: {'username': 'alice', 'display_name': 'Alice', 'bio': 'Digital artist', 'registered_at': 1718901234}
    [3] Sending 10000000 stroops (1 XLM) to creator...
    [3] Tip recorded at index 0.
    [4] On-chain state: tip_count=1, creator balance=9750000

The creator balance is 9 750 000 stroops, not 10 000 000, because the
contract deducts the 2.5 % platform fee (init-time `fee_bps = 250`):

    fee = (10_000_000 * 250) / 10_000 = 250_000
    creator credit = 10_000_000 - 250_000 = 9_750_000

If `fee_bps = 0` was used at init time, the result would be `10000000`
(zero fee).

============================================================================
"""
