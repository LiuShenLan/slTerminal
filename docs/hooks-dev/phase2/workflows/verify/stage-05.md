# Stage 5 逐项验证断言

> stage-5 脚本与 fix-loop 的 verify agent 均以本文件为准。

## 断言清单

- **P2-DOC-01**：`src/ipc/CLAUDE.md` 模块映射表格含 `notification.ts` 条目；`hooks.ts` 条目含 `contextUsage` 命令说明；命令名与参数字段与代码一致。
- **P2-DOC-02**：`src-tauri/src/hooks/CLAUDE.md` 存在（若阶段 1 未创建则本 Stage 新建）；含 `hooks_context_usage` 命令说明、ContextUsage DTO 字段、尾部读取实现要点、测试位置。
- **P2-DOC-03**：`src/features/sideViews/CLAUDE.md` 扩展指南示例含 `agent-status`；DEFAULT_ZONES 描述含 agent-status；文件表格含 AgentStatusView 相关文件（若已知）。
- **P2-DOC-04**：`.claude/test-inventory.md` 追加阶段 2 测试文件条目（含 notifications / agent-status-view / agent-status-hook / hooks_context_usage L1 / E2E 追加）。
- **P2-DOC-05**：`docs/hooks-dev/contract.md` 的 C12 段回填完成；包含 `hooks_context_usage` 命令名、参数 `{ transcriptPath: string }`、返回 DTO `{ inputTokens, outputTokens } | null`、后端实现要点。

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`

## 语义式断言

- 文档中的命令名、DTO 字段、侧栏视图 id 必须与 checklist/stages/execution-plan 中的跨边界契约一致（逐字比对）。
- `src-tauri/src/hooks/CLAUDE.md` 若新建，不得包含根 CLAUDE.md 的全局约束，只写本模块实现细节（渐进式披露原则）。
- 文档修改不得遗漏代码 Stage 的最终中间态（如 agent-status 注册后 DEFAULT_ZONES 已变，文档必须同步）。
