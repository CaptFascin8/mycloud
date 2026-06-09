//! MyCloud — `hopeandgrace` canister
//!
//! Immutable ledger of Hope & Grace blessing ceremonies + versioned legal
//! document registry. Powers the Ripples of Compassion public transparency
//! page on hopeandgrace.space.
//!
//! Privacy model: only anonymized data lives on-chain (UUIDs not names,
//! aggregate donor counts not identities). Story text lives on IPFS by CID
//! with content_hash on chain for integrity — supports right-to-be-forgotten
//! via IPFS unpinning without breaking the audit trail.
//!
//! See `docs/HOPEANDGRACE_INTEGRATION_SPEC.md` for the full design rationale.
//!
//! This file is Phase 1a (data model + storage scaffolding). Methods,
//! invariant validation, and access control land in Phase 1b.
//!
//! Stable storage layout:
//!   * Memory 0: BTreeMap<u64, SettlementRecord>   — ceremonies, keyed by ceremony_number
//!   * Memory 1: BTreeMap<LegalDocKey, LegalDoc>   — legal docs, keyed by (kind, version)
//!   * Memory 2: BTreeMap<u8, CanisterConfig>      — singleton owner + writers
//!   * Memory 3: BTreeMap<u8, u64>                 — singleton counters (reserved for future use)

use candid::{CandidType, Decode, Encode, Principal};
use ic_cdk::{init, post_upgrade, query, update};
use ic_stable_structures::memory_manager::{MemoryId, MemoryManager, VirtualMemory};
use ic_stable_structures::storable::Bound;
use ic_stable_structures::{DefaultMemoryImpl, StableBTreeMap, Storable};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::cell::RefCell;

// ---------------------------------------------------------------------------
// Money + percentages — integer representations only
// ---------------------------------------------------------------------------
//
// All monetary values are `nat64` cents on the wire and `u64` cents in Rust.
// All percentages are basis points (1 bps = 0.01%, so 2000 = 20.00%) as
// `nat32` / `u32`. Floats are never stored or compared on chain.
//
// The `i64` you'll see on LedgerEntry.amount_cents is signed because ledger
// entries can be debits (negative). The cumulative balance stays >= 0
// (enforced by invariants in Phase 1b), so balance_after_cents stays `u64`.

// ---------------------------------------------------------------------------
// Ceremony outcomes
// ---------------------------------------------------------------------------

#[derive(CandidType, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum CeremonyOutcome {
    /// Soul accepted the blessing; funds disbursed.
    Claimed,
    /// Soul declined or didn't respond in window; pool returned to chalice.
    RevertedToChalice,
    /// Should not appear in archived records (we only archive terminal
    /// states), but kept type-complete in case H&G sends one as a defensive
    /// audit trail entry.
    Pending,
}

// ---------------------------------------------------------------------------
// Ledger entries — the per-cent paper trail
// ---------------------------------------------------------------------------

#[derive(CandidType, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct LedgerEntry {
    /// Free-form entry type. Typical values: "soul_blessing_base",
    /// "angel_share_kept", "angel_share_donated", "divine_offering",
    /// "direct_blessing", "rollover_to_chalice". H&G owns this taxonomy.
    pub entry_type: String,

    /// Signed cents. Negative for debits from chalice, positive for credits
    /// to parties. The sum across all ledger entries for one ceremony must
    /// reconcile to zero (Phase 1b invariant).
    pub amount_cents: i64,

    /// Running balance of the chalice/pool after this entry, in cents.
    pub balance_after_cents: u64,

    /// Anonymized party identifier. Format suggestions:
    /// "soul#<uuid>", "angel#<uuid>", "ops", "chalice".
    pub party: String,

    /// Human-readable description for the public ledger view. Should NOT
    /// contain any PII; this string ends up on Ripples.
    pub description: String,

    /// Timestamp in nanoseconds since Unix epoch. Should be derived from the
    /// original ceremony time, not the archive time.
    pub at_ns: u64,
}

// ---------------------------------------------------------------------------
// SettlementRecord sub-structs
// ---------------------------------------------------------------------------

#[derive(CandidType, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct CeremonySplit {
    pub soul_base_cents:       u64,
    pub angel_gross_cents:     u64,
    pub divine_offering_cents: u64,
    /// Basis points of the pool that went to divine offering. 2000 = 20.00%.
    /// Kept separately for readability; could be derived from the cents
    /// fields, but storing it makes the Ripples page math easier to verify.
    pub divine_offering_bps:   u32,
}

#[derive(CandidType, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct AngelOutcome {
    /// Anonymized angel UUID (H&G generates and stores the mapping).
    pub uuid:          String,
    /// True if the angel accepted the ceremony invitation.
    pub claimed:       bool,
    /// Basis points of `angel_gross_cents` that the angel chose to donate.
    pub donated_bps:   u32,
    /// Cents donated (derived from donated_bps but stored for verifiability).
    pub donated_cents: u64,
    /// Cents the angel kept.
    pub kept_cents:    u64,
}

#[derive(CandidType, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct SoulOutcome {
    /// Anonymized soul UUID.
    pub uuid:                 String,
    /// Soul engaged with the ceremony (logged in, opened claim email, etc).
    pub engaged:              bool,
    /// Soul reverted/declined the blessing (overrides engaged for outcome).
    pub reverted:             bool,
    /// When reverted, the timestamp. None if not reverted.
    pub reverted_at_ns:       Option<u64>,
    /// Total cents soul received from the ceremony (base + direct blessings).
    pub total_received_cents: u64,
    /// IPFS CID of the story text. None if soul didn't grant share_permission.
    pub story_cid:            Option<String>,
    /// sha256 hex of the story bytes. None if story_cid is None.
    /// If story_cid is Some, story_hash must also be Some — Phase 1b invariant.
    pub story_hash:           Option<String>,
}

#[derive(CandidType, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct DirectBlessings {
    /// Total cents received in direct blessings during this ceremony.
    pub total_cents: u64,
    /// Aggregate count of donors only — no identities.
    pub donor_count: u64,
}

// ---------------------------------------------------------------------------
// SettlementRecordInput — what H&G's @dfinity/agent code sends
// ---------------------------------------------------------------------------
//
// Critical property: no content_hash, no archived_at_ns. Those are
// canister-populated. Making them structurally absent from the input
// means H&G's TypeScript client physically cannot send them — confusion
// you can't express is confusion that can't happen.
//
// FIELD ORDER IS THE CANONICAL ENCODING CONTRACT. The cross-language
// CBOR round-trip test in scripts/test_hopeandgrace_cbor.js asserts
// byte-equal hashes between Rust and Node for this struct. Reordering
// any field below is a `record_version` bump.

#[derive(CandidType, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct SettlementRecordInput {
    pub record_version:   u32,
    pub ceremony_number:  u64,
    pub ceremony_date:    String,
    pub random_seed:      String,
    pub pool_total_cents: u64,
    pub split:            CeremonySplit,
    pub angel:            AngelOutcome,
    pub soul:             SoulOutcome,
    pub direct_blessings: DirectBlessings,
    pub outcome:          CeremonyOutcome,
    pub rollover_cents:   u64,
    pub ops_ledger_entry: LedgerEntry,
    pub ledger:           Vec<LedgerEntry>,
    pub generated_at_ns:  u64,
}

// ---------------------------------------------------------------------------
// SettlementRecord — the primary on-chain artifact
// ---------------------------------------------------------------------------

#[derive(CandidType, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct SettlementRecord {
    /// Schema version. v1 is the initial. If H&G ever needs to evolve the
    /// schema, bump this and the canister rejects records with unknown
    /// versions until upgraded.
    pub record_version:   u32,

    /// Unique, monotonically-assigned-by-H&G ceremony identifier. Primary key.
    pub ceremony_number:  u64,

    /// ISO 8601 date string "YYYY-MM-DD" of the ceremony. Always UTC.
    pub ceremony_date:    String,

    /// The H&G-generated random seed used for ceremony selection (uuid or hex).
    /// Recorded verbatim so the selection is independently verifiable.
    pub random_seed:      String,

    /// Total cents in the pool at ceremony time.
    pub pool_total_cents: u64,

    pub split:            CeremonySplit,
    pub angel:            AngelOutcome,
    pub soul:             SoulOutcome,
    pub direct_blessings: DirectBlessings,
    pub outcome:          CeremonyOutcome,

    /// Cents rolled over to the next chalice (when reverted) or 0 when claimed.
    pub rollover_cents:   u64,

    /// The ops fee / divine offering entry, broken out for visibility.
    pub ops_ledger_entry: LedgerEntry,

    /// Complete ledger of cents movements for this ceremony.
    pub ledger:           Vec<LedgerEntry>,

    /// When H&G generated this record (their server time, nanoseconds).
    pub generated_at_ns:  u64,

    /// When the canister stored it (canister time, nanoseconds). Set by
    /// archive_ceremony, ignored if writer passes a value.
    pub archived_at_ns:   u64,

    /// sha256 hex of a canonical encoding of all the above fields. The
    /// canister recomputes this on archive and rejects if it doesn't match
    /// what the writer sent — defense against accidental corruption in transit.
    pub content_hash:     String,
}

#[derive(CandidType, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct SettlementSummary {
    pub ceremony_number:     u64,
    pub ceremony_date:       String,
    pub outcome:             CeremonyOutcome,
    pub pool_total_cents:    u64,
    pub soul_received_cents: u64,
    pub has_story:           bool,
    pub content_hash:        String,
}

#[derive(CandidType, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Totals {
    pub ceremonies:                   u64,
    pub total_pool_cents:             u64,
    pub total_to_souls_cents:         u64,
    pub total_divine_offering_cents:  u64,
    pub total_direct_blessings_cents: u64,
    /// Distinct soul UUIDs that have received any blessing.
    pub souls_blessed:                u64,
    /// Distinct angel UUIDs that have been part of any ceremony.
    pub angels_active:                u64,
}

#[derive(CandidType, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct RecordRef {
    pub ceremony_number: u64,
    pub content_hash:    String,
    pub archived_at_ns:  u64,
}

// ---------------------------------------------------------------------------
// Legal documents
// ---------------------------------------------------------------------------

#[derive(CandidType, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct LegalDoc {
    /// Document kind. Suggested values: "terms", "privacy", "disclosures".
    /// Free-form string; H&G owns the taxonomy.
    pub kind:            String,
    /// Monotonic version per kind, 1-indexed. Same kind+version is an error.
    pub version:         u32,
    /// ISO 8601 date this version takes effect.
    pub effective_date:  String,
    /// Markdown source of the document. Plain text, not HTML.
    pub content_md:      String,
    /// sha256 hex of `content_md` bytes. Verified on write.
    pub content_hash:    String,
    /// When the canister stored it (canister time).
    pub published_at_ns: u64,
}

#[derive(CandidType, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct LegalDocMeta {
    pub kind:            String,
    pub version:         u32,
    pub effective_date:  String,
    pub content_hash:    String,
    pub published_at_ns: u64,
}

/// Composite key for the legal doc map: (kind, version).
/// Lexicographic ordering means listing all versions of one kind is a
/// contiguous range scan.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct LegalDocKey {
    kind:    String,
    version: u32,
}

// ---------------------------------------------------------------------------
// Access control
// ---------------------------------------------------------------------------

#[derive(CandidType, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct CanisterConfig {
    /// Can rotate writers, change owner (with care), do admin.
    /// Defaults to the deploying principal on init.
    pub owner:   Principal,
    /// Principals allowed to call archive_ceremony and put_legal_doc.
    /// Starts empty; owner adds via add_writer.
    pub writers: Vec<Principal>,
    /// Set by `set_owner_initiate` to arm a pending ownership transfer.
    /// Cleared on `set_owner_accept` (when the pending owner finalizes it)
    /// or `set_owner_cancel` (when the current owner aborts the transfer).
    /// `#[serde(default)]` lets old Phase 1a records deserialize cleanly
    /// (they'll get None, which is correct — no pending transfer).
    #[serde(default)]
    pub pending_owner: Option<Principal>,
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(CandidType, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum HopeAndGraceError {
    /// Caller is not in the writers list (for write methods) or is not
    /// the owner (for admin methods).
    Unauthorized,
    /// Requested ceremony / legal doc doesn't exist.
    NotFound,
    /// archive_ceremony called with a ceremony_number that's already stored.
    /// Returns the existing ceremony_number for idempotency support.
    AlreadyArchived(u64),
    /// Record failed structural validation. The String explains which check.
    InvalidRecord(String),
    /// Anonymous (default) principals are always rejected for writes.
    AnonymousCaller,
    /// One of the conservation/invariant checks failed in archive_ceremony.
    /// The String explains which check (e.g. "split sums to 600 but
    /// pool_total is 599").
    InvariantViolated(String),
}

// ---------------------------------------------------------------------------
// Health
// ---------------------------------------------------------------------------

#[derive(CandidType, Serialize, Deserialize, Clone, Debug)]
pub struct HealthStatus {
    pub canister:        String,
    pub ok:              bool,
    pub ceremony_count:  u64,
    pub legal_doc_count: u64,
    pub timestamp_ns:    u64,
}

// ---------------------------------------------------------------------------
// Storable impls — required for ic-stable-structures
// ---------------------------------------------------------------------------

impl Storable for SettlementRecord {
    fn to_bytes(&self) -> Cow<'_, [u8]> { Cow::Owned(Encode!(self).unwrap()) }
    fn from_bytes(b: Cow<[u8]>) -> Self { Decode!(&b, SettlementRecord).unwrap() }
    const BOUND: Bound = Bound::Unbounded;
}

impl Storable for LegalDoc {
    fn to_bytes(&self) -> Cow<'_, [u8]> { Cow::Owned(Encode!(self).unwrap()) }
    fn from_bytes(b: Cow<[u8]>) -> Self { Decode!(&b, LegalDoc).unwrap() }
    const BOUND: Bound = Bound::Unbounded;
}

impl Storable for CanisterConfig {
    fn to_bytes(&self) -> Cow<'_, [u8]> { Cow::Owned(Encode!(self).unwrap()) }
    fn from_bytes(b: Cow<[u8]>) -> Self { Decode!(&b, CanisterConfig).unwrap() }
    const BOUND: Bound = Bound::Unbounded;
}

impl Storable for LegalDocKey {
    fn to_bytes(&self) -> Cow<'_, [u8]> {
        // Layout: kind_len (1 byte) | kind (utf-8) | version (4 bytes big-endian)
        let kind_bytes = self.kind.as_bytes();
        let mut out = Vec::with_capacity(1 + kind_bytes.len() + 4);
        out.push(kind_bytes.len() as u8);
        out.extend_from_slice(kind_bytes);
        out.extend_from_slice(&self.version.to_be_bytes());
        Cow::Owned(out)
    }
    fn from_bytes(b: Cow<[u8]>) -> Self {
        let kind_len = b[0] as usize;
        let kind = String::from_utf8(b[1..1 + kind_len].to_vec()).unwrap();
        let mut v = [0u8; 4];
        v.copy_from_slice(&b[1 + kind_len..1 + kind_len + 4]);
        let version = u32::from_be_bytes(v);
        LegalDocKey { kind, version }
    }
    // 1 (len byte) + 64 (max kind length we'd realistically see) + 4 (version)
    const BOUND: Bound = Bound::Bounded { max_size: 80, is_fixed_size: false };
}

// ---------------------------------------------------------------------------
// Stable memory layout
// ---------------------------------------------------------------------------

type Memory = VirtualMemory<DefaultMemoryImpl>;

const MEM_CEREMONIES: MemoryId = MemoryId::new(0);
const MEM_LEGAL_DOCS: MemoryId = MemoryId::new(1);
const MEM_CONFIG:     MemoryId = MemoryId::new(2);
const MEM_COUNTERS:   MemoryId = MemoryId::new(3);

const CONFIG_KEY: u8 = 0;

thread_local! {
    static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> =
        RefCell::new(MemoryManager::init(DefaultMemoryImpl::default()));

    static CEREMONIES: RefCell<StableBTreeMap<u64, SettlementRecord, Memory>> = RefCell::new(
        StableBTreeMap::init(MEMORY_MANAGER.with(|m| m.borrow().get(MEM_CEREMONIES)))
    );

    static LEGAL_DOCS: RefCell<StableBTreeMap<LegalDocKey, LegalDoc, Memory>> = RefCell::new(
        StableBTreeMap::init(MEMORY_MANAGER.with(|m| m.borrow().get(MEM_LEGAL_DOCS)))
    );

    static CONFIG_STORE: RefCell<StableBTreeMap<u8, CanisterConfig, Memory>> = RefCell::new(
        StableBTreeMap::init(MEMORY_MANAGER.with(|m| m.borrow().get(MEM_CONFIG)))
    );

    // Reserved for future counters (e.g. distinct-uuid tallies if we ever
    // want O(1) Totals instead of O(n) recomputes).
    static _COUNTERS: RefCell<StableBTreeMap<u8, u64, Memory>> = RefCell::new(
        StableBTreeMap::init(MEMORY_MANAGER.with(|m| m.borrow().get(MEM_COUNTERS)))
    );
}

// ---------------------------------------------------------------------------
// Lifecycle
// ---------------------------------------------------------------------------

#[init]
fn init() {
    // First-deploy initialization: caller becomes owner, writers list is empty.
    // Phase 1b methods will check `caller in writers` before accepting writes,
    // so the canister is effectively read-only until the owner adds writers.
    let cfg = CanisterConfig {
        owner:         ic_cdk::api::caller(),
        writers:       Vec::new(),
        pending_owner: None,
    };
    CONFIG_STORE.with(|c| c.borrow_mut().insert(CONFIG_KEY, cfg));
    ic_cdk::println!("hopeandgrace canister initialized");
}

#[post_upgrade]
fn post_upgrade() {
    // Stable storage persists across upgrades automatically. If we ever add
    // background work (timers, etc), this is where we'd re-arm. For now,
    // just log that we survived.
    ic_cdk::println!("hopeandgrace canister upgraded; stable storage preserved");
}

// ---------------------------------------------------------------------------
// Candid export — Phase 1a has no service block yet (no methods).
// The export macro still emits the type definitions, which is what we want
// for the .did file generation.
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------
//
// These are private to the canister. Public methods in the next section
// compose them. Keeping the logic here keeps the public methods short and
// readable — they read like prose, the gnarly stuff lives below.

/// Wrapper around `ic_cdk::api::time()` so we have one place to mock if
/// we ever want to. (For now it's just a clarity rename.)
fn now_ns() -> u64 {
    ic_cdk::api::time()
}

/// Load the singleton CanisterConfig. Panics if not present — init() is
/// responsible for setting it, so absence here is a bug, not a runtime
/// condition.
fn get_config() -> CanisterConfig {
    CONFIG_STORE.with(|c| {
        c.borrow()
            .get(&CONFIG_KEY)
            .expect("CanisterConfig must exist; init() should have created it")
    })
}

/// Save the singleton CanisterConfig back to stable storage.
fn save_config(cfg: CanisterConfig) {
    CONFIG_STORE.with(|c| c.borrow_mut().insert(CONFIG_KEY, cfg));
}

/// Returns the current caller's principal. Separated for clarity and so
/// future mocking (if we ever do test-only callers) has one place to touch.
fn caller() -> Principal {
    ic_cdk::api::caller()
}

/// Returns Ok(()) if the caller is the owner. Otherwise Err with the
/// appropriate variant (AnonymousCaller for the anonymous principal,
/// Unauthorized for any other non-owner principal).
fn caller_must_be_owner() -> Result<(), HopeAndGraceError> {
    let c = caller();
    if c == Principal::anonymous() {
        return Err(HopeAndGraceError::AnonymousCaller);
    }
    if c != get_config().owner {
        return Err(HopeAndGraceError::Unauthorized);
    }
    Ok(())
}

/// Returns Ok(()) if the caller is in the writers list. Otherwise the
/// appropriate error variant. The owner is NOT automatically a writer —
/// owner is for admin, writers is for data. Keep them separate so we can
/// rotate writers without compromising owner identity.
fn caller_must_be_writer() -> Result<(), HopeAndGraceError> {
    let c = caller();
    if c == Principal::anonymous() {
        return Err(HopeAndGraceError::AnonymousCaller);
    }
    let cfg = get_config();
    if !cfg.writers.contains(&c) {
        return Err(HopeAndGraceError::Unauthorized);
    }
    Ok(())
}

/// Computes the content_hash for a SettlementRecordInput using the
/// canonical encoding contract: sha256(canonical_cbor(input)).
///
/// FIELD ORDER IN SettlementRecordInput IS THE CANONICAL ENCODING CONTRACT.
/// Any reordering of fields in that struct is a `record_version` bump.
///
/// ciborium encodes structs as CBOR maps in field-declaration order; it
/// does NOT sort keys per RFC 8949 §4.2. That's deterministic for our
/// fixed schema but means cross-language verifiers (the Node round-trip
/// test in scripts/test_hopeandgrace_cbor.js) must encode keys in the
/// same field-declaration order, not sorted.
fn compute_content_hash(input: &SettlementRecordInput) -> String {
    use sha2::{Sha256, Digest};

    let mut bytes: Vec<u8> = Vec::new();
    ciborium::into_writer(input, &mut bytes)
        .expect("ciborium encoding of SettlementRecordInput should never fail");

    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let digest = hasher.finalize();

    hex::encode(digest)
}

// ---------------------------------------------------------------------------
// Invariant checks for archive_ceremony
// ---------------------------------------------------------------------------
//
// Each function returns Ok(()) if the invariant holds, or
// Err(InvariantViolated(msg)) with a precise diagnostic if not. Numbered
// per the spec; #7 (content_hash matches) is gone because content_hash is
// now canister-computed, not client-asserted.

/// #1 — Caller must be an authorized writer. (Auth is technically not a
/// "data invariant" but it's the first gate, so we check it like one.)
/// This wraps caller_must_be_writer() for symmetry with the others.
fn invariant_1_auth() -> Result<(), HopeAndGraceError> {
    caller_must_be_writer()
}

/// #2 — Idempotency: ceremony_number must not already be archived.
/// Returns AlreadyArchived (not InvariantViolated) so H&G can detect and
/// skip on retry.
fn invariant_2_idempotent(input: &SettlementRecordInput) -> Result<(), HopeAndGraceError> {
    let exists = CEREMONIES.with(|c| c.borrow().contains_key(&input.ceremony_number));
    if exists {
        Err(HopeAndGraceError::AlreadyArchived(input.ceremony_number))
    } else {
        Ok(())
    }
}

/// #3 — Pool conservation: split sums to pool_total.
/// split.soul_base + split.angel_gross + split.divine_offering == pool_total_cents
fn invariant_3_split_sums_to_pool(input: &SettlementRecordInput)
    -> Result<(), HopeAndGraceError>
{
    let sum = input.split.soul_base_cents
        .saturating_add(input.split.angel_gross_cents)
        .saturating_add(input.split.divine_offering_cents);
    if sum != input.pool_total_cents {
        return Err(HopeAndGraceError::InvariantViolated(format!(
            "split sums to {} cents but pool_total_cents is {}",
            sum, input.pool_total_cents
        )));
    }
    Ok(())
}

/// #4 — Angel split conservation: donated + kept == angel_gross.
fn invariant_4_angel_split(input: &SettlementRecordInput)
    -> Result<(), HopeAndGraceError>
{
    let sum = input.angel.donated_cents.saturating_add(input.angel.kept_cents);
    if sum != input.split.angel_gross_cents {
        return Err(HopeAndGraceError::InvariantViolated(format!(
            "angel.donated_cents ({}) + angel.kept_cents ({}) = {} but angel_gross_cents is {}",
            input.angel.donated_cents,
            input.angel.kept_cents,
            sum,
            input.split.angel_gross_cents
        )));
    }
    Ok(())
}

/// #5 — Soul receipt sanity: soul.total_received <= pool_total + direct_blessings.
/// Souls can receive more than soul_base (direct blessings add on top), but
/// they cannot receive more than the entire pool plus direct blessings.
fn invariant_5_soul_receipt(input: &SettlementRecordInput)
    -> Result<(), HopeAndGraceError>
{
    let ceiling = input.pool_total_cents
        .saturating_add(input.direct_blessings.total_cents);
    if input.soul.total_received_cents > ceiling {
        return Err(HopeAndGraceError::InvariantViolated(format!(
            "soul.total_received_cents ({}) exceeds pool_total + direct_blessings ({})",
            input.soul.total_received_cents, ceiling
        )));
    }
    Ok(())
}

/// #6 — Schema version: only known versions are accepted. Today that's
/// just v1. Future v2 records will be rejected by a v1 canister until
/// upgrade — which is what we want, so old code never silently
/// misinterprets new records.
fn invariant_6_schema_version(input: &SettlementRecordInput)
    -> Result<(), HopeAndGraceError>
{
    if input.record_version != 1 {
        return Err(HopeAndGraceError::InvariantViolated(format!(
            "record_version is {} but this canister only accepts version 1",
            input.record_version
        )));
    }
    Ok(())
}

/// #8 — Ledger consistency: the ledger must be non-empty, and the final
/// entry's balance_after_cents must equal the running balance after all
/// entries are applied (each amount_cents added to the previous balance).
///
/// We don't validate the starting balance (chalice balance varies between
/// ceremonies) — we just check internal consistency.
fn invariant_8_ledger_consistency(input: &SettlementRecordInput)
    -> Result<(), HopeAndGraceError>
{
    if input.ledger.is_empty() {
        return Err(HopeAndGraceError::InvariantViolated(
            "ledger must not be empty".into()
        ));
    }

    // Walk the ledger: each entry's balance_after_cents must equal the
    // previous entry's balance_after_cents plus this entry's amount_cents.
    // We seed the walk with the first entry's balance_after_cents (so the
    // first entry's amount is "the change from prior chalice state to this
    // ceremony's starting balance").
    let mut prev_balance: i128 = input.ledger[0].balance_after_cents as i128;
    for (i, entry) in input.ledger.iter().enumerate().skip(1) {
        let expected = prev_balance.saturating_add(entry.amount_cents as i128);
        if expected != entry.balance_after_cents as i128 {
            return Err(HopeAndGraceError::InvariantViolated(format!(
                "ledger entry #{}: balance_after_cents is {} but {} + {} = {}",
                i,
                entry.balance_after_cents,
                prev_balance,
                entry.amount_cents,
                expected
            )));
        }
        prev_balance = entry.balance_after_cents as i128;
    }

    Ok(())
}

/// Run all invariants in order, returning the first failure (or Ok if all
/// pass). Order matters — auth first (cheap, gates everything), then
/// idempotency (cheap, avoids wasted work on duplicates), then the
/// conservation checks (the actual data validation).
fn check_all_invariants(input: &SettlementRecordInput) -> Result<(), HopeAndGraceError> {
    invariant_1_auth()?;
    invariant_2_idempotent(input)?;
    invariant_3_split_sums_to_pool(input)?;
    invariant_4_angel_split(input)?;
    invariant_5_soul_receipt(input)?;
    invariant_6_schema_version(input)?;
    invariant_8_ledger_consistency(input)?;
    Ok(())
}
// ---------------------------------------------------------------------------
// Public API — administrative methods (owner + writers management)
// ---------------------------------------------------------------------------
//
// Access model recap:
//   * owner   — singleton Principal, can rotate writers, transfer ownership
//   * writers — Vec<Principal>, allowed to call archive_ceremony +
//               put_legal_doc (lands in Pass 4)
//
// Ownership transfer uses a two-step initiate/accept pattern:
//   1. Current owner calls set_owner_initiate(new) — arms pending_owner.
//      Does NOT change the active owner. Reversible by calling
//      set_owner_initiate again or set_owner_cancel.
//   2. New owner calls set_owner_accept() from THEIR own identity.
//      This proves they actually control the principal — guarding against
//      typos and against transferring to a principal nobody owns.
//
// Compare to plain `set_owner(new)`, which has no way to verify the new
// principal is real. One typo with that and the canister is permanently
// unadministrable. The two-step pattern eliminates that footgun.

/// Step 1 of ownership transfer. Arms the pending_owner slot. Reversible.
/// The new owner must subsequently call set_owner_accept() from their own
/// identity to complete the transfer.
#[update]
fn set_owner_initiate(new: Principal) -> Result<(), HopeAndGraceError> {
    caller_must_be_owner()?;
    if new == Principal::anonymous() {
        return Err(HopeAndGraceError::InvalidRecord(
            "cannot transfer ownership to the anonymous principal".into()
        ));
    }
    let mut cfg = get_config();
    cfg.pending_owner = Some(new);
    save_config(cfg);
    Ok(())
}

/// Step 2 of ownership transfer. Must be called BY the pending_owner.
/// Completes the transfer atomically: clears pending_owner, sets owner.
#[update]
fn set_owner_accept() -> Result<(), HopeAndGraceError> {
    let c = caller();
    if c == Principal::anonymous() {
        return Err(HopeAndGraceError::AnonymousCaller);
    }
    let mut cfg = get_config();
    match cfg.pending_owner {
        Some(pending) if pending == c => {
            cfg.owner          = c;
            cfg.pending_owner  = None;
            save_config(cfg);
            Ok(())
        }
        Some(_) => Err(HopeAndGraceError::Unauthorized),
        None    => Err(HopeAndGraceError::NotFound),  // no pending transfer armed
    }
}

/// Cancel a pending ownership transfer. Owner-only. Useful if the initial
/// new-owner principal turns out to be wrong before they've accepted.
#[update]
fn set_owner_cancel() -> Result<(), HopeAndGraceError> {
    caller_must_be_owner()?;
    let mut cfg = get_config();
    cfg.pending_owner = None;
    save_config(cfg);
    Ok(())
}

/// Add a principal to the writers list. Idempotent: adding the same
/// principal twice is a no-op success.
#[update]
fn add_writer(p: Principal) -> Result<(), HopeAndGraceError> {
    caller_must_be_owner()?;
    if p == Principal::anonymous() {
        return Err(HopeAndGraceError::InvalidRecord(
            "cannot add the anonymous principal as a writer".into()
        ));
    }
    let mut cfg = get_config();
    if !cfg.writers.contains(&p) {
        cfg.writers.push(p);
        save_config(cfg);
    }
    Ok(())
}

/// Remove a principal from the writers list. Idempotent: removing a
/// principal that isn't a writer is a no-op success.
#[update]
fn remove_writer(p: Principal) -> Result<(), HopeAndGraceError> {
    caller_must_be_owner()?;
    let mut cfg = get_config();
    cfg.writers.retain(|w| w != &p);
    save_config(cfg);
    Ok(())
}

/// List all current writers. Public query — transparency about who can
/// archive records is part of the trust story.
#[query]
fn list_writers() -> Vec<Principal> {
    get_config().writers
}

/// Get the current owner. Public query — anyone can verify who admin's
/// the canister.
#[query]
fn get_owner() -> Principal {
    get_config().owner
}

/// Get the current pending_owner (if any). Public query — visibility into
/// in-flight ownership transfers.
#[query]
fn get_pending_owner() -> Option<Principal> {
    get_config().pending_owner
}

/// Convenience query — is this principal a writer? Useful for the dashboard
/// to enable/disable UI affordances based on the current identity.
#[query]
fn is_writer(p: Principal) -> bool {
    get_config().writers.contains(&p)
}

// ---------------------------------------------------------------------------
// Public API — health check
// ---------------------------------------------------------------------------

/// Health snapshot. Manager canister polls this; dashboard displays it.
/// Public query — never sensitive, always available.
#[query]
fn health_check() -> HealthStatus {
    let ceremony_count  = CEREMONIES.with(|c| c.borrow().len());
    let legal_doc_count = LEGAL_DOCS.with(|d| d.borrow().len());
    HealthStatus {
        canister:        "hopeandgrace".to_string(),
        ok:              true,
        ceremony_count,
        legal_doc_count,
        timestamp_ns:    now_ns(),
    }
}

ic_cdk::export_candid!();
// ---------------------------------------------------------------------------
// Unit tests — Storable roundtrips and ordering
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_ledger_entry() -> LedgerEntry {
        LedgerEntry {
            entry_type:          "soul_blessing_base".into(),
            amount_cents:        40000,
            balance_after_cents: 60000,
            party:               "soul#abc-uuid".into(),
            description:         "Base blessing to anonymous soul".into(),
            at_ns:               1_700_000_000_000_000_000,
        }
    }

    fn sample_settlement_record() -> SettlementRecord {
        SettlementRecord {
            record_version:   1,
            ceremony_number:  42,
            ceremony_date:    "2026-06-08".into(),
            random_seed:      "deadbeef-cafe-1234-5678-90abcdef1234".into(),
            pool_total_cents: 100000,
            split: CeremonySplit {
                soul_base_cents:       40000,
                angel_gross_cents:     40000,
                divine_offering_cents: 20000,
                divine_offering_bps:   2000,
            },
            angel: AngelOutcome {
                uuid:          "angel-uuid-1".into(),
                claimed:       true,
                donated_bps:   5000,
                donated_cents: 20000,
                kept_cents:    20000,
            },
            soul: SoulOutcome {
                uuid:                 "soul-uuid-1".into(),
                engaged:              true,
                reverted:             false,
                reverted_at_ns:       None,
                total_received_cents: 65000,
                story_cid:            Some("bafybeih5fp4kkxvxnxfqz5gqxnxfqz5gqxnxfqz5gqxnxfqz5gqxnxfqz5gq".into()),
                story_hash:           Some("a".repeat(64)),
            },
            direct_blessings: DirectBlessings {
                total_cents: 5000,
                donor_count: 3,
            },
            outcome:          CeremonyOutcome::Claimed,
            rollover_cents:   0,
            ops_ledger_entry: sample_ledger_entry(),
            ledger:           vec![sample_ledger_entry()],
            generated_at_ns:  1_700_000_000_000_000_000,
            archived_at_ns:   1_700_000_000_500_000_000,
            content_hash:     "b".repeat(64),
        }
    }

    #[test]
    fn settlement_record_roundtrip() {
        let original = sample_settlement_record();
        let bytes    = original.to_bytes();
        let decoded  = SettlementRecord::from_bytes(bytes);
        assert_eq!(original, decoded);
    }

    #[test]
    fn legal_doc_roundtrip() {
        let original = LegalDoc {
            kind:            "terms".into(),
            version:         1,
            effective_date:  "2026-06-08".into(),
            content_md:      "# Terms of Service\n\nBy using this service…".into(),
            content_hash:    "c".repeat(64),
            published_at_ns: 1_700_000_000_000_000_000,
        };
        let bytes   = original.to_bytes();
        let decoded = LegalDoc::from_bytes(bytes);
        assert_eq!(original, decoded);
    }

    #[test]
    fn canister_config_roundtrip() {
        let original = CanisterConfig {
            owner:         Principal::from_slice(&[1, 2, 3, 4, 5]),
            writers:       vec![
                Principal::from_slice(&[6, 7, 8]),
                Principal::from_slice(&[9, 10, 11, 12]),
            ],
            pending_owner: None,
        };
        let bytes   = original.to_bytes();
        let decoded = CanisterConfig::from_bytes(bytes);
        assert_eq!(original, decoded);
    }

    #[test]
    fn legal_doc_key_roundtrip() {
        let original = LegalDocKey {
            kind:    "privacy".into(),
            version: 7,
        };
        let bytes   = original.to_bytes();
        let decoded = LegalDocKey::from_bytes(bytes);
        assert_eq!(original.kind,    decoded.kind);
        assert_eq!(original.version, decoded.version);
    }

    #[test]
    fn legal_doc_key_ordering_groups_by_kind() {
        let terms_v1   = LegalDocKey { kind: "terms".into(),   version: 1 };
        let terms_v2   = LegalDocKey { kind: "terms".into(),   version: 2 };
        let privacy_v1 = LegalDocKey { kind: "privacy".into(), version: 1 };

        // The encoding prefixes with kind_len, so byte-wise ordering groups
        // by (kind_len, kind_bytes, version). This means:
        //   * all entries with the same kind are contiguous (the property
        //     that makes range scans efficient)
        //   * within one kind, versions sort ascending (1, 2, 3, ...)
        // The exact ordering across different kinds depends on length first,
        // then bytes — that's fine, we don't care about cross-kind ordering.
        let mut entries = vec![
            (terms_v2.to_bytes().into_owned(),   "t2"),
            (privacy_v1.to_bytes().into_owned(), "p1"),
            (terms_v1.to_bytes().into_owned(),   "t1"),
        ];
        entries.sort();
        let labels: Vec<&str> = entries.iter().map(|(_, l)| *l).collect();

        // "terms" (length 5) sorts before "privacy" (length 7) because the
        // encoding starts with kind_len. Within "terms", v1 before v2.
        // What we genuinely care about: same-kind entries are contiguous.
        assert_eq!(labels, vec!["t1", "t2", "p1"]);

        // Additionally verify the "same-kind contiguous" property explicitly
        // — find the indices of each kind and check they form a contiguous run.
        let t_positions: Vec<usize> = labels.iter().enumerate()
            .filter(|(_, l)| l.starts_with('t'))
            .map(|(i, _)| i)
            .collect();
        assert_eq!(t_positions, vec![0, 1], "all 'terms' entries should be contiguous");
    }

    #[test]
    fn ceremony_outcome_eq_works() {
        // Sanity check that the enum derives Eq properly (matters for tests
        // and for potential future filtering logic).
        assert_eq!(CeremonyOutcome::Claimed, CeremonyOutcome::Claimed);
        assert_ne!(CeremonyOutcome::Claimed, CeremonyOutcome::RevertedToChalice);
        assert_ne!(CeremonyOutcome::Pending, CeremonyOutcome::Claimed);
    }

    #[test]
    fn settlement_record_with_no_story_roundtrips() {
        // The case where share_permission was false — story_cid + story_hash
        // are both None.
        let mut record = sample_settlement_record();
        record.soul.story_cid  = None;
        record.soul.story_hash = None;
        let bytes   = record.to_bytes();
        let decoded = SettlementRecord::from_bytes(bytes);
        assert_eq!(record, decoded);
        assert!(decoded.soul.story_cid.is_none());
        assert!(decoded.soul.story_hash.is_none());
    }
    // -----------------------------------------------------------------------
    // Pass 2C tests — compute_content_hash + invariant checks
    // -----------------------------------------------------------------------
    //
    // A small helper to build a valid SettlementRecordInput for invariant
    // testing. Each test then perturbs one field to exercise the specific
    // invariant under test. Keeping a single source of truth for "what
    // does a valid record look like" makes the tests readable and the
    // failure messages precise.

    fn sample_input() -> SettlementRecordInput {
        SettlementRecordInput {
            record_version:   1,
            ceremony_number:  100,
            ceremony_date:    "2026-06-08".into(),
            random_seed:      "abc-123-def-456".into(),
            pool_total_cents: 100_000,
            split: CeremonySplit {
                soul_base_cents:       40_000,
                angel_gross_cents:     40_000,
                divine_offering_cents: 20_000,
                divine_offering_bps:   2_000,
            },
            angel: AngelOutcome {
                uuid:          "angel-1".into(),
                claimed:       true,
                donated_bps:   5_000,
                donated_cents: 20_000,
                kept_cents:    20_000,
            },
            soul: SoulOutcome {
                uuid:                 "soul-1".into(),
                engaged:              true,
                reverted:             false,
                reverted_at_ns:       None,
                total_received_cents: 40_000,
                story_cid:            None,
                story_hash:           None,
            },
            direct_blessings: DirectBlessings {
                total_cents: 0,
                donor_count: 0,
            },
            outcome:          CeremonyOutcome::Claimed,
            rollover_cents:   0,
            ops_ledger_entry: LedgerEntry {
                entry_type:          "divine_offering".into(),
                amount_cents:        20_000,
                balance_after_cents: 80_000,
                party:               "ops".into(),
                description:         "20% divine offering".into(),
                at_ns:               1_700_000_000_000_000_000,
            },
            // Two-entry consistent ledger: start at 100_000, debit 40_000
            // to bring it to 60_000.
            ledger: vec![
                LedgerEntry {
                    entry_type:          "pool_start".into(),
                    amount_cents:        0,
                    balance_after_cents: 100_000,
                    party:               "chalice".into(),
                    description:         "ceremony pool opened".into(),
                    at_ns:               1_700_000_000_000_000_000,
                },
                LedgerEntry {
                    entry_type:          "soul_blessing_base".into(),
                    amount_cents:        -40_000,
                    balance_after_cents: 60_000,
                    party:               "soul#soul-1".into(),
                    description:         "soul base blessing".into(),
                    at_ns:               1_700_000_000_000_000_001,
                },
            ],
            generated_at_ns:  1_700_000_000_500_000_000,
        }
    }

    // --- compute_content_hash tests ---

    #[test]
    fn content_hash_is_deterministic() {
        let input = sample_input();
        let h1 = compute_content_hash(&input);
        let h2 = compute_content_hash(&input);
        assert_eq!(h1, h2);
        // Sanity: hex-encoded sha256 is 64 chars.
        assert_eq!(h1.len(), 64);
        // Sanity: all hex chars.
        assert!(h1.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn content_hash_changes_with_ceremony_number() {
        let mut a = sample_input();
        let mut b = sample_input();
        a.ceremony_number = 100;
        b.ceremony_number = 101;
        assert_ne!(compute_content_hash(&a), compute_content_hash(&b));
    }

    #[test]
    fn content_hash_changes_with_amount() {
        let mut a = sample_input();
        let mut b = sample_input();
        a.ledger[1].amount_cents = -40_000;
        b.ledger[1].amount_cents = -40_001;
        // Re-balance b's ledger so it stays internally consistent, since
        // we're only testing that the hash sees the change.
        b.ledger[1].balance_after_cents = 59_999;
        assert_ne!(compute_content_hash(&a), compute_content_hash(&b));
    }

    #[test]
    fn content_hash_distinguishes_none_from_some_story() {
        // CRITICAL test for cross-language correctness. CBOR encodes
        // Option::None and Option::Some(value) as different bytes — if
        // the Node round-trip test ever sees these collide, encoding is
        // broken on one side.
        let mut a = sample_input();
        let mut b = sample_input();
        a.soul.story_cid  = None;
        a.soul.story_hash = None;
        b.soul.story_cid  = Some("bafyabc".into());
        b.soul.story_hash = Some("d".repeat(64));
        assert_ne!(compute_content_hash(&a), compute_content_hash(&b));
    }

    #[test]
    fn content_hash_distinguishes_outcome_variants() {
        // Same critical-for-Node-parity check, but for the CeremonyOutcome
        // enum. CBOR-encodes enum variants by their name, not by index;
        // confirm Rust sees these as different bytes.
        let mut a = sample_input();
        let mut b = sample_input();
        a.outcome = CeremonyOutcome::Claimed;
        b.outcome = CeremonyOutcome::RevertedToChalice;
        assert_ne!(compute_content_hash(&a), compute_content_hash(&b));
    }

    // --- invariant_3: split sums to pool ---

    #[test]
    fn invariant_3_accepts_correct_split() {
        let input = sample_input();
        // sample is 40k + 40k + 20k = 100k = pool_total_cents
        assert!(invariant_3_split_sums_to_pool(&input).is_ok());
    }

    #[test]
    fn invariant_3_rejects_undersized_split() {
        let mut input = sample_input();
        // Drop divine offering by 1 cent — sum is now 99_999, not 100_000.
        input.split.divine_offering_cents = 19_999;
        let result = invariant_3_split_sums_to_pool(&input);
        assert!(matches!(result, Err(HopeAndGraceError::InvariantViolated(_))));
    }

    #[test]
    fn invariant_3_rejects_oversized_split() {
        let mut input = sample_input();
        // Bump soul base by 1 cent — sum is now 100_001, not 100_000.
        input.split.soul_base_cents = 40_001;
        assert!(matches!(
            invariant_3_split_sums_to_pool(&input),
            Err(HopeAndGraceError::InvariantViolated(_))
        ));
    }

    // --- invariant_4: angel donated + kept == angel gross ---

    #[test]
    fn invariant_4_accepts_correct_angel_split() {
        let input = sample_input();
        // sample is 20k donated + 20k kept = 40k = angel_gross
        assert!(invariant_4_angel_split(&input).is_ok());
    }

    #[test]
    fn invariant_4_rejects_mismatched_angel_split() {
        let mut input = sample_input();
        input.angel.donated_cents = 25_000;
        // kept stays 20_000, gross stays 40_000 → 25+20 != 40 → reject
        assert!(matches!(
            invariant_4_angel_split(&input),
            Err(HopeAndGraceError::InvariantViolated(_))
        ));
    }

    // --- invariant_5: soul receipt within ceiling ---

    #[test]
    fn invariant_5_accepts_within_ceiling() {
        let input = sample_input();
        // soul received 40k, ceiling is pool (100k) + direct (0) = 100k
        assert!(invariant_5_soul_receipt(&input).is_ok());
    }

    #[test]
    fn invariant_5_rejects_exceeding_ceiling() {
        let mut input = sample_input();
        input.soul.total_received_cents = 100_001;
        assert!(matches!(
            invariant_5_soul_receipt(&input),
            Err(HopeAndGraceError::InvariantViolated(_))
        ));
    }

    #[test]
    fn invariant_5_accepts_with_direct_blessings() {
        let mut input = sample_input();
        // Soul can receive MORE than soul_base if direct blessings exist.
        // Pool 100k + direct 50k = ceiling 150k. Soul receiving 130k is fine.
        input.direct_blessings.total_cents = 50_000;
        input.direct_blessings.donor_count = 5;
        input.soul.total_received_cents    = 130_000;
        assert!(invariant_5_soul_receipt(&input).is_ok());
    }

    // --- invariant_6: schema version ---

    #[test]
    fn invariant_6_accepts_v1() {
        let input = sample_input();
        assert!(invariant_6_schema_version(&input).is_ok());
    }

    #[test]
    fn invariant_6_rejects_v2() {
        let mut input = sample_input();
        input.record_version = 2;
        assert!(matches!(
            invariant_6_schema_version(&input),
            Err(HopeAndGraceError::InvariantViolated(_))
        ));
    }

    // --- invariant_8: ledger consistency ---

    #[test]
    fn invariant_8_accepts_consistent_ledger() {
        let input = sample_input();
        assert!(invariant_8_ledger_consistency(&input).is_ok());
    }

    #[test]
    fn invariant_8_rejects_empty_ledger() {
        let mut input = sample_input();
        input.ledger.clear();
        let result = invariant_8_ledger_consistency(&input);
        assert!(matches!(result, Err(HopeAndGraceError::InvariantViolated(_))));
        // Verify the message specifically mentions emptiness.
        if let Err(HopeAndGraceError::InvariantViolated(msg)) = result {
            assert!(msg.contains("empty"),
                "expected 'empty' in error message, got: {}", msg);
        }
    }

    #[test]
    fn invariant_8_rejects_inconsistent_ledger() {
        let mut input = sample_input();
        // Tamper with the second entry's balance — should be 60_000 given
        // -40_000 from 100_000. Set it to 50_000 instead.
        input.ledger[1].balance_after_cents = 50_000;
        assert!(matches!(
            invariant_8_ledger_consistency(&input),
            Err(HopeAndGraceError::InvariantViolated(_))
        ));
    }

    #[test]
    fn invariant_8_accepts_single_entry_ledger() {
        let mut input = sample_input();
        // A ledger with just the opening entry is valid — we don't require
        // a minimum number of entries beyond "not empty".
        input.ledger.truncate(1);
        assert!(invariant_8_ledger_consistency(&input).is_ok());
    }
}    