# Tencent Cloud Bring-Up

1. Build and upload `remote-code-control-plane` plus `apps/remote-code-gui/dist`.
2. Create user `remote-code`, then place the env file at `/etc/remote-code/control-plane.env`.
3. Copy [remote-code-control-plane.service](/C:/Users/Yanzh/Desktop/remote-code-rust/deploy/tencent-cloud/remote-code-control-plane.service) to `/etc/systemd/system/`.
4. Copy [Caddyfile.example](/C:/Users/Yanzh/Desktop/remote-code-rust/deploy/tencent-cloud/Caddyfile.example) into Caddy and adjust paths if your install root differs from `/opt/remote-code`.
5. Before first boot, set a strong `REMOTE_CODE_CONTROL_PLANE_BOOTSTRAP_SECRET`, start the service, then run `remote-code remote auth bootstrap` from your trusted machine to mint the first device token.
