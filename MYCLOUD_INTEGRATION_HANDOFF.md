# Hope & Grace ⇄ mycloud — Canister Integration Handoff

**For:** the mycloud project (Rust/ICP canisters + VPS + IPFS)
**From:** the Hope & Grace backend side
**Purpose:** define the *contract* for storing Hope & Grace's ceremony records and legal
documents immutably on an ICP canister, and rendering them on the public **Ripples of
Compassion** page (and, later, a **Hall of Angels** view).

> This is an integration contract, not an implementation. It says **what data flows, in
> what shape, when, and what is public** — so the mycloud side can build the canister and
> dashboard, and the Hope & Grace side can build the push, without either guessing at the
> other. Hope & Grace is a Node.js/Express + MySQL backend on a separate VPS
> (`api.hopeandgrace.space`). mycloud owns the canister(s) and the on-chain storage.

---

## 0. The page decision (so the canister supports the right reads)

- **Ripples of Compassion** (priority): the public, Soul-centric story archive. Each settled
  ceremony shows an anonymized story, the outcome, community gifts, and a link proving the
  record is permanently on-chain.
- **Hall of Angels** (optional, Phase 2): a giver-centric recognition view — angel handles,
  badges, and the Dove pay-it-forward chains.

Both read the **same** immutable records; only the rendering differs. Build the canister to
serve both facets; build the Ripples page first.

---

## 1. The lifecycle that produces a record

```
Ceremony (weekly)  →  Blessed Board / blessed.html  →  settled & archived  →  Ripples
   draw + split        30-day community donation         push to canister      reads canister
```

1. A weekly ceremony draws one Angel + one Soul and splits the pool (Soul gets a guaranteed
   20%, the Angel may gift more, the charity takes a sliding "Divine Offering").
2. For 30 days the community can donate directly to that Soul's story (the Blessed Board).
3. The blessing reaches a **terminal, settled state** (Soul engaged & kept funds, or Soul
   never engaged and funds reverted) **and** the 30-day window closes.
4. **Hope & Grace then pushes one immutable `SettlementRecord` to the canister.**
5. The Ripples page renders from the canister.

**Trigger for the push (Hope & Grace side, my responsibility):** a daily job selects
blessings where the settlement is terminal **and** the 30-day Blessed-Board window has
elapsed **and** `archived_on_chain` is not yet set; it calls the canister's write method,
stores the returned on-chain reference locally, and marks the blessing archived. Idempotent:
each ceremony is pushed exactly once.

---

## 2. Privacy boundary (critical)

**Only anonymized data goes on-chain. No PII, ever.**

- Souls and Angels are identified **only by their UUID** (already how the record is built) —
  never name, email, phone, or address.
- The Soul's story is the **anonymized snapshot** captured at ceremony time (it's already
  scrubbed of identifying detail by the vetting/interview step before it can be shared).
- Direct-donation data is **aggregate only** (total + donor count) — no donor identities.
- Because the chain is public and permanent, treat every field as world-readable forever.
  If in doubt, leave it off.

A Soul also controls visibility: only stories with `share_permission = true` should have
their `story` text included; otherwise push the record with the story omitted (the financial
facts can still be archived for transparency, just without the narrative).

---

## 3. The SettlementRecord (the core payload)

This is the exact object the Hope & Grace engine assembles per ceremony. Treat it as the
canister's input schema (translate to Candid as you see fit).

```json
{
  "record_version": 1,
  "ceremony_number": 12,
  "ceremony_date": "2026-06-20",
  "random_seed": "hex-or-uuid",
  "pool_total": 600.00,
  "split": {
    "soul_base": 120.00,
    "angel_gross": 384.00,
    "divine_offering": 96.00,
    "divine_offering_pct": 20.00
  },
  "angel": {
    "uuid": "…",
    "claimed": true,
    "donated_pct": 50,
    "donated_amt": 192.00,
    "kept": 192.00
  },
  "soul": {
    "uuid": "…",
    "engaged": true,
    "reverted": false,
    "reverted_at": null,
    "total_received": 312.00,
    "story": "anonymized narrative, only if share_permission is true"
  },
  "direct_blessings": { "total": 0.00, "donor_count": 0 },
  "outcome": "claimed",
  "rollover_amount": 0.00,
  "ops_ledger_entry": { "amount": 96.00, "balance_after": 96.00, "at": "iso-ts" },
  "ledger": [
    { "type": "soul_blessing_base", "amount": 120.00, "balance_after": 120.00,
      "party": "soul#…", "description": "…", "at": "iso-ts" }
  ],
  "generated_at": "iso-ts"
}
```

- `outcome` ∈ `claimed` | `reverted_to_chalice` | `pending`.
- `random_seed` + `split` + `ledger` make every ceremony **independently verifiable** —
  anyone can recompute the math from the seed and confirm conservation
  (`soul_base + divine_offering + angel_gross = pool_total`). Store them verbatim.
- Money is in dollars with 2 decimals. If the canister prefers integers, store **cents**
  (multiply by 100) and document it; the Hope & Grace side will send whichever you specify.

---

## 4. Canister methods the Ripples flow needs

Conceptual signatures (Candid is yours to define). "restricted" = only the authorized
Hope & Grace writer principal may call; everything else is a public query.

```
// ---- Settlement ledger ----
archive_ceremony(record: SettlementRecord) -> Result<RecordRef, Error>   // update, restricted
get_ceremony(ceremony_number: nat) -> opt SettlementRecord               // query, public
list_ceremonies(offset: nat, limit: nat) -> vec SettlementSummary        // query, public
public_totals() -> Totals                                                // query, public

// RecordRef = { index: nat; content_hash: text; timestamp: nat }  // the immutable proof
// SettlementSummary = a light projection for list views:
//   { ceremony_number, ceremony_date, outcome, pool_total, soul_received,
//     has_story: bool, content_hash }
// Totals = running transparency aggregate, e.g.:
//   { ceremonies: nat; total_pool: float; total_to_souls: float;
//     total_divine_offering: float; total_direct_blessings: float;
//     souls_blessed: nat; angels_active: nat }
```

- `archive_ceremony` should be **idempotent by `ceremony_number`** (re-submitting the same
  ceremony returns the existing `RecordRef`, doesn't duplicate). Hope & Grace guards against
  resends too, but defense in depth is good.
- `content_hash` = a hash of the canonicalized record. Returning it lets the Ripples page
  show a verifiable fingerprint ("recorded on-chain · 0xabc…").
- `public_totals` powers the Ripples page header and your future transparency/990 view
  without N calls.

---

## 5. Legal document registry (immutable, versioned)

The charity's legal pages (Terms of Service, Privacy Policy, Legal Disclosures) are a perfect
fit for immutable, timestamped versioning — a charity that can prove exactly what its terms
said on any date earns trust. Drafts currently live at
`…/hopeandgrace.spaceNEW/legaldocs/` (`TERMS_OF_SERVICE_DRAFT.md`,
`PRIVACY_POLICY_DRAFT.md`, `LEGAL_DISCLOSURES_DRAFT.md`) — mycloud Claude can read those for
real content once the council/attorney approve them.

```
put_legal_doc(doc: LegalDoc) -> Result<RecordRef, Error>     // update, restricted
get_legal_doc(kind: text) -> opt LegalDoc                    // query, public — latest version
list_legal_doc_versions(kind: text) -> vec LegalDocMeta      // query, public — full history

// kind ∈ "terms" | "privacy" | "disclosures"
// LegalDoc = { kind, version: nat, effective_date: text, content_md: text,
//              content_hash: text, published_at: nat }
// LegalDocMeta = { kind, version, effective_date, content_hash, published_at }
```

- Each publish appends a new immutable version; `get_legal_doc` returns the latest.
- The legal pages render the latest `content_md`, show "Version N · effective DATE", and link
  to the version history with hashes. Tamper-proof and audit-friendly.
- Publishing cadence is rare and human-initiated (after council/attorney approval), so this
  can be driven from the mycloud dashboard directly rather than from the Hope & Grace
  backend, if that's simpler for you.

---

## 6. Access control

- One **authorized writer principal** (a dedicated service identity for the Hope & Grace
  backend) is the only caller allowed to invoke the `restricted` update methods. Set it at
  canister init / via an admin method.
- All `query` methods are **public** (the whole point is open transparency).
- Please tell the Hope & Grace side: the **canister ID**, the **method names/Candid**, and
  whether you want the writer to authenticate via a service identity in `@dfinity/agent`
  (preferred — the Node backend can hold a dedicated identity whose principal you authorize)
  or via a canister HTTP endpoint.

---

## 7. The Hope & Grace side (what I will build, for your awareness)

Once your write method + canister ID + authorized-writer principal exist, I'll add on the
Hope & Grace backend:

- a dedicated service identity (its principal is what you authorize as the writer),
- a daily `archiveSettledCeremonies` job that builds each due `SettlementRecord`
  (the assembler already exists), calls `archive_ceremony`, and stores the returned
  `RecordRef` + `content_hash` on the local blessing row (so we never resend),
- (optionally) a thin read-proxy endpoint if you'd rather the Ripples page read through
  `api.hopeandgrace.space` than call the canister directly.

So the boundary is clean: **Hope & Grace pushes records; the canister stores and serves them;
the Ripples page reads them.**

---

## 8. Open questions for mycloud Claude

1. **Where does this live** — a new dedicated "ledger/archive" canister, or a module on an
   existing one? (You know the auth/registry/manager layout; your call.)
2. **Read path for Ripples** — does the page call the canister directly via `@dfinity/agent`
   in the browser, or through a Hope & Grace read-proxy? (Direct is more "sovereign"; proxy
   is simpler to cache/rate-limit.)
3. **Money representation** — dollars-as-float or integer cents on-chain?
4. **Writer auth** — service identity via agent-js, or an HTTP ingress?
5. **Canister ID + Candid** — send these back so I can wire the push.

Answer these and the Hope & Grace push is a short, well-scoped build on my side.

---

## Appendix — verification property (worth surfacing on Ripples)

Because each record stores the `random_seed`, the `split`, and the full `ledger`, the Ripples
page (or any visitor) can prove a ceremony was fair and the math was honest:

1. Recompute the split from `pool_total` and `divine_offering_pct`.
2. Confirm `soul_base = floor(pool_total × 0.20)` and
   `soul_base + divine_offering + angel_gross = pool_total`.
3. Confirm the `ledger` entries sum consistently with the stated outcome.
4. Confirm the `content_hash` matches the stored record.

That's a genuinely powerful trust signal for a charity — "don't trust us, verify us."
