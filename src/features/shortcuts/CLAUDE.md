# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 职责

快捷键模块——全局键盘事件管理、命令注册表、上下文感知匹配、**用户自定义重绑定**。面板通过此模块注册快捷键，无需自行管理 `window` 事件。

## 架构决策

### Command / Keybinding 分离 + 分层合并（核心设计）

为支持用户自定义重绑定，命令与键位解耦：

- **Command（`types.ts`）** = `CommandMeta`（`id/title/category/context/defaultKey/priority`）+ `handler`。代码定义、不可变。handler 随命令（高内聚）。
- **绑定表**由 `ShortcutRegistry` 运行期合并 **默认层（各命令 defaultKey）⊕ 用户覆盖层（overrides）** 得到，索引为 `Map<指纹, Command[]>`。键位在绑定层可换（低耦合）。
- **overrides**（`KeybindingOverrides = Record<commandId, keystrokeString | null>`）来自 `stores/keybindings.ts`，经 `App.tsx` 的 `wireKeybindings` 注入 `setOverrides()`；`null` = 解绑，缺省 = 用 defaultKey。

设计模式：命令模式 + 注册表单例 + 分层配置合并 + 观察者（overrides 变更→重建绑定表）。

### 命令目录单一真值源（`commandCatalog.ts`）

所有可重绑命令的 `title/category/defaultKey/priority` 集中在 `COMMAND_CATALOG`。各面板工厂经 `commandFromMeta(id, handler)` 合并 handler。新增可重绑命令 = 目录追加一条 + 工厂提供 handler。当前命令：`global.closeTab`、`terminal.copy`、`terminal.paste`、`terminal.newline`、`editor.save`。

### 命令注册一次 + active 指针派发到聚焦实例（多实例正确性）

面板命令**在 `App.tsx` 一次性注册**（`register([...global, ...terminal, ...editor])`），**不随面板实例增删**。命令 handler 不闭包捕获特定实例，而是经模块级 **active 指针**（`panels/terminal/activeTerminal.ts`、`panels/editor/activeEditor.ts`）派发到**当前聚焦**实例：

- 面板 `focusin` → `setActiveTerminal/Editor(自身 actions)`；`focusout`（离开子树）→ `clearActive*(自身 actions)`（仅在仍为 active 时清，防竞态）。
- 命令 handler：`const t = getActiveTerminal(); if (!t) return false; …`（无聚焦实例则透传）。

这从根本上解决**多实例共享命令 id** 的两个 bug：①"最后注册的 handler 赢" → 聚焦 A 却作用于 B；②任一实例卸载 `unregister` 删除共享命令 → 其余实例快捷键失效。因命令只注册一次、handler 按聚焦派发，两者皆不再发生。

### keystroke 字符串格式（`keystroke.ts`）

可读规范格式 `"Ctrl+Shift+KeyC"`（修饰键固定序 Ctrl→Shift→Alt→Meta，末接 `event.code`），与指纹格式一致。`formatKeystroke`/`parseKeystroke`/`isValidKeystrokeString`。`event.code` 不含 `+`，故 `split("+")` 安全。用于 settings.json 覆盖层与指纹索引。

### 上下文栈（非单值）

`contextStack: ShortcutContext[]` 记录焦点上下文栈。`pushContext` 在面板 `focusin` 时追加到栈尾，`popContext` 在 `focusout` 时弹出——但**仅在栈顶匹配时弹出**（防竞态：A blur → B focus 时 A 的 blur 不清 B 的上下文）。

### 匹配算法 `findWinner`（handleKeyDown 与 resolve 共用）

指纹 O(1) 查候选 → IME 守卫 + 上下文过滤（`context==="global" || (forceContext ? ctx===forceContext : contextStack.includes(ctx))`）→ priority DESC + 上下文优先排序 → 返回唯一 winner（winner-take-all，不级联）。

- `handleKeyDown`（window capture）：winner.handler 返回 true 才 `preventDefault()+stopPropagation()`。
- `resolve(event, forceContext?)`：供 xterm `attachCustomKeyEventHandler` 委托。强制 context 不依赖焦点栈；返回"是否消费"布尔，**不**调 preventDefault（无 window 事件可取消，调用方自行处理）。

### resolve 委托 + 无双触发

终端 `useXterm.ts` 的 `attachCustomKeyEventHandler` 委托进 `resolve(event, "terminal")`（单一真值源，可重绑）。正常路径：window capture 命中即 `stopPropagation`，事件到不了 xterm，委托层不触发。仅当 capture 路径因 xterm 6.1 focusin 未冒泡而失效时，委托层用 forceContext 兜底命中。透传命令（handler 返回 false）两路径都不产生副作用，安全。

### 校验 + 静默降级（`reserved.ts` + `effectiveKeystroke`）

构建绑定表时校验每条**用户覆盖**（默认键可信、不校验）：
- 绑到保留键（`isReserved`）或非法 keystroke → `console.warn` + 回退默认键。
- `null` → 解绑（不进索引）。
- 同 context 同键冲突 → rebuildIndex 时 `console.warn`，运行期由 priority/栈选 winner。
- 运行期永不因坏配置崩溃。

**保留键（不可绑）**：终端控制字符透传 `Ctrl+KeyC/V/X/Z/A`（保护 SIGINT）；CodeMirror 内部键 `Ctrl+KeyF/KeyZ/KeyY`、`Ctrl+Shift+KeyZ`、`Tab`、`Shift+Tab`。`global` context 须同时避开两套。

### 前向接口（未来扩展预留）

- `exportContextBindings(context)`：导出某 context 当前生效绑定（含 global，排除解绑），供将来 HTML iframe 转发脚本注入用。
- `listCommands()`：列出已注册命令元数据，供将来可视化设置 UI 枚举用。

### 指纹索引 + 引用计数

命令按 `"Ctrl+Shift+KeyC"` 格式指纹存入 `Map<string, Command[]>`，`keydown` O(1) 查候选。`refCount` = `commands.size`，归零时移除全局监听器。

## 文件

| 文件 | 职责 |
|------|------|
| `types.ts` | 核心类型：`KeyStroke`、`CommandMeta`、`Command`、`KeybindingOverrides`、`ExportedBinding`、`ShortcutContext`、`ShortcutRegistryAPI` |
| `ShortcutRegistry.ts` | 注册表单例：命令注册/注销、绑定表（默认⊕覆盖）、指纹索引、上下文栈、`findWinner`/`handleKeyDown`/`resolve`、`setOverrides`/`exportContextBindings`/`listCommands`、引用计数 |
| `keystroke.ts` | keystroke 字符串互转：`formatKeystroke`/`parseKeystroke`/`isValidKeystrokeString` |
| `reserved.ts` | 保留键判定 `isReserved(ks, context)`（终端控制字符 + CM 内部键两套集合） |
| `commandCatalog.ts` | 命令目录单一真值源：`COMMAND_CATALOG`/`COMMAND_META_BY_ID`/`commandFromMeta` |
| `usePanelFocus.ts` | React hook：focusin→pushContext+onActivate，focusout→popContext+onDeactivate，卸载清理。**只跟踪焦点上下文与聚焦实例，不每实例注册命令**（取代旧 useShortcutContext） |
| `wireKeybindings.ts` | 覆盖层→注册表接线纯 helper：`wireKeybindings(registry, store)`（立即应用 + 订阅重应用，返回 unsubscribe） |
| `forwardGlobalShortcuts.ts` | iframe 键盘桥：`attachGlobalShortcutForwarder(doc)` 给同源 iframe.contentDocument 挂 capture keydown，命中全局绑定则 preventDefault + 重放到父 window，返回 detach |
| `globalCommands.ts` | 全局快捷键工厂：`createGlobalShortcuts`（`global.closeTab`） |
| `index.ts` | barrel export |

## 优先级约定

| 范围 | 用途 |
|------|------|
| 0-99 | 全局/fallback（`context: "global"`） |
| 100-199 | 面板级（terminal、editor） |
| 200+ | 覆盖级（调试/测试） |

## 扩展指南

### 新增可重绑命令（三步）

1. `commandCatalog.ts` 的 `COMMAND_CATALOG` 追加一条元数据（含 `defaultKey`，须对自身 context 非保留）。
2. 对应面板工厂（`globalCommands.ts` / `panels/terminal/keyboard.ts` / `panels/editor/keyboard.ts`）用 `commandFromMeta(id, handler)` 提供 handler；handler 经 `getActive*()` 派发到聚焦实例。
3. 命令在 `App.tsx` 一次性注册；面板经 `usePanelFocus` 在聚焦时把自身设为 active。

### 面板级快捷键集成

```typescript
// 1. 面板 hook 中构造"本实例动作" + activate/deactivate（稳定引用）
const actions = useMemo(() => ({ getSelection: () => termRef.current?.getSelection(), … }), […]);
const activate = useCallback(() => setActiveTerminal(actions), [actions]);
const deactivate = useCallback(() => clearActiveTerminal(actions), [actions]);
// 2. usePanelFocus — focusin→pushContext+activate，focusout→popContext+deactivate，卸载清理
usePanelFocus("terminal", container, activate, deactivate);
// 3. 命令在 App.tsx 一次性注册（createTerminalShortcuts()），handler 经 getActiveTerminal() 派发
```

- `handler` 返回 `true` → `preventDefault()+stopPropagation()`；返回 `false` → 透传（含"无聚焦实例"）。
- `context` 与 `PANEL_TYPES` 保持一致。
- 面板实例经 active 指针访问，**不闭包捕获**——多实例下始终作用于聚焦实例。

### 编辑器已接入注册表

`editor.save`（Ctrl+S）在 `App.tsx` 注册（`createEditorShortcuts()`），handler 经 `getActiveEditor().save()` 派发。编辑器 `useCodeMirror` 经 `usePanelFocus("editor", container, …)` 在聚焦时 `setActiveEditor`。window capture 命中 → `stopPropagation` 屏蔽 CM keymap；`Ctrl+F`/撤销/重做未注册 → 冒泡回 CodeMirror。

### 全局快捷键

`context: "global"`，`App.tsx` 中 `registry.register([...createGlobalShortcuts(), ...createTerminalShortcuts(), ...createEditorShortcuts()])` 一次性注册；overrides 经 `wireKeybindings(getShortcutRegistry(), useKeybindings)` 持续同步。优先级 0-99，面板级可覆盖。

## 用户自定义重绑定

- 覆盖层存 `~/.slterminal/settings.json` 的 `keybindings` 段（`{ commandId: "Ctrl+Alt+KeyC" | null }`），由 `stores/keybindings.ts` 管理（sanitize + loaded 守卫 + debounce）。后端 `save_settings` 浅合并，不擦其他段。
- 本期仅数据模型 + 文件配置，用户手编 settings.json 即可重绑；可视化 UI 为后续 feature（`listCommands` 已预留枚举接口）。

## 注意事项

### WebView2 三层按键控制

| 层级 | 机制 | 示例 |
|------|------|------|
| 1. WebView2 硬编码 | 运行时级别 | `Ctrl+W`（窗口关闭被禁用，事件可穿透 DOM） |
| 2. `tauri-plugin-prevent-default` | 浏览器加速键拦截 | `Ctrl+P`、`Ctrl+R`、F12（`Ctrl+F` 已排除） |
| 3. 应用 ShortcutRegistry | 前端 capture-phase keydown | `Ctrl+Shift+C/V`、`Ctrl+W`、`Ctrl+S` |

### Ctrl+C 保留为中断

终端 `Ctrl+C` 通过**不注册命令**实现——目录中无 Ctrl+C，`resolve`/`handleKeyDown` 无匹配即透传，xterm.js 发送 `\x03` 到 PTY。且 `isReserved` 将 `Ctrl+KeyC` 在 terminal/global 标记为保留键，**用户覆盖也无法绑到它**。勿在任何地方注册 Ctrl+C 命令。

### StrictMode 安全

- 同一 `id` 重复注册幂等覆盖，`refCount` 不变。
- 注销不存在的 `id` 不抛异常。
- `_reset()` 清空全部状态（含 overrides，仅测试用）。

### handler 防悬空引用

命令 handler 经 `getActiveTerminal()/getActiveEditor()` 取聚焦实例动作，动作内部再经面板 ref 解引用——不闭包捕获特定实例：

```typescript
// 命令 handler（App 一次性注册）
handler: () => { const t = getActiveTerminal(); if (!t) return false; t.getSelection(); return true; }
// 面板提供的 actions（内部经 ref，实例销毁后 ref.current 为 null，安全）
const actions = { getSelection: () => terminalRef.current?.getSelection() };
```

## HTML iframe 全局键转发（已实现）

HTML 面板内容在 `<iframe sandbox="allow-scripts allow-same-origin" srcDoc>` 中，iframe 内 `keydown` 不冒泡到父 window，焦点进 iframe 后全局快捷键（如 Ctrl+W）本会失效。

因 `allow-same-origin` 与父同源，`forwardGlobalShortcuts.ts` 的 `attachGlobalShortcutForwarder(doc)` 给 `iframe.contentDocument` 挂 capture keydown 监听（运行在父上下文）：命中**全局 context 绑定**（动态 `exportContextBindings("global")`）→ `preventDefault` + 合成 keydown 重放到父 window → 注册表正常分发（`global.closeTab` → 关活跃面板）。`HtmlPanel` 在 iframe `onLoad` 时挂载、重载/卸载时 detach。仅重放命中全局绑定的键，页面其余键不受影响。

## 测试覆盖

| 文件 | 覆盖范围 |
|------|----------|
| `keystroke.test.ts` | formatKeystroke 各组合 + 固定序、parseKeystroke 合法往返/各类非法、isValidKeystrokeString、format∘parse 恒等 |
| `reserved.test.ts` | isReserved 各 context、每个保留键命中、非保留键放行、global 两集并集 |
| `commandCatalog.test.ts` | 5 命令齐全 + 元数据、id 唯一、defaultKey 合法且非自身保留、commandFromMeta 合并/抛错 |
| `shortcuts.test.ts` | 注册/注销、引用计数、上下文栈竞态、匹配排序、IME 透传、global、handler 返回值、**setOverrides 重绑/解绑/降级/冲突、resolve/forceContext、exportContextBindings、listCommands、_reset 清 overrides** |
| `wireKeybindings.test.ts` | 立即应用、store 变更重应用、unsubscribe |
| `forwardGlobalShortcuts.test.ts` | 命中全局键→preventDefault+重放(props 正确)、非全局键不转发、多绑定/空绑定、detach 后失效、修饰键指纹区分 |
| `usePanelFocus.test.ts` | focusin→pushContext+onActivate、focusout(离子树)→popContext+onDeactivate、内部焦点转移不触发、卸载清理 |
| `globalCommands.test.ts` | createGlobalShortcuts 命令结构（defaultKey/title/category）、Ctrl+W 关闭、无面板透传、延迟求值 |
| `keyboard.test.ts` | createTerminalShortcuts()（无参）copy/paste/newline 经 getActiveTerminal 派发、无 active 透传、Ctrl+C 不注册 |
| `editor-keyboard.test.ts` | createEditorShortcuts()（无参）save 经 getActiveEditor 派发、无 active 透传 |
| `activeTerminal.test.ts` / `activeEditor.test.ts` | active 指针 set/get/覆盖、clear 仅在匹配时生效 |

```bash
npm test                          # 全量
npx vitest run shortcuts          # 仅注册表
```
