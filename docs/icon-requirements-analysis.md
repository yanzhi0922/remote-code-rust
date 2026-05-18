# Remote Code 图标系统落地报告

审计日期：2026-05-18
范围：桌面软件、移动 App、PWA/远程 Web、安装包/下载页、运行态 UI、Agent/Skill/Plugin 生态、未来可扩展入口。
实施状态：已生成并接入 Remote Code 品牌图标系统；图标母版、桌面/PWA/Android/iOS 预备资源、社交预览和运行态品牌标识均来自同一套可复现生成脚本。

## 1. 项目形态判断

当前仓库不是单一 CLI，而是一个多入口产品：

- Windows 桌面主入口：`apps/remote-code-gui`，Tauri v2 + React 19，当前打包目标为 NSIS 安装包。
- 移动端：同一个 Tauri GUI 工程已有 `tauri.android.conf.json` / `tauri.ios.conf.json`，Android 工程已初始化，iOS 工程尚未生成。
- PWA / 远程 Web：`apps/remote-code-gui/dist` 会作为云端 control-plane 的静态前端发布。
- 云端下载页：`remote-code-control-plane` 提供 `/download` 和 `/downloads/*`，用于分发 APK/IPA/EXE 等二进制。
- CLI / TUI / Runner / Control Plane：发布包里有多个可执行文件，但当前没有面向用户的独立 GUI 图标配置；TUI 内部使用 Unicode 状态符号。
- Codex/Skill/Plugin 生态：协议层支持 skill/plugin 图标字段，未来 marketplace 和 composer 会需要小图标、品牌色和插件图标规范。

因此，“Remote Code”使用一套统一品牌母版图标，再派生桌面、移动、Web、商店、通知、小组件和生态扩展图标。本次设计采用“远程链路 + 代码提示符”的组合符号：环形链路表达桌面/手机/云中继的远控闭环，中心提示符表达开发者和 coding agent，青蓝主色承接专业开发工具气质，黄色光标作为审批/执行动作的视觉锚点。

## 2. 已落地资源盘点

现有图标文件集中在：

- `apps/remote-code-gui/assets/brand/`
- `apps/remote-code-gui/src-tauri/icons/`
- `apps/remote-code-gui/public/`
- `apps/remote-code-gui/src-tauri/android/app/src/main/res/`
- `apps/remote-code-gui/scripts/generate-icons.py`

主要资源：

| 文件 | 尺寸 | 备注 |
|---|---:|---|
| `assets/brand/app-icon-master.svg` | vector | 品牌母版 |
| `assets/brand/app-icon-master-1024.png` | 1024x1024 | 桌面/商店/iOS 源图 |
| `assets/brand/app-icon-maskable-1024.png` | 1024x1024 | PWA/Android maskable 源图 |
| `assets/brand/mark.svg` | vector | 透明品牌符号 |
| `assets/brand/mark-monochrome.svg` | vector | 单色/通知/themed 源 |
| `src-tauri/icons/icon.png` | 512x512 | Tauri 主图标 |
| `public/pwa-icon.png` | 512x512 | 兼容旧引用，与 Tauri 主图标一致 |
| `public/favicon.svg` / `favicon.ico` | vector / multi-size | 浏览器标签页 |
| `public/pwa-icon-192.png` / `pwa-icon-512.png` | 192 / 512 | PWA any 图标 |
| `public/pwa-maskable-192.png` / `pwa-maskable-512.png` | 192 / 512 | PWA maskable 图标 |
| `public/apple-touch-icon.png` | 180x180 | iOS Web clip |
| `public/og-image.png` | 1200x630 | 社交/发布预览 |
| `src-tauri/icons/ios/AppIcon.appiconset` | 19 files | iOS/iPadOS AppIcon 预备资源 |

容器图标实测：

- `icon.ico` 内含 16、24、32、48、64、128、256 七档，覆盖 Windows 常见 shell/taskbar 场景。
- `icon.icns` 内含 `icp4`、`icp5`、`icp6`、`ic07`、`ic08`、`ic09`、`ic10`，覆盖 16 到 1024。

已修复项：

- `index.html` 已从 `/vite.svg` 切换到 Remote Code `favicon.svg` / `favicon.ico` / 32px PNG。
- `manifest.webmanifest` 已补齐 192/512 any、192/512 maskable、monochrome SVG。
- `public/sw.js` 已预缓存品牌图标和 PWA 图标。
- Android `mipmap-*` launcher PNG、`mipmap-anydpi-v26` adaptive XML、monochrome vector、notification small icon 已生成。
- `tauri.android.conf.json` 已移除 Android 不合适的 `.ico` 引用，改用 `icons/icon.png`。
- ActivityBar 和移动初始化页已使用 Remote Code 品牌资产，不再使用 `Bot`/`RC` 占位。
- iOS 工程尚未生成，但 AppIcon.appiconset 预备资源已生成；后续 `tauri ios init` 后可导入。

## 3. 必需图标清单

### 3.1 品牌母版

必须先定义一个单一源头，否则各平台会继续漂移。

建议新增：

| 资产 | 建议路径 | 用途 |
|---|---|---|
| `app-icon-master.svg` | `apps/remote-code-gui/assets/brand/` | 可编辑矢量母版 |
| `app-icon-master-1024.png` | 同上 | Tauri / iOS / store 生成源 |
| `app-icon-maskable-1024.png` | 同上 | Android/PWA maskable 源，保留安全区 |
| `app-icon-monochrome.svg` | 同上 | Android themed icon、通知图标、tray/template icon 源 |
| `mark.svg` | 同上 | Web favicon、下载页、README/social |
| `wordmark.svg` | 同上 | 下载页、发布页、文档页 |

设计约束：

- 16px 下仍能识别，不依赖细线、长文本或复杂透视。
- App icon 不放完整 “Remote Code” 文字；小尺寸会糊。
- 可表达 “本地桌面 + 手机远控 + 安全中继 + AI coding”，但只保留一个核心符号。
- 建议沿用产品 UI 的蓝色/青色/近黑体系：`#2563eb`、`#0891b2`、`#17181a`，避免继续使用当前 Tauri 黄青默认风格。

### 3.2 Windows 桌面 / NSIS

当前实际发布目标是 Windows NSIS，因此这是 P0。

需要：

- `src-tauri/icons/icon.ico`：至少 16、24、32、48、64、128、256 px，32-bit RGBA。
- `src-tauri/icons/32x32.png`、`128x128.png`、`128x128@2x.png`：Tauri bundle 已引用。
- 安装包、卸载器、开始菜单、桌面快捷方式、任务栏、Alt-Tab 都应使用同一个 `.ico`。
- 若未来支持 MSIX / Microsoft Store，再补完整 Store tile 资源：`Square44x44Logo`、`Square71x71Logo`、`Square150x150Logo`、`Square310x310Logo`、`StoreLogo`、`Wide310x150Logo`、targetsize 系列和高对比度版本。

当前缺口：

- 当前 `.ico` 缺 48/64 档位。
- 当前 `Square*Logo` 有部分尺寸，但没有 `Wide310x150Logo`、SplashScreen 和完整 scale/targetsize 系列。
- `bundle.targets` 只有 `nsis`，所以 Store tile 暂时不是发布阻塞。

### 3.3 macOS 桌面

当前 workflow 没有构建 macOS GUI 安装包，但 `tauri.conf.json` 已引用 `.icns`，属于未来桌面发布准备项。

需要：

- 完整 `icon.icns`：16、32、64、128、256、512、1024 系列。
- 若走 Mac App Store，准备 1024x1024 PNG marketing icon，扁平、不透明、无 alpha。
- 若走 DMG，建议另做 DMG background 和 volume icon，但当前未配置，不是必须。

当前缺口：

- `.icns` 只检测到单一 `ic09` 档位，未来 macOS 发布前必须重生成。

### 3.4 Linux 桌面

当前主要发布 CLI 二进制和云端 relay，没有 Linux GUI 打包目标。但 Tauri 跨平台潜在目标需要预留。

需要：

- PNG：16、22、24、32、48、64、128、256、512 px。
- 可选 `scalable.svg`。
- `.desktop` 文件中 `Icon=remote-code` 对应 hicolor icon theme。

当前缺口：

- 只有 32、128、256、512 档位，没有完整 hicolor 系列。

### 3.5 PWA / 远程 Web

这是 P0，因为 Web/PWA 已经随 cloud relay 发布。

需要：

- `favicon.svg`：浏览器标签页首选。
- `favicon.ico`：兼容旧浏览器，包含 16/32/48。
- `favicon-16x16.png`、`favicon-32x32.png`：可选但建议提供。
- `apple-touch-icon.png`：180x180。
- `pwa-icon-192.png`：manifest `purpose: any`。
- `pwa-icon-512.png`：manifest `purpose: any`。
- `pwa-maskable-192.png`、`pwa-maskable-512.png`：manifest `purpose: maskable`，核心图形放在安全区内。
- `pwa-monochrome.svg` 或 PNG：未来支持 monochrome purpose / pinned / badge 时使用。

当前缺口：

- `index.html` favicon 仍是 `/vite.svg`。
- manifest 只有 `/pwa-icon.png` 一个 512 资源。
- `apple-touch-icon` 直接复用 512，实际可用但不精细。
- 没有 Open Graph / Twitter card 图，分享链接时没有品牌预览。

### 3.6 Android 原生 App

Android 工程已存在，manifest 已声明 launcher 图标，因此这是移动端 P0。

需要：

- Adaptive launcher icon：
  - `mipmap-anydpi-v26/ic_launcher.xml`
  - foreground layer：矢量或密度 PNG
  - background layer：纯色/渐变/图层资源
  - `ic_launcher_round.xml`
- Legacy launcher PNG：
  - mdpi 48x48
  - hdpi 72x72
  - xhdpi 96x96
  - xxhdpi 144x144
  - xxxhdpi 192x192
- Adaptive layer PNG 常用生成：
  - mdpi 108x108
  - hdpi 162x162
  - xhdpi 216x216
  - xxhdpi 324x324
  - xxxhdpi 432x432
- Android 13 themed icon：
  - `drawable/ic_launcher_monochrome.xml` 或等价 monochrome 资源。
- 通知小图标：
  - `drawable/ic_stat_remote_code.xml`，白色单色，不能直接使用彩色 app icon。
- Play Console：
  - high-res icon 512x512 PNG，最大 1024KB。
  - feature graphic 1024x500。

当前缺口：

- `src-tauri/android/app/src/main/res/mipmap-*` 目录为空。
- manifest 已引用 `@mipmap/ic_launcher` / `@mipmap/ic_launcher_round`，但实际资源不存在。
- 没有 notification small icon；移动端已有 push/local notification 代码，后续会暴露这个缺口。
- 没有 Android themed monochrome icon。

### 3.7 iOS / iPadOS 原生 App

当前仅有 `tauri.ios.conf.json`，没有 `src-tauri/ios` 工程。正式移动端发布前需要完整 AppIcon asset catalog。

需要：

- 1024x1024 App Store marketing icon，PNG、不透明、无 alpha。
- iPhone/iPad AppIcon.appiconset 常用尺寸：
  - 20pt：2x/3x
  - 29pt：2x/3x
  - 40pt：2x/3x
  - 60pt：2x/3x
  - 76pt：1x/2x
  - 83.5pt：2x
  - 1024pt：1x
- Launch screen：可用系统 storyboard + 品牌 mark，不建议用复杂启动图。

当前缺口：

- iOS 工程未生成。
- 没有 App Store marketing icon。
- 当前 app icon 源带透明背景；iOS/App Store 需要确认导出为不透明版本。

### 3.8 通知、Badge、Deep Link

需要：

- Android notification small icon：单色白图，矢量优先。
- Android notification large icon：可复用 app icon 256/512。
- iOS notification：默认使用 app icon；如启用 badge，只需确认 app icon 在小红点覆盖下仍可识别。
- PWA/Web Push：如果后续开启，应提供 `badge-72.png`、`notification-icon-192.png`。
- Deep link `remotecode://` 使用系统 app icon，无单独图标需求。

当前缺口：

- 没有任何专用通知图标。
- 移动端代码已有 push/local notification 能力，图标资产会成为发布前问题。

### 3.9 运行态 UI 图标

当前 React UI 使用 `lucide-react`，这是合理方向；不需要自绘一套全量 UI 图标。但需要把语义整理成稳定的图标规范。

已出现的主要语义：

- 品牌/Agent：`Bot`
- 会话：`MessageSquare` / `MessageSquareText`
- 设置/主题：`Settings2` / `Sun` / `Moon`
- 安全/审批：`Shield` / `ShieldAlert` / `ShieldCheck`
- 产物/下载/分享：`FileOutput` / `Download` / `Share2`
- 连接状态：`Wifi` / `WifiOff`
- 运行状态：`LoaderCircle` / `AlertTriangle` / `X`
- 上下文/存储：`Database`
- 子任务/批量：`GitBranch` / `Layers`
- MCP/插件：`PlugZap` / `Cable`
- 终端/系统：`TerminalSquare` / `Stethoscope`
- 工作区概览：`Layers3` / `Activity` / `Gauge`
- 输入：`Send` / `Mic` / `Cpu` / `Sparkles`

建议：

- 保留 lucide 作为 UI 操作图标库。
- 品牌 mark 不再用 `Bot` 代替，ActivityBar 顶部和移动初始化页的 `RC` 应替换为真实品牌 mark。
- Claude / Codex / Roo 三个 Agent 不建议直接使用第三方商标，除非确认授权；建议做自有 C/CX/R 字母徽章或统一 Agent badge。
- 文件产物应扩展文件类型图标：code、markdown、image、pdf、archive、diff、log、unknown。
- 权限模式应固定图标映射：ask、allow、deny、sandbox、danger/full-access，避免不同页面语义漂移。

### 3.10 Skill / Plugin / Marketplace 图标

Codex 协议已经支持：

- Skill：`interface.icon_small`、`interface.icon_large`、`brand_color`
- Plugin：`interface.composerIcon`、`composerIconUrl`、`brandColor`

需要规范：

- skill small：建议 SVG 或 64x64/128x128 PNG。
- skill large：建议 512x512 PNG 或 SVG。
- plugin composer icon：建议 24/32px 可读，支持深浅色背景。
- brand color：必须通过对比度检查，不能只靠颜色表达状态。

当前状态：

- 仓库内 sample skills 有示例图标。
- Remote Code 自身没有统一生态图标风格规范。

### 3.11 下载页、README、社交预览

当前 `/download` 页面使用 emoji 区分 APK/IPA/DMG/EXE：

- APK：手机 emoji
- IPA：苹果 emoji
- DMG/APP：电脑 emoji
- EXE/MSI：桌面 emoji
- 其他：包裹 emoji

建议：

- 下载页标题区加入 `mark.svg` 或 64x64 PNG。
- 文件类型图标改为一致的 lucide/SVG 图标，避免跨平台 emoji 渲染差异。
- 增加 `og-image.png` 1200x630，用于 README、GitHub Release、PWA 分享和中继下载页预览。
- 增加 `docs-logo.svg` 用于 README 顶部和文档站。

## 4. 优先级建议

P0：发布前必须修

- 设计/确定 Remote Code 品牌母版图标。
- 用母版重生成 Tauri icons，替换当前默认 Tauri 风格图标。
- 把 `index.html` 的 `/vite.svg` favicon 改成 Remote Code favicon。
- 补 PWA 192/512、maskable、apple-touch-icon。
- 生成 Android `ic_launcher` / `ic_launcher_round` / adaptive icon 资源，保证 manifest 引用存在。
- 准备 iOS 1024 不透明 marketing icon，等 iOS 工程生成后注入 asset catalog。

P1：移动正式发布和体验完整性

- Android notification small icon。
- Android 13 themed monochrome icon。
- Play feature graphic 1024x500。
- App Store / TestFlight 图标资产完整性检查。
- 下载页品牌 mark 和文件类型图标。
- ActivityBar 顶部品牌标识、移动初始化页品牌标识。

P2：生态和未来扩展

- Tray/menu bar template icon。
- 文件关联图标，例如未来的 session/export bundle。
- Skill/plugin marketplace 图标规范。
- Social / Open Graph 图。
- Windows MSIX/Microsoft Store 完整 tile asset。
- macOS DMG 背景/volume icon。

## 5. 推荐生成流程

1. 先产出 `assets/brand/app-icon-master-1024.png` 和 `app-icon-master.svg`。
2. 运行 Tauri 图标生成：
   - `cd apps/remote-code-gui`
   - `npx tauri icon assets/brand/app-icon-master-1024.png`
3. 手动补齐 Web/PWA：
   - `public/favicon.svg`
   - `public/favicon.ico`
   - `public/apple-touch-icon.png`
   - `public/pwa-icon-192.png`
   - `public/pwa-icon-512.png`
   - `public/pwa-maskable-192.png`
   - `public/pwa-maskable-512.png`
4. 更新 `index.html` 和 `manifest.webmanifest`。
5. Android 用 Android Studio Image Asset 或等价脚本生成 adaptive icon、round icon、monochrome icon、notification icon。
6. iOS 在生成 `src-tauri/ios` 后，用 Xcode asset catalog 导入 AppIcon。
7. 重新构建并做小尺寸审查：16、24、32、48、64、128、256、512、1024。

## 6. 验收清单

- Windows 安装包图标、卸载器图标、开始菜单图标、桌面快捷方式、任务栏图标一致。
- 浏览器标签页不再显示 Vite 图标。
- PWA 安装到桌面/手机后，图标不被系统 mask 裁切关键内容。
- Android APK 安装后 launcher、round launcher、通知栏小图标正常。
- iOS/TestFlight 构建不报 missing marketing icon 或 alpha channel 问题。
- 深色/浅色桌面、移动首屏、下载页都能识别同一品牌。
- 16px 仍可识别，1024px 不显粗糙。
- 不直接使用 OpenAI/Anthropic/Google/Apple/Microsoft 商标作为自有 Agent/Provider 图标，除非确认授权。

## 7. 官方参考

- Tauri App Icons：https://v2.tauri.app/develop/icons/
- Apple HIG App Icons：https://developer.apple.com/design/human-interface-guidelines/app-icons
- Android Adaptive Icons：https://developer.android.com/develop/ui/compose/system/icon_design_adaptive
- Google Play preview assets：https://support.google.com/googleplay/android-developer/answer/9866151
- Microsoft Windows app icons：https://learn.microsoft.com/en-us/windows/apps/design/style/app-icons-and-logos
- Windows app icon construction：https://learn.microsoft.com/en-us/windows/apps/design/iconography/app-icon-construction
- MDN PWA app icons：https://developer.mozilla.org/en-US/docs/Web/Progressive_web_apps/How_to/Define_app_icons
- MDN Web App Manifest icons：https://developer.mozilla.org/en-US/docs/Web/Progressive_web_apps/Manifest/Reference/icons
