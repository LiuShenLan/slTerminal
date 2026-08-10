# Stage 08 逐项验证断言（唯一真值源）

> stage-08 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。
> 文档类断言须对照真实代码/磁盘实态核实，防文档撒谎。

## 断言清单

- **S08-01**（MC-8 根文件）：`.claude/CLAUDE.md` 模块索引含三个新模块行（grep 命中：`src/features/cliProfiles`、`src/features/agentHistory`、`src-tauri/src/agent_history`）、无两个旧模块行（grep 零命中：`src/features/claudeHistory`、`src-tauri/src/claude_history`）；「需求编号索引」段含 MC 家族说明（grep 命中）
- **S08-02**（MC-8 文件表）：各新建/改动模块 CLAUDE.md 的文件表逐行 Glob 命中磁盘实态（语义式：Read `src/features/cliProfiles/CLAUDE.md`、`src/features/agentHistory/CLAUDE.md`、`src-tauri/src/agent_history/CLAUDE.md`、`src-tauri/src/hooks/CLAUDE.md`、`src/__tests__/CLAUDE.md` 的文件表，逐行对照 Glob——无幽灵文件、无遗漏文件）；旧 `src/features/claudeHistory/CLAUDE.md` 与 `src-tauri/src/claude_history/CLAUDE.md` 不存在（Glob 断言）
- **S08-03**（MC-8 test-inventory）：`.claude/test-inventory.md` 计数与实跑一致（取数口径：L1/L2/L3 以本 Stage 全量测试输出统计行为准、L4 以 spec 文件用例计数为准；Read 文档计数逐层比对）；旧测试名（tab-title-registry/tab-rules/cli-icons/claude-status/ipc-hooks-contract/ipc-claude-history-contract/claude-history-*）零残留（grep 确认）
- **S08-04**（MC-109）：`src-tauri/src/pty/CLAUDE.md` 与 `src/panels/CLAUDE.md` 无「专为 claude|claude 定制」归属表述（grep 零命中）；触发点描述保留（grep「供 claude 取消」命中——若 pty CLAUDE.md 现状无此字面则 Read 确认机制段落的历史动机描述未删）
- **S08-05**（MC-110）：pty 相关文档含 SLTERM_PANEL_ID 保留为通用每终端路由键的记录 + 「无此变量 exit(0)」门控语义归各 CLI reporter 实现的说明（grep/Read 确认）
- **S08-06**（MC-318）：`src/features/agentHistory/CLAUDE.md` 含「已知限制」段两条（组键漂移 / 历史区相对时间无 ticker）且注明「规格确认不修（决策 6）」（Read 确认）
- **S08-07**（MC-223）：`src/panels/CLAUDE.md`（hooksConfig 段）与 `src/features/hooksConfig/CLAUDE.md` 含「claude 专属」注明（grep 命中两处）
- **S08-08**（MC-8 命名终态）：全部模块 CLAUDE.md 旧命名零残留（grep 于 `src/**/CLAUDE.md`、`src-tauri/**/CLAUDE.md`、`e2e-tests/CLAUDE.md`：`hook-event`、`onHookEvent`、`claude_history_scan|claude_history_delete`、`useClaudeHistory`、`ClaudeHistorySections`、`claudeStatus`、`cliIcons` 零命中；`hooks_inject` 等旧命令名词边界零命中；`claude` 作为模块/功能归属表述仅出现于 profiles/claude 或「claude 专属」注明语境——语义式 Read 抽查）
- **S08-09**（CONTEXT.md）：术语与终态一致（Read CONTEXT.md 核对：cliId / CLI profile / agent-event / agent_history 等词条与代码终态命名一致，无旧术语残留矛盾）
- **S08-10**（AC-6 终验）：文档与代码终态一致性抽查（语义式：随机抽 3 处文档行为描述对照真实代码核实不撒谎）+ 全量门禁绿即终验 AC-1/AC-2

## 全量测试（全部通过为门禁——本 Stage 为终验）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`（L1）
6. `npm test`（L2）
7. `npm run test:l3`（L3）
8. `npm run e2e`（L4——最后单独串行执行，禁与其他命令并行）
