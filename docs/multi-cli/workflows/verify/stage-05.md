# Stage 05 逐项验证断言（唯一真值源）

> stage-05 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。
> 计数类取数口径：「生产代码」= `src/` 排除 `src/__tests__/`；grep 计数断言均按此口径，测试目录引用不计。

## 断言清单

- **S05-01**（MC-310）：Glob 断言 `src/features/agentHistory/` 9 文件存在（index.ts、AgentHistorySections.tsx、HistorySessionList.tsx、HistorySessionRow.tsx、SessionActionDialog.tsx、historyContextMenu.ts、historyModel.ts、useAgentHistory.ts、restoreSession.ts）；`src/features/claudeHistory/` 目录不存在；`src/__tests__/agent-history-{model,hook,view,restore,row,action-dialog}.test.ts(x)` 更名同步存在、`claude-history-*.test.ts(x)` 不存在
- **S05-02**（MC-310/D-05）：grep 零残留（`src/` 全仓含测试）：`claudeHistory`、`useClaudeHistory`、`ClaudeHistorySections`（三词各自零命中）；`AgentStatusView.tsx` import = agentHistory barrel（grep 命中）
- **S05-03**（MC-311）：`HistorySessionRow.tsx` 行 logo 来源 = 注册表查 `session.cliId` 的 profile.iconSrc（语义式：Read 确认取值链，非固定常量）；「logo 仅随 status emoji」与孤儿 ✗ 不加图保留（Read 确认）；未注册 cliId → 无 logo 不报错（Read 确认防御分支）；生产代码 `get(CLAUDE_CLI_ID)` 过渡形态清零（grep 计数 = 0）
- **S05-04**（MC-313/314）：复合键 `` `${cliId}|${sessionId}` `` 三处（语义式：Read 确认竖线拼接形态）——`historyModel.ts` deriveActiveSessionStatuses、`HistorySessionList.tsx` findPanelForSession、`AgentStatusView.tsx` titleBySessionId；transcriptPath basename 回退用例保留且绿（依 npm test）；回退路径缺省值经 `CLAUDE_CLI_ID` 常量引用非字面量（Read 确认）
- **S05-05**（MC-315）：`restoreSession.ts` 无值等于 `"claude"` 的字符串字面量（grep 零命中）；addPanel `title` 来自 `profile.tabTitle`（Read 确认）；pty.write 注入内容 = claude profile `buildRestoreInput` 策略输出且与现状逐字一致（`claude --resume <id>` + fork 追加 ` --fork-session` + `\r` 结尾——L2 断言存在且绿，依 npm test）
- **S05-06**（MC-316）：`historyContextMenu.ts` 的 `buildResumeCommand` 委托 `profile.capabilities?.history?.buildResumeCommand(session)`（Read 确认）；supportsFork=false → 不展示「分支恢复」L2 用例存在且绿（允许用例内局部注册测试 profile；依 npm test）；孤儿/无 cwd 禁分支恢复、运行中禁删除矩阵保留（Read 确认）
- **S05-07**（MC-314/317）：AgentStatusView 标题覆盖复合键 + 活跃区/历史区标题联动用例绿（依 npm test）；删除链（ask → `agent_history_delete(session.cliId, session.sessionId)` → removeLocal 不重扫）与双击三分派语义不变（Read 确认）
- **S05-08**（MC-312 核对）：`historyModel.ts` groupByCwd 分组维度保持 cwd（Read 确认无按 cliId 二次分组）；「全部项目」区聚合全部 provider 数据、无前端二次过滤（Read 确认）
- **S05-09**（AC-5 预检）：grep 七路径（`src/lib/`、`src/panels/terminal/`、`src/features/agentStatus/`、`src/features/agentHistory/`、`src/features/notifications/`、`src/ipc/`、`src/types/`）生产代码：无值等于 `"claude"` 的字符串字面量、无 claude 事件名字符串字面量（SessionStart/SessionEnd/UserPromptSubmit/Stop/StopFailure/PreToolUse/PostToolUse/PostToolUseFailure/Notification/PermissionRequest）、无 `~/.claude` 路径（import 路径指向 `features/cliProfiles/profiles/claude/` 属豁免形态，不算命中）
- **S05-10**（D-14 恢复注入段）：history.e2e 恢复编排用例零改动通过（依 npm run e2e；断言漂移即实现有误，判 not_fixed）
- **S05-11**（test-inventory）：`.claude/test-inventory.md` 已就近登记本 Stage 变动（agent-history-* 更名、cli-profile-claude 扩 history 策略用例、supportsFork 显隐新增用例，grep 确认）

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`（L1）
6. `npm test`（L2）
7. `npm run test:l3`（L3）
8. `npm run e2e`（L4——history.e2e 恢复注入断言在此层验证；最后单独串行执行，禁与其他命令并行）
