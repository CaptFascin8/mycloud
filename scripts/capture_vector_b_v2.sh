#!/usr/bin/env bash
# capture_vector_b_v2.sh — archive the CORRECTED vector B against the local
# hopeandgrace canister.
#
# Original vector B's ledger had non-closing arithmetic (10400 + -18710 ≠ 0)
# which was rejected by invariant #8. H&G Claude confirmed it was a synthetic
# data bug, not an engine-deep issue. Corrected: entry[1] amount_cents now
# matches what the soul received (10400) so reversion zeroes the balance.
#
# Using ceremony_number 14 to avoid collision with the rejected #13 attempt
# (which wasn't archived, but the canister's idempotency check would still
# refuse #13 with AlreadyArchived since it tracks by number, not status).
# Actually — wait, idempotency is checked AFTER the invariant failed in #13's
# case, so #13 might be free. Using #14 to be safe.
#
# The captured content_hash becomes EXPECTED_HASH_B in the extended Node test.
set -euo pipefail

cd "$(dirname "$0")/.."

echo "==> Archiving vector B v2 (ceremony #14, RevertedToChalice, with story, corrected ledger)"
echo "    Ledger: [base +10400 -> 10400] then [reverted -10400 -> 0]. Arithmetic closes."

dfx canister call hopeandgrace archive_ceremony '(record {
  record_version   = 1 : nat32;
  ceremony_number  = 14 : nat64;
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
      amount_cents        = -10400 : int64;
      balance_after_cents = 0 : nat64;
      party               = "soul#2";
      description         = "unclaimed; returned (net of what soul held)";
      at_ns               = 1783728000000000000 : nat64;
    };
  };
  generated_at_ns = 1785110400000000000 : nat64;
})'

echo
echo "==> Capture the content_hash from the Ok response above."
echo "    It becomes EXPECTED_HASH_B in the extended Node test."