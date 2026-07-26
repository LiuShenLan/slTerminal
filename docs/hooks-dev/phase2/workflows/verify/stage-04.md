# Stage 4 逐项验证断言

> stage-4 脚本与 fix-loop 的 verify agent 均以本文件为准。

## 断言清单

- **P2-TE-01**：存在 `src/__tests__/notifications.test.ts`；包含失焦/聚焦门控用例；包含三类事件触发用例；包含 `requestUserAttention(UserAttentionType.Critical)` 仅权限请求用例；包含 toast 点击路由用例；断言 toast 点击通过 `sendClickableNotification` 工厂绑定 `onclick`，不通过 `sendNotification({ onClick })`。
- **P2-TE-02**：存在 `src/__tests__/agent-status-view.test.tsx`；包含 no-root / empty / ready 三态用例；包含多行渲染用例；包含行点击调用 switchToPage + focus 用例；覆盖用量条降级与颜色 token 引用。
- **P2-TE-03**：存在 `src/__tests__/agent-status-hook.test.ts`；包含事件驱动插入/更新用例；包含 Stop 事件后状态为 `done` 且仍保留用例；包含 `SessionEnd`/exit 移除用例；包含项目过滤用例；包含排序用例；包含 contextUsage 事件驱动调用用例。
- **P2-TE-04**：L2 用量条降级测试存在（可在 agent-status-view.test.tsx 或独立文件中）；覆盖 contextUsage 返回 null 时显示 "--"；正常值时百分比按 `CLAUDE_CONTEXT_LIMIT` 计算正确；颜色 token 来自 `AGENT_STATUS_USAGE_COLORS`。
- **P2-TE-05**：L1 测试存在（`src-tauri/src/hooks/` 内或 `tests/hooks_context_usage_tests.rs`）；覆盖正常 JSONL / 无 usage / 损坏 / 空文件 / 大文件 tail 读取。
- **P2-TE-06**：`e2e-tests/test.e2e.ts` 追加阶段 2 describe 块；至少包含 Agent Status 视图存在性用例与 toast 链路骨架（可为 it.skip + 人工验证注释）。

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`

## 语义式断言

- L2 mock 必须在 `vi.hoisted()` 中创建（须 Read 测试文件确认 vi.hoisted 存在）。
- L1 测试不得依赖真实 claude transcript 文件路径；必须使用 tempfile 隔离（须 Read 测试代码确认）。
- 测试不得为通过而修改生产代码逻辑；生产代码修改必须由对应 Stage 完成（本 Stage 只新增/修改测试文件）。
- L4 用例若因系统通知中心不可控而无法自动化，必须以 `it.skip` 或注释明确标记为“人工验证点”，不得写必然 flaky 的断言（须 Read e2e 文件确认）。
- 测试中断言 Stop 移除的行为必须修正为 Stop → `done` 保留；SessionEnd/exit → 移除（须 Read 测试代码确认）。
