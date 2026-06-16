import type { Principal } from '@icp-sdk/core/principal';
import type { ActorMethod } from '@icp-sdk/core/agent';
import type { IDL } from '@icp-sdk/core/candid';

export interface AngelOutcome {
  'donated_bps' : number,
  'uuid' : string,
  'claimed' : boolean,
  'donated_cents' : bigint,
  'kept_cents' : bigint,
}
export interface CanisterConfig {
  'owner' : Principal,
  'writers' : Array<Principal>,
  'pending_owner' : [] | [Principal],
}
export type CeremonyOutcome = { 'Claimed' : null } |
  { 'RevertedToChalice' : null } |
  { 'Pending' : null };
export interface CeremonySplit {
  'divine_offering_bps' : number,
  'angel_gross_cents' : bigint,
  'divine_offering_cents' : bigint,
  'soul_base_cents' : bigint,
}
export interface DirectBlessings {
  'total_cents' : bigint,
  'donor_count' : bigint,
}
export interface HealthStatus {
  'ok' : boolean,
  'timestamp_ns' : bigint,
  'canister' : string,
  'ceremony_count' : bigint,
  'legal_doc_count' : bigint,
}
export type HopeAndGraceError = { 'InvalidRecord' : string } |
  { 'AlreadyArchived' : bigint } |
  { 'NotFound' : null } |
  { 'InvariantViolated' : string } |
  { 'Unauthorized' : null } |
  { 'AnonymousCaller' : null };
export interface LedgerEntry {
  'at_ns' : bigint,
  'balance_after_cents' : bigint,
  'amount_cents' : bigint,
  'entry_type' : string,
  'description' : string,
  'party' : string,
}
export interface LegalDoc {
  'content_md' : string,
  'kind' : string,
  'content_hash' : string,
  'published_at_ns' : bigint,
  'version' : number,
  'effective_date' : string,
}
export interface LegalDocMeta {
  'kind' : string,
  'content_hash' : string,
  'published_at_ns' : bigint,
  'version' : number,
  'effective_date' : string,
}
export interface RecordRef {
  'content_hash' : string,
  'ceremony_number' : bigint,
  'archived_at_ns' : bigint,
}
/**
 * What gets stored and returned. Input + content_hash + archived_at_ns.
 */
export interface SettlementRecord {
  'direct_blessings' : DirectBlessings,
  'angel' : AngelOutcome,
  'record_version' : number,
  'content_hash' : string,
  'soul' : SoulOutcome,
  'ceremony_number' : bigint,
  'generated_at_ns' : bigint,
  'split' : CeremonySplit,
  'ledger' : Array<LedgerEntry>,
  'archived_at_ns' : bigint,
  'pool_total_cents' : bigint,
  'random_seed' : string,
  'outcome' : CeremonyOutcome,
  'ops_ledger_entry' : LedgerEntry,
  'rollover_cents' : bigint,
  'ceremony_date' : string,
}
/**
 * What H&G sends. Field order = canonical encoding contract.
 * NO content_hash, NO archived_at_ns — those are canister-populated.
 */
export interface SettlementRecordInput {
  'direct_blessings' : DirectBlessings,
  'angel' : AngelOutcome,
  'record_version' : number,
  'soul' : SoulOutcome,
  'ceremony_number' : bigint,
  'generated_at_ns' : bigint,
  'split' : CeremonySplit,
  'ledger' : Array<LedgerEntry>,
  'pool_total_cents' : bigint,
  'random_seed' : string,
  'outcome' : CeremonyOutcome,
  'ops_ledger_entry' : LedgerEntry,
  'rollover_cents' : bigint,
  'ceremony_date' : string,
}
export interface SettlementSummary {
  'soul_received_cents' : bigint,
  'content_hash' : string,
  'ceremony_number' : bigint,
  'has_story' : boolean,
  'pool_total_cents' : bigint,
  'outcome' : CeremonyOutcome,
  'ceremony_date' : string,
}
export interface SoulOutcome {
  'story_cid' : [] | [string],
  'uuid' : string,
  'total_received_cents' : bigint,
  'reverted' : boolean,
  'reverted_at_ns' : [] | [bigint],
  'story_hash' : [] | [string],
  'engaged' : boolean,
}
export interface Totals {
  'total_pool_cents' : bigint,
  'ceremonies' : bigint,
  'souls_blessed' : bigint,
  'total_to_souls_cents' : bigint,
  'angels_active' : bigint,
  'total_divine_offering_cents' : bigint,
  'total_direct_blessings_cents' : bigint,
}
export interface _SERVICE {
  'add_writer' : ActorMethod<
    [Principal],
    { 'Ok' : null } |
      { 'Err' : HopeAndGraceError }
  >,
  /**
   * ----- Write methods (restricted to authorized writers) -----
   */
  'archive_ceremony' : ActorMethod<
    [SettlementRecordInput],
    { 'Ok' : RecordRef } |
      { 'Err' : HopeAndGraceError }
  >,
  /**
   * ----- Read methods (public queries) -----
   */
  'get_ceremony' : ActorMethod<[bigint], [] | [SettlementRecord]>,
  'get_legal_doc' : ActorMethod<[string], [] | [LegalDoc]>,
  'get_legal_doc_version' : ActorMethod<[string, number], [] | [LegalDoc]>,
  'get_owner' : ActorMethod<[], Principal>,
  'get_pending_owner' : ActorMethod<[], [] | [Principal]>,
  /**
   * ----- Health -----
   */
  'health_check' : ActorMethod<[], HealthStatus>,
  'is_writer' : ActorMethod<[Principal], boolean>,
  'list_ceremonies' : ActorMethod<[bigint, bigint], Array<SettlementSummary>>,
  'list_legal_doc_versions' : ActorMethod<[string], Array<LegalDocMeta>>,
  'list_writers' : ActorMethod<[], Array<Principal>>,
  'public_totals' : ActorMethod<[], Totals>,
  'put_legal_doc' : ActorMethod<
    [LegalDoc],
    { 'Ok' : RecordRef } |
      { 'Err' : HopeAndGraceError }
  >,
  'remove_writer' : ActorMethod<
    [Principal],
    { 'Ok' : null } |
      { 'Err' : HopeAndGraceError }
  >,
  'set_owner_accept' : ActorMethod<
    [],
    { 'Ok' : null } |
      { 'Err' : HopeAndGraceError }
  >,
  'set_owner_cancel' : ActorMethod<
    [],
    { 'Ok' : null } |
      { 'Err' : HopeAndGraceError }
  >,
  /**
   * ----- Access control (owner-only writes, public queries) -----
   */
  'set_owner_initiate' : ActorMethod<
    [Principal],
    { 'Ok' : null } |
      { 'Err' : HopeAndGraceError }
  >,
}
export declare const idlFactory: IDL.InterfaceFactory;
export declare const init: (args: { IDL: typeof IDL }) => IDL.Type[];
