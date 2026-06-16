export const idlFactory = ({ IDL }) => {
  const HopeAndGraceError = IDL.Variant({
    'InvalidRecord' : IDL.Text,
    'AlreadyArchived' : IDL.Nat64,
    'NotFound' : IDL.Null,
    'InvariantViolated' : IDL.Text,
    'Unauthorized' : IDL.Null,
    'AnonymousCaller' : IDL.Null,
  });
  const DirectBlessings = IDL.Record({
    'total_cents' : IDL.Nat64,
    'donor_count' : IDL.Nat64,
  });
  const AngelOutcome = IDL.Record({
    'donated_bps' : IDL.Nat32,
    'uuid' : IDL.Text,
    'claimed' : IDL.Bool,
    'donated_cents' : IDL.Nat64,
    'kept_cents' : IDL.Nat64,
  });
  const SoulOutcome = IDL.Record({
    'story_cid' : IDL.Opt(IDL.Text),
    'uuid' : IDL.Text,
    'total_received_cents' : IDL.Nat64,
    'reverted' : IDL.Bool,
    'reverted_at_ns' : IDL.Opt(IDL.Nat64),
    'story_hash' : IDL.Opt(IDL.Text),
    'engaged' : IDL.Bool,
  });
  const CeremonySplit = IDL.Record({
    'divine_offering_bps' : IDL.Nat32,
    'angel_gross_cents' : IDL.Nat64,
    'divine_offering_cents' : IDL.Nat64,
    'soul_base_cents' : IDL.Nat64,
  });
  const LedgerEntry = IDL.Record({
    'at_ns' : IDL.Nat64,
    'balance_after_cents' : IDL.Nat64,
    'amount_cents' : IDL.Int64,
    'entry_type' : IDL.Text,
    'description' : IDL.Text,
    'party' : IDL.Text,
  });
  const CeremonyOutcome = IDL.Variant({
    'Claimed' : IDL.Null,
    'RevertedToChalice' : IDL.Null,
    'Pending' : IDL.Null,
  });
  const SettlementRecordInput = IDL.Record({
    'direct_blessings' : DirectBlessings,
    'angel' : AngelOutcome,
    'record_version' : IDL.Nat32,
    'soul' : SoulOutcome,
    'ceremony_number' : IDL.Nat64,
    'generated_at_ns' : IDL.Nat64,
    'split' : CeremonySplit,
    'ledger' : IDL.Vec(LedgerEntry),
    'pool_total_cents' : IDL.Nat64,
    'random_seed' : IDL.Text,
    'outcome' : CeremonyOutcome,
    'ops_ledger_entry' : LedgerEntry,
    'rollover_cents' : IDL.Nat64,
    'ceremony_date' : IDL.Text,
  });
  const RecordRef = IDL.Record({
    'content_hash' : IDL.Text,
    'ceremony_number' : IDL.Nat64,
    'archived_at_ns' : IDL.Nat64,
  });
  const SettlementRecord = IDL.Record({
    'direct_blessings' : DirectBlessings,
    'angel' : AngelOutcome,
    'record_version' : IDL.Nat32,
    'content_hash' : IDL.Text,
    'soul' : SoulOutcome,
    'ceremony_number' : IDL.Nat64,
    'generated_at_ns' : IDL.Nat64,
    'split' : CeremonySplit,
    'ledger' : IDL.Vec(LedgerEntry),
    'archived_at_ns' : IDL.Nat64,
    'pool_total_cents' : IDL.Nat64,
    'random_seed' : IDL.Text,
    'outcome' : CeremonyOutcome,
    'ops_ledger_entry' : LedgerEntry,
    'rollover_cents' : IDL.Nat64,
    'ceremony_date' : IDL.Text,
  });
  const LegalDoc = IDL.Record({
    'content_md' : IDL.Text,
    'kind' : IDL.Text,
    'content_hash' : IDL.Text,
    'published_at_ns' : IDL.Nat64,
    'version' : IDL.Nat32,
    'effective_date' : IDL.Text,
  });
  const HealthStatus = IDL.Record({
    'ok' : IDL.Bool,
    'timestamp_ns' : IDL.Nat64,
    'canister' : IDL.Text,
    'ceremony_count' : IDL.Nat64,
    'legal_doc_count' : IDL.Nat64,
  });
  const SettlementSummary = IDL.Record({
    'soul_received_cents' : IDL.Nat64,
    'content_hash' : IDL.Text,
    'ceremony_number' : IDL.Nat64,
    'has_story' : IDL.Bool,
    'pool_total_cents' : IDL.Nat64,
    'outcome' : CeremonyOutcome,
    'ceremony_date' : IDL.Text,
  });
  const LegalDocMeta = IDL.Record({
    'kind' : IDL.Text,
    'content_hash' : IDL.Text,
    'published_at_ns' : IDL.Nat64,
    'version' : IDL.Nat32,
    'effective_date' : IDL.Text,
  });
  const Totals = IDL.Record({
    'total_pool_cents' : IDL.Nat64,
    'ceremonies' : IDL.Nat64,
    'souls_blessed' : IDL.Nat64,
    'total_to_souls_cents' : IDL.Nat64,
    'angels_active' : IDL.Nat64,
    'total_divine_offering_cents' : IDL.Nat64,
    'total_direct_blessings_cents' : IDL.Nat64,
  });
  return IDL.Service({
    'add_writer' : IDL.Func(
        [IDL.Principal],
        [IDL.Variant({ 'Ok' : IDL.Null, 'Err' : HopeAndGraceError })],
        [],
      ),
    'archive_ceremony' : IDL.Func(
        [SettlementRecordInput],
        [IDL.Variant({ 'Ok' : RecordRef, 'Err' : HopeAndGraceError })],
        [],
      ),
    'get_ceremony' : IDL.Func(
        [IDL.Nat64],
        [IDL.Opt(SettlementRecord)],
        ['query'],
      ),
    'get_legal_doc' : IDL.Func([IDL.Text], [IDL.Opt(LegalDoc)], ['query']),
    'get_legal_doc_version' : IDL.Func(
        [IDL.Text, IDL.Nat32],
        [IDL.Opt(LegalDoc)],
        ['query'],
      ),
    'get_owner' : IDL.Func([], [IDL.Principal], ['query']),
    'get_pending_owner' : IDL.Func([], [IDL.Opt(IDL.Principal)], ['query']),
    'health_check' : IDL.Func([], [HealthStatus], ['query']),
    'is_writer' : IDL.Func([IDL.Principal], [IDL.Bool], ['query']),
    'list_ceremonies' : IDL.Func(
        [IDL.Nat64, IDL.Nat64],
        [IDL.Vec(SettlementSummary)],
        ['query'],
      ),
    'list_legal_doc_versions' : IDL.Func(
        [IDL.Text],
        [IDL.Vec(LegalDocMeta)],
        ['query'],
      ),
    'list_writers' : IDL.Func([], [IDL.Vec(IDL.Principal)], ['query']),
    'public_totals' : IDL.Func([], [Totals], ['query']),
    'put_legal_doc' : IDL.Func(
        [LegalDoc],
        [IDL.Variant({ 'Ok' : RecordRef, 'Err' : HopeAndGraceError })],
        [],
      ),
    'remove_writer' : IDL.Func(
        [IDL.Principal],
        [IDL.Variant({ 'Ok' : IDL.Null, 'Err' : HopeAndGraceError })],
        [],
      ),
    'set_owner_accept' : IDL.Func(
        [],
        [IDL.Variant({ 'Ok' : IDL.Null, 'Err' : HopeAndGraceError })],
        [],
      ),
    'set_owner_cancel' : IDL.Func(
        [],
        [IDL.Variant({ 'Ok' : IDL.Null, 'Err' : HopeAndGraceError })],
        [],
      ),
    'set_owner_initiate' : IDL.Func(
        [IDL.Principal],
        [IDL.Variant({ 'Ok' : IDL.Null, 'Err' : HopeAndGraceError })],
        [],
      ),
  });
};
export const init = ({ IDL }) => { return []; };
