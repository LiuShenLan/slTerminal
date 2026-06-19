# Phase 1 执行计划审查报告

审查日期：2026-06-18 | 审查依据：`plan/` 下所有源文档 + 多 agent 网络并行检索交叉核验
审查对象：`plan/execute/Phase 1 执行计划.md`

---

## 一、发现的错漏（需修正）

### 1.1 portable-pty 版本矛盾（严重）

**问题**：
- 执行计划 E5 + A1 要求「锁定 0.8.x」
- §7.4 明确写「不用 0.9.0：`PSEUDOCONSOLE_INHERIT_CURSOR` 引入 DSR 查询等待」
- 但 Phase 0 的 `Cargo.toml` 已锁 `portable-pty = "0.9.0"`，`version-pins.md` 钉表也已记录 0.9.0

**核查结果**（6 agent 并行网络检索交叉核验）：

✅ **审查意见确认正确，需修正执行计划。**

补充证据：
1. **terax-ai（6.9k star，最成熟的 Tauri 终端项目）使用 portable-pty 0.9.0**——Cargo.toml 声明 `portable-pty = "0.9"`，Cargo.lock 解析为 0.9.0（[源码](https://raw.githubusercontent.com/crynta/terax-ai/main/src-tauri/Cargo.toml)）
2. `PSEUDOCONSOLE_INHERIT_CURSOR` 标志**仅在 0.9.0 引入**（wezterm commits `9d83458632` + `454ec0a7df`），0.8.x 完全没有
3. **重要修正（经 2 轮 agent 交叉核验后定论）**：xterm.js **确实**会自动响应 DSR 查询 `\x1b[6n` → `\x1b[1;1R`（cluesmith/codev#228、kwannoel/the-controller#57 独立验证）。**但** DSR 的发送时机在 `CreatePseudoConsole()` / `openpty()` **内部**——`PSEUDOCONSOLE_INHERIT_CURSOR` 标志导致 ConPTY 在初始化阶段就发出 DSR 并阻塞等待 CPR 响应。此时：
   - Reader 线程尚未创建（在 `openpty()` 返回后才 `spawn`）
   - Channel 尚未建立
   - 前端 xterm.js 尚未 `term.open()`
   - `\x1b[6n` 字节堆积在 ConPTY 输出管道缓冲区，**从未到达 xterm.js**
   
   因此 xterm.js 的 VT 解析能力在此时无效——不是能力问题，是**时序竞赛**问题。turborepo PR #11816 的 bug 报告证实了此 hang 场景（CPU 100% spin）。**CPR 手动写（`\x1b[1;1R`）仍必需**，理由更正为"xterm.js 自动响应 CPR 的完整回路（PTY 输出管道→前端 Event→PTY 输入管道）在启动瞬间不可用，ConPTY 的 `VtInputThread` 在 `VtIo::StartIfNeeded()` 阶段 3 秒内收不到 stdin 响应即超时"。

	> 注意：DSR 不在 `openpty()` / `CreatePseudoConsole()` **内部**发送（与初审描述不同），而在 `spawn()` 后子进程连接 conhost 时的 ConPTY 内部 `VtIo::StartIfNeeded()` 阶段（[DeepWiki](https://deepwiki.com/microsoft/terminal/2.4-conpty-and-console-host-integration) 源码级确认）。即使 `openpty()` 之后 reader 线程已创建，DSR 也是在 `spawn()` 之后才发出，此时 CPR 完整回路（PTY 输出→xterm.js→前端 IPC→PTY 输入）尚不可用。Turborepo PR #11816 的 CPR 写入紧接 `openpty()` 后——此时 DSR 尚未发送，CPR 已提前排队在 stdin 管道中，后续 spawn→attach→DSR→阻塞→读到排队 CPR→解锁，时序正确。
4. `#[cfg(not(windows))]` 保护 stdin drop 是 0.9.0 需要的，但这是通用 Windows 安全措施，与版本无关

**修正结论**：执行计划应**与 Phase 0 保持一致，使用 0.9.0**。删除 §7.4 中"锁定 0.8.x"和"不用 0.9.0"的说法。A1 中**必须保留** CPR 手动写（`writer.write_all(b"\x1b[1;1R")`）和 stdin drop `#[cfg(not(windows))]` 保护。CPR 写入时机：`openpty()` 之后、reader 线程启动之前，加 `#[cfg(windows)]` 条件编译。

---

### 1.2 IPC 输出通道：emit vs Channel（中等，与架构文档矛盾）

**问题**：
- 执行计划 E3 决定「PTY IPC 输出用 Event（`window.emit`），payload 纯文本；不用 Channel（绑单次 invoke）」
- 架构文档 §三、§五 明确要求「优先用 Tauri `Channel<T>`（有序、定向单会话、开销小）做高频流（PTY 输出、hooks 状态）」
- 详细方案 §四 的 IPC 契约也指定 `Channel<PtyEvent>`

**核查结果**（Tauri 2 官方文档 + terax-ai 源码 + godly-terminal 架构多重交叉核验）：

✅ **审查意见确认正确，E3 必须改为 Channel。**

补充证据：
1. **Channel 可通过 spawn 模式长期持有**：命令函数立即返回 `Ok(())`，Channel 被 `move` 进 `tauri::async_runtime::spawn` 的后台任务中，持续有效直到任务结束或前端断开（Tauri 2 官方 calling-frontend 文档）
2. **Tauri 官方明确不推荐 Event 用于高吞吐**：原文 "The event system is **not designed for low latency or high throughput situations**. See the channels section for the implementation optimized for streaming data"（[v2.tauri.app](https://v2.tauri.app/develop/calling-frontend/)）
3. **Event payload 强制 JSON 序列化**：`Vec<u8>` 通过 `emit` 会序列化为 JSON 数字数组 `[65,66,67,...]`，膨胀约 4 倍。Tauri Issue [#5641](https://github.com/tauri-apps/tauri/issues/5641) 实测同链路 500MB 数据耗时 30s vs 裸 JSON 5s（6x 减速）
4. **terax-ai 使用 Channel 做 PTY 流**：Tauri 2 社区最成熟的终端项目，`invoke("pty_spawn", { on_data: channel })` → Channel 流式推送输出
5. **fellanH/origin #153 的 Channel→Event 迁移是 iframe 隔离架构驱动的特殊需求**：L0（进程内插件，直接 Channel）→ L1（sandboxed iframe，Channel 无法跨边界 → 拆为 Event postMessage）。Host 侧 PTY Bridge 内部仍用 Channel。不是 Channel 性能有问题
6. **架构文档 `plan/3. 技术架构设计文档.md` 第 68 行原文**："优先用 Tauri `Channel<T>`（有序、定向单会话、开销小）做高频流（PTY 输出、hooks 状态）；低频用全局 `emit`"

**修正结论**：PTY 输出必须用 `Channel<PtyEvent>`（spawn 模式长期持有），与架构文档一致。emit 仅用于退出通知、CWD 变更等低频事件。

---

### 1.3 纯 DOM 渲染延后 WebGL 与 GPU 加速基线矛盾（中等）

**核查结果**（xterm.js 6.0 changelog + 需求文档 + caniuse 三重核验）：

✅ **审查意见确认正确。E2 偏离需求 GPU 加速基线，应在 Phase 1 即加 WebGL。**

补充证据链：
1. **需求基线**：`plan/1. 需求与分阶段计划.md` 第 13 行定「渲染层 **GPU 加速**（作为终端/编辑器渲染的**基线属性**）」；`plan/3. 技术架构设计文档.md` 技术栈表写死「xterm.js + **addon-webgl** + addon-fit」
2. **xterm.js 6.0 已移除 Canvas 渲染器**（PR [#5105](https://github.com/xtermjs/xterm.js/pull/5105)），只剩 DOM + WebGL。下游 Cockpit、Daintree、RStudio 均确认此 breaking change
3. **WebGL 可用率 ~95.64%**（caniuse.com/webgl2），WebView2 单窗口单实例下 16 context 限制不构成威胁（Daintree 结论：仅焦点终端 1 个 WebGL context 即可）
4. **实现量约 20 行**：`detectWebgl()` 预检（`failIfMajorPerformanceCaveat: true`）+ `loadAddon(new WebglAddon())` + `onContextLoss` 回退 DOM。相关代码参考 Proxmox VE 实现（[pve-xtermjs.git commit aa9a479](https://git.proxmox.com/?p=pve-xtermjs.git;a=commit;h=aa9a479c51658fe7c89d34fa0ba2836c05db3465)）
5. **Daintree 两次全 WebGL 失败、最终收敛到约 50 行"聚焦独占"策略**——证明简单方案可行，复杂池化不可行

**修正结论**：Phase 1 加载 WebGL addon（带 `failIfMajorPerformanceCaveat` 预检 + DOM 兜底回退），仅焦点终端持有 1 个 context，失焦 dispose。约 20 行代码，不改变面板架构。如坚持先 DOM，E2 必须标注"偏离需求基线"。

---

### 1.4 合批渲染方案未落地到 agent 操作（中低）

**核查结果**（xterm.js 6.0 DEC 2026 + turborepo 四层合批 交叉核验）：

✅ **审查意见确认正确。A6 缺少合批策略。**

补充证据：
1. **xterm.js 6.0 DEC Mode 2026**（`\x1b[?2026h` ... data ... `\x1b[?2026l`）是原生同步更新模式——包裹的输出原子提交，零撕裂。比手动 rAF 批处理更低层。在 PTY→前端推送侧包裹即可，不依赖 shell
2. **turborepo 四层累积合批**：PTY 层（stdout+stderr 合并）→ 字节归一化（LF→CRLF）→ vt100 整批解析 → 行缓冲+原子 flush。PR #7760、#11804、#12343 经验直接可用
3. **可选 rAF 批处理**：小数据（<256 bytes）立即写 + 大数据 rAF 合并。适用于 DEC 2026 不可用时的回退

**修正结论**：A6 操作第④步补入合批策略：DEC 2026 包裹优先 + rAF 批处理回退。

---

### 1.5 `plan-generation.md` 引用飘空（低）

**核查结果**：

⚠️ **审查意见部分正确。文件存在但路径不同。**

`plan-generation.md` 实际位于 `.claude/rules/plan-generation.md`（Claude Code 渐进式披露规则文件），不在 `plan/` 下。执行计划 §1.2 写"按 `plan-generation.md` 决策树"无误——该文件配置了 `paths: "plan/**/*.md"`，读取 plan/ 下任何文件时自动加载到上下文。此处措辞澄清即可，不需修改。

---

### 1.6 测试文件位置未指定（中低）

**核查结果**：

✅ **审查意见确认正确。Phase 0 已有明确测试布局，Phase 1 新测试应继承。**

Phase 0 已建立的测试布局需在 agent 任务卡中引用：
- L1 集成测试：`src-tauri/tests/`（如 `pty_roundtrip.rs`、`fs_roundtrip.rs`）
- L2 前端单测：`src/__tests__/`（如 `terminal.test.tsx`、`editor.test.tsx`）
- L3 终端渲染：`test/terminal/`（如 `render.test.ts`、`keyboard.test.ts`）
- L4 E2E：`e2e-tests/`（如 `phase1.e2e.ts`）

---

## 二、模糊与待澄清（需决策）

### 2.1 OSC 序列方案与源文档不一致（需澄清）

**核查结果**（ConEmu 规范 + Windows Terminal 源码 + freedesktop terminal-wg 多重核验）：

⚠️ **审查意见确认正确。OSC 9;9 起源需纠正为 ConEmu，选型方向合理。**

补充证据（agent 交叉核验确认）：
1. **OSC 9;9 起源是 ConEmu**，不是 Windows Terminal。Windows Terminal v1.6 通过 PR [#8330](https://github.com/microsoft/terminal/commit/e557a867ee1d981b81658909b0bed3315cda8d8f) 采纳（commit message："Implement ConEmu's OSC 9;9 to set the CWD"）
2. **OSC 7 是事实标准非 de jure 标准**。freedesktop terminal-wg MR #7 试图标准化但至今未合并（[gitlab](https://gitlab.freedesktop.org/terminal-wg/specifications/-/merge_requests/7)）
3. **xterm.js 可同时注册处理两者**：`parser.registerOscHandler(7, cb)` + `parser.registerOscHandler(9, cb)`，互不冲突
4. **VS Code shellIntegration.ps1 实际用 OSC 633（私标）**，非 OSC 133。633 的 `E` 序列可显式上报命令行文本（支撑"Run Recent Command"），这是 OSC 133 没有的核心差异。VS Code 的 ShellIntegrationAddon 同时注册 633 和 133 的处理器做兼容
5. **OSC 1337 SetUserVar** 仅 iTerm2 + WezTerm 支持，非通用标准。值必须 Base64 编码。GNOME Terminal/Konsole/Alacritty/Kitty 均不支持。对 slTerminal 价值有限

**修正结论**：执行计划应**同时支持 OSC 7 + OSC 9;9**（xterm.js 双注册），源文档应解释为何新增 OSC 9;9。OSC 1337 SetUserVar 暂不引入。

---

### 2.2 PowerShell 编码问题（新增）

**核查结果**（VS Code shellIntegration.ps1 + Microsoft PowerShell 文档核验）：

✅ **审查意见确认正确。UTF-8 编码修复是行业标准做法，必须加入 A3。**

补充证据：
1. `-NoProfile` 阻止 profile 中一切自定义生效，`$OutputEncoding` 保持出厂默认。PS 5.1 出厂默认是 **US-ASCII**（CodePage 20127），中文全部变 `?`。PS 7 出厂默认是 UTF-8
2. `[Console]::OutputEncoding` 在中文 Windows 上默认为 **CP936 (GBK)**（Win32 API `GetConsoleOutputCP()` 返回），导致 PTY 输出中非 ASCII 字符乱码
3. **行业标准注入代码**（VS Code、Windows Terminal、ConPTY 宿主均如此）：
   ```powershell
   [Console]::OutputEncoding = [System.Text.Encoding]::UTF8
   [Console]::InputEncoding  = [System.Text.Encoding]::UTF8
   $OutputEncoding = [System.Text.Encoding]::UTF8
   chcp 65001 >$null
   ```
4. PS 5.1 兼容注意：Escape 字符必须用 `$([char]0x1b)` 而非 `` `e ``（后者 PS 5.1 不支持）

**修正结论**：A3 的 `shell-integration.ps1` 开头必须加入编码修复，作为节点①之前的预先操作。

---

### 2.3 `with_browser_accelerator_keys` 的具体配置路径（需澄清）

**核查结果**（Tauri 2 plugin 生态 + Wry 源码核验）：

✅ **审查意见确认正确。A9 必须明确使用 `tauri-plugin-prevent-default`。**

补充证据：
1. **Tauri 2 中无法直接在 Rust builder 调 Wry API**——Tauri 2 的 `webview_created` hook 接收的是已创建好的 `Webview<R>`，不是 `WebViewBuilder`。`tauri.conf.json` 中无对应配置项
2. **唯一推荐方案**：`tauri-plugin-prevent-default` v5.0.0（crates.io 实时查询最新版），封装了 Wry `with_browser_accelerator_keys(false)`：
   ```rust
   // Cargo.toml: tauri-plugin-prevent-default = "5"
   use tauri_plugin_prevent_default::Flags;

   let prevent = tauri_plugin_prevent_default::Builder::default()
       .with_flags(Flags::all().difference(Flags::FIND | Flags::DEV_TOOLS))
       .build();

   tauri::Builder::default()
       .plugin(prevent)
       .run(tauri::generate_context!())?;
   ```
   - 注：`Ctrl+C` 不受影响（SIGINT 层面，非浏览器加速键范畴）
3. 可精细节制：`Flags::all().difference(Flags::FIND | Flags::RELOAD)` 保留部分快捷键（FIND/CARET_BROWSING/DEV_TOOLS/DOWNLOADS/FOCUS_MOVE/RELOAD/SOURCE/OPEN/PRINT/CONTEXT_MENU/ZOOM 等 12 种 flag 可自由组合）
4. 需在 `capabilities/default.json` 加权限：`"prevent-default:default"`（自动含 `allow-keyboard` + `allow-pointer`）

**修正结论**：A9 操作第①步明确写"使用 `tauri-plugin-prevent-default` crate v5.0.0"，附 `Builder::default().with_flags(...).build()` 示例代码。版本号以 crates.io 实时查询为准（当前 v5，后续如有大版本更新跟进）。

---

### 2.4 `renderer: 'always'` vs Dockview 隐藏时 PTY 保活（需澄清）

**核查结果**（Dockview 6.6 官方文档核验）：

❌ **审查意见不正确。Dockview `addPanel` 确实有 `renderer` 参数。**

Dockview 6.6 `AddPanelOptions` 类型定义包含 `renderer?: DockviewPanelRenderer`，可选值：`'onlyWhenVisible'`（默认，隐藏时移除 DOM）和 `'always'`（隐藏时 CSS `display:none`，不销毁 DOM）。执行计划 A8 写 `renderer: 'always'` 完全正确，语义清晰——面板切换到后台时 xterm 实例保活、PTY 连接不断开。

审查文档此条应撤回。

---

## 三、可优化（建议改进，非阻塞）

### 3.1 IME 防御代码可精简（xterm.js 侧 2 个 issue 已修复）

**核查结果**（第 3 轮 agent 直接查询 GitHub API，2026-06-18 最新状态）：

⚠️ **审查意见部分正确。xterm.js #5023 已关闭，但 #5734 修复仅在未发布的 v7.0.0 中。**

更新后证据：
1. **xterm.js #5023（中文 IME 重复输入）→ Closed**：PR [#5024](https://github.com/xtermjs/xterm.js/pull/5024) 已合并（2024-04-08，milestone 6.0.0）。**当前 npm 6.0.0 已包含此修复** ✅
2. **xterm.js #5734（候选窗定位到占位文本）→ Closed**：PR [#5759](https://github.com/xtermjs/xterm.js/pull/5759) 已合并（2026-03-11，milestone 7.0.0）。**但 npm latest 仍是 6.0.0，v7.0.0 尚未正式发布**——当前安装的 xterm.js 6.0.0 **不含此修复** ⚠️
3. **WebView2Feedback #5475 → 仍 Open**：IME 组合期失焦 → `STATUS_BREAKPOINT` 硬崩溃（Chromium 渲染引擎层缺陷）。唯一 workaround：`mousedown + preventDefault()`。**无微软官方回复**
4. **XtermBlazor #58 → Closed**：不影响纯 JS 场景
5. **`windowsPty` 选项**（xterm.js v5.2+）：建议 A6 启用 `{ windowsPty: { backend: 'conpty', buildNumber: 21376 } }`。行为：控制 ConPTY reflow/scrollback/换行推断

**修正结论**：由于 #5734 修复未在 npm 正式版中，执行计划中仍应包含 #5734 的已知限制说明。防御措施：WebView2 #5475 `mousedown + preventDefault()` + `isComposing` 标志位 + blur 清理。A6 启用 `windowsPty`。

### 3.2 缺少 WebView2 快捷键禁用的 Rust 侧配置 → 已合并至 §2.3

见 §2.3 修正结论：使用 `tauri-plugin-prevent-default` v2.0.0。

### 3.3 xterm.js 6.0 DEC 2026 → 已合并至 §1.4

见 §1.4 修正结论：A6 补入 DEC 2026 + rAF 批处理回退。

### 3.4 缺少 WebGL 预检逻辑 → 已合并至 §1.3

见 §1.3 修正结论：Phase 1 加载 WebGL，需 `detectWebgl()` 预检。

### 3.5 CI 和 clippy 验收

✅ **审查意见确认正确。** A1–A4 agent 任务卡应加入 `cargo clippy -- -D warnings` 验收项。

### 3.6 Phase 0 回归验证

✅ **审查意见确认正确。** W2 验证应显式包含 Phase 0 已有测试回归。

### 3.7 Dockview #341/#1003 已修复

✅ **审查意见确认正确。** Dockview 6.6.1 不适用这两个已修复 bug，version-pins.md 红旗 #6 可关闭。

### 3.8 React 19 StrictMode guard

**核查结果**（xterm.js #4983/#5181 + 社区实践核验）：

✅ **审查意见确认正确。`useRef` guard clause 是社区标准防御模式。**

A6 的 `useXterm.ts` 必须在 useEffect 中加 `if (terminalRef.current) return` 防御二次挂载。cleanup 中同时 `dispose()` + 置 null。

### 3.9 CM6 @codemirror/merge 预留

✅ **审查意见正确。** A7 可预留 CM6 extensions 扩展点，Phase 3 复用 `@codemirror/merge` 包。

---

## 四、与源文档的全量覆盖对比（核查后定论）

| 源文档要求 (5.1) | 执行计划覆盖 | 核查后 | 核查证据 |
|---|---|---|---|
| 1.1 PowerShell 终端面板 | A1+A2+A3+A6 | ✅ | — |
| PTY spawn/write/resize/kill | A1+A2 | ✅ | — |
| Channel\<PtyEvent\> 推送输出 | A2（但用 emit 非 Channel） | ❌ E3 须改为 Channel | §1.2：Tauri 官方 + terax-ai |
| PowerShell profile 注入 | A3 | ✅（OSC 9;9 需补充说明） | §2.1 |
| SPAWN_LOCK 串行化 | A1 | ✅ | — |
| cwd 反斜杠规范化 | A1 | ✅ | — |
| 合批渲染 | **缺失** | ❌ A6 须补入 DEC 2026 | §1.4 |
| xterm 随面板存活/切走隐藏 | A8 | ✅ | §2.4：renderer always 有效 |
| UTF-8 编码修复 | **缺失** | ❌ A3 须补入编码修复 | §2.2 |
| 1.2 打开并编辑文件 | A4+A7 | ✅ | — |
| 1.3 多页签管理 | A8 | ✅ | — |
| 1.4 拖拽分屏+嵌套+调比例 | A8 | ✅ | — |
| 1.5 跑 claude | A9 + 人工 | ✅ | — |
| 中文 IME | A9 | ⚠️ 防御不完整 | §3.1 |
| StrictMode 双重挂载防御 | **缺失** | ❌ A6 须补入 useRef guard | §3.8 |

---

## 五、决策台账争议项（核查后定论）

| # | 执行计划决策 | 核查后定论 | 核查依据 |
|---|-----------|---------|---------|
| E1 | /goal 主干 + W0/W1/W2 workflow | ✅ 合理，PTY 后端是最难点 | — |
| E2 | Phase 1 纯 DOM，WebGL 延后 | ❌ 偏离需求 GPU 加速基线 | §1.3：需求文档基线 + xterm.js 6.0 仅 DOM+WebGL + ~20 行可实现 |
| E3 | PTY 输出用 emit 不用 Channel | ❌ 与架构文档矛盾，Channel spawn 模式可长期持有 | §1.2：Tauri 官方文档 + terax-ai 验证 + Event Vec<u8> JSON 膨胀 4x |
| E4 | PTY 输入用 invoke | ✅ 合理 | — |
| E5 | portable-pty 锁定 0.8.x | ❌ 与 Phase 0 的 0.9.0 矛盾，terax-ai 也用 0.9.0 | §1.1：terax-ai Cargo.toml 验证 + CPR 手动写补偿 xterm.js→PTY 回路启动时序 |
| E6 | PowerShell profile CLI 参数注入 | ✅ 合理 | — |
| E7 | 集成脚本存储（debug 读文件/release include_str!） | ✅ 合理 | — |
| E8 | OSC 9;9 优先 | ⚠️ 技术可行，需同时支持 OSC 7 + 解释为何新增 OSC 9;9 | §2.1：OSC 9;9 起源 ConEmu（非 WT 默认），xterm.js 可双注册 |
| E9 | OSC 133 A/B/C/D 命令边界 | ✅ 合理 | — |
| E10 | 面板创建时序（前端先生成 panelId） | ✅ 合理 | — |
| E11 | 双层 ID（panelId + sessionId） | ✅ 合理 | — |
| E12 | 终端 Session 用模块级 Map 不用 Zustand | ✅ 合理 | — |
| E13 | 3 个 JS handler 键盘策略 | ⚠️ IME 防御不完整 | §3.1：需加 5s 超时 + blur 清理 + mousedown preventDefault |
| E14 | with_browser_accelerator_keys(false) | ⚠️ 方向正确，需明确用 `tauri-plugin-prevent-default` | §2.3：Tauri 2 无法直接调 Wry API |
| E15 | 纯鼠标面板管理 | ✅ 合理 | — |
| E16 | Ctrl+Shift+C/V 复制粘贴 | ✅ 合理 | — |
| E17 | Phase 1 不做全局热键 | ✅ 合理 | —

---

## 六、审查总结（核查后定论）

**总体评价**：Phase 1 执行计划结构完整、编排逻辑清晰。经 6 个 agent 大规模并行网络检索交叉核验后：

| 审查意见 | 核查结果 |
|----------|---------|
| §1.1 portable-pty 版本矛盾 | ✅ 确认改 0.9.0，但 CPR 仍需手动写——DSR 在 `VtIo::StartIfNeeded()`（spawn 后）发出，xterm.js→前端→PTY 回路未闭合 |
| §1.2 emit vs Channel | ✅ 确认，改 Channel（Tauri 官方 + 架构文档一致） |
| §1.3 DOM vs WebGL | ✅ 确认，Phase 1 加 WebGL（需求基线） |
| §1.4 合批渲染缺失 | ✅ 确认，A6 补入 DEC 2026 |
| §1.5 plan-generation.md 引用 | ⚠️ 文件存在（`.claude/rules/`），不需修改 |
| §1.6 测试文件位置 | ✅ 确认，agent 任务卡引用 Phase 0 布局 |
| §2.1 OSC 协议 | ✅ 确认：OSC 9;9 起源 ConEmu（非 WT），OSC 7 为事实标准（MR #7 未合并），xterm.js 可双注册；OSC 1337 SetUserVar 暂不引入 |
| §2.2 PowerShell 编码 | ✅ 确认，A3 补入 UTF-8 编码修复 |
| §2.3 browser_accelerator_keys | ✅ 确认，明确用 `tauri-plugin-prevent-default` |
| §2.4 renderer always | ❌ **撤回**：Dockview 6.6 确实有 `renderer` 参数 |
| §3.1 IME 防御 | ⚠️ 部分正确：#5023 已关闭（6.0.0 已含）；#5734 虽关闭但修复仅在未发布 v7.0.0 中（6.0.0 不含）；WebView2 #5475 需防御 |
| §3.5 clippy 验收 | ✅ 确认，A1–A4 加入 clippy 验收项 |
| §3.6 Phase 0 回归 | ✅ 确认，W2 显式包含 Phase 0 测试 |
| §3.7 Dockview 红旗 | ✅ 确认，version-pins.md 红旗 #6 可关 |
| §3.8 StrictMode guard | ✅ 确认，A6 补入 useRef guard clause |
| §3.9 CM6 merge 预留 | ✅ 确认，A7 预留 extensions 扩展点 |

**最终：14/16 条确认正确或撤回，§1.1 方向正确（0.9.0 + CPR 手动写）但时序描述需修正——DSR 在 `VtIo::StartIfNeeded()`（spawn 后）发出不在 `openpty()` 内部，CPR 需手动写的理由从"时序竞赛"更正为"xterm.js→前端→PTY 的 CPR 响应回路在启动瞬间未闭合"；§3.1 xterm.js #5734 虽关闭但修复仅存在未发布 v7.0.0 中（npm 6.0.0 不含）；§2.3 `tauri-plugin-prevent-default` v5.0.0 经 crates.io 确认存在。** 执行计划需修正的 3 个核心项：
1. **E3 → Channel**（与架构文档一致）
2. **E5 → 0.9.0**（与 Phase 0/terax-ai 一致），**A1 保留 CPR 手动写**（理由更正为 openpty() 时序竞赛）
3. **E2 → Phase 1 加载 WebGL**（需求基线 + 仅 ~20 行）

**审查报告自身修正**：§2.3 `tauri-plugin-prevent-default` 版本号 v2.0.0 → v5.0.0（crates.io 实时查询）。
