# Stage 01 逐项验证断言（唯一真值源）

> stage-01 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。
> 计数类取数口径：「生产代码」= `src/` 排除 `src/__tests__/`；grep 计数断言均按此口径，测试目录引用不计。

## 断言清单

- **S01-01**（MC-101/103/104）：`src/features/cliProfiles/cliProfileRegistry.ts` 存在且导出模块级单例，含五方法 `register` / `get` / `getAll` / `matchByCommand` / `_reset`（Read 确认签名）；`profiles/claude/index.ts` 的 claude profile 身份域完整：id/displayName/tabTitle = "claude"、commands = ["claude"]、iconSrc = "/cli-icons/claude.png"、capabilities 为 `{}`（本 Stage 中间态）；`profiles/claude/` 导出 `CLAUDE_CLI_ID = "claude"` 常量。L2 用例（register 覆盖/get/getAll 注册序/matchByCommand 多 commands·带参·空命令行·仅空白·未命中·不 toLowerCase/_reset）全绿——依全量测试 npm test 结果判定
- **S01-02**（MC-102）：首 token 解析单点化——`matchByCommand` 内部实现 `trim().split(/\s+/)[0]`（Read 确认）；全仓生产代码中不存在第二份 `trim().split(/\s+/)[0]` 拷贝（grep `split(/\s+/)\[0\]` 于 `src/` 排除 cliProfileRegistry.ts 零命中；测试/L3 复刻段不计）
- **S01-03**（退役）：Glob 断言以下文件均不存在：`src/lib/cliIcons.ts`、`src/panels/terminal/tabRules.ts`、`src/panels/terminal/TabTitleRegistry.ts`、`src/__tests__/tab-title-registry.test.ts`、`src/__tests__/tab-rules.test.ts`、`src/__tests__/cli-icons.test.ts`
- **S01-04**（MC-106）：grep 断言 `src/panels/terminal/useCommandDetection.ts` 无 `"🟡"` 字面量、无 `TabTitleRegistry|CliIconRegistry` import；全仓（`src/` + `test/` + `e2e-tests/`）无 `cliIconRegistry|CliIconRegistry|tabTitleRegistry|TabTitleRegistry` 代码引用（grep 零命中）
- **S01-05**（过渡形态白名单）：生产代码（`src/` 排除 `src/__tests__/`）中 `get(CLAUDE_CLI_ID)` grep 计数 = 2，且仅现于 `src/features/agentStatus/AgentStatusRow.tsx` 与 `src/features/claudeHistory/HistorySessionRow.tsx`；两文件中无值等于 `"claude"` 的字符串字面量（grep `"claude"` 于两文件零命中，import 路径不算）
- **S01-06**（D-07/D-02）：`src/workspace/Workspace.tsx` 含指向 `features/cliProfiles/profiles` 的 import（grep 命中）；`src/lib/index.ts` 无 cliIcons 导出（grep `cliIcons` 零命中）
- **S01-07**（TabState 承接）：`src/panels/terminal/useCommandDetection.ts` 顶部导出 `TabState` 类型（含 `logo` 字段，grep `export (interface|type) TabState` 命中）；`TerminalPanel.tsx` / `useXterm.ts` / `usePtyOutput.ts` 的 TabState import 来源 = useCommandDetection（Read 确认）；TerminalPanel 消费链（`tabIcon && tabLogo` 双条件 / inactive 双清）零改动（Read 确认保留）
- **S01-08**（D-08/MC-108/S01 资源）：`test/terminal/production-osc.test.ts` 的 OSC 133 复刻段与生产 `matchByCommand` 取值逻辑一致（语义式：Read 对照复刻段与 cliProfileRegistry 实现，逐段来源行号注释已同步）；`src/__tests__/cli-profile-registry.test.ts` 含资源守卫用例——遍历注册表全部 profile 断言 iconSrc 磁盘存在 + PNG 魔数（Read 确认）；Glob 命中 `public/cli-icons/mockcli.png`
- **S01-09**（test-inventory）：`.claude/test-inventory.md` 已就近登记本 Stage 用例变动：删 tab-title-registry/tab-rules/cli-icons 三行（或标记迁移）、增 cli-profile-registry/cli-profile-claude 行（grep 确认）；L2 全绿且新注册表用例数与文档一致（依 npm test 结果）

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`（L1）
6. `npm test`（L2）
7. `npm run test:l3`（L3——production-osc 复刻段在此层验证）
8. `npm run e2e`（L4——最后单独串行执行，禁与其他命令并行）
