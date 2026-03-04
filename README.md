# RustMail

RustMail is a modern mail server stack built in Rust, with SMTP handling, an HTTP API, and a React web console for mailbox operations.

## Features

- Rust-based core services focused on performance and memory safety
- SMTP service and API daemon packaged as `rustmaild`
- Web console (`web/mail-ui`) built with React + TypeScript + Vite
- Postgres for persistent data, Redis for cache/session workloads
- Caddy reverse proxy with automatic HTTPS and static UI hosting
- Container-first deployment with Docker Compose

## Quick Install

For a fresh server (Linux/macOS with Docker installed):

```bash
curl -fsSL https://raw.githubusercontent.com/Lewen-Cai/RustMail-Light/main/scripts/install.sh | bash
```

You can customize installer variables:

```bash
RUSTMAIL_DOMAIN=mail.example.com \
RUSTMAIL_ROOT_DOMAIN=example.com \
RUSTMAIL_ACME_EMAIL=admin@example.com \
curl -fsSL https://raw.githubusercontent.com/Lewen-Cai/RustMail-Light/main/scripts/install.sh | bash
```

## Manual Installation

1. Clone the repository.
2. Build the web UI:

```bash
cd web/mail-ui
npm install
npm run build
```

3. Prepare runtime files:

```bash
mkdir -p config
cp config.example.toml config/rustmaild.toml
```

4. Create a `.env` file in the project root:

```dotenv
DOMAIN=mail.example.com
ACME_EMAIL=admin@example.com
POSTGRES_DB=rustmail
POSTGRES_USER=rustmail
POSTGRES_PASSWORD=replace-this-password
REDIS_PASSWORD=replace-this-password
```

5. Start the stack:

```bash
docker compose -f deploy/docker-compose.yml --env-file .env up -d
```

## Configuration

- Main daemon config example: `config.example.toml`
- Runtime config path used by Compose: `config/rustmaild.toml`
- Frontend API URL (optional): `VITE_API_BASE_URL` (defaults to `/api/v1`)
- Vite local API proxy target: `VITE_API_PROXY_TARGET` (defaults to `http://localhost:8080`)

Important sections in `rustmaild.toml`:

- `[database]` Postgres connection and pool settings
- `[smtp]` SMTP bind addresses and TLS behavior
- `[api]` HTTP API bind address
- `[auth]` JWT secret and token TTL settings
- `[logging]` log level and log format

## DNS Guide

For domain `example.com` and mail host `mail.example.com`:

- `MX` record: `example.com -> 10 mail.example.com.`
- `A/AAAA` record: `mail.example.com -> your server IP`
- `SPF` TXT: `v=spf1 mx -all`
- `DKIM` TXT (selector `mail`): `v=DKIM1; k=rsa; p=<public-key>`
- `DMARC` TXT (`_dmarc.example.com`): `v=DMARC1; p=quarantine; rua=mailto:postmaster@example.com`

The install script generates DKIM keys under `dkim/` and prints the exact TXT value.

## Development

Backend:

```bash
cargo check --workspace
cargo test --workspace
cargo run -p rustmaild -- --config config/rustmaild.toml
```

Frontend:

```bash
cd web/mail-ui
npm install
npm run dev
```

CI pipeline (`.github/workflows/ci.yml`) runs:

- `cargo check`
- `cargo clippy`
- `cargo test`
- Docker image build (`deploy/Dockerfile`)

## License

This project is dual-licensed under MIT or Apache-2.0.
