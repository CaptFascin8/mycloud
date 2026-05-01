# Setup

You're on Ubuntu 24.04 (WSL on HOPE or directly on the VPS — same commands).

## 1. Rust + Wasm target
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
rustup target add wasm32-unknown-unknown
```

## 2. dfx
```bash
sh -ci "$(curl -fsSL https://internetcomputer.org/install.sh)"
dfx --version
```

## 3. Node 20 (for the dashboard)
```bash
curl -fsSL https://deb.nodesource.com/setup_20.x | sudo -E bash -
sudo apt-get install -y nodejs
node --version
```

## 4. First build
```bash
cd /mnt/c/MY_CLOUD
cargo check --workspace
dfx start --background --clean
dfx deploy --network local
dfx canister call auth whoami
```

## VPS setup (Checkpoint 4)
```bash
ssh root@82.25.91.136
apt update && apt upgrade -y
apt install -y docker.io docker-compose-plugin ufw certbot
systemctl enable --now docker
ufw default deny incoming
ufw default allow outgoing
ufw allow 22/tcp
ufw allow 80/tcp
ufw allow 443/tcp
ufw allow 4001
ufw enable

# from local:
rsync -avz vps/docker/ root@82.25.91.136:/opt/mycloud/

# back on VPS:
cd /opt/mycloud
docker compose up -d
docker compose logs -f ipfs
```
