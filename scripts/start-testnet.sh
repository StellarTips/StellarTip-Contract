#!/usr/bin/env bash
set -euo pipefail

# ---------------------------------------------------------------------------
# Start a local Stellar testnet (Soroban) using Docker.
#
# This spins up a standalone Stellar network suitable for fork/integration
# tests against a live RPC endpoint.
#
# Prerequisites:
#   - Docker
#   - stellar CLI  (cargo install stellar-cli)
#
# Usage:
#   ./scripts/start-testnet.sh          # start and wait
#   ./scripts/start-testnet.sh -d       # daemonize (detach)
# ---------------------------------------------------------------------------

NETWORK_NAME="${NETWORK_NAME:-stellar-tip-testnet}"
CONTAINER_NAME="${CONTAINER_NAME:-stellar-tip-soroban}"
RPC_PORT="${RPC_PORT:-8000}"
FRIENDBOT_PORT="${FRIENDBOT_PORT:-8001}"
IMAGE="${IMAGE:-stellar/quickstart:testing}"

detach=false
while getopts "d" opt; do
  case $opt in
    d) detach=true ;;
    *) echo "Usage: $0 [-d]" >&2; exit 1 ;;
  esac
done

echo "Starting Soroban testnet container '$CONTAINER_NAME'..."
echo "  Image:     $IMAGE"
echo "  RPC port:  $RPC_PORT"
echo "  Friendbot: $FRIENDBOT_PORT"

if docker ps --format '{{.Names}}' | grep -q "^${CONTAINER_NAME}$"; then
  echo "Container '$CONTAINER_NAME' is already running."
  exit 0
fi

docker_run_args=(
  --rm
  --name "$CONTAINER_NAME"
  -p "${RPC_PORT}:8000"
  -p "${FRIENDBOT_PORT}:8001"
  "$IMAGE"
  --standalone
  --enable-soroban-rpc
)

if [ "$detach" = true ]; then
  docker run -d "${docker_run_args[@]}"
  echo "Container started in detached mode."
else
  echo "Starting container (Ctrl+C to stop)..."
  docker run "${docker_run_args[@]}"
fi
