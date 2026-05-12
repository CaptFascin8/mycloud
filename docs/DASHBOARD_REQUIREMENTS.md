# MyCloud Dashboard — Requirements for Crystal Dragon Integration

**Created:** May 12, 2026
**Purpose:** This document defines what the MyCloud Dashboard needs to support
the Crystal Dragon website builder platform. Share this with the Claude
session working on the MyCloud project.

---

## What the Dashboard IS

The MyCloud Dashboard is YOUR admin control panel. It is NOT customer-facing.
Customers interact with their own Dragon Console on their subdomain.
The Dashboard lets you (Nate) monitor, manage, and troubleshoot all
sold websites, canister health, and infrastructure.

---

## Dashboard Views Required

### 1. Fleet Overview (Home)
- Total sites sold / active / purified / expired
- Total revenue (KEYs sold × price)
- VPS resource usage (CPU, memory, disk, bandwidth)
- ICP cycle balance (master canister)
- IPFS storage usage
- Alerts: low cycles, down containers, errors

### 2. Site Registry (from registry canister)
- Table of all registered sites:
  - Subdomain (e.g., myart.crystaldragon.tech)
  - Owner wallet address
  - KEY tier (ROOT/TRUNK/BRANCH/CROWN)
  - KEY number (#0001-#10000)
  - Status: provisioning / active / purified / suspended
  - Created date
  - Last active date
  - Storage used (MB)
  - Container ID (Docker)
  - Canister IDs (if purified)
- Search/filter by status, tier, date
- Click row → site detail view

### 3. Site Detail View
- Full site metadata
- Container health (CPU, memory, uptime)
- Storage breakdown (images, videos, 3D models)
- Owner's wallet activity
- "Open Site" link → opens subdomain
- "Open Admin" link → opens subdomain admin panel
- Actions:
  - Restart container
  - Suspend site (policy violation)
  - Extend free API period
  - Send notification to owner

### 4. Canister Health (from manager canister)
- All watched canisters with cycle balances
- Color-coded: green (>1T cycles), yellow (<1T), red (<100B)
- Auto-alert threshold configuration
- "Top Up" button (from treasury)
- Per-canister: method call history, error rate
- Purified site canisters grouped by owner

### 5. Financial Overview
- KEYs sold by tier (chart)
- Revenue by month
- Active subscriptions (if renewal model added)
- Cycle costs (ICP) vs revenue
- VPS costs vs revenue

### 6. Agent Zero Console
- Send commands to Agent Zero
- View recent command log
- Test intent routing
- n8n workflow status

---

## Data Sources

| Dashboard View | Data Source | Method |
|----------------|------------|--------|
| Fleet Overview | All three canisters + Docker API | Aggregated |
| Site Registry | Registry canister | `list_sites()` |
| Site Detail | Registry + Docker + IPFS | Combined |
| Canister Health | Manager canister | `get_health_events()` |
| Financial | Registry canister + local DB | `list_sites()` + accounting |
| Agent Zero | Agent Zero HTTP API | Port 50003 |

---

## Integration Points with Crystal Dragon

### When a KEY is burned (Crystal Dragon → MyCloud)
1. Crystal Dragon frontend calls registry canister `register_site()`
2. Passes: owner principal, subdomain, ownership proof (Solana NFT)
3. Registry stores the record, emits event
4. Bridge daemon (on VPS) polls for new registrations
5. Bridge creates Docker container from template image
6. Bridge configures Nginx for new subdomain
7. Bridge calls registry `update_site_status("active")`
8. Dashboard shows new site in fleet overview

### When a site is Purified (Crystal Dragon → MyCloud)
1. Crystal Dragon frontend triggers Purify flow
2. Site config exported as JSON, pinned to IPFS
3. Static build deployed as ICP asset canister
4. Config canister deployed for dynamic state
5. Child manager canister deployed for cycles
6. Registry updated: status = "purified", canister IDs stored
7. Dashboard shows purified site with canister health

### When cycles are low (MyCloud → Crystal Dragon owner)
1. Manager canister detects low cycle balance
2. Dashboard shows alert
3. Automated email/notification to site owner
4. Owner tops up via their Dragon Console (Web3 wallet → ICP cycles)
5. Manager distributes cycles to child canisters

---

## Template Image for New Cloud Cans

When a KEY is burned, the bridge deploys a Docker container from a
template image. This template needs:

- Crystal Dragon site builder (pre-built static files)
- Clean default config (NEW_CUSTOMER_DEFAULTS, not the demo config)
- No API key pre-loaded (customer adds their own)
- 30-day trial API key mechanism (uses Nate's key, then expires)
- NFT-gated authentication (wallet connect, not password)
- IPFS client for media storage
- Health endpoint for manager canister monitoring

The template image is built from the Crystal Dragon repo:
```
docker build -t crystaldragon-template:latest .
```

---

## Authentication

- Dashboard uses Internet Identity (ICP auth canister)
- Only Nate's principal has admin access
- Future: add team member principals
- Customers NEVER see the dashboard
- Customers use their Dragon Console on their subdomain

---

## Tech Stack for Dashboard

- Frontend: React (in MyCloud repo at frontend/dashboard/)
- Deployed as: ICP asset canister
- Talks to: auth, registry, manager canisters via @dfinity/agent
- Talks to: VPS Docker API via Agent Zero proxy
- Styling: match Crystal Dragon aesthetic (dark theme, cyan/purple)

---

## Priority Order

1. Fleet Overview (see all sites at a glance)
2. Site Registry (manage individual sites)
3. Canister Health (monitor cycles)
4. Site Detail View (troubleshoot specific sites)
5. Financial Overview (track revenue)
6. Agent Zero Console (advanced)

---

*Share this document with the MyCloud Claude session to build the dashboard.*
