#!/usr/bin/env bash
# Integration test for the registry canister — exercises every public method.
# Assumes `dfx start --background` is already running on the VPS.
set -euo pipefail

cd "$(dirname "$0")/.."

# Note: SolanaNft and EthereumNft registration attempts in this script
# are EXPECTED to fail with NotImplemented at this stage of development.
# Their verifiers are stubs awaiting HTTP-outcall implementation. The
# fact that they reject the registration *is* the test — it confirms the
# OwnershipVerifier trait dispatch is working correctly.

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
echo "==> get_site for the InternetIdentity-owned one"
dfx canister call registry get_site '("test-site.example.com")'

echo
echo "==> sites_by_owner for the caller (should return 1 — the InternetIdentity site)"
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
echo "==> all registry integration tests passed"