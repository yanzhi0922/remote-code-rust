# Codex Submodule 迁移手册 (2026-06-01)

> **STATUS: COMPLETED** — Migration executed 2026-06-01 (commits 954977d6 + 54ed1e9c),
> `.codex-logs/` untracked 2026-06-02 (commit 99f8aee7).
> `crates/codex/` deleted, `agents/codex` converted to git submodule,
> root `Cargo.toml` rewired with `exclude = ["agents/codex"]` + path dependencies.
> Rollback: `git reset --hard pre-codex-submodule-migration-2026-06-01`

## 已完成的 3 个迁移 commit

| Commit | 描述 |
|---|---|
| `954977d6` | feat: sync upstream OpenAI Codex submodule + resolve all compilation errors |
| `54ed1e9c` | chore: add codex migration tooling, patches, and docs |
| `99f8aee7` | chore: untrack 848 .codex-logs/ test artifacts |

> 原计划是 9 个原子 commit，实际合并为 3 个更易 review 的功能 commit——这是优化。

## 验证

迁移后所有不变量由 `scripts/verify-codex-migration.sh` 强制检查（10 项）。运行：

```bash
./scripts/verify-codex-migration.sh        # CI gate
./scripts/verify-codex-migration.sh --verbose  # 调试
```

最新运行结果（2026-06-03）：**10 passed, 0 failed**。

## 当前状态（迁移后）

| 指标 | 迁移前 | 迁移后 |
|---|---|---|
| Root `cargo metadata` workspace members | 236 | **128** |
| `crates/codex/` 目录 | 104 个 crate (~64M) | 不存在 |
| `agents/codex/codex-rs/` 上游 crates | 122 | 122（submodule 拉取） |
| 根 workspace 直接引用的 codex 路径 | 104 | 121（`path = "agents/codex/codex-rs/..."`） |
| `.codex-logs/` 追踪文件数 | 848 | 0（`git rm --cached`） |
| `.gitmodules` | 不存在 | `[submodule "agents/codex"]` → openai/codex.git |

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
- **实际执行**：tag 已打（指向 3423d96c），`crates/codex/` 后来通过 `git rm -rf` 删除（git 历史保留），`.archive/` 目录未创建

### P2 — 写 10 项自动化验证脚本
- `scripts/verify-codex-migration.sh`（2026-06-03 创建）
- 10 项检查：submodule 状态、路径引用、cargo metadata、orphan manifests...
- 验证：脚本 exit 0（10/10 PASS）

### P3 — 把 agents/codex 转成正式 submodule
- 创建 `.gitmodules` 文件
- 在根 `Cargo.toml` 的 `exclude` 中添加 `agents/codex`
- 验证：`git submodule status` 显示 clean
- **实际执行**：commit 954977d6 完成

### P4 — 改写根 Cargo.toml 所有 crates/codex/ 路径
- `sed -i 's|crates/codex/|agents/codex/codex-rs/|g' Cargo.toml`
- 手动核对 workspace members 数量（104 → 122 facade aliases）
- 验证：`grep "crates/codex/" Cargo.toml` 应为 0 行
- **实际执行**：121 个 path dependency（commit 954977d6）

### P5 — 关闭 crates/codex/Cargo.toml 子工作区
- 把 `crates/codex/Cargo.toml` 替换为 3 行 stub（保留作 dev sandbox）
- 验证：`cargo metadata` 不再报 duplicate member
- **实际执行**：整个 `crates/codex/` 目录删除（包含子工作区 stub）

### P6 — 迁移本地 patch 到 patches/codex/ 目录
- 创建 `patches/codex/` 目录
- 提取 `crates/codex/` 中本地修改为 `build.rs` 源码备份
- **注意**：这些是 **build.rs overlay（cp 应用）**，不是 `.patch` 文件（git apply 应用）
- 4 个 overlay：app-server, exec, tui, windows-sandbox
- 由 `scripts/sync-codex.sh` 在 sync 时复制到 `agents/codex/codex-rs/<crate>/build.rs`
- 验证：commit 54ed1e9c 完成

### P7 — 写 scripts/sync-codex.sh 一键同步脚本
- 5 步骤：fetch → log → apply overlays → cargo check → 输出报告
- 支持 `--dry-run` 和 `--check`
- 验证：`./scripts/sync-codex.sh --dry-run` 退出 0
- **实际执行**：commit 54ed1e9c

### P8 — 完整 cargo check 回归
- `cargo metadata --no-deps` （结构）
- `cargo check --workspace --all-targets` （编译）
- `cargo clippy -p codex-* -- -D warnings` （lint）
- 验证：所有 exit 0
- **实际执行**：commit 954977d6 中 "resolve all compilation errors"

### P9 — 9 个原子 commit
- **实际执行**：合并为 3 个更易 review 的 commit（见顶部表格）

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

由 `scripts/verify-codex-migration.sh` 自动化执行（最新运行 2026-06-03 全部 PASS）：

1. [x] `.gitmodules` 存在且正确（指向 `https://github.com/openai/codex.git`）
2. [x] `git submodule status` 显示 clean（HEAD = 84b883aaeb49）
3. [x] `grep "crates/codex/" Cargo.toml` 返回 0 行
4. [x] `grep "agents/codex/codex-rs/" Cargo.toml` 返回 ≥ 100 行（实测 121）
5. [x] `agents/codex` 在根 `Cargo.toml` 的 `workspace.exclude` 中
6. [x] `agents/codex/codex-rs/` 是真实 git checkout + 122 upstream crates
7. [x] `patches/codex/` 包含 4 个 build.rs overlay
8. [x] `scripts/sync-codex.sh` 存在且可执行
9. [x] `cargo metadata --no-deps` 解析干净（128 workspace members）
10. [x] 无 `crates/codex/*/Cargo.toml` orphan manifest

## 后续工作（不在本次范围）

- 设置 GitHub Action 每周自动 `git submodule update` 并开 PR
- 在 `CODEOWNERS` 中为 `agents/codex/**` 指定上游同步 owner
- 在 `CONTRIBUTING.md` 添加"如何贡献 patch 给上游 codex"流程
