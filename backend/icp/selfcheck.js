'use strict';
// ============================================================================
//  selfcheck.js — offline (no network, no DB) validation of convert.js, and
//  emitter of the shared test vectors for the cross-language round-trip test.
//
//  Run: node backend/icp/selfcheck.js
//
//  The vectors below deliberately hit the CBOR cross-language divergence points:
//    • Option::None  (vector A: no story)   and  Option::Some (vector B: story)
//    • both CeremonyOutcome variants (Claimed / RevertedToChalice)
//    • a NEGATIVE amount_cents in the ledger (a reversion debit)
//    • a populated multi-entry ledger vec
//  Hand the emitted JSON to mycloud Claude so the Rust round-trip test hashes
//  the SAME values — that's the executable contract.
// ============================================================================

const assert = require('assert');
const C = require('./convert');

function bigintReplacer(_k, v) { return typeof v === 'bigint' ? v.toString() + 'n' : v; }
let failures = 0;
function check(name, fn) {
  try { fn(); console.log('  ✓ ' + name); }
  catch (e) { failures++; console.error('  ✗ ' + name + ' — ' + e.message); }
}

console.log('convert.js unit checks:');
check('toCents rounds float-safe (313.73 -> 31373n)', () => assert.strictEqual(C.toCents(313.73), 31373n));
check('toCents preserves sign (-312 -> -31200n)', () => assert.strictEqual(C.toCents(-312), -31200n));
check('toCentsNat clamps negatives to 0', () => assert.strictEqual(C.toCentsNat(-5), 0n));
check('toBps (20 -> 2000)', () => assert.strictEqual(C.toBps(20), 2000));
check('toBps (19.28 -> 1928)', () => assert.strictEqual(C.toBps(19.28), 1928));
check('optNat64(null) -> []', () => assert.deepStrictEqual(C.optNat64(null), []));
check('optText("") -> []', () => assert.deepStrictEqual(C.optText(''), []));
check('outcome variant (claimed)', () => assert.deepStrictEqual(C.toOutcomeVariant('claimed'), { Claimed: null }));

// ---- Vector A: claimed, NO story (Option::None), positive ledger ----
const recA = {
  record_version: 1, ceremony_number: 12, ceremony_date: '2026-06-20',
  random_seed: 'seed-abc-123', pool_total: 600,
  split: { soul_base: 120, angel_gross: 384, divine_offering: 96, divine_offering_pct: 20 },
  angel: { uuid: 'angel-uuid-1', claimed: true, donated_pct: 50, donated_amt: 192, kept: 192 },
  soul: { uuid: 'soul-uuid-1', engaged: true, reverted: false, reverted_at: null, total_received: 312 },
  direct_blessings: { total: 0, donor_count: 0 },
  outcome: 'claimed', rollover_amount: 0,
  ops_ledger_entry: { amount: 96, balance_after: 96, at: '2026-06-20T00:00:00.000Z' },
  ledger: [
    { type: 'soul_blessing_base', amount: 120, balance_after: 120, party: 'soul#1', description: 'base', at: '2026-06-20T00:00:00.000Z' },
    { type: 'angel_gift',         amount: 192, balance_after: 312, party: 'soul#1', description: 'gift', at: '2026-06-27T00:00:00.000Z' },
  ],
  generated_at: '2026-07-20T00:00:00.000Z',
};

// ---- Vector B: reverted, WITH story (Option::Some), NEGATIVE ledger entry ----
const recB = {
  record_version: 1, ceremony_number: 13, ceremony_date: '2026-06-27',
  random_seed: 'seed-def-456', pool_total: 520,
  split: { soul_base: 104, angel_gross: 332.42, divine_offering: 83.58, divine_offering_pct: 19.28 },
  angel: { uuid: 'angel-uuid-2', claimed: true, donated_pct: 25, donated_amt: 83.10, kept: 249.32 },
  soul: { uuid: 'soul-uuid-2', engaged: false, reverted: true, reverted_at: '2026-07-11T00:00:00.000Z', total_received: 0 },
  direct_blessings: { total: 15.00, donor_count: 3 },
  outcome: 'reverted_to_chalice', rollover_amount: 202.10,
  ops_ledger_entry: { amount: 83.58, balance_after: 179.58, at: '2026-06-27T00:00:00.000Z' },
  ledger: [
    { type: 'soul_blessing_base', amount: 104,     balance_after: 104, party: 'soul#2', description: 'base', at: '2026-06-27T00:00:00.000Z' },
    { type: 'blessing_reverted',  amount: -187.10,  balance_after: 0,   party: 'soul#2', description: 'unclaimed; returned', at: '2026-07-11T00:00:00.000Z' },
  ],
  generated_at: '2026-07-27T00:00:00.000Z',
};

const inA = C.toSettlementRecordInput(recA, {});
const inB = C.toSettlementRecordInput(recB, { story_cid: 'bafybeexamplecidB', story_hash: 'a'.repeat(64) });

console.log('\nSettlementRecordInput checks:');
check('A: no content_hash field', () => assert.ok(!('content_hash' in inA)));
check('A: no archived_at_ns field', () => assert.ok(!('archived_at_ns' in inA)));
check('A: pool conservation in cents', () =>
  assert.strictEqual(inA.split.soul_base_cents + inA.split.angel_gross_cents + inA.split.divine_offering_cents, inA.pool_total_cents));
check('A: story None -> []', () => assert.deepStrictEqual(inA.soul.story_cid, []));
check('A: outcome Claimed', () => assert.deepStrictEqual(inA.outcome, { Claimed: null }));
check('A: divine_offering_bps = 2000', () => assert.strictEqual(inA.split.divine_offering_bps, 2000));

check('B: story Some -> [cid]', () => assert.deepStrictEqual(inB.soul.story_cid, ['bafybeexamplecidB']));
check('B: reverted_at Some (opt nat64 len 1)', () => assert.strictEqual(inB.soul.reverted_at_ns.length, 1));
check('B: outcome RevertedToChalice', () => assert.deepStrictEqual(inB.outcome, { RevertedToChalice: null }));
check('B: negative ledger amount preserved', () => {
  const rev = inB.ledger.find((e) => e.entry_type === 'blessing_reverted');
  assert.ok(rev.amount_cents < 0n, 'expected negative amount_cents');
  assert.strictEqual(rev.amount_cents, -18710n);
});
check('B: divine_offering_bps = 1928', () => assert.strictEqual(inB.split.divine_offering_bps, 1928));
check('B: donor_count is BigInt', () => assert.strictEqual(typeof inB.direct_blessings.donor_count, 'bigint'));

console.log('\nEmitting shared test vectors (give these to mycloud Claude for the Rust round-trip test):');
console.log(JSON.stringify({ vectorA: inA, vectorB: inB }, bigintReplacer, 2));

if (failures) { console.error(`\n${failures} CHECK(S) FAILED`); process.exit(1); }
console.log('\nAll converter self-checks passed.');
