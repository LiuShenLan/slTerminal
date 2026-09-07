# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 存在理由

DTO 类型定义层（硬约束 #4，CP-024 单源化）。`src/types/` 的 9 个域文件由 ts-rs 从 Rust `#[derive(TS)]` 生成（`src-tauri/` 侧各 DTO 模块），**禁手改**。本目录只放类型定义，不含逻辑；残面（Rust 无对应或组合别名的前端形态）收容于 `local.ts` / `hooksConfigGui.ts`。

## 单源化（ts-rs）契约

**Rust derive 为唯一真源**：改 DTO = 改 Rust（`src-tauri/src/*` 的 `#[ts(...)]` 属性与字段）→ 跑导出测试刷新 → git 漂移守卫确认。

### 生成/刷新操作指令

1. `cargo test --test lib_tests export_bindings -- --test-threads=1`（TQ-COV-06 形态；ts-rs 为每个 `#[ts(export)]` 类型自动生成 `export_bindings_<类型>` 单测，测试体内执行写盘，**必须串行**——同文件多类型合并导出依赖进程内 EXPORT_PATHS 锁表）
2. 提交前守卫：`cargo test --test lib_tests export_bindings -- --test-threads=1 && git diff --exit-code -- src/types`——漂移即红（生成物与磁盘不一致 = 红）

### 生成关系对照（类型 → 域文件）

| Rust 类型（`#[ts(export_to)]`） | 生成文件 | 备注 |
|---|---|---|
| `pty/spawn.rs` PtyEvent / SpawnRequest、`pty/conpty_api.rs` ConptyStatus | `pty.ts` | ConptyStatus 为 CP-010 DTO（CP-024 清单外追加——手写 pty.ts 含之，生成物覆盖后消费面经 ipc/pty.ts 依赖） |
| `fs/mod.rs` DirEntry | `fs.ts` | |
| `git/mod.rs` GitStatusEntry / DiffHunk | `git.ts` | |
| `notify/mod.rs` FsEventPayload | `notify.ts` | |
| `hooks/signal.rs` AgentEventPayload、`hooks/mod.rs` AgentInjectionStatus / AgentHookInjectionStatus | `agent.ts` | |
| `agent_history/mod.rs` AgentHistorySession / AgentHistoryTitle | `agentHistory.ts` | |
| `hooks/claude/config.rs` Layer(→`HooksLayer`) / MatcherGroup(→`MatcherGroupJson`) / HookHandler(→`HookHandlerJson`) | `hooksConfig.ts` | 类型级 `#[ts(rename)]` 对齐前端消费名 |
| `background_tasks/mod.rs` BackgroundTaskInfo | `backgroundTasks.ts` | |
| `plan_balance/mod.rs` PlanBalanceInfo / AmountInfo / WindowsInfo / WindowInfo | `planBalance.ts` | |

### ts-rs 坑（10.1 实测登记，改版本前先核对）

- **serde-compat 只解析五键**：`rename / skip / flatten / default / with`——`skip_serializing_if` **不识别**，含它的整条 serde 属性被忽略并告警。凡 serde-default 可选字段必须显式 `#[ts(optional)]`（`?: T`，不带 null）；需要 `?: T | null` 用 `#[ts(optional = nullable)]`。
- **flatten map 不可导出**：serde `flatten` 字段不能配 `#[ts(type)]`/`#[ts(optional)]` 且 map 类型 `decl`/`inline_flattened` 直接 panic——`HooksSubtree`（flatten BTreeMap）不导出，`HooksConfigJson` 组合别名落 `local.ts`。
- **u64 → TS `bigint`**：ts-rs 10 对 u64 输出 bigint；本项目契约 = JS number（值域 2^53 内），u64 字段一律补 `#[ts(type = "number")]`（Option 包裹时 type 文本需自带 `| null`，如 fs DirEntry）。
- **export_to 相对路径以运行期 `./bindings/` 为 out_dir**：目标 `src/types/<域>.ts` 需写 `../../src/types/<域>.ts`（比字面直觉多一级 `..` 抵消 bindings 前缀；测试运行目录 = crate 根）。
- 生成物为 `export type X = {...}` 形态（非 interface）——结构等价，消费方无感。

### 关键语义登记（字段语义收窄仍以此为准，字面量值集随 Rust 注释/枚举自证）

- `HooksLayer` 值集 `"user" | "project" | "local"`（FE-14），后端 `parse_layer` 只认这三值。
- `AgentEventPayload` 含可选 `cliId` / `usageSourcePath` / `usedPercentage`（ContextUsage 官方口径；`#[ts(optional)]`/`= nullable` 按手写面逐字段对齐）。
- `AgentInjectionStatus` 值集四态 `"injected" | "notInjected" | "outdated" | "pendingConfirmation"`（CP-043）；`AgentHookInjectionStatus.suspiciousCommand` 仅 pendingConfirmation 态存在（Rust `skip_serializing_if`，其余状态序列化缺键）。
- `AgentHistorySession.titleSource` / `AgentHistoryTitle.titleSource` 为开放字符串（`TitleSource` 别名在 local.ts）。
- `BackgroundTaskInfo` 六键无 default 字段；`BACKGROUND_TASK_IDS` 值集与后端 registry TASKS 键集双侧字面量测试锁死（`local.ts` 常量）。

### 残面清单（禁手改生成物；以下文件可手写）

| 文件 | 内容 | 理由 |
|---|---|---|
| `local.ts` | `TitleSource` 开放串别名；`HooksConfigJson = Record<string, MatcherGroupJson[]>` 组合别名（import 生成物）；`ContextUsageSignal` 前端窄视图；`BACKGROUND_TASK_IDS` / `BackgroundTaskId` / `PLAN_BALANCE_TASK_ID` / `SESSION_REFRESH_TASK_ID` 常量族 | 前端专有或组合别名——Rust 无对应可导出形态 |
| `hooksConfigGui.ts` | `HooksConfigGui` / `HookEventGroup` / `HookMatcherGroup` / `HookHandlerGui` | 前端 GUI 模型，非后端 DTO（configEditor 面板展示/编辑用；configEditor/configModel.ts 另有同构镜像） |

### 修改注意事项

改 Rust DTO 字段必须同步：Rust derive/`#[ts]` 属性 → 跑导出测试 → git diff 守卫 → `src/ipc/` 对应 wrapper（如需）→ 模块 CLAUDE.md（如需）→ `src/__tests__/ipc-*-contract.test.ts`（契约键集合零改动策略——expectExactKeys 守 JS 侧 payload 形状，与类型来源无关）。**禁止直接编辑 9 个域生成文件**。

## 测试模式

无独立测试文件。类型正确性由两侧守护：Rust 侧 derive 自动生成 `export_bindings_<类型>` 单测（内容与磁盘比对）+ serde 测试；前端 `src/__tests__/ipc-*-contract.test.ts` 契约测试（camelCase 真实转换由 L4 E2E 守卫，见 `src/ipc/CLAUDE.md` mockIPC 盲区声明）。
