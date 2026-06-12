#!/usr/bin/env bash
# Wrapper to run the Node-side CBOR round-trip test from the project root.
# This script auto-installs Node deps on first run.
#
# See scripts/cbor-test/test_hopeandgrace_cbor.js for the actual test logic.

set -euo pipefail

cd "$(dirname "$0")/cbor-test"

if [[ ! -d "node_modules" ]]; then
    echo "==> Installing Node dependencies (one-time setup)..."
    npm install --silent
    echo "    Done."
    echo
fi

echo "==> Running CBOR round-trip test..."
node test_hopeandgrace_cbor.js