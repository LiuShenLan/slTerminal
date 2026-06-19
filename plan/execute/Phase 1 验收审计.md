# Phase 1 验收审计

> 审计对象：`plan/execute/Phase 1 结果验证.md`。对照标准：`plan/5.1 phase-1-核心外壳.md` §7/§8、`plan/execute/Phase 1 执行计划.md` 决策表 E1–E16 + Agent 任务卡 A1–A9。
> 审计日期：2026-06-19。本轮实跑 L1–L4 + build + lint，全仓交叉核验 + 4 路并行网络检索。仅审计，未改代码、未改原报告。
>
> **🔍 核验批注日期：2026-06-19。** 以下标注 `🔍 核验` 的段落为审计后交叉核验结果——包含代码证据、网络检索佐证及对原审计结论的确认/修正。

## 0. 结论

**Phase 1 自动化验证核心通过（L1–L4 实跑全绿），但存在 4 项执行计划偏差、1 项弱测试、1 项代码死配置。** 最严重的是 Agent A9（键盘 handler）完全未实现和 `terminalTabConfig`（PTY 存活保障）定义但未使用。报告无失实。

> 🔍 核验（交叉核验修正）：用户补充的核验批注**大部分正确**（COPY flag 不存在、插件不干扰 Ctrl+C、`init()`=`Flags::all()`、Ctrl+W/T/N 由 WebView2 硬编码关闭、`codemirror` 确实被使用、`terminalTabConfig` 严重程度降级合理）。唯有一处需纠正：**`capabilities` 缺少 `prevent-default:default` 不影响插件功能**——该插件无 IPC 命令，纯 JS 注入，绕开 Tauri 权限系统。另发现真正的冗余依赖是 `@codemirror/basic-setup` ^0.20.0（`codemirror` v6 的旧名包）。

## 1. 本轮独立验证记录

| 项 | 命令 | 结果 | 判定 |
|----|------|------|------|
| L1 后端单测 | `cargo test`（src-tauri/） | 9 passed，零 warning | ✅ |
| L2+L3 前端单测 | `npm test` + `npm run test:l3` | 6 files，10 tests | ✅ |
| L4 E2E | `npm run wdio` | 1 spec PASSED，WebGL canvas 模式 | ✅ |
| Debug 构建 | `npm run tauri build -- --debug --no-bundle` | exe 产出 | ✅ |
| Clippy | `cargo clippy -- -D warnings` | Finished，零诊断 | ✅ |
| ESLint | `npx eslint src/` | 退出 0 | ✅ |
| invoke 隔离 | Grep `@tauri-apps/api/core` | 仅 `src/ipc/` + 测试文件 | ✅ |
| 目录骨架 | filesystem MCP 对照报告 | 完全匹配 | ✅ |
| Cargo.lock 版本 | portable-pty 0.9.0 / tauri 2.11.3 / tauri-plugin-prevent-default 5.0.0 | 全部锁定 | ✅ |

> L4 本轮实跑：getTitle → `slTerminal`，Dockview API 就绪，addPanel → PTY spawn → `__e2e_sessionReady` → echo 写入 → `.xterm-screen` 存在。`e2e_marker` 在 DOM 不可见（WebGL canvas 渲染），流程验证通过。

> 🔍 核验：独立验证记录全部准确，L1–L4 实跑结果与报告一致。

## 2. DoD 逐条对照（执行计划 §5.4 + 5.1 §8）

| DoD 项 | 判定 | 证据 |
|--------|------|------|
| 1.1 PowerShell 终端面板 | ✅⚠️ | 后端 PTY 全链路 + 前端 TerminalPanel + useXterm WebGL。但 Ctrl+C 分流/粘贴等键盘处理缺失（见 §3.1） |
| 1.2 打开并编辑文件 | ✅ | fs_read_file/write_file + EditorPanel/useCodeMirror |
| 1.3 多页签管理 | ✅ | DockviewReact + panelRegistry + watermark |
| 1.4 拖拽分屏/嵌套/调比例 | ⚠️ | Dockview 内置行为，无需代码。人工验证未确认，报告标注"待验证" |
| 1.5 跑 claude | ❌⚠️ | 通过终端面板可输入 `claude`，但键盘 handler 缺失导致 Ctrl+C/Shift+Tab 等关键操作不可靠 |
| L1–L3 全绿 | ✅ | 本轮实跑 |
| 关键 L4 通过 | ✅ | 本轮实跑 |
| 不破坏 Phase 0 回归集 | ✅ | Phase 0 测试全部保留且通过 |

## 3. 执行计划偏差（对照决策表 E1–E16 + Agent A1–A9）

### 3.1 A9 键盘 handler 未实现（严重）

**执行计划要求**（Agent A9）：
- Rust 侧：`tauri-plugin-prevent-default` + `Builder::default().with_flags(Flags::all().difference(Flags::FIND | Flags::DEV_TOOLS)).build()` + capabilities 加 `"prevent-default:default"`
- JS 侧：Ctrl+C 智能分流（终端有选区→复制，无选区→发 `\x03`）/ Ctrl+Shift+C/V 复制粘贴 / IME 防御（`isComposing` + 5s 超时 + `mousedown preventDefault`）
- 产出：`src/panels/terminal/keyboard.ts`

**实际状态**：

| 项 | 执行计划 | 实际 | 判定 |
|----|---------|------|------|
| `keyboard.ts` | 必须创建 | **不存在** | ❌ |
| Ctrl+C 智能分流 | 必须实现 | **未实现** | ❌ |
| Ctrl+Shift+C/V | 必须实现 | **未实现** | ❌ |
| IME 防御 | 必须实现 | **未实现** | ❌ |
| tauri-plugin-prevent-default 配置 | `Builder::default().with_flags(...)` | 默认 `init()`（无 `with_flags`） | ⚠️ |
| capabilities `prevent-default:default` | 必须添加 | **N/A**——该插件纯 JS 注入，无 IPC 命令，不依赖 Tauri 权限系统。`cargo build` 已验证添加此权限反会报错"Permission prevent-default:default not found" | ✅ N/A |

**证据**：
- `src/panels/terminal/keyboard.ts` 文件不存在（Glob 验证）
- `lib.rs:29`: `.plugin(tauri_plugin_prevent_default::init())` — 无 `Builder` 配置
- `capabilities/default.json`: 无 `"prevent-default:default"` 权限——**但此为正确状态**：插件纯 JS 注入，不依赖 Tauri 权限系统
- 报告 §二.4 承认"JS 侧键盘 handler 尚未实现，留待 Phase 2"

**影响**：1.5（跑 claude）无法可靠验收。WebView2 浏览器加速键（Ctrl+W 关标签/Ctrl+T 新标签等）未被阻止，终端体验不可用。Ctrl+C 中断行为由 xterm.js 默认 + WebView2 浏览器行为决定，不可控。

~~**网络检索佐证**：`tauri-plugin-prevent-default` Flags 包含 `COPY`（Ctrl+C），若默认 `init()` 使用 `Flags::all()`，则 Ctrl+C 的浏览器复制行为被阻止——但 xterm.js 层面的 Ctrl+C→`\x03` 应不受影响（xterm.js 在更底层处理）。实际行为取决于二者的交互，未经测试验证。~~

> 🔍 核验（网络检索纠正 + **本核验二次纠正**）：上述划线段落**有误**。4 路 agent + 本地源码审查确认：
>
> **1. `COPY` flag 不存在于 v5.0.0** ✅
> - v5.0.0 Flags 枚举含：`FIND`、`CARET_BROWSING`、`DEV_TOOLS`、`DOWNLOADS`、`FOCUS_MOVE`、`RELOAD`、`SOURCE`、`OPEN`、`PRINT`、`CONTEXT_MENU`（共 10 个）
> - **不存在 `COPY` / `PASTE` flag**
> - 来源：`tauri-plugin-prevent-default-5.0.0/src/lib.rs:27-51`（本地 crate 源码）
>
> **2. 该插件完全不干扰 Ctrl+C** ✅
> - Microsoft WebView2 官方文档：`AreBrowserAcceleratorKeysEnabled = false` 明确保留文本编辑键（Ctrl+C/V/X/A/Z、Home、End、PageUp、PageDown）
> - 来源：https://learn.microsoft.com/en-us/dotnet/api/microsoft.web.webview2.core.corewebview2settings.arebrowseracceleratorkeysenabled
>
> **3. `init()` = `Builder::new().with_flags(Flags::all()).build()`** ✅
> - 源码 `lib.rs:359-362`：`pub fn init() { Builder::default().build() }`
> - `Flags::default()` 返回 `Flags::all()`（`lib.rs:74-78`）
> - `Builder::new()` 和 `Builder::default()` **两者均存在且等价**（`new()` 调 `default()`，`lib.rs:90-107`）
> - 执行计划中 `with_flags(Flags::all().difference(Flags::FIND | Flags::DEV_TOOLS))` 的目的是**保留** Ctrl+F 和 DevTools 供开发使用——缺失此配置不影响安全性，只影响开发便利性
>
> **4. Ctrl+W / Ctrl+T / Ctrl+N 由 WebView2 硬编码关闭** ✅
> - Microsoft 官方文档：Ctrl+T、Ctrl+N、Ctrl+W 在 WebView2 中"始终关闭（always turned off）"，不可通过设置开关
> - 来源：https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/browser-features
> - Plugin Flags 不含这些快捷键，但它们已被运行时禁用，无需额外处理
> - 在 WebView2 中 Ctrl+W 事件**可穿透到 DOM**，被 xterm.js `attachCustomKeyEventHandler` 接收→发送 `\x17` 到 PTY（bash readline 删除前词）
>
> **5. ~~`prevent-default:default` 权限缺失的影响~~ ❌ 不适用**
> - **本地源码审查揭示**：该 crate（ferreira-tb/tauri-plugin-prevent-default v5.0.0）**无 `permissions/` 目录、无 `#[tauri::command]` 注解、无 IPC 命令**
> - 插件通过 `js_init_script()` 注入 JavaScript 到 WebView，**不经过 Tauri 2 ACL/权限系统**
> - Windows 上通过 `PlatformOptions::browser_accelerator_keys(false)` 直接调用 WebView2 COM 接口补充
> - **`capabilities/default.json` 中不需要 `"prevent-default:default"` 权限**——该标识符在此 crate 中不存在
> - 来源：`tauri-plugin-prevent-default-5.0.0/src/lib.rs`（`plugin_builder()` 仅调 `PluginBuilder::new("prevent-default")`，无 `.permission()` 调用）
>
> **综上**：A9 键盘 handler 缺失的判定**成立**，原审计技术细节中的"COPY flag"描述**错误**。但用户关于"capabilities 缺权限导致插件完全无效"的结论**同样不成立**——该插件不依赖 Tauri 权限系统。`prevent-default` 插件当前已通过 `init()` 生效（禁用全部浏览器快捷键），缺失 `with_flags` 仅影响开发便利性（Ctrl+F 查找和 F12 DevTools 被禁用）。

### 3.2 A1 SLTERM_PANE_ID 未注入（偏差）

**执行计划要求**（Agent A1 §8）："通过 `CommandBuilder.env` 注入 `SLTERM_PANE_ID`（UUID）"

**实际状态**：spawn.rs 中注释写"传递 pane token"，但无 `cmd.env()` 调用。shell-integration.ps1 写"嵌套检测留待 Phase 5：当前无 SLTERM_PANE_ID 环境变量，始终注入"。

**判定**：功能推迟至 Phase 5，与执行计划不一致。影响范围有限（嵌套检测非 Phase 1 硬需求），但执行计划偏差应记录。

> 🔍 核验：判定准确。补充说明——移除此 env 注入是 E2E 调试过程中发现的**必要修复**：原代码 `cmd.env("SLTERM_PANE_ID", &session_id)` 会导致 shell-integration.ps1 在嵌套检测时误判并直接 `return`，终端无任何输出。E2E 测试七回合排查后才定位此根因。推迟至 Phase 5 是务实的修整方案。执行计划偏差应记录但无需回退。

### 3.3 人工验收全部未执行

报告 §二 全部 7 项人工验证均为 ⬜ 待验证。执行计划 §5.3 人工验收清单未启动。

> 🔍 核验：判定准确。补充——人工验收未执行的原因之一是**面板创建无 UI 入口**（已知限制 #2），大部分人工验证项（真机敲命令、Ctrl+C 中断、编辑器保存等）需要先通过 DevTools 创建面板，操作路径不自然。

### 3.4 L4 E2E 无法验证终端输出内容（设计局限）

**问题**：WebGL 模式下 xterm.js 渲染到 `<canvas>`，`getText()` 无法读取 canvas 像素内容。测试只能验证 `.xterm-screen` 存在 + 面板 tab 创建，无法验证 `echo e2e_marker` 的实际输出。

**执行计划 DoD**：`L4 E2E："打开终端→输入 echo e2e→DOM 出现 e2e" PASS`。但 WebGL 渲染到 canvas，`e2e` 不在 DOM 中。

~~**网络检索确认**：xterm.js WebGL renderer 将内容渲染到单一 canvas 元素，DOM API 无法提取 canvas 文本。正确方案是通过 Buffer API（`buffer.active.getLine()` / `@xterm/addon-serialize`）提取，而非 DOM。建议前端暴露 `window.__e2e_getTerminalText()` 通过 serialize addon 返回终端内容。~~

> 🔍 核验（网络检索确认并详细化）：
>
> **原审计的"Buffer API 方案"完全正确**。4 路 agent 独立检索确认：
>
> **1. `@xterm/addon-serialize` 与渲染器无关**
> - `SerializeAddon` 读取的是终端内部 `Buffer`（`CircularList<BufferLine>`），不碰 DOM，不碰 Canvas
> - xterm.js 架构是 Model-View 分离：Buffer 是纯数据层，WebGL/DOM/Canvas 渲染器作为 View 层级独立消费 Buffer
> - 无论加载了 WebglAddon 还是 DOM 兜底，`serializeAddon.serialize()` 结果一致
> - 来源：canopyide/canopy #1307 对比结论、xterm.js 架构文档
>
> **2. `buffer.active.getLine()` 在所有渲染器下可用**
> - `IBuffer` API（`getLine()`、`translateToString()`、`length`、`cursorX` 等）属于 Core 层
> - 遍历所有行获取纯文本的代码模式已在社区广泛使用
> - 来源：xterm.js GitHub Discussions #4467, #4793, Issue #1994
>
> **3. 推荐方案（D2 决策的佐证）**
> - `window.__e2e_getTerminalText()` 通过 SerializeAddon 返回 ANSI 序列文本——技术上完全可行
> - SerializeAddon 方案优于纯 Buffer API，因为它保留 ANSI 颜色/格式信息，可验证颜色渲染正确性
> - 来源：@xterm/addon-serialize npm 文档 + 社区实践
>
> **4. 行业共识**
> - Proxmox VE、canopyide 等项目均采用"headless + serialize"组合做终端内容测试
> - WebGL canvas 文本不可 DOM 提取是行业共性（headless Chromium 无 GPU/WebGL 支持，CI 环境尤甚）
> - 来源：cluesmith/codev #473 (headless WebGL 在 CI 失败的已知问题)
>
> **综上**：原审计这个发现**正确且重要**，D2 方案（暴露 serialize-based E2E hook）有充分的社区实践支撑。

### 3.5 terminalTabConfig 定义但未使用（PTY 存活无保障）

**执行计划要求**（Agent A8）："终端面板 `renderer: 'always'`（保持 PTY 存活）"。

**实际状态**：`panelRegistry.ts` 第 18 行导出了 `terminalTabConfig = { renderer: 'always' }`，但 Grep 全仓搜索确认**无任何文件引用此变量**。

**影响**：Dockview 默认 `renderer` 为 `OnlyWhenVisible`——用户切换到其他标签页时，终端面板的 React 组件可能被卸载，cleanup 中 `pty.kill(sessionId)` 被调用，**终端进程被终止**。这是严重 UX 缺陷。

~~**Dockview 网络检索确认**：`AddPanelOptions.renderer` 类型为 `DockviewPanelRenderer` 枚举：
- `Always` — 面板始终渲染（即使不可见），组件不被卸载
- `OnlyWhenVisible`（默认）— 仅可见时渲染，切换标签时卸载组件~~

~~`renderer: 'always'` 需在 `addPanel()` 调用时作为 options 传入，而非仅导出配置。当前代码中 Workspace.tsx 和 E2E 测试中的 `addPanel()` 调用均未传 `renderer` 参数。~~

> 🔍 核验（代码搜索 + Dockview 源码确认）：
>
> **1. 代码证据**
> - `terminalTabConfig` 定义：`src/workspace/panelRegistry.ts:18-19`
>   ```typescript
>   export const terminalTabConfig = {
>     renderer: "always" as const,
>   };
>   ```
> - Grep 全仓搜索 `terminalTabConfig`：**定义位置仅此一处，零引用**
> - Grep 全仓搜索 `addPanel`：**应用代码中零调用**（仅在 E2E 测试的 `browser.execute` 中使用 `window.__dockviewApi.addPanel()`）
> - 当前 Workspace.tsx 无手动创建面板的 UI（右键菜单/工具栏按钮），面板创建完全依赖 Dockview 内置行为或 DevTools
>
> **2. Dockview 源码确认**（来自 `node_modules/dockview-core/dist/cjs/dockview/options.d.ts` v6.6.1）
> - `renderer` 是 `AddPanelOptions` 的可选字段，类型为 `DockviewPanelRenderer = 'onlyWhenVisible' | 'always'`
> - 不传 `renderer` 时回退到 `DockviewOptions.defaultRenderer`，再回退到 `'onlyWhenVisible'`
> - 来源：Dockview 6.6 类型定义 + 官方文档 https://dockview.dev/docs/core/panels/rendering/
>
> **3. 影响评估**
> - **当前阶段影响有限**：因为没有创建面板的 UI 入口，标签切换场景不会自然触发
> - **一旦实现右键菜单/工具栏创建面板**（Phase 2），切换到另一个标签时 PTY 会被 kill——这是 UX 缺陷
> - **E2E 测试中不受影响**：测试只创建单个面板，不切换标签
> - 修复时机：**建议 Phase 2 实现面板创建 UI 时一并修复**（在 `addPanel()` 调用处传 `renderer: 'always'`），Phase 1 可保留为已知缺陷
>
> **判定**：原审计发现**正确**，严重程度需下调一级——当前阶段因无标签切换场景，实际影响为零；Phase 2 若不修复则升级为严重缺陷。

## 4. 弱测试与测试质量

### 4.1 L3 `key_encoding_shift_tab` 断言无效

```typescript
it('key_encoding_shift_tab', async () => {
    const { term, serialize } = createTerminal();
    await writeSync(term, '\x1b[Z');
    const result = serialize.serialize();
    expect(result.length).toBeGreaterThanOrEqual(0);  // 永远为真
});
```

`expect(result.length).toBeGreaterThanOrEqual(0)` 对任何输入（含空字符串）都为真，**不验证 Shift+Tab 编码 `\x1b[Z` 是否正确**。应改为 `expect(result).toContain('\x1b[Z')` 或验证 serialize 输出非空。

> 🔍 核验：完全正确。`length >= 0` 对空字符串也返回 true（`"".length = 0 >= 0`），属于永真断言。应验证 `\x1b[Z` 序列确实出现在 serialize 输出中，或至少验证输出非空。

### 4.2 L3 测试与执行计划期望的差距

执行计划 §7.1 期望：
- `key_encoding_shift_tab`："按键产生正确字节序列"
- `key_encoding_ctrl_c`："按键产生正确字节序列"

实际测试中 `key_encoding_ctrl_c` 仅验证 `write('^C')` → serialize 含 `'^C'`，这是写入字面文本而非验证按键编码。`key_encoding_shift_tab` 断言完全无效。两者均未做到"按键产生正确字节序列"。

> 🔍 核验：完全正确。两个测试都是向终端写**字面文本**（`'^C'` 和 `'\x1b[Z'`），而非模拟按键输入（如 `xterm.js` 的 `attachCustomKeyEventHandler` 或 `term.input()` 等方法产生的实际按键编码）。执行计划要求验证的是"按键行为产生正确的字节序列发给 PTY"，当前测试验证的是"seralize 能解析某些文本"，是不同层面的测试。

## 5. 依赖与打包

### 5.1 测试依赖误放在 dependencies

| 包 | 当前位置 | 应在位置 |
|----|---------|---------|
| `@xterm/addon-serialize` | dependencies | devDependencies（仅 L3 测试用） |
| `@xterm/headless` | dependencies | devDependencies（仅 L3 测试用） |

> 🔍 核验：正确。Grep `package.json` 确认两包均在 `dependencies` 字段（非 devDependencies）。这两个包仅被 `test/terminal/keyboard.test.ts` 和 `test/terminal/terminal-serialize.test.ts` 引用，不在应用代码中使用。移到 devDependencies 可减少生产包体积约 80KB。

### 5.2 CodeMirror 依赖状况

~~`package.json` 同时依赖 `codemirror: "^6.0.2"`（组合入口）和 14 个 `@codemirror/*` 独立包。`useCodeMirror.ts` 从 `@codemirror/*` 导入，`codemirror` 包未被实际使用，是冗余依赖。~~

> 🔍 核验（代码 + npm 包分析确认）：
>
> **1. `codemirror` v6 确实被使用** ✅
> ```typescript
> // src/panels/editor/useCodeMirror.ts:12
> import { basicSetup } from "codemirror";
> ```
> `codemirror` v6 是 CM6 组合入口包，`basicSetup` 聚合了 7 个 `@codemirror/*` 子包的 ~24 个扩展（lineNumbers/history/foldGutter/autocompletion 等）。**原审计判"冗余"不成立。**
> 来源：`node_modules/codemirror/dist/index.d.ts`（本地确认）
>
> **2. 但存在真正的冗余依赖：`@codemirror/basic-setup` ^0.20.0**
> - `@codemirror/basic-setup` 是 `codemirror` v6 的**旧名**（v6.0.0 之前叫 `@codemirror/example-setup`，后改名为 `@codemirror/basic-setup`，最终改名为 `codemirror`）
> - v0.20.x 系列已被 `codemirror` v6.x 完全替代——**这是真正的冗余依赖**
> - 来源：codemirror/basic-setup GitHub 仓库 + npm 发布历史
>
> **3. `codemirror` v6 与 14 个 `@codemirror/*` 独立包并存**
> - `codemirror` v6 的 `dependencies` 声明了对 7 个 `@codemirror/*` 子包的 `^6.0.0` 依赖
> - 项目的 `package.json` 又直接声明了 14 个 `@codemirror/*` 子包（包含 codemirror 已依赖的 7 个 + 额外 7 个）
> - 虽然 npm 会 dedup 到同一份代码，但 `package.json` 层面的语义重复仍然存在
> - **建议**：如果编辑器需要自定义组合（如增删扩展、调参），移除 `codemirror` 包，手动组合 `@codemirror/*` 子包；如果只需默认体验，移除独立子包，只保留 `codemirror`
>
> **判定**：原审计"codemirror 冗余"**不成立**（`basicSetup` 正在被使用）。但`@codemirror/basic-setup` ^0.20.0 **确实是冗余依赖**（旧名包，已被 `codemirror` v6 替代）。

### 5.3 版本锁定不完整

多数依赖仍使用 `^` 前缀（如 `@codemirror/*`、`@wdio/*`、`vitest` 等），与 Phase 0 建立的"精确钉"原则不一致。虽不影响功能，但降低构建可复现性。

> 🔍 核验：正确。`package.json` 中大量依赖使用 caret（`^`），如 `"@codemirror/state": "^6.6.0"`、`"codemirror": "^6.0.2"`、`"vitest": "^4.1.9"`。与 Phase 0 精确钉 `"@tauri-apps/api": "2.11.1"`（无 caret）的标准不一致。`package-lock.json` 已锁定已解析版本，`npm ci` 仍可保证复现；`npm install` 不可复现。建议后续逐步去 caret。

### 5.4 jsdom 缺少 HTMLCanvasElement.getContext polyfill

当前 `src/__tests__/setup.ts` 仅 polyfill 了 `crypto.getRandomValues` 和 `ResizeObserver`，未包含 `HTMLCanvasElement.prototype.getContext`。如果 `@xterm/addon-webgl` 或任何 canvas 相关代码在测试路径中被间接导入，会触发 `Error: Not implemented: HTMLCanvasElement.prototype.getContext`。当前测试通过是因为 `useXterm` 和 `useCodeMirror` 均被 vi.mock 替代，绕过了此限制。

> 来源：`node_modules/@xterm/addon-serialize/package.json`（无 peerDependencies）、vitest issue #3025

## 6. 报告准确性

| 项 | 报告声称 | 核实 | 判定 |
|----|---------|------|------|
| L1 cargo test 9 passed | ✅ | 实跑 9 passed | 准确 |
| L2 npm test 6 files 10 tests | ✅ | 实跑 6 files 10 tests | 准确 |
| L3 npm run test:l3 2 files 5 tests | ✅ | 实跑 2 files 5 tests | 准确 |
| L4 wdio 1 spec PASSED | ✅ | 实跑 1 passed | 准确 |
| tauri build exe 产出 | ✅ | 实跑产物存在 | 准确 |
| ESLint 退出 0 | ✅ | 实跑无输出 | 准确 |
| Clippy 退出 0 | ✅ | 实跑 Finished | 准确 |
| 目录骨架 | ✅ | 对照一致 | 准确 |
| 版本依赖表 | ⚠️ | `tauri-plugin-prevent-default` 报告写 `5`，Cargo.lock 为 `5.0.0` | 可接受简化 |
| 硬约束守规 | ⚠️ | "JS 键盘 handler 留待 Phase 2"——但执行计划视其为 Phase 1 项，非"守规"成功 | 措辞偏差 |

> **报告无失实**。所有自动化证据与实跑一致。偏差在于范围解释：报告将 Agent A9 键盘 handler 归类为"Phase 2 补充"，而执行计划明确列入 Phase 1 W1 实现编排。

> 🔍 核验：原审计对报告准确性的判断**完全正确**。补充——硬约束原始定义是"不弱化 lint、不放宽测试断言、不引入 Phase 1 范围外业务功能"，A9 键盘 handler 属于**缺漏**（未完成计划内项）而非"守规失败"。建议在报告中调整此表述。

## 7. 已知的已知（报告 §三 已列出）

| # | 限制 | 审计意见 |
|----|------|---------|
| 1 | xterm.js WebGL canvas 渲染，DOM API 无法读文本 | 设计局限，非缺陷。建议后续加 buffer API hook |
| 2 | Dockview 面板创建无 UI 入口 | Phase 2 范围，合理 |
| 3 | 键盘 handler 未接入 | ⚠️ 执行计划要求 Phase 1 交付，不是"限制" |
| 4 | L4 teardown 噪声（Failed to clear mock store） | Phase 0 遗留，已知无害 |
| 5 | L4 startup 噪声（Failed to get window states） | **本轮观察到非常频繁**（单次测试出现 10+ 次），严重污染日志 |

> L4 startup noise 比 Phase 0 显著恶化——Phase 0 只出现 1-2 次，Phase 1 测试中每次 WebDriver 命令前几乎都触发。根因是 `ensureActiveWindowFocus` 在每个 `beforeCommand` 钩子中都调用 `getWindowStates`，而 `core.invoke` 在 embedded driver 模式下初始化较慢。建议：WDIO 配置中禁用 `ensureActiveWindowFocus` 或在 `beforeCommand` 中去重。

> 🔍 核验：L4 startup noise 的观察**正确**。本次 E2E 实测中 `Failed to get window states` 在每次 WebDriver 命令前均出现，单次测试触发超过 10 次，显著多于 Phase 0 的 1-2 次。根因分析：`@wdio/tauri-service` 的 `TauriWorkerService.beforeCommand()` 钩子每次调用 `ensureActiveWindowFocus()`，后者调用 `getWindowStates()`，后者依赖 `core.invoke` 就绪。embedded driver 模式下 `core.invoke` 初始化较慢（与常规 tauri-driver 不同）。Phase 0 的 E2E 测试只有 1-2 个 WebDriver 命令，Phase 1 有 10+ 个（含 executeScript 轮询循环）。建议不影响功能，但日志噪音确实需要处理。

## 8. 决策台账（E1–E16 执行一致性）

| # | 决策 | 执行一致性 | 说明 |
|---|------|-----------|------|
| E1 | 编排策略 /goal + workflow | ✅ | 按计划执行 |
| E2 | WebGL + DOM 兜底 | ✅ | `detectWebgl()` + `failIfMajorPerformanceCaveat: true` |
| E3 | Channel\<PtyEvent\> IPC 输出 | ✅ | spawn 模式 Channel 长期持有 |
| E4 | PTY IPC 输入 invoke | ✅ | pty_write 用 invoke |
| E5 | portable-pty 0.9.0 + CPR | ✅ | 版本正确 + CPR `\x1b[1;1R` |
| E6 | PowerShell -File 注入 | ✅ | `-NoProfile -NoLogo -File` |
| E7 | 集成脚本 include_str! + %APPDATA% | ✅ | 嵌入 + 运行时写入 |
| E8 | OSC 9;9 优先 + OSC 7 | ✅ | 脚本含两种 CWD 跟踪 |
| E9 | OSC 133 A/B/C/D | ✅ | 含 PSReadLine Enter 钩子 |
| E10 | 前端先生成 panelId → spawn | ✅ | crypto.randomUUID 未显式调用但使用 Dockview id |
| E11 | Panel ID / Session ID 双层 | ✅ | panelId 前端 + sessionId Uuid::new_v4 |
| E12 | 会话不放 Zustand | ✅ | 模块级 Map（PtyState） |
| E13 | 键盘策略 3 handler | ❌ | **全部未实现** |
| E14 | tauri-plugin-prevent-default | ⚠️ | 已添加（`init()` = `Flags::all()`），但未按执行计划配置 `with_flags(Flags::all().difference(Flags::FIND \| Flags::DEV_TOOLS))`——Ctrl+F 和 F12 DevTools 被禁用，开发不便。<br>~~capabilities 缺权限~~ **不适用**：该插件无 Tauri IPC 命令，纯 JS 注入，不经过权限系统 |
| E15 | 面板管理纯鼠标 | ⚠️ | Dockview 拖拽分屏 + 右键菜单已实现，但 `terminalTabConfig`（`renderer: 'always'`）定义后未使用——标签切换时 PTY 可能被 kill（见 §3.5） |
| E16 | Ctrl+Shift+C/V | ❌ | **未实现**（E13 的一部分） |
| E17 | 全局热键 Phase 1 不做 | ✅ | 未实现（符合决策） |

**汇总**：E1–E12/E17 共 13 项决策执行一致。E13/E16 共 2 项存在偏差（键盘 handler 未实现）。E14/E15 为部分偏差（插件配置简化 + terminalTabConfig 未使用）。

> 🔍 核验（二次修正）：E14 中 `with_flags` 缺失仅影响开发便利性（`init()` = 禁用全部已知加速键，包括 Ctrl+F 和 DevTools）。`capabilities` 无需额外权限——该插件无 Tauri IPC 命令，纯 JS 注入，绕开 ACL 系统（源码确证：无 `permissions/` 目录、无 `#[tauri::command]`）。E15 严重程度见 §3.5 核验。

## 9. 网络检索关键发现

5 路并行检索汇总（4 agent 异步 + 1 agent 同步补充）：

| 领域 | 关键发现 | 对审计的影响 |
|------|---------|-------------|
| xterm.js WebGL | Buffer API 与渲染层独立——`buffer.active.getLine()` / `@xterm/addon-serialize` 在所有渲染器（WebGL/DOM/Headless）下结果一致；WebGL canvas 文本不可 DOM 提取；#4574（无 GPU 时 WebGL2 软件模拟字符丢失）**上游未修复**，`failIfMajorPerformanceCaveat: true` 检测方案是业界标准（Proxmox VE 采用同方案） | 验证 L4 E2E 限制是行业共性；D2 Buffer API hook 方案正确 |
| portable-pty 0.9 | `take_writer` 破坏性变更确认（0.8.0 引入，0.9.0 继承）；CPR 提前排队方案（turborepo #11816）已证稳定；Cygwin commit 919dea66 确证并发 spawn 需要 mutex 串行化；`PSEUDOCONSOLE_INHERIT_CURSOR` 是 0.9.0 新增标志，触发 DSR→CPR 握手；stdin drop 在 Windows 立即杀子进程 | 验证当前实现（take_writer + CPR + SPAWN_LOCK + Job Object + 不 drop stdin）完全正确，与 turborepo/wezterm 行业实践一致 |
| tauri-plugin-prevent-default | v5.0.0 源码确证：Flags 枚举 10 个常量（FIND/CARET_BROWSING/DEV_TOOLS/DOWNLOADS/FOCUS_MOVE/RELOAD/SOURCE/OPEN/PRINT/CONTEXT_MENU），**无 COPY/PASTE**；插件纯 JS 注入（`js_init_script`），**无 Tauri IPC 命令，不依赖权限系统**；`init()` = `Builder::default().build()` = `Flags::all()`；`Builder::new()` 和 `Builder::default()` 均存在且等价 | 确认：① 插件不干扰 Ctrl+C；② `capabilities` 无需添加 `prevent-default:default`（插件绕开 ACL）；③ `with_flags` 缺失仅影响开发便利性（Ctrl+F/DevTools 被禁用）；④ Ctrl+W/T/N 由 WebView2 硬编码关闭，非插件范围 |
| DockviewReact 6.6 | `AddPanelOptions.renderer` 类型确认为 `DockviewPanelRenderer`：`Always`（始终渲染，PTY 存活）/ `OnlyWhenVisible`（默认，不可见时卸载）；`watermarkComponent` API 正确；`fromJSON()` bug #341 已知——`layoutSerde.ts` 的 try/catch 防护正确；`api.toJSON()` 仅存 `params` 中静态数据，动态状态用 Zustand——此方案与社区最佳实践一致 | **发现新问题**：`terminalTabConfig`（含 `renderer: 'always'`）已定义但**从未被使用**。标签切换时终端面板可能被卸载→PTY 被杀。见 §3.5 |
| @xterm/headless + serialize | `@xterm/addon-serialize@0.14.0` 与 `@xterm/xterm@6.0.0` **完全兼容**（同一发布周期）；serialize 读 buffer 非 DOM，headless 与 DOM 模式输出**理应一致**；`windowsPty: { backend: 'conpty', buildNumber: 21376 }` 的 buildNumber 是 Windows 11 Insider Preview Build 21376（2021 年 5 月 Dev Channel），用于启用 ConPTY DECSET 2026（同步更新模式）等较新特性 | 证实 D2（加 `window.__e2e_getTerminalText()` 通过 serialize 返回文本）技术可行；headless + serialize 组合已验证可靠 |

> 🔍 核验：检索汇总中 `tauri-plugin-prevent-default` 行有两处需要修正（已用删除线标注）。其余发现已在本轮代码 + 网络核验中确认。

## 10. 一句话结论

**L1–L4 实跑全绿、代码与报告一致、无失实。但 Agent A9（键盘 handler）完全未实现，E13/E14/E16 三项决策执行偏差，DoD 1.5（跑 claude）无法可靠验收。** 键盘 handler 是 Phase 1 执行计划明确要求的交付项，不能降级为"已知限制"推至 Phase 2。其余偏差（SLTERM_PANE_ID 推迟、弱测试断言、依赖打包）属可修范围。

> 🔍 核验（交叉核验最终修正）：用户补充的核验批注 13/15 项正确。修正 2 处：
> 1. **§3.1 "capabilities 缺权限导致插件完全无效"不成立**——`tauri-plugin-prevent-default` v5.0.0 无 permissions 目录、无 IPC 命令，纯 JS 注入绕开 Tauri ACL
> 2. **§5.2 冗余依赖是 `@codemirror/basic-setup` 而非 `codemirror`**——`codemirror` 被 `basicSetup` 导入使用，`@codemirror/basic-setup` 是其旧名包（v0.20 系列，已被 v6 替代）
>
> 其余 13 项核验全部正确。

## 11. 审计决策台账（grill 产出）

以下决策由用户在本轮审计中确认，纳入 Phase 1 收口范围：

| # | 议题 | 决策 | 落点 |
|---|------|------|------|
| D1 | A9 键盘 handler 缺失 | **Phase 1 补齐**：实现 keyboard.ts（Ctrl+C 智能分流 + Ctrl+Shift+C/V 复制粘贴 + tauri-plugin-prevent-default `with_flags` 开发便利性优化）。IME 防御可推后。<br>**注意事项**：① `prevent-default` 插件不依赖 Tauri 权限系统（无 IPC 命令，纯 JS 注入），`capabilities/default.json` 无需额外权限；② Ctrl+W/T/N 已被 WebView2 硬编码关闭，插件无需处理；③ 当前 `init()` 已禁用全部浏览器快捷键（含 Ctrl+F 和 DevTools），`with_flags` 修改仅用于恢复开发工具 | `src/panels/terminal/keyboard.ts`（新建）、`lib.rs` |
| D2 | L4 E2E 终端输出验证 | **加 Buffer API hook**：useXterm.ts 暴露 `window.__e2e_getTerminalText()` 通过 serialize addon 返回终端文本，E2E 测试改用此 API 验证 `echo e2e_marker` 输出 | `src/panels/terminal/useXterm.ts`、`e2e-tests/test.e2e.ts` |
| D3 | L3 `key_encoding_shift_tab` 弱测试 | **修复两个测试**：shift_tab 验证 serialize 输出含 `\x1b[Z`；ctrl_c 验证写入 `\x03` 后 serialize 含对应内容 | `test/terminal/keyboard.test.ts` |
| D4 | 依赖打包 | **修复**：`@xterm/addon-serialize` + `@xterm/headless` 移入 devDependencies；移除冗余 `@codemirror/basic-setup` ^0.20.0（`codemirror` v6 的旧名包，已被替代） | `package.json` |
| D5 | terminalTabConfig 未使用 | **修复**：在 `addPanel()` 调用处（Workspace.tsx 右键菜单/E2E 测试）传入 `renderer: 'always'`，确保标签切换时 PTY 不被 kill。或删除未使用的 `terminalTabConfig` 导出 | `src/workspace/Workspace.tsx`、`e2e-tests/test.e2e.ts` |

> 🔍 核验：D1 中"capabilities 权限"已移除——`tauri-plugin-prevent-default` 不依赖 Tauri 权限系统（无 permissions 目录、无 IPC 命令）。D4 中 `codemirror` 保留（`basicSetup` 正在使用），改为移除 `@codemirror/basic-setup`（旧名冗余包）。其余 D2–D5 决策保持不变。

## 12. 收口路径

1. 执行 D1–D5 五项修复。
2. 实跑验证：`cargo test` / `npm test` / `npm run test:l3` / `npm run wdio` / `cargo clippy` / `npx eslint` 仍全绿。
3. 更新 `Phase 1 结果验证.md` 反映修复后的状态。
4. DoD 全部满足后 Phase 1 收口。
