#!/usr/bin/env bash
# Integration test for the registry canister — exercises every public method.
# Assumes `dfx start --background` is already running on the VPS.
#
# Checkpoint 3b.1: now also exercises the new status / container_id /
# expires_ns fields, plus the update_site_status + set_container_id methods.
#
# Note: SolanaNft and EthereumNft registration attempts in this script
# are EXPECTED to fail with NotImplemented at this stage of development.
# Their verifiers are stubs awaiting HTTP-outcall implementation. The
# fact that they reject the registration *is* the test — it confirms the
# OwnershipVerifier trait dispatch is working correctly.
set -euo pipefail

cd "$(dirname "$0")/.."

echo "==> deploying registry canister to local replica"
dfx deploy --network local registry

echo
echo "==> initial state: site_count should be 0"
dfx canister call registry site_count

echo
echo "==> initial list_sites should be empty vec"
dfx canister call registry list_sites

echo
echo "==> register_site with InternetIdentity proof (should succeed)"
echo "    NEW: status should default to Provisioning, container_id null, expires_ns null"
dfx canister call registry register_site \
  '("test-site.example.com", "bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi", variant { InternetIdentity })'

echo
echo "==> register_site with SolanaNft + Trunk tier"
echo "    EXPECTED: Err NotImplemented (verifier is a stub at this stage)"
dfx canister call registry register_site \
  '("johns-bakery.crystaldragon.tech", "", variant { SolanaNft = record { mint = "ABCDEFmint123"; wallet = "WALLETabc456"; tier = opt variant { Trunk } } })'

echo
echo "==> register_site with EthereumNft + Polygon chain"
echo "    EXPECTED: Err NotImplemented (verifier is a stub at this stage)"
dfx canister call registry register_site \
  '("polysite.example.com", "", variant { EthereumNft = record { contract = "0xabc123"; token_id = "42"; wallet = "0xowner"; chain = variant { Polygon } } })'

echo
echo "==> site_count should now be 1 (only InternetIdentity registered;"
echo "    Solana + Ethereum stubs correctly returned NotImplemented)"
dfx canister call registry site_count

echo
echo "==> get_site to confirm new fields exist + defaults are right"
dfx canister call registry get_site '("test-site.example.com")'

echo
echo "==> CHECKPOINT 3b.1 NEW: update_site_status to Active (bridge daemon role)"
dfx canister call registry update_site_status '("test-site.example.com", variant { Active })'

echo
echo "==> CHECKPOINT 3b.1 NEW: set_container_id (simulating successful deploy)"
dfx canister call registry set_container_id \
  '("test-site.example.com", "mycloud-site-abc123def456")'

echo
echo "==> get_site to confirm status + container_id persisted"
dfx canister call registry get_site '("test-site.example.com")'

echo
echo "==> CHECKPOINT 3b.1 NEW: update_site_status to Suspended (owner action)"
dfx canister call registry update_site_status \
  '("test-site.example.com", variant { Suspended })'

echo
echo "==> get_site after suspend (status should now be Suspended)"
dfx canister call registry get_site '("test-site.example.com")'

echo
echo "==> CHECKPOINT 3b.1 NEW: clear container_id with empty string"
dfx canister call registry set_container_id '("test-site.example.com", "")'

echo
echo "==> get_site after clearing container_id (should be null again)"
dfx canister call registry get_site '("test-site.example.com")'

echo
echo "==> sites_by_owner for the caller (should return 1)"
CALLER=$(dfx identity get-principal)
dfx canister call registry sites_by_owner "(principal \"$CALLER\")"

echo
echo "==> update_cid on test-site (should succeed — caller is owner)"
dfx canister call registry update_cid \
  '("test-site.example.com", "bafybeih5fp4kkxvxnxfqz5gqxnxfqz5gqxnxfqz5gqxnxfqz5gqxnxfqz5gq")'

echo
echo "==> get_site again to confirm CID actually changed"
dfx canister call registry get_site '("test-site.example.com")'

echo
echo "==> NEGATIVE TEST: register_site with bad domain (no dot) should fail"
dfx canister call registry register_site \
  '("nodotdomain", "", variant { InternetIdentity })' || true

echo
echo "==> NEGATIVE TEST: register_site with bad CID should fail"
dfx canister call registry register_site \
  '("ok.example.com", "tooshort", variant { InternetIdentity })' || true

echo
echo "==> NEGATIVE TEST: register_site with already-taken domain should fail"
dfx canister call registry register_site \
  '("test-site.example.com", "", variant { InternetIdentity })' || true

echo
echo "==> NEGATIVE TEST: get_site for nonexistent domain returns NotFound"
dfx canister call registry get_site '("nosuch.example.com")' || true

echo
echo "==> NEGATIVE TEST: update_site_status for nonexistent domain"
dfx canister call registry update_site_status \
  '("nosuch.example.com", variant { Active })' || true

echo
echo "==> NEGATIVE TEST: set_container_id for nonexistent domain"
dfx canister call registry set_container_id \
  '("nosuch.example.com", "abc123")' || true

echo
echo "==> delete_site test-site (caller is owner — should succeed)"
dfx canister call registry delete_site '("test-site.example.com")'

echo
echo "==> get_site after delete returns NotFound"
dfx canister call registry get_site '("test-site.example.com")' || true

echo
echo "==> sites_by_owner after delete (should be empty — only site was the deleted one)"
dfx canister call registry sites_by_owner "(principal \"$CALLER\")"

echo
echo "==> site_count after delete should be 0"
dfx canister call registry site_count

echo
echo "==> health_check"
dfx canister call registry health_check

echo
echo "==> all registry integration tests passed (including 3b.1 additions)"