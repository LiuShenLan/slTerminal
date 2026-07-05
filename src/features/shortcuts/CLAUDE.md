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

## 添加新快捷键

1. 在面板 hook 中通过 `useMemo` 构造 `ShortcutCommand[]`（稳定引用）
2. 调用 `useShortcutContext("panelType", commands, domElement)`
3. `handler` 返回 `true` 阻止默认行为，返回 `false` 透传
4. `context` 设为面板类型标识符（与 `PANEL_TYPES` 保持一致）

## 全局快捷键

全局快捷键（`context: "global"`）不依赖焦点上下文，在任何面板聚焦时都匹配。在 `App.tsx` 挂载时通过 `getShortcutRegistry().register(createGlobalShortcuts(...))` 注册一次，应用生命周期内有效。

优先级 0-99（低于面板级 100-199），面板可为同按键注册更高优先级的命令覆盖全局行为。

## 关键约束

- **前端不碰 OS**：命令 handler 中所有系统操作（剪贴板等）经 `src/ipc/` 层调用
- **IME 透传**：默认所有命令在 `isComposing` 时不匹配。需在 IME 组合态触发的命令设置 `allowDuringComposition: true`
- **Ctrl+C 不注册命令**：终端 Ctrl+C 通过不注册命令实现透传——xterm.js 自然发送 `\x03` 到 PTY
