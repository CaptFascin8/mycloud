# MyCloud Proof Plan

Five checkpoints, each a "stop, verify, proceed" gate.

## Checkpoint 1 — Scaffold
- [x] Directory tree at /mnt/c/MY_CLOUD/
- [x] Cargo workspace + rust-toolchain.toml
- [x] dfx.json declares all 3 canisters + Internet Identity + dashboard
- [x] Each canister has Cargo.toml, src/lib.rs, .did
- [x] vps/docker/docker-compose.yml defines IPFS + Nginx
- [x] frontend/dashboard has Vite skeleton

**Verify:** `git init && git add -A && git status` shows the tree.

## Checkpoint 2 — Canisters compile
**Done when:** `cargo check --workspace` passes with zero errors.

**Verify:**
```bash
cd /mnt/c/MY_CLOUD
cargo check --workspace
```

## Checkpoint 3 — Real canister logic
- auth: stable storage for users + credential vault
- registry: smartsite CRUD with pluggable ownership verifier trait
- manager: ic-cdk-timers periodic task + ring-buffer HealthEvents
- Rust unit tests in `#[cfg(test)]` modules
- scripts/integration_tests.sh exercises every public method via dfx

## Checkpoint 4 — VPS provisioned
- SSH hardened, ufw enabled (22, 80, 443, 4001 only)
- Docker + compose installed
- IPFS Kubo running, server profile, API localhost only
- Let's Encrypt cert for srv825251.hstgr.cloud
- `curl https://srv825251.hstgr.cloud/ipfs/<known-cid>` returns content

## Checkpoint 5 — End-to-end
- Dashboard builds (`npm run build`)
- `dfx deploy --network ic` (or stay on local)
- Internet Identity login -> registry.register_site -> file pinned to VPS IPFS
