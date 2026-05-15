#!/bin/bash
set -euo pipefail
set -x

SERVER="root@49.235.163.138"
SSH_KEY="$HOME/.ssh/id_ed25519"
SSH="ssh -o StrictHostKeyChecking=no -i $SSH_KEY $SERVER"
REMOTE_DIR="/opt/remote-code"

echo "=== Deploying remote-code-control-plane ==="

# 1. Push code to server
echo ">>> Syncing code..."
$SSH "mkdir -p $REMOTE_DIR/src"
rsync -azP --delete \
    --exclude='.git' \
    --exclude='target' \
    --exclude='node_modules' \
    --exclude='.claude' \
    -e "ssh -i $SSH_KEY" \
    ./ $SERVER:$REMOTE_DIR/src/

# 2. Build on server
echo ">>> Building on server..."
$SSH "source \$HOME/.cargo/env && cd $REMOTE_DIR/src && cargo build --release -p remote-code-control-plane 2>&1 | tail -20"

# 3. Install binary
echo ">>> Installing binary..."
$SSH "mkdir -p $REMOTE_DIR/bin && cp $REMOTE_DIR/src/target/release/remote-code-control-plane $REMOTE_DIR/bin/"

# 4. Install config
echo ">>> Installing config..."
$SSH "if [ ! -f $REMOTE_DIR/control-plane.env ]; then cp $REMOTE_DIR/src/deploy/remote-code-control-plane.env $REMOTE_DIR/control-plane.env; fi"
$SSH "cp $REMOTE_DIR/src/deploy/Caddyfile /etc/caddy/Caddyfile"
$SSH "cp $REMOTE_DIR/src/deploy/remote-code-control-plane.service /etc/systemd/system/"

# 5. Create downloads directory
echo ">>> Setting up downloads directory..."
$SSH "mkdir -p $REMOTE_DIR/downloads"

# 6. Generate secrets if not already set
echo ">>> Checking secrets..."
$SSH "grep -q 'AUTH_TOKEN=.\{16,\}' $REMOTE_DIR/control-plane.env || \
    sed -i 's/^REMOTE_CODE_CONTROL_PLANE_AUTH_TOKEN=.*/REMOTE_CODE_CONTROL_PLANE_AUTH_TOKEN='\$(openssl rand -hex 32)'/' $REMOTE_DIR/control-plane.env"
$SSH "grep -q 'BOOTSTRAP_SECRET=.\{16,\}' $REMOTE_DIR/control-plane.env || \
    sed -i 's/^REMOTE_CODE_CONTROL_PLANE_BOOTSTRAP_SECRET=.*/REMOTE_CODE_CONTROL_PLANE_BOOTSTRAP_SECRET='\$(openssl rand -hex 32)'/' $REMOTE_DIR/control-plane.env"

# 7. Start services
echo ">>> Starting services..."
$SSH "systemctl daemon-reload"
$SSH "systemctl enable remote-code-control-plane caddy"
$SSH "systemctl restart remote-code-control-plane"
$SSH "systemctl restart caddy"

# 8. Verify
echo ">>> Verifying..."
sleep 2
$SSH "systemctl status remote-code-control-plane --no-pager -l | head -15"
echo ""
$SSH "curl -s http://127.0.0.1:8787/healthz | head -5"
echo ""
echo "=== Deploy complete! ==="
echo "Control plane: https://remote-code.yz520gzy.top"
echo "Health check:  https://remote-code.yz520gzy.top/healthz"
echo ""
echo "Secrets are in: $REMOTE_DIR/control-plane.env"
echo "View with:      $SSH cat $REMOTE_DIR/control-plane.env"
