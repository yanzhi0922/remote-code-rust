# 验收审计报告 — Remote Code Rust

> **审计日期**: 2026-04-26  
> **审计范围**: 全代码库（Rust crates + 前端 GUI + 移动端）  
> **审计目的**: 交付验收前全面问题排查  
> **基准提交**: `306b7bb`（占位符修复后）

---

## 一、审计概要

| 类别 | 扫描项 | 结果 |
|------|--------|------|
| 残留占位符 | `todo!()` / `unimplemented!()` | ✅ **0 处** |
| 残留标记 | `// TODO` / `FIXME` / `HACK` / `XXX` / `STUB` / `PLACEHOLDER` | ✅ **0 处** |
| 硬编码密钥 | API Key / Secret / Password 明文 | ✅ **0 处** |
| `unsafe` 代码 | `unsafe_code = "forbid"` 工作区级禁用 | ✅ **通过** |
| TypeScript `any` | `: any` 类型注解 | ✅ **0 处** |
| 生产代码 `unwrap()` | 非 `#[cfg(test)]` 的 `.unwrap()` | ✅ **0 处** |
| 生产代码 `expect()` | 非 `#[cfg(test)]` 的 `.expect()` | ✅ **0 处** |
| Clippy 警告 | `cargo clippy` 全工作区 | ⚠️ **15 处** |
| 生产 `console.log` | 前端非测试代码 | ⚠️ **1 处** |
| 死代码标记 | `#[allow(dead_code)]` | ⚠️ **40 处** |
| ESLint 忽略 | `// eslint-disable` | ⚠️ **3 处** |
| 未使用参数 | `let _ =` 丢弃模式（生产代码） | ⚠️ **约 15 处需关注** |
| 测试模块覆盖 | `#[cfg(test)] mod tests` | ✅ **249 个模块** |

---

## 二、问题清单

### P1 — 高优先级（功能缺陷 / 未完成实现）

#### P1-01: `post_failure_hook` 忽略所有参数 — 空实现

- **文件**: [`crates/rc-tools/src/tool_hooks.rs`](crates/rc-tools/src/tool_hooks.rs:124)
- **代码**:
  ```rust
  let _ = (tool_name, tool_input, error);
  Ok(None)
  ```
- **影响**: 工具执行失败后的后置钩子完全无效，无法记录失败日志、触发告警或执行恢复逻辑
- **建议**: 实现真正的失败后处理逻辑，至少应记录错误信息

#### P1-02: `SessionMemoryCompactor::compact()` 忽略关键参数

- **文件**: [`crates/rc-compact/src/session_memory.rs`](crates/rc-compact/src/session_memory.rs:96)
- **代码**:
  ```rust
  let _ = options;
  let _ = provider;
  ```
- **影响**: `CompactOptions`（压缩策略选项）和 `SummaryProvider`（摘要生成器）被完全忽略，压缩功能无法按配置执行，也无法使用 LLM 生成摘要
- **建议**: 将 `options` 和 `provider` 传递给 `session_memory_compact()` 或其内部调用

#### P1-03: TUI 命令处理函数大量忽略 `config` 参数

- **文件与行号**:
  - [`crates/rc-tui/src/commands/keybindings.rs`](crates/rc-tui/src/commands/keybindings.rs:70) — `dispatch()` 中 `let _ = config`
  - [`crates/rc-tui/src/commands/misc_commands.rs`](crates/rc-tui/src/commands/misc_commands.rs:19) — `render_ide()` 中 `let _ = config`
  - [`crates/rc-tui/src/commands/mode_commands.rs`](crates/rc-tui/src/commands/mode_commands.rs:243) — `let _ = config`
  - [`crates/rc-tui/src/commands/mode_commands.rs`](crates/rc-tui/src/commands/mode_commands.rs:283) — `let _ = config`
  - [`crates/rc-tui/src/commands/security.rs`](crates/rc-tui/src/commands/security.rs:59) — `let _ = config`
- **影响**: TUI 命令无法读取运行时配置（如 IDE 连接状态、权限模式等），显示的信息可能不准确
- **建议**: 各命令函数应使用 `config` 获取运行时状态并展示给用户

#### P1-04: `handle_resize` 中 `width` 参数未使用

- **文件**: [`crates/rc-tui/src/event.rs`](crates/rc-tui/src/event.rs:180)
- **代码**:
  ```rust
  fn handle_resize(app: &mut App, width: u16, height: u16) {
      let _ = (width, height);  // height 实际在第 185 行使用了
      app.scroll.set_viewport_height(
          height.saturating_sub(3).max(1) as usize,
      );
  }
  ```
- **影响**: `width` 被丢弃未使用，终端宽度变化不会触发任何布局调整；`let _ = (width, height)` 掩盖了 `height` 实际被使用的事实
- **建议**: 使用 `width` 更新布局宽度；修正 `let _` 为仅丢弃 `width`

---

### P2 — 中优先级（代码质量 / 潜在风险）

#### P2-01: Clippy 警告 — 15 处

| 文件 | 行号 | 警告类型 | 数量 |
|------|------|----------|------|
| [`crates/rc-utils/src/session_restore.rs`](crates/rc-utils/src/session_restore.rs) | 213, 218, 236, 269 | `collapsible_if`、borrowed expression | 4 |
| [`crates/rc-skill-search/src/remote_loader.rs`](crates/rc-skill-search/src/remote_loader.rs) | 206 | `is_multiple_of` | 3 |
| [`crates/rc-ide/src/bridge.rs`](crates/rc-ide/src/bridge.rs) | 319, 369 | `collapsible_if` | 2 |
| [`crates/rc-ide/src/connection.rs`](crates/rc-ide/src/connection.rs) | 214, 230, 256, 263, 476, 519 | `collapsible_if`、borrowed expression、char comparison | 6 |

- **影响**: 代码不够规范，可读性降低
- **建议**: 运行 `cargo clippy --fix` 自动修复大部分

#### P2-02: 生产代码中的 `console.log`

- **文件**: [`apps/remote-code-gui/src/hooks/system/useSpeechInput.ts`](apps/remote-code-gui/src/hooks/system/useSpeechInput.ts:119)
- **影响**: 语音转写功能为 stub 实现，生产环境会在浏览器控制台输出调试信息
- **建议**: 移除 `console.log` 或替换为条件性调试日志

#### P2-03: `#[allow(dead_code)]` 标注 — 40 处

- **分布**:
  - `rc-telemetry` — 5 处（指标/链路字段未使用）
  - `rc-tui` — 8 处（主题、样式、补全函数）
  - `rc-ide` — 4 处（JSON-RPC 结构体字段、write_framed_message）
  - `rc-tools` — 5 处（代理配置字段、辅助函数）
  - `rc-runtime-prompt` — 3 处（记忆相关函数）
  - `rc-query-engine` — 2 处
  - `rc-lsp` — 2 处（client 字段）
  - `rc-swarm` — 2 处
  - 其他各 crate 零散分布
- **影响**: 可能存在未完成的功能或遗留代码，增加维护负担
- **建议**: 逐一审查，删除真正无用的代码，或完成对应功能

#### P2-04: 前端 ESLint 忽略 — 3 处

- **文件**:
  - [`apps/remote-code-gui/src/hooks/ui/useDebounce.ts`](apps/remote-code-gui/src/hooks/ui/useDebounce.ts:43) — `react-hooks/exhaustive-deps`
  - [`apps/remote-code-gui/src/hooks/ui/useThrottle.ts`](apps/remote-code-gui/src/hooks/ui/useThrottle.ts:43) — `react-hooks/exhaustive-deps`
  - [`apps/remote-code-gui/src/components/layout/McpTab.tsx`](apps/remote-code-gui/src/components/layout/McpTab.tsx:172) — `react-hooks/exhaustive-deps`
- **影响**: 可能导致 React Hook 依赖不完整，引发 stale closure 问题
- **建议**: 检查依赖数组是否完整，移除不必要的 eslint-disable

#### P2-05: Linux Landlock 沙箱未集成

- **文件**: [`crates/rc-tools/src/sandbox.rs`](crates/rc-tools/src/sandbox.rs:65)
- **代码**: `let _ = workspace;` 在非 macos/linux/windows 平台分支
- **影响**: Linux 平台的 Landlock 沙箱策略定义了但未在执行路径中使用（`execute_basic` 而非 Landlock 专用路径）
- **建议**: 集成 Landlock Linux 内核特性，或明确文档说明当前 Linux 仅使用基础沙箱

#### P2-06: `DatadogExporter` 和 `FirstPartyExporter` 无真实 HTTP 发送

- **文件**: [`crates/rc-analytics/src/exporter.rs`](crates/rc-analytics/src/exporter.rs:64)
- **现状**: `DatadogExporter::export()` 和 `FirstPartyExporter::export()` 使用 `reqwest::Client` 发送请求，但测试中未验证真实端点可达性
- **影响**: 分析事件导出功能在生产环境可能静默失败
- **建议**: 添加导出失败的告警机制和重试逻辑

---

### P3 — 低优先级（建议改进）

#### P3-01: `let _ =` 火忘模式 — 约 15 处值得关注

虽然大部分 `let _ =` 是合理的（fire-and-forget 通道发送、best-effort 清理），以下场景值得关注：

| 文件 | 行号 | 丢弃内容 | 风险评估 |
|------|------|----------|----------|
| [`crates/rc-tools/src/misc.rs`](crates/rc-tools/src/misc.rs:913) | 913-914 | `std::fs::remove_file` 语音临时文件 | 低 — 临时文件未清理 |
| [`crates/rc-tools/src/hooks.rs`](crates/rc-tools/src/hooks.rs) | 多处 | hook 执行结果 | 低 — 错误已通过 stderr 输出 |
| [`crates/rc-utils/src/chrome_extension.rs`](crates/rc-utils/src/chrome_extension.rs:259) | 259 | `cmdkey /delete` 结果 | 低 — 卸载时清理 |
| [`crates/rc-provider/src/lib.rs`](crates/rc-provider/src/lib.rs:730) | 730-734 | 调试 dump 文件写入 | 低 — 调试功能 |
| [`crates/rc-provider/src/streaming.rs`](crates/rc-provider/src/streaming.rs:737) | 737-741 | 流式响应 dump | 低 — 调试功能 |

#### P3-02: 命令注入防护 — 已有但需持续关注

- **现状**: 代码库中有 300+ 处 `Command::new()` 调用，大部分使用 `.args()` 传参（安全），少数使用 shell 解释器（`sh -c`、`cmd /C`）
- **正面**: 权限系统有路径验证（`validate_path`），插件验证有 ".." 检查，worktree slug 有路径段校验
- **建议**: 对所有 `sh -c` / `cmd /C` 调用点进行定期审计

#### P3-03: 文件操作错误处理

- **现状**: 232 处 `std::fs::*` 调用，生产代码中大部分使用 `?` 传播错误或 `.context()` 添加说明
- **正面**: 关键路径（文件写入、配置保存）使用 atomic write（先写临时文件再 rename）
- **建议**: 无需立即修改，但建议后续统一使用 `atomic_write` 模式

---

## 三、安全审计

| 检查项 | 状态 | 说明 |
|--------|------|------|
| `unsafe` 代码 | ✅ 通过 | 工作区级 `unsafe_code = "forbid"` |
| 硬编码密钥 | ✅ 通过 | 所有密钥通过环境变量/Keychain 读取 |
| 路径遍历防护 | ✅ 通过 | `validate_path`、`validate_path_within_base`、插件 ".." 检查 |
| 命令注入防护 | ✅ 基本通过 | 大部分使用 `.args()` 传参；少数 shell 调用需关注 |
| 反序列化安全 | ✅ 通过 | 使用 `serde_json` 强类型反序列化，无 `serde_json::Value` 直接信任 |
| 敏感数据存储 | ✅ 通过 | 使用系统 Keychain（macOS Keychain / Windows cmdkey / Linux secret-tool） |
| CORS/CSRF | N/A | Tauri 桌面应用，无浏览器 CORS 场景 |
| 依赖安全 | ⚠️ 未检查 | 建议运行 `cargo audit` 检查已知漏洞 |

---

## 四、测试覆盖评估

| 指标 | 数据 |
|------|------|
| Rust 测试模块数 | **249 个** `#[cfg(test)] mod tests` |
| 集成测试文件 | **12 个**（`crates/rc-integration-tests/tests/`） |
| 前端测试文件 | **19 个** `.test.ts` / `.test.tsx` |
| 总测试用例数 | **862+**（`cargo test` 通过） |

### 缺少测试的关键模块

以下生产模块没有对应的 `#[cfg(test)]` 测试：

| 模块 | 说明 |
|------|------|
| `crates/rc-tools/src/sandbox.rs` | 沙箱执行 — 有基础测试但缺少 Landlock/Seatbelt 集成测试 |
| `crates/rc-tools/src/web_browser.rs` | 浏览器截图 — 依赖外部浏览器，难以单元测试 |
| `crates/rc-voice/src/stt.rs` / `tts.rs` | 语音识别/合成 — 依赖外部 Whisper/系统 TTS |
| `apps/remote-code-gui/src/remote/` | 远程连接 — 多数有测试但覆盖不完整 |
| `apps/remote-code-mobile/src/native/` | 原生桥接 — 依赖 Capacitor 插件 |

---

## 五、前端专项审计

| 检查项 | 结果 |
|--------|------|
| TypeScript 严格模式 | ✅ `tsconfig.json` 启用 `strict` |
| `any` 类型使用 | ✅ 0 处 |
| `as any` 类型断言 | ✅ 0 处 |
| `@ts-ignore` / `@ts-expect-error` | ✅ 0 处 |
| 空 catch 块 | ✅ 0 处 |
| 生产 `console.log/warn/error` | ⚠️ 1 处（`useSpeechInput.ts`） |
| ESLint 忽略 | ⚠️ 3 处（`react-hooks/exhaustive-deps`） |
| PWA 配置 | ✅ `manifest.webmanifest` + `sw.js` 存在 |

---

## 六、依赖审计建议

以下命令应在验收前执行：

```bash
# 1. Rust 依赖漏洞扫描
cargo install cargo-audit
cargo audit

# 2. 前端依赖漏洞扫描
cd apps/remote-code-gui && npm audit
cd apps/remote-code-mobile && npm audit

# 3. 编译验证（全平台特性）
cargo build --all-features --workspace

# 4. 完整测试套件
cargo test --all-features --workspace

# 5. Clippy 修复
cargo clippy --fix --allow-dirty --workspace
```

---

## 七、问题统计汇总

| 优先级 | 数量 | 说明 |
|--------|------|------|
| **P1 高** | **4** | 功能缺陷/未完成实现 |
| **P2 中** | **6** | 代码质量/潜在风险 |
| **P3 低** | **3** | 建议改进 |
| **总计** | **13** | 需交付方处理的问题 |

---

## 八、正面发现（交付质量确认）

以下方面质量良好，确认通过：

1. **零残留占位符** — 无 `todo!()`、`unimplemented!()`、`TODO`/`FIXME` 注释
2. **零生产 panic 风险** — 所有 `unwrap()`/`expect()` 均在 `#[cfg(test)]` 内
3. **零硬编码密钥** — 所有敏感信息通过环境变量或系统密钥链读取
4. **零 TypeScript `any`** — 前端类型安全
5. **全面的测试覆盖** — 249 个测试模块，862+ 测试用例
6. **安全机制完善** — 路径验证、权限系统、沙箱执行、插件验证
7. **Atomic Write 模式** — 关键文件写入使用临时文件+rename 保证原子性
8. **强类型反序列化** — 使用 serde 强类型，不直接信任 JSON Value

---

> **审计结论**: 代码库整体质量良好，安全机制完善。发现 **4 个高优先级问题**（未完成的功能实现）和 **6 个中优先级问题**（代码质量），建议在验收前要求交付方处理所有 P1 问题，P2 问题可协商处理时限。
