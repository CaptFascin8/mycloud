# Cloud Factory — One Canister Per Project + Agent Zero Routing

> The long-term architecture for MyCloud as a generalized "puffy cloud"
> hosting many independent projects, each with its own canister, all
> reachable through a single Agent Zero router.

**Status:** Planned. Not yet built. Predicated on Checkpoints 3b, 3c, 4
shipping first. **Do not pre-build the routing layer.** Build canisters
first; the pattern emerges from their Candid interfaces.

---

## What this is

MyCloud's three core canisters (auth, registry, manager) are the
foundation. The "cloud factory" is what comes next: a repeatable pattern
for adding new projects as their own ICP canisters, with shared identity
(auth), shared registry (registry), shared health (manager), and a
unified routing layer (Agent Zero on the VPS) that lets external
triggers (n8n, webhooks, the dashboard) call any canister method by
name without knowing canister IDs.

```
                     ┌──────────────────────────┐
                     │   Agent Zero (VPS)       │
                     │   reads Candid IDLs,     │
                     │   picks the right        │
                     │   canister + method      │
                     │   port 9603              │
                     └────────────┬─────────────┘
                                  │
        ┌─────────────┬───────────┼───────────┬────────────┐
        ▼             ▼           ▼           ▼            ▼
  ┌──────────┐  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐
  │   auth   │  │ registry │ │  manager │ │ hopeand  │ │ metiverse│
  │ (MyCloud)│  │ (MyCloud)│ │ (MyCloud)│ │  grace   │ │          │
  └──────────┘  └──────────┘ └──────────┘ └──────────┘ └──────────┘
   identity     smartsites    health +     project H+G    metaverse
                              cycles mgmt   n8n hooks      n8n hooks
                                  ▲
                                  │
                              n8n triggers → Agent Zero → right canister
```

---

## The five design principles

### 1. One canister per project, not per feature

| Right | Wrong |
|------|------|
| `auth`, `registry`, `manager` (3 core) | `auth-users`, `auth-sessions`, `auth-tokens` |
| `hopeandgrace_main` (1 per project) | `hopeandgrace_users`, `_events`, `_donations` |
| `metiverse_main` | `metiverse_avatars`, `_rooms`, `_objects` |

Within a canister, separate concerns into Rust modules
(`mod users; mod sessions;`), not separate canisters. Canister creation
costs ~10 trillion cycles (~$10) plus ongoing storage; granular
canisters get expensive fast.

### 2. Manager handles cycles for the entire fleet

There is no separate "master" or "billing" canister. The MyCloud
`manager` canister owns:
- Periodic health checks of every other canister
- Cycle balance monitoring with low-balance alerts
- Automated top-ups: when a target canister drops below threshold,
  manager transfers cycles from its own balance
- A simple admin method `top_up(canister_id, amount)` for manual control

This means topping up the master is the only direct interaction needed.
The master tops up everything else. From DFINITY's documentation, this
pattern is called the **cycles management canister** and is a recommended
ICP architecture.

### 3. Every project canister exposes a small standard interface

Every canister built for the cloud factory implements a base trait:

```rust
trait CloudCanister {
    fn health_check() -> HealthStatus;          // for manager
    fn list_triggers() -> Vec<TriggerSpec>;     // for Agent Zero
    fn cycles_balance() -> u64;                 // for manager
    fn metadata() -> CanisterMetadata;          // version, project, owner
}
```

Plus the project-specific methods. This standard interface is what makes
Agent Zero's introspection work — it can discover what triggers a
canister offers without per-canister custom code.

We'll create a shared `mycloud-canister-template` Rust crate that
provides the boilerplate (stable storage setup, error types, the trait
above). New canister = `cargo new-canister-from-template <name>`.

### 4. Agent Zero is a VPS service, not a canister

Agent Zero runs as a Docker container on the Hostinger VPS, port 9603,
in `vps/agents/agent-zero/`. It's a Python service using `ic-py`.

**Why not on-chain?** Three reasons:
- The Agent Zero codebase is Python-native; rewriting in Rust/Motoko
  for ICP would be a from-scratch project, losing all upstream Agent
  Zero improvements
- LLM inference happens on the VPS anyway, so Agent Zero is colocated
  with where the actual reasoning happens
- Inter-canister calls cost cycles; calling from VPS to canister costs
  nothing extra beyond the canister's own execution

**Trade-off:** Agent Zero on the VPS means if the VPS is compromised,
an attacker with Agent Zero's principal can fire any trigger method.
Mitigated by (a) caller-principal verification on every trigger
method, (b) Agent Zero's principal is distinct and revocable, (c) all
trigger methods log their caller for audit.

### 5. NFT gating: use what you already have

Crystal Dragon's Yggdrasil KEYs are minted on Solana — that integration
goes in the registry canister regardless. For *MyCloud dashboard*
gating, use whatever NFTs you already hold; the `OwnershipVerifier`
trait supports multiple chains:

- `OwnershipProof::SolanaNft { mint, wallet, tier }` for Crystal Dragon KEYs
- `OwnershipProof::EthereumNft { contract, token_id, wallet }` for
  Polygon NFTs (Polygon is EVM-compatible, so Ethereum-shaped verifier
  works) or for any other EVM chain (Base, Arbitrum, Optimism)

The cost of "yet another chain" is mostly minting + maintaining the
collection. If you already minted Polygon NFTs you want to use, the
HTTP outcall to a Polygon RPC adds maybe 50 lines of Rust to the
verifier — not 50 lines per dashboard, just 50 lines once.

What we **don't** do: add a chain just to add it. If a feature can be
done with the chains we already integrate, it should be.

---

## Agent Zero — concrete deployment design

### Where it lives

```
vps/agents/agent-zero/
├── Dockerfile              # python:3.12-slim + ic-py
├── docker-compose.yml      # exposes port 9603, mounts /knowledge
├── agent_zero/
│   ├── main.py             # FastAPI server: /trigger endpoint
│   ├── icp_router.py       # the script you pasted, refined
│   ├── candid_cache.py     # fetches IDLs from canisters at startup
│   └── auth.py             # validates incoming HMAC for n8n
├── knowledge/
│   ├── canisters.yaml      # list of {name, principal, candid_path}
│   └── *.did               # cached Candid IDLs (refreshed on boot)
└── identity/
    └── agent_zero.pem      # Agent Zero's ICP identity, gitignored
```

### What it exposes

A single HTTP endpoint:

```
POST https://srv825251.hstgr.cloud/agent-zero/trigger
Content-Type: application/json
Authorization: HMAC-SHA256 <n8n shared secret>

{
  "intent":  "register_smartsite",
  "payload": { "domain": "...", "owner": "...", ... },
  "context": { "source": "n8n", "workflow_id": "..." }
}
```

Agent Zero's job is to:
1. Verify the HMAC (or fail with 401)
2. Match `intent` to a `(canister_name, method_name)` tuple via its
   knowledge base
3. Call the canister method via `ic-py`
4. Return the response (or a structured error)

### The Candid-from-canister pattern

Don't trust on-disk `.did` files. At Agent Zero startup, for each
canister in `knowledge/canisters.yaml`:

```python
def fetch_candid_from_canister(canister_id: str) -> str:
    """Fetch the canister's Candid metadata directly from IC.
    Survives any drift between checked-in .did files and actual canister."""
    response = agent.query_raw(
        canister_id,
        "__get_candid_interface_tmp_hack",
        encode([])
    )
    return decode(response, return_type="text")
```

This fetches the live Candid every time Agent Zero starts. If a
canister was upgraded, the new IDL is picked up automatically.

### The base script (refined)

The script you found is a good starting point. Here's the production-ready
version with the changes from above:

```python
# vps/agents/agent-zero/agent_zero/icp_router.py
from ic.canister import Canister
from ic.client import Client
from ic.identity import Identity
from ic.agent import Agent
import json, os, hmac, hashlib

class CanisterRouter:
    def __init__(self, identity_pem: str, network: str = "ic"):
        # Load Agent Zero's persistent identity (NOT anonymous)
        with open(identity_pem) as f:
            self.identity = Identity.from_pem(f.read())
        self.client = Client(url=f"https://{network}.app")
        self.agent = Agent(self.identity, self.client)
        self.canisters = {}  # name -> Canister instance

    def register(self, name: str, canister_id: str):
        """Fetch live Candid and register a canister by name."""
        candid = self._fetch_candid(canister_id)
        self.canisters[name] = Canister(
            agent=self.agent, canister_id=canister_id, candid=candid
        )

    def call(self, canister_name: str, method: str, *args):
        if canister_name not in self.canisters:
            raise KeyError(f"Unknown canister: {canister_name}")
        method_fn = getattr(self.canisters[canister_name], method)
        return method_fn(*args)

    def _fetch_candid(self, canister_id: str) -> str:
        # Implementation: query __get_candid_interface_tmp_hack
        ...
```

The intent-to-canister mapping (the LLM/routing part) is layered on
top — Agent Zero's reasoning fills in `canister_name` and `method`
based on `knowledge/canisters.yaml` and the natural-language `intent`.

---

## Security: caller-principal verification

Every "trigger" method on a project canister MUST verify the caller is
Agent Zero (or another authorized principal). Pattern from MyCloud's
auth canister:

```rust
const AGENT_ZERO_PRINCIPAL: &str = "abcde-xxxxx-xxxxx-xxxxx-cai";

fn require_authorized_caller() -> Result<(), Error> {
    let caller = ic_cdk::api::caller();
    let allowed = Principal::from_text(AGENT_ZERO_PRINCIPAL)?;
    if caller != allowed {
        return Err(Error::Unauthorized);
    }
    Ok(())
}

#[update]
fn distribute_rewards(amount: u64) -> Result<(), Error> {
    require_authorized_caller()?;
    // ... do the thing ...
}
```

Without this, the method is callable by anyone who knows the canister
ID — public methods are public. The principal of Agent Zero is stored
in the canister's stable state and updateable by the canister's owner
(you).

### Key rotation

If Agent Zero's identity is compromised:

1. Generate a new identity on the VPS:
   `dfx identity new agent-zero-v2 && dfx identity get-principal`
2. Update each project canister: call its `set_authorized_caller`
   admin method (gated to your owner principal) with the new principal
3. Deploy the new key to Agent Zero, restart the container
4. Old Agent Zero principal is now powerless

This rotation is something the manager canister will eventually
automate (warning when keys haven't been rotated in N days).

---

## Inter-canister call cost considerations

Every cross-canister call is async, takes ~2 seconds, costs cycles.
This means:

**Don't do this** (chatty serial calls):
```rust
let user = auth.get_user(p).await?;
let sites = registry.sites_by_owner(p).await?;
let health = manager.get_status_for(p).await?;
return combine(user, sites, health);
```
That's 6 seconds of latency for one user-facing request.

**Do this** (batched composite endpoint on the manager):
```rust
let dashboard = manager.get_user_dashboard(p).await?;
return dashboard;  // manager did the parallel calls internally
```

Manager exposes "complete a thing" methods, not "fetch one field"
methods. ICP allows parallel inter-canister calls via `futures::join!`,
which can run multiple in parallel and reduce 6s to 2s. Manager
exploits this.

---

## Staged rollout

Don't build all of this at once. The order matters because each stage
informs the next:

### Stage 0 (current) — Build the core canisters
- ✅ auth (Checkpoint 3a)
- ✅ registry (Checkpoint 3b)
- ✅ manager with cycles management (Checkpoint 3c)

**Status update May 12, 2026:** Agent Zero, n8n, Ollama, and the
supporting services (Redis, Postgres, ChromaDB) are already running on
the VPS — they were deployed during Crystal Dragon development. See
`docs/PORTS.md` for the full container list. This means Stage 1 below
is closer to "wire it up" than "build it."

### Stage 1 — Wire MyCloud canisters to the existing Agent Zero
- Agent Zero is already running on the VPS at port 50001 (web UI) +
  port 50003 (HTTP/MCP API).
- What's missing: a tool definition inside Agent Zero that uses `ic-py`
  to call our canisters by name.
- Validation: one Agent Zero command, like "list smartsites," that
  successfully calls registry.list_sites() and returns the result.
- This is small work (~half a day), not a new container deployment.

**⚠️ Known issue (discovered May 12, 2026):** the Agent Zero container
is currently crash-looping. `docker ps` reports it "Up" because
supervisord is fine, but the Python application inside fails to start
with:
```
ModuleNotFoundError: No module named 'langchain_groq'
  File "/a0/models.py", line 16, in <module>
    from langchain_groq import ChatGroq
```
Ports 50001 and 50003 return HTTP 000 (no listener). Image:
`frdel/agent-zero-run:latest`. Configured for `MISTRAL_MODEL=mistral`
against the local Ollama instance.

Fix paths when we get to Stage 1 (NOT urgent — we don't need Agent
Zero until the cloud factory needs routing):
- Pin to an older tag of `frdel/agent-zero-run` that pre-dates the
  langchain_groq dependency
- Rebuild the image with `pip install langchain_groq>=0.X` added
- Fork the upstream repo and fix the import
- Switch to a different agent framework entirely (Open-WebUI, etc.)

The langchain ecosystem has had several reorganizations; this is a
common kind of breakage with rapidly-evolving Python deps.

### Stage 2 — Add the MyCloud dashboard
- Vite/React dashboard at `frontend/dashboard/`
- Internet Identity login
- Reads from auth/registry/manager canisters
- Shows site fleet, canister health, cycles balance
- Deployed as ICP asset canister
- See `docs/DASHBOARD_PLAN.md` (to be written) for the incremental
  build plan

### Stage 3 — First external project canister: hopeandgrace
- New canister `backend/hopeandgrace_main/`
- Implements `CloudCanister` trait
- Add to dfx.json
- Register with Agent Zero (canister_id added to canisters.yaml)
- n8n workflows that target hopeandgrace methods through Agent Zero

### Stage 4 — metiverse and beyond
- Same template, different project
- At this point you have a real cloud factory
- Each new project: ~1 day from `cargo generate` to live triggers

### Stage 5 (optional) — Router canister
- If Agent Zero on the VPS becomes a bottleneck or trust concern,
  add a small router canister that does the intent-to-canister mapping
  on-chain
- Agent Zero still does LLM reasoning; the router enforces routing
  rules immutably

### Stage 6 — Future hardening: vetKeys for sensitive on-chain data

vetKeys (Verifiably Encrypted Threshold Keys) have been in production
on ICP mainnet since 2025. DFINITY ships ready-made KeyManager and
EncryptedMaps libraries on top.

Three places vetKeys would meaningfully improve MyCloud:

1. **`auth` canister credential vault** — currently stores user
   credentials in stable storage as plain bytes. vetKeys would
   encrypt each entry to the owning Principal, so even canister
   state inspection couldn't reveal credentials. Strongest fit.

2. **Per-blessing canisters for Hope & Grace** — recipient story
   text and financial details could be vetKey-encrypted so the
   public canister proves the blessing happened, but only the
   recipient (and delegated auditors) can read the sensitive parts.

3. **Crystal Dragon site secrets** — `site_config` JSON, PayPal
   tokens, custom-domain registrar credentials encrypted to the
   wallet that owns the Yggdrasil KEY NFT. Selling the NFT
   transfers vetKey access automatically.

**Why deferred:** vetKey integration is non-trivial (~1-2 weeks):
client-side decryption flow, key-derivation namespace design,
rotation handling. And we have zero users today — adding vetKeys to
an empty registry is theater. Revisit after the dashboard ships and
the first real smartsite is registered.

When we're ready: depend on `ic-vetkd-utils`, follow DFINITY's
KeyManager pattern, scope the work into Crystal Dragon's audit budget
so it gets reviewed once not twice.

**Caveat for whoever implements this:** vetKeys API renamed multiple
times during its 2-year preview period. Snippets found via LLMs or
old blog posts likely reference outdated method names (`vetkey_encrypted_key`,
`G1`/`G2` curve enums, separate system canister IDs). The current API
goes through the management canister (`aaaaa-aa`) via `vetkd_public_key`
and `vetkd_derive_key`. Always read DFINITY's live docs at implementation
time rather than trusting any cached snippet, including this one.

---

## What this is NOT

To keep the design honest:

- **Not "make MyCloud a SaaS product."** No customer-facing access
  controls, no billing for third parties, no support tickets. Cloud
  factory = your own multi-project infrastructure.
- **Not a microservices architecture in the traditional sense.**
  Canister boundaries are project boundaries, not feature boundaries.
- **Not a replacement for n8n.** Agent Zero is *invoked by* n8n; n8n
  remains where workflows are designed and triggered.
- **Not a way to "add cycles management to ICP."** Cycles are a
  fundamental ICP concept; we're just centralizing how *your* canisters
  get them.
- **Not a Polygon-NFT-gated product.** Solana NFTs (same as Crystal
  Dragon) for any access gating, OR Internet Identity allowlist for
  internal use.

---

## When to update this doc

Append to this file whenever:
- A stage transition happens ("Stage 2 complete, Stage 3 underway")
- A design decision changes ("decided to add the router canister
  earlier than planned because X")
- A new project canister joins the factory
- A security pattern is updated (e.g. key rotation policy changes)

Keep the staged rollout section honest — if you skip a stage, document
*why*. If a stage takes 3x longer than expected, note that. Future
you (and any collaborators) will thank present you.

---

## See also

- `ARCHITECTURE.md` — the "what is MyCloud" overview
- `OPERATIONS.md` — the daily operator's manual
- `ECOSYSTEM.md` — how MyCloud relates to Crystal Dragon, Agentic Acres, etc.
- `PROOF_PLAN.md` — current checkpoint progress
- `PORTS.md` — port reservations including 9603 for Agent Zero
- `C:\Crystal_DRagon_Smart_site\docs\MYCLOUD_integration_data.md` —
  how Crystal Dragon plugs into the registry canister
