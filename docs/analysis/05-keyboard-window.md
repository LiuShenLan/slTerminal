# 键盘/快捷键/窗口 — claude 定制优化盘点

> 盘点范围：快捷键框架（ShortcutRegistry）、保留键、复制键约定、终端/编辑器快捷键、WebView2 按键控制、窗口关闭清理与孤儿进程防护。仅描述现状，不含优化建议。

## 相关文件

| 领域 | 文件 |
|------|------|
| 快捷键框架 | `src/features/shortcuts/ShortcutRegistry.ts`、`reserved.ts`、`commandCatalog.ts`、`globalCommands.ts`、`usePanelFocus.ts`、`wireKeybindings.ts`、`keystroke.ts`、`src/features/shortcuts/CLAUDE.md` |
| 终端键盘 | `src/panels/terminal/keyboard.ts`、`activeTerminal.ts`、`useXterm.ts`（attachCustomKeyEventHandler 委托，行 229-237）、`theme.ts`（Kitty 键盘协议，行 20-22） |
| 编辑器/资源管理器键盘 | `src/panels/editor/keyboard.ts`、`activeEditor.ts`、`src/features/explorer/keyboard.ts`、`activeExplorer.ts` |
| 剪贴板/窗口 IPC | `src/ipc/clipboard.ts`、`src/ipc/window.ts`（registerCloseHandler/onFocusChanged/requestUserAttention） |
| 命令注册与关窗编排 | `src/App.tsx`（命令一次性注册行 165-173、wireKeybindings 行 176-178、关窗清理行 102-162、焦点监听行 181-191、SHUTDOWN_TIMEOUT_MS 行 28） |
| 后端窗口/进程 | `src-tauri/src/lib.rs`（prevent-default 插件配置行 56-69）、`src-tauri/src/pty/spawn.rs`（Job Object，行 813-818 / 1030-1038 / 1324-1425）、`src-tauri/src/state.rs`（job_object 字段行 33） |
| 文档 | 根 `CLAUDE.md`（复制键约定、P1-19 需求编号索引）、`src/panels/CLAUDE.md`（Ctrl+C 保留为中断节、attachCustomKeyEventHandler 节、Kitty 节、中断场景已知行为节）、`src/features/shortcuts/CLAUDE.md`（WebView2 三层按键控制节、Ctrl+C 保留为中断节）、`src-tauri/src/pty/CLAUDE.md`（Job Object 节） |

## 优化项清单

| # | 优化 | 位置(file:line) | 机制 | 触发点（claude 哪个行为） | 专属程度 |
|---|------|----------------|------|--------------------------|----------|
| 1 | 复制键约定 Ctrl+Shift+C/V（Ctrl+C 让位） | `src/panels/terminal/keyboard.ts:20-39`、`src/features/shortcuts/commandCatalog.ts:36-50`、`src/ipc/clipboard.ts:4`、根 `CLAUDE.md`「项目性质」 | 复制/粘贴绑定 `Ctrl+Shift+KeyC`/`Ctrl+Shift+KeyV`，Ctrl+C 不占用 | claude 用 Ctrl+C 取消操作，复制必须避开该键 | 硬编码 claude |
| 2 | Ctrl+C 保留为中断（不注册命令） | `src/panels/terminal/keyboard.ts:47`、`src/features/shortcuts/commandCatalog.ts`（无 Ctrl+C 条目）、`src/panels/CLAUDE.md`「Ctrl+C 保留为中断」节 | 目录中无 Ctrl+C 命令 → ShortcutRegistry 无匹配即透传 → xterm.js 自然发送 `\x03` 到 PTY | claude 取消操作（Ctrl+C → SIGINT） | 硬编码 claude |
| 3 | 终端控制字符保留键（Ctrl+C/V/X/Z/A） | `src/features/shortcuts/reserved.ts:14-20`、`ShortcutRegistry.ts:181-199`（effectiveKeystroke 校验） | `TERMINAL_RESERVED` 集合标记 5 个控制字符键，用户覆盖命中即 console.warn + 回退默认键，保留键值永不进绑定表（命令仍以默认键进指纹索引） | 保护 SIGINT 等控制字符语义——claude 中断依赖 Ctrl+C 不被抢 | 通用机制但 claude 触发 |
| 4 | CM 内部键保留 + Ctrl+F 排除出浏览器加速键拦截 | `src/features/shortcuts/reserved.ts:23-30`、`src-tauri/src/lib.rs:59-61`（`Flags::all().difference(Flags::FIND)`） | `EDITOR_RESERVED`（Ctrl+F/Z/Y、Shift+Tab、Tab）；prevent-default 全拦截排除 FIND，让 Ctrl+F 落到 CodeMirror 搜索 | 无 claude 关联——CodeMirror 编辑器自身键管理 | 完全通用 |
| 5 | terminal.newline：Ctrl+Enter → 写 `\n` 不提交 | `src/panels/terminal/keyboard.ts:40-46`、`commandCatalog.ts:52-58`、`src/panels/CLAUDE.md`「attachCustomKeyEventHandler」节 | 命令 handler 经 `writeToPty` 写 `0x0a`（Ctrl+J 等价）到 PTY，注释明写"Ink 据此插入换行不提交" | claude 的 Ink 渲染器区分"换行"与"提交" | 硬编码 claude |
| 6 | attachCustomKeyEventHandler 委托式 fallback | `src/panels/terminal/useXterm.ts:229-237`、`src/panels/CLAUDE.md`「attachCustomKeyEventHandler — 委托式 fallback」节 | xterm handler 委托 `getShortcutRegistry().resolve(event, "terminal")`（forceContext 兜底），命中 → preventDefault+return false，未命中 → 透传（注释：Ctrl+C 等控制字符发往 PTY） | xterm 6.1.0-beta 焦点异常时的终端按键兜底——激活条件是 xterm 版本行为而非 claude 行为，claude 工作流（Ctrl+Shift+C/V、Ctrl+Enter、Ctrl+C 透传）为当前主要受益场景 | 通用机制但 claude 触发 |
| 7 | Kitty 键盘协议（CSI u）被动启用 | `src/panels/terminal/theme.ts:20-22`（`vtExtensions: { kittyKeyboard: true }`）、`src/panels/CLAUDE.md`「Kitty 键盘协议」节 | 终端声明 CSI u 能力，子进程主动 `CSI>1u` 激活 Disambiguate 模式；注释明写"允许子进程（如 Claude Code）…激活" | claude 主动启用 Kitty 差异化键编码 | 通用机制但 claude 触发 |
| 8 | WebView2 三层按键控制 | `src-tauri/src/lib.rs:56-69`（prevent-default 插件）、`ShortcutRegistry.ts:63`（window capture keydown）、`src/features/shortcuts/CLAUDE.md`「WebView2 三层按键控制」节 | ① WebView2 硬编码禁用 Ctrl+W 关窗 ② prevent-default 拦 Ctrl+P/R（F12 经 `dev_tools(false)` 禁用，非 Flags 集）、`browser_accelerator_keys(false)` ③ ShortcutRegistry capture 处理 Ctrl+Shift+C/V、Ctrl+W、Ctrl+S、Alt+Z（Ctrl+S/Alt+Z 为 editor 聚焦时生效） | 保证 claude 工作流的全局键（关页签/复制/中断）不被浏览器加速键吞掉 | 通用机制但 claude 触发 |
| 9 | 快捷键框架：指纹索引 + 上下文栈 + active 指针一次注册 | `ShortcutRegistry.ts`（指纹索引 202-218、上下文栈 106-125、findWinner 224-249）、`commandCatalog.ts:26-99`（9 条命令目录）、`usePanelFocus.ts:24-63`、`App.tsx:165-173`、`activeTerminal.ts`/`activeEditor.ts` | 命令 App 一次性注册，handler 经模块级 active 指针派发到聚焦实例；指纹 O(1) 匹配 + 上下文过滤 + 优先级排序 | claude 多会话多终端场景下，Ctrl+Shift+C/V 等命令恒作用于聚焦终端 | 通用机制但 claude 触发 |
| 10 | 用户自定义重绑定 + 校验静默降级 | `wireKeybindings.ts:20-27`、`src/stores/keybindings.ts`、`ShortcutRegistry.ts:181-199`、`App.tsx:176-178`、`src/features/shortcuts/CLAUDE.md`「用户自定义重绑定」节 | overrides 经 `setOverrides` 重建绑定表；非法/保留键 console.warn + 回退默认，null 解绑；settings.json `keybindings` 段持久化 | 用户可重绑 claude 工作流键位，同时保留键保护确保 Ctrl+C 中断通道不可被绑走 | 通用机制但 claude 触发 |
| 11 | global.closeTab（Ctrl+W 关页签） | `commandCatalog.ts:27-34`、`globalCommands.ts:21-31`、`src/features/shortcuts/CLAUDE.md`「全局快捷键」节 | Ctrl+W 关闭活跃面板；无活跃面板 → 透传（注释：xterm.js 可接收 `\x17` 用于 bash readline） | 无 claude 关联——通用页签关闭操作（Ctrl+W 由用户按下、作用于任意类型页签） | 完全通用 |
| 12 | HTML iframe 键盘转发（postMessage） | `src/panels/html/HtmlPanel.tsx`（INJECTED_SCRIPT）、`ShortcutRegistry.ts:151-159`（exportContextBindings）、`src/features/shortcuts/CLAUDE.md`「HTML iframe 全局键转发」节 | iframe keydown → postMessage → 父窗口 origin/source 两层校验 + type/fingerprint 比对 → 比对 `exportContextBindings("global")` → 合成 KeyboardEvent（打信任标记）重放进 ShortcutRegistry | claude 工作流中预览 HTML 时全局快捷键（Ctrl+W 等）不失效 | 通用机制但 claude 触发 |
| 13 | 关窗杀子进程（P1-19） | `src/App.tsx:102-162`（registerCloseHandler 编排）、`src/ipc/window.ts:56-69`（onCloseRequested → preventDefault → destroy）、`src/App.tsx:28`（SHUTDOWN_TIMEOUT_MS=3000） | onCloseRequested → preventDefault → 遍历 TerminalRegistry 全部 session 并发 `pty.kill`（`Promise.race` 3s 超时）→ flush 布局/持久化 → `destroy` | 窗口关闭时 claude 会话（PTY 子进程树）必须被杀干净，不留后台残留 | 通用机制但 claude 触发 |
| 14 | Job Object KILL_ON_JOB_CLOSE 孤儿防护兜底 | `src-tauri/src/pty/spawn.rs:813-818`（JobHandle RAII）、`:1030-1038`（spawn 后 add_to_job_object，失败显式 kill）、`:1324-1425`（job_name/job_limits 纯函数 + Win32 三调用 + 两步句柄管理，详见详细节）、`state.rs:33`、`src-tauri/src/pty/CLAUDE.md`「Job Object 孤儿防护」节 | 每个子进程 `CreateJobObjectW` + `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` + `AssignProcessToJobObject`；JobHandle 全程持有防过早触发，drop 时 CloseHandle | 无（进程生命周期通用）；claude 作为长驻子进程是主要受益对象，与前端 P1-19 双保险 | 完全通用 |
| 15 | 窗口焦点监听 + 任务栏闪烁回窗引导 | `src/App.tsx:181-191`、`src/ipc/window.ts:13-36`（onFocusChanged/requestUserAttention） | `onFocusChanged` 维护 `window.__slterm_windowFocused`；失焦时 claude 通知触 toast + 任务栏闪烁（FLASHW_TIMERNOFG），聚焦恢复时 `requestUserAttention(null)` 停闪 | claude 后台运行（权限请求/任务完成/错误）需回窗引导 | 通用机制但 claude 触发 |
| 16 | Ctrl+Wheel 字体缩放 / 编辑器与 explorer 快捷键 | `src/lib/useFontSizeWheel.ts`、`src/panels/editor/keyboard.ts:17-32`（save/toggleWordWrap）、`src/features/explorer/keyboard.ts:18-38`（delete/open/rename） | Ctrl+Wheel 共享 hook 缩放字号；`editor.save`（Ctrl+S）/`editor.toggleWordWrap`（Alt+Z）/explorer 三键（重命名 input 活跃时透传）均为标准编辑器/文件树能力 | 无 claude 关联——通用编辑器/资源管理器操作 | 完全通用 |
| 17 | CSP 放行 HTML 预览内联脚本（HTML 面板执行前提） | `src-tauri/tauri.conf.json:24-25` | `script-src 'self' 'unsafe-inline'` + `dangerousDisableAssetCspModification: ["script-src"]`；srcdoc iframe 继承父窗口 CSP 且子策略只能收紧不能放宽，须放宽主窗口 CSP 并关 script-src nonce 注入（代价是全局失去 script nonce 加固） | claude 工作流中预览 claude 生成的 HTML 输出（与 05-12 同源） | 通用机制但 claude 触发 |

---

## 详细机制描述

### 1. 复制键约定 Ctrl+Shift+C/V（硬编码 claude）

根 `CLAUDE.md`「项目性质」明示定位约束："复制 = `Ctrl+Shift+C`（`Ctrl+C` 保留为中断，供 claude 取消）"。实现为 `commandCatalog.ts:36-50` 中 `terminal.copy`（defaultKey `Ctrl+Shift+KeyC`）与 `terminal.paste`（defaultKey `Ctrl+Shift+KeyV`），handler 在 `keyboard.ts:20-39`：复制取 xterm 选区经 `src/ipc/clipboard.ts` 的 `writeText` 写入（总是阻止默认防 xterm 见到该组合），粘贴 `readText` 后 `t.paste`。剪贴板 IPC 为 `@tauri-apps/plugin-clipboard-manager` 直 re-export，与 OSC 52 拦截共用同一写入路径（`src/panels/CLAUDE.md`「OSC 52 剪贴板拦截」节）。该约定是项目定位约束之一（根 `CLAUDE.md`「项目性质」共 6 条定位约束：3 个 bullet，仅首条 bullet 用分号分隔 4 条主约束子项 + 默认 shell 回退 + 复制约定），直接为 claude 的 Ctrl+C 中断让路。

### 2. Ctrl+C 保留为中断（硬编码 claude）

三层保障（`src/panels/CLAUDE.md`「Ctrl+C 保留为中断」节：`keyboard.ts` 的 `createTerminalShortcuts` 不注册 Ctrl+C 命令——ShortcutRegistry 无匹配即透传，xterm.js 自然发送 `\x03` 到 PTY，**claude 用它取消操作**）：

1. `keyboard.ts:47` 注释明写"Ctrl+C 不注册命令 → 自然透传，xterm.js 发送 \x03 到 PTY"；`commandCatalog.ts` 9 条命令中无 Ctrl+C 条目
2. `reserved.ts:15` 将 `Ctrl+KeyC` 标记为保留键，用户覆盖无法绑定（保留键完整集合与 `effectiveKeystroke` 校验见 #3——此处仅交叉引用，不展开）
3. `src/features/shortcuts/CLAUDE.md`「Ctrl+C 保留为中断」节红线："勿在任何地方注册 Ctrl+C 命令"

配套行为记录：`src/panels/CLAUDE.md`「中断场景已知行为（Ctrl+C）」节——claude 主动 Ctrl+C 中断时不发射任何 hook 事件，四态状态机 `working`（⚡）无中断出边，页签滞留 ⚡ 直至下一事件覆盖（含 60s `idle_prompt` 内置衰减转 🟡），属已接受的规划缺口。

### 3. 终端控制字符保留键（通用机制但 claude 触发）

`reserved.ts:14-20` 的 `TERMINAL_RESERVED = { Ctrl+KeyC(→\x03 SIGINT), Ctrl+KeyV(\x16), Ctrl+KeyX(\x18), Ctrl+KeyZ(\x1a), Ctrl+KeyA(\x01 行首) }`。`isReserved(ks, context)`（行 36-42）按 context 分集判定，global context 取终端+编辑器两集并集。校验点在 `ShortcutRegistry.effectiveKeystroke`（行 181-199）：仅校验用户覆盖层（默认键可信不校验），命中保留键 → `console.warn` + 回退默认键；`null` → 解绑。`src/features/shortcuts/CLAUDE.md`「校验 + 静默降级」节：保护 SIGINT 是核心动机，运行期永不因坏配置崩溃。机制本身是通用终端语义保护，但保护对象正是 claude 的中断通道。

### 4. CM 内部键保留 + Ctrl+F 排除（完全通用）

`reserved.ts:23-30` 的 `EDITOR_RESERVED = { Ctrl+KeyF(搜索), Ctrl+KeyZ(撤销), Ctrl+KeyY(重做), Ctrl+Shift+KeyZ(重做), Tab(缩进), Shift+Tab(反缩进) }`——归 CodeMirror 自身 keymap 管理，本期不纳入自定义（文件头注释）。配套：`lib.rs:59-61` prevent-default 插件配置 `Flags::all().difference(Flags::FIND)`——拦截全 flags 减 FIND：Flags 共 10 项（9 键盘类 + 1 CONTEXT_MENU pointer），拦截 9 项 = 8 项键盘 + CONTEXT_MENU，显式排除 Ctrl+F，让搜索落到 CM。与 claude 无关联。

### 5. terminal.newline：Ctrl+Enter 换行不提交（硬编码 claude）

`commandCatalog.ts:52-58` 注册 `terminal.newline`（defaultKey `Ctrl+Enter`），handler 在 `keyboard.ts:40-46`：经 `getActiveTerminal()` 取聚焦终端 → `writeToPty(new Uint8Array([0x0a]))` 写 `\n` 到 PTY。代码注释（`keyboard.ts:41`）明写动机："Ctrl+Enter → 写 \n（0x0a）到 PTY（Ctrl+J 等价，**Ink 据此插入换行不提交**）"；`src/panels/CLAUDE.md`「attachCustomKeyEventHandler」节同步记载。该命令为 Claude Code Ink 渲染器的"多行输入换行不提交"行为设计——普通 shell 无此区分。

### 6. attachCustomKeyEventHandler 委托式 fallback（通用机制但 claude 触发）

`useXterm.ts:229-237`：`term.open()` 后注册自定义 key handler，keydown 委托 `getShortcutRegistry().resolve(event, "terminal")`（forceContext 兜底，不依赖焦点栈）；resolve 命中 → `preventDefault()` + return false（不交给 xterm）；未命中 → return true 透传（panels/CLAUDE.md 记载：Ctrl+C 等控制字符发往 PTY——`useXterm.ts:236` 实际代码无此注释，注释位于 `src/panels/CLAUDE.md`「attachCustomKeyEventHandler — 委托式 fallback」节代码示例内）。动机（同上节）：xterm.js 6.1.0-beta 升级后 window capture 路径在真实 WebView2 可能因 focusin 未冒泡而失效——双保险确保 `Ctrl+Shift+C/V`、`Ctrl+Enter` 可用且 Ctrl+C 透传不丢。无双触发：capture 命中即 stopPropagation，委托层不触发。

### 7. Kitty 键盘协议（CSI u）被动启用（通用机制但 claude 触发）

`theme.ts:20-22`：`vtExtensions: { kittyKeyboard: true }`，注释明写"允许子进程（**如 Claude Code**）通过 CSI>1u 激活 Disambiguate 模式，使 xterm.js 被动编码 Ctrl+Enter、Shift+Enter 等修饰键组合为独立的 CSI u 序列"。`src/panels/CLAUDE.md`「Kitty 键盘协议」节：协议为被动模式——终端声明能力后应用需主动 push flags，未激活时 `KeyboardService.useKitty` 返回 false 回退传统 handler。分类说明：本项为一行通用终端能力声明，对任何主动发送 `CSI>1u` 的子进程生效（行为无 claude 专属代码路径），归「通用机制但 claude 触发」（激活者是 claude；与 01-19 同机制条目归类一致）。

### 8. WebView2 三层按键控制（通用机制但 claude 触发）

`src/features/shortcuts/CLAUDE.md`「WebView2 三层按键控制」节分层：

| 层级 | 机制 | 示例 |
|------|------|------|
| 1. WebView2 硬编码 | 运行时级别 | Ctrl+W（窗口关闭被禁用，事件可穿透 DOM——WebView2 平台行为，仓库内无法直接验证，外部平台假设；佐证：shortcuts/CLAUDE.md 记载 + 05-11 详述节依赖说明） |
| 2. `tauri-plugin-prevent-default` | 浏览器加速键拦截 | （示例，与正文拦截集明细同批键位）F5/Ctrl+F5/Shift+F5/Ctrl+R/Ctrl+Shift+R、Ctrl+P/Ctrl+Shift+P、F7、Ctrl+Shift+I、Ctrl+J、Shift+Tab、Ctrl+U、Ctrl+O、右键菜单（Ctrl+F 已排除；F12 经 `PlatformOptions::dev_tools(false)` 禁用，非 Flags 集） |
| 3. 应用 ShortcutRegistry | 前端 capture-phase keydown | Ctrl+Shift+C/V（terminal context）、Ctrl+W（global 恒生效）、Ctrl+S/Alt+Z（editor 聚焦时生效） |

后端配置在 `lib.rs:56-69`：`PreventDefaultBuilder` 全 flags 减 FIND + `PlatformOptions::new().browser_accelerator_keys(false).dev_tools(false).default_script_dialogs(true)`（`default_script_dialogs(true)` = 允许 WebView2 默认脚本对话框——JS `alert`/`confirm` 正常弹出，不拦截）。拦截集明细（`tauri-plugin-prevent-default` 5.0.0 `Flags` 共 10 项 = 9 键盘类 + 1 CONTEXT_MENU pointer，`Flags::all().difference(Flags::FIND)` 拦截 9 项 = 8 项键盘 + CONTEXT_MENU）：`FIND`（Ctrl+F/G/Shift+G/F3）排除、`CARET_BROWSING`（F7）、`DEV_TOOLS`（Ctrl+Shift+I）、`DOWNLOADS`（Ctrl+J）、`FOCUS_MOVE`（Shift+Tab）、`RELOAD`（F5/Ctrl+F5/Shift+F5/Ctrl+R/Ctrl+Shift+R）、`SOURCE`（Ctrl+U）、`OPEN`（Ctrl+O）、`PRINT`（Ctrl+P/Ctrl+Shift+P）、`CONTEXT_MENU`（右键菜单）；**F12 不在 Flags 集**——来自 `PlatformOptions::dev_tools(false)`（`SetAreDevToolsEnabled(false)`，与 Flags 的脚本级拦截是两处独立机制）。两点值得记录：①**Shift+Tab 双份拦截**——WebView2 层（FOCUS_MOVE flag）与 05-4 `EDITOR_RESERVED` 层各拦一次（**两层语义不同**：WebView2 层拦浏览器「焦点移动」默认行为、注册表层拦「用户覆盖绑定」，并非同一事件被拦两次）；②**CONTEXT_MENU flag 抑制 WebView2 默认右键菜单**，与 FileTree/Commit 等自绘 ContextMenu 配套（默认菜单不出现，自绘菜单独占）。这套分层保证 claude 工作流依赖的全局键不被浏览器/窗口层吞掉（Ctrl+W 关页签而非关窗口、Ctrl+Shift+C 复制而非浏览器命令）。

### 9. 快捷键框架（通用机制但 claude 触发）

`ShortcutRegistry.ts` 模块级单例：命令按 id 索引（行 41）、指纹索引 `Map<指纹, Command[]>` O(1) 查候选（行 202-218）、上下文栈 push/pop 带竞态防护（行 106-125，仅在栈顶匹配时弹出）、`findWinner`（行 224-249）：IME 守卫 → 上下文过滤 → priority DESC + 上下文优先排序，winner-take-all。命令目录 `commandCatalog.ts:26-99` 集中 9 条可重绑命令元数据（global.closeTab/terminal.copy/terminal.paste/terminal.newline/editor.save/editor.toggleWordWrap/explorer.delete/explorer.open/explorer.rename）。命令在 `App.tsx:165-173` 一次性注册，handler 经模块级 active 指针（`activeTerminal.ts`/`activeEditor.ts`/`activeExplorer.ts`——explorer 三键（delete/open/rename）同模式经 `getActiveExplorer()` 派发，`usePanelFocus.ts:24-63` focusin/focusout 驱动）派发到聚焦实例——`src/features/shortcuts/CLAUDE.md`「命令注册一次 + active 指针派发」节：解决多实例共享命令 id 的派发错误（聚焦 A 却作用于 B；任一实例卸载删除共享命令致其余失效）。claude 多会话多终端场景是**当前主要受益场景**（非激活条件——多终端/多实例是任何终端模拟器的通用使用形态，随 claude 解耦该机制保留）。

### 10. 用户自定义重绑定 + 校验静默降级（通用机制但 claude 触发）

`wireKeybindings.ts:20-27`：立即应用 + 订阅 store 变更重应用；`App.tsx:176-178` 接线 `useKeybindings` store。**时序修正**：`keybindings.loadFromDisk`（`App.tsx:48`）位于 async `init()` 首个 await（fontSize :41）**之后**的挂起段内——init() 自首个 await 即挂起，故该调用在同步 effect 执行时尚未完成，而命令注册（`App.tsx:165-173`）与 wireKeybindings（`:176-178`）为同步 effect 先执行——注册表首次构建时覆盖层**尚未加载**；覆盖最终生效的保障是 wireKeybindings 的「立即应用 + `store.subscribe(apply)` 重应用」（`wireKeybindings.ts:24-26` 注释「load 可能已完成」——load 完成后 store `set()` 触发订阅重应用）。覆盖层存 `~/.slterminal/settings.json` 的 `keybindings` 段（`{ commandId: "Ctrl+Alt+KeyC" | null }`，null=解绑）。降级语义：**effectiveKeystroke（`ShortcutRegistry.ts:181-199`）的保留键/非法降级细则见 #3——此处仅交叉引用**；补充：同 context 同键冲突 → warn + 运行期按 priority/栈选 winner（`src/features/shortcuts/CLAUDE.md`「校验 + 静默降级」节）。保留键校验（见 #3/#4）保证用户重绑无法绑走 claude 依赖的 Ctrl+C 中断通道。

### 11. global.closeTab（Ctrl+W 关页签）（完全通用）

`commandCatalog.ts:27-34`：`global.closeTab`，defaultKey `Ctrl+KeyW`，priority 10（global 档 0-99）。`globalCommands.ts:21-31`：handler 经 `getDockviewApi()`（App 注入 `() => window.__dockviewApi` 延迟求值）取活跃面板 → `api.close()`；无活跃面板 → 返回 false 透传（注释："xterm.js 可接收 \x17 用于 bash readline"）。依赖 WebView2 硬编码禁用 Ctrl+W 关窗（见 #8）才能作为页签关闭键使用。**分类说明**：本项为通用注册表命令（`commandCatalog.ts:27-34` 与 `globalCommands.ts:21-31` 无任何 claude 字样或行为假设，机制上等同 05-16 中的 editor/explorer 注册表命令）；Ctrl+W 由用户按下、作用于任意类型页签——触发点无 claude 行为参与，归「完全通用」（与 05-16 同构）。claude 会话页签为高频使用对象，但使用频率主张不改变触发点客观口径。

### 12. HTML iframe 键盘转发（通用机制但 claude 触发）

`src/panels/html/HtmlPanel.tsx` 的 `INJECTED_SCRIPT` 在 sandboxed iframe 内 `keydown` capture → `window.parent.postMessage({type:"slterm_key", fingerprint, ...}, "null")`；父窗口 `handleMessage` 实施 **origin/source 两层入站校验**（`e.origin === "null"`（opaque origin 序列化）+ `e.source === iframeRef.current.contentWindow`）+ type/fingerprint 比对（`HtmlPanel.tsx:97-104`，SEC-03）→ `ShortcutRegistry.exportContextBindings("global")`（`ShortcutRegistry.ts:151-159`）动态比对 → 命中则 `window.dispatchEvent(合成 KeyboardEvent)` → 注册表正常分发（如 `global.closeTab`）。**信任标记语义**：`__slterm_postMessage` 在合成事件时打上（`Object.defineProperty(event, TRUSTED_MARKER, {value:true})`，`HtmlPanel.tsx:118-119`），**不拦截任何入站消息**——仅预留供 ShortcutRegistry 识别 postMessage 重放来源（当前匹配逻辑未使用）。**口径说明**：入站校验实为 origin/source 两层 + 预留信任标记（SEC-03 三层校验口径中的第三层，00-summary 2.5 行同此口径）。`src/features/shortcuts/CLAUDE.md`「HTML iframe 全局键转发」节：iframe 内 keydown 不冒泡到父 window，无此桥则焦点进 iframe 后全局快捷键全部失效。机制完全通用，服务 claude 工作流中打开 HTML 预览（claude 生成的 HTML 输出）时快捷键可用性。

### 13. 关窗杀子进程（P1-19）（通用机制但 claude 触发）

`App.tsx:28` 定义 `SHUTDOWN_TIMEOUT_MS = 3000`；`App.tsx:102-162` 经 `src/ipc/window.ts:56-69` 的 `registerCloseHandler`（`onCloseRequested` → `event.preventDefault()` → 执行回调 → `finally { destroy() }`）编排关窗：① 遍历 `TerminalRegistry.getAll()` 全部 session 并发 `pty.kill(sessionId, panelId)`，`Promise.race([Promise.all(killPromises), 3s 超时])`；② flush 活跃页布局（`saveLayout` → `updatePageLayout`）；③ `cancelPendingSave()` 清四 store debounce 定时器 + `saveAllProjects()` 3s 超时；④ localStorage 存 lastPage。根 `CLAUDE.md` 需求编号索引登记 P1-19："窗口关闭前杀子进程——前端 registerCloseHandler（onCloseRequested → 遍历 TerminalRegistry pty.kill，SHUTDOWN_TIMEOUT_MS=3000）+ 后端 Job Object KILL_ON_JOB_CLOSE 兜底"。动机即窗口内长驻的 claude 会话（含其子进程树）必须在应用退出时清理。

### 14. Job Object KILL_ON_JOB_CLOSE 孤儿防护兜底（完全通用）

`spawn.rs:1324-1425`（`#[cfg(windows)]`）：`pty_spawn` 中 spawn 成功后（`spawn.rs:1030-1038`）取子进程 pid → `add_to_job_object(pid)`——`job_name`（行 1352-1354，格式 `slTerminal_pty_{pid}`）与 `job_limits`（行 1361-1368，1360 为 `#[cfg(windows)]` 属性行；锁死 `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`=0x2000）为 D2 抽取纯函数，**Win32 调用链五步**（`CreateJobObjectW` → `SetInformationJobObject` → `OpenProcess`（打开子进程句柄，`AssignProcessToJobObject` 入参来源，`PROCESS_SET_QUOTA | PROCESS_TERMINATE` 权限，行 1390,1411）→ `AssignProcessToJobObject` → `CloseHandle`（释放 process 句柄，行 1422））内联；分配失败 → 显式 `child.kill()` 不留孤儿（注释 BE-02）。`JobHandle`（行 813-818）HANDLE RAII：PTY 会话期间持有防 `KILL_ON_JOB_CLOSE` 过早触发，drop 时 `CloseHandle`；`state.rs:33` 存入 `PtySession.job_object`。`src-tauri/src/pty/CLAUDE.md`「Job Object 孤儿防护（Windows）」节：父进程崩溃/退出时 OS 自动杀所有子进程。兜底对象即 pwsh→claude 进程树（`CLAUDE.md` PASSTHROUGH_MODE 段实证进程树形态），与前端 P1-19 构成双保险。

### 15. 窗口焦点监听 + 任务栏闪烁回窗引导（通用机制但 claude 触发）

`App.tsx:181-191`：初始置 `window.__slterm_windowFocused = true`，`onFocusChanged`（`ipc/window.ts:13-23`）维护焦点标志（`src/features/notifications` 通知调度失焦门控消费）；聚焦恢复时 `requestUserAttention(null)` 停闪。`ipc/window.ts:31-36`：`requestUserAttention` 用 `UserAttentionType.Critical`（= FLASHW_TIMERNOFG 持续闪烁直到窗口获焦）。`src/features/notifications/CLAUDE.md`：toast 点击路由已放弃，**任务栏闪烁是唯一回窗引导通道**。触发链路为 claude hook-event 五类事件值映射到三类通知（`classifyEvent` 输出三类——permission = `PermissionRequest` ∪（`Notification` 且 `notificationType=permission_prompt`）、error = `StopFailure` ∪ `PostToolUseFailure`、done = `Stop`；输入侧匹配 5 个事件值，其余事件不触发）在窗口失焦时打扰用户。**激活者归属**：机制被 claude hook 通知激活（触发链路 = claude hook-event 五类事件值映射到三类通知），随 claude 解耦后失效（00-summary 第 10 行口径已列入激活者示例）。

### 16. Ctrl+Wheel 字体缩放 / 编辑器与 explorer 快捷键（完全通用）

`src/lib/useFontSizeWheel.ts`：Ctrl+Wheel 字体缩放共享 hook（终端 useXterm、编辑器 useCodeMirror、gitshow/diff 面板复用），范围 [8,32] 经 `stores/fontSize.ts` clamp + 2s debounce 持久化。`src/panels/editor/keyboard.ts:17-32`：`editor.save`（Ctrl+S，经 `getActiveEditor().save()` 派发）/`editor.toggleWordWrap`（Alt+Z）。`src/features/explorer/keyboard.ts`：`explorer.delete`（Delete）/`explorer.open`（Enter）/`explorer.rename`（F2）。均为标准编辑器/文件树能力，无 claude 关联。

### 17. CSP 放行 HTML 预览内联脚本（通用机制但 claude 触发）

`src-tauri/tauri.conf.json:24-25`：全局 CSP 含 `script-src 'self' 'unsafe-inline'` 且 `dangerousDisableAssetCspModification: ["script-src"]`——HTML 预览面板（`src/panels/html/HtmlPanel.tsx`）内联 `<script>` 与内联事件属性得以执行的全局前提。机制（`src/panels/CLAUDE.md`「HTML 面板内联脚本/事件执行（CSP 放行）」段）：①`<iframe srcDoc>` 加载的 `about:srcdoc` 是 local scheme，按 CSP3 规范**继承父窗口 CSP**，子文档无法自注入宽松 meta 绕过（子策略只能收紧不能放宽）→ 必须放宽主窗口 CSP；②只加 `'unsafe-inline'` 不够——Tauri 注入的 nonce 会使 `'unsafe-inline'` 被浏览器忽略，故须 `dangerousDisableAssetCspModification: ["script-src"]` 关掉 script-src 的 nonce 注入；③安全权衡：代价是主应用全局失去 script 的 nonce 加固（`default-src 'self'` 仍拦远程脚本加载，`'unsafe-inline'` 只放行内联；无已知注入路径——xterm 只解释 ANSI、CodeMirror 纯文本、React 全程转义），有 L2 `csp-config.test.ts` 守卫配置不变量，真实执行由 L4 E2E 验收。与 05-12（iframe 键盘转发桥）同源——均为 HTML 预览特性，服务 claude 工作流中预览 claude 生成的 HTML 输出时内联脚本/快捷键可用。
