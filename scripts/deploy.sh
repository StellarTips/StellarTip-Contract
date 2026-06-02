#!/usr/bin/env bash
set -euo pipefail

# ---------------------------------------------------------------------------
# StellarTip Contract Deploy Script
#
# Usage:
#   ./scripts/deploy.sh [network] [identity]
#
# Arguments:
#   network   – Stellar network to deploy to (testnet | mainnet). Default: testnet
#   identity  – Stellar CLI identity to use for signing.     Default: default
#
# Examples:
#   ./scripts/deploy.sh                          # deploy to testnet with default identity
#   ./scripts/deploy.sh mainnet my-key           # deploy to mainnet with "my-key" identity
# ---------------------------------------------------------------------------

NETWORK="${1:-testnet}"
IDENTITY="${2:-default}"
WASM="target/wasm32-unknown-unknown/release/stellar_tip.wasm"

# ---------- Preflight checks ----------

if ! command -v stellar &> /dev/null; then
  echo "Error: 'stellar' CLI is not installed."
  echo "Install it with: cargo install stellar-cli"
  exit 1
fi

if [ ! -f "$WASM" ]; then
  echo "Error: WASM artifact not found at $WASM"
  echo "Build the contract first: cargo build --release"
  exit 1
fi

# ---------- Deploy ----------

echo "Deploying StellarTip contract to '$NETWORK' using identity '$IDENTITY'..."
echo "WASM: $WASM ($(ls -lh "$WASM" | awk '{print $5}'))"
echo ""

stellar contract deploy \
  --wasm "$WASM" \
  --network "$NETWORK" \
  --source "$IDENTITY"

echo ""
echo "Deploy complete. Copy the contract ID printed above for use in your application."
