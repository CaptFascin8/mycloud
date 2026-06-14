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

## Phase 1b refinement — content_hash is canister-produced (June 8, 2026)

Decision revised after H&G Claude review. The Phase 0 plan had H&G sending
a `content_hash` field that the canister would verify. Both Claudes
independently concluded this was a false-assurance pattern: a hash whose
canonical encoding isn't precisely defined is a fingerprint that verifies
nothing while looking like it does.

The fix changes two things at once:

**1. Two distinct types instead of one.**

```rust
// What H&G's @dfinity/agent code sends. NO content_hash, NO archived_at_ns.
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
    pub generated_at_ns:  u64,   // H&G's record-generation time (when they
                                 // assembled this from MySQL). Distinct from
                                 // canister's archived_at_ns. H&G sends this;
                                 // it goes into the canonical CBOR hash.
}

// What gets stored on-chain. Input + two canister-populated fields.
pub struct SettlementRecord {
    // ... all fields from SettlementRecordInput, in identical order ...
    pub content_hash:   String,  // canister-computed
    pub archived_at_ns: u64,     // canister-computed
}
```

Making the omitted fields *structurally absent* from the input type
eliminates a whole class of "did I set this?" confusion. H&G's TypeScript
client physically cannot send them.

**2. The hashed view is `SettlementRecordInput`, exactly.**

```rust
content_hash = sha256_hex(canonical_cbor(input_as_received))
```

The canister hashes the input *before* it adds anything. There is no
"excluded fields" list to keep in sync between sides — the type system
IS the exclusion. Field-order contract lives in exactly one place: the
declaration order of `SettlementRecordInput`. Reordering those fields is
a `record_version` bump.

This means invariant #7 from the original spec is no longer a gate
("reject if hash doesn't match"). It's just a derived field the canister
computes deterministically from the bytes it received. Phase 1b ships
invariants 1–6 + #8 as gates; #7 becomes assignment, not validation.

### Why canonical CBOR (via ciborium)?

- **No library-version drift** — CBOR's encoding rules are RFC 8949,
  not the library's whim. Upgrading ciborium doesn't change output bytes.
  (Candid does NOT have this property — type tables can reorder across
  candid library versions.)
- **Cross-language friendly** — every major language has a canonical
  CBOR library. H&G's Node-side round-trip test uses `cbor-x` or similar.
- **Serde-derive includes all fields automatically** — no "forgot to
  extend the encoder" gap that would silently leave a field unhashed.
  Adding a field to `SettlementRecordInput` means it's in the hash by
  construction.
- **Handles nested structs and Vec<LedgerEntry> for free.**
- **~10 lines of code** in the canister, ~50KB wasm size impact.

### ciborium-specific caveat (worth knowing)

ciborium encodes a struct as a CBOR **map in field-declaration order**
and does NOT sort keys per RFC 8949 §4.2 by default. For our fixed
schema this is perfectly deterministic — but the cross-language verifier
must emit keys in that same field-declaration order, not sorted.

The cross-language round-trip test (see below) is the safeguard: if Node
and Rust ever disagree on encoding order, the test fails loudly. If
matching field order across libraries proves annoying, we can switch
the input to a CBOR array (no keys → nothing to order). For now, start
with the map approach and let the test prove correctness.

### Cross-language round-trip test (Phase 1b deliverable)

`scripts/test_hopeandgrace_cbor.{js,ts}` — a small Node script that:
1. Builds sample `SettlementRecordInput` values
2. CBOR-encodes them by the documented rule (cbor-x or similar)
3. sha256-hexes the result
4. Asserts byte-equal to what the canister returns in `RecordRef.content_hash`

The test IS the canonical-encoding spec. Documentation in this file is
for humans; the test is for machines. Both should agree.

Sample vectors must hit the CBOR cross-language divergence hotspots:

- **An `Option::None` and an `Option::Some` for `story_cid`/`story_hash`** —
  None vs Some encode differently in CBOR; classic mismatch point.
- **Both `CeremonyOutcome` variants (`Claimed` and `RevertedToChalice`)** —
  confirm the enum hashes as the string variant name (e.g. `"Claimed"`),
  not a numeric index, and that Node emits it identically.
- **A negative `amount_cents` in a ledger entry** — CBOR negative integers
  are their own major type (major 1, not major 0); worth a vector.
- **A populated `ledger` Vec with 2–3 entries** — exercises CBOR array
  encoding.

Floats would have been the fourth and worst landmine — already eliminated
by the cents + basis-points decision in Phase 0.

### Provenance / non-repudiation is NOT what content_hash provides

Worth being explicit: `content_hash` proves the canister stored exactly
these bytes. It does NOT prove "H&G authored these bytes." That property
comes from the **authorized-writer principal check** on `archive_ceremony` —
the canister knows it was H&G because the caller's principal is in the
`writers` Vec. Authenticity via access control, not hash comparison.

If end-to-end cryptographic non-repudiation is ever needed (H&G's signing
key signs each record, anyone can verify against H&G's public key), that's
a future Checkpoint 4.5.2 item with `ic_cdk::ecdsa_public_key` or similar.
Not in Phase 1b scope. Not what hashes are for.

### Two different hashes doing two different jobs

When the canister stores a record with a story:

| Field          | What it is                                          | Who computes   |
|----------------|-----------------------------------------------------|----------------|
| `story_hash`   | sha256 of raw UTF-8 story bytes (the story file)    | H&G            |
| `content_hash` | sha256 of canonical CBOR of `SettlementRecordInput` | canister       |

`story_hash` lets a verifier confirm that the bytes fetched from IPFS via
`story_cid` are the bytes H&G originally pinned. It's about story content
integrity. H&G computes it because H&G owns the original bytes — no
encoding ambiguity because raw UTF-8 has no canonical encoding question.

`content_hash` lets a verifier confirm the record itself wasn't mangled
during storage. It's about record integrity. Canister computes it because
the canonical CBOR rule lives in canister code (the source of truth).

These are NOT redundant. They protect different surfaces. Keep both.

### The general principle for who computes a hash

H&G Claude framed this elegantly during the Pass 4 design:

> **Canister computes the hash when the hash is over a canonical-encoded
> structure (no external party can be the encoding authority). The client
> supplies the hash, and the canister verifies, when the hash is over raw
> bytes (anyone can reproduce it).**

This single rule justifies the asymmetry in the API:

- `archive_ceremony(SettlementRecordInput) -> RecordRef` — canister
  computes `content_hash`. The hash is over a structure that requires
  canonical CBOR encoding; only the canister can be the encoding authority.
  No gate; the hash is derived, populated in the returned RecordRef.

- `put_legal_doc(LegalDoc) -> RecordRef` — canister VERIFIES the
  client-supplied `content_hash`. The hash is over raw UTF-8 bytes of
  `content_md`; anyone can reproduce it with `sha256(content_md.bytes())`.
  Gate: reject with `Err(InvariantViolated)` on mismatch.

For LegalDoc the verification gate IS legitimate (catches transmission
corruption, no canonical-encoding ambiguity to litigate). For
SettlementRecord the verification gate is NOT legitimate (would require
agreeing on a canonical encoding the client doesn't control), which is
why Phase 1b removed that gate.

### LegalDoc.content_hash — precise specification

To prevent representation disagreement between H&G and the canister,
the LegalDoc hash is pinned to three specific properties:

1. **Digest:** sha256
2. **Bytes:** raw UTF-8 encoding of `content_md` (the markdown source).
   Not the parsed/normalized markdown — the literal bytes the H&G server
   has in memory.
3. **Output format:** lowercase hexadecimal, 64 chars.

So `LegalDoc.content_hash == lowercase_hex(sha256(content_md.bytes()))`.
Any deviation (uppercase hex, base64, normalized markdown) is a bug on
the H&G side, caught by the canister's verification gate.

### Coordination notes (for Phase 3 wiring)

When the canister is deployed and the Candid file is shared with H&G
Claude for Phase 3 work, the Candid MUST export BOTH types:

- `SettlementRecordInput` — what H&G's `@dfinity/agent` code encodes
- `SettlementRecord` — what queries return

H&G Claude's archive push code on the Node side will encode against
`SettlementRecordInput`, so its field order must exactly match the
Rust struct declaration. If the spec pins this struct as the canonical
field-order source, the two sides cannot drift.

The `archive_ceremony` method signature becomes:

```candid
archive_ceremony : (SettlementRecordInput)
                    -> (variant { Ok: RecordRef; Err: HopeAndGraceError });
```

(Takes Input, returns Ref containing the computed content_hash.)

### Updated IPFS workflow (replaces the old version in this doc)

```
Hope & Grace daily archive job:
  for ceremony in due_ceremonies:
    if soul.share_permission and soul.story_text:
      story_bytes = utf8(soul.story_text)
      story_hash  = sha256_hex(story_bytes)
      story_cid   = mycloud_ipfs_pin(story_bytes)
      soul_payload.story_cid  = Some(story_cid)
      soul_payload.story_hash = Some(story_hash)
    else:
      soul_payload.story_cid  = None
      soul_payload.story_hash = None

    input = build_settlement_record_input(...)
    // Note: no content_hash, no archived_at_ns. Those come back in RecordRef.
    result = hopeandgrace.archive_ceremony(input)
    // result is Ok(RecordRef { ceremony_number, content_hash, archived_at_ns })
    mysql.mark_archived(
        ceremony_id,
        result.ceremony_number,
        result.content_hash,
        result.archived_at_ns
    )
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

**Phase 0** (~30 min): ✅ DONE  decisions locked, spec written
**Phase 1a** (~90 min): ✅ DONE  data model + storage scaffolding, 7 unit tests
**Phase 1b** (~4 hours, fresh session): canister methods + invariants + tests
  - Add `SettlementRecordInput` type per the June 8 refinement above
  - Add `ciborium` to workspace deps
  - Implement `archive_ceremony(SettlementRecordInput) -> Result<RecordRef, _>`:
    - Auth check (caller in writers, not anonymous)
    - Compute content_hash from `sha256_hex(canonical_cbor(input))`
    - Build `SettlementRecord = input + content_hash + archived_at_ns`
    - Run invariants 1–6 + #8 (NOT #7 — that's now just assignment)
    - Insert into stable storage
    - Return RecordRef
  - Implement `get_ceremony`, `list_ceremonies`, `public_totals`
  - Implement `put_legal_doc`, `get_legal_doc`, `list_legal_doc_versions`
  - Implement access control: `set_owner`, `add_writer`, `remove_writer`,
    `list_writers`, `get_owner`
  - Implement `health_check`
  - Integration tests via `scripts/test_hopeandgrace.sh` (all positive
    and negative paths)
  - Cross-language CBOR round-trip test in
    `scripts/test_hopeandgrace_cbor.{js,ts}` (the canonical-encoding contract)
**Phase 2** (~1 hour): deploy to mainnet
  - estimate cost: ~5T cycles for creation + initial allocation, ~$5 USD
  - record canister ID, share with Hope & Grace Claude
  - share the Candid file (which MUST include `SettlementRecordInput`)
  - authorize H&G service identity as a writer
**Phase 3** (Hope & Grace Claude's responsibility, ~2 hours):
  - service identity setup
  - daily archive job sending `SettlementRecordInput`
  - IPFS pinning workflow
  - mark-archived bookkeeping (stores returned content_hash + archived_at_ns)
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
- **Checkpoint 4.5.2 — H&G-signed records for non-repudiation.** Today the
  canister authenticates H&G via the authorized-writer principal check.
  If we ever want end-to-end cryptographic proof "this record was produced
  by H&G's signing key, not just stored under H&G's principal," the
  correct primitive is a signature — H&G signs the canonical CBOR before
  sending, the canister verifies against H&G's pubkey (stored in canister
  state), and anyone querying can verify independently. Uses
  `ic_cdk::ecdsa_public_key` or similar. NOT what content_hash is for.
- **Migration path if SettlementRecord schema changes.** Bumping
  `record_version` is the lever. Old records stay readable; new records reject
  if writer sends old version. We'll think about this when v2 is actually needed.
