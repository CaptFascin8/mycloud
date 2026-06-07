# Hope & Grace Canister — Integration Spec (Phase 0 decisions)

**Status:** Spec locked Sunday, June 7, 2026. Ready for Phase 1 (canister implementation).
**Source handoff:** `MYCLOUD_INTEGRATION_HANDOFF.md` (Hope & Grace Claude)
**Build plan:** see "Phases" section at the bottom.

This document records the decisions made in response to the integration handoff's
open questions and the architectural pushbacks. It supersedes anything in the
handoff that conflicts. The handoff itself is preserved as the source-of-truth
description of Hope & Grace's data shape and lifecycle.

---

## Decision summary

| # | Question | Decision |
|---|----------|----------|
| 1 | Where does this live? | New dedicated `hopeandgrace` canister |
| 2 | Read path for Ripples? | Direct via `@dfinity/agent` in browser; static prerender + "verify on-chain" button |
| 3 | Money representation? | Integer cents (`nat64`) on-chain, always |
| 4 | Writer auth? | Service identity via `@dfinity/agent`, principal authorized in canister state |
| 5 | Canister ID + Candid? | TBD on deploy (Phase 2). Local replica first, then mainnet. |

Plus six architectural decisions from the review:

| # | Decision |
|---|----------|
| A | Story TEXT lives on IPFS (or H&G MySQL); only the CID + content_hash on chain |
| B | Money in `nat64` cents, never floats — applies to all monetary fields |
| C | One dedicated canister; not a module on auth/registry/manager |
| D | Owner + Vec<writers> access model (see "Access control" below) |
| E | Phase 2 Hall of Angels: angel handle resolution is off-chain (MySQL) |
| F | Ripples reads canister directly; H&G server-side proxy optional, secondary |

---

## Architecture overview

```
                Hope & Grace MySQL                      MyCloud IPFS
              (souls, angels, identities,             (anonymized story
               donor records, handles)                 text by CID)
                          │                                  │
                          │  story text                      │
                          ├──────────────────────────────────┤
                          │  CID + hash returned             │
                          ▼                                  ▼
              ┌──────────────────────────────────────────────────┐
              │  Hope & Grace daily archive job (Node.js)        │
              │  - selects settled ceremonies past 30-day window │
              │  - pins story text to IPFS, gets CID + hash      │
              │  - builds SettlementRecord (integer cents)       │
              │  - calls hopeandgrace.archive_ceremony()         │
              │  - stores RecordRef back in MySQL                │
              └─────────────────────┬────────────────────────────┘
                                    │
                                    │ @dfinity/agent
                                    ▼
              ┌──────────────────────────────────────────────────┐
              │  hopeandgrace canister (ICP, Rust)               │
              │  - stable BTreeMap<u64, SettlementRecord>        │
              │  - stable BTreeMap<(kind, version), LegalDoc>    │
              │  - owner: Principal (you, II)                    │
              │  - writers: Vec<Principal> (H&G service ident.)  │
              │  - methods per Section 4 of handoff              │
              └─────────────────────┬────────────────────────────┘
                                    │
                                    │ public queries
                                    ▼
              ┌──────────────────────────────────────────────────┐
              │  Ripples of Compassion page on hopeandgrace.space│
              │  - static prerender from canister at build time  │
              │  - live "verify on-chain" button calls canister  │
              │  - story text fetched by CID from MyCloud IPFS   │
              └──────────────────────────────────────────────────┘
```

---

## SettlementRecord — canister storage shape

Translated from the handoff's JSON shape to canister types with the
integer-cents and IPFS-story decisions applied.

```rust
pub struct SettlementRecord {
    pub record_version:        u32,             // schema version (start at 1)
    pub ceremony_number:        u64,            // primary key, unique
    pub ceremony_date:          String,         // ISO 8601 date "YYYY-MM-DD"
    pub random_seed:            String,         // hex or uuid, verbatim from H&G
    pub pool_total_cents:       u64,            // INTEGER cents
    pub split:                  CeremonySplit,
    pub angel:                  AngelOutcome,
    pub soul:                   SoulOutcome,
    pub direct_blessings:       DirectBlessings,
    pub outcome:                CeremonyOutcome,
    pub rollover_cents:         u64,            // INTEGER cents
    pub ops_ledger_entry:       LedgerEntry,
    pub ledger:                 Vec<LedgerEntry>,
    pub generated_at_ns:        u64,            // canister-time
    pub archived_at_ns:         u64,            // canister-time, when archive_ceremony ran
    pub content_hash:           String,         // sha256 of canonical encoding
}

pub struct CeremonySplit {
    pub soul_base_cents:        u64,
    pub angel_gross_cents:      u64,
    pub divine_offering_cents:  u64,
    pub divine_offering_bps:    u32,   // basis points (1bps = 0.01%) — 2000 = 20%
}

pub struct AngelOutcome {
    pub uuid:                   String,         // H&G anonymized UUID
    pub claimed:                bool,
    pub donated_bps:            u32,            // basis points of angel_gross
    pub donated_cents:          u64,
    pub kept_cents:             u64,
}

pub struct SoulOutcome {
    pub uuid:                   String,
    pub engaged:                bool,
    pub reverted:               bool,
    pub reverted_at_ns:         Option<u64>,
    pub total_received_cents:   u64,
    pub story_cid:              Option<String>, // IPFS CID if share_permission=true
    pub story_hash:             Option<String>, // sha256 of story text
}

pub struct DirectBlessings {
    pub total_cents:            u64,
    pub donor_count:            u64,            // aggregate only, no identities
}

pub enum CeremonyOutcome {
    Claimed,
    RevertedToChalice,
    Pending,                                    // shouldn't archive Pending, but type-complete
}

pub struct LedgerEntry {
    pub entry_type:             String,         // "soul_blessing_base" etc.
    pub amount_cents:           i64,            // can be negative
    pub balance_after_cents:    u64,
    pub party:                  String,         // "soul#uuid", "angel#uuid", "ops"
    pub description:            String,
    pub at_ns:                  u64,
}
```

### Why basis points instead of percentages?

Same reason as cents instead of dollars — integer math is exact, never rounds
weirdly. `divine_offering_bps: 2000` is 20.00%, no float ambiguity. 1bps =
0.01%. Industry-standard for finance.

---

## Access control model (Decision D)

```rust
pub struct CanisterConfig {
    pub owner:    Principal,      // can rotate writers, do admin
    pub writers:  Vec<Principal>, // can call restricted methods
}

// At init: owner = the deployer (you, via II during deploy)
// writers starts empty; owner adds H&G service identity via add_writer

// Methods:
// - set_owner(new: Principal)   — owner-only, requires confirmation
// - add_writer(p: Principal)    — owner-only
// - remove_writer(p: Principal) — owner-only
// - list_writers() -> Vec<Principal>  — public query (transparency)
// - get_owner() -> Principal    — public query
```

Restricted methods check `caller in writers`. Anonymous principals always rejected.

---

## Method signatures (Candid spec preview)

```candid
type CeremonyOutcome = variant { Claimed; RevertedToChalice; Pending };

type LedgerEntry = record {
  entry_type:           text;
  amount_cents:         int64;
  balance_after_cents:  nat64;
  party:                text;
  description:          text;
  at_ns:                nat64;
};

type CeremonySplit = record {
  soul_base_cents:        nat64;
  angel_gross_cents:      nat64;
  divine_offering_cents:  nat64;
  divine_offering_bps:    nat32;
};

type AngelOutcome = record {
  uuid:           text;
  claimed:        bool;
  donated_bps:    nat32;
  donated_cents:  nat64;
  kept_cents:     nat64;
};

type SoulOutcome = record {
  uuid:                  text;
  engaged:               bool;
  reverted:              bool;
  reverted_at_ns:        opt nat64;
  total_received_cents:  nat64;
  story_cid:             opt text;
  story_hash:            opt text;
};

type DirectBlessings = record {
  total_cents:  nat64;
  donor_count:  nat64;
};

type SettlementRecord = record {
  record_version:     nat32;
  ceremony_number:    nat64;
  ceremony_date:      text;
  random_seed:        text;
  pool_total_cents:   nat64;
  split:              CeremonySplit;
  angel:              AngelOutcome;
  soul:               SoulOutcome;
  direct_blessings:   DirectBlessings;
  outcome:            CeremonyOutcome;
  rollover_cents:     nat64;
  ops_ledger_entry:   LedgerEntry;
  ledger:             vec LedgerEntry;
  generated_at_ns:    nat64;
  archived_at_ns:     nat64;
  content_hash:       text;
};

type SettlementSummary = record {
  ceremony_number:      nat64;
  ceremony_date:        text;
  outcome:              CeremonyOutcome;
  pool_total_cents:     nat64;
  soul_received_cents:  nat64;
  has_story:            bool;
  content_hash:         text;
};

type Totals = record {
  ceremonies:              nat64;
  total_pool_cents:        nat64;
  total_to_souls_cents:    nat64;
  total_divine_offering_cents: nat64;
  total_direct_blessings_cents: nat64;
  souls_blessed:           nat64;  // distinct soul uuids
  angels_active:           nat64;  // distinct angel uuids
};

type RecordRef = record {
  ceremony_number: nat64;
  content_hash:    text;
  archived_at_ns:  nat64;
};

type LegalDoc = record {
  kind:            text;       // "terms" | "privacy" | "disclosures"
  version:         nat32;
  effective_date:  text;       // ISO date
  content_md:      text;       // markdown source
  content_hash:    text;       // sha256 of content_md
  published_at_ns: nat64;
};

type LegalDocMeta = record {
  kind:            text;
  version:         nat32;
  effective_date:  text;
  content_hash:    text;
  published_at_ns: nat64;
};

type HopeAndGraceError = variant {
  Unauthorized;
  NotFound;
  AlreadyArchived: nat64;      // ceremony_number that already exists
  InvalidRecord: text;
  AnonymousCaller;
  InvariantViolated: text;     // e.g. split doesn't sum to pool_total
};

service : {
  // Settlement archive (restricted writes, public reads)
  archive_ceremony  : (SettlementRecord)
                       -> (variant { Ok: RecordRef; Err: HopeAndGraceError });
  get_ceremony      : (nat64)
                       -> (opt SettlementRecord) query;
  list_ceremonies   : (nat64, nat64)            // offset, limit
                       -> (vec SettlementSummary) query;
  public_totals     : ()
                       -> (Totals) query;

  // Legal documents (restricted writes, public reads)
  put_legal_doc            : (LegalDoc)
                              -> (variant { Ok: RecordRef; Err: HopeAndGraceError });
  get_legal_doc            : (text)              // kind
                              -> (opt LegalDoc) query;
  list_legal_doc_versions  : (text)              // kind
                              -> (vec LegalDocMeta) query;

  // Access control (owner-only writes, public reads for transparency)
  set_owner       : (principal) -> (variant { Ok; Err: HopeAndGraceError });
  add_writer      : (principal) -> (variant { Ok; Err: HopeAndGraceError });
  remove_writer   : (principal) -> (variant { Ok; Err: HopeAndGraceError });
  list_writers    : () -> (vec principal) query;
  get_owner       : () -> (principal) query;

  // Health (matches MyCloud convention)
  health_check    : () -> (record {
    canister:      text;
    ok:            bool;
    ceremony_count: nat64;
    legal_doc_count: nat64;
    timestamp_ns:  nat64;
  }) query;
}
```

---

## Invariant validation (defense in depth)

`archive_ceremony` MUST verify before storing:

1. `caller in writers` (auth)
2. `ceremony_number` not already in storage (idempotency)
3. `split.soul_base_cents + split.angel_gross_cents + split.divine_offering_cents == pool_total_cents` (conservation)
4. `angel.donated_cents + angel.kept_cents == split.angel_gross_cents` (angel split sound)
5. `soul.total_received_cents <= pool_total_cents + direct_blessings.total_cents` (no impossible gains)
6. `record_version == 1` (forward compatibility — reject unknown schemas)
7. `content_hash` matches what the canister recomputes from canonical encoding
8. ledger.last().balance_after_cents is internally consistent with ops_ledger_entry

If any check fails: return `Err(InvariantViolated)` with a useful message. Don't store.
The canister is a notary, not just a database — its job is to refuse bad records.

---

## Story-on-IPFS workflow (Decision A)

```
Hope & Grace daily archive job:
  for ceremony in due_ceremonies:
    if soul.share_permission and soul.story_text:
      story_bytes = utf8(soul.story_text)
      story_hash  = sha256_hex(story_bytes)
      story_cid   = mycloud_ipfs_pin(story_bytes)
            // POST to MyCloud IPFS API or via Hope & Grace's own IPFS access
      soul_payload.story_cid  = Some(story_cid)
      soul_payload.story_hash = Some(story_hash)
    else:
      soul_payload.story_cid  = None
      soul_payload.story_hash = None

    record = build_settlement_record(...)
    record.content_hash = sha256_hex(canonical_encode(record))
    result = hopeandgrace.archive_ceremony(record)
    mysql.mark_archived(ceremony_id, result.ceremony_number, result.content_hash)
```

Right-to-be-forgotten path: if a Soul later requests redaction, MyCloud unpins
the IPFS CID. The chain still has `story_hash` and `story_cid` proving a story
existed, but the content becomes unretrievable. This is the best compromise
between "permanent record" and "user dignity."

The story_hash is also useful for integrity: if anyone fetches the story by
CID and the bytes don't hash to story_hash, it's been tampered with (or
they got the wrong CID).

---

## Phases

**Phase 0** (this doc, ~30 min): ✅ DONE  decisions locked, spec written
**Phase 1** (~4 hours, fresh session this evening): canister implementation
  - `backend/hopeandgrace/` Rust crate
  - `backend/hopeandgrace/src/lib.rs` with all types + methods
  - `backend/hopeandgrace/hopeandgrace.did` with Candid
  - `dfx.json` updated to declare the new canister
  - `scripts/test_hopeandgrace.sh` integration tests covering all methods
    + all 8 invariant validations + access control
**Phase 2** (~1 hour): deploy to mainnet
  - estimate cost: ~5T cycles for creation + initial allocation, ~$5 USD
  - record canister ID, share with Hope & Grace Claude
  - authorize H&G service identity as a writer
**Phase 3** (Hope & Grace Claude's responsibility, ~2 hours):
  - service identity setup
  - daily archive job
  - IPFS pinning workflow
  - mark-archived bookkeeping
**Phase 4** (~3 hours, can parallel with Phase 3):
  - Ripples page on hopeandgrace.space
  - `@dfinity/agent` browser integration
  - story fetch via MyCloud IPFS gateway
**Phase 5** (~1 hour, low priority): legal doc publishing UI

---

## Decisions NOT made (future open questions)

- **Mainnet vs local for Phase 1 testing.** Default plan: local replica through
  Phase 1, mainnet for Phase 2. Hope & Grace Claude works against a local
  canister using dfx, then we switch the canister ID once mainnet is live.
- **Cycles management.** The hopeandgrace canister will need ongoing cycles.
  MyCloud's manager canister was designed for exactly this — add hopeandgrace
  to manager's watched canister list and have it top up when low. (This is
  not the urgent path; we can monitor manually for the first month.)
- **vetKeys for legal doc authoring.** Future hardening — sign published
  legal docs with a vetKey so versions are provably authored by the council.
  Defer per CLOUD_FACTORY.md Stage 6.
- **Migration path if SettlementRecord schema changes.** Bumping
  `record_version` is the lever. Old records stay readable; new records reject
  if writer sends old version. We'll think about this when v2 is actually needed.
