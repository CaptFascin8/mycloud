# MyCloud Operations Manual

Daily operating guide. If something is broken at 11pm, this is the file you want.

## Important context: the VPS is shared

`srv825251.hstgr.cloud` hosts THREE projects, not just MyCloud:

- **crystaldragon.tech** + **www.crystaldragon.tech** — Crystal Dragon's
  static site (served from `/usr/share/nginx/html`)
- **hopeandgrace.space** + **www.hopeandgrace.space** + **api.hopeandgrace.space** —
  Hope & Grace charitable network (static frontend + PM2-backed API on port 3000)
- **srv825251.hstgr.cloud** — MyCloud's IPFS gateway and (eventually) dashboard

This means:
- **System-level Nginx** is the front door for all three projects. Don't
  stop or replace it.
- **TLS certs** for all three are managed by a single `certbot` install
  with auto-renewal in `/etc/letsencrypt/renewal/`.
- **PM2** manages the Hope & Grace API. Don't kill PM2 processes you
  don't recognize.
- **MyCloud's docker stack** runs IPFS Kubo only — Nginx duties are
  handled by the system-level install.

If you find yourself thinking "let me just take port 80," stop. Read
this section again. There are real production services on this box.

## Quick reference

| Task | Command |
|------|---------|
| Connect to VPS | `ssh mycloud` (from WSL) or VS Code Remote-SSH |
| Open project in editor | VS Code → Ctrl+Shift+P → Remote-SSH: Connect to Host → mycloud → Open Folder /opt/mycloud |
| Compile all canisters | `cargo check --workspace` |
| Deploy auth canister | `bash scripts/test_auth.sh` |
| Check dfx replica health | `dfx ping` |
| Restart dfx replica | `bash /tmp/dfx-restart.sh` |
| Check git history | `git log --oneline` |
| See what changed | `git status` then `git diff` |

## Where everything lives

### On the Hostinger VPS (srv825251.hstgr.cloud, IP 82.25.91.136)
- `/opt/mycloud/` — the project (canisters, frontend, docker compose)
- `/root/.cargo/bin/` — Rust + Cargo binaries
- `/root/.local/share/dfx/bin/` — dfx binary
- `/root/.cache/dfinity/` — dfx-managed binaries (PocketIC, replica)
- `/var/log/dfx.log` — dfx daemon output
- `/tmp/dfx-restart.sh` — clean dfx restart script

### On HOPE (your local Windows machine via WSL)
- `C:\MY_CLOUD\` — local mirror of the project
- `~/.ssh/mycloud_vps` (in WSL) — private SSH key
- `C:\Users\Nathaniel Brown\.ssh\mycloud_vps` — Windows-side copy for VS Code
- `C:\Users\Nathaniel Brown\.ssh\config` — SSH alias config

## How to access the cloud

### Option A — VS Code (recommended for editing)
1. Open VS Code on Windows
2. Ctrl+Shift+P → Remote-SSH: Connect to Host → mycloud
3. Pick Linux if asked
4. File → Open Folder → /opt/mycloud → trust the workspace
5. Open integrated terminal with Ctrl+` (backtick)
6. You're now editing files on the VPS as if they were local

### Option B — Pure SSH (for quick checks)
From WSL on HOPE:
```
ssh mycloud
ssh -n mycloud 'command here'
```

### Option C — Browser (Candid UI, when dfx is running)
Forward port 4943, then open in browser:
```
ssh -L 4943:127.0.0.1:4943 mycloud
```

## Typical daily flow

1. Open VS Code, Remote-SSH to mycloud, open /opt/mycloud
2. Edit code in the editor (rust-analyzer gives hover types + autocomplete)
3. Save (Ctrl+S)
4. In integrated terminal: `cargo check --workspace`
5. Fix any errors shown (rust-analyzer flags them inline before you compile)
6. `bash scripts/test_auth.sh` (or test_registry.sh, etc.)
7. `git add -A && git commit -m "what you changed"`
8. `git push origin main` (once GitHub remote is set up)

## Troubleshooting

### "dfx: command not found"
Shell didn't load PATH. Run with `bash -l` or:
```
export PATH="$HOME/.cargo/bin:$HOME/.local/share/dfx/bin:$PATH"
```

### "dfx ping says cannot connect to local replica"
Replica isn't running, or died. Restart cleanly:
```
pkill -9 -x pocket-ic
bash /tmp/dfx-restart.sh
```
Verify: `dfx ping` should return `replica_health_status: healthy`.

### Canister deploy fails with "Failed during wasm installation call"
Replica is wedged. Same fix:
```
pkill -9 -x pocket-ic
rm -rf .dfx
bash /tmp/dfx-restart.sh
bash scripts/test_auth.sh
```

### Candid parser error: "Unexpected token"
Used a Candid reserved keyword as a field name. Reserved list:
bool, nat, int, nat8/16/32/64, int8/16/32/64, float32/64, text, null,
reserved, empty, principal, blob, vec, opt, record, variant, func, service,
query, oneway. Rename the field.

### "dubious ownership in repository"
Files were chowned to a different user. Fix:
```
chown -R root:root /opt/mycloud
git config --global --add safe.directory /opt/mycloud
```

## Restarting the whole stack (nuclear option)

```
ssh mycloud
pkill -9 -x pocket-ic
cd /opt/mycloud
rm -rf .dfx target
bash /tmp/dfx-restart.sh
cargo check --workspace
bash scripts/test_auth.sh
```
Takes ~5 minutes for a complete cold start.

## Backups

| What | Where | How often |
|------|-------|-----------|
| Source code (3 copies) | HOPE + VPS + GitHub | Every commit |
| Canister IDs (.dfx/) | VPS only — gitignored | Wiped on `--clean` |
| Stable canister state | Inside the canister, on the replica | Survives upgrades, NOT --clean |
| SSH keys | HOPE WSL + Windows copy | Manual, don't lose them |

If you lose the VPS: rebuild from HOPE in ~30 minutes.
If you lose HOPE: VPS still has everything plus git history.
If you lose both: GitHub has the source.

## Stopping for the night

Nothing required. dfx keeps running cheaply. If you want to be tidy:
```
ssh -n mycloud 'pkill -9 -x pocket-ic'
```
Tomorrow morning: `bash /tmp/dfx-restart.sh` and you're back.