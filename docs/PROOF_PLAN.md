# MyCloud Proof Plan

Five checkpoints, each a "stop, verify, proceed" gate.

## ✅ Checkpoint 1 — Scaffold (DONE)
- Directory tree at /opt/mycloud (and /mnt/c/MY_CLOUD on HOPE)
- Cargo workspace + rust-toolchain.toml pinning Rust stable
- dfx.json declares all 3 canisters + Internet Identity + dashboard
- Each canister has Cargo.toml, src/lib.rs, .did
- vps/docker/docker-compose.yml defines IPFS + Nginx (96xx port block)
- frontend/dashboard has Vite + React skeleton
- Bootstrapped via single bootstrap.sh script

**Commit:** `ce15974` — "checkpoint 2: scaffold compiles clean"

## ✅ Checkpoint 2 — Canisters compile (DONE)
- `cargo check --workspace` passes with zero errors
- Both unit tests pass: vault_key_roundtrip, vault_key_ordering_groups_by_owner

## ✅ Checkpoint 3a — auth canister real logic (DONE)
- Stable BTreeMap<Principal, User> survives canister upgrades
- Per-user credential vault with composite (Principal, label) keys
- health_check() endpoint shaped for future manager polling
- 12-call integration test passes end-to-end via scripts/test_auth.sh
- Caller-scoped access enforcement (anonymous principals rejected)

**Commit:** `8e0724a` — "checkpoint 3a: auth canister with stable storage + credential vault"

### Architectural pivots discovered during 3a
1. **WSL2 + dfx PocketIC has a known initialization bug.** Symptom: 400 Bad Request on /instances. Fix: don't develop on WSL, develop directly on the VPS via SSH.
2. **Candid reserved keywords cannot be used as record field names.** We hit `principal: principal` and `blob: blob` collisions. Renamed to `id` and `data`.
3. **SSH-backgrounded `dfx start` needs `nohup ... </dev/null & disown`** to survive the SSH session ending.
4. **Remote scripting via `bash -lc "..."` is fragile.** Always write scripts to a file and execute by path.

## ✅ Checkpoint 3b — registry canister (DONE)
- Smartsite CRUD with stable storage (BTreeMap<Domain, Smartsite>)
- Owner secondary index (BTreeMap<(Owner, Domain), ()>) for fast sites_by_owner
- OwnershipVerifier trait with InternetIdentity impl + Solana/Ethereum stubs
- EvmChain enum supports Polygon/ETH/Base/Arbitrum/Optimism
- Trait shape ready for Crystal Dragon Yggdrasil KEY tier verification
- Domain validation, CID validation, owner-only enforcement
- 7 unit tests + 18-call integration test (scripts/test_registry.sh)

**Commit:** `65ed6e1` — "checkpoint 3b: registry canister with smartsite CRUD + ownership verifier trait"

## ✅ Checkpoint 3c — manager canister (DONE)
- ic-cdk-timers periodic tick (60s default, configurable 10..=86400)
- Ring buffer of HealthEvents (max 100, configurable, in stable BTreeMap)
- Inter-canister calls to auth + registry health_check()
- WatchedCanister entries track last_check_ns, last_status_ok per target
- Owner-based access control on all admin methods
- top_up validation logic (real cycle transfer stubbed pending controller setup)
- Timer survives upgrades via post_upgrade re-arm
- 3 unit tests + 18-call integration test (scripts/test_manager.sh)

**Commit:** `9abf5d8` — "checkpoint 3c: manager canister with health watcher + cycles bursar"

## ✅ Checkpoint 4 — VPS provisioned (DONE)

What we have:
- Ubuntu 24.04 on srv825251.hstgr.cloud
- Rust 1.95, dfx 0.32, Node 20, Docker 29 installed
- ufw firewall with only 22/80/443/4001 open
- SSH key authentication working
- Project deployed to /opt/mycloud with full git history
- IPFS Kubo container running via docker-compose (mycloud-ipfs)
  - Swarm: 0.0.0.0:4001/tcp + 4001/udp (libp2p peering)
  - RPC API: 127.0.0.1:9600 (localhost-only, root-equivalent access)
  - Gateway: 127.0.0.1:9601 (localhost-only, system Nginx proxies)
- System Nginx (apt 1.24) routing srv825251.hstgr.cloud → IPFS gateway
- Let's Encrypt TLS cert for srv825251.hstgr.cloud, ECDSA key, 90-day
  auto-renewal alongside Crystal Dragon + Hope & Grace certs
- Public IPFS gateway tested end-to-end:
  https://srv825251.hstgr.cloud/ipfs/<cid> returns content over TLS
- Production sites (crystaldragon.tech, hopeandgrace.space) confirmed
  unaffected throughout

Architectural note: the VPS is shared with Crystal Dragon and Hope &
Grace. MyCloud's docker-compose runs IPFS only — system Nginx (already
present) handles all TLS termination and domain routing. See
`docs/OPERATIONS.md` "shared VPS" section.

**Commit:** TBD on next push — "checkpoint 4: phase A shipped (IPFS Kubo + Nginx + TLS)"

## Checkpoint 5 — End-to-end
- Dashboard builds and deploys as an asset canister
- Internet Identity login from dashboard
- Register a smartsite via dashboard → registry canister
- Pin a file to local IPFS, register the CID
- Visit https://srv825251.hstgr.cloud/ipfs/<cid> and see the content

## ⏭️ Checkpoint 4.5 — Hope & Grace canister (PRIORITY — next)

First external project canister deployment. Hope & Grace is going to
live battle testing this week with real bank account, real donations,
real users. The hopeandgrace canister provides the immutable ceremony
ledger and legal-doc registry that powers the Ripples of Compassion
public transparency page.

Spec locked in `docs/HOPEANDGRACE_INTEGRATION_SPEC.md` (June 7, 2026).
Six architectural decisions made in response to Hope & Grace Claude's
integration handoff:

- Integer cents on-chain (no floats)
- Story text on IPFS via CID + hash on chain (right-to-be-forgotten path)
- New dedicated `hopeandgrace` canister (not a module on existing ones)
- Owner + Vec<writers> access model
- Hall of Angels handle resolution stays off-chain
- Ripples reads canister directly via @dfinity/agent

Build phases (~10 hours total across MyCloud + Hope & Grace sides):
1. Spec lock — DONE
2. Canister implementation (Rust, ~4 hours fresh session)
3. Deploy to mainnet (~1 hour, ~$5 USD cycles)
4. Hope & Grace side daily archive job (~2 hours, H&G Claude's work)
5. Ripples page on hopeandgrace.space (~3 hours, can parallel)
6. Legal doc publishing UI (~1 hour, low priority)

## Checkpoint 5 — Dashboard MVP (DEFERRED behind 4.5)

See `docs/DASHBOARD_PLAN.md`. Now will read hopeandgrace canister
as its first real data source rather than the empty registry.