# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 职责

快捷键模块——全局键盘事件管理、命令注册表、上下文感知匹配。面板通过此模块注册快捷键，无需自行管理 `window` 事件。

## 架构决策

### 命令模式 + 注册表单例

每个快捷键对应一个 `ShortcutCommand`（命令模式）。`ShortcutRegistry` 为模块级单例，维护全局 capture-phase `keydown` 监听器。面板通过 `useShortcutContext` hook 注册命令并声明焦点上下文。

### 上下文栈（非单值）

`contextStack: ShortcutContext[]` 记录焦点上下文栈。`pushContext` 在面板 `focusin` 时追加到栈尾，`popContext` 在 `focusout` 时弹出——但**仅在栈顶匹配时弹出**（防竞态：A blur → B focus 时 A 的 blur 不清 B 的上下文）。

### 指纹索引

命令按 `"Ctrl+Shift+KeyC"` 格式的指纹存入 `Map<string, ShortcutCommand[]>`。`keydown` 时按指纹 O(1) 查找候选命令，无需遍历全部。

### 引用计数

`refCount` 始终等于 `commands.size`。归零时移除全局监听器，避免无注册命令时的空转开销。

## 文件

| 文件 | 职责 |
|------|------|
| `types.ts` | 核心类型：`KeyStroke`、`ShortcutCommand`、`ShortcutContext`、`ShortcutRegistryAPI` |
| `ShortcutRegistry.ts` | 注册表单例：命令注册/注销、指纹索引、上下文栈、keydown 匹配、引用计数 |
| `useShortcutContext.ts` | React hook：注册命令 + focusin/focusout → pushContext/popContext + 卸载清理 |
| `globalCommands.ts` | 全局快捷键工厂：`createGlobalShortcuts`，注册 context:"global" 的命令（如 Ctrl+W 关闭页签） |
| `index.ts` | barrel export |

## 优先级约定

| 范围 | 用途 |
|------|------|
| 0-99 | 全局/fallback（`context: "global"`） |
| 100-199 | 面板级（terminal、editor） |
| 200+ | 覆盖级（调试/测试） |

## 扩展指南

### 面板级快捷键

终端 `Ctrl+Shift+C/V` 为典型模式——三步集成：

```typescript
// 1. 在面板 hook 中用 useMemo 构造命令（稳定引用）
const cmds = useMemo(() => createTerminalShortcuts(() => terminalRef.current), []);

// 2. 调用 useShortcutContext — 自动处理注册 + focusin→pushContext + focusout→popContext + 卸载清理
useShortcutContext("terminal", cmds, container);

// 3. handler 通过 getter 访问 Terminal ref，避免闭包捕获已销毁实例
//    createTerminalShortcuts(getTerm) 内部：handler 调 getTerm()?.getSelection()
```

**关键约束**：
- `handler` 返回 `true` → `preventDefault()` + `stopPropagation()`（阻止 xterm.js 看到此按键）
- `handler` 返回 `false` → 事件透传（自然冒泡到 xterm.js / CodeMirror 内置 handler）
- `context` 设为面板类型标识符（与 `PANEL_TYPES` 保持一致）
- handler 中的面板实例（Terminal / EditorView）必须通过 ref/getter 访问——**不能闭包捕获**，否则面板销毁后 handler 仍持有已 dispose 的实例

### 全局快捷键

不依赖焦点上下文的快捷键（如 `Ctrl+W` 关闭页签）使用 `context: "global"`：

```typescript
// globalCommands.ts
export function createGlobalShortcuts(getApi: () => DockviewApi | undefined): ShortcutCommand[] {
  return [{
    id: "global.closeTab",
    keystroke: { ctrlKey: true, shiftKey: false, altKey: false, metaKey: false, code: "KeyW" },
    context: "global",
    priority: 10,  // 低于面板级，面板可覆盖
    handler: () => {
      const api = getApi();
      api?.activePanel?.api.close();
      return !!api?.activePanel;  // 有关闭目标→阻止默认，无目标→透传
    },
  }];
}

// App.tsx — useEffect 中注册一次
useEffect(() => {
  const registry = getShortcutRegistry();
  return registry.register(createGlobalShortcuts(() => window.__dockviewApi));
}, []);
```

优先级 0-99（低于面板级 100-199），面板可为同按键注册更高优先级的命令覆盖全局行为。

### 编辑器快捷键迁移（CodeMirror → ShortcutRegistry）

当前编辑器 `Ctrl+S` 通过 CodeMirror `keymap.of([{ key: "Mod-s", ... }])` 处理。迁移到 ShortcutRegistry 的方法：

```typescript
// useCodeMirror.ts 中替换 CodeMirror keymap
const editorShortcuts = useMemo(() => [
  {
    id: "editor.save",
    keystroke: { ctrlKey: true, shiftKey: false, altKey: false, metaKey: false, code: "KeyS" },
    context: "editor",
    priority: 100,
    handler: () => { /* 保存逻辑 */ return true; },
  },
], [/* deps */]);
useShortcutContext("editor", editorShortcuts, viewRef.current?.dom ?? null);
```

**与 CodeMirror keymap 的关系**：CodeMirror `keymap` 在 bubbling 阶段执行（EditorView 内部监听）。ShortcutRegistry handler 返回 `false` 时事件继续冒泡到 CodeMirror 的 keymap——因此可以"部分覆盖"（如接管 `Ctrl+S` 但不影响 `Ctrl+F` 由 `@codemirror/search` 处理）。

## 注意事项

### WebView2 三层按键控制

| 层级 | 机制 | 示例 |
|------|------|------|
| 1. WebView2 硬编码 | 运行时级别，始终关闭 | `Ctrl+W`（关闭 WebView2 窗口被禁用，但事件可穿透到 DOM） |
| 2. `tauri-plugin-prevent-default` | 浏览器加速键拦截 | `Ctrl+P`、`Ctrl+R`、F12（`Ctrl+F` 已排除） |
| 3. 应用 ShortcutRegistry | 前端 capture-phase keydown | `Ctrl+Shift+C/V`、`Ctrl+W` 自定义快捷键 |

> `tauri-plugin-prevent-default` Flags 枚举不含 `CLOSE`/`COPY`/`PASTE`。`Ctrl+W` 的窗口关闭阻止由 WebView2 运行时硬编码实现，非该插件。

### Ctrl+C 保留为中断

终端 `Ctrl+C` 通过**不注册命令**实现——`createTerminalShortcuts` 不返回 `Ctrl+C` 对应的 ShortcutCommand，ShortcutRegistry 无匹配即透传，xterm.js 自然发送 `\x03` 到 PTY。

**勿在面板级或全局注册 `Ctrl+C` 命令**——这会阻止 SIGINT 传递，导致 claude 无法取消操作。

### StrictMode 安全

- 同一 `id` 重复注册为幂等覆盖，`refCount` 不变
- 注销不存在的 `id` 不抛异常（幂等）
- `_reset()` 清空全部状态（仅测试用）

### handler 防悬空引用

面板实例（Terminal / EditorView）在 handler 中通过 getter/ref 访问，不能闭包捕获：

```typescript
// ✅ 正确：handler 调用时通过 ref 获取当前实例
const getTerm = () => terminalRef.current;
handler: () => { const term = getTerm(); if (term) term.getSelection(); }

// ❌ 错误：闭包捕获的 term 在面板销毁后仍持有引用
const term = terminalRef.current;
handler: () => { term.getSelection(); }
```

## 测试覆盖

### 测试文件

| 文件 | 用例数 | 覆盖范围 |
|------|--------|----------|
| `src/__tests__/shortcuts.test.ts` | 26 | ShortcutRegistry 注册/注销、引用计数、上下文栈竞态、KeyStroke 严格匹配、匹配排序、IME 透传、global 上下文、handler 返回值 |
| `src/__tests__/globalCommands.test.ts` | 12 | createGlobalShortcuts 命令结构、Ctrl+W 关闭活跃面板、无面板透传、getDockviewApi 延迟求值 |
| `src/__tests__/keyboard.test.ts` | 终端 handler | createTerminalShortcuts handler 直接测试：复制/粘贴/Ctrl+C 不注册/null 安全 |
| `src/__tests__/useXterm.test.ts` | hook 集成 | useShortcutContext 调用验证（context + commands + element 参数） |

### 测试模式

```typescript
// 注册表测试：_reset() 隔离
import { getShortcutRegistry } from "../features/shortcuts/ShortcutRegistry";
const registry = getShortcutRegistry();
afterEach(() => { registry._reset(); });

// 命令测试：直接调用 handler
const cmds = createGlobalShortcuts(() => mockApi);
cmds[0].handler(new KeyboardEvent("keydown", { ctrlKey: true, code: "KeyW" }));

// 集成测试：vi.hoisted + vi.mock 隔离
const { mockRegister } = vi.hoisted(() => ({ mockRegister: vi.fn(() => vi.fn()) }));
vi.mock("../features/shortcuts", () => ({ ... }));
```

### 运行测试

```bash
npm test                          # 全量 635 条
npx vitest run shortcuts          # 仅快捷键相关
npx vitest run globalCommands     # 仅全局快捷键
```
