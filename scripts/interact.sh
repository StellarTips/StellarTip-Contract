#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# StellarTip Contract — Interaction Driver (testnet)
#
# Usage:
#   ./scripts/interact.sh <function_name> [arg1 arg2 ...]
#
# The script reads the deployed contract ID from the CONTRACT_ID environment
# variable or from .contract-id (written by scripts/deploy.sh). It then
# invokes the given function on the StellarTip contract on Stellar testnet
# using the Stellar CLI and prints the result in human-readable form.
#
# Examples:
#   ./scripts/interact.sh get_contract_version
#   ./scripts/interact.sh is_paused
#   ./scripts/interact.sh get_creator_count
#   ./scripts/interact.sh get_profile --address GABC123...
#   ./scripts/interact.sh tip \
#       --creator GABC123... \
#       --token CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC \
#       --amount 1000000 \
#       --message "great content!"
#
# Environment variables:
#   CONTRACT_ID   – Soroban contract address (overrides .contract-id file)
#   NETWORK       – Stellar network to use (default: testnet)
#   IDENTITY      – Stellar CLI identity to sign with (default: default)
# ---------------------------------------------------------------------------

set -euo pipefail

# ---------------------------------------------------------------------------
# Resolve configuration
# ---------------------------------------------------------------------------

NETWORK="${NETWORK:-testnet}"
IDENTITY="${IDENTITY:-default}"
CONTRACT_ID_FILE=".contract-id"

# Require at least one argument (the function name)
if [[ $# -lt 1 ]]; then
  echo "Usage: $0 <function_name> [arg1 arg2 ...]" >&2
  echo "       CONTRACT_ID=<id> $0 <function_name> [arg1 arg2 ...]" >&2
  exit 1
fi

FUNCTION_NAME="$1"
shift  # remaining args are passed through to the CLI

# ---------------------------------------------------------------------------
# Preflight: Stellar CLI
# ---------------------------------------------------------------------------

if ! command -v stellar &>/dev/null; then
  echo "Error: 'stellar' CLI not found." >&2
  echo "Install it with: cargo install stellar-cli --locked" >&2
  exit 1
fi

# ---------------------------------------------------------------------------
# Resolve contract ID
# ---------------------------------------------------------------------------

if [[ -z "${CONTRACT_ID:-}" ]]; then
  if [[ -f "$CONTRACT_ID_FILE" ]]; then
    CONTRACT_ID="$(cat "$CONTRACT_ID_FILE")"
    CONTRACT_ID="${CONTRACT_ID//[[:space:]]/}"  # strip whitespace
  else
    echo "Error: CONTRACT_ID is not set and '$CONTRACT_ID_FILE' does not exist." >&2
    echo "Deploy the contract first ('make deploy-testnet') or set CONTRACT_ID." >&2
    exit 1
  fi
fi

if [[ -z "$CONTRACT_ID" ]]; then
  echo "Error: CONTRACT_ID is empty." >&2
  exit 1
fi

# ---------------------------------------------------------------------------
# Invoke the function
# ---------------------------------------------------------------------------

echo "Network  : $NETWORK"
echo "Contract : $CONTRACT_ID"
echo "Function : $FUNCTION_NAME"
[[ $# -gt 0 ]] && echo "Args     : $*"
echo "---"

stellar contract invoke \
  --id "$CONTRACT_ID" \
  --network "$NETWORK" \
  --source "$IDENTITY" \
  -- \
  "$FUNCTION_NAME" \
  "$@"

echo ""
echo "Done."
