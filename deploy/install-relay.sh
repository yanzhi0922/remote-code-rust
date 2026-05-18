#!/usr/bin/env bash
set -euo pipefail

REPO="${REMOTE_CODE_REPO:-yanzhi0922/remote-code-rust}"
VERSION="${REMOTE_CODE_VERSION:-latest}"
DOMAIN="${REMOTE_CODE_DOMAIN:-remote-code.yz520gzy.top}"
ACME_EMAIL="${REMOTE_CODE_ACME_EMAIL:-}"
REMOTE_DIR="${REMOTE_CODE_REMOTE_DIR:-/opt/remote-code}"
ENV_DIR="${REMOTE_CODE_ENV_DIR:-/etc/remote-code}"
STATE_DIR="${REMOTE_CODE_STATE_DIR:-/var/lib/remote-code/control-plane}"
BIND_ADDR="${REMOTE_CODE_BIND:-127.0.0.1:8787}"
ARTIFACT="remote-code-cloud-relay-x86_64-unknown-linux-gnu"

usage() {
  cat <<EOF
Usage:
  sudo REMOTE_CODE_DOMAIN=remote-code.example.com REMOTE_CODE_ACME_EMAIL=admin@example.com bash install-relay.sh

Environment:
  REMOTE_CODE_REPO        GitHub repo, default: ${REPO}
  REMOTE_CODE_VERSION     release tag or "latest", default: ${VERSION}
  REMOTE_CODE_DOMAIN      public domain, default: ${DOMAIN}
  REMOTE_CODE_ACME_EMAIL  email for Let's Encrypt certbot automation
  REMOTE_CODE_REMOTE_DIR  install dir, default: ${REMOTE_DIR}
  REMOTE_CODE_ENV_DIR     env dir, default: ${ENV_DIR}
  REMOTE_CODE_STATE_DIR   state dir, default: ${STATE_DIR}
  REMOTE_CODE_BIND        local bind, default: ${BIND_ADDR}
EOF
}

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  usage
  exit 0
fi

if [[ "$(id -u)" -ne 0 ]]; then
  echo "install-relay.sh must run as root; use sudo." >&2
  exit 77
fi

need_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "missing required command: $1" >&2
    exit 69
  fi
}

need_cmd curl
need_cmd tar
need_cmd sha256sum
need_cmd systemctl
need_cmd openssl

if [[ "$VERSION" == "latest" ]]; then
  BASE_URL="https://github.com/${REPO}/releases/latest/download"
else
  BASE_URL="https://github.com/${REPO}/releases/download/${VERSION}"
fi

tmp_dir="$(mktemp -d)"
cleanup() {
  rm -rf "$tmp_dir"
}
trap cleanup EXIT

archive="${tmp_dir}/${ARTIFACT}.tar.gz"
checksum="${tmp_dir}/${ARTIFACT}.sha256"

echo "Downloading ${ARTIFACT} from ${BASE_URL}"
curl -fL --retry 5 --retry-delay 2 -o "$archive" "${BASE_URL}/${ARTIFACT}.tar.gz"
if curl -fL --retry 3 --retry-delay 2 -o "$checksum" "${BASE_URL}/${ARTIFACT}.sha256"; then
  (cd "$tmp_dir" && sha256sum -c "${ARTIFACT}.sha256")
else
  echo "warning: checksum asset not found; continuing without release checksum verification" >&2
fi

mkdir -p "${tmp_dir}/unpack"
tar -C "${tmp_dir}/unpack" -xzf "$archive"

if [[ ! -x "${tmp_dir}/unpack/remote-code-control-plane" ]]; then
  echo "release artifact is missing remote-code-control-plane" >&2
  exit 66
fi
if [[ ! -f "${tmp_dir}/unpack/frontend/index.html" ]]; then
  echo "release artifact is missing frontend/index.html" >&2
  exit 66
fi

if ! id remote-code >/dev/null 2>&1; then
  useradd --system --home-dir /var/lib/remote-code --shell /usr/sbin/nologin remote-code
fi

install -d -m 755 "$REMOTE_DIR" "$REMOTE_DIR/bin" "$REMOTE_DIR/downloads" "$REMOTE_DIR/deploy/tencent-cloud"
install -d -m 750 -o remote-code -g remote-code "$(dirname "$STATE_DIR")" "$STATE_DIR"
install -d -m 750 "$ENV_DIR"

systemctl disable --now remote-code-runner.service remote-code.service >/dev/null 2>&1 || true
pkill -f '/opt/remote-code/bin/remote-code-runner' >/dev/null 2>&1 || true
pkill -f '/opt/remote-code/bin/remote-code($| )' >/dev/null 2>&1 || true

install -m 755 "${tmp_dir}/unpack/remote-code-control-plane" "$REMOTE_DIR/bin/remote-code-control-plane"
cp -a "${tmp_dir}/unpack/deploy/." "$REMOTE_DIR/deploy/"

rm -rf "${REMOTE_DIR}/frontend.tmp"
mkdir -p "${REMOTE_DIR}/frontend.tmp"
cp -a "${tmp_dir}/unpack/frontend/." "${REMOTE_DIR}/frontend.tmp/"
find "${REMOTE_DIR}/frontend.tmp" -type d -exec chmod 755 {} +
find "${REMOTE_DIR}/frontend.tmp" -type f -exec chmod 644 {} +
rm -rf "${REMOTE_DIR}/frontend.prev"
if [[ -e "${REMOTE_DIR}/frontend" ]]; then
  mv "${REMOTE_DIR}/frontend" "${REMOTE_DIR}/frontend.prev"
fi
mv "${REMOTE_DIR}/frontend.tmp" "${REMOTE_DIR}/frontend"

env_file="${ENV_DIR}/control-plane.env"
if [[ ! -f "$env_file" ]]; then
  cp "${tmp_dir}/unpack/deploy/tencent-cloud/remote-code-control-plane.env.example" "$env_file"
fi

ensure_env() {
  local key="$1"
  local value="$2"
  if grep -q "^${key}=" "$env_file"; then
    sed -i "s|^${key}=.*|${key}=${value}|" "$env_file"
  else
    printf '%s=%s\n' "$key" "$value" >>"$env_file"
  fi
}

ensure_env REMOTE_CODE_CONTROL_PLANE_BIND "$BIND_ADDR"
ensure_env REMOTE_CODE_CONTROL_PLANE_PUBLIC_BASE_URL "https://${DOMAIN}"
ensure_env REMOTE_CODE_CORS_ORIGINS "https://${DOMAIN},tauri://localhost,http://tauri.localhost"
ensure_env REMOTE_CODE_PROFILE_DIR "$STATE_DIR"
ensure_env REMOTE_CODE_DOWNLOADS_DIR "$REMOTE_DIR/downloads"
ensure_env REMOTE_CODE_CONTROL_PLANE_RELAY_ONLY true

if ! grep -Eq '^REMOTE_CODE_CONTROL_PLANE_BOOTSTRAP_SECRET=.{16,}$' "$env_file" ||
  grep -q '^REMOTE_CODE_CONTROL_PLANE_BOOTSTRAP_SECRET=change-this-before-first-boot$' "$env_file"; then
  ensure_env REMOTE_CODE_CONTROL_PLANE_BOOTSTRAP_SECRET "$(openssl rand -hex 32)"
fi

chown root:remote-code "$env_file"
chmod 640 "$env_file"
chown -R remote-code:remote-code "$(dirname "$STATE_DIR")" "$REMOTE_DIR/downloads"
rm -rf /root/.remote-code-rust "$REMOTE_DIR/src"

install -m 644 "${tmp_dir}/unpack/deploy/tencent-cloud/remote-code-control-plane.service" /etc/systemd/system/remote-code-control-plane.service
systemctl daemon-reload
systemctl enable remote-code-control-plane
systemctl restart remote-code-control-plane

apt_updated=0
apt_install_packages() {
  if [[ "$apt_updated" -eq 0 ]]; then
    apt-get update
    apt_updated=1
  fi
  DEBIAN_FRONTEND=noninteractive apt-get install -y "$@"
}

if ! command -v nginx >/dev/null 2>&1; then
  apt_install_packages nginx
fi

nginx_conf="/etc/nginx/sites-available/remote-code.conf"
install -d -m 755 /var/www/remote-hub-public

if [[ ! -f "/etc/letsencrypt/live/${DOMAIN}/fullchain.pem" && -n "$ACME_EMAIL" ]]; then
  if ! command -v certbot >/dev/null 2>&1; then
    apt_install_packages certbot python3-certbot-nginx
  fi

  cat >"$nginx_conf" <<EOF
server {
    listen 80;
    server_name ${DOMAIN};

    location /.well-known/acme-challenge/ {
        root /var/www/remote-hub-public;
        allow all;
    }

    location /healthz {
        proxy_pass http://${BIND_ADDR};
        proxy_http_version 1.1;
        proxy_set_header Host \$host;
        proxy_set_header X-Real-IP \$remote_addr;
    }

    location / {
        root ${REMOTE_DIR}/frontend;
        try_files \$uri \$uri/ /index.html;
    }
}
EOF
  ln -sfn "$nginx_conf" /etc/nginx/sites-enabled/remote-code.conf
  nginx -t
  systemctl reload nginx || systemctl restart nginx
  certbot --nginx -d "$DOMAIN" --non-interactive --agree-tos -m "$ACME_EMAIL" --redirect
fi

if [[ -f "/etc/letsencrypt/live/${DOMAIN}/fullchain.pem" ]]; then
  install -m 644 "${tmp_dir}/unpack/deploy/tencent-cloud/nginx-remote-code.conf.example" "$nginx_conf"
  sed -i "s|remote-code.yz520gzy.top|${DOMAIN}|g" "$nginx_conf"
  ln -sfn "$nginx_conf" /etc/nginx/sites-enabled/remote-code.conf
  nginx -t
  systemctl reload nginx
else
  echo "TLS certificate missing for ${DOMAIN}; set REMOTE_CODE_ACME_EMAIL to enable certbot automation." >&2
  echo "The control plane is running on ${BIND_ADDR}, but public HTTPS was not activated." >&2
fi

forbidden_processes="$(pgrep -af '/opt/remote-code/bin/remote-code-runner|/opt/remote-code/bin/remote-code($| )' || true)"
if [[ -n "$forbidden_processes" ]]; then
  echo "Forbidden local runner/agent process is still running on relay host:" >&2
  echo "$forbidden_processes" >&2
  exit 70
fi

systemctl is-active remote-code-control-plane >/dev/null
curl -fsS "http://${BIND_ADDR}/healthz" >/dev/null

echo "Remote Code relay installed."
echo "Public URL: https://${DOMAIN}"
echo "Relay service: remote-code-control-plane"
echo "This host is relay-only. Run coding agents and runners on user desktops, not on this server."
