'use strict';
// ============================================================================
//  idl.js — @dfinity/candid interface for the hopeandgrace canister.
//
//  ⚠️  THIS IS A STAND-IN for local development only.
//  Once the canister is deployed, run `dfx generate hopeandgrace` and import the
//  GENERATED idlFactory from src/declarations/hopeandgrace instead — that one is
//  guaranteed to match the deployed .did byte-for-byte. Replace the require in
//  actor.js with the generated factory and delete this file.
//
//  Field order here mirrors the locked spec. SettlementRecordInput is the
//  stored SettlementRecord MINUS content_hash and archived_at_ns (canister-
//  populated). generated_at_ns is included as H&G-supplied — confirm against
//  the final .did and remove from both this file and convert.js if it moves
//  canister-side.
// ============================================================================

const idlFactory = ({ IDL }) => {
  const CeremonyOutcome = IDL.Variant({
    Claimed: IDL.Null,
    RevertedToChalice: IDL.Null,
    Pending: IDL.Null,
  });

  const LedgerEntry = IDL.Record({
    entry_type: IDL.Text,
    amount_cents: IDL.Int64,
    balance_after_cents: IDL.Nat64,
    party: IDL.Text,
    description: IDL.Text,
    at_ns: IDL.Nat64,
  });

  const CeremonySplit = IDL.Record({
    soul_base_cents: IDL.Nat64,
    angel_gross_cents: IDL.Nat64,
    divine_offering_cents: IDL.Nat64,
    divine_offering_bps: IDL.Nat32,
  });

  const AngelOutcome = IDL.Record({
    uuid: IDL.Text,
    claimed: IDL.Bool,
    donated_bps: IDL.Nat32,
    donated_cents: IDL.Nat64,
    kept_cents: IDL.Nat64,
  });

  const SoulOutcome = IDL.Record({
    uuid: IDL.Text,
    engaged: IDL.Bool,
    reverted: IDL.Bool,
    reverted_at_ns: IDL.Opt(IDL.Nat64),
    total_received_cents: IDL.Nat64,
    story_cid: IDL.Opt(IDL.Text),
    story_hash: IDL.Opt(IDL.Text),
  });

  const DirectBlessings = IDL.Record({
    total_cents: IDL.Nat64,
    donor_count: IDL.Nat64,
  });

  // What H&G SENDS. No content_hash, no archived_at_ns.
  const SettlementRecordInput = IDL.Record({
    record_version: IDL.Nat32,
    ceremony_number: IDL.Nat64,
    ceremony_date: IDL.Text,
    random_seed: IDL.Text,
    pool_total_cents: IDL.Nat64,
    split: CeremonySplit,
    angel: AngelOutcome,
    soul: SoulOutcome,
    direct_blessings: DirectBlessings,
    outcome: CeremonyOutcome,
    rollover_cents: IDL.Nat64,
    ops_ledger_entry: LedgerEntry,
    ledger: IDL.Vec(LedgerEntry),
    generated_at_ns: IDL.Nat64,
  });

  const RecordRef = IDL.Record({
    ceremony_number: IDL.Nat64,
    content_hash: IDL.Text,
    archived_at_ns: IDL.Nat64,
  });

  const HopeAndGraceError = IDL.Variant({
    Unauthorized: IDL.Null,
    NotFound: IDL.Null,
    AlreadyArchived: IDL.Nat64,
    InvalidRecord: IDL.Text,
    AnonymousCaller: IDL.Null,
    InvariantViolated: IDL.Text,
  });

  const ArchiveResult = IDL.Variant({ Ok: RecordRef, Err: HopeAndGraceError });

  const LegalDoc = IDL.Record({
    kind: IDL.Text,
    version: IDL.Nat32,
    effective_date: IDL.Text,
    content_md: IDL.Text,
    content_hash: IDL.Text,
    published_at_ns: IDL.Nat64,
  });

  // The push side only needs the two write methods. For reconciliation reads
  // (get_ceremony / public_totals) import the GENERATED declarations once the
  // canister is deployed — they carry the full read types.
  return IDL.Service({
    archive_ceremony: IDL.Func([SettlementRecordInput], [ArchiveResult], []),
    put_legal_doc: IDL.Func([LegalDoc], [ArchiveResult], []),
  });
};

module.exports = { idlFactory };
