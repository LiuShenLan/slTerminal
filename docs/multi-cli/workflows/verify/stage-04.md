# Stage 04 逐项验证断言（唯一真值源）

> stage-04 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。
> 旧命令名 grep 一律用词边界（`rg -w` 或等价精确边界）——`agent_history_scan` 命中 `history_scan` 属正常子串，非残留。

## 断言清单

- **S04-01**（MC-301）：Glob 断言 `src-tauri/src/agent_history/{mod,provider}.rs` 与 `src-tauri/src/agent_history/claude/{scan,jsonl,ops}.rs` 存在；`src-tauri/src/claude_history/` 目录不存在；`is_uuid_filename` 作为可复用工具保留（grep 命中）
- **S04-02**（MC-301/303/304）：`CliHistoryProvider` trait 三方法（scan/delete/validate_session_id）+ cliId 键注册表（Read `provider.rs` 确认）；trait 契约注释写明「validate_session_id 是 delete 的强制前置」（Read 确认）；L1 新增用例（聚合 scan 遍历多 provider / 单 provider 失败不阻塞 / delete 未知 cliId Validation / validate 前置）存在且绿（依 cargo test 结果）
- **S04-03**（MC-302）：`AgentHistorySession` serde 八键 camelCase（sessionId/cwd/title/titleSource/firstPrompt/mtimeMs/cwdExists/cliId）键集合测试 + titleSource 开放字符串序列化用例存在且绿（依 cargo test + Read `mod.rs` 确认 titleSource 为 String 非枚举）；claude provider 产出条目打标 `cli_id: "claude"`（Read scan.rs 确认）
- **S04-04**（MC-304/305）：63 条 L1 用例（jsonl 28/scan 19/ops 9/mod 7）迁移全绿（依 cargo test）；SEC-05 用例（UUID 形态校验 + locate_session_jsonl 遍历定位不信托前端路径）保留（grep/Read 确认）；env 覆盖 `SLTERM_CLAUDE_PROJECTS_DIR` 用例保留且每次调用读 env 不缓存（Read 确认）；tempdir 8.3 短名处理（dunce::canonicalize）保留（grep 确认）
- **S04-05**（MC-303）：`src-tauri/src/lib.rs` 注册 `agent_history_scan`、`agent_history_delete`（grep 命中）；旧命令名词边界 grep（`claude_history_scan|claude_history_delete`）于 `src-tauri/`、`src/`、`e2e-tests/` 零命中
- **S04-06**（MC-306/D-03）：Glob 断言 `src/types/agentHistory.ts`、`src/ipc/agentHistory.ts` 存在，旧 `src/types/claudeHistory.ts`、`src/ipc/claudeHistory.ts` 不存在；barrel 同步（grep `claudeHistory` 于 src/types/index.ts、src/ipc/index.ts 零命中）；契约测试 ipc-agent-history-contract 8 用例（scan 无参 / delete 双参 {cliId, sessionId} camelCase）四维全绿（依 npm test）
- **S04-07**（MC-306 调用点）：删除链调用点传 `session.cliId`（Read `src/features/claudeHistory/HistorySessionList.tsx` 与 `historyContextMenu.ts` 确认——删除调用实参 = `session.cliId, session.sessionId`；本 Stage 中间态：目录名 claudeHistory 保留，Stage 05 才更名）
- **S04-08**（D-14）：history.e2e 命令名断言同步泛化且全绿（依 npm run e2e；grep `claude_history_` 于 e2e-tests/ 零命中）
- **S04-09**（test-inventory）：`.claude/test-inventory.md` 已就近登记本 Stage 变动（L1 63 用例迁移 + 新增聚合/前置校验用例、契约测试更名条目，grep 确认）

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`（L1）
6. `npm test`（L2）
7. `npm run test:l3`（L3）
8. `npm run e2e`（L4——history.e2e 在此层验证；最后单独串行执行，禁与其他命令并行）
