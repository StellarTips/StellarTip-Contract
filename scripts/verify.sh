#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# StellarTip Contract — Post-Deploy Verification Script
#
# Usage:
#   ./scripts/verify.sh <contract_id> [expected_version]
#
# The script probes the deployed contract by calling get_contract_version()
# and asserts that the response matches the expected version.  If the
# response does not match, or if the call fails, the script exits non-zero
# so it can be used as a CI gate after deployment.
#
# Arguments:
#   contract_id       – The deployed Soroban contract address (required)
#   expected_version  – Expected integer version string (default: 3)
#
# Environment variables:
#   NETWORK    – Stellar network to probe (default: testnet)
#   IDENTITY   – Stellar CLI identity (default: default)
#
# Examples:
#   ./scripts/verify.sh CABC123...
#   ./scripts/verify.sh CABC123... 3
#   NETWORK=mainnet ./scripts/verify.sh CABC123... 3
# ---------------------------------------------------------------------------

set -euo pipefail

# ---------------------------------------------------------------------------
# Arguments
# ---------------------------------------------------------------------------

if [[ $# -lt 1 ]]; then
  echo "Usage: $0 <contract_id> [expected_version]" >&2
  exit 1
fi

CONTRACT_ID="$1"
EXPECTED_VERSION="${2:-3}"
NETWORK="${NETWORK:-testnet}"
IDENTITY="${IDENTITY:-default}"

# ---------------------------------------------------------------------------
# Preflight
# ---------------------------------------------------------------------------

if ! command -v stellar &>/dev/null; then
  echo "Error: 'stellar' CLI not found." >&2
  echo "Install it with: cargo install stellar-cli --locked" >&2
  exit 1
fi

echo "=== StellarTip Post-Deploy Verification ==="
echo "Network           : $NETWORK"
echo "Contract ID       : $CONTRACT_ID"
echo "Expected version  : $EXPECTED_VERSION"
echo ""

# ---------------------------------------------------------------------------
# Probe: get_contract_version
# ---------------------------------------------------------------------------

echo "Calling get_contract_version() ..."

ACTUAL_VERSION="$(
  stellar contract invoke \
    --id "$CONTRACT_ID" \
    --network "$NETWORK" \
    --source "$IDENTITY" \
    -- \
    get_contract_version \
    2>&1
)"

# Strip surrounding quotes that the CLI sometimes adds (e.g. "3")
ACTUAL_VERSION="${ACTUAL_VERSION//\"/}"
ACTUAL_VERSION="${ACTUAL_VERSION//[[:space:]]/}"

echo "Returned version  : $ACTUAL_VERSION"
echo ""

# ---------------------------------------------------------------------------
# Assert
# ---------------------------------------------------------------------------

if [[ "$ACTUAL_VERSION" == "$EXPECTED_VERSION" ]]; then
  echo "✅  Version check PASSED ($ACTUAL_VERSION == $EXPECTED_VERSION)"
  exit 0
else
  echo "❌  Version check FAILED" >&2
  echo "    Expected : $EXPECTED_VERSION" >&2
  echo "    Got      : $ACTUAL_VERSION" >&2
  exit 1
fi
