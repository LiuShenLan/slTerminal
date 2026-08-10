# Stage 03 逐项验证断言（唯一真值源）

> stage-03 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。
> 旧命令名 grep 一律用词边界（`rg -w` 或等价精确边界）——`agent_hooks_inject` 命中 `hooks_inject` 属正常子串，非残留。

## 断言清单

- **S03-01**（MC-201）：`AgentEventPayload` 九键（panelId/event/timestamp/sessionId/transcriptPath/cwd/toolName/notificationType + 可选 cliId）serde 键集合测试存在且绿；含无 cliId 旧信号反序列化兼容用例（cliId serde default）（依 cargo test 结果 + Read `src-tauri/src/hooks/signal.rs` 确认 `cli_id` 字段 `#[serde(default)]`）
- **S03-02**（MC-202/D-01）：grep 零残留（`src/` + `src-tauri/` + `e2e-tests/` + `test/`）：`"hook-event"`、`onHookEvent`、`HookEventPayload`（三词各自零命中）；`src/__tests__/setup.ts` 全局 mock 路径 = `../ipc/agentHooks` 且 mock 导出名 `onAgentEvent`（grep 命中）
- **S03-03**（MC-210/211）：`src-tauri/src/hooks/provider.rs` 存在，`CliHooksProvider` trait 六方法（inject/uninstall/injection_status/context_usage/config_read/config_write）+ cliId 键注册表 + claude 注册（Read 确认）；L1 新增用例（注册表 get / 未知 cliId Validation / 无 hooks 能力 Validation 分支 / 命令 cliId 透传 block_on 直测）存在且绿（依 cargo test 结果）
- **S03-04**（MC-211）：`src-tauri/src/lib.rs` 注册 6 泛化命令（grep 命中 `agent_hooks_inject`、`agent_hooks_uninstall`、`agent_hooks_injection_status`、`agent_context_usage`、`agent_hooks_config_read`、`agent_hooks_config_write`）；旧命令名词边界 grep（`hooks_inject|hooks_uninstall|hooks_injection_status|hooks_context_usage|hooks_config_read|hooks_config_write` 带词边界）于 `src-tauri/src/lib.rs`、`src/ipc/`、`e2e-tests/` 零命中
- **S03-05**（MC-213/214）：Glob 断言 `src-tauri/src/hooks/claude/{inject,usage,config}.rs` 存在、`src-tauri/src/hooks/{inject,usage,config}.rs` 顶层同名不存在；133 条 L1 用例（inject 34/signal 14/watcher 20/usage 26/config 27/mod 12）迁移全绿（依 cargo test 结果）；`ContextUsage` 四字段（cache serde default 0）保留（Read 确认）
- **S03-06**（MC-215 决策 7）：Glob 命中 `src-tauri/src/hooks/claude/slterm-hook-reporter.js`、`src-tauri/assets/slterm-hook-reporter.js` 不存在；reporter 模板含显式 `cliId: "claude"`（grep 命中）且 SCRIPT_VERSION 已递增（L1 模板内嵌校验断言存在且绿——依 cargo test；Read 确认递增）；注入目标路径 `~/.slterminal/hooks/slterm-hook-reporter.js` 不变（grep 确认）；E2E-06 链路绿（依 npm run e2e）
- **S03-07**（MC-203/204 核对）：`src-tauri/src/hooks/watcher.rs` 双通道（notify 50ms debounce + 3s 轮询补漏 + 目录自动重建）与 process_signal_file 读→emit→删契约零改动（Read 确认；语义式：轮询补漏间隔未被削弱/删除）
- **S03-08**（MC-212/D-03/决策 3）：Glob 断言 `src/types/agent.ts`、`src/ipc/agentHooks.ts` 存在，旧 `src/types/hooks.ts`、`src/ipc/hooks.ts` 不存在；`src/types/index.ts` barrel 导出 agent（grep `from.*types/hooks` 于 src/ 零命中）；决策 3 更名落实（Read 确认 `AgentInjectionStatus`/`AgentHookInjectionStatus`）；契约测试 ipc-agent-hooks-contract 22 用例四维（命令名/参数 cliId camelCase/返回/异常）+ ipc-hooks-config-contract 全绿（依 npm test）
- **S03-09**（中间态写死）：`useHooksConfig.ts` / `HooksConfigPanel.tsx` 的泛化命令 cliId 实参来自 `CLAUDE_CLI_ID` 常量 import（语义式：Read 确认实参来源，非 `"claude"` 字面量）；`useAgentStatus.ts` 的 contextUsage 调用传行 cliId（Read 确认）
- **S03-10**（决策 4/D-11）：`e2e-tests/run-wdio.cjs` 备份集合保持 claude 硬编码且含「随第二 CLI 接入扩展」注释（grep 命中）
- **S03-11**（test-inventory）：`.claude/test-inventory.md` 已就近登记本 Stage 变动（L1 hooks 133 用例位置迁移 + 新增用例、契约测试更名、E2E 断言同步条目，grep 确认）

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`（L1）
6. `npm test`（L2）
7. `npm run test:l3`（L3）
8. `npm run e2e`（L4——hooks.e2e 泛化命令 + agent.e2e 事件名断言在此层验证；最后单独串行执行，禁与其他命令并行）
