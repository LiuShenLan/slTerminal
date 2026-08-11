# Stage 06 逐项验证断言（唯一真值源）

> stage-06 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。
> 中间态说明：本 Stage 为收尾终验——断言针对 Stage 01-05 全部完成后的最终状态；文档类断言须对照真实代码核实，防文档撒谎。

## 断言清单

- **YS-1**：`src/panels/CLAUDE.md` 不再存在 `cliIconRegistry.match` 描述（:249 与 :275 描述一致——Read 两处对照；新描述与 `useCommandDetection.ts` 现行实现一致：matchByCommand 命中 → logo = profile.iconSrc）；全仓 grep `CliIconRegistry` 仅存迁移溯源形态（逐处 Read 甄别）
- **YS-2**：`src/__tests__/terminal.test.tsx:292` 与 `src/__tests__/use-xterm-lifecycle.test.ts:1363` 两处注释不再引用 CliIconRegistry.match（改现行 API 或已删除——Read 确认）
- **YS-3**：7 处「TabTitleRegistry 模式先例」引用已更新（`src/features/sideViews/CLAUDE.md` 4 处、`src/theme/CLAUDE.md` 1 处、`src/theme/schemeRegistry.ts` 1 处、`src/features/cliProfiles/CLAUDE.md` 2 处——改指现存注册表或删先例留自身机制描述，逐处 Read）；合法迁移溯源两处保留（panels/CLAUDE.md:275、cliProfiles/CLAUDE.md:29 后半）未被误删；全仓 grep `TabTitleRegistry` 仅溯源形态
- **YS-4**：`src/theme/schemes/types.ts:46` 注释为 `agentHistory`（grep `claudeHistory` 于该文件零命中）
- **YS-5**：`src/features/agentStatus/AgentStatusRow.tsx:55` 不再含「与原 cliIconRegistry.getSrc 语义一致」对照半句（Read 确认新表述自含：按行 cliId 查 profile.iconSrc，未注册不报错——与代码一致）
- **WD-1**：`.claude/CLAUDE.md` 需求编号索引表含 F9 行（终端页签/侧栏 CLI 品牌 logo——按命令行首 token 匹配 profile.iconSrc；Read 确认表内存在）
- **WD-2**：`src/panels/CLAUDE.md:152`、`src/features/hooksConfig/CLAUDE.md:21`、`src/features/hooksConfig/schema/index.ts:15` 三处「补全/悬停/波浪线」中的「补全」字样已移除（grep `补全` 于三文件零命中或仅剩「无自动补全」否定表述——逐处 Read 甄别）
- **WD-3**：`CONTEXT.md:75-76` 描述为「前端统一 CliProfileRegistry；后端按能力拆分 hooks/history 两个 cliId 键注册表」（Read 对照 `src-tauri/src/hooks/provider.rs` 与 `src-tauri/src/agent_history/provider.rs` 实际 REGISTRY 确认一致）
- **WD-4**：`src/features/agentHistory/AgentHistorySections.tsx:49` 注释为复合键口径 `Map<cliId|sessionId, status>`（Read 确认与 Stage 02 落地的 keyOf 单点一致）
- **KZ-6**：`src/features/cliProfiles/CLAUDE.md`「新增 CLI 步骤」含四要素（Read 确认与 Stage 03/04 终态一致）：后端 hooks provider 注册（hooks/provider.rs REGISTRY）、后端 history provider 注册（agent_history/provider.rs REGISTRY）、test-inventory 用例数同步、hasConfigEditor=true 时编辑器组件挂入 profile configEditor + configLayers
- **终态核对**：root-doc 报告的全仓 grep 兜底结果复核——`CliIconRegistry`/`TabTitleRegistry`/`claudeHistory`/`transcriptPath` 残留均为豁免形态（逐处 Read 甄别，非豁免残留判 not_fixed）；`.claude/test-inventory.md` 用例数 = 本 Stage 全量复跑实跑计数（L1/L2/L3/L4 各级对齐，测试 agent 报告为据）

## 全量测试（全部通过为门禁——收尾终验，执行前确认无运行中的 slterminal.exe）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`（L1，必须单线程）
6. `npm test`（L2）
7. `npm run test:l3`（L3）
8. `npm run e2e`（L4——其余命令全部完成后单独串行执行）
