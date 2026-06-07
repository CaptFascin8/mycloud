# MyCloud Dashboard — Build Plan

**Status:** DEFERRED behind Hope & Grace canister (June 7, 2026).

Hope & Grace is going to live battle testing this week with real bank
account, real donations, real users. The hopeandgrace canister is now
the priority path. See `docs/HOPEANDGRACE_INTEGRATION_SPEC.md`.

When we come back to the dashboard, the first canister it'll read from
will be `hopeandgrace` (real data) rather than `registry` (still empty),
which is actually better — the dashboard is more useful with real data
to display.

The rest of this document remains accurate for when we resume; just
swap "Site Registry MVP" for "Hope & Grace Ceremonies MVP" as the
first view we build.

---

**Status (original):** Plan written May 16, 2026. Ready to execute when there's
a fresh focus block.

**Goal:** Build the first React page that talks to a MyCloud canister
with Internet Identity authentication. Just one view (Site Registry),
end-to-end working. Defer the other five views in `DASHBOARD_REQUIREMENTS.md`
until this validates the pipeline.

---

## Why one view first

The dashboard has six designed views (Fleet, Registry, Detail, Health,
Financial, Agent Zero). Building all six simultaneously means:
- We don't validate the React + @dfinity/agent + Internet Identity
  pipeline until the end, when fixing it is expensive
- We build features for an empty registry (zero KEYs sold = nothing
  to show)
- We commit to design decisions before we've used the tools once

Building the Registry view first:
- Validates the entire toolchain end-to-end (~3-4 hours)
- Gives Crystal Dragon's bridge daemon something real to write into
- Other views become "copy this pattern, hit a different canister method"
- The first 200 lines of working code teach us more than 2000 of plans

---

## Pre-flight (do these first, ~30 min)

Before touching any React, three small canister upgrades make the
dashboard meaningful.

### 1. Add three fields to the `Smartsite` struct in registry

```rust
pub struct Smartsite {
    // ... existing fields ...
    pub status:       SiteStatus,        // NEW
    pub container_id: Option<String>,    // NEW
    pub expires_ns:   Option<u64>,       // NEW
}

#[derive(CandidType, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum SiteStatus {
    Provisioning,   // bridge daemon has the request, not yet deployed
    Active,         // Cloud Can running, site serving
    Purified,       // migrated to fully-on-chain (Phase H of INTEGRATION_PLAN)
    Suspended,      // owner action or non-payment
    Decommissioned, // explicit teardown
}
```

### 2. Add two new methods to registry

```rust
#[update]
fn update_site_status(domain: String, new_status: SiteStatus)
    -> Result<Smartsite, RegistryError>;

#[update]
fn set_container_id(domain: String, container_id: String)
    -> Result<Smartsite, RegistryError>;
```

Both owner-only. Both update `updated_ns`.

### 3. Stable storage migration

Existing entries in the BTreeMap don't have these fields. Two options:

**Option A (simpler):** make the new fields `Option<T>` even for status
(default `Active`) so deserialize works on old records. Add a one-shot
migration method `migrate_v2()` that fills in defaults for existing
records.

**Option B (cleaner):** use Candid's optional-field semantics where
adding a new field with `Option<T>` is backward-compatible automatically.

Option B is what we want. ic-stable-structures handles this if we keep
the Storable encoding via `Encode!`/`Decode!` (we already do).

### 4. Update integration test

`scripts/test_registry.sh` should:
- Register a site (gets default status = Provisioning or Active)
- Call `update_site_status` to change to Active
- Call `set_container_id` to set a fake container ID
- Verify both fields persist via `get_site`

### 5. Commit + push

```
checkpoint 3b.1: registry adds status/container_id/expires_ns fields

Adds the fields the dashboard needs to display site state beyond
basic CRUD. Bridge daemon will use update_site_status and
set_container_id to report deployment progress back to the registry.

SiteStatus enum: Provisioning | Active | Purified | Suspended | Decommissioned

Backward-compatible: old records deserialize cleanly via Candid's
optional-field rules.
```

---

## The dashboard build (~3-4 hours focus block)

### Tech stack decisions

- **Vite + React + TypeScript** (already scaffolded in
  `frontend/dashboard/`)
- **`@dfinity/agent`** — talk to canisters from the browser
- **`@dfinity/auth-client`** — Internet Identity login flow
- **`@dfinity/principal`** — handle Principal types
- **`@dfinity/candid`** — encode/decode Candid values
- **Tailwind CSS** — quick styling, no design library overhead
- **Generated Candid bindings via `dfx generate registry`** — get
  TypeScript types from our .did files automatically

### Build target (per Option C decision)

Vite build outputs plain static files to `frontend/dashboard/dist/`.
These get served two ways:

**Initial deployment:** copy `dist/` to `/var/www/srv825251-dashboard/`
on the VPS, add an Nginx `location /dashboard/` block. Updates are
`rsync` or `git pull` + build.

**Future on-chain deployment:** add `dashboard` to `dfx.json` as an
asset canister pointing at `frontend/dashboard/dist/`. Deploy with
`dfx deploy --network ic dashboard`. Same `dist/` artifacts, different
host.

Build it once, deploy either way.

---

## The actual build steps

### Step 1 — Install deps + generate types (~15 min)

```bash
cd /opt/mycloud/frontend/dashboard
npm install \
    @dfinity/agent \
    @dfinity/auth-client \
    @dfinity/principal \
    @dfinity/candid \
    tailwindcss postcss autoprefixer
npx tailwindcss init -p

# Generate TypeScript types from our Candid files
cd /opt/mycloud
dfx generate registry
dfx generate auth
dfx generate manager
# This creates src/declarations/{registry,auth,manager}/
```

### Step 2 — Wire Internet Identity (~45 min)

Create `frontend/dashboard/src/lib/auth.ts`:

```typescript
import { AuthClient } from "@dfinity/auth-client";
import { HttpAgent, Actor } from "@dfinity/agent";

const II_URL = "https://identity.ic0.app";
const HOST = process.env.NODE_ENV === "production"
  ? "https://ic0.app"
  : "http://localhost:4943";

export async function getAuthClient() {
  return await AuthClient.create();
}

export async function login(client: AuthClient) {
  return new Promise<void>((resolve) => {
    client.login({
      identityProvider: II_URL,
      onSuccess: () => resolve(),
    });
  });
}

export async function getAgent(client: AuthClient) {
  const identity = client.getIdentity();
  const agent = new HttpAgent({ identity, host: HOST });
  if (process.env.NODE_ENV !== "production") {
    await agent.fetchRootKey();  // local replica only
  }
  return agent;
}
```

### Step 3 — Build the Site Registry view (~90 min)

Create `frontend/dashboard/src/pages/SiteRegistry.tsx`:

```typescript
// Pseudocode — actual code lives here, written in the session
import { useEffect, useState } from "react";
import { createActor } from "../../../../src/declarations/registry";
import { getAuthClient, login, getAgent } from "../lib/auth";

export function SiteRegistry() {
  const [authed, setAuthed] = useState(false);
  const [sites, setSites] = useState([]);
  const [loading, setLoading] = useState(true);

  // On mount: check auth, if authed fetch sites
  // Show login button if not authed
  // Show table of sites with: domain, owner, status, ipfs_cid, updated_ns
  // Refresh button
  // Empty state: "No smartsites yet — register one via Crystal Dragon"
}
```

### Step 4 — Wire to canister + test against local dfx (~30 min)

- Make sure dfx is running on the VPS (`dfx ping`)
- `npm run dev` to start Vite at `127.0.0.1:5173`
- SSH-tunnel from HOPE: `ssh -L 5173:127.0.0.1:5173 mycloud`
- Open `http://localhost:5173` in browser
- Click login, do Internet Identity flow, see empty registry
- Manually register a site via `dfx canister call registry register_site ...`
- Refresh in browser, see the site appear

### Step 5 — Build for production + deploy to Nginx (~30 min)

```bash
cd /opt/mycloud/frontend/dashboard
npm run build
# outputs to dist/

# Copy to VPS web root
sudo mkdir -p /var/www/srv825251-dashboard
sudo cp -r dist/* /var/www/srv825251-dashboard/

# Add Nginx location block to /etc/nginx/sites-enabled/srv825251.hstgr.cloud:
#   location /dashboard {
#       alias /var/www/srv825251-dashboard;
#       try_files $uri $uri/ /dashboard/index.html;
#   }

sudo nginx -t && sudo systemctl reload nginx
```

Visit `https://srv825251.hstgr.cloud/dashboard` from any browser.

### Step 6 — Commit + push

```
checkpoint 5.1: dashboard MVP — site registry view with Internet Identity

First React + @dfinity/agent + Internet Identity integration.
One view (Site Registry), full end-to-end working:
- II login flow
- Reads registry canister via TypeScript-generated bindings
- Renders sites in a table with status, owner, CID, timestamps
- Deployed to https://srv825251.hstgr.cloud/dashboard/

Next: Fleet view, Site Detail, Canister Health (in order of
when their data becomes meaningful as Crystal Dragon ships KEYs).
```

---

## Decision points to surface during the build

These are choices we'll hit during the session. Worth knowing they're
coming:

1. **Production II vs local II.** Mainnet II at `https://identity.ic0.app`
   only authenticates against mainnet canisters. Our canisters are on
   the local dfx replica. We'll either: (a) deploy registry to mainnet
   first (costs ~$10 in cycles + a few minutes), or (b) run a local
   Internet Identity canister in the dfx replica for dev. (b) is
   simpler for the MVP; (a) is the actual production path.

2. **Owner-only access pattern.** DASHBOARD_REQUIREMENTS says
   "admin-only — only Nate's principal." For the MVP, do we hardcode
   your principal in the React app and reject others, or do we let
   anyone log in and they just see an empty registry (since the
   registry is currently empty)? **Probably the latter for MVP**, since
   it's friendlier and the data isn't sensitive yet.

3. **Styling philosophy.** "Functional and ugly" vs "polished from
   day one." Recommend **functional and ugly** for MVP. Polish
   happens after the data flow is proven.

---

## What we are explicitly NOT doing in the MVP

- Fleet overview (no data to aggregate yet)
- Financial view (no transactions yet)
- Canister health graphs (manager has events, but graphs are polish)
- Agent Zero console (Agent Zero is broken + we don't need it yet)
- Site Detail view (Registry list is enough until there are 10+ sites)
- vetKeys integration (defer until after first user; see CLOUD_FACTORY.md)
- Cloud Can deployment trigger UI (no Cloud Cans exist yet)

Each of these gets its own focused build session later when there's
real reason to build it.

---

## When you sit down to do this

1. Read this file
2. Read `docs/DASHBOARD_REQUIREMENTS.md` (the Crystal Dragon Claude
   reference doc) for context
3. Read `docs/STARTUP_GUIDE.md` if you've been away (the personal
   private doc)
4. Open VS Code Remote-SSH to mycloud
5. Make sure dfx is running: `dfx ping`
6. Start with the Pre-flight section (the 5 steps above)
7. Commit + push pre-flight changes before starting the React work
8. Then the dashboard build (Steps 1-6)
9. Commit + push the MVP

Budget 4 hours total. If something blocks, paste the error here and
we troubleshoot. Most likely friction points:
- `dfx generate` output paths vs Vite import paths
- Internet Identity flow CORS in dev mode
- Tailwind config interfering with Vite's HMR

None are fatal; all have known fixes.
