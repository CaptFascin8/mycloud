# MyCloud Port Assignments

The Hostinger VPS `srv825251.hstgr.cloud` hosts **three projects**:
MyCloud, Crystal Dragon, and Hope & Grace. This file documents the
full port landscape so future work doesn't accidentally collide.

MyCloud's own services live in the **96xx** block to avoid collisions
with HOPE Master Stack, Agentic Acres, and Metivi Mesh Studio.

## MyCloud — active

| Service              | Host port | Container port | Bind        | Notes                |
|----------------------|-----------|----------------|-------------|----------------------|
| IPFS RPC API         | 9600      | 5001           | 127.0.0.1   | mycloud-ipfs         |
| IPFS HTTP gateway    | 9601      | 8080           | 127.0.0.1   | proxied by Nginx     |
| IPFS libp2p swarm    | 4001      | 4001           | public      | TCP + UDP            |
| dfx local replica    | 4943      | -              | 127.0.0.1   | only when developing |
| Vite dev server      | 5173      | -              | 127.0.0.1   | only when developing |

## Shared infrastructure on the VPS (system-level, not Docker)

| Service          | Port  | Owner            | Notes                              |
|------------------|-------|------------------|------------------------------------|
| SSH              | 22    | system           | root + keys only                   |
| SMTP             | 25    | system           | postfix outbound mail              |
| HTTP             | 80    | system Nginx     | redirects to HTTPS for all domains |
| HTTPS            | 443   | system Nginx     | TLS termination for all domains    |
| DNS resolver     | 53    | systemd-resolved | localhost only                     |
| Prometheus       | 9090  | system           | metrics scraper                    |
| Prometheus node  | 9100  | system           | node-exporter                      |
| Hope & Grace API | 3000  | PM2 / node       | proxied by Nginx as api.hopeandgrace.space |
| (Python service) | 4200  | system           | TBD — investigate before claiming  |

## Crystal Dragon — Docker stack (NOT owned by MyCloud)

These were running 6 days before MyCloud's IPFS came up. They support
Crystal Dragon's site builder and (eventually) the cloud factory's
agent layer. **Do not stop or modify these without coordination.**

| Container               | Port    | Purpose                              |
|-------------------------|---------|--------------------------------------|
| crystal_dragon_voice    | 5001    | voice service for Guardian assistant |
| crystal_dragon_chromadb | 8000    | vector DB for AI                     |
| crystal_dragon_redis    | 6379    | session + cache for Crystal Dragon   |
| cd_n8n_redis            | 6380    | Redis instance for n8n               |
| cd_n8n_postgres         | 5432    | Postgres for n8n                     |
| n8n_app                 | 5678    | workflow automation                  |
| ollama                  | 11434   | local LLM inference                  |
| agent_zero              | 50001   | Agent Zero web UI (mapped from :80) — see ⚠️ below |
| agent_zero              | 50003   | Agent Zero MCP / HTTP API — see ⚠️ below |
| agent_zero (internal)   | 9000–9009 | Agent Zero worker pool             |

**⚠️ Agent Zero current status (May 12, 2026):** Container is "Up" per
`docker ps` but the Python app inside is crash-looping with
`ModuleNotFoundError: No module named 'langchain_groq'`. Ports 50001
and 50003 currently return HTTP 000 (no listener). Not urgent to fix —
no MyCloud component depends on it yet. See `CLOUD_FACTORY.md` Stage 1
for fix plans when we need it.

When MyCloud's canisters need to call Agent Zero, the endpoint is
`http://127.0.0.1:50003`. When the cloud factory's Stage 1 plan
referenced "Agent Zero on port 9603" — that plan was written before
Agent Zero already existed on this VPS. **The real port is 50003**;
9603 is no longer reserved for that purpose (see "Reserved" below).

## Reserved for future MyCloud services

| Port | Intended use                                          |
|------|-------------------------------------------------------|
| 9602 | Kubo Web UI (if we ever expose it via SSH tunnel)     |
| 9603 | (was Agent Zero; now free, see note above)            |
| 9604 | First MyCloud-resident agent                          |
| 9605 | Next MyCloud-resident agent                           |
| 9606–9620 | Future MyCloud services                          |

When adding a new MyCloud service, claim the next free 96xx port and
update this table in the same commit.

## Audit command

Run on the VPS to see the full current picture:

```bash
docker ps --format 'table {{.Names}}\t{{.Ports}}'
ss -tlnp | awk 'NR>1 {print $4, $NF}' | sort -u
```

If anything new shows up between audits, document it here before it
becomes a port-collision mystery.
