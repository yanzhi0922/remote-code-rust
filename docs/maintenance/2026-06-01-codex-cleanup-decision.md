# Codex 清理决策记录 (2026-06-01)

## 背景

`cargo metadata` 显示 2026-06-01 仓库存在两类"未被根 workspace 集成"的内容：
- `crates/codex/Cargo.toml` 仍是 `[workspace]` 子工作区，123 members 与根 workspace 104 members 高度重叠
- 18 个 `crates/codex/*` 子目录有 `Cargo.toml` 但既不在根 workspace、也不在子工作区

## 全部 18 个孤儿 crate（按字母序）

```
app-server-daemon
aws-auth
bwrap
execpolicy-legacy
ext/extension-api
ext/goal
ext/guardian
ext/image-generation
ext/memories
ext/web-search
file-search
file-watcher
keyring-store
message-history
realtime-webrtc
thread-manager-sample
v8-poc
```

`crates/codex/Cargo.toml` 自身也出现在孤儿列表中——这是子工作区自身引用，不应删除。

## 待你决策

### 决策 1：18 个未集成 codex crate 的去留

请在下方选择其一（A/B/C/D）并在"已选方案"处填入你的选择：

**A. 全部 .gitignore + 从磁盘删除（推荐）** — 视为上游快照残留。git 历史保留（先 `git rm` 再 `rm -rf`）。
**B. 全部保留到磁盘 + 加入 crates/codex/Cargo.toml members** — 子工作区纳管。
**C. 逐个评估** — 用 ext/ 表示扩展（实验），其余归入上游快照分类 .gitignore。
**D. 暂不处理** — 等下一次 codex 上游同步时再决定。

<!-- 已选方案: A -->

### 决策 2：`crates/codex/Cargo.toml` 子工作区去留

**A. 保留子工作区 + 添加 exclude 排除决策 1 中删除的 crate** — 最小变更。
**B. 让 crates/codex/Cargo.toml members 与根完全一致** — 中等变更。
**C. 完全删除子工作区** — 激进。所有 cargo 命令从根工作。

<!-- 已选方案: A -->

### 决策 3：实现位置

填好上述选择后，我会：
1. 决策 1.A：执行 `cat >> .gitignore <<EOF` + `git rm --cached -r <dirs>`
2. 决策 2.A：在 `crates/codex/Cargo.toml` 头部加 `exclude = [ ... ]`
3. 运行 `cargo metadata --no-deps` 验证
4. 单独 commit "chore: codex cleanup 2026-06-01"

