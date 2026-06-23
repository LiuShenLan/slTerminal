---
paths:
   - "plan/execute/**/*结果验证.md"
description: "Phase 验收审计方法论——记录 Phase 0/1 审计积累的审查流程、常见陷阱、技术事实速查和证据判断准绳"
---

# Phase 验收审计规则

本文件在读取 `plan/execute/` 下以"结果验证"结尾的 md 文件时自动加载。记录 Phase 0 三轮 + Phase 1 三轮审计积累的审查方法论、常见陷阱和证据判断准绳。

## 0. 审计核心原则

**先查决策表 + 取一手证据，再下"失实"结论。** 本轮三次误判均因跳过此步。

## 1. 证据判据（从强到弱）

| 层级 | 来源 | 示例 | 可信度 |
|------|------|------|--------|
| L0 | 实跑命令 | `cargo test`、`npm run wdio`、`npx tauri info` | 最高 |
| L1 | 一手源文件 | `action.yml`（GitHub raw）、`Cargo.lock`、`ci.yml` | 高 |
| L2 | 项目决策表 | `plan/execute/Phase 0 执行计划.md` 中的 E1–E5 决策 | 高（既定决策是审计上限） |
| L3 | 联网一手资料 | crate 官方文档、Tauri 官方 changelog、npm registry | 中 |
| L4 | 二手搜索总结 | Web 搜索的 AI 摘要、社区博客 | 低（本轮以此为判据导致 §3.1 误判） |
| L5 | 被审报告自述 | `Phase 0 结果验证.md` 中的声称 | 待验证（不可直接采信） |

- **只有 L0–L2 证据矛盾时才可标"失实"**。L3–L4 仅作线索，不作判据。
- CI 全绿需 `gh run view` 独立复核；本环境若无 `gh`，标明"未独立复核"。

## 2. 审计流程（按顺序执行）

1. **读报告** → 提取所有"声称"（测试通过、CI 绿、版本对齐、配置生效等）。
2. **查决策表** → `grep -n '^| E' plan/execute/Phase\ 0\ 执行计划.md`。对照 E1–E5 决策，确认报告的收口声称是否与既定决策一致。**决策表说了"去自定义 CI 检查"（如 E4），就不要要求"加 CI 检查"。**
3. **取一手证据** → 按 §1 层级逐条验证：
   - 代码一致性：`Bash cat`/`grep` 读取 `ci.yml`、`lib.rs`、`Cargo.toml`、`package.json`、`version-pins.md`、`test.e2e.ts` 等
   - 锁文件验证：`grep` Cargo.lock / package-lock.json 核对版本
   - action 默认值：以 `raw.githubusercontent.com/<owner>/<repo>/<ref>/action.yml` 为准
   - Tauri 版本兼容性：以 `npx tauri info` 是否报 mismatch 为准
4. **实跑命令** → `cargo test`、`npm test`、`npm run build`、`npm run wdio`（至少跑 L1+L2+L4）。
5. **联网检索** → 对不确定的技术点（如 tauri 版本检查行为），搜索后汇总，再判断。二手总结仅作线索，以一手资料核实。
6. **问题分类** → 区分"失实"（报告与证据矛盾）、"缺漏"（未覆盖的交付项）、"小修建议"（无害改进）、"已知限制"。
7. **grill 决策** → 每个需用户定夺的发现，用 `AskUserQuestion` 提一个问题，给推荐答案（作为第一个选项）。不重复 grill 已定决策。

## 3. 已知易误判的技术事实（勿重复踩坑）

### 3.1 CI 相关
- `actions-rust-lang/setup-rust-toolchain@v1` 的 `rustflags` 输入**默认值 = `"-D warnings"`**。不传 `with: rustflags` ⇒ CI 已启用 `-D warnings`。传 `rustflags: ""` 才是关闭。
- 判断 action 默认值以 `action.yml` 为准，不要信 web 搜索的二手总结。参见 `memory/setup-rust-toolchain-default-deny-warnings.md`。

### 3.2 Tauri 版本检查
- Tauri CLI 2.8+ 内置 JS/Rust 版本兼容性检查。`npx tauri info` 若**无 mismatch 提示** ⇒ pair 兼容。
- 项目 E4 决策已定"精确钉 + tauri build 内置检测"为版本一致性方案，**不要建议额外 CI 检查**。
- `@tauri-apps/api` 最新即 2.11.1（无 2.11.3），Rust/JS patch 天然不同步是 Tauri 发布常态。

### 3.3 E2E / L4 相关
- embedded driver（`@wdio/tauri-service` + `tauri-plugin-wdio-webdriver`）依赖 `webview2-com`，**零 msedgedriver 依赖**。
- `browser.getTitle()` 取 `document.title`（非 OS 窗口标题）。可能因 webview 加载时序返回初始值（如 `localhost`），应用 `waitUntil` 轮询等待。
- `wdio.conf.ts` specs 必须指向实际 spec 文件，`capabilities` 必须配 `browserName:'tauri'`。

### 3.4 版本锁定
- 精确锁版本 = package.json/Cargo.toml 无 `^`/`~` + lockfile 已提交。
- `Cargo.lock` 中的已解析版本（如 `tauri-plugin-wdio-webdriver v1.1.0`）是运行的精确版本；Cargo.toml 中的 `"1"` 是 spec。报告写 1.1.0 是准确的（与 lockfile 一致）。

### 3.5 Tauri 插件权限判断（新增于 Phase 1 审计）

- **不能仅凭"capabilities 中没有 X 权限"就判"缺权限"**。必须先确认 X 权限是否真实存在于该插件的权限清单中。
- 验证路径：
  1. 查 crate 源码中是否有 `permissions/` 目录 → 无目录 = 插件不注册 Tauri 权限
  2. 查 crate 源码中是否有 `#[tauri::command]` 注解 → 无注解 = 插件无 IPC 命令
  3. 查生成的 `acl-manifests.json`（`src-tauri/gen/schemas/`）中是否有该插件的权限条目 → 无条目 = 权限标识符在 Tauri ACL 中不存在
  4. 若 1/2/3 全否 → 添加该权限到 capabilities 会导致 `cargo build` 硬失败（`UnknownPermission` 错误）
- **纯 JS 注入类插件**（通过 `js_init_script()` 工作）**不经过 Tauri 权限系统**，无需在 capabilities 中放行。
- **反面案例（Phase 1）**：初审判 `tauri-plugin-prevent-default` "capabilities 缺 `prevent-default:default` 权限"，但该插件无 permissions 目录、无 IPC 命令、纯 JS 注入。添加此权限反会触发构建错误。

### 3.6 tauri-plugin-prevent-default 已知事实（新增于 Phase 1 审计）

- v5.0.0 Flags 枚举 **10 个常量**：FIND、CARET_BROWSING、DEV_TOOLS、DOWNLOADS、FOCUS_MOVE、RELOAD、SOURCE、OPEN、PRINT、CONTEXT_MENU。**无 COPY、无 PASTE**。
- 插件**完全不干扰 Ctrl+C**——WebView2 `AreBrowserAcceleratorKeysEnabled = false` 明确保留文本编辑键（Ctrl+C/V/X/A/Z）。
- `init()` = `Builder::default().build()` = `Flags::all()`——禁用全部已知浏览器快捷键。
- `Builder::new()` 和 `Builder::default()` 均存在且等价。
- 插件无 `permissions/` 目录、无 IPC 命令、纯 JS 注入绕开 Tauri ACL。

### 3.7 WebView2 快捷键已知事实（新增于 Phase 1 审计）

- **Ctrl+T、Ctrl+N、Ctrl+W** 在 WebView2 中**硬编码始终关闭**（always turned off），不可通过设置开关。来源：Microsoft Edge WebView2 特性差异文档。
- Ctrl+W 虽被禁用，但键盘事件**可穿透到 DOM**，被 xterm.js `attachCustomKeyEventHandler` 接收（可用于 bash readline `\x17`）。
- 判断"哪些快捷键被阻止"时，需区分三个层次：① WebView2 运行时硬编码关闭（Ctrl+T/N/W）；② 插件 Flags 覆盖的浏览器加速键（Ctrl+F/P/R 等）；③ xterm.js 自定义 handler 可处理的终端快捷键。

### 3.8 Dockview renderer 与 PTY 存活（新增于 Phase 1 审计）

- Dockview `AddPanelOptions.renderer` 类型：`'onlyWhenVisible'`（默认，切换标签时卸载组件）/ `'always'`（始终保持挂载）。
- **终端面板必须使用 `renderer: 'always'`**——否则切换标签时 React 组件卸载 → cleanup 中 `pty.kill()` → PTY 进程被杀。
- 审计时若发现 `renderer: 'always'` 已定义但未在 `addPanel()` 调用中传入，应标注为"代码死配置"。

### 3.9 测试质量检查要点（新增于 Phase 1 审计）

- **永真断言**：如 `expect(result.length).toBeGreaterThanOrEqual(0)`——对任何输入（含空字符串）恒为真，不验证任何行为。
- **"写入字面文本" ≠ "验证按键编码"**：`term.write('^C')` 测试的是"终端能解析 `^C` 文本"，而非"Ctrl+C 按键产生 `\x03` 字节序列发送到 PTY"。两者是不同层面的测试。
- L3 键盘测试应验证**实际按键产生的字节序列**（如 `\x1b[Z]`、`\x03`），而非仅验证"write 的文本能在 serialize 中找回"。

### 3.10 CodeMirror 依赖辨析（新增于 Phase 1 审计）

- npm 包 `codemirror` v6 是 CM6 **组合入口**（导出 `basicSetup`），非冗余。
- `@codemirror/basic-setup` ^0.20.0 是 `codemirror` v6 的**旧名包**（v0.20 系列已被 v6 替代），如存在且无引用 → 冗余依赖。
- 审计时判断"依赖是否冗余"：先 Grep 确认该包是否被 `import`，再查其 npm 发布历史确认是否已被替代包覆盖。

### 3.11 Dockview `onReady` 中同步 `addPanel` 竞态（新增于 Phase 1 收口审计）

- **已知 bug**（dockview-core #341 / #866）：React Strict Mode 下 `onReady` 触发两次，同步 `addPanel` 可能操作已释放资源（"resource is already disposed"）。dockview-core v4.2.2 已修复。
- **症状**：终端面板"已创建但无法使用"、关闭面板后应用卡死（"未响应"）。
- **变通方案**（无法升级版本时）：`useState` + `useEffect` 延迟调用 `addPanel`。将 api 存入 state，在下一个渲染 tick 中创建面板。
- **审计检查项**：若发现 `onReady` 回调中直接调用 `api.addPanel()`，标注为潜在竞态风险。

### 3.12 静默失败模式审计（新增于 Phase 1 收口审计）

- **反面案例**：`useCodeMirror.ts` 的 `handleSave` 检查 `if (!path) return;`——所有编辑器入口均不传 `filePath` → 静默返回 → 人工验收表现为"Ctrl+S 没反应"，难以诊断。
- **教训**：审计时对"条件判断后静默返回"的代码路径要专门检查——这类路径在自动化测试中不触发（mock 了写操作），在人工验收中表现为"无反应"而非报错。特征：`if (!x) return;` 无 `console.warn`/`console.error`。
- **审计检查项**：搜索 `if (!.*) return;` 模式，验证每个静默返回路径是否在正常使用场景下可达。

### 3.13 xterm.js 数据流与测试验证（新增于 Phase 1 收口审计）

- **`term.input()` vs `term.write()`**：
  - `term.input(data)` → `CoreService.triggerDataEvent()` → **仅触发 `onData`**，不经 parser/buffer。这是前端→后端的用户输入路径。
  - `term.write(data)` → `WriteBuffer.write()` → `_inputHandler.parse()` → **经 parser→buffer**，不触发 `onData`。这是后端→前端的渲染路径。
- **C0 控制字符消费机制**：`\x03`（ETX）在 VT500 状态转移表中是 `EXECUTABLE`，被 InputHandler 消费但不进 buffer。`serialize()` 从 buffer 单元格反向重建 ANSI——**无法恢复已被 parser 消费的字节**。
- **测试验证策略**：序列化工具（`serializeAddon.serialize()`）验证 buffer 内容（渲染结果）；`onData` 回调验证通过 parser 前的原始字节（按键编码）。两者覆盖不同层面，不可互相替代。
- **来源**：`node_modules/@xterm/xterm/src/common/`（CoreTerminal.ts、CoreService.ts、InputHandler.ts、EscapeSequenceParser.ts）

### 3.14 serde tagged enum + rename_all 与 TypeScript 判别器一致性（新增于 Phase 1 第三轮收口）

- **反面案例**：`PtyEvent` 用 `#[serde(tag = "type", content = "data", rename_all = "camelCase")]`——serde 把变体名 `Output` 序列化为 `"output"`（camelCase），但 TypeScript 类型和 `===` 匹配写的是 `"Output"`（PascalCase）。`===` 比较永远失败→PTY 输出静默丢弃。**从项目第一天存在，通过全部自动化门（cargo test、npm test、ESLint、tsc）——因为无 IPC roundtrip 测试覆盖前端分支逻辑。**
- **审计检查项**：
  1. 对每个 `#[serde(tag = "...", rename_all = "camelCase")]` Rust enum → 逐变体列出 serde 实际输出的 tag 值（`Output`→`"output"`、`Exit`→`"exit"`）
  2. 逐变体交叉核验：TypeScript 类型中的字面量类型（`type: "output"`）和前端 `===` 匹配字面量
  3. **用 DevTools Console 的 `Object.keys()` 核验实际 IPC 消息**（L0 证据）——不可仅凭 TypeScript 类型定义判断，因类型定义本身可能写错
- **通用性**：此 bug 模式不仅限于 PtyEvent——任何 `#[serde(tag = "...", rename_all = "camelCase")]` 的 Rust enum，其变体名的 camelCase 转换可能与 TypeScript 类型的预期不一致。审计时必须逐变体核对，不留盲区。

### 3.15 IPC 通路端到端覆盖检查（新增于 Phase 1 第三轮收口）

- **反面案例**：`terminal.test.tsx` 将 `useXterm` 完全 mock（`vi.mock`）→ 测试仅验证 React 能渲染 div；`pty_integration_tests.rs` 仅测 Rust 侧 spawn/resize → 前端 `handlePtyOutput` 的 `event.type` 分支从未在任何测试中执行。PtyEvent 大小写 mismatch 存留多轮未发现。
- **审计检查项**：
  1. 对每个 IPC 数据通路（invoke → Rust → Channel → 前端 handler → term.write/DOM 写入），检查是否存在端到端 roundtrip 验证
  2. 若无 roundtrip 测试 → 标注为"IPC 通路无 E2E 覆盖"，审计报告中独立列出
  3. 至少对每个 `#[serde(tag = "...")]` 枚举的**每个变体分支**验证一次——确保前端 handler 的所有 `if (event.type === "X")` 分支均可被执行到且正确匹配

### 3.16 npm 依赖 CSS 导入完整性检查（新增于 Phase 1 第三轮收口）

- **反面案例**：`@xterm/xterm/css/xterm.css` 从项目初始未导入 → textarea resize handle 角标可见、viewport 无定位、终端布局破碎。自动化全绿因为 jsdom 不渲染 CSS——textarea 默认 resize handle 是浏览器行为，不在 jsdom 测试中暴露。
- **审计检查项**：
  1. 对每个含 CSS 样式的 npm 依赖（xterm.js、CodeMirror、Dockview、等），Grep 确认 `import '...css'`（或 CSS `@import`）存在于前端源码中
  2. 验证方式：① 读 npm 包 `package.json` 的 `style`/`css` 字段 ② 查 `node_modules/<pkg>/css/` 目录是否存在 ③ Grep 项目 `src/` 是否有对应 import
  3. 缺失 CSS import → 标注为"阻断：npm 包 CSS 未加载"，因 jsdom 测试不可检测此类问题
- **通用性**：此模式适用于所有自带 CSS 的 npm UI 组件（CodeMirror、Dockview、Monaco、等）。生产构建可能通过 Vite 打包掩盖部分问题，但开发模式（`vite dev`）和 jsdom 测试均不可检测未导入的 CSS 文件。

1. **开红旗** → version-pins.md 新增条目，描述风险与条件。
2. **补救/收口** → 修改代码/配置消除风险，或在报告记录"已评估可接受"。
3. **关红旗** → 原条目追加"已收口"或"不再适用"，附证据（如 `tauri info` 无 mismatch、Cargo.lock 锁定版本）。
4. **审计核验** → 对照报告与 version-pins.md，确认每个红旗的收口状态与报告一致。

## 5. 当前项目记忆索引

本项目已在 `C:\Users\liush\.claude\projects\D--data-learn-code-slTerminal\memory\` 积累的记忆：

| 记忆 | 内容 |
|------|------|
| `setup-rust-toolchain-default-deny-warnings.md` | action rustflags 默认 -D warnings；`rustflags:""` 才禁用 |
| `audit-verify-before-flagging.md` | 标"失实"前查决策表 E1–E5 + 取一手证据 |
| `edit-chinese-md-with-builtin-tools.md` | 中文文档用内置 Read+Edit，filesystem mcp 有乱码风险 |

刷新记忆时用 `Read` 查阅上述文件，验证当前代码是否仍与之吻合。若记忆过时或错误，`Write` 更新。

## 6. 审查交付规范

- **审计报告** → `plan/execute/Phase <N> 验收审计.md`。逐条对照 DoD，含独立验证记录、失实/缺漏清单、决策台账、收口路径。
- **grill 节奏** → 一次只问一个问题，附带推荐答案选项。不猜测用户意图，不替用户做价值判断。
- **措辞精准** → "失实"仅用于 L0–L2 证据明确矛盾的情况。不确定时用降级措辞（"时序敏感""弱证据""待确认"）。
- **收口** → commit + push + `gh run watch` 确认 CI 全绿后，方可声明 Phase 完成。

## 7. 审计报告的交叉核验（新增于 Phase 1 审计）

**审计报告产出后，其核验批注本身也可能出错。** Phase 1 审计经历了两轮交叉核验：

- **第一轮**：用户补充核验批注后，4 路 agent 并行检索 + 本地源码审查发现：用户关于 "capabilities 缺权限导致插件完全无效" 的结论不成立（插件纯 JS 注入绕开 ACL）。15 项核验中 2 项需修正。
- **第二轮**：用户修正并再次补充后，agent 检索 + ACL manifest 验证确认本轮变更全部正确。

**教训**：
- 审计报告的 `> 🔍 核验` 批注**也应被核验**——对照 L0–L2 证据（源码、实跑命令、ACL manifest）逐条确认。
- **关于 Tauri 权限的结论必须查 crate 源码**：不能假设每个插件都需要 capabilities 权限。验证标志：`permissions/` 目录 + `#[tauri::command]` 注解 + `acl-manifests.json` 条目。
- **用户补充的核验结论 ≠ 终局真相**——审计者应以同等证据标准交叉核验用户的核验批注。

### 7.1 自动化通过 ≠ 端到端正常（新增于 Phase 1 收口审计）

- **反面案例（Phase 1 收口）**：F1–F4 全部 grep 验证通过 + L2+L3 测试全绿，但人工验收发现 4 项严重问题（关闭面板卡死、终端不可用、Ctrl+S 无反应、页签无法拖拽）。grep 验证的是**代码存在性**（文件存在、字符串匹配），测试验证的是**单元行为**（mock 隔离环境），两者都不等于**端到端用户体验**。
- **教训**：收口审计的自动化验证不能仅靠 grep + 单元测试。必须包含：① 人工验收清单的**实际操作记录**（非仅勾选）② 对"静默失败"路径的代码审查 ③ grep 结果仅在确认代码可被执行到时才构成证据。
- **Dockview `onReady` 同步 `addPanel` 的竞态**：测试中 `jsdom` 环境的 `ResizeObserver` 是 stub，不触发真实 Dockview 生命周期——竞态只在真实 WebView2 中暴露。

### 7.2 Tauri 2 + WebView2 全窗口关闭 freeze 风险（新增于 Phase 1 收口审计）

- 多个上游已知 issue：#14088（全窗口隐藏崩溃）、#11254（事件处理器死锁）、#13498（随机灰页冻结）。
- **触发模式**：关闭所有面板 → 级联 `pty.kill()` → `reader_handle.join()` 同步等待 → WebView2 事件循环死锁。
- **规避**：① 始终保留至少一个面板 ② kill 操作改为异步 fire-and-forget ③ 升级到 dockview-core ≥4.2.2 修复 onReady 竞态。

### 7.3 "随 X 自动恢复"假设必须经代码路径追踪确证（新增于 Phase 1 第二轮收口审计）

- **反面案例**：执行计划声称 G2（终端创建后不可用）"随 G1a 自然恢复"。人工验收证实 G2 未修复——Watermark/RHA 终端仍不可用。G1a 修的是 StrictMode `onReady` 双发，G2 是容器布局时序问题（`addPanel` 后 CSS 布局未提交到 DOM，xterm.js 在 0×0 容器初始化）。**不同根因**。
- **教训**：修复项之间的因果声称必须经代码路径追踪 + 人工验收确证。不同创建路径（onReady / Watermark / RHA / 右键菜单）可能经过不同的 Dockview 内部代码路径。

### 7.4 mock 全部核心逻辑的测试不计入功能覆盖率（新增于 Phase 1 第二轮收口审计）

- **反面案例**：L2 `terminal.test.tsx` 和 `editor.test.tsx` 将 `useXterm` / `useCodeMirror` 完全 mock（`vi.mock`），测试仅验证 React 组件能渲染 `<div>`。全部通过——但真实环境中终端/编辑器完全不可用。
- **教训**：审计时对 `vi.mock` 的 hook 需专门核查——mock 了什么、还剩什么真实覆盖。此类测试不应计入 DoD 通过计数，应标注为"零覆盖测试"。核心 hook 的 mock 应视为技术债务。

### 7.5 插件 import 也应受架构约束（新增于 Phase 1 第二轮收口审计）

- **反面案例**：`useCodeMirror.ts` 直接 `import { save } from "@tauri-apps/plugin-dialog"`——`save()` 是 IPC 调用（打开原生文件对话框），架构上等同于 `invoke`。但 ESLint `no-restricted-imports` 仅限 `@tauri-apps/api/core`，未覆盖插件 import。
- **教训**：ESLint 架构规则需覆盖所有 Tauri 插件的直接 import（`@tauri-apps/plugin-*`）。插件暴露的函数同样是 OS 级 IPC 调用，应经 `src/ipc/` 封装。

### 7.6 死依赖检测（新增于 Phase 1 第二轮收口审计）

- **反面案例**：8 个 `@codemirror/lang-*` 包已安装在 `package.json` 但从未被任何源文件 `import`——编辑器无语法高亮但依赖安装浪费。
- **教训**：审计时应 Grep 每个依赖包名在 `src/` 中的出现次数。建议 CI 中加入 dead dependency 检查（`knip`、`depcheck`）。

### 7.7 Dockview 容器布局时序——addPanel 后 CSS 布局异步提交（新增于 Phase 1 第二轮收口审计）

- **已知机制**（Dockview 源码 + #318/#640/#866）：`addPanel()` 后 Dockview 通过 `rAF` 异步提交 CSS 布局。Watermark 空态下新面板容器首次渲染时尺寸可能为 0×0。
- **症状**：xterm.js 在 0×0 容器中 `open()` → `FitAddon.fit()` 无法计算行列数 → 终端渲染为极小区域或空白。WebView2 中 ResizeObserver 触发时序与标准 Chrome 不同（canopyide #4913）。
- **修复模式**：① `term.open()` 后双 `rAF` 延迟再 `fit()` ② ResizeObserver 回调中加非零尺寸守卫 ③ 面板首次可见时 `term.resize(80, 24)` 设初始尺寸。

### 3.17 Vite/ESBuild 预构建时可损坏第三方依赖的枚举/常量（新增于 Phase 2 DnD 方向 bug 定位）

- **反面案例**：2 页签上下分屏变左右分屏——根因是 Vite/ESBuild 预构建 `dockview-react` 时，将 `dockview-core` 内联到同一 chunk（`dockview-react.js`），错误优化了 `Orientation` 枚举：`Orientation.HORIZONTAL` → `undefined`。`orthogonal(undefined)` 返回 `'HORIZONTAL'`（fallback 分支），导致 BranchNode 被创建为 HORIZONTAL（左右）而非 VERTICAL（上下）。
- **教训**：
  1. **修改 node_modules 调试前，确认实际加载的文件**。Vite 预构建产物在 `.vite/deps/`，非 npm 原始 `dist/` 文件。`package.json` 的 `module`/`main` 字段决定入口，栈回溯中的文件名可通过 sourcemap 映射回原始文件。
  2. **修改 node_modules 后必须清 Vite 缓存**（`rm -rf node_modules/.vite`），否则预构建 chunk 不会被重新生成。
  3. **ESBuild 可能错误 tree-shake 枚举/常量对象**。症状：原本应是字符串字面量的值变为 `undefined`。可通过在关键位置添加对枚举值的直接引用（如 `console.log(Enum.VALUE)`）验证——若引用后问题消失，说明 ESBuild 误优化。
  4. **修复方向**：用 `optimizeDeps.exclude` 排除受影响的依赖，避免 ESBuild 将其打包为单一 chunk，而非修改 node_modules。
- **审计检查项**：当遇到第三方 UI 库行为异常而逻辑代码审查无误时，检查：
  1. `ls node_modules/.vite/deps/` 确认预构建 chunk
  2. `grep -n "关键词" node_modules/.vite/deps/<chunk>.js` 验证关键常量是否被保留
  3. 若常量缺失或为 `undefined` → 在 `vite.config.ts` 中 `optimizeDeps.exclude` 排除该依赖
  4. 清缓存重试：`rm -rf node_modules/.vite && npm run tauri dev`

## 8. 审计决策台账模板（新增于 Phase 1 审计）

每轮 grill 产出的决策应纳入审计报告的独立章节：

```
| # | 议题 | 决策 | 落点 |
|---|------|------|------|
| D1 | <议题> | <一句话决策 + 注意事项> | <落点文件> |
```

编号从 D1 开始，连续递增。若某决策在后继核验中被修正（如 D1 中 "capabilities 权限" 被移除），应在决策下方加核验批注说明修正原因和证据。
