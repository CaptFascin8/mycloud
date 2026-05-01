#!/usr/bin/env bash
# Checkpoint 2 verifier. Run from project root.
set -euo pipefail

echo "==> cargo check --workspace"
cargo check --workspace --target wasm32-unknown-unknown

echo "==> dfx build"
if ! pgrep -f "dfx start" >/dev/null; then
  echo "    starting local replica in background..."
  dfx start --background --clean
fi
dfx build

echo "==> all checks passed"
