# MyCloud — Sovereign Hybrid Cloud

A self-hosted hybrid cloud combining **Internet Computer Protocol (ICP)
canisters** for trustless identity/registry/health logic with a **Hostinger
VPS** running **IPFS (Kubo)**, **Nginx**, and modular Docker-based AI agents.
Designed to host personal apps and "smartsites" — decentralized sites with
NFT-based ownership, eventually verified on Solana.

## Components

### Backend canisters (`backend/`)
- **auth** — Internet Identity + per-principal credential vault
- **registry** — Smartsite metadata: domain -> owner -> IPFS CID. Carries a
  chain-agnostic `OwnershipProof` enum so Solana NFT verification (Crystal
  Dragon Yggdrasil KEY tiers) can plug in later without breaking the API.
- **manager** — "Smart Agent": health checks, cycle balance, error reporting

### VPS services (`vps/`)
- **docker/** — Compose stack: IPFS Kubo + Nginx (TLS termination)
- **agents/** — Modular Docker containers for AI agents (Project H.O.P.E. etc.)

### Frontend (`frontend/`)
- **dashboard/** — Vite + React UI

## Tech stack

| Component       | Version           |
|-----------------|-------------------|
| Ubuntu          | 24.04 LTS         |
| Rust            | stable (1.83+)    |
| ic-cdk          | 0.17              |
| candid          | 0.10              |
| dfx             | 0.32+ via dfxvm   |
| Kubo (IPFS)     | latest stable     |
| Docker Compose  | v2                |
| Node.js         | 20 LTS            |

## Quick start
See `docs/SETUP.md` for full steps. TL;DR for local dev:
```bash
dfx start --background --clean
dfx deploy --network local
```

## Status
**Checkpoint 1: Scaffold complete.** Next: `cargo check --workspace` ->
write real canister logic. See `docs/PROOF_PLAN.md`.
