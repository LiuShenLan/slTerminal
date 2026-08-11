# Stage 02 逐项验证断言（唯一真值源）

> stage-02 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。
> 中间态说明：本 Stage 在 Stage 01 之后（守卫已扩八路径）——本 Stage 新增文件/改动不得引入守卫违规。

## 断言清单

- **ZQ-1**：复合键生产/消费五处全部经单点函数（语义式——Read 确认 `src/features/agentHistory/historyModel.ts`、`src/features/agentHistory/HistorySessionList.tsx`、`src/features/agentStatus/AgentStatusView.tsx` 中不再存在 `${...}|${...}` 裸模板拼接构造复合键，不限函数名与变量名；HistorySessionList 的 rowFlags 取键与 findPanelForSession 比较键均经同一函数，cliId 缺省回退在函数内部完成、消费方无各自回退）
- **ZQ-7**：单点函数内部对 cliId 与 sessionId 两侧均做竖线转义（Read 确认）；L2 存在「cliId/sessionId 含竖线时生产消费两侧键一致」用例 + cliId 缺省回退用例，且 L2 全绿
- **ZQ-2**：`src/panels/terminal/resolvePayloadCliId.ts` 存在（导出解析 helper）；三处消费方（`src/panels/terminal/useXterm.ts`、`src/features/agentStatus/useAgentStatus.ts`、`src/features/notifications/useAgentNotifications.ts`）改经 helper（语义式——三文件不再存在 `payload.cliId ??` 形式的裸三级链；空串/仅空白 cliId 与 null/undefined 同等回退）；三消费方空串回退 L2 用例各一存在，且 L2 全绿
- **ZQ-3**：`src/features/agentStatus/useAgentStatus.ts` 建行路径不存在 `?? "attention"` 兜底（语义式——Read 确认 null 映射事件首达建行时 status 落 null 而非 attention；AgentSessionRow.status 类型含 null；更新已有行的 null 不覆盖逻辑保持）；L2 存在「null 映射事件首达 → 建行 status=null 无图标」用例（附 SessionStart 丢失感知存活场景注释），且 L2 全绿
- **ZQ-6**：`src/panels/terminal/useXterm.ts` 清图标分支覆盖 SESSION_END_EVENT 与 EXIT_EVENT 双事件（Read 确认，与删 agentSession 分支的双事件判定对齐）；L2 存在 Exit 事件清图标用例，且 L2 全绿
- **ZQ-4**：`src/features/agentHistory/restoreSession.ts` 的 panelId 含自增序号段（语义式——Read 确认存在模块级计数器且 panelId 拼接含其前置递增，同毫秒两次调用结果相异；不限变量名）；L2 存在 mock Date.now 同值两次恢复 panelId 相异用例，且 L2 全绿
- **L3 零波及**：`test/terminal/production-osc.test.ts` 复刻段（OSC 52/133/8）未随本 Stage 改动（Read 确认本 Stage 三 agent 未触碰该文件生产复刻逻辑）
- **文档同步**：`src/features/agentHistory/CLAUDE.md`（keyOf 单点）、`src/features/agentStatus/CLAUDE.md`、`src/features/notifications/CLAUDE.md`、`src/panels/CLAUDE.md`（resolvePayloadCliId + 清图标双事件）四处与代码终态一致（Read 对照核实）；`.claude/test-inventory.md` 登记本 Stage 全部新用例（composite-key 单点负责口径）

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`（L1，必须单线程）
6. `npm test`（L2）
7. `npm run test:l3`（L3）
