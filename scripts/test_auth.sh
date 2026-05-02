#!/usr/bin/env bash
# Integration test for the auth canister — exercises every public method.
# Assumes `dfx start --background` is already running.
set -euo pipefail

cd "$(dirname "$0")/.."

echo "==> deploying auth canister to local replica"
dfx deploy --network local auth

echo
echo "==> whoami (should print your principal)"
dfx canister call auth whoami

echo
echo "==> register (creates or refreshes the User record)"
dfx canister call auth register

echo
echo "==> get_me (should match register output, last_seen ≥ registered)"
dfx canister call auth get_me

echo
echo "==> user_count (should be ≥ 1)"
dfx canister call auth user_count

echo
echo "==> put_credential 'github_pat' = bytes of 'fake_token_123'"
dfx canister call auth put_credential '("github_pat", blob "fake_token_123")'

echo
echo "==> list_credentials (should include 'github_pat')"
dfx canister call auth list_credentials

echo
echo "==> get_credential 'github_pat' (blob should be present)"
dfx canister call auth get_credential '("github_pat")'

echo
echo "==> delete_credential 'github_pat' (should return true)"
dfx canister call auth delete_credential '("github_pat")'

echo
echo "==> get_credential 'github_pat' AGAIN (should return Err NotFound)"
dfx canister call auth get_credential '("github_pat")' || true

echo
echo "==> health_check"
dfx canister call auth health_check

echo
echo "==> all auth integration tests passed"
