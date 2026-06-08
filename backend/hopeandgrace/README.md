# hopeandgrace canister

Hope & Grace's immutable ceremony ledger + versioned legal document registry.

This canister is the on-chain transparency layer for hopeandgrace.space.
It stores anonymized blessing ceremony records and signed legal documents,
queryable by anyone. Story text is stored off-chain via IPFS CID + hash.

**Spec:** see `docs/HOPEANDGRACE_INTEGRATION_SPEC.md` in the project root.

## Build status

- **Phase 1a (this commit):** data model + storage scaffolding, no methods.
  Types and `Storable` impls exist; the canister compiles, init runs, but
  there's no public API yet.
- **Phase 1b (next session):** methods, invariant validation, access control,
  full integration tests.
- **Phase 2:** mainnet deploy.
- **Phase 3 (Hope & Grace side):** daily archive job + service identity.
- **Phase 4:** Ripples page on hopeandgrace.space.

## Key design decisions

See `docs/HOPEANDGRACE_INTEGRATION_SPEC.md` for the full rationale. In brief:
- All money is integer cents (`nat64`), never floats.
- All percentages are basis points (`nat32`), where 1 bps = 0.01%.
- Story text lives on IPFS by CID; only CID + sha256 hash on chain.
- Owner + `Vec<writers>` access control for restricted methods.
- Canister rejects records that fail conservation invariants (Phase 1b).

## Sibling canisters

- `backend/auth/` — identity + credential vault (MyCloud core)
- `backend/registry/` — smartsite registry with chain-agnostic ownership
- `backend/manager/` — health watcher + cycles bursar
- `backend/hopeandgrace/` — this canister (first external project canister)