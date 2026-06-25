#!/usr/bin/env bash
set -euo pipefail

# ---------------------------------------------------------------------------
# Capture a ledger snapshot from a local Soroban testnet for use in fork
# tests.
#
# Prerequisites:
#   - stellar CLI  (cargo install stellar-cli)
#   - A running local testnet (see scripts/start-testnet.sh)
#
# Usage:
#   ./scripts/capture-snapshot.sh [network] [output-dir]
#
# Arguments:
#   network     – Stellar RPC network name (from stellar CLI config).
#                 Default: local-testnet
#   output-dir  – Directory to write snapshot files.  Default: snapshots/
#
# The generated snapshot files can be loaded in tests with
# `Env::from_snapshot_file("snapshots/<file>.json")`.
# ---------------------------------------------------------------------------

NETWORK="${1:-local-testnet}"
OUTDIR="${2:-snapshots}"

if ! command -v stellar &> /dev/null; then
  echo "Error: 'stellar' CLI is not installed."
  echo "Install it with: cargo install stellar-cli"
  exit 1
fi

mkdir -p "$OUTDIR"

echo "Capturing ledger snapshot from network '$NETWORK'..."
echo "Output directory: $OUTDIR"
echo ""

TIMESTAMP=$(date +%Y%m%d_%H%M%S)
SNAPSHOT_FILE="${OUTDIR}/ledger_snapshot_${TIMESTAMP}.json"

# Capture the full ledger snapshot (ledger entries + info).
# This includes all deployed contracts, balances, and storage.
stellar lab snapshot capture \
  --network "$NETWORK" \
  --output "$SNAPSHOT_FILE"

echo ""
echo "Snapshot saved to: $SNAPSHOT_FILE"
echo ""
echo "To use in fork tests:"
echo "  let env = Env::from_snapshot_file(\"${SNAPSHOT_FILE}\");"
