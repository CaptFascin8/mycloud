# H&G → ICP Archive Push (`backend/icp/`)

The Hope & Grace side of the mycloud integration: it pushes each terminally
settled ceremony (past its 30-day Blessed-Board window) to the `hopeandgrace`
canister as a `SettlementRecordInput`, and records the returned `RecordRef` so
each ceremony is archived exactly once.

This is a **scaffold staged ahead of the canister deploy.** It runs and
self-tests today; it goes live the moment mycloud hands back three things
(see "What I need from mycloud Claude" below).

## Files

| File | Role |
|---|---|
| `convert.js` | Maps the engine's `buildSettlementRecord()` output (dollars/percent) → canister `SettlementRecordInput` (integer cents + basis points, BigInt). |
| `idl.js` | **Stand-in** Candid interface for local dev. Replace with `dfx generate` output after deploy. |
| `identity.js` | Loads the low-privilege H&G **writer** identity from a PEM. |
| `actor.js` | Builds the `@dfinity/agent` actor; network-switchable (local replica ↔ mainnet). |
| `pinStory.js` | Pins a consented story to IPFS; computes `story_hash` = sha256 of raw UTF-8 bytes (lowercase hex). |
| `archive.js` | The sweep: select → build → pin → convert → `archive_ceremony` → record. Idempotent, per-record error isolation. |
| `index.js` | CLI (`--dry` / `--once`) + `registerArchiveCron()` for engine.js. |
| `selfcheck.js` | Offline converter checks + emits the shared round-trip test vectors. |
| `../migrate_icp.js` | Idempotent migration adding archive bookkeeping columns. |

## Setup

```bash
# 1. Deps (not yet in package.json)
npm install @dfinity/agent @dfinity/candid @dfinity/principal \
            @dfinity/identity @dfinity/identity-secp256k1

# 2. Migration (adds archived_on_chain, content_hash, story_cid, ... to blessings_history)
node migrate_icp.js

# 3. Confirm the converter is sound + grab the test vectors for mycloud Claude
node icp/selfcheck.js
```

## Local-replica test path (before mainnet — matches the spec's plan)

```bash
# Against mycloud Claude's LOCAL dfx replica:
export ICP_NETWORK=local
export ICP_HOST=http://127.0.0.1:4943
export ICP_CANISTER_ID=<local hopeandgrace canister id>
export ICP_WRITER_PEM=/secure/path/hg-writer.pem

node icp/index.js --dry     # build + convert only, no canister calls
node icp/index.js --once    # one real sweep against the local replica
```

`--dry` exercises selection + the assembler + the converter end-to-end without
touching the canister — safe to run against production data anytime.

## Going live

```bash
export ICP_NETWORK=ic
export ICP_HOST=https://icp-api.io
export ICP_CANISTER_ID=<mainnet canister id>
export ICP_ARCHIVE_ENABLED=true     # registerArchiveCron is a no-op until this is set
```

Then in `engine.js` `scheduleTasks()`, alongside the other crons:

```js
const { registerArchiveCron } = require('./icp');
registerArchiveCron(cron, db);   // daily 08:00 UTC (just after the 07:00 soul-settlement sweep)
```

## Environment variables

| Var | Default | Meaning |
|---|---|---|
| `ICP_NETWORK` | `local` | `local` or `ic`. On `local` the agent fetches the replica root key; on `ic` it never does. |
| `ICP_HOST` | per network | Replica/boundary host. A localhost host with `ic` is refused. |
| `ICP_CANISTER_ID` | — | The deployed `hopeandgrace` canister id. |
| `ICP_WRITER_PEM` | — | Path to the writer identity `.pem` (keep `0600`, out of git). |
| `ICP_ARCHIVE_ENABLED` | unset | Must be `true` for the cron to register. |
| `ICP_ARCHIVE_CRON` | `0 8 * * *` | Cron schedule. |
| `ICP_ARCHIVE_WINDOW_DAYS` | `30` | Blessed-Board window before a claimed ceremony is eligible. |
| `ICP_ARCHIVE_MAX_ATTEMPTS` | `5` | Stop retrying a record after this many failures (surfaced in `archive_last_error`). |
| `ICP_ARCHIVE_BATCH` | `25` | Max ceremonies per sweep. |
| `IPFS_API_URL` | unset | mycloud Kubo RPC (e.g. `http://127.0.0.1:5001`). Unset → story runs in DRY (hash only, no CID). |
| `IPFS_API_AUTH` | unset | Optional `Authorization` header value for the IPFS API. |

## Locked design notes (don't "fix" without a `record_version` bump)

- **`content_hash` and `archived_at_ns` are never sent.** The canister computes
  the hash (canonical CBOR) and stamps the archive time. They are absent from
  `SettlementRecordInput` by construction — `selfcheck.js` asserts this.
- **`generated_at_ns` is sent by H&G** (our generation time, distinct from the
  canister's archive time). If the final `.did` puts it canister-side instead,
  delete it from both `convert.js` and `idl.js` — it's one line in each, clearly
  marked.
- **Money → `Math.round(dollars * 100)` cents; percent → `Math.round(pct * 100)`
  basis points.** Integers only on the wire — no float canonicalization risk.
- **`story_hash` (raw UTF-8 bytes) ≠ `content_hash` (canonical CBOR of the
  record).** Two hashes, two jobs.

## What I need from mycloud Claude to finish wiring

1. **Canister ID** (and whether it's the local replica or mainnet).
2. The **generated Candid declarations** (`dfx generate hopeandgrace`) — so
   `idl.js` is replaced by the guaranteed-correct factory. The Candid **must
   include `SettlementRecordInput`** (the exact shape encoded here) and `LegalDoc`.
3. Confirmation the **writer principal** (printed by `selfcheck`/`dfx identity
   get-principal --identity hg-writer`) has been authorized via `add_writer`.

Then: point env at the local replica, run `node icp/index.js --once`, and we
watch a real ceremony land on chain. The shared test vectors from `selfcheck.js`
let your Rust round-trip test hash the identical values, so we prove byte
agreement before we ever touch mainnet.
