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

## ⏭️ Checkpoint 3b — registry canister (NEXT)
- Smartsite CRUD with stable storage
- OwnershipVerifier trait with one working impl (InternetIdentity)
- Stub impls for SolanaNft/EthereumNft returning Err("not implemented")
- Trait shape ready for Crystal Dragon Yggdrasil KEY tier verification later
- Integration test scripts/test_registry.sh

## Checkpoint 3c — manager canister
- ic-cdk-timers periodic tick (every 60s)
- Ring buffer of HealthEvents (last 100, queryable by dashboard)
- Polls auth + registry health_check() via inter-canister calls
- Cycle balance threshold warnings
- HTTP outcall stub for future "self-healing" agent

## Checkpoint 4 — VPS provisioned (PARTIALLY DONE)
What we already have:
- Ubuntu 24.04 on srv825251.hstgr.cloud
- Rust 1.95, dfx 0.32, Node 20, Docker 29 installed
- ufw firewall with only 22/80/443/4001 open
- SSH key authentication working
- Project deployed to /opt/mycloud with full git history

What's left:
- IPFS Kubo container running (`docker compose up -d`)
- Nginx with Let's Encrypt TLS for srv825251.hstgr.cloud
- End-to-end `curl https://srv825251.hstgr.cloud/ipfs/<known-cid>` test

## Checkpoint 5 — End-to-end
- Dashboard builds and deploys as an asset canister
- Internet Identity login from dashboard
- Register a smartsite via dashboard → registry canister
- Pin a file to local IPFS, register the CID
- Visit https://srv825251.hstgr.cloud/ipfs/<cid> and see the content

## Future milestones
- **Solana NFT verification** — registry's OwnershipProof::SolanaNft becomes trustlessly verifiable via HTTP outcalls to Solana RPC
- **Project H.O.P.E.** — first agent under vps/agents/hope/
- **Self-healing manager** — when a health check fails, manager makes an HTTP outcall to a healer agent that has Docker socket access and can `docker restart mycloud-ipfs`
- **GitHub Actions CI** — every push runs cargo check, cargo test, dfx build automatically
- **Migration to icp-cli** — when dfx 0.32 is fully replaced