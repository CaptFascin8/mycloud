// scripts/cbor-test/test_hopeandgrace_cbor.js
//
// CROSS-LANGUAGE CBOR ROUND-TRIP TEST — the canonical-encoding contract.
//
// The hopeandgrace canister computes content_hash as
//     sha256(canonical_cbor(SettlementRecordInput))
// using Rust's `ciborium` crate. For "don't trust us, verify us" to be real,
// anyone else must be able to reproduce that exact hash from the same data.
// This script proves that's possible from Node — meaning H&G's Node-side
// archive job AND any independent auditor can verify our records.
//
// REGRESSION ANCHORS — this test pins THREE vectors with their exact expected
// hashes. The hashes were captured by archiving the inputs against the local
// hopeandgrace canister (Rust ciborium side). Node re-encodes the same inputs
// (cbor-x), sha256s, and asserts byte-equal.
//
// If any of these three drift in future maintenance — ciborium version bump,
// cbor-x version bump, field reorder, serde derive change, anything that
// silently alters canonical CBOR output — the test fails loudly. That's the
// difference between "we checked once" and "we can't silently break this."
//
// The vectors together cover all CBOR cross-language divergence points:
//   • Ceremony #1 — Claimed, Option::None for story, positive ledger only
//   • Vector A   — Claimed, Option::None for story, positive multi-entry ledger
//   • Vector B   — RevertedToChalice, Option::Some for both story_cid and
//                  story_hash, Option::Some for reverted_at_ns, NEGATIVE
//                  amount_cents in ledger, multi-entry ledger with closure
//
// Maintenance: if the sample data is ever changed, re-archive each vector
// against a fresh canister, copy the returned content_hash into the
// corresponding EXPECTED constant below. See scripts/capture_vectors_a_b.sh
// and scripts/capture_vector_b_v2.sh for the dfx commands.

import { Encoder } from 'cbor-x';
import { createHash } from 'node:crypto';

// ============================================================================
// EXPECTED HASHES — captured from canister runs against the local replica.
// ============================================================================

const EXPECTED_HASH_CEREMONY_1 = "0c8eefd7d9ba7b3124e42a448619ecc65b9d5fc9bbe240cabe99d196c4f63e8a";
const EXPECTED_HASH_VECTOR_A   = "e500f4cac409b8980dc44decf1ee525ab0b763297206dd5e4b42b76ce9c4a7a7";
const EXPECTED_HASH_VECTOR_B   = "28781661190b065e00acf763e32c587f1d6955d051621a1d2fc6d7ffff6d0a2b";

// ============================================================================
// CBOR ENCODER — tuned to match ciborium's output:
//   - useRecords: false           emit as CBOR map (not cbor-x's record table)
//   - variableMapSize: true       definite-length maps (also ciborium default)
//   - useFloat32: 0               never use float32 (we have no floats anyway)
//
// CRITICAL WORKAROUND: cbor-x encodes ALL BigInts using the full 8-byte form
// regardless of value, violating canonical CBOR's shortest-encoding rule
// (RFC 8949 §4.2). We work around it by using plain JS Numbers for values
// that fit (≤ 2^53 - 1) and reserving BigInt only for timestamp fields that
// need the full 8 bytes. The regression check below verifies the workaround
// is still needed.
// ============================================================================

const encoder = new Encoder({
    useRecords:      false,
    variableMapSize: true,
    largeBigIntToFloat: false,
    useFloat32:      0,
});

// ============================================================================
// VECTOR DATA — the same logical SettlementRecordInput data archived against
// the canister. Field declaration order matters and must match the Rust
// SettlementRecordInput struct in backend/hopeandgrace/src/lib.rs.
// ============================================================================

const ceremony1Input = {
    record_version:   1,
    ceremony_number:  1,                       // small, fits in Number
    ceremony_date:    "2026-06-09",
    random_seed:      "seed-001",
    pool_total_cents: 100000,
    split: {
        soul_base_cents:       40000,
        angel_gross_cents:     40000,
        divine_offering_cents: 20000,
        divine_offering_bps:   2000,
    },
    angel: {
        uuid:          "angel-001",
        claimed:       true,
        donated_bps:   5000,
        donated_cents: 20000,
        kept_cents:    20000,
    },
    soul: {
        uuid:                 "soul-001",
        engaged:              true,
        reverted:             false,
        reverted_at_ns:       null,
        total_received_cents: 40000,
        story_cid:            null,
        story_hash:           null,
    },
    direct_blessings: {
        total_cents: 0,
        donor_count: 0,
    },
    outcome:          "Claimed",
    rollover_cents:   0,
    ops_ledger_entry: {
        entry_type:          "divine_offering",
        amount_cents:        20000,
        balance_after_cents: 80000,
        party:               "ops",
        description:         "20% divine offering",
        at_ns:               1700000000000000000n,
    },
    ledger: [
        {
            entry_type:          "pool_start",
            amount_cents:        0,
            balance_after_cents: 100000,
            party:               "chalice",
            description:         "ceremony pool opened",
            at_ns:               1700000000000000000n,
        },
        {
            entry_type:          "soul_blessing_base",
            amount_cents:        -40000,
            balance_after_cents: 60000,
            party:               "soul#soul-001",
            description:         "soul base blessing",
            at_ns:               1700000000000000001n,
        },
    ],
    generated_at_ns:  1700000000500000000n,
};

// Vector A: from H&G Claude's selfcheck.js — Claimed, no story, positive ledger
const vectorA = {
    record_version:   1,
    ceremony_number:  12,
    ceremony_date:    "2026-06-20",
    random_seed:      "seed-abc-123",
    pool_total_cents: 60000,
    split: {
        soul_base_cents:       12000,
        angel_gross_cents:     38400,
        divine_offering_cents: 9600,
        divine_offering_bps:   2000,
    },
    angel: {
        uuid:          "angel-uuid-1",
        claimed:       true,
        donated_bps:   5000,
        donated_cents: 19200,
        kept_cents:    19200,
    },
    soul: {
        uuid:                 "soul-uuid-1",
        engaged:              true,
        reverted:             false,
        reverted_at_ns:       null,
        total_received_cents: 31200,
        story_cid:            null,
        story_hash:           null,
    },
    direct_blessings: {
        total_cents: 0,
        donor_count: 0,
    },
    outcome:          "Claimed",
    rollover_cents:   0,
    ops_ledger_entry: {
        entry_type:          "offering_income",
        amount_cents:        9600,
        balance_after_cents: 9600,
        party:               "ops",
        description:         "Divine Offering",
        at_ns:               1781913600000000000n,
    },
    ledger: [
        {
            entry_type:          "soul_blessing_base",
            amount_cents:        12000,
            balance_after_cents: 12000,
            party:               "soul#1",
            description:         "base",
            at_ns:               1781913600000000000n,
        },
        {
            entry_type:          "angel_gift",
            amount_cents:        19200,
            balance_after_cents: 31200,
            party:               "soul#1",
            description:         "gift",
            at_ns:               1782518400000000000n,
        },
    ],
    generated_at_ns:  1784505600000000000n,
};

// Vector B (corrected, v2): from H&G Claude's selfcheck.js with ledger fix —
// RevertedToChalice, WITH story, NEGATIVE amount_cents, multi-entry ledger
// with closing arithmetic.
const vectorB = {
    record_version:   1,
    ceremony_number:  14,
    ceremony_date:    "2026-06-27",
    random_seed:      "seed-def-456",
    pool_total_cents: 52000,
    split: {
        soul_base_cents:       10400,
        angel_gross_cents:     33242,
        divine_offering_cents: 8358,
        divine_offering_bps:   1928,
    },
    angel: {
        uuid:          "angel-uuid-2",
        claimed:       true,
        donated_bps:   2500,
        donated_cents: 8310,
        kept_cents:    24932,
    },
    soul: {
        uuid:                 "soul-uuid-2",
        engaged:              false,
        reverted:             true,
        reverted_at_ns:       1783728000000000000n,           // Option::Some
        total_received_cents: 0,
        story_cid:            "bafybeexamplecidB",            // Option::Some
        story_hash:           "a".repeat(64),                  // Option::Some
    },
    direct_blessings: {
        total_cents: 1500,
        donor_count: 3,
    },
    outcome:          "RevertedToChalice",
    rollover_cents:   20210,
    ops_ledger_entry: {
        entry_type:          "offering_income",
        amount_cents:        8358,
        balance_after_cents: 17958,
        party:               "ops",
        description:         "Divine Offering",
        at_ns:               1782518400000000000n,
    },
    ledger: [
        {
            entry_type:          "soul_blessing_base",
            amount_cents:        10400,
            balance_after_cents: 10400,
            party:               "soul#2",
            description:         "base",
            at_ns:               1782518400000000000n,
        },
        {
            entry_type:          "blessing_reverted",
            amount_cents:        -10400,        // negative i64
            balance_after_cents: 0,
            party:               "soul#2",
            description:         "unclaimed; returned (net of what soul held)",
            at_ns:               1783728000000000000n,
        },
    ],
    generated_at_ns:  1785110400000000000n,
};

// ============================================================================
// REGRESSION CHECK — cbor-x encodes BigInt as full 8-byte form regardless of
// value, which is NOT canonical CBOR. We work around it by using plain JS
// Numbers for values that fit (≤ 2^53 - 1) and BigInt only for the timestamp
// fields. This block re-confirms the workaround is still needed every run —
// if cbor-x ever fixes the bug upstream, this output will change and we can
// drop the workaround.
// ============================================================================

function checkBigIntWorkaroundStillNeeded() {
    const bigOne = encoder.encode(1n);
    const numOne = encoder.encode(1);
    const bigHex = Buffer.from(bigOne).toString('hex');
    const numHex = Buffer.from(numOne).toString('hex');
    console.log(`BigInt 1n -> ${bigHex}  (canonical: 01)`);
    console.log(`Number 1  -> ${numHex}  (canonical: 01)`);
    if (bigHex === '01') {
        console.log("⚠️  cbor-x now encodes BigInt canonically. The Number-coercion");
        console.log("   workaround in vector data above could be removed. Revisit.");
    }
    console.log();
}

// ============================================================================
// Per-vector verification
// ============================================================================

function verify(name, input, expectedHash) {
    const encoded     = encoder.encode(input);
    const computed    = createHash('sha256').update(encoded).digest('hex');
    const match       = computed === expectedHash;
    const status      = match ? '✅' : '❌';

    console.log(`${status} ${name}`);
    console.log(`   Expected: ${expectedHash}`);
    console.log(`   Computed: ${computed}`);
    console.log(`   CBOR bytes: ${encoded.length}, first 32 hex: ${Buffer.from(encoded).slice(0, 32).toString('hex')}`);
    console.log();

    return match;
}

// ============================================================================
// Run
// ============================================================================

console.log("================================================================");
console.log("  hopeandgrace canonical CBOR round-trip — 3 vectors");
console.log("================================================================");
console.log();

checkBigIntWorkaroundStillNeeded();

const r1 = verify('Ceremony #1   (Claimed, None story, positive+negative ledger)', ceremony1Input, EXPECTED_HASH_CEREMONY_1);
const r2 = verify('Vector A      (Claimed, None story, all-positive multi-ledger)', vectorA,         EXPECTED_HASH_VECTOR_A);
const r3 = verify('Vector B v2   (RevertedToChalice, Some story, negative ledger)', vectorB,         EXPECTED_HASH_VECTOR_B);

const total  = 3;
const passed = [r1, r2, r3].filter(Boolean).length;

console.log("================================================================");
if (passed === total) {
    console.log(`✅ ALL ${total} VECTORS MATCH — canonical-encoding contract proven across the full divergence surface.`);
    console.log();
    console.log("Divergence points covered:");
    console.log("  • Option::None and Option::Some (story_cid, story_hash, reverted_at_ns)");
    console.log("  • Both CeremonyOutcome variants (Claimed, RevertedToChalice)");
    console.log("  • Negative amount_cents (CBOR major type 1)");
    console.log("  • Multi-entry ledger Vec (CBOR array)");
    console.log("  • BigInt timestamps (full 8-byte nat64 encoding)");
    console.log("  • Number small values (minimum-length integer encoding)");
    console.log();
    console.log("Rust ciborium and Node cbor-x produce byte-equal canonical CBOR for");
    console.log("all three vectors. The cross-language contract holds for the production");
    console.log("archive path. Mainnet deploy can proceed once funded.");
    process.exit(0);
} else {
    console.log(`❌ ${passed}/${total} vectors matched. ${total - passed} regression(s).`);
    console.log();
    console.log("This means something silently changed canonical CBOR output —");
    console.log("possibly a ciborium or cbor-x version bump, a field reorder in");
    console.log("SettlementRecordInput, or a serde derive change. Investigate before");
    console.log("shipping anything new. The hash-mismatch diagnostics above show");
    console.log("where the divergence is.");
    process.exit(1);
}