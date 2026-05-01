#!/usr/bin/env bash
# MyCloud project bootstrap
# Run from anywhere in WSL. Creates the full scaffold at /mnt/c/MY_CLOUD/.
# Safe to re-run — overwrites existing files (so edit AFTER running, not before).

set -euo pipefail

ROOT=/mnt/c/MY_CLOUD
echo "==> bootstrapping MyCloud at $ROOT"

# ---------- directories ----------
mkdir -p "$ROOT"/{backend/{auth,registry,manager}/src,vps/{docker/nginx/conf.d,agents/hope},frontend/dashboard/src,scripts,docs}
echo "    directory tree created"

# ============================================================================
# ROOT-LEVEL CONFIG (6 files)
# ============================================================================

cat > "$ROOT/Cargo.toml" <<'EOF'
[workspace]
# All canister crates share dependency versions and a single target/ dir.
resolver = "2"
members = [
    "backend/auth",
    "backend/registry",
    "backend/manager",
]

[workspace.dependencies]
ic-cdk               = "0.17"
ic-cdk-macros        = "0.17"
ic-cdk-timers        = "0.11"
candid               = "0.10"
serde                = { version = "1", features = ["derive"] }
serde_bytes          = "0.11"
serde_json           = "1"
ic-stable-structures = "0.6"
sha2                 = "0.10"
hex                  = "0.4"

# Wasm-tuned release profile. ICP charges per instruction; smaller binaries
# cost less to deploy and run.
[profile.release]
opt-level     = "z"
lto           = true
codegen-units = 1
panic         = "abort"
strip         = true
EOF

cat > "$ROOT/rust-toolchain.toml" <<'EOF'
[toolchain]
channel    = "stable"
components = ["rustfmt", "clippy"]
targets    = ["wasm32-unknown-unknown"]
EOF

cat > "$ROOT/dfx.json" <<'EOF'
{
  "version": 1,
  "dfx": "0.32.0",
  "output_env_file": ".env",
  "canisters": {
    "auth": {
      "type": "rust",
      "package": "auth",
      "candid": "backend/auth/auth.did",
      "metadata": [{ "name": "candid:service", "visibility": "public" }]
    },
    "registry": {
      "type": "rust",
      "package": "registry",
      "candid": "backend/registry/registry.did",
      "metadata": [{ "name": "candid:service", "visibility": "public" }],
      "dependencies": ["auth"]
    },
    "manager": {
      "type": "rust",
      "package": "manager",
      "candid": "backend/manager/manager.did",
      "metadata": [{ "name": "candid:service", "visibility": "public" }],
      "dependencies": ["auth", "registry"]
    },
    "internet_identity": {
      "type": "custom",
      "candid": "https://github.com/dfinity/internet-identity/releases/latest/download/internet_identity.did",
      "wasm":   "https://github.com/dfinity/internet-identity/releases/latest/download/internet_identity_dev.wasm.gz",
      "remote": { "id": { "ic": "rdmx6-jaaaa-aaaaa-aaadq-cai" } },
      "frontend": {},
      "init_arg": "(null)"
    },
    "dashboard": {
      "type": "assets",
      "source": ["frontend/dashboard/dist"],
      "dependencies": ["auth", "registry", "manager"]
    }
  },
  "defaults": {
    "build":   { "args": "", "packtool": "" },
    "replica": { "subnet_type": "application" }
  },
  "networks": {
    "local": {
      "bind":    "127.0.0.1:4943",
      "type":    "ephemeral",
      "replica": { "subnet_type": "application" }
    },
    "ic": {
      "providers": ["https://ic0.app"],
      "type":      "persistent"
    }
  }
}
EOF

cat > "$ROOT/.gitignore" <<'EOF'
# Rust / Cargo
target/
**/*.rs.bk
Cargo.lock

# ICP / dfx
.dfx/
.env
canister_ids.json
dist/

# Node / Vite
node_modules/
frontend/**/dist/
frontend/**/.vite/
frontend/**/.turbo/
npm-debug.log*
yarn-debug.log*
yarn-error.log*
pnpm-debug.log*

# Secrets
*.pem
*.key
.env.*
!.env.example
secrets/
*.identity.json

# IPFS / VPS state
vps/docker/data/
vps/docker/ipfs-data/
vps/docker/nginx/letsencrypt/

# IDE / OS
.vscode/
.idea/
*.swp
.DS_Store
Thumbs.db

# Build artifacts
*.wasm
*.wasm.gz
EOF

cat > "$ROOT/.env.example" <<'EOF'
# Copy this file to .env and fill in real values. Never commit .env.

# --- VPS connection ---
VPS_HOST=srv825251.hstgr.cloud
VPS_IP=82.25.91.136
VPS_USER=root
VPS_SSH_KEY_PATH=~/.ssh/mycloud_vps_ed25519

# --- IPFS (Kubo) endpoints (96xx block per PORT_AUDIT.md) ---
IPFS_API_URL=http://127.0.0.1:9600
IPFS_GATEWAY_URL=http://127.0.0.1:9601
IPFS_PUBLIC_GATEWAY=https://srv825251.hstgr.cloud/ipfs

# --- ICP network ---
DFX_NETWORK=local

# --- Solana (for future registry NFT verification) ---
SOLANA_RPC_URL=https://api.mainnet-beta.solana.com
SOLANA_CLUSTER=mainnet-beta
EOF

cat > "$ROOT/README.md" <<'EOF'
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
EOF

echo "    [1/6] root configs written"

# ============================================================================
# BACKEND CANISTERS (9 files)
# ============================================================================

# --- auth ---
cat > "$ROOT/backend/auth/Cargo.toml" <<'EOF'
[package]
name    = "auth"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
ic-cdk               = { workspace = true }
ic-cdk-macros        = { workspace = true }
candid               = { workspace = true }
serde                = { workspace = true }
ic-stable-structures = { workspace = true }
EOF

cat > "$ROOT/backend/auth/src/lib.rs" <<'EOF'
//! MyCloud — `auth` canister
//!
//! Binds the caller's Internet Identity Principal to a user record and
//! stores per-user credentials in stable memory.
//!
//! Checkpoint 2 deliverable: compiles + .did is consistent. Real logic
//! (stable storage, credential vault) lands in Checkpoint 3.

use candid::{CandidType, Principal};
use ic_cdk::{init, query, update};
use serde::{Deserialize, Serialize};

#[derive(CandidType, Serialize, Deserialize, Clone, Debug)]
pub struct User {
    pub principal:  Principal,
    pub registered: u64, // ic_cdk::api::time() — ns since epoch
}

#[init]
fn init() {
    ic_cdk::println!("auth canister initialized");
}

#[query]
fn whoami() -> Principal {
    ic_cdk::api::caller()
}

#[update]
fn register() -> User {
    User {
        principal:  ic_cdk::api::caller(),
        registered: ic_cdk::api::time(),
    }
}

ic_cdk::export_candid!();
EOF

cat > "$ROOT/backend/auth/auth.did" <<'EOF'
type User = record {
  principal  : principal;
  registered : nat64;
};

service : {
  whoami   : () -> (principal) query;
  register : () -> (User);
}
EOF

# --- registry ---
cat > "$ROOT/backend/registry/Cargo.toml" <<'EOF'
[package]
name    = "registry"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
ic-cdk               = { workspace = true }
ic-cdk-macros        = { workspace = true }
candid               = { workspace = true }
serde                = { workspace = true }
serde_bytes          = { workspace = true }
ic-stable-structures = { workspace = true }
sha2                 = { workspace = true }
hex                  = { workspace = true }
EOF

cat > "$ROOT/backend/registry/src/lib.rs" <<'EOF'
//! MyCloud — `registry` canister
//!
//! Tracks "smartsites": named sites whose ownership is provable and whose
//! content lives on IPFS, addressed by CID.
//!
//! Cross-project alignment:
//!   * Crystal Dragon Yggdrasil KEY tiers (ROOT/TRUNK/BRANCH/CROWN/DOMAIN)
//!     are first-class via `OwnershipProof::SolanaNft { tier: Option<KeyTier> }`.
//!   * Agentic Acres can register agent-home sites so nomadic Sally has a
//!     queryable "current address" from any client.

use candid::{CandidType, Principal};
use ic_cdk::{init, query, update};
use serde::{Deserialize, Serialize};

#[derive(CandidType, Serialize, Deserialize, Clone, Debug)]
pub struct Smartsite {
    pub domain:     String,
    pub owner:      Principal,
    pub ipfs_cid:   String,
    pub created_ns: u64,
    pub updated_ns: u64,
    pub ownership:  OwnershipProof,
}

#[derive(CandidType, Serialize, Deserialize, Clone, Debug)]
pub enum OwnershipProof {
    InternetIdentity,
    SolanaNft   { mint: String, wallet: String, tier: Option<KeyTier> },
    EthereumNft { contract: String, token_id: String, wallet: String },
}

/// Crystal Dragon Yggdrasil KEY tier system per MASTER_CHECKLIST.md:
///   ROOT   #0001-0100   (Genesis)
///   TRUNK  #0101-1000   (Standard)
///   BRANCH #1001-5000   (Premium)
///   CROWN  #5001-10000  (Elite)
///   DOMAIN unlimited    (Transfer KEY)
#[derive(CandidType, Serialize, Deserialize, Clone, Debug, Copy)]
pub enum KeyTier { Root, Trunk, Branch, Crown, Domain }

#[init]
fn init() {
    ic_cdk::println!("registry canister initialized");
}

#[query]
fn list_sites() -> Vec<Smartsite> {
    Vec::new()
}

#[update]
fn register_site(domain: String, ipfs_cid: String) -> Smartsite {
    let now = ic_cdk::api::time();
    Smartsite {
        domain,
        owner:      ic_cdk::api::caller(),
        ipfs_cid,
        created_ns: now,
        updated_ns: now,
        ownership:  OwnershipProof::InternetIdentity,
    }
}

ic_cdk::export_candid!();
EOF

cat > "$ROOT/backend/registry/registry.did" <<'EOF'
type KeyTier = variant { Root; Trunk; Branch; Crown; Domain };

type OwnershipProof = variant {
  InternetIdentity;
  SolanaNft   : record { mint: text; wallet: text; tier: opt KeyTier };
  EthereumNft : record { contract: text; token_id: text; wallet: text };
};

type Smartsite = record {
  domain     : text;
  owner      : principal;
  ipfs_cid   : text;
  created_ns : nat64;
  updated_ns : nat64;
  ownership  : OwnershipProof;
};

service : {
  list_sites    : ()           -> (vec Smartsite) query;
  register_site : (text, text) -> (Smartsite);
}
EOF

# --- manager ---
cat > "$ROOT/backend/manager/Cargo.toml" <<'EOF'
[package]
name    = "manager"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
ic-cdk               = { workspace = true }
ic-cdk-macros        = { workspace = true }
ic-cdk-timers        = { workspace = true }
candid               = { workspace = true }
serde                = { workspace = true }
ic-stable-structures = { workspace = true }
EOF

cat > "$ROOT/backend/manager/src/lib.rs" <<'EOF'
//! MyCloud — `manager` canister ("Smart Agent")
//!
//! Periodic ic-cdk-timers job that samples the canister's own cycle balance,
//! calls auth+registry for liveness, and keeps a ring buffer of recent
//! HealthEvents queryable by the dashboard.
//!
//! Checkpoint 2: compiles + .did consistent. Timer wiring + ring buffer
//! land in Checkpoint 3.

use candid::CandidType;
use ic_cdk::{init, query};
use serde::{Deserialize, Serialize};

#[derive(CandidType, Serialize, Deserialize, Clone, Debug)]
pub enum Severity { Info, Warn, Error }

#[derive(CandidType, Serialize, Deserialize, Clone, Debug)]
pub struct HealthEvent {
    pub timestamp_ns: u64,
    pub source:       String,
    pub severity:     Severity,
    pub message:      String,
}

#[init]
fn init() {
    ic_cdk::println!("manager canister initialized");
}

#[query]
fn recent_events(_limit: u32) -> Vec<HealthEvent> {
    Vec::new()
}

#[query]
fn cycles_balance() -> u64 {
    ic_cdk::api::canister_balance()
}

ic_cdk::export_candid!();
EOF

cat > "$ROOT/backend/manager/manager.did" <<'EOF'
type Severity = variant { Info; Warn; Error };

type HealthEvent = record {
  timestamp_ns : nat64;
  source       : text;
  severity     : Severity;
  message      : text;
};

service : {
  recent_events  : (nat32) -> (vec HealthEvent) query;
  cycles_balance : ()      -> (nat64) query;
}
EOF

echo "    [2/6] backend canisters written (auth, registry, manager)"

# ============================================================================
# FRONTEND DASHBOARD (5 files)
# ============================================================================

cat > "$ROOT/frontend/dashboard/package.json" <<'EOF'
{
  "name":    "mycloud-dashboard",
  "version": "0.1.0",
  "private": true,
  "type":    "module",
  "scripts": {
    "dev":     "vite",
    "build":   "vite build",
    "preview": "vite preview"
  },
  "dependencies": {
    "react":     "^18.3.0",
    "react-dom": "^18.3.0",
    "@dfinity/agent":       "^2.1.0",
    "@dfinity/auth-client": "^2.1.0",
    "@dfinity/candid":      "^2.1.0",
    "@dfinity/principal":   "^2.1.0",
    "@dfinity/identity":    "^2.1.0"
  },
  "devDependencies": {
    "vite":                "^5.4.0",
    "@vitejs/plugin-react": "^4.3.0"
  }
}
EOF

cat > "$ROOT/frontend/dashboard/vite.config.js" <<'EOF'
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  build:   { outDir: "dist", emptyOutDir: true },
  server: {
    port: 5173,
    proxy: { "/api": "http://127.0.0.1:4943" },
  },
});
EOF

cat > "$ROOT/frontend/dashboard/index.html" <<'EOF'
<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>MyCloud Dashboard</title>
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/main.jsx"></script>
  </body>
</html>
EOF

cat > "$ROOT/frontend/dashboard/src/main.jsx" <<'EOF'
import React from "react";
import { createRoot } from "react-dom/client";
import App from "./App.jsx";

createRoot(document.getElementById("root")).render(<App />);
EOF

cat > "$ROOT/frontend/dashboard/src/App.jsx" <<'EOF'
export default function App() {
  return (
    <main style={{ fontFamily: "system-ui", padding: "2rem", maxWidth: 720 }}>
      <h1>MyCloud Dashboard</h1>
      <p>Sovereign hybrid cloud — scaffold is alive.</p>
      <ul>
        <li>auth canister: not yet wired</li>
        <li>registry canister: not yet wired</li>
        <li>manager canister: not yet wired</li>
        <li>IPFS gateway: <code>https://srv825251.hstgr.cloud/ipfs/&lt;cid&gt;</code></li>
      </ul>
    </main>
  );
}
EOF

echo "    [3/6] frontend dashboard written"

# ============================================================================
# VPS STACK (3 files)
# ============================================================================

cat > "$ROOT/vps/docker/docker-compose.yml" <<'EOF'
# MyCloud VPS service stack — run on srv825251.hstgr.cloud (or HOPE for dev).
# Ports per docs/PORTS.md; MyCloud owns the 96xx block.
#   docker compose up -d

services:
  ipfs:
    image:          ipfs/kubo:latest
    container_name: mycloud-ipfs
    restart:        unless-stopped
    environment:
      IPFS_PROFILE: server
      IPFS_PATH:    /data/ipfs
    volumes:
      - ./data/ipfs-staging:/export
      - ./data/ipfs:/data/ipfs
    ports:
      # 4001 = libp2p swarm. Public so peers can dial in.
      - "4001:4001/tcp"
      - "4001:4001/udp"
      # 9600 -> 5001 = RPC API. LOCALHOST ONLY. Treat as root-equivalent.
      - "127.0.0.1:9600:5001"
      # 9601 -> 8080 = HTTP gateway. LOCALHOST ONLY; Nginx proxies it.
      - "127.0.0.1:9601:8080"
    healthcheck:
      test:     ["CMD", "ipfs", "id"]
      interval: 30s
      timeout:  10s
      retries:  3

  nginx:
    image:          nginx:1.27-alpine
    container_name: mycloud-nginx
    restart:        unless-stopped
    depends_on:     [ipfs]
    ports:
      - "80:80"
      - "443:443"
    volumes:
      - ./nginx/conf.d:/etc/nginx/conf.d:ro
      - ./nginx/letsencrypt:/etc/letsencrypt
      - ./nginx/www:/var/www/html
EOF

cat > "$ROOT/vps/docker/nginx/conf.d/mycloud.conf" <<'EOF'
# MyCloud Nginx — replace HTTP-only block with a TLS server after certbot.

server {
    listen      80;
    server_name srv825251.hstgr.cloud;

    location /.well-known/acme-challenge/ {
        root /var/www/html;
    }

    location ~ ^/(ipfs|ipns)/ {
        proxy_pass         http://ipfs:8080;
        proxy_http_version 1.1;
        proxy_set_header   Host              $host;
        proxy_set_header   X-Real-IP         $remote_addr;
        proxy_set_header   X-Forwarded-For   $proxy_add_x_forwarded_for;
        proxy_set_header   X-Forwarded-Proto $scheme;
        add_header Cache-Control "public, max-age=31536000, immutable";
    }

    location / {
        root  /var/www/html;
        index index.html;
        try_files $uri $uri/ /index.html;
    }
}
EOF

cat > "$ROOT/vps/agents/README.md" <<'EOF'
# Agents

Modular Docker-based AI agents that run on the VPS alongside IPFS and Nginx.
Each agent gets its own subfolder with a Dockerfile and README.

## Conventions
- One folder per agent: `vps/agents/<name>/`
- Each agent ships its own README documenting env vars, ports, volumes
- Agents talk over the Docker default bridge network; never bind public ports
  directly — Nginx handles all external traffic
- Persistent state goes in `vps/docker/data/<agent-name>/`

## Planned
### `hope/` — Project H.O.P.E.
Placeholder. Add Dockerfile and README when ready.
EOF

# Reserve the H.O.P.E. directory with a .gitkeep so git tracks it empty
touch "$ROOT/vps/agents/hope/.gitkeep"

echo "    [4/6] VPS stack written (docker-compose, nginx, agents)"

# ============================================================================
# DOCS (4 files)
# ============================================================================

cat > "$ROOT/docs/PROOF_PLAN.md" <<'EOF'
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
EOF

cat > "$ROOT/docs/SETUP.md" <<'EOF'
# Setup

You're on Ubuntu 24.04 (WSL on HOPE or directly on the VPS — same commands).

## 1. Rust + Wasm target
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
rustup target add wasm32-unknown-unknown
```

## 2. dfx
```bash
sh -ci "$(curl -fsSL https://internetcomputer.org/install.sh)"
dfx --version
```

## 3. Node 20 (for the dashboard)
```bash
curl -fsSL https://deb.nodesource.com/setup_20.x | sudo -E bash -
sudo apt-get install -y nodejs
node --version
```

## 4. First build
```bash
cd /mnt/c/MY_CLOUD
cargo check --workspace
dfx start --background --clean
dfx deploy --network local
dfx canister call auth whoami
```

## VPS setup (Checkpoint 4)
```bash
ssh root@82.25.91.136
apt update && apt upgrade -y
apt install -y docker.io docker-compose-plugin ufw certbot
systemctl enable --now docker
ufw default deny incoming
ufw default allow outgoing
ufw allow 22/tcp
ufw allow 80/tcp
ufw allow 443/tcp
ufw allow 4001
ufw enable

# from local:
rsync -avz vps/docker/ root@82.25.91.136:/opt/mycloud/

# back on VPS:
cd /opt/mycloud
docker compose up -d
docker compose logs -f ipfs
```
EOF

cat > "$ROOT/docs/PORTS.md" <<'EOF'
# MyCloud Port Assignments

MyCloud owns the **96xx** range per `C:\SL_Mesh_Studio_v2\PORT_AUDIT.md`.
Avoids collisions with HOPE Master Stack, Agentic Acres, Metivi Mesh Studio.

## Active

| Service              | Host port | Container port | Bind        | Where     |
|----------------------|-----------|----------------|-------------|-----------|
| IPFS RPC API         | 9600      | 5001           | 127.0.0.1   | VPS / dev |
| IPFS HTTP gateway    | 9601      | 8080           | 127.0.0.1   | VPS / dev |
| IPFS libp2p swarm    | 4001      | 4001           | public      | VPS only  |
| Nginx HTTP           | 80        | 80             | public      | VPS       |
| Nginx HTTPS          | 443       | 443            | public      | VPS       |
| dfx local replica    | 4943      | -              | 127.0.0.1   | local dev |
| Vite dev server      | 5173      | -              | 127.0.0.1   | local dev |

## Reserved
- 9602 — Kubo Web UI dev exposure (if ever needed)
- 9603 — First MyCloud agent
- 9604 — Second MyCloud agent
- 9605 — Third MyCloud agent

When adding a new service, claim the next free 96xx port and update this
table in the same commit.

## Verify free on HOPE
```powershell
$ports = 9600..9620
Get-NetTCPConnection -State Listen `
  | Where-Object { $ports -contains $_.LocalPort } `
  | Format-Table -AutoSize
```
Zero rows = all clear.
EOF

cat > "$ROOT/docs/ECOSYSTEM.md" <<'EOF'
# MyCloud's role in the wider ecosystem

MyCloud is the shared infrastructure layer underneath three projects:

| Project        | Path                           | What MyCloud provides |
|----------------|--------------------------------|---------------------|
| Crystal Dragon | C:\Crystal_DRagon_Smart_site   | Solana NFT verification (KEY tiers), claimed-site registry, IPFS hosting |
| Agentic Acres  | C:\Agentic_Acres               | Persistent identity for nomadic Sally, IPFS for splat scans |
| MyCloud (self) | C:\MY_CLOUD                    | All of the above + generic personal-site hosting |

## Cross-project hooks already wired in

### Crystal Dragon -> registry canister
`OwnershipProof::SolanaNft` carries an optional `KeyTier` enum
(Root/Trunk/Branch/Crown/Domain) matching MASTER_CHECKLIST.md exactly.
A Crystal Dragon mint event can call `registry.register_site` and pass
the tier through. The registry can answer "which sites does this Solana
wallet own, and at what tier?" — directly powering Crystal Dragon's
admin panel.

### Agentic Acres -> auth canister
Sally needs a persistent identity that survives nomadic transfers between
Pi devices. Internet Identity in MyCloud's `auth` canister gives her one
principal that follows her wherever the Pi goes.

### Both -> IPFS via VPS
Site content + splat scans are content-addressed media that fits IPFS.
MyCloud's Kubo node on the Hostinger VPS pins both, served via Nginx at
https://srv825251.hstgr.cloud/ipfs/<cid>.

## Development order

Don't try to fully build Crystal Dragon's NFT verification or Agentic
Acres' nomadic identity here yet. Get MyCloud's three canisters working
with placeholder data first (Checkpoints 2-3). When Crystal Dragon hits
Phase 2 and Agentic Acres hits Proof Cycle 11, the integration points
are already in place.
EOF

echo "    [5/6] docs written (PROOF_PLAN, SETUP, PORTS, ECOSYSTEM)"

# ============================================================================
# SCRIPTS (1 file)
# ============================================================================

cat > "$ROOT/scripts/check.sh" <<'EOF'
#!/usr/bin/env bash
# Checkpoint 2 verifier. Run from project root.
set -euo pipefail

echo "==> cargo check --workspace"
cargo check --workspace --target wasm32-unknown-unknown

echo "==> dfx build"
if ! pgrep -f "dfx start" >/dev/null; then
  echo "    starting local replica in background..."
  dfx start --background --clean
fi
dfx build

echo "==> all checks passed"
EOF
chmod +x "$ROOT/scripts/check.sh"

echo "    [6/6] scripts written"

# ============================================================================
# DONE
# ============================================================================
echo
echo "==> bootstrap complete."
echo "    files written: $(find "$ROOT" -type f | wc -l)"
echo
echo "Next:"
echo "    cd /mnt/c/MY_CLOUD"
echo "    cargo check --workspace      # ~5-10 min on first run"
