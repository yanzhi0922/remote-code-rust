# Tencent Cloud Bring-Up

This deployment is relay-only. The Tencent Cloud host runs the control plane,
serves the Web/PWA frontend, and stores downloadable app binaries. It must not
run `remote-code-runner`, `remote-code`, Codex/Roo/Claude agent loops, workspace
tools, or provider credentials.

1. Build the Linux `remote-code-control-plane` release binary and `apps/remote-code-gui/dist` on a trusted build machine, then upload only the release binary and built static files. On Windows, use the root [deploy.ps1](../deploy.ps1) script with `-ControlPlaneBin target\x86_64-unknown-linux-gnu\release\remote-code-control-plane`; on Linux/macOS, use [deploy.sh](../deploy.sh). Do not upload a Windows `.exe` as the cloud control-plane binary.
   For the GUI, upload the built directory to a temporary path on the server and then run [deploy-remote-code-gui.sh](deploy-remote-code-gui.sh) so static files land with nginx-safe permissions:
   `sudo bash /opt/remote-code/deploy/tencent-cloud/deploy-remote-code-gui.sh /tmp/remote-code-gui-dist /opt/remote-code/frontend`
2. Create user `remote-code`, then place the env file at `/etc/remote-code/control-plane.env`.
3. Copy [remote-code-control-plane.service](remote-code-control-plane.service) to `/etc/systemd/system/`.
4. Install [nginx-remote-code.conf.example](nginx-remote-code.conf.example) as the nginx site for `remote-code.yz520gzy.top`, or copy [Caddyfile.example](Caddyfile.example) into Caddy if nginx is not already bound to ports 80/443. Do not run both on 80/443.
5. Before first boot, set a strong `REMOTE_CODE_CONTROL_PLANE_BOOTSTRAP_SECRET`, start the service, then run `remote-code remote auth bootstrap` from your trusted machine to mint the first device token.

## Cloud Host Guardrails

- [ ] `systemctl list-units '*remote-code*'` shows `remote-code-control-plane` only.
- [ ] No `remote-code-runner`, `remote-code`, `cargo`, `rustc`, Codex, Roo, or Claude agent process is running on the cloud host.
- [ ] `/opt/remote-code/src` is absent; deployment uploads release artifacts, not the full source tree.
- [ ] Provider API keys and workspace paths are configured only on the user's desktop or trusted runner machine.
- [ ] `/download` and `/downloads/*` are authenticated through the control plane unless intentionally exposed by a separate nginx rule.

---

## PWA / 远程 Web 发布回归检查清单

> 每次远程 Web 改动上线前**必须**逐项确认。

### 版本与缓存

- [ ] `apps/remote-code-gui/src/main.tsx` 中 `SERVICE_WORKER_VERSION` 已更新为新值
- [ ] `sw.js` 通过 URL 参数 `?v=` 读取版本号（**无需手动修改 `sw.js`**）
- [ ] 部署后浏览器加载的 `sw.js?v=NEW_VERSION` 与 `main.tsx` 中的版本一致

### 静态资源

- [ ] 部署脚本使用原子替换（`deploy-remote-code-gui.sh` 自动处理）
- [ ] 目录权限 `755`，文件权限 `644`（部署脚本自动处理）
- [ ] 构建产物中的 hash 文件名已完整替换到目标目录

### 浏览器回归

- [ ] 英文浏览器首次打开正常
- [ ] 中文浏览器首次打开正常
- [ ] 配对成功后刷新仍能恢复认证态
- [ ] active session 恢复正确
- [ ] runner 离线时输入框和 interrupt 同时禁用
- [ ] approvals / artifacts / timeline 同时可见
- [ ] 运行时异常会落到 `AppErrorBoundary`（不会白屏）
- [ ] "清缓存并重载"后可以重新进入页面

### 移动端视口

- [ ] 390×844 视口下首屏无横向溢出
- [ ] session list 可开合
- [ ] composer 可输入中文
- [ ] approvals 可操作
- [ ] artifact 下载入口可点击
