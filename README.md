# MyCloud

> A sovereign hybrid cloud — own your trust layer, own your bytes, own your future.

MyCloud is a self-hosted infrastructure stack that combines **Internet
Computer Protocol (ICP) canisters** for trustless identity and metadata
with a **Hostinger VPS** running **IPFS (Kubo)**, **Nginx**, and modular
Docker-based AI agents. It's designed for personal apps, "smartsites"
(decentralized sites with NFT-based ownership), and any project that
benefits from putting trust on-chain while keeping bytes affordable.

## What this is good for

MyCloud is a foundation, not a finished product. Here are concrete use
cases it's been designed around:

### Personal sovereign hosting
You run your own websites, your identity is an Internet Identity
principal, your credentials live in an on-chain encrypted vault, and
your VPS hosts the actual content. No platform can deplatform you —
your domain points at infrastructure you control, and your trust
relationships are recorded on a decentralized blockchain.

### NFT-gated "smartsites"
Mint an NFT on Solana (or Polygon, Ethereum, Base, Arbitrum, Optimism
— any EVM chain). The NFT becomes a key to claim a permanent subdomain
on your platform. Ownership is verified trustlessly via HTTP outcalls
from the canister to the chain's RPC. Sell the NFT → ownership
transfers automatically. This is the architecture behind
[Crystal Dragon](https://github.com/CaptFascin8) and similar projects.

### Nonprofit transparency
Publish aggregate statistics (donations received, distributions made,
ceremonies completed) to a public canister that anyone can query.
Donors get a tamper-proof public ledger separate from your operational
database. Auditors can verify any claim against on-chain state.

### Nomadic AI agents
An AI agent that travels between hardware (a Raspberry Pi today, a
laptop tomorrow, a server next month) needs a persistent identity
that doesn't depend on the host machine. MyCloud's `auth` canister
issues that identity — one Internet Identity principal that follows
the agent across devices.

### Multi-project home (the "cloud factory")
One VPS + one ICP infrastructure stack hosting many small projects,
each with its own canister. A unified routing layer (Agent Zero) lets
n8n workflows trigger canister methods across all projects through a
single endpoint. See [docs/CLOUD_FACTORY.md](docs/CLOUD_FACTORY.md).

### Educational
Hands-on Rust + ICP + IPFS + multi-chain NFT learning project.
Touches stable storage, inter-canister calls, async timers,
HTTP outcalls, secondary indexes, trait-based dispatch, and
distributed-systems debugging. The commit history is a curriculum.

## Architecture at a glance

```
                     ┌──────────────────────────┐
                     │   Internet Computer (ICP)│
                     │                          │
                     │   ┌──────┐ ┌──────────┐  │
                     │   │ auth │ │ registry │  │
                     │   └──────┘ └──────────┘  │
                     │      ┌────────┐          │
                     │      │ manager│          │
                     │      └────────┘          │
                     └────────────┬─────────────┘
                                  │
                       canister state replicated
                       across 13 nodes per subnet
                                  │
                                  ▼
        ┌─────────────────────────────────────────────┐
        │   Hostinger VPS (sovereign storage + serve) │
        │                                             │
        │   IPFS Kubo  →  Nginx (TLS) →  Internet     │
        │      ▲                                      │
        │      │                                      │
        │      └─ pinned content (CIDs registered     │
        │         in the registry canister)           │
        └─────────────────────────────────────────────┘
                          ▲
                          │
                Browser / API client / agent
```

The canisters store trust-critical data (who owns what, who's healthy,
who's authorized). The VPS stores content-addressed bytes via IPFS
and serves them through Nginx with Let's Encrypt TLS. Neither half
trusts the other — they cross-verify via cryptographic identifiers.

For a deeper architecture walkthrough, see
[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Status

**Three of three core canisters are real, deployed, and tested.**

| Layer | Status | What it does |
|-------|--------|--------------|
| `auth` canister | ✅ shipped | Internet Identity binding + per-user encrypted credential vault |
| `registry` canister | ✅ shipped | Smartsite CRUD + chain-agnostic OwnershipVerifier trait |
| `manager` canister | ✅ shipped | Periodic health watcher + cycles bursar with ring-buffer event log |
| VPS provisioning | partial | Toolchain installed (Rust, dfx, Node, Docker); IPFS + Nginx pending |
| Frontend dashboard | scaffold | Vite + React skeleton, real UI pending |
| HTTP outcall verifiers | stubbed | Trait shape exists; Solana/Polygon RPC implementations pending |

See [docs/PROOF_PLAN.md](docs/PROOF_PLAN.md) for the full checkpoint
roadmap.

## Quick start (for cloning + trying it yourself)

This project is designed to be fork-and-deploy for anyone. You'll need:

- **Ubuntu 24.04** (WSL2 is fine for *building*, but `dfx` development is
  most reliable on a real Linux machine — see the WSL caveat in
  [docs/OPERATIONS.md](docs/OPERATIONS.md))
- **Rust stable** (1.83+) with `wasm32-unknown-unknown` target
- **dfx 0.32+** (DFINITY's IC SDK)
- **Node.js 20 LTS**
- **Docker + docker-compose** (for VPS-side services)
- **A Hostinger Pro Cloud VPS** (or any Ubuntu 24.04 server with root SSH)

Install Rust + dfx:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
rustup target add wasm32-unknown-unknown

sh -ci "$(curl -fsSL https://internetcomputer.org/install.sh)"
```

Clone + build:

```bash
git clone https://github.com/CaptFascin8/mycloud.git
cd mycloud
cargo check --workspace
```

Deploy to a local replica:

```bash
dfx start --background --clean
dfx deploy --network local auth
dfx deploy --network local registry
dfx deploy --network local manager
```

Run the integration tests:

```bash
bash scripts/test_auth.sh
bash scripts/test_registry.sh
bash scripts/test_manager.sh
```

Each script exercises every public method on its canister and prints
"all <name> integration tests passed" on success.

For step-by-step VPS setup, see [docs/SETUP.md](docs/SETUP.md). For
day-to-day operations, see [docs/OPERATIONS.md](docs/OPERATIONS.md).

## Project structure

```
mycloud/
├── README.md                     This file
├── LICENSE                       Apache 2.0
├── Cargo.toml                    Rust workspace (3 canister crates)
├── rust-toolchain.toml           Pinned Rust stable
├── dfx.json                      Canister declarations + networks
├── .env.example                  Template for local config
├── bootstrap.sh                  One-shot project scaffold script
├── backend/
│   ├── auth/                     Identity + credential vault
│   ├── registry/                 Smartsites with multi-chain ownership
│   └── manager/                  Health watcher + cycles bursar
├── frontend/dashboard/           Vite + React UI (scaffold)
├── vps/
│   ├── docker/                   IPFS Kubo + Nginx compose stack
│   └── agents/                   Modular Docker agents (planned)
├── scripts/
│   ├── test_auth.sh              Integration tests per canister
│   ├── test_registry.sh
│   └── test_manager.sh
└── docs/
    ├── ARCHITECTURE.md           What MyCloud is and why
    ├── OPERATIONS.md             Daily operator's manual
    ├── PROOF_PLAN.md             Checkpoint progress
    ├── PORTS.md                  Port assignments (96xx block)
    ├── ECOSYSTEM.md              How MyCloud relates to other projects
    ├── CLOUD_FACTORY.md          Multi-project canister architecture
    └── SETUP.md                  Toolchain install steps
```

## Tech stack (pinned)

| Component | Version | Notes |
|-----------|---------|-------|
| Ubuntu | 24.04 LTS | VPS OS |
| Rust | stable (1.83+) | pinned via `rust-toolchain.toml` |
| ic-cdk | 0.17 | ICP Rust CDK |
| ic-cdk-timers | 0.11 | for the manager's periodic tick |
| ic-stable-structures | 0.6 | BTreeMap on stable memory |
| candid | 0.10 | IDL for canister interfaces |
| dfx | 0.32+ via dfxvm | ICP SDK |
| Kubo (IPFS) | latest stable | reference IPFS implementation |
| Docker Compose | v2 | VPS service orchestration |
| Node.js | 20 LTS | for the dashboard build |

## Documentation

- **[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)** — what MyCloud is, why it's hybrid, the three-layer model
- **[docs/OPERATIONS.md](docs/OPERATIONS.md)** — how to access, run, troubleshoot
- **[docs/PROOF_PLAN.md](docs/PROOF_PLAN.md)** — checkpoint-by-checkpoint progress
- **[docs/CLOUD_FACTORY.md](docs/CLOUD_FACTORY.md)** — long-term multi-project + Agent Zero pattern
- **[docs/PORTS.md](docs/PORTS.md)** — port allocations on the VPS
- **[docs/ECOSYSTEM.md](docs/ECOSYSTEM.md)** — how MyCloud feeds Crystal Dragon, Agentic Acres, Hope & Grace
- **[docs/SETUP.md](docs/SETUP.md)** — exact install commands

## Contributing

This project is currently developed as a personal infrastructure stack
with a learning goal. Issues, ideas, and pull requests are welcome,
but understand that:

- The default branch is `main`. PRs target it.
- The architecture decisions in `docs/CLOUD_FACTORY.md` and
  `docs/ARCHITECTURE.md` are deliberate — proposed changes that
  reorganize them need a rationale tied to a concrete use case.
- Cosmetic warnings in the auth canister (Cow lifetime hints from
  Rust 1.95) are intentionally carried; they're not bugs.

## License

[Apache 2.0](LICENSE) — use it, fork it, build on top of it. The
patent-grant clause protects you (and contributors) from patent claims
on the code itself.

## Acknowledgments

- **DFINITY** for the Internet Computer Protocol and the dfx SDK
- **Protocol Labs** for IPFS and Kubo
- **Anthropic** for Claude, who pair-programmed this entire project
  one bash heredoc at a time. The conversation is the curriculum.
- **Hostinger** for the surprisingly capable Pro Cloud VPS that
  hosts the production half
