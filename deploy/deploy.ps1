param(
    [string]$Server = "root@49.235.163.138",
    [string]$SshKey = "$env:USERPROFILE\.ssh\id_ed25519",
    [string]$Domain = "remote-code.yz520gzy.top",
    [string]$RemoteDir = "/opt/remote-code",
    [string]$EnvDir = "/etc/remote-code",
    [string]$StateDir = "/var/lib/remote-code/control-plane",
    [string]$ControlPlaneBin = "target\x86_64-unknown-linux-gnu\release\remote-code-control-plane",
    [string]$GuiDist = "apps\remote-code-gui\dist",
    [string]$Installer = "target\release\bundle\nsis\Remote Code_0.1.0_x64-setup.exe",
    [string]$SshStrictHostKeyChecking = $env:REMOTE_CODE_SSH_STRICT_HOST_KEY_CHECKING
)

$ErrorActionPreference = "Stop"

function Require-File([string]$Path) {
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "Missing required file: $Path"
    }
}

function Require-Directory([string]$Path) {
    if (-not (Test-Path -LiteralPath $Path -PathType Container)) {
        throw "Missing required directory: $Path"
    }
}

function Quote-RemoteShell([string]$Value) {
    return "'" + ($Value -replace "'", "'\''") + "'"
}

Require-File $ControlPlaneBin
Require-Directory $GuiDist
Require-File (Join-Path $GuiDist "index.html")
Require-File $SshKey

if ([string]::IsNullOrWhiteSpace($SshStrictHostKeyChecking)) {
    $SshStrictHostKeyChecking = "yes"
}
$SshOptions = @("-o", "BatchMode=yes", "-o", "StrictHostKeyChecking=$SshStrictHostKeyChecking", "-i", $SshKey)

if ([System.IO.Path]::GetExtension($ControlPlaneBin) -ieq ".exe") {
    throw "ControlPlaneBin must be a Linux remote-code-control-plane binary, not a Windows .exe"
}

$tempRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("remote-code-deploy-" + [System.Guid]::NewGuid())
New-Item -ItemType Directory -Path $tempRoot | Out-Null

try {
    $frontendArchive = Join-Path $tempRoot "remote-code-gui-dist.tar.gz"
    tar -C $GuiDist -czf $frontendArchive .

    Write-Host "=== Deploying relay-only remote-code cloud host ==="
    Write-Host "Server: $Server"
    Write-Host "Domain: $Domain"
    Write-Host "Control plane binary: $ControlPlaneBin"
    Write-Host "Frontend dist: $GuiDist"

    & ssh @SshOptions $Server "mkdir -p /tmp/remote-code-deploy"
    & scp @SshOptions $ControlPlaneBin "$Server`:/tmp/remote-code-deploy/remote-code-control-plane"
    & scp @SshOptions $frontendArchive "$Server`:/tmp/remote-code-deploy/remote-code-gui-dist.tar.gz"
    & scp @SshOptions "deploy\tencent-cloud\deploy-remote-code-gui.sh" "$Server`:/tmp/remote-code-deploy/deploy-remote-code-gui.sh"
    & scp @SshOptions "deploy\tencent-cloud\audit-relay-host.sh" "$Server`:/tmp/remote-code-deploy/audit-relay-host.sh"
    & scp @SshOptions "deploy\tencent-cloud\remote-code-control-plane.service" "$Server`:/tmp/remote-code-deploy/remote-code-control-plane.service"
    & scp @SshOptions "deploy\tencent-cloud\remote-code-control-plane.env.example" "$Server`:/tmp/remote-code-deploy/control-plane.env.example"
    & scp @SshOptions "deploy\tencent-cloud\nginx-remote-code.conf.example" "$Server`:/tmp/remote-code-deploy/nginx-remote-code.conf.example"

    if (Test-Path -LiteralPath $Installer -PathType Leaf) {
        & scp @SshOptions $Installer "$Server`:/tmp/remote-code-deploy/Remote-Code-0.1.0-x64-setup.exe"
    }

    $remoteScript = @'
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
install -m 755 /tmp/remote-code-deploy/audit-relay-host.sh "$REMOTE_DIR/deploy/tencent-cloud/audit-relay-host.sh"
install -m 644 /tmp/remote-code-deploy/nginx-remote-code.conf.example "$REMOTE_DIR/deploy/tencent-cloud/nginx-remote-code.conf.example"

if [ -f /tmp/remote-code-deploy/Remote-Code-0.1.0-x64-setup.exe ]; then
  install -m 644 /tmp/remote-code-deploy/Remote-Code-0.1.0-x64-setup.exe "$REMOTE_DIR/downloads/Remote-Code-0.1.0-x64-setup.exe"
fi

if [ ! -f "$ENV_DIR/control-plane.env" ]; then
  if [ -f "$REMOTE_DIR/control-plane.env" ]; then
    cp "$REMOTE_DIR/control-plane.env" "$ENV_DIR/control-plane.env"
  else
    cp /tmp/remote-code-deploy/control-plane.env.example "$ENV_DIR/control-plane.env"
  fi
fi

ensure_env() {
  key="$1"
  value="$2"
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
ensure_env REMOTE_CODE_CONTROL_PLANE_RELAY_ONLY true

if ! grep -Eq '^REMOTE_CODE_CONTROL_PLANE_BOOTSTRAP_SECRET=.{16,}$' "$ENV_DIR/control-plane.env" ||
  grep -q '^REMOTE_CODE_CONTROL_PLANE_BOOTSTRAP_SECRET=change-this-before-first-boot$' "$ENV_DIR/control-plane.env"; then
  ensure_env REMOTE_CODE_CONTROL_PLANE_BOOTSTRAP_SECRET "$(openssl rand -hex 32)"
fi

chown root:remote-code "$ENV_DIR/control-plane.env"
chmod 640 "$ENV_DIR/control-plane.env"

rm -rf /root/.remote-code-rust "$REMOTE_DIR/src" "$REMOTE_DIR/.git" "$REMOTE_DIR/Cargo.toml" "$REMOTE_DIR/Cargo.lock" "$REMOTE_DIR/crates" "$REMOTE_DIR/apps" "$REMOTE_DIR/agents" "$REMOTE_DIR/.research"
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

rm -rf /tmp/remote-code-deploy /tmp/remote-code-gui-dist "$REMOTE_DIR/src" "$REMOTE_DIR/.git" "$REMOTE_DIR/Cargo.toml" "$REMOTE_DIR/Cargo.lock" "$REMOTE_DIR/crates" "$REMOTE_DIR/apps" "$REMOTE_DIR/agents" "$REMOTE_DIR/.research"

forbidden_processes="$(pgrep -af '/opt/remote-code/bin/remote-code-runner|/opt/remote-code/bin/remote-code($| )' || true)"
if [ -n "$forbidden_processes" ]; then
  echo "Forbidden local runner/agent process is still running on relay host:" >&2
  echo "$forbidden_processes" >&2
  exit 70
fi

systemctl is-active remote-code-control-plane
curl -fsS http://127.0.0.1:8787/healthz >/dev/null
bash "$REMOTE_DIR/deploy/tencent-cloud/audit-relay-host.sh"
'@

    $remoteCommand = "DOMAIN=$(Quote-RemoteShell $Domain) REMOTE_DIR=$(Quote-RemoteShell $RemoteDir) ENV_DIR=$(Quote-RemoteShell $EnvDir) STATE_DIR=$(Quote-RemoteShell $StateDir) bash -s"
    $remoteScript | & ssh @SshOptions $Server $remoteCommand

    Write-Host "=== Deploy complete ==="
    Write-Host "Control plane: https://$Domain"
    Write-Host "The cloud host is relay-only; run coding agents and runners on trusted desktop machines."
}
finally {
    Remove-Item -LiteralPath $tempRoot -Recurse -Force -ErrorAction SilentlyContinue
}
