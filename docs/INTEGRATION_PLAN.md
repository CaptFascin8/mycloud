# Crystal Dragon × MyCloud — Integration Plan

**Created:** May 9, 2026
**Author:** Nate + Claude
**Canonical copies:** `C:\MY_CLOUD\docs\MERGE_PLAN.md` + `C:\Crystal_DRagon_Smart_site\docs\MERGE_PLAN.md`

> **Note on "merge" vs "integration":** Despite the filename, this doc
> describes how the two projects *integrate* — they stay separate repos,
> separate codebases, separate purposes. Crystal Dragon is a product;
> MyCloud is infrastructure. Future products (Hope & Grace, Agentic
> Acres, others) will integrate with MyCloud the same way. Don't fold
> Crystal Dragon's product code into MyCloud's infrastructure repo,
> or the next product has to fold in too.

---

## The one-paragraph version

Crystal Dragon is a product. MyCloud is the infrastructure under it.
Crystal Dragon sells NFT-gated website ownership to users. MyCloud
provides the identity layer (ICP canisters), storage layer (IPFS),
serving layer (Nginx), and orchestration layer (Agent Zero + Bridge)
that makes those websites exist, stay online, and remain sovereign.
This document maps every piece to its home and orders the work so
nothing gets built before its dependencies exist.

---

## Terminology

The original draft of this doc used "Cloud Can" loosely for three
different things. To prevent future confusion, we use these terms
consistently throughout:

- **Site container** — a Docker container on the VPS hosting one
  user's claimed site. Phases D-E. Shorthand: "container."
- **Project canister** — an ICP canister representing a whole project
  (auth, registry, manager are the core three; future ones include
  hopeandgrace_main, metiverse_main, etc.). Phase F-G. This is the
  pattern documented in `CLOUD_FACTORY.md`.
- **Router canister** — a small canister deployed per Purified site
  in Phase H, handling per-site request routing and data bridging
  between the site's own canisters and external services.

The phrase "Cloud Can" still appears in some headings as a familiar
shorthand, but every appearance maps to one of the three above. When
in doubt, prefer the precise term.

---

## Architecture overview

Three layers, two projects, one product:

```
┌─────────────────────────────────────────────────────────────┐
│  TRUST LAYER — ICP Blockchain (C:\MY_CLOUD\backend\)       │
│                                                             │
│  ┌──────────┐  ┌──────────────┐  ┌──────────────┐          │
│  │   auth   │  │   registry   │  │   manager    │          │
│  │ II login │  │ smartsites + │  │ health polls │          │
│  │ cred vault│ │ NFT proof    │  │ cycles mgmt  │          │
│  └──────────┘  └──────────────┘  └──────────────┘          │
│                                                             │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐   │
│  │ H.O.P.E. │  │ metiverse│  │ git vault│  │ auth     │   │
│  │ canister │  │ canister │  │ canister │  │ vault    │   │
│  └──(future)┘  └──(future)┘  └──(future)┘  └──(future)┘   │
└─────────────────────────────────────────────────────────────┘
         │              │              │
         ▼              ▼              ▼
┌─────────────────────────────────────────────────────────────┐
│  WEIGHT LAYER — Hostinger VPS (C:\MY_CLOUD\vps\)           │
│  srv825251.hstgr.cloud  (82.25.91.136)                     │
│                                                             │
│  ┌────────┐  ┌──────┐  ┌────────────┐  ┌──────┐           │
│  │ Nginx  │  │ IPFS │  │ Agent Zero │  │ n8n  │           │
│  │ TLS +  │  │ Kubo │  │ port 9603  │  │      │           │
│  │ routing│  │ CIDs │  │ ic-py      │  │      │           │
│  └────────┘  └──────┘  └────────────┘  └──────┘           │
│       │                      │                              │
│       ▼                      ▼                              │
│  ┌──────────────────────────────────────────┐  ┌────────┐  │
│  │  Cloud Cans (Docker containers)          │  │ Bridge │  │
│  │  myart.cd.tech │ shop.cd.tech │ ...      │  │ daemon │  │
│  │  Each has: template site + editor        │  │        │  │
│  └──────────────────────────────────────────┘  └────────┘  │
└─────────────────────────────────────────────────────────────┘
         │              │              │
         ▼              ▼              ▼
┌─────────────────────────────────────────────────────────────┐
│  HUMAN LAYER — Browser                                      │
│                                                             │
│  crystaldragon.tech    dashboard     *.crystaldragon.tech   │
│  (storefront + mint)   (admin)       (user sites w/ editor) │
└─────────────────────────────────────────────────────────────┘
```

---

## Where things live

### Already built

| Component | Project | Path | Status |
|-----------|---------|------|--------|
| Auth canister | MyCloud | `backend/auth/` | ✅ Checkpoint 3a |
| Registry canister | MyCloud | `backend/registry/` | ✅ Checkpoint 3b |
| Manager canister | MyCloud | `backend/manager/` | ✅ Checkpoint 3c |
| Template site builder | Crystal Dragon | `C:\Crystal_DRagon_Smart_site\` | ✅ 85% complete |
| Visual editor (sections, blocks) | Crystal Dragon | `components/Section.tsx` | ✅ Working |
| 3D model support (model-viewer) | Crystal Dragon | `components/Section.tsx` | ✅ Just added |
| AI assistant (Gemini Live) | Crystal Dragon | `components/CrystalAssistant.tsx` | ✅ Working |
| Storage engine (IndexedDB) | Crystal Dragon | `storage.ts` | ✅ v2 shipped |
| VPS provisioned | MyCloud | `srv825251.hstgr.cloud` | ✅ Ubuntu 24.04 |

### Next to build

| Component | Project | Path | Phase |
|-----------|---------|------|-------|
| IPFS Kubo container | MyCloud | `vps/docker/` | A (now) |
| Nginx + TLS | MyCloud | `vps/docker/nginx/` | A (now) |
| Dashboard (real) | MyCloud | `frontend/dashboard/` | B |
| Solana NFT verifier | MyCloud | `backend/registry/src/` | C |
| Storefront site | Crystal Dragon | `C:\Crystal_DRagon_Smart_site\` or new | D |
| Wallet adapter (Phantom) | Crystal Dragon | `components/WalletProvider.tsx` | D |
| Mint page | Crystal Dragon | `components/MintPage.tsx` | D |
| Subdomain claim flow | Both | registry canister + bridge | E |
| Cloud Can deployment | MyCloud | `vps/docker/` + bridge daemon | E |
| Agent Zero | MyCloud | `vps/agents/agent-zero/` | F |

---

## The user journey (what we're building toward)

1. Visitor arrives at **crystaldragon.tech**
2. Sees marketing page: "Own your website. Powered by blockchain."
3. Clicks **"Get Started"** → wallet connect modal (Phantom/Solflare)
4. Chooses tier → mints **Yggdrasil KEY** NFT (Solana, via Candy Machine)
5. Enters desired subdomain: `myart`
6. System checks availability via **registry canister** → `register_site()`
7. KEY NFT gets burned → proves commitment, prevents hoarding
8. **Bridge daemon** sees the new registry entry → spins up Docker container
9. Template site deploys to `myart.crystaldragon.tech`
10. User visits their new subdomain → sees Crystal Dragon editor
11. Connects wallet → **registry canister** confirms `OwnershipProof::SolanaNft`
12. Admin panel unlocks → user builds their website

---

## Phase breakdown

### Phase A — VPS provisioning (MyCloud Checkpoint 4)
**Status:** In progress
**Blocker for:** Everything else

What's left:
- [ ] IPFS Kubo container: `docker compose up -d`
- [ ] Nginx with Let's Encrypt TLS for `srv825251.hstgr.cloud`
- [ ] End-to-end test: `curl https://srv825251.hstgr.cloud/ipfs/<cid>`

**No Crystal Dragon work needed here.** Pure infrastructure.

### Phase B — Dashboard MVP (MyCloud Checkpoint 5)
**Depends on:** Phase A

Build the real dashboard at `frontend/dashboard/`:
- [ ] Internet Identity login (using `@dfinity/auth-client`)
- [ ] Show auth canister state (user count, vault entries)
- [ ] Show registry canister state (site list, owners)
- [ ] Show manager canister state (health events, cycle balances)
- [ ] Upload a file → pin to IPFS → get CID → register as smartsite
- [ ] Visit `https://srv825251.hstgr.cloud/ipfs/<cid>` and see content
- [ ] Deploy dashboard as ICP asset canister

This validates the entire pipeline before we add commerce.

### Phase C — Solana NFT verification
**Depends on:** Phase B (need dashboard to test with)

- [ ] Implement `verify_solana_nft()` in registry canister
  - HTTP outcall to Solana RPC (`getTokenAccountsByOwner`)
  - Check if wallet holds a mint from our Candy Machine collection
  - Extract tier from metadata attributes
- [ ] Add `verify_ownership` method to registry Candid interface
- [ ] Test on devnet with test NFTs
- [ ] Dashboard shows "Verified: KEY #0042 (ROOT tier)"

**Estimated work:** ~100 lines of Rust in `backend/registry/src/`

### Phase D — Crystal Dragon storefront
**Depends on:** Phase C (need NFT verification working)

This is where the two projects merge visually:

- [ ] Solana wallet adapter in Crystal Dragon
  - `npm install @solana/wallet-adapter-react` etc.
  - `components/WalletProvider.tsx` wrapping App
  - Replace password login with wallet connect
- [ ] Mint page (`components/MintPage.tsx`)
  - Connect to Candy Machine v3
  - Show tier pricing, available count
  - "Mint KEY" button → transaction signing
  - Success: show NFT preview + link to Solscan
- [ ] Subdomain claim modal
  - Check NFT ownership (calls registry canister)
  - Enter desired subdomain
  - Availability check (real-time)
  - "Claim" button → burn NFT + register_site
- [ ] "How to Buy" documentation page
  - Walk through the actual flow
  - Screenshots of each step
  - Explainer video

**This is where you can't document until you can do it.** The mint
page and claim flow have to work before you can screenshot them.

### Phase E — Site container deployment automation
**Depends on:** Phase D (need the claim flow to trigger deployment)

> **Order note:** The original draft put Agent Zero deployment after
> this phase. In practice, Agent Zero is the engine that triggers the
> automated deployments — it has to exist BEFORE there's an automated
> way to spin up site containers. The "bridge daemon" listed here is
> essentially Agent Zero's first job. We've revised the ordering: a
> minimal Agent Zero MVP (just a webhook receiver that calls canister
> methods) goes in BEFORE the full deployment pipeline. Phase F below
> elaborates Agent Zero into its full role.

The automation layer:

- [ ] Bridge daemon (`vps/bridge/`)
  - Polls registry canister for new `register_site` events
  - On new site: creates Docker container from template
  - Registers container back with registry
  - Configures Nginx for the new subdomain
- [ ] Template Docker image
  - Crystal Dragon site builder pre-installed
  - Config loaded from IPFS CID (the site's state)
  - Auto-starts Vite dev server (or pre-built static)
- [ ] Wildcard DNS on crystaldragon.tech
  - `*.crystaldragon.tech` → VPS IP
  - Nginx virtual hosts route to correct container
- [ ] Wildcard SSL via Let's Encrypt
- [ ] Health monitoring
  - Manager canister watches each Cloud Can container
  - Bridge reports container health back to manager

### Phase F — Agent Zero + Cloud Factory
**Depends on:** Phase E (need running containers to route to)

- [ ] Deploy Agent Zero container (port 9603)
- [ ] Configure to talk to all canisters via ic-py
- [ ] n8n integration: webhook → Agent Zero → canister method
- [ ] Intent routing: "register a site" → registry.register_site()
- [ ] CloudCanister trait for future project canisters
- [ ] First external project canister (H.O.P.E.)

### Phase G — Future Cloud Cans (post-launch)

Once the factory pattern works, adding new cans is straightforward:

- **Git Vault Can** — stores git credentials, auto-connects to
  GitHub/HuggingFace, pulls models into project containers
- **Auth Vault Can** — private credential store, 2FA management,
  owned by your cloud (not Google/Apple)
- **AI Agent Can** — runs Agent Zero or custom LLM agents per project
- **H.O.P.E. Can** — Hope & Grace Angel Network project canister
- **Metiverse Can** — metaverse/3D world project canister

Each new can: `cargo generate` template → implement CloudCanister
trait → add to dfx.json → register with Agent Zero → done.

---

## Where Agent Zero lives and what it does

Agent Zero is a **Docker container on the VPS**, not an ICP canister.

```
VPS (srv825251.hstgr.cloud)
├── docker-compose.yml
│   ├── nginx          (ports 80, 443)
│   ├── ipfs           (port 9600-9601)
│   ├── agent-zero     (port 9603)    ← HERE
│   ├── bridge-daemon  (internal)
│   ├── n8n            (port 5678)
│   ├── cloudcan-001   (user site A)
│   ├── cloudcan-002   (user site B)
│   └── ...
```

**Why on the VPS, not on-chain:**
- Agent Zero is Python + LLM inference — can't run in a WASM canister
- Colocated with the containers it manages (low latency)
- Talks to canisters via `ic-py` (ICP's Python SDK)
- Costs nothing per call (unlike inter-canister calls which cost cycles)

**What it does:**
- Receives intents from n8n, dashboard, or direct HTTP calls
- Routes to the correct canister + method based on Candid IDL introspection
- Manages Cloud Can lifecycle (via bridge daemon)
- Future: LLM-powered intent understanding ("create a portfolio site
  for a photographer" → picks template, creates can, configures)

**Security:**
- Has its own ICP principal (dedicated keypair)
- Every canister method checks `caller == agent_zero_principal`
- Principal is rotatable via `set_authorized_caller` admin method
- All trigger calls are logged for audit

---

## File ownership between projects

| File/folder | Lives in | Reason |
|-------------|----------|--------|
| Rust canisters (auth, registry, manager) | MyCloud | Infrastructure |
| Candid interfaces (.did files) | MyCloud | Contract definitions |
| Docker compose, Nginx config | MyCloud | VPS orchestration |
| Agent Zero code | MyCloud | Infrastructure agent |
| Bridge daemon | MyCloud | Container lifecycle |
| Dashboard frontend | MyCloud | Admin interface |
| Template site builder (React) | Crystal Dragon | The product |
| Visual editor, sections, blocks | Crystal Dragon | The product |
| AI assistant (Crystal Guardian) | Crystal Dragon | Product feature |
| Storefront / marketing pages | Crystal Dragon | Sales |
| Mint page + wallet adapter | Crystal Dragon | Sales |
| Subdomain claim flow | Crystal Dragon | Sales (calls MyCloud) |
| This merge plan | Both | Shared reference |

---

## Integration touchpoints (where they talk)

1. **Wallet → Registry canister**
   Crystal Dragon's mint page calls `registry.register_site()` with
   `OwnershipProof::SolanaNft { mint, wallet, tier }` after KEY burn.

2. **Registry canister → Bridge daemon**
   Bridge polls `registry.list_sites()` or `get_events_since()` for
   new registrations, then spins up containers.

3. **Nginx → Cloud Can containers**
   Wildcard subdomain routing: `myart.crystaldragon.tech` →
   container for that site.

4. **Cloud Can → IPFS**
   Site content (images, videos, configs) pinned to IPFS. CID stored
   in registry canister. Container reads content from local IPFS node.

5. **Dashboard → All three canisters**
   Admin view showing health, sites, users, cycles across the fleet.

6. **Agent Zero → Any canister**
   Routes n8n triggers and LLM intents to the correct method.

---

## What to work on right now

**You are here:** Phase A (VPS provisioning), about to start IPFS + Nginx.

After that, the critical path is: **B → C → D** (dashboard → NFT
verification → storefront). You can't document the "How to Buy" flow
until the mint page and claim flow are actually working (Phase D).

The Cloud Factory (Phase E-F) and future Cloud Cans (Phase G) come
after launch. Don't pre-build them.

---

## Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2025-02-21 | Solana over Polygon for NFTs | Lower fees, faster finality |
| 2025-02-21 | Metaplex Candy Machine v3 | No custom contract needed |
| 2026-05-08 | MyCloud 3 canisters complete | auth, registry, manager all tested |
| 2026-05-09 | 3D model support added to template | model-viewer web component |
| 2026-05-09 | Merge plan created | Unify Crystal Dragon + MyCloud vision |

---

## Remember

- MyCloud is the engine. Crystal Dragon is the car.
- One canister per project, not per feature.
- Manager canister is the cycles bursar for the whole fleet.
- Agent Zero lives on the VPS, talks to canisters via ic-py.
- Cloud Cans are Docker containers, one per claimed site.
- The registry canister is the single source of truth for "who owns what."
- Don't build the factory until there's something to manufacture.

---

## Phase H — "Purify": fully on-chain websites (long-term vision)

**Status:** Vision. Not scheduled. Depends on Phases A–F shipping first.

### The dream

After a user buys their KEY, claims their subdomain, and builds their
site using the Crystal Dragon editor, a **"Purify"** button appears in
the admin panel. Clicking it transforms their draft site into a
permanent, self-sovereign, on-chain website:

```
┌─────────────────────────────────────────────────────────┐
│  User clicks "PURIFY"                                   │
│                                                         │
│  1. Site config (JSON) → pinned to IPFS → CID stored   │
│  2. Static frontend build → deployed as asset canister  │
│  3. Site's own manager canister deployed (cycles mgmt)  │
│  4. Cloud Can Zero deployed (routing + data bridge)     │
│  5. Registry updated: site status = "on-chain"          │
│  6. DNS updated: subdomain → ICP canister (no VPS)      │
│                                                         │
│  Result: site lives permanently on ICP + IPFS            │
│  Owner tops up their master canister to keep it alive    │
│  VPS becomes optional (cache/proxy only)                 │
└─────────────────────────────────────────────────────────┘
```

### What gets deployed per Purified site

Each Purified site is a small fleet of canisters:

1. **Asset canister** — serves the static HTML/JS/CSS (the Crystal
   Dragon React app shell). ICP natively serves HTTP from asset
   canisters, so the site loads directly from the blockchain.

2. **Config canister** — holds the site's JSON config (sections,
   blocks, styles). On page load, the React app fetches config from
   this canister instead of localStorage. Edits save back here.

3. **Manager canister (child)** — a lightweight clone of MyCloud's
   manager. Monitors the site's own canisters, handles cycle top-ups,
   alerts when balance is low. The user tops up THIS canister, and it
   distributes to the others.

4. **Cloud Can Zero** — the optional "brain" canister. Handles:
   - Routing form submissions or user input to other canisters
   - Bridging data to off-chain services (VPS, databases, APIs)
   - Event hooks: "when someone submits the contact form → store
     in canister + email notification via VPS agent"

### What this means for the user

- **Before Purify:** Site runs on VPS Docker container. Fast, flexible,
  but depends on Hostinger staying online and Nate maintaining it.
- **After Purify:** Site runs on ICP + IPFS. Permanent, censorship-
  resistant, self-sovereign. User manages their own cycles. Even if
  crystaldragon.tech goes offline, their site keeps running at its
  canister URL.

The user's only ongoing cost: ICP cycles. Order-of-magnitude estimate:
~$5–20/year for a low-traffic static site, depending on storage size
and request volume. Real numbers will come from running actual sites
in Phase H — treat current figures as ballpark, not quote.

### The Purify sequence (technical)

```
User clicks "Purify" in admin panel
  │
  ├── 1. Frontend runs `npm run build` (or pre-built template)
  │      Output: dist/ folder with static assets
  │
  ├── 2. Pin dist/ to IPFS → backup CID
  │
  ├── 3. Deploy asset canister with dist/ contents
  │      Uses dfx or ic-agent from VPS bridge
  │
  ├── 4. Export site config JSON
  │      Pin to IPFS → config CID
  │      Deploy config canister with initial state
  │
  ├── 5. Deploy child manager canister
  │      Set owner = user's principal
  │      Watch: asset canister + config canister
  │      Initial cycle deposit from MyCloud treasury
  │
  ├── 6. (Optional) Deploy Cloud Can Zero canister
  │      If site has forms, webhooks, or data bridges
  │      Set authorized callers: user + Agent Zero
  │
  ├── 7. Update registry canister
  │      site.status = "purified"
  │      site.asset_canister_id = <new canister>
  │      site.config_canister_id = <new canister>
  │
  ├── 8. Update DNS / routing
  │      subdomain.crystaldragon.tech → ICP boundary nodes
  │      (via custom domain registration with ICP, OR a small
  │      reverse-proxy on a generic VPS — either way, MyCloud's
  │      Hostinger box no longer needs to be involved in serving
  │      this site's content)
  │
  │      Note: ICP canisters are accessed over HTTPS through ICP's
  │      boundary node infrastructure. There's no "DNS directly to
  │      a canister" — DNS still resolves to a server somewhere. The
  │      win of Purify is that the server is ICP's, not yours.
  │
  └── 9. Show user their Purified site
         "Your site is now permanently on-chain."
         "Top up cycles here: [link to child manager]"
         "Your site will run as long as it has cycles."
```

### Why this is Phase H, not Phase D

The Purify flow requires:
- Working asset canister deployment (dfx deploy from VPS)
- Working cycle transfer mechanics (manager → child canisters)
- Working ICP custom domains (boundary nodes or alternative)
- The entire Crystal Dragon editor working as a static export
- All of Phases A–F complete

Building this before the VPS-based flow works would be building the
roof before the walls. The VPS version is the MVP; Purify is the
endgame.

### The pitch

"Design your website with AI. Own it with an NFT. Purify it to live
forever on-chain. Your art, your site, your sovereignty."

That's the one-line version of everything we're building.

---

## Decision log

(Appended)

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-05-09 | "Purify" concept documented | Long-term vision for fully on-chain sites |
| 2026-05-09 | Cloud Can Zero = routing canister | Not an AI agent on-chain; routes data between canisters/services |

---

**Next review:** After Phase A completes (IPFS + Nginx working).
