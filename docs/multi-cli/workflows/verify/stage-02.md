# Stage 02 逐项验证断言（唯一真值源）

> stage-02 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。
> 计数类取数口径：「生产代码」= `src/` 排除 `src/__tests__/`；grep 计数断言均按此口径，测试目录引用不计。

## 断言清单

- **S02-01**（MC-401/413）：Glob 断言 `src/lib/agentStatus.ts` 存在、`src/lib/claudeStatus.ts` 与 `src/features/agentStatus/consts.ts` 不存在；`src/lib/index.ts` 导出名 agentStatus（grep `claudeStatus` 于 src/ 零命中）
- **S02-02**（MC-401）：grep `src/lib/` 无 claude 事件名字面量（`SessionStart|SessionEnd|UserPromptSubmit|PreToolUse|PostToolUse|PostToolUseFailure|StopFailure|PermissionRequest` 作为字符串字面量零命中）；lib 层保留 `AgentStatus` 类型（四态值集）/ `STATUS_EMOJI` / `getStatusIcon`（Read 确认）
- **S02-03**（MC-402/413）：grep 全仓零残留（`src/` + `test/` + `e2e-tests/` 含测试）：`claudeSession`、`setClaudeSession`、`ClaudeSessionInfo`、`CLAUDE_CONTEXT_LIMIT`（四词各自 grep 零命中）
- **S02-04**（MC-402）：`src/panels/terminal/TerminalRegistry.ts` 的 `AgentSessionInfo` 含 `cliId: string` 字段（Read 确认）；setAgentSession merge 语义 / null 清空 / undefined 不覆盖 / lastEventAt 自动填 / register 幂等用例全绿（依 npm test 结果）
- **S02-05**（MC-403/422）：useXterm 事件路径 L2 用例——测试/mock profile 的 `eventToStatus` 被真实调用（入参断言）；无 hooks 能力 profile → `console.warn` + 跳过；SessionEnd 清图标、Exit 清会话用例保留（依 npm test 结果 + Read 确认分支保留）；`payload` 空串归一 `|| undefined` 防御保留（Read 确认）；本 Stage 订阅函数名仍为 `onHookEvent`（grep useXterm.ts 确认——Stage 03 才更名）
- **S02-06**（MC-205/206）：三级解析 L2 三分支用例（显式——经 HookEventPayload 可选字段 `cliId?: string` 注入 / 反查 agentSession / 缺省 CLAUDE_CLI_ID）全绿（依 npm test 结果）；`src/types/hooks.ts` 的 `HookEventPayload` 含可选 `cliId?: string`（Read 确认）；未知 cliId → `console.warn` + 跳过用例存在且绿
- **S02-07**（MC-411/412）：`AgentStatusRow.tsx` 行 logo 按 `row.cliId` 查 profile.iconSrc（语义式：Read 确认 iconSrc 来源 = `cliProfileRegistry.get(row.cliId)`，非固定常量）；用量 `contextLimit` 来自 `profile.capabilities?.hooks?.contextLimit`，缺失显示 `--`（Read 确认）；生产代码 `get(CLAUDE_CLI_ID)` grep 计数 ≤ 1（仅 `HistorySessionRow.tsx`——AgentStatusRow 过渡形态已清扫）
- **S02-08**（MC-414）：空态文案「无运行中的编码 CLI 会话」grep 命中 `AgentStatusView.tsx` 与 `e2e-tests/agent.e2e.ts`；E2E 红线逐字保留（grep 命中：`data-e2e="agent-status-view"`、`agent-status-row`、标题栏 `"AGENT STATUS"`、「选择一个项目」于 AgentStatusView.tsx）
- **S02-09**（MC-404/405）：`CATEGORY_EMOJI`（通知模块）与 `STATUS_EMOJI`（lib/agentStatus.ts）注释互相指引（Read 两处确认互引注释）；F8 禁用判定 = `TerminalRegistry.get(panelId)?.agentSession != null`（Read `PageDockviewHost.tsx` 确认）；workspace-header-actions 测试绿（依 npm test）
- **S02-10**（MC-410/420）：useAgentStatus hook 事件通道建行写入行 cliId（三级解析，Read 确认）；useAgentNotifications 类别判定委托 `profile.capabilities?.hooks?.classifyNotification(payload)`、无 hooks 能力不通知（Read 确认 + 依 npm test 用例）；profiles/claude/ 含 eventToStatus（10 事件 + ATTENTION_NOTIFICATION_TYPES）与 classifyNotification 五映射实现（Read 确认）且用例落点 cli-profile-claude.test.ts 全绿（依 npm test）
- **S02-11**（test-inventory）：`.claude/test-inventory.md` 已就近登记本 Stage 变动（claude-status 删除/迁入、agent-status-lib 新增、更名同步条目，grep 确认）

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`（L1）
6. `npm test`（L2）
7. `npm run test:l3`（L3）
8. `npm run e2e`（L4——agent.e2e 空态文案断言在此层验证；最后单独串行执行，禁与其他命令并行）
