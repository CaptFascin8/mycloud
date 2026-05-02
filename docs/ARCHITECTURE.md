# MyCloud — What This Is

## The one-paragraph version

MyCloud is a sovereign hybrid cloud. The "trust" parts (identity, ownership, metadata) live on the Internet Computer Protocol (ICP) as Rust canisters running on a decentralized blockchain. The "weight" parts (file storage, HTTP serving, AI agents) live on a Hostinger VPS you fully control. The two halves talk through the canister's HTTP outcall feature and through public IPFS gateways. The result: nobody except you can take your sites or your data offline, but you don't have to maintain a blockchain to get those guarantees.

## Why "sovereign"

Most clouds are someone else's computer. MyCloud is yours:
- The VPS is rented, but the OS, code, and data are yours
- Canisters are deployed to ICP but owned by your principal — only you can upgrade them
- IPFS content is mirrored peer-to-peer; even if your VPS goes down, files are retrievable from any node that pinned them

You can shut Hostinger off tomorrow and re-provision the VPS half on AWS, your basement, or a Raspberry Pi in 30 minutes. The canister half keeps running on ICP regardless.

## The three parts

### Part 1 — Canisters (the trust layer)

Three Rust canisters compiled to WebAssembly and deployed to ICP:

- **auth** — Internet Identity binding + per-user credential vault. Log in with II, get a Principal, the canister stores your record in stable memory. Stash labeled blobs (API keys, tokens) under your Principal.

- **registry** — Smartsites. A smartsite is a named site whose ownership is provable and whose content is on IPFS. Today ownership = "registered under your Internet Identity." Soon: also "you hold this Solana NFT (specifically a Yggdrasil KEY)" — verified trustlessly via HTTP outcall.

- **manager** — The watchdog. Polls auth and registry every minute, keeps a ring buffer of recent events, alerts when cycles get low. Future: self-healing — when a health check fails, manager calls a "healer" agent on the VPS that can `docker restart` whatever is broken.

### Part 2 — VPS (the weight layer)

Your Hostinger 4-vCPU Ubuntu box runs three things:

- **Nginx** terminates TLS (Let's Encrypt cert), proxies `/ipfs/<cid>` paths to the IPFS gateway, serves the dashboard frontend. The only thing talking to the public internet.

- **IPFS (Kubo)** is the storage backbone. Files get a CID (a hash); the CID is the address. Anyone with the CID can retrieve the file from any IPFS node that has it. Your VPS pins everything you care about; other IPFS nodes worldwide can mirror it for free redundancy.

- **Agents** are Docker containers that do work canisters can't (AI tasks, filesystem operations, anything stateful or compute-heavy). Invoked by the manager canister via HTTP outcall.

### Part 3 — Frontend (the human layer)

A Vite + React dashboard. Today a placeholder; eventually it'll let you:
- Log in with Internet Identity
- Manage smartsites (register, update CIDs, transfer ownership)
- Inspect the credential vault
- See manager's health timeline
- Upload files, get CIDs, share gateway URLs

The dashboard can be served as a canister asset (fully on-chain) or as static files from VPS Nginx. Both work.

## Smartsites — the unifying idea

A "smartsite" is the unit of value MyCloud exists to host:
- A **domain** (or subdomain — myart.crystaldragon.tech)
- An **owner** (Internet Identity principal, or Solana wallet via NFT)
- An **IPFS CID** (the content)
- An **ownership proof** (today: II; tomorrow: also Solana NFT)

The same data structure powers:
- Personal sites (you register, you own, you serve)
- **Crystal Dragon claimed sites** — NFT-gated; owning a KEY tier NFT lets you claim a subdomain in your tier's namespace
- **Agentic Acres agent homes** — a nomadic AI agent like Sally has a smartsite for her "current address"

This is why the registry is chain-agnostic from day one: same canister, three use cases, one source of truth.

## Why both ICP and a VPS?

**Pure VPS** would mean trusting your own server for identity. If hacked, attacker rewrites your "who owns what" database. Single point of failure for trust.

**Pure ICP** would mean storing everything on-chain. ICP charges per byte and per instruction. Storing video/photo/large files would be expensive. Full app-server workloads don't fit cleanly.

**Hybrid** puts the right thing in the right place:
- Trust + ownership on the chain (cheap, immutable, verifiable)
- Bytes + serving on the VPS (cheap per byte, fast, easily replaced)

The two halves don't trust each other — VPS proves it has a file by giving you a CID; canister proves you own the site by checking the chain. Neither can lie about its half.

## What this is NOT

- **Not a startup.** No business model, no users to scale to.
- **Not a blockchain product.** ICP is a tool, not the point.
- **Not a marketplace.** Crystal Dragon is its own thing; this is the infrastructure under it.
- **Not "mine forever."** The point is sovereignty, which means anyone could fork this and run their own. Take it with you whenever you want.