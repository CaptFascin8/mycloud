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
