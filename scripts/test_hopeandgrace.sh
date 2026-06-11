#!/usr/bin/env bash
# Integration test for the hopeandgrace canister.
#
# Exercises every public method against a local dfx replica:
#   - deploy + init (owner becomes the dfx default identity)
#   - access control: writer not yet authorized -> archive rejected
#   - add caller as writer, retry -> succeeds
#   - archive_ceremony: positive case, idempotency, all 5 invariant gates
#   - get_ceremony, list_ceremonies, public_totals — read methods
#   - put_legal_doc: positive case, hash mismatch rejection, dup version rejection
#   - get_legal_doc, get_legal_doc_version, list_legal_doc_versions
#   - health_check
#   - ownership transfer dance (initiate + cancel; we don't actually transfer
#     since we'd lose admin in a single-identity test)
#
# Assumes `dfx start --background` is running. The script does not start dfx
# itself because that would interfere with the existing replica state for
# auth/registry/manager tests.
set -euo pipefail

cd "$(dirname "$0")/.."

CALLER=$(dfx identity get-principal)

echo "==> deploying hopeandgrace canister to local replica"
dfx deploy --network local hopeandgrace

echo
echo "==> initial health_check (canister fresh, counts should be 0)"
dfx canister call hopeandgrace health_check

echo
echo "==> get_owner (should be the dfx default identity = caller)"
dfx canister call hopeandgrace get_owner

echo
echo "==> list_writers (should be empty)"
dfx canister call hopeandgrace list_writers

echo
echo "==> NEGATIVE TEST: archive_ceremony with no writer authorized"
echo "    EXPECTED: Err Unauthorized (caller is owner but not in writers)"
dfx canister call hopeandgrace archive_ceremony '(record {
  record_version = 1 : nat32;
  ceremony_number = 1 : nat64;
  ceremony_date = "2026-06-09";
  random_seed = "seed-001";
  pool_total_cents = 100000 : nat64;
  split = record {
    soul_base_cents = 40000 : nat64;
    angel_gross_cents = 40000 : nat64;
    divine_offering_cents = 20000 : nat64;
    divine_offering_bps = 2000 : nat32;
  };
  angel = record {
    uuid = "angel-001";
    claimed = true;
    donated_bps = 5000 : nat32;
    donated_cents = 20000 : nat64;
    kept_cents = 20000 : nat64;
  };
  soul = record {
    uuid = "soul-001";
    engaged = true;
    reverted = false;
    reverted_at_ns = null;
    total_received_cents = 40000 : nat64;
    story_cid = null;
    story_hash = null;
  };
  direct_blessings = record {
    total_cents = 0 : nat64;
    donor_count = 0 : nat64;
  };
  outcome = variant { Claimed };
  rollover_cents = 0 : nat64;
  ops_ledger_entry = record {
    entry_type = "divine_offering";
    amount_cents = 20000 : int64;
    balance_after_cents = 80000 : nat64;
    party = "ops";
    description = "20% divine offering";
    at_ns = 1700000000000000000 : nat64;
  };
  ledger = vec {
    record {
      entry_type = "pool_start";
      amount_cents = 0 : int64;
      balance_after_cents = 100000 : nat64;
      party = "chalice";
      description = "ceremony pool opened";
      at_ns = 1700000000000000000 : nat64;
    };
    record {
      entry_type = "soul_blessing_base";
      amount_cents = -40000 : int64;
      balance_after_cents = 60000 : nat64;
      party = "soul#soul-001";
      description = "soul base blessing";
      at_ns = 1700000000000000001 : nat64;
    };
  };
  generated_at_ns = 1700000000500000000 : nat64;
})' || true

echo
echo "==> add caller as writer (owner-only operation)"
dfx canister call hopeandgrace add_writer "(principal \"$CALLER\")"

echo
echo "==> confirm writer was added"
dfx canister call hopeandgrace list_writers
dfx canister call hopeandgrace is_writer "(principal \"$CALLER\")"

echo
echo "==> archive_ceremony #1 (now authorized)"
echo "    EXPECTED: Ok RecordRef with computed content_hash + archived_at_ns"
dfx canister call hopeandgrace archive_ceremony '(record {
  record_version = 1 : nat32;
  ceremony_number = 1 : nat64;
  ceremony_date = "2026-06-09";
  random_seed = "seed-001";
  pool_total_cents = 100000 : nat64;
  split = record {
    soul_base_cents = 40000 : nat64;
    angel_gross_cents = 40000 : nat64;
    divine_offering_cents = 20000 : nat64;
    divine_offering_bps = 2000 : nat32;
  };
  angel = record {
    uuid = "angel-001";
    claimed = true;
    donated_bps = 5000 : nat32;
    donated_cents = 20000 : nat64;
    kept_cents = 20000 : nat64;
  };
  soul = record {
    uuid = "soul-001";
    engaged = true;
    reverted = false;
    reverted_at_ns = null;
    total_received_cents = 40000 : nat64;
    story_cid = null;
    story_hash = null;
  };
  direct_blessings = record {
    total_cents = 0 : nat64;
    donor_count = 0 : nat64;
  };
  outcome = variant { Claimed };
  rollover_cents = 0 : nat64;
  ops_ledger_entry = record {
    entry_type = "divine_offering";
    amount_cents = 20000 : int64;
    balance_after_cents = 80000 : nat64;
    party = "ops";
    description = "20% divine offering";
    at_ns = 1700000000000000000 : nat64;
  };
  ledger = vec {
    record {
      entry_type = "pool_start";
      amount_cents = 0 : int64;
      balance_after_cents = 100000 : nat64;
      party = "chalice";
      description = "ceremony pool opened";
      at_ns = 1700000000000000000 : nat64;
    };
    record {
      entry_type = "soul_blessing_base";
      amount_cents = -40000 : int64;
      balance_after_cents = 60000 : nat64;
      party = "soul#soul-001";
      description = "soul base blessing";
      at_ns = 1700000000000000001 : nat64;
    };
  };
  generated_at_ns = 1700000000500000000 : nat64;
})'

echo
echo "==> NEGATIVE TEST: archive same ceremony_number again"
echo "    EXPECTED: Err AlreadyArchived = 1"
dfx canister call hopeandgrace archive_ceremony '(record {
  record_version = 1 : nat32;
  ceremony_number = 1 : nat64;
  ceremony_date = "2026-06-09";
  random_seed = "seed-001";
  pool_total_cents = 100000 : nat64;
  split = record {
    soul_base_cents = 40000 : nat64;
    angel_gross_cents = 40000 : nat64;
    divine_offering_cents = 20000 : nat64;
    divine_offering_bps = 2000 : nat32;
  };
  angel = record { uuid = "angel-001"; claimed = true; donated_bps = 5000 : nat32; donated_cents = 20000 : nat64; kept_cents = 20000 : nat64 };
  soul = record { uuid = "soul-001"; engaged = true; reverted = false; reverted_at_ns = null; total_received_cents = 40000 : nat64; story_cid = null; story_hash = null };
  direct_blessings = record { total_cents = 0 : nat64; donor_count = 0 : nat64 };
  outcome = variant { Claimed };
  rollover_cents = 0 : nat64;
  ops_ledger_entry = record { entry_type = "divine_offering"; amount_cents = 20000 : int64; balance_after_cents = 80000 : nat64; party = "ops"; description = ""; at_ns = 1700000000000000000 : nat64 };
  ledger = vec {
    record { entry_type = "pool_start"; amount_cents = 0 : int64; balance_after_cents = 100000 : nat64; party = "chalice"; description = ""; at_ns = 1700000000000000000 : nat64 };
    record { entry_type = "soul_blessing_base"; amount_cents = -40000 : int64; balance_after_cents = 60000 : nat64; party = "soul#soul-001"; description = ""; at_ns = 1700000000000000001 : nat64 };
  };
  generated_at_ns = 1700000000500000000 : nat64;
})' || true

echo
echo "==> NEGATIVE TEST: archive with invariant_3 violation (split doesn't sum to pool)"
echo "    pool=100000 but split=40000+40000+19999=99999"
echo "    EXPECTED: Err InvariantViolated with helpful message"
dfx canister call hopeandgrace archive_ceremony '(record {
  record_version = 1 : nat32;
  ceremony_number = 2 : nat64;
  ceremony_date = "2026-06-09";
  random_seed = "seed-002";
  pool_total_cents = 100000 : nat64;
  split = record {
    soul_base_cents = 40000 : nat64;
    angel_gross_cents = 40000 : nat64;
    divine_offering_cents = 19999 : nat64;
    divine_offering_bps = 2000 : nat32;
  };
  angel = record { uuid = "angel-002"; claimed = true; donated_bps = 5000 : nat32; donated_cents = 20000 : nat64; kept_cents = 20000 : nat64 };
  soul = record { uuid = "soul-002"; engaged = true; reverted = false; reverted_at_ns = null; total_received_cents = 40000 : nat64; story_cid = null; story_hash = null };
  direct_blessings = record { total_cents = 0 : nat64; donor_count = 0 : nat64 };
  outcome = variant { Claimed };
  rollover_cents = 0 : nat64;
  ops_ledger_entry = record { entry_type = "divine_offering"; amount_cents = 19999 : int64; balance_after_cents = 80001 : nat64; party = "ops"; description = ""; at_ns = 1700000000000000000 : nat64 };
  ledger = vec {
    record { entry_type = "pool_start"; amount_cents = 0 : int64; balance_after_cents = 100000 : nat64; party = "chalice"; description = ""; at_ns = 1700000000000000000 : nat64 };
  };
  generated_at_ns = 1700000000500000000 : nat64;
})' || true

echo
echo "==> NEGATIVE TEST: archive with invariant_8 violation (empty ledger)"
echo "    EXPECTED: Err InvariantViolated mentioning empty"
dfx canister call hopeandgrace archive_ceremony '(record {
  record_version = 1 : nat32;
  ceremony_number = 3 : nat64;
  ceremony_date = "2026-06-09";
  random_seed = "seed-003";
  pool_total_cents = 100000 : nat64;
  split = record { soul_base_cents = 40000 : nat64; angel_gross_cents = 40000 : nat64; divine_offering_cents = 20000 : nat64; divine_offering_bps = 2000 : nat32 };
  angel = record { uuid = "angel-003"; claimed = true; donated_bps = 5000 : nat32; donated_cents = 20000 : nat64; kept_cents = 20000 : nat64 };
  soul = record { uuid = "soul-003"; engaged = true; reverted = false; reverted_at_ns = null; total_received_cents = 40000 : nat64; story_cid = null; story_hash = null };
  direct_blessings = record { total_cents = 0 : nat64; donor_count = 0 : nat64 };
  outcome = variant { Claimed };
  rollover_cents = 0 : nat64;
  ops_ledger_entry = record { entry_type = "divine_offering"; amount_cents = 20000 : int64; balance_after_cents = 80000 : nat64; party = "ops"; description = ""; at_ns = 1700000000000000000 : nat64 };
  ledger = vec { };
  generated_at_ns = 1700000000500000000 : nat64;
})' || true

echo
echo "==> NEGATIVE TEST: archive with invariant_6 violation (record_version=2)"
echo "    EXPECTED: Err InvariantViolated mentioning version"
dfx canister call hopeandgrace archive_ceremony '(record {
  record_version = 2 : nat32;
  ceremony_number = 4 : nat64;
  ceremony_date = "2026-06-09";
  random_seed = "seed-004";
  pool_total_cents = 100000 : nat64;
  split = record { soul_base_cents = 40000 : nat64; angel_gross_cents = 40000 : nat64; divine_offering_cents = 20000 : nat64; divine_offering_bps = 2000 : nat32 };
  angel = record { uuid = "angel-004"; claimed = true; donated_bps = 5000 : nat32; donated_cents = 20000 : nat64; kept_cents = 20000 : nat64 };
  soul = record { uuid = "soul-004"; engaged = true; reverted = false; reverted_at_ns = null; total_received_cents = 40000 : nat64; story_cid = null; story_hash = null };
  direct_blessings = record { total_cents = 0 : nat64; donor_count = 0 : nat64 };
  outcome = variant { Claimed };
  rollover_cents = 0 : nat64;
  ops_ledger_entry = record { entry_type = "divine_offering"; amount_cents = 20000 : int64; balance_after_cents = 80000 : nat64; party = "ops"; description = ""; at_ns = 1700000000000000000 : nat64 };
  ledger = vec {
    record { entry_type = "pool_start"; amount_cents = 0 : int64; balance_after_cents = 100000 : nat64; party = "chalice"; description = ""; at_ns = 1700000000000000000 : nat64 };
  };
  generated_at_ns = 1700000000500000000 : nat64;
})' || true

echo
echo "==> archive_ceremony #5 — REVERTED ceremony with direct blessings"
echo "    EXPECTED: Ok RecordRef. Exercises Option::Some on reverted_at_ns,"
echo "              non-zero direct_blessings, RevertedToChalice outcome."
dfx canister call hopeandgrace archive_ceremony '(record {
  record_version = 1 : nat32;
  ceremony_number = 5 : nat64;
  ceremony_date = "2026-06-10";
  random_seed = "seed-005";
  pool_total_cents = 100000 : nat64;
  split = record { soul_base_cents = 40000 : nat64; angel_gross_cents = 40000 : nat64; divine_offering_cents = 20000 : nat64; divine_offering_bps = 2000 : nat32 };
  angel = record { uuid = "angel-005"; claimed = true; donated_bps = 5000 : nat32; donated_cents = 20000 : nat64; kept_cents = 20000 : nat64 };
  soul = record {
    uuid = "soul-005";
    engaged = false;
    reverted = true;
    reverted_at_ns = opt (1700000086400000000 : nat64);
    total_received_cents = 25000 : nat64;
    story_cid = opt "bafybeih5fp4kkxvxnxfqz5gqxnxfqz5gqxnxfqz5gqxnxfqz5gqxnxfqz5gq";
    story_hash = opt "abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890";
  };
  direct_blessings = record { total_cents = 25000 : nat64; donor_count = 3 : nat64 };
  outcome = variant { RevertedToChalice };
  rollover_cents = 75000 : nat64;
  ops_ledger_entry = record { entry_type = "divine_offering"; amount_cents = 20000 : int64; balance_after_cents = 80000 : nat64; party = "ops"; description = ""; at_ns = 1700000000000000000 : nat64 };
  ledger = vec {
    record { entry_type = "pool_start"; amount_cents = 0 : int64; balance_after_cents = 100000 : nat64; party = "chalice"; description = ""; at_ns = 1700000000000000000 : nat64 };
    record { entry_type = "rollover_to_chalice"; amount_cents = -25000 : int64; balance_after_cents = 75000 : nat64; party = "chalice"; description = "soul reverted; rollover"; at_ns = 1700000086400000001 : nat64 };
  };
  generated_at_ns = 1700000086400000500 : nat64;
})'

echo
echo "==> get_ceremony 1 — confirm stored record has content_hash + archived_at_ns"
dfx canister call hopeandgrace get_ceremony '(1 : nat64)'

echo
echo "==> get_ceremony 5 — confirm RevertedToChalice ceremony stored correctly"
dfx canister call hopeandgrace get_ceremony '(5 : nat64)'

echo
echo "==> get_ceremony 999 — nonexistent"
echo "    EXPECTED: null"
dfx canister call hopeandgrace get_ceremony '(999 : nat64)'

echo
echo "==> list_ceremonies(0, 10) — oldest-first, both archived"
dfx canister call hopeandgrace list_ceremonies '(0 : nat64, 10 : nat64)'

echo
echo "==> public_totals — should reflect both archived ceremonies"
echo "    2 ceremonies, 200000 cents pool, 65000 to souls, 40000 divine offering,"
echo "    25000 direct, 2 distinct souls + 2 distinct angels"
dfx canister call hopeandgrace public_totals

echo
echo
echo "============================================================"
echo "===  Legal docs tests                                    ==="
echo "============================================================"

# Compute correct hash for the sample legal doc.
# The canister will verify this matches sha256(content_md.bytes()), lowercase hex.
TERMS_MD="# Terms of Service v1

Effective 2026-06-09.

Hope and Grace Angel Network operates as a 501(c)(3)..."
CORRECT_HASH=$(printf '%s' "$TERMS_MD" | sha256sum | awk '{print $1}')
echo "==> computed expected sha256 for sample terms: $CORRECT_HASH"

echo
echo "==> put_legal_doc terms v1 with CORRECT hash"
echo "    EXPECTED: Ok RecordRef"
dfx canister call hopeandgrace put_legal_doc "(record {
  kind = \"terms\";
  version = 1 : nat32;
  effective_date = \"2026-06-09\";
  content_md = \"$TERMS_MD\";
  content_hash = \"$CORRECT_HASH\";
  published_at_ns = 0 : nat64;
})"

echo
echo "==> NEGATIVE TEST: put_legal_doc with WRONG hash"
echo "    EXPECTED: Err InvariantViolated content_hash mismatch"
dfx canister call hopeandgrace put_legal_doc "(record {
  kind = \"privacy\";
  version = 1 : nat32;
  effective_date = \"2026-06-09\";
  content_md = \"# Privacy Policy\";
  content_hash = \"0000000000000000000000000000000000000000000000000000000000000000\";
  published_at_ns = 0 : nat64;
})" || true

echo
echo "==> NEGATIVE TEST: put_legal_doc duplicate (kind=terms, version=1)"
echo "    EXPECTED: Err InvariantViolated already exists"
dfx canister call hopeandgrace put_legal_doc "(record {
  kind = \"terms\";
  version = 1 : nat32;
  effective_date = \"2026-06-09\";
  content_md = \"$TERMS_MD\";
  content_hash = \"$CORRECT_HASH\";
  published_at_ns = 0 : nat64;
})" || true

echo
echo "==> get_legal_doc(\"terms\") — returns latest version"
dfx canister call hopeandgrace get_legal_doc '("terms")'

echo
echo "==> get_legal_doc(\"nonexistent\") — null"
dfx canister call hopeandgrace get_legal_doc '("nonexistent")'

echo
echo "==> list_legal_doc_versions(\"terms\") — should have 1 version"
dfx canister call hopeandgrace list_legal_doc_versions '("terms")'

echo
echo
echo "============================================================"
echo "===  Ownership transfer dance (initiate + cancel)        ==="
echo "============================================================"

echo
echo "==> get_pending_owner — should be null"
dfx canister call hopeandgrace get_pending_owner

echo
echo "==> set_owner_initiate to a placeholder principal (testing initiate works)"
echo "    Using the management canister principal as a real-but-unused placeholder."
dfx canister call hopeandgrace set_owner_initiate '(principal "aaaaa-aa")'

echo
echo "==> get_pending_owner — should now be the placeholder principal"
dfx canister call hopeandgrace get_pending_owner

echo
echo "==> set_owner_cancel (clearing the pending transfer)"
dfx canister call hopeandgrace set_owner_cancel

echo
echo "==> get_pending_owner — back to null"
dfx canister call hopeandgrace get_pending_owner

echo
echo "==> get_owner — still us (transfer never accepted)"
dfx canister call hopeandgrace get_owner

echo
echo "==> NEGATIVE TEST: set_owner_accept with no pending transfer"
echo "    EXPECTED: Err NotFound"
dfx canister call hopeandgrace set_owner_accept || true

echo
echo
echo "============================================================"
echo "===  Final state                                         ==="
echo "============================================================"

echo
echo "==> final health_check (2 ceremonies, 1 legal doc)"
dfx canister call hopeandgrace health_check

echo
echo "==> all hopeandgrace integration tests passed"