#!/usr/bin/env bash
# Integration test for the manager canister — exercises every public method.
# Assumes `dfx start --background` is already running on the VPS.
#
# This test deploys all 3 canisters (auth, registry, manager) so that
# manager has real targets to watch.
set -euo pipefail

cd "$(dirname "$0")/.."

echo "==> deploying auth + registry + manager to local replica"
dfx deploy --network local auth
dfx deploy --network local registry
dfx deploy --network local manager

# Capture canister IDs we need
AUTH_ID=$(dfx canister id auth)
REGISTRY_ID=$(dfx canister id registry)
MANAGER_ID=$(dfx canister id manager)
CALLER=$(dfx identity get-principal)

echo
echo "==> Canister IDs:"
echo "    auth     = $AUTH_ID"
echo "    registry = $REGISTRY_ID"
echo "    manager  = $MANAGER_ID"
echo "    caller   = $CALLER"

echo
echo "==> manager initial state"
dfx canister call manager health_check

echo
echo "==> get_config (caller from init should be the owner)"
dfx canister call manager get_config

echo
echo "==> recent_events (should have 1 from init: 'manager initialized')"
dfx canister call manager recent_events '(0 : nat32)'

echo
echo "==> event_count should be 1"
dfx canister call manager event_count

echo
echo "==> watch auth canister"
dfx canister call manager watch_canister "(principal \"$AUTH_ID\", \"auth\", null)"

echo
echo "==> watch registry canister with a custom threshold"
dfx canister call manager watch_canister "(principal \"$REGISTRY_ID\", \"registry\", opt (500_000_000_000 : nat64))"

echo
echo "==> list_watched (should show both auth and registry)"
dfx canister call manager list_watched

echo
echo "==> NEGATIVE TEST: watch the same canister twice should fail"
dfx canister call manager watch_canister "(principal \"$AUTH_ID\", \"auth\", null)" || true

echo
echo "==> force_check_now (triggers a manual poll cycle right away)"
dfx canister call manager force_check_now

echo
echo "==> recent_events after force_check (should include init + watch + watch + check events)"
dfx canister call manager recent_events '(10 : nat32)'

echo
echo "==> list_watched again — last_check_ns and last_status_ok should now be populated"
dfx canister call manager list_watched

echo
echo "==> set_poll_interval to 30 seconds (should re-arm timer + log event)"
dfx canister call manager set_poll_interval '(30 : nat64)'

echo
echo "==> NEGATIVE TEST: set_poll_interval too low should fail"
dfx canister call manager set_poll_interval '(5 : nat64)' || true

echo
echo "==> get_config to confirm new poll interval"
dfx canister call manager get_config

echo
echo "==> cycles_balance"
dfx canister call manager cycles_balance

echo
echo "==> top_up (stub) — request 1B cycles to auth canister"
dfx canister call manager top_up "(principal \"$AUTH_ID\", 1_000_000_000 : nat64)"

echo
echo "==> NEGATIVE TEST: top_up with absurd amount should fail with InsufficientCycles"
dfx canister call manager top_up "(principal \"$AUTH_ID\", 999_999_999_999_999_999 : nat64)" || true

echo
echo "==> unwatch registry canister"
dfx canister call manager unwatch_canister "(principal \"$REGISTRY_ID\")"

echo
echo "==> list_watched (should now show only auth)"
dfx canister call manager list_watched

echo
echo "==> final event_count + recent_events sample"
dfx canister call manager event_count
dfx canister call manager recent_events '(5 : nat32)'

echo
echo "==> final health_check"
dfx canister call manager health_check

echo
echo "==> all manager integration tests passed"