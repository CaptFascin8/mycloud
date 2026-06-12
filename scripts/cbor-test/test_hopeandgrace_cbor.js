// scripts/cbor-test/test_hopeandgrace_cbor.js
//
// CROSS-LANGUAGE CBOR ROUND-TRIP TEST — the canonical-encoding contract.
//
// Background: the hopeandgrace canister computes content_hash as
//     sha256(canonical_cbor(SettlementRecordInput))
// where canonical_cbor is produced by the Rust `ciborium` crate.
//
// For "don't trust us, verify us" to be real, anyone else must be able
// to reproduce that hash from the same data. This script proves that's
// possible from Node — meaning H&G's Node-side archive job AND any
// independent auditor in any language can verify our records.
//
// The test:
//   1. Build the exact same SettlementRecordInput we sent to the canister
//      as ceremony #1 in scripts/test_hopeandgrace.sh
//   2. CBOR-encode it via cbor-x with map-style records and field-order
//      preserved (matching ciborium's struct-as-map behavior)
//   3. sha256-hash the bytes, hex-encode lowercase
//   4. Assert it equals the canister's content_hash
//
// If those agree, the canonical-encoding contract holds across Rust and Node.
// If they don't, this script's diagnostic output points at the byte where
// they diverge so we can fix the config on one side.
//
// Run: cd scripts/cbor-test && npm install && node test_hopeandgrace_cbor.js
// Or:  bash scripts/test_hopeandgrace_cbor.sh

import { Encoder } from 'cbor-x';
import { createHash } from 'node:crypto';

// ---------------------------------------------------------------------------
// EXPECTED HASH — captured from the canister on Pass 4B integration test.
// ---------------------------------------------------------------------------
// This is the content_hash that hopeandgrace.archive_ceremony returned for
// ceremony #1's input. If we change the sample data below, this must be
// re-captured by running scripts/test_hopeandgrace.sh and reading the
// content_hash from the "archive_ceremony #1" output.

const EXPECTED_HASH = "0c8eefd7d9ba7b3124e42a448619ecc65b9d5fc9bbe240cabe99d196c4f63e8a";

// ---------------------------------------------------------------------------
// SAMPLE INPUT — must match scripts/test_hopeandgrace.sh ceremony #1 exactly
// ---------------------------------------------------------------------------
// CRITICAL: field declaration order matters. cbor-x will encode keys in
// the order they appear in the object literal below; ciborium encodes
// struct fields in declaration order. The two orderings must match.
//
// Field types and order MUST match the Rust SettlementRecordInput struct
// in backend/hopeandgrace/src/lib.rs — that struct's declaration order
// IS the canonical encoding contract.

const sampleInput = {
    record_version:   1,
    ceremony_number:  1,                  // small, fits in Number (canonical short form)
    ceremony_date:    "2026-06-09",
    random_seed:      "seed-001",
    pool_total_cents: 100000,             // small, fits in Number
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
        reverted_at_ns:       null,       // Option::None in Rust
         total_received_cents: 40000,
        story_cid:            null,       // Option::None
        story_hash:           null,       // Option::None
    },
    direct_blessings: {
        total_cents: 0,
        donor_count: 0,
    },
    outcome:          "Claimed",          // CeremonyOutcome enum variant
    rollover_cents:   0,
    ops_ledger_entry: {
        entry_type:          "divine_offering",
        amount_cents:        20000,        // small, fits in Number
        balance_after_cents: 80000,
        party:               "ops",
        description:         "20% divine offering",
        at_ns:               1700000000000000000n,  // BigInt: exceeds 2^53, needs 8-byte form
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
            amount_cents:        -40000,    // negative, small, fits in Number
            balance_after_cents: 60000,
            party:               "soul#soul-001",
            description:         "soul base blessing",
            at_ns:               1700000000000000001n,
        },
    ],
    generated_at_ns:  1700000000500000000n,
};

// ---------------------------------------------------------------------------
// Encoding
// ---------------------------------------------------------------------------
// cbor-x configuration to match ciborium's output:
//   - mapsAsObjects: false ... no, we WANT object-as-map (default)
//   - useRecords: false ........ no, also default; records are a cbor-x
//                                extension that ciborium doesn't understand
//   - variableMapSize: true .... yes, ciborium emits variable-length maps
//                                (major type 5 with variable length marker)
//   - tagUint8Array: false ..... we don't have any binary fields
//
// What we're betting on: cbor-x's default object encoding (variable-length
// map, keys in object-property order, no record tags) matches ciborium's
// default struct encoding.

const encoder = new Encoder({
    useRecords:      false,   // emit as CBOR map, not cbor-x's record table
    variableMapSize: true,    // matches ciborium's variable-length encoding
    largeBigIntToFloat: false,
    useFloat32:      0,       // never use float32 (we have no floats anyway)
});

const encodedBytes = encoder.encode(sampleInput);

// ---------------------------------------------------------------------------
// Hashing
// ---------------------------------------------------------------------------

const computedHash = createHash('sha256').update(encodedBytes).digest('hex');

// ---------------------------------------------------------------------------
// Report
// ---------------------------------------------------------------------------

console.log("================================================================");
console.log("  hopeandgrace canonical CBOR round-trip test");
console.log("================================================================");
console.log();
console.log("Expected (from canister):  " + EXPECTED_HASH);
console.log("Computed (from this Node): " + computedHash);
console.log();
console.log(`CBOR encoded length: ${encodedBytes.length} bytes`);
console.log("First 64 bytes (hex): " + Buffer.from(encodedBytes).slice(0, 64).toString('hex'));

// REGRESSION CHECK: cbor-x encodes BigInt as full 8-byte form regardless
// of value, which is NOT canonical CBOR. We work around it by using plain
// JS Numbers for values that fit (≤ 2^53 - 1) and BigInt only for the
// timestamp fields. This block re-confirms the workaround is still needed
// every run — if cbor-x ever fixes the bug upstream, this output will
// change and we can drop the workaround.
{
    const bigOne = encoder.encode(1n);
    const numOne = encoder.encode(1);
    console.log(`BigInt 1n -> ${Buffer.from(bigOne).toString('hex')}  (canonical: 01)`);
    console.log(`Number 1 -> ${Buffer.from(numOne).toString('hex')}  (canonical: 01)`);
    if (Buffer.from(bigOne).toString('hex') === '01') {
        console.log("⚠️  cbor-x now encodes BigInt canonically. The Number-coercion");
        console.log("   workaround in sampleInput could be removed. Revisit.");
    }
    console.log();
}

console.log();

if (computedHash === EXPECTED_HASH) {
    console.log("✅ MATCH — canonical-encoding contract holds.");
    console.log();
    console.log("This means:");
    console.log("  * The Rust canister and Node clients agree on canonical");
    console.log("    CBOR byte representation for SettlementRecordInput.");
    console.log("  * H&G's archive job can pre-compute content_hash for any");
    console.log("    record and verify it matches what the canister returns.");
    console.log("  * Independent auditors in any CBOR-supporting language");
    console.log("    can verify records on-chain.");
    process.exit(0);
} else {
    console.log("❌ MISMATCH — canonical-encoding contract is broken.");
    console.log();
    console.log("Likely causes (in order of probability):");
    console.log("  1. Field-order mismatch between this script and the Rust");
    console.log("     struct. Verify scripts/cbor-test/test_hopeandgrace_cbor.js");
    console.log("     declares fields in the same order as backend/hopeandgrace");
    console.log("     /src/lib.rs's SettlementRecordInput struct.");
    console.log();
    console.log("  2. cbor-x encoding option mismatch. Most common: ciborium");
    console.log("     uses variable-length CBOR maps; if cbor-x is using a");
    console.log("     'records' / 'tag 105' style, that's a different encoding.");
    console.log("     The `useRecords: false` option above prevents that.");
    console.log();
    console.log("  3. Enum representation. ciborium encodes a Rust enum unit");
    console.log("     variant as a CBOR text string ('Claimed') — confirmed in");
    console.log("     Pass 2C unit tests. If cbor-x is encoding the string");
    console.log("     'Claimed' the same way, this dimension is fine.");
    console.log();
    console.log("  4. Option encoding. Rust's Option::None is CBOR null (0xf6);");
    console.log("     Option::Some(T) is just T's encoding (ciborium 'untagged'");
    console.log("     representation). null in JS / Node should also encode to");
    console.log("     0xf6 in cbor-x by default.");
    console.log();
    console.log("  5. Numeric types — verify BigInts are encoded as CBOR major");
    console.log("     type 0/1 (unsigned/negative integers), not floats.");
    console.log();
    console.log("Diagnostic: dump the encoded bytes and compare side-by-side");
    console.log("with what ciborium produces. Add this temporary check to the");
    console.log("Rust canister to print bytes during archive_ceremony, then");
    console.log("compare hex side by side with the output above.");
    process.exit(1);
}