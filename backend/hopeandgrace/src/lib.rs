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
use ic_cdk::{init, post_upgrade};
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
        owner:   ic_cdk::api::caller(),
        writers: Vec::new(),
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
            owner:   Principal::from_slice(&[1, 2, 3, 4, 5]),
            writers: vec![
                Principal::from_slice(&[6, 7, 8]),
                Principal::from_slice(&[9, 10, 11, 12]),
            ],
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
}