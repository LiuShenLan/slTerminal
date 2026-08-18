# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 模块职责

DTO 类型定义层（硬约束 #4）——`src/types/` ↔ Rust 模块 DTO 一一对应；Rust `snake_case` ↔ JS `camelCase`，改一边必须改另一边。本目录只放类型定义（`export type`），不含逻辑。

## 架构决策

### 双边对应契约（硬约束 #4）

| 文件 | 对应 Rust 模块 | 关键类型 |
|------|----------------|---------|
| `pty.ts` | `pty/spawn.rs`（PtyEvent + 请求 DTO） | `PtyEvent`（Output/Exit 带 tag serde 枚举）、`SpawnRequest` |
| `fs.ts` | `fs/mod.rs` | `DirEntry` |
| `git.ts` | `git/mod.rs` | `GitStatusEntry`（含 `oldPath: string \| null`）、`DiffHunk`（oldStart/oldLines/newStart/newLines） |
| `notify.ts` | `notify/mod.rs` | `FsEventPayload` |
| `agent.ts` | `hooks/`（MC-212 决策 3 更名，原 `hooks.ts`） | `AgentEventPayload`（十字段 + 可选 `cliId`/`usageSourcePath`/`usedPercentage`——ContextUsage 信号字段，旧信号缺省 undefined）、`ContextUsageSignal`（`{ usedPercentage }`——官方 used_percentage 口径，原 `ContextUsage` transcript 四字段已退役）、`AgentInjectionStatus`、`AgentHookInjectionStatus` |
| `hooksConfig.ts` | `hooks/config.rs` | `HooksLayer`（**FE-14 收窄：`"user" \| "project" \| "local"`**——当前仅 claude 三层，未来 CLI 加层再泛化；值集声明于 `profile.capabilities.hooks.configLayers`（KZ-4），后端 parse_layer 只认这三值，TS 联合类型编译期防误传）、`HooksConfigJson`、`MatcherGroupJson`、`HookHandlerJson`、GUI 模型（`HooksConfigGui`/`HookEventGroup`/`HookMatcherGroup`/`HookHandlerGui`） |
| `agentHistory.ts` | `agent_history/mod.rs` | `AgentHistorySession`（**八字段含 `cliId`**，provider 打标）、`AgentHistoryTitle`（**两字段** `title`/`titleSource`——`agent_history_read_title` 返回，运行中会话页签/导航树行标题通道，人工验证问题 3）、`TitleSource`（**开放字符串**——claude 值集 customTitle/aiTitle/summary/firstPrompt/none，UI 不消费具体值） |
| `index.ts` | — | barrel export，统一对外暴露 |

### 命名与序列化

- 类型名/字段名 camelCase（JS 侧），Tauri invoke 序列化时自动转换 `snake_case` ↔ `camelCase`（字段名），命令名保持 snake_case（见 `src/ipc/` 封装）
- `Uint8Array` 不入 DTO——PTY 写数据在 `ipc/pty.ts` 转 `number[]` 后传参
- 空串归一 `|| undefined` 等防御性约定在消费方（`useXterm.ts`），不在类型层

### 修改注意事项

改 Rust DTO 字段（增/删/改名/类型）必须同步：本目录对应文件 → `src/ipc/` 对应 wrapper → 模块 CLAUDE.md → 模块 serde 测试 + `src/__tests__/ipc-*-contract.test.ts`。

## 文件

| 文件 | 职责 |
|------|------|
| 上述 7 个 DTO 文件 + `index.ts` | 见上表 |

## 测试模式

无独立测试文件——类型正确性由两侧守护：Rust 侧 serde camelCase 测试（各模块 `#[cfg(test)]`）+ 前端 `src/__tests__/ipc-*-contract.test.ts` 契约测试（mockIPC 守 JS 侧形状；camelCase 真实转换由 L4 E2E 守卫，见 `src/ipc/CLAUDE.md` mockIPC 盲区声明）。
