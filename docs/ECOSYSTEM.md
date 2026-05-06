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


## Future architecture: project canisters + Agent Zero (the cloud factory)

Beyond the three core canisters (auth, registry, manager), the long-term
shape of MyCloud is **one canister per project** — small, focused canisters
each owning a domain (hopeandgrace, metiverse, etc.) — with a Python-based
**Agent Zero** running on the VPS that routes incoming work to the right
canister by reading their Candid IDLs.

This pattern is documented separately in `CLOUD_FACTORY.md` — including:
- Why "one canister per project" (not per feature) is the right granularity
- Agent Zero deployment design (Docker, ic-py, port 9603)
- Inter-canister call cost considerations
- Caller-principal verification for trigger methods
- A staged rollout: don't build the cloud factory before there are canisters
  worth routing to.

**Status:** planned for after Checkpoints 3b + 3c + 4 ship. Don't pre-build
the routing layer; build canisters first, the routing pattern emerges
naturally from their Candid interfaces.
