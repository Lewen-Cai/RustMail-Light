#!/usr/bin/env bash
set -euo pipefail

OS_NAME="$(uname -s)"
case "${OS_NAME}" in
  Linux|Darwin)
    ;;
  *)
    echo "Unsupported operating system: ${OS_NAME}" >&2
    exit 1
    ;;
esac

for cmd in curl openssl docker; do
  if ! command -v "${cmd}" >/dev/null 2>&1; then
    echo "Missing required command: ${cmd}" >&2
    exit 1
  fi
done

if ! docker info >/dev/null 2>&1; then
  echo "Docker daemon is not running or not accessible to current user." >&2
  exit 1
fi

if docker compose version >/dev/null 2>&1; then
  COMPOSE_CMD=(docker compose)
elif command -v docker-compose >/dev/null 2>&1; then
  COMPOSE_CMD=(docker-compose)
else
  echo "Docker Compose is required. Install docker compose plugin or docker-compose." >&2
  exit 1
fi

INSTALL_DIR="${RUSTMAIL_INSTALL_DIR:-/opt/rustmail}"
if [ ! -w "$(dirname "${INSTALL_DIR}")" ]; then
  INSTALL_DIR="${HOME}/rustmail"
  echo "No write permission for /opt. Falling back to ${INSTALL_DIR}."
fi

REPO_RAW_BASE="${RUSTMAIL_REPO_RAW:-https://raw.githubusercontent.com/Lewen-Cai/RustMail-Light/main}"
DOMAIN="${RUSTMAIL_DOMAIN:-mail.example.com}"
ROOT_DOMAIN="${RUSTMAIL_ROOT_DOMAIN:-example.com}"
ACME_EMAIL="${RUSTMAIL_ACME_EMAIL:-admin@${ROOT_DOMAIN}}"

echo "Installing RustMail into ${INSTALL_DIR}"
mkdir -p "${INSTALL_DIR}/deploy" "${INSTALL_DIR}/config" "${INSTALL_DIR}/dkim" "${INSTALL_DIR}/data/mail" "${INSTALL_DIR}/web/mail-ui/dist"

curl -fsSL "${REPO_RAW_BASE}/deploy/docker-compose.yml" -o "${INSTALL_DIR}/deploy/docker-compose.yml"
curl -fsSL "${REPO_RAW_BASE}/deploy/Caddyfile" -o "${INSTALL_DIR}/deploy/Caddyfile"

JWT_SECRET="$(openssl rand -hex 32)"
POSTGRES_PASSWORD="$(openssl rand -hex 18)"
REDIS_PASSWORD="$(openssl rand -hex 18)"

cat > "${INSTALL_DIR}/.env" <<EOF
DOMAIN=${DOMAIN}
ACME_EMAIL=${ACME_EMAIL}
POSTGRES_DB=rustmail
POSTGRES_USER=rustmail
POSTGRES_PASSWORD=${POSTGRES_PASSWORD}
REDIS_PASSWORD=${REDIS_PASSWORD}
EOF

cat > "${INSTALL_DIR}/config/rustmaild.toml" <<EOF
[database]
url = "postgres://rustmail:${POSTGRES_PASSWORD}@postgres:5432/rustmail"
max_connections = 20

[smtp]
bind = "0.0.0.0:25"
submission_bind = "0.0.0.0:587"
smtps_bind = "0.0.0.0:465"
starttls = true

[imap]
bind = "0.0.0.0:993"

[pop3]
bind = "0.0.0.0:995"

[api]
bind = "0.0.0.0:8080"

[auth]
jwt_secret = "${JWT_SECRET}"

[logging]
level = "info"
format = "json"
EOF

if [ ! -f "${INSTALL_DIR}/dkim/private.key" ]; then
  openssl genrsa -out "${INSTALL_DIR}/dkim/private.key" 2048 >/dev/null 2>&1
  openssl rsa -in "${INSTALL_DIR}/dkim/private.key" -pubout -out "${INSTALL_DIR}/dkim/public.pem" >/dev/null 2>&1
fi

DKIM_PUBLIC_KEY="$(grep -v '-----' "${INSTALL_DIR}/dkim/public.pem" | tr -d '\n')"

cat > "${INSTALL_DIR}/web/mail-ui/dist/index.html" <<'EOF'
<!doctype html>
<html lang="en">
  <head><meta charset="utf-8" /><title>RustMail</title></head>
  <body>
    <h1>RustMail is running</h1>
    <p>Build and upload web/mail-ui/dist for the full UI.</p>
  </body>
</html>
EOF

(
  cd "${INSTALL_DIR}/deploy"
  "${COMPOSE_CMD[@]}" --env-file ../.env up -d
)

echo
echo "RustMail started. Configure these DNS records:"
echo "MX    ${ROOT_DOMAIN}         10 ${DOMAIN}."
echo "TXT   ${ROOT_DOMAIN}         \"v=spf1 mx -all\""
echo "TXT   mail._domainkey.${ROOT_DOMAIN} \"v=DKIM1; k=rsa; p=${DKIM_PUBLIC_KEY}\""
echo "TXT   _dmarc.${ROOT_DOMAIN}  \"v=DMARC1; p=quarantine; rua=mailto:postmaster@${ROOT_DOMAIN}\""
echo
echo "Config files:"
echo "- ${INSTALL_DIR}/.env"
echo "- ${INSTALL_DIR}/config/rustmaild.toml"
echo "- ${INSTALL_DIR}/dkim/private.key"
