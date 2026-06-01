# Codex Submodule 迁移手册 (2026-06-01)

> **STATUS: COMPLETED** — Migration executed 2026-06-01.
> `crates/codex/` deleted, `agents/codex` converted to git submodule,
> root `Cargo.toml` rewired with `exclude = ["agents/codex"]` + path dependencies.
> Rollback: `git reset --hard pre-codex-submodule-migration-2026-06-01`

> **目标**：把根 workspace 引用的 `crates/codex/*` 切换到上游 `https://github.com/openai/codex.git` 的 `codex-rs/*` 子目录，实现"一行命令即时同步上游最新功能"。

## 背景与动机

### 现状
- `agents/codex/` 已是上游 OpenAI Codex 仓库的完整 git clone（HEAD = `35aaa5d`，7.8G，122 crates）
- `crates/codex/` 是手工 fork（104 crates），根 workspace 引用本地版本
- 两者结构 100% 匹配但内容已分叉
  - 本地有自定义：`sha2`、`notify`、`utils/template`、`utils/readiness`、`ext/memories`
  - 上游有更新：`codex-extension-api`、`codex-guardian`、`codex-image-generation-extension`、`codex-memories-extension`、`codex-web-search-extension`、`codex-file-watcher`

### 目标
1. 根 workspace 直接引用 `agents/codex/codex-rs/*` 路径
2. 本地 patch 通过 `patches/codex/*.patch` 文件管理
3. `git submodule update --remote agents/codex` 拿到上游最新
4. 一次 commit 完成切换，可独立回滚

## 切到 Submodule 的优势

| 优势 | 现状痛点 | Submodule 后 |
|---|---|---|
| 即时同步 | 手动 rsync，易遗漏 | `git submodule update --remote` |
| 本地 patch | 散落在 `crates/codex/*` 各种 hack | 集中 `patches/codex/` 用 `git apply` |
| 上游锁定 | 软依赖（手 fork 可能落后数月） | 硬依赖（git submodule pin commit） |
| 切换风险 | 大爆炸式合并 | 9 个原子 commit 独立 revert |
| 上游协作 | 几乎不可能贡献回去 | fork → PR 路径畅通 |

## 9 阶段执行计划

### P0 — 写完整迁移手册供你审阅
- 本文档
- 验证：你 review 通过

### P1 — 建立切换前快照 + 备份目录
- `git tag pre-codex-submodule-migration-<date>`
- `git mv crates/codex .archive/crates-codex-legacy-2026-06-01`
- 验证：`git tag -l | grep pre-codex`

### P2 — 写 10 项自动化验证脚本
- `scripts/verify-codex-migration.sh`
- 10 项检查：submodule 状态、路径引用、patch 应用、cargo metadata、依赖闭包...
- 验证：脚本 exit 0

### P3 — 把 agents/codex 转成正式 submodule
- 创建 `.gitmodules` 文件
- 在根 `Cargo.toml` 的 `exclude` 中添加 `agents/codex`
- 验证：`git submodule status` 显示 clean

### P4 — 改写根 Cargo.toml 所有 crates/codex/ 路径
- `sed -i 's|crates/codex/|agents/codex/codex-rs/|g' Cargo.toml`
- 手动核对 workspace members 数量（104 → 122）
- 验证：`grep "crates/codex/" Cargo.toml` 应为 0 行

### P5 — 关闭 crates/codex/Cargo.toml 子工作区
- 把 `crates/codex/Cargo.toml` 替换为 3 行 stub（保留作 dev sandbox）
- 验证：`cargo metadata` 不再报 duplicate member

### P6 — 迁移本地 patch 到 patches/codex/ 目录
- 创建 `patches/codex/` 目录
- 提取 `crates/codex/` 中 5 个本地修改为 `.patch` 文件
- 在 `crates/codex/Cargo.toml` 关闭后，patch 改在 `Cargo.toml` 的 `[patch.crates-io]` 中通过 `path = "patches/codex/xxx"` 引用
- 验证：`cargo check -p codex-core` 通过

### P7 — 写 scripts/sync-codex.sh 一键同步脚本
- 5 步骤：fetch → rebase 检查 → apply patches → cargo check → 输出报告
- 支持 `--dry-run`
- 验证：`./scripts/sync-codex.sh --dry-run` 退出 0

### P8 — 完整 cargo check 回归
- `cargo metadata --no-deps` （结构）
- `cargo check --workspace --all-targets` （编译）
- `cargo clippy -p codex-* -- -D warnings` （lint）
- 验证：所有 exit 0

### P9 — 9 个原子 commit
每个 P 阶段一个 commit，commit message 模板：
```
<P阶段>: <简短描述>

- 详细变更
- 影响范围
- 回滚方式：git revert <commit>
```

## 回滚策略

如果任何阶段失败：
```bash
git reset --hard pre-codex-submodule-migration-2026-06-01
```

或单独 revert：
```bash
git revert <commit-sha>  # 撤销特定 P 阶段
```

## 风险与缓解

| 风险 | 缓解 |
|---|---|
| 上游破坏我们依赖的 API | 锁定 commit，不自动 sync |
| patch 冲突 | patch 文件粒度足够小，冲突时人工处理 |
| build 时间变长 | codex-rs 是已编译的，可复用 cache |
| Submodule clone 慢 | CI 缓存 `agents/codex` 目录 |

## 验证检查清单（10 项）

1. [ ] `.gitmodules` 存在且正确
2. [ ] `git submodule status` 显示 clean
3. [ ] `grep "crates/codex/" Cargo.toml` 返回 0 行
4. [ ] `grep "agents/codex/codex-rs/" Cargo.toml` 返回 ≥ 200 行
5. [ ] `cargo metadata --no-deps` exit 0
6. [ ] `cargo check -p codex-core` exit 0
7. [ ] `cargo check -p codex-app-server` exit 0
8. [ ] `cargo check -p rc-codex-adapter` exit 0
9. [ ] `cargo check -p remote-code-gui --features desktop` exit 0
10. [ ] `./scripts/sync-codex.sh --dry-run` exit 0

## 后续工作（不在本次范围）

- 设置 GitHub Action 每周自动 `git submodule update` 并开 PR
- 在 `CODEOWNERS` 中为 `agents/codex/**` 指定上游同步 owner
- 在 `CONTRIBUTING.md` 添加"如何贡献 patch 给上游 codex"流程
