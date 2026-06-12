# Hope & Grace canonical CBOR round-trip test

This directory contains the **canonical-encoding contract** for the
`hopeandgrace` canister's `content_hash`.

## The problem this test solves

The hopeandgrace canister computes `content_hash` as
`sha256(canonical_cbor(SettlementRecordInput))` using Rust's `ciborium`
crate. For the "don't trust us, verify us" property to be real, anyone
else — H&G's Node backend, an auditor, a Ripples page reader using
@dfinity/agent — must be able to reproduce that exact hash from the
same data.

This test proves it's reproducible from Node, which is the language H&G's
archive job is written in. If the test passes, the canonical-encoding
contract holds across implementations. If it fails, we know our two
ecosystems disagree about CBOR encoding and we need to fix the config on
one side.

## How to run

From the project root:

```bash
bash scripts/test_hopeandgrace_cbor.sh
```

The wrapper auto-installs `cbor-x` on first run (~5 seconds), then
executes the test. Exit code 0 = pass; 1 = mismatch.

You can also run directly:

```bash
cd scripts/cbor-test
npm install   # first run only
npm test
```

## What it does

1. Builds the same `SettlementRecordInput` data as ceremony #1 from
   `scripts/test_hopeandgrace.sh`.
2. CBOR-encodes via `cbor-x` (Node-side standard library).
3. Computes sha256 of the bytes, lowercase hex.
4. Compares to the hardcoded expected hash from the canister.

## Maintenance

The expected hash is captured from a real canister execution. If you
ever change the sample data in `test_hopeandgrace_cbor.js`:

1. Update `scripts/test_hopeandgrace.sh` to match (these two files must
   describe the same ceremony #1).
2. Run `bash scripts/test_hopeandgrace.sh` against a clean dfx replica.
3. Read the new `content_hash` from the "archive_ceremony #1" output.
4. Update `EXPECTED_HASH` in `test_hopeandgrace_cbor.js` to match.

If the test starts failing without you changing anything, that means the
Rust ciborium or Node cbor-x library updated and changed encoding
behavior. That would be a serious regression — investigate before
shipping anything new.

## The field-order contract

The canonical encoding is sensitive to field order. The Rust struct
`SettlementRecordInput` in `backend/hopeandgrace/src/lib.rs` is the
authoritative declaration. The JS object in `test_hopeandgrace_cbor.js`
must declare fields in the same order. If they ever drift, this test
will catch it on the next run.