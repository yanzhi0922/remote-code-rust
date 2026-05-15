#!/bin/bash
set -euo pipefail

SERVER="${SERVER:-root@49.235.163.138}"
SSH_KEY="${SSH_KEY:-$HOME/.ssh/id_ed25519}"
DOMAIN="${REMOTE_CODE_DOMAIN:-remote-code.yz520gzy.top}"
REMOTE_DIR="${REMOTE_DIR:-/opt/remote-code}"
ENV_DIR="${ENV_DIR:-/etc/remote-code}"
STATE_DIR="${STATE_DIR:-/var/lib/remote-code/control-plane}"
CONTROL_PLANE_BIN="${CONTROL_PLANE_BIN:-target/release/remote-code-control-plane}"
GUI_DIST="${GUI_DIST:-apps/remote-code-gui/dist}"
INSTALLER="${INSTALLER:-target/release/bundle/nsis/Remote Code_0.1.0_x64-setup.exe}"

SSH=(ssh -o StrictHostKeyChecking=no -i "$SSH_KEY" "$SERVER")
SCP=(scp -o StrictHostKeyChecking=no -i "$SSH_KEY")

require_file() {
  if [[ ! -f "$1" ]]; then
    echo "missing required file: $1" >&2
    exit 66
  fi
}

require_dir() {
  if [[ ! -d "$1" ]]; then
    echo "missing required directory: $1" >&2
    exit 66
  fi
}

require_file "$CONTROL_PLANE_BIN"
require_dir "$GUI_DIST"
require_file "$GUI_DIST/index.html"

tmp_dir="$(mktemp -d)"
cleanup() {
  rm -rf "$tmp_dir"
}
trap cleanup EXIT

frontend_archive="$tmp_dir/remote-code-gui-dist.tar.gz"
tar -C "$GUI_DIST" -czf "$frontend_archive" .

echo "=== Deploying relay-only remote-code cloud host ==="
echo "Server: $SERVER"
echo "Domain: $DOMAIN"
echo "Control plane binary: $CONTROL_PLANE_BIN"
echo "Frontend dist: $GUI_DIST"

"${SSH[@]}" "mkdir -p /tmp/remote-code-deploy"
"${SCP[@]}" "$CONTROL_PLANE_BIN" "$SERVER:/tmp/remote-code-deploy/remote-code-control-plane"
"${SCP[@]}" "$frontend_archive" "$SERVER:/tmp/remote-code-deploy/remote-code-gui-dist.tar.gz"
"${SCP[@]}" deploy/tencent-cloud/deploy-remote-code-gui.sh "$SERVER:/tmp/remote-code-deploy/deploy-remote-code-gui.sh"
"${SCP[@]}" deploy/tencent-cloud/remote-code-control-plane.service "$SERVER:/tmp/remote-code-deploy/remote-code-control-plane.service"
"${SCP[@]}" deploy/tencent-cloud/remote-code-control-plane.env.example "$SERVER:/tmp/remote-code-deploy/control-plane.env.example"
"${SCP[@]}" deploy/tencent-cloud/nginx-remote-code.conf.example "$SERVER:/tmp/remote-code-deploy/nginx-remote-code.conf.example"

if [[ -f "$INSTALLER" ]]; then
  "${SCP[@]}" "$INSTALLER" "$SERVER:/tmp/remote-code-deploy/Remote-Code-0.1.0-x64-setup.exe"
fi

"${SSH[@]}" "DOMAIN='$DOMAIN' REMOTE_DIR='$REMOTE_DIR' ENV_DIR='$ENV_DIR' STATE_DIR='$STATE_DIR' bash -s" <<'REMOTE_SCRIPT'
set -euo pipefail

if ! id remote-code >/dev/null 2>&1; then
  useradd --system --home-dir /var/lib/remote-code --shell /usr/sbin/nologin remote-code
fi

install -d -m 755 "$REMOTE_DIR" "$REMOTE_DIR/bin" "$REMOTE_DIR/downloads" "$REMOTE_DIR/deploy/tencent-cloud"
install -d -m 750 -o remote-code -g remote-code "$(dirname "$STATE_DIR")" "$STATE_DIR"
install -d -m 750 "$ENV_DIR"

systemctl disable --now remote-code-runner.service remote-code.service >/dev/null 2>&1 || true
pkill -f '/opt/remote-code/bin/remote-code-runner' >/dev/null 2>&1 || true
pkill -f '/opt/remote-code/bin/remote-code($| )' >/dev/null 2>&1 || true

install -m 755 /tmp/remote-code-deploy/remote-code-control-plane "$REMOTE_DIR/bin/remote-code-control-plane"
install -m 755 /tmp/remote-code-deploy/deploy-remote-code-gui.sh "$REMOTE_DIR/deploy/tencent-cloud/deploy-remote-code-gui.sh"
install -m 644 /tmp/remote-code-deploy/nginx-remote-code.conf.example "$REMOTE_DIR/deploy/tencent-cloud/nginx-remote-code.conf.example"

if [[ -f /tmp/remote-code-deploy/Remote-Code-0.1.0-x64-setup.exe ]]; then
  install -m 644 /tmp/remote-code-deploy/Remote-Code-0.1.0-x64-setup.exe "$REMOTE_DIR/downloads/Remote-Code-0.1.0-x64-setup.exe"
fi

if [[ ! -f "$ENV_DIR/control-plane.env" ]]; then
  if [[ -f "$REMOTE_DIR/control-plane.env" ]]; then
    cp "$REMOTE_DIR/control-plane.env" "$ENV_DIR/control-plane.env"
  else
    cp /tmp/remote-code-deploy/control-plane.env.example "$ENV_DIR/control-plane.env"
  fi
fi

ensure_env() {
  local key="$1"
  local value="$2"
  if grep -q "^${key}=" "$ENV_DIR/control-plane.env"; then
    sed -i "s|^${key}=.*|${key}=${value}|" "$ENV_DIR/control-plane.env"
  else
    printf '%s=%s\n' "$key" "$value" >>"$ENV_DIR/control-plane.env"
  fi
}

ensure_env REMOTE_CODE_CONTROL_PLANE_BIND 127.0.0.1:8787
ensure_env REMOTE_CODE_CONTROL_PLANE_PUBLIC_BASE_URL "https://${DOMAIN}"
ensure_env REMOTE_CODE_CORS_ORIGINS "https://${DOMAIN},tauri://localhost,http://tauri.localhost"
ensure_env REMOTE_CODE_PROFILE_DIR "$STATE_DIR"
ensure_env REMOTE_CODE_DOWNLOADS_DIR "$REMOTE_DIR/downloads"

if ! grep -Eq '^REMOTE_CODE_CONTROL_PLANE_BOOTSTRAP_SECRET=.{16,}$' "$ENV_DIR/control-plane.env" ||
  grep -q '^REMOTE_CODE_CONTROL_PLANE_BOOTSTRAP_SECRET=change-this-before-first-boot$' "$ENV_DIR/control-plane.env"; then
  ensure_env REMOTE_CODE_CONTROL_PLANE_BOOTSTRAP_SECRET "$(openssl rand -hex 32)"
fi

chown root:remote-code "$ENV_DIR/control-plane.env"
chmod 640 "$ENV_DIR/control-plane.env"

rm -rf /root/.remote-code-rust "$REMOTE_DIR/src"
chown -R remote-code:remote-code "$(dirname "$STATE_DIR")" "$REMOTE_DIR/downloads"

rm -rf /tmp/remote-code-gui-dist
mkdir -p /tmp/remote-code-gui-dist
tar -C /tmp/remote-code-gui-dist -xzf /tmp/remote-code-deploy/remote-code-gui-dist.tar.gz
bash "$REMOTE_DIR/deploy/tencent-cloud/deploy-remote-code-gui.sh" /tmp/remote-code-gui-dist "$REMOTE_DIR/frontend"

install -m 644 /tmp/remote-code-deploy/remote-code-control-plane.service /etc/systemd/system/remote-code-control-plane.service
systemctl daemon-reload
systemctl enable remote-code-control-plane
systemctl restart remote-code-control-plane

if command -v nginx >/dev/null 2>&1; then
  install -m 644 /tmp/remote-code-deploy/nginx-remote-code.conf.example /etc/nginx/sites-available/remote-code.conf
  ln -sfn /etc/nginx/sites-available/remote-code.conf /etc/nginx/sites-enabled/remote-code.conf
  nginx -t
  systemctl reload nginx
fi

rm -rf /tmp/remote-code-deploy /tmp/remote-code-gui-dist "$REMOTE_DIR/src"

forbidden_processes="$(pgrep -af '/opt/remote-code/bin/remote-code-runner|/opt/remote-code/bin/remote-code($| )' || true)"
if [[ -n "$forbidden_processes" ]]; then
  echo "Forbidden local runner/agent process is still running on relay host:" >&2
  echo "$forbidden_processes" >&2
  exit 70
fi

systemctl is-active remote-code-control-plane
curl -fsS http://127.0.0.1:8787/healthz >/dev/null
REMOTE_SCRIPT

echo "=== Deploy complete ==="
echo "Control plane: https://$DOMAIN"
echo "The cloud host is relay-only; run coding agents and runners on trusted desktop machines."
