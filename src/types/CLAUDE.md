# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 存在理由

DTO 类型定义层（硬约束 #4）。`src/types/` ↔ Rust 模块 DTO 一一对应；Rust `snake_case` ↔ JS `camelCase`，改一边必须改另一边。本目录只放类型定义，不含逻辑。

## 关键约束与决策

### 双边对应契约

- `pty.ts` ↔ `src-tauri/src/pty/spawn.rs`
- `fs.ts` ↔ `src-tauri/src/fs/mod.rs`
- `git.ts` ↔ `src-tauri/src/git/mod.rs`
- `notify.ts` ↔ `src-tauri/src/notify/mod.rs`
- `agent.ts` ↔ `src-tauri/src/hooks/`
- `hooksConfig.ts` ↔ `src-tauri/src/hooks/config.rs`
- `agentHistory.ts` ↔ `src-tauri/src/agent_history/mod.rs`

字段级映射见源文件对照；关键语义收窄：

- `HooksLayer` 值集 `"user" | "project" | "local"`（FE-14），后端 `parse_layer` 只认这三值。
- `AgentEventPayload` 含可选 `cliId` / `usageSourcePath` / `usedPercentage`（ContextUsage 官方口径）。
- `AgentHistorySession` 八字段含 `cliId`（provider 打标）。
- `AgentHistoryTitle` 两字段 `title` / `titleSource`，`TitleSource` 为开放字符串。

### 命名与序列化

- JS 侧 camelCase；Tauri invoke 自动字段名 `snake_case` ↔ `camelCase`，命令名保持 snake_case（由 `src/ipc/` 封装）。
- `Uint8Array` 不入 DTO——PTY 写数据在 `ipc/pty.ts` 转 `number[]` 后传参。
- 空串归一等防御性约定在消费方处理，不在类型层。

### 修改注意事项

改 Rust DTO 字段必须同步：本目录对应文件 → `src/ipc/` 对应 wrapper → 模块 CLAUDE.md → 模块 serde 测试 + `src/__tests__/ipc-*-contract.test.ts`。

## 测试模式

无独立测试文件。类型正确性由两侧守护：Rust 侧 serde camelCase 测试 + 前端 `src/__tests__/ipc-*-contract.test.ts` 契约测试（camelCase 真实转换由 L4 E2E 守卫，见 `src/ipc/CLAUDE.md` mockIPC 盲区声明）。
