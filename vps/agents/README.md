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
