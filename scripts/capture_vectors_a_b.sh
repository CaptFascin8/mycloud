#!/usr/bin/env bash
# capture_vectors_a_b.sh — archive vectors A and B against the local hopeandgrace
# canister and print the canister-computed content_hash for each.
#
# These vectors come from scripts/icp/selfcheck.js (the H&G side) and exercise
# the CBOR cross-language divergence points: Option None/Some, both
# CeremonyOutcome variants, negative amount_cents, multi-entry ledger.
#
# The two content_hashes printed by this script become the EXPECTED_HASH values
# in the extended Pass 4C Node test, which proves Rust ↔ Node byte agreement
# for vectors A and B (currently we've only proven ceremony #1).
#
# Requires:
#   - dfx running locally
#   - hopeandgrace canister deployed locally
#   - default identity authorized as a writer (we set this up yesterday)
#
# Run once. Capture the two hex strings it prints. Done.
set -euo pipefail

cd "$(dirname "$0")/.."

echo "==> Archiving vector A (ceremony #12, Claimed, no story)"
dfx canister call hopeandgrace archive_ceremony '(record {
  record_version   = 1 : nat32;
  ceremony_number  = 12 : nat64;
  ceremony_date    = "2026-06-20";
  random_seed      = "seed-abc-123";
  pool_total_cents = 60000 : nat64;
  split = record {
    soul_base_cents       = 12000 : nat64;
    angel_gross_cents     = 38400 : nat64;
    divine_offering_cents = 9600 : nat64;
    divine_offering_bps   = 2000 : nat32;
  };
  angel = record {
    uuid          = "angel-uuid-1";
    claimed       = true;
    donated_bps   = 5000 : nat32;
    donated_cents = 19200 : nat64;
    kept_cents    = 19200 : nat64;
  };
  soul = record {
    uuid                 = "soul-uuid-1";
    engaged              = true;
    reverted             = false;
    reverted_at_ns       = null;
    total_received_cents = 31200 : nat64;
    story_cid            = null;
    story_hash           = null;
  };
  direct_blessings = record {
    total_cents = 0 : nat64;
    donor_count = 0 : nat64;
  };
  outcome        = variant { Claimed };
  rollover_cents = 0 : nat64;
  ops_ledger_entry = record {
    entry_type          = "offering_income";
    amount_cents        = 9600 : int64;
    balance_after_cents = 9600 : nat64;
    party               = "ops";
    description         = "Divine Offering";
    at_ns               = 1781913600000000000 : nat64;
  };
  ledger = vec {
    record {
      entry_type          = "soul_blessing_base";
      amount_cents        = 12000 : int64;
      balance_after_cents = 12000 : nat64;
      party               = "soul#1";
      description         = "base";
      at_ns               = 1781913600000000000 : nat64;
    };
    record {
      entry_type          = "angel_gift";
      amount_cents        = 19200 : int64;
      balance_after_cents = 31200 : nat64;
      party               = "soul#1";
      description         = "gift";
      at_ns               = 1782518400000000000 : nat64;
    };
  };
  generated_at_ns = 1784505600000000000 : nat64;
})'

echo
echo "==> Archiving vector B (ceremony #13, RevertedToChalice, with story, negative ledger entry)"
dfx canister call hopeandgrace archive_ceremony '(record {
  record_version   = 1 : nat32;
  ceremony_number  = 13 : nat64;
  ceremony_date    = "2026-06-27";
  random_seed      = "seed-def-456";
  pool_total_cents = 52000 : nat64;
  split = record {
    soul_base_cents       = 10400 : nat64;
    angel_gross_cents     = 33242 : nat64;
    divine_offering_cents = 8358 : nat64;
    divine_offering_bps   = 1928 : nat32;
  };
  angel = record {
    uuid          = "angel-uuid-2";
    claimed       = true;
    donated_bps   = 2500 : nat32;
    donated_cents = 8310 : nat64;
    kept_cents    = 24932 : nat64;
  };
  soul = record {
    uuid                 = "soul-uuid-2";
    engaged              = false;
    reverted             = true;
    reverted_at_ns       = opt (1783728000000000000 : nat64);
    total_received_cents = 0 : nat64;
    story_cid            = opt "bafybeexamplecidB";
    story_hash           = opt "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
  };
  direct_blessings = record {
    total_cents = 1500 : nat64;
    donor_count = 3 : nat64;
  };
  outcome        = variant { RevertedToChalice };
  rollover_cents = 20210 : nat64;
  ops_ledger_entry = record {
    entry_type          = "offering_income";
    amount_cents        = 8358 : int64;
    balance_after_cents = 17958 : nat64;
    party               = "ops";
    description         = "Divine Offering";
    at_ns               = 1782518400000000000 : nat64;
  };
  ledger = vec {
    record {
      entry_type          = "soul_blessing_base";
      amount_cents        = 10400 : int64;
      balance_after_cents = 10400 : nat64;
      party               = "soul#2";
      description         = "base";
      at_ns               = 1782518400000000000 : nat64;
    };
    record {
      entry_type          = "blessing_reverted";
      amount_cents        = -18710 : int64;
      balance_after_cents = 0 : nat64;
      party               = "soul#2";
      description         = "unclaimed; returned";
      at_ns               = 1783728000000000000 : nat64;
    };
  };
  generated_at_ns = 1785110400000000000 : nat64;
})'

echo
echo "==> Capture the content_hash values from the two Ok responses above."
echo "    They become EXPECTED_HASH_A and EXPECTED_HASH_B in the extended Node test."