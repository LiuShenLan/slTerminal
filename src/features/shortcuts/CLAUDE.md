# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 存在理由

快捷键模块负责全局键盘事件管理、命令注册表、上下文感知匹配与**用户自定义重绑定**。面板通过此模块注册快捷键，无需自行管理 `window` 事件；命令与键位解耦后，用户可在 settings.json 中覆盖默认绑定而不改动代码。

## 关键约束与决策

### Command / Keybinding 分离 + 分层合并

- **Command（`types.ts`）** = `CommandMeta`（`id/title/category/context/defaultKey/priority`）+ `handler`。代码定义、不可变。handler 随命令高内聚。
- **绑定表**由 `ShortcutRegistry` 运行期合并 **默认层（各命令 defaultKey）⊕ 用户覆盖层（overrides）**，索引为 `Map<指纹, Command[]>`。键位在绑定层可换。
- **overrides**（`Record<commandId, keystrokeString | null>`）来自 `stores/keybindings.ts`，经 `App.tsx` 的 `wireKeybindings` 注入 `setOverrides()`；`null` = 解绑，缺省 = 用 defaultKey。

### 命令目录单一真值源（`commandCatalog.ts`）

所有可重绑命令的 `title/category/defaultKey/priority` 集中在 `COMMAND_CATALOG`。各面板工厂经 `commandFromMeta(id, handler)` 合并 handler。新增可重绑命令 = 目录追加一条 + 工厂提供 handler。

### 命令注册一次 + active 指针派发到聚焦实例

面板命令**在 `App.tsx` 一次性注册**，**不随面板实例增删**。命令 handler 不闭包捕获特定实例，而是经模块级 **active 指针**（`activeTerminal` / `activeEditor` / `activeExplorer`）派发到**当前聚焦**实例：

- 面板 `focusin` → `setActive*(自身 actions)`；`focusout`（离开子树）→ `clearActive*(自身 actions)`（仅在仍为 active 时清，防竞态）。
- 命令 handler 无聚焦实例则返回 `false` 透传。

这解决多实例共享命令 id 的两个问题：① 最后注册的 handler 赢 → 聚焦 A 却作用于 B；② 任一实例卸载 `unregister` 删除共享命令 → 其余实例快捷键失效。

### keystroke 字符串格式（`keystroke.ts`）

可读规范格式 `"Ctrl+Shift+KeyC"`（修饰键固定序 Ctrl→Shift→Alt→Meta，末接 `event.code`），与指纹格式一致。`event.code` 不含 `+`，故 `split("+")` 安全。用于 settings.json 覆盖层与指纹索引。

### 上下文栈（非单值）

`contextStack: ShortcutContext[]` 记录焦点上下文栈。`pushContext` 在面板 `focusin` 时追加到栈尾，`popContext` 在 `focusout` 时弹出——但**仅在栈顶匹配时弹出**（防竞态：A blur → B focus 时 A 的 blur 不清 B 的上下文）。

### 匹配算法 `findWinner`

指纹 O(1) 查候选 → IME 守卫 + 上下文过滤（`context==="global" || (forceContext ? ctx===forceContext : contextStack.includes(ctx))`）→ priority DESC + 上下文优先排序 → 返回唯一 winner（winner-take-all，不级联）。

- `handleKeyDown`（window capture）：winner.handler 返回 true 才 `preventDefault()+stopPropagation()`。
- `resolve(event, forceContext?)`：供 xterm `attachCustomKeyEventHandler` 委托。强制 context 不依赖焦点栈；返回"是否消费"布尔，**不**调 preventDefault（调用方自行处理）。

### resolve 委托 + 无双触发

终端 `useXterm.ts` 的 `attachCustomKeyEventHandler` 委托进 `resolve(event, "terminal")`。正常路径：window capture 命中即 `stopPropagation`，事件到不了 xterm，委托层不触发。仅当 capture 路径因 xterm 6.1 focusin 未冒泡而失效时，委托层用 forceContext 兜底命中。透传命令（handler 返回 false）两路径都不产生副作用。

### 校验 + 静默降级（`reserved.ts` + `effectiveKeystroke`）

构建绑定表时校验每条**用户覆盖**（默认键可信、不校验）：

- 绑到保留键（`isReserved`）或非法 keystroke → `console.warn` + 回退默认键。
- `null` → 解绑（不进索引）。
- 同 context 同键冲突 → rebuildIndex 时 `console.warn`，运行期由 priority/栈选 winner。
- 运行期永不因坏配置崩溃。

**保留键**：终端控制字符透传 `Ctrl+KeyC/V/X/Z/A`（保护 SIGINT）；CodeMirror 内部键 `Ctrl+KeyF/KeyZ/KeyY`、`Ctrl+Shift+KeyZ`、`Tab`、`Shift+Tab`。`global` context 须同时避开两套。

### 前向接口

- `exportContextBindings(context)`：导出某 context 当前生效绑定（含 global，排除解绑），供 HtmlPanel postMessage 键盘转发动态比对全局快捷键用。
- `listCommands()`：列出已注册命令元数据，供可视化设置 UI（快捷键设置页 KeybindingsPage，F11）枚举用。
- `getEffectiveKeystroke(id)`（F11 登记）：**生效键查询**——设置页显示与运行期同源，防显示/运行漂移；`null` = 解绑或无默认键。含用户 overrides 语义（有键→合法用之/非法回退默认/null 解绑），设置页直接用它渲染每行当前生效键。
- `setCaptureSuspended(suspended)`（F11 登记）：**录制态屏蔽**——true 时 `handleKeyDown`/`resolve` 起始即不消费任何按键，快捷键设置页录制期间置位，防录制键触发命令（如录 Ctrl+Shift+C 真执行复制）；录制结束/取消/卸载必须复位（`_reset()` 亦清）。

### 指纹索引 + 引用计数

命令按 `"Ctrl+Shift+KeyC"` 格式指纹存入 `Map<string, Command[]>`，`keydown` O(1) 查候选。`refCount` = `commands.size`，归零时移除全局监听器。

### 优先级约定

| 范围 | 用途 |
|------|------|
| 0-99 | 全局/fallback（`context: "global"`） |
| 100-199 | 面板级（terminal、editor） |
| 200+ | 覆盖级（调试/测试） |

## 外部坑/红线

- **WebView2 三层按键控制**：
  - 运行时级硬编码：`Ctrl+W`（窗口关闭被禁用，事件可穿透 DOM）。
  - `tauri-plugin-prevent-default`：`Ctrl+P`、`Ctrl+R`、F12（`Ctrl+F` 已排除）。
  - 应用 ShortcutRegistry：前端 capture-phase keydown。
- **Ctrl+C 保留为中断**：终端 `Ctrl+C` 通过**不注册命令**实现；`isReserved` 将 `Ctrl+KeyC` 在 terminal/global 标记为保留键，**用户覆盖也无法绑到它**。勿在任何地方注册 Ctrl+C 命令。
- **handler 必须返回布尔**：返回 `true` → 消费并阻止默认/冒泡；返回 `false` → 透传。无聚焦实例必须返回 `false`。
- **禁止命令 handler 闭包捕获实例**：多实例下必须经 active 指针派发。
- **同一 `id` 重复注册幂等覆盖**，`refCount` 不变；注销不存在的 `id` 不抛异常；`_reset()` 仅测试用。

## 扩展指南

### 新增可重绑命令

1. `commandCatalog.ts` 的 `COMMAND_CATALOG` 追加元数据（含 `defaultKey`，须对自身 context 非保留）。
2. 对应面板工厂用 `commandFromMeta(id, handler)` 提供 handler；handler 经 `getActive*()` 派发到聚焦实例。
3. 命令在 `App.tsx` 一次性注册；面板经 `usePanelFocus` 在聚焦时把自身设为 active。

### 面板级快捷键集成

面板 hook 中构造"本实例动作" → `usePanelFocus("context", container, activate, deactivate)` → 命令在 App.tsx 一次性注册，handler 经 active 指针派发。`context` 与面板类型保持一致。

### 全局快捷键

`context: "global"`，在 `App.tsx` 中一次性注册；overrides 经 `wireKeybindings(getShortcutRegistry(), useKeybindings)` 持续同步。优先级 0-99，面板级可覆盖。

**「配置」钮 = 设置中心唯一入口（F11）**（原 `global.openHooksConfig` Ctrl+Shift+H 命令已删除；SidebarTree 右键菜单随其退役）：`openSettings()` → 先 `switchToPageShared` 切页 → `openSettingsPanel(pageId)`（同页单例语义继承 C13-7，面板 id `settings-` 前缀）。无项目 → toast「请先创建项目」。编排细节见 `features/settingsCenter/CLAUDE.md`。

### 用户自定义重绑定

覆盖层存 `~/.slterminal/settings.json` 的 `keybindings` 段（`{ commandId: "Ctrl+Alt+KeyC" | null }`），由 `stores/keybindings.ts` 管理（sanitize + loaded 守卫 + debounce）。后端 `save_settings` 浅合并，不擦其他段。

**可视化 UI 已落地（F11）**：快捷键设置页（`panels/settings/pages/KeybindingsPage`）——`listCommands()` 按 category 分组渲染，行显生效键（override 高亮 + ↺ 回默认 + 默认键小字；`getEffectiveKeystroke` null → 「未绑定」占位）；录制期间 `setCaptureSuspended(true)` 屏蔽全局派发，`isReserved` 拒绝保留键、`findConflict` 同 context 冲突警告放行写入。测试 `settings-keybindings.test.tsx` + `shortcuts.test.ts`（suspended 两例）。

## HTML iframe 全局键转发

HTML 面板内容在 `<iframe sandbox="allow-scripts" srcDoc={...}>` 中（不含 `allow-same-origin`），iframe 内 keydown 不冒泡到父 window。

**注入脚本 postMessage 路径**：`HtmlPanel.tsx` 注入脚本在 iframe 内 `keydown` capture → `window.parent.postMessage({type:"slterm_key", fingerprint, ...}, "null")`。父窗口 `handleMessage` 监听 `"message"`：校验 `e.origin === "null"` + `e.source === iframe.contentWindow` → `exportContextBindings("global")` 动态比对 → 命中则 `window.dispatchEvent(合成KeyboardEvent)`（附带 `__slterm_postMessage` 信任标记）→ ShortcutRegistry 正常分发。

旧 `forwardGlobalShortcuts.ts` 已删除：原方案需 `allow-same-origin` 才能访问 `iframe.contentDocument`，与 sandbox 安全策略冲突；postMessage 方案无需同源。

## 测试模式

- **核心**：注册/注销、引用计数、上下文栈竞态、匹配排序、IME 透传、setOverrides 重绑/解绑/降级/冲突、resolve/forceContext、exportContextBindings。
- **命令目录守卫**：默认键非保留、id 唯一、命令齐全。
- **usePanelFocus**：focusin→pushContext+onActivate、focusout（离子树）→popContext+onDeactivate、内部焦点转移不触发、卸载清理。
- 各面板 keyboard 测试测命令经 active 指针派发、无 active 透传、Ctrl+C 不注册。
