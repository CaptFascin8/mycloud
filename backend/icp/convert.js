'use strict';
// ============================================================================
//  convert.js — map the H&G in-app record shape (dollars as floats, percent as
//  a number) into the canister's SettlementRecordInput (integer cents + basis
//  points, BigInt for nat64/int64, Number for nat32).
//
//  Source of truth for the *input* shape: engine.js buildSettlementRecord().
//  Source of truth for the *output* shape: hopeandgrace.did SettlementRecordInput.
//
//  LOCKED DESIGN (do not "fix" without bumping record_version):
//    • content_hash   — NEVER sent. The canister computes it (canonical CBOR).
//    • archived_at_ns — NEVER sent. The canister sets it at archive time.
//    • generated_at_ns — SENT by H&G (this is *our* generation time). If the
//      final .did puts this canister-side instead, delete the one line below
//      and the field in idl.js. (Flagged to mycloud Claude as a spec nit.)
//
//  Money rule: every dollar value -> Math.round(dollars * 100) cents.
//  Percent rule: every percent value -> Math.round(pct * 100) basis points.
// ============================================================================

/** Dollars (float) -> integer cents as BigInt (nat64 / int64). Sign preserved. */
function toCents(dollars) {
  const n = Number(dollars);
  if (!Number.isFinite(n)) throw new Error(`toCents: non-finite value ${dollars}`);
  return BigInt(Math.round(n * 100));
}

/** Like toCents but clamps to >= 0 — for nat64 fields that must never be negative. */
function toCentsNat(dollars) {
  const c = toCents(dollars);
  return c < 0n ? 0n : c;
}

/** Percent (e.g. 19.28) -> basis points as Number (nat32). 19.28 -> 1928. */
function toBps(pct) {
  const n = Number(pct);
  if (!Number.isFinite(n)) throw new Error(`toBps: non-finite value ${pct}`);
  return Math.round(n * 100);
}

/** Datetime/ISO/Date -> nanoseconds since unix epoch as BigInt (nat64). */
function toNs(when) {
  const ms = (when instanceof Date) ? when.getTime() : new Date(when).getTime();
  if (!Number.isFinite(ms)) throw new Error(`toNs: unparseable timestamp ${when}`);
  return BigInt(ms) * 1_000_000n;
}

/** Normalise a DB date/Date to an ISO "YYYY-MM-DD" string. */
function toIsoDate(d) {
  if (d == null) return '';
  if (d instanceof Date) return d.toISOString().slice(0, 10);
  const s = String(d);
  // already "YYYY-MM-DD..." -> take the date part; else parse.
  if (/^\d{4}-\d{2}-\d{2}/.test(s)) return s.slice(0, 10);
  const parsed = new Date(s);
  return Number.isFinite(parsed.getTime()) ? parsed.toISOString().slice(0, 10) : s;
}

/** Candid opt helpers: [] = None, [v] = Some(v). */
const optNat64 = (v) => (v == null ? [] : [toNs(v)]);
const optText  = (v) => (v == null || v === '' ? [] : [String(v)]);

/** Map H&G outcome string -> Candid CeremonyOutcome variant. */
function toOutcomeVariant(outcome) {
  switch (outcome) {
    case 'claimed':             return { Claimed: null };
    case 'reverted_to_chalice': return { RevertedToChalice: null };
    case 'pending':             return { Pending: null };
    default: throw new Error(`unknown outcome "${outcome}"`);
  }
}

/** Map one H&G ledger entry -> Candid LedgerEntry. */
function toLedgerEntry(t) {
  return {
    entry_type:          String(t.type),
    amount_cents:        toCents(t.amount),          // int64, may be negative
    balance_after_cents: toCentsNat(t.balance_after),// nat64
    party:               String(t.party),
    description:         String(t.description || ''),
    at_ns:               toNs(t.at),
  };
}

/**
 * Build the canister SettlementRecordInput from a buildSettlementRecord() result.
 * @param {object} rec   the H&G record (engine.js buildSettlementRecord output)
 * @param {object} [story]  optional { story_cid, story_hash } from the IPFS pin step
 * @returns {object} SettlementRecordInput ready for the actor call
 */
function toSettlementRecordInput(rec, story = {}) {
  if (!rec) throw new Error('toSettlementRecordInput: null record');
  if (rec.outcome === 'pending') {
    throw new Error(`refusing to archive a pending ceremony #${rec.ceremony_number}`);
  }
  // ops_ledger_entry is non-optional in SettlementRecord. A settled ceremony
  // always took its Divine Offering at draw time, so this should exist. If it
  // doesn't, that's a data-integrity problem worth surfacing loudly, not zero-filling.
  if (!rec.ops_ledger_entry) {
    throw new Error(`ceremony #${rec.ceremony_number} has no offering_income ops entry; refusing to archive`);
  }

  return {
    record_version:   Number(rec.record_version) || 1,        // nat32
    ceremony_number:  BigInt(rec.ceremony_number),            // nat64
    ceremony_date:    toIsoDate(rec.ceremony_date),           // text
    random_seed:      String(rec.random_seed || ''),          // text
    pool_total_cents: toCentsNat(rec.pool_total),             // nat64
    split: {
      soul_base_cents:       toCentsNat(rec.split.soul_base),
      angel_gross_cents:     toCentsNat(rec.split.angel_gross),
      divine_offering_cents: toCentsNat(rec.split.divine_offering),
      divine_offering_bps:   toBps(rec.split.divine_offering_pct),     // nat32
    },
    angel: {
      uuid:          String(rec.angel.uuid || ''),
      claimed:       !!rec.angel.claimed,
      donated_bps:   toBps(rec.angel.donated_pct),            // nat32
      donated_cents: toCentsNat(rec.angel.donated_amt),
      kept_cents:    toCentsNat(rec.angel.kept),
    },
    soul: {
      uuid:                 String(rec.soul.uuid || ''),
      engaged:              !!rec.soul.engaged,
      reverted:             !!rec.soul.reverted,
      reverted_at_ns:       optNat64(rec.soul.reverted_at),   // opt nat64
      total_received_cents: toCentsNat(rec.soul.total_received),
      story_cid:            optText(story.story_cid),         // opt text
      story_hash:           optText(story.story_hash),        // opt text
    },
    direct_blessings: {
      total_cents: toCentsNat(rec.direct_blessings.total),
      donor_count: BigInt(rec.direct_blessings.donor_count || 0), // nat64
    },
    outcome:        toOutcomeVariant(rec.outcome),            // variant
    rollover_cents: toCentsNat(rec.rollover_amount),
    ops_ledger_entry: toLedgerEntry({
      type: rec.ops_ledger_entry.type || 'offering_income',
      amount: rec.ops_ledger_entry.amount,
      balance_after: rec.ops_ledger_entry.balance_after,
      party: rec.ops_ledger_entry.party || 'ops',
      description: rec.ops_ledger_entry.description || 'Divine Offering',
      at: rec.ops_ledger_entry.at,
    }),
    ledger: (rec.ledger || []).map(toLedgerEntry),           // vec LedgerEntry
    // SEE LOCKED DESIGN note at top — delete next line if final .did sets it canister-side:
    generated_at_ns: toNs(rec.generated_at),                 // nat64
  };
}

module.exports = {
  toCents, toCentsNat, toBps, toNs, toIsoDate,
  optNat64, optText, toOutcomeVariant, toLedgerEntry,
  toSettlementRecordInput,
};
