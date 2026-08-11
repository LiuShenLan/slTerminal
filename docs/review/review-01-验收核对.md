# review-01 验收核对

> 范围：docs/multi-cli/ 8 Stage verify 断言（workflows/verify/stage-01.md ~ stage-08.md）逐条静态复核。
> 方法：Grep/Glob/Read 一手证据；L1/L2/L3 绿态以基线门禁实跑为准（见 review-00），L4 未复跑、采信收尾报告 b294c67（39 用例全绿）。
> 计数口径：「生产代码」= `src/` 排除 `src/__tests__/`。

## 验收矩阵

### Stage 01（前端 CliProfileRegistry + 身份域）

| 断言 | 结论 | 证据 |
|------|------|------|
| 退役 12 文件（cliIcons.ts / TabTitleRegistry.ts / tabRules 等） | ✅ | Glob 全仓零命中 |
| `split(/\s+/)[0]` 首 token 解析全仓唯一 | ✅ | 生产代码仅 `cliProfileRegistry.ts` 命中 |
| 生产注册触发点 = Workspace.tsx 显式 import | ✅ | `Workspace.tsx:30` import `features/cliProfiles/profiles` |
| `get(CLAUDE_CLI_ID)` 生产代码计数 = 0（终态口径） | ✅ | grep 生产零命中；仅测试 5 处合法引用 |
| useCommandDetection 消费 matchByCommand / TabState 顶部导出 / `STATUS_EMOJI.attention` / setAgentSession 携 cliId | ✅ | `useCommandDetection.ts:17`（TabState 导出）/`:71`（attention）/`:76-79`（setAgentSession 携 `cliId: profile.id`） |
| matchedCommand 值 = profile.id | ✅ 设计内 | 测试锁死：`mock-cli-profile.test.tsx:414` 断言 `matchedCommand === profile.id` |

### Stage 02（前端状态域去 claude 化）

| 断言 | 结论 | 证据 |
|------|------|------|
| `claudeSession`/`setClaudeSession`/`ClaudeSessionInfo`/`CLAUDE_CONTEXT_LIMIT`/`claudeStatus` 零残留 | ✅ | 全仓 grep 零命中 |
| src/lib/ 无 claude 事件名字面量 | ✅ | grep 零命中 |
| 空态文案更名「无运行中的编码 CLI 会话」（MC-414） | ✅ | `AgentStatusView.tsx:183`「当前项目无运行中的编码 CLI 会话」；agent.e2e.ts includes 断言兼容 |

### Stage 03（后端 hooks 泛化 + CliHooksProvider）

| 断言 | 结论 | 证据 |
|------|------|------|
| 旧命令名 `hooks_inject` 等 6 条词边界零残留 | ✅ | grep（排除 `agent_` 前缀）零命中 |
| `hook-event`/`onHookEvent`/`HookEventPayload` 全仓零残留 | ✅ | src/src-tauri/e2e-tests/test 四目录零命中 |
| lib.rs 注册 6 条 agent_hooks_* 泛化命令 | ✅ | `lib.rs:100-105` |
| reporter payload 显式 `cliId:"claude"` + SCRIPT_VERSION 递增（决策 7） | ✅ | `slterm-hook-reporter.js:6`（SCRIPT_VERSION=2）/`:46`（cliId） |
| 注入目标路径 `~/.slterminal/hooks/` 不变（E2E 零波及） | ✅ | `hooks/claude/mod.rs:25`/`:29` |
| setup.ts 全局 mock 指向 `../ipc/agentHooks` | ✅ | `src/__tests__/setup.ts:93` |
| trait 六方法 + cliId 键注册表 + MC-211 双分支错误语义 | ✅ | `hooks/provider.rs:25-78`（含「无 hooks 能力」预留分支与测试） |

### Stage 04（后端历史会话泛化 + CliHistoryProvider）

| 断言 | 结论 | 证据 |
|------|------|------|
| `claude_history_scan`/`claude_history_delete` 零残留 | ✅ | 词边界 grep 零命中 |
| lib.rs 注册 agent_history_scan/delete | ✅ | `lib.rs:106-107` |
| trait 三方法 + SEC-05 等价强制契约注释 | ✅ | `agent_history/provider.rs:19-54`（`validate_session_id` 为 delete 强制前置写入 trait doc） |

### Stage 05（前端历史聚合 UI 泛化 + 复合键）

| 断言 | 结论 | 证据 |
|------|------|------|
| S05-01 agentHistory/ 9 文件 + 测试更名 6 文件、claudeHistory/ 不存在 | ✅ | Glob：`agent-history-{model,hook,restore,row,action-dialog,view}.test.ts(x)` 均在；`claude-history-*` 零命中 |
| S05-02 `useClaudeHistory`/`ClaudeHistorySections` 零残留 | ✅ | grep 零命中 |
| S05-02 `claudeHistory` 零残留 | ⚠️ 2 处注释残留 | 见 YS-4（失实）/ strategies.ts:10（迁移溯源，可接受） |
| S05-03 行 logo = `cliProfileRegistry.get(session.cliId)?.iconSrc`，仅随 status emoji，未注册不报错 | ✅ | `HistorySessionRow.tsx:54` + `:85-93` |
| S05-04 复合键三处竖线拼接 + CLAUDE_CLI_ID 常量回退（非字面量） | ✅ | `historyModel.ts:138`、`HistorySessionList.tsx:196/278/380`、`AgentStatusView.tsx:133/144` |
| S05-05 agentHistory/ 无 "claude" 字面量；addPanel title = profile.tabTitle | ✅ | grep 零命中；`agent-history-restore.test.ts` 断言锁死 |
| S05-08 groupByCwd 维度保持 cwd、无 cliId 二次分组 | ✅ | `historyModel.ts:48-71` |
| S05-09 AC-5 预检七路径 | ✅ | 由 no-claude-literals.test.ts 守卫（见 Stage 07） |
| S05-10 history.e2e 恢复编排零改动通过（L4） | ◻ 采信 | 收尾报告 b294c67：L4 39 用例全绿 |

### Stage 06（hub 面板 + CLI 选择行）

| 断言 | 结论 | 证据 |
|------|------|------|
| S06-07 `CLAUDE_CLI_ID` 于 panels/hooksConfig/ 零命中（中间态回收） | ✅ | grep 零命中；`panels/CLAUDE.md:150` 记录回收（MC-220） |
| S06-01~05/08 hub 机制（选择行/持久化/dirty 守卫/空态/restartHint/入口零改动） | ◻ 采信 | 文档侧描述完整（`panels/CLAUDE.md:146-157`）；L2 用例绿待基线确认 |
| S06-09 hooks.e2e hub 用例（L4） | ◻ 采信 | 收尾报告 b294c67 |

### Stage 07（mock profile 全链路 + AC-5 守卫）

| 断言 | 结论 | 证据 |
|------|------|------|
| S07-01 夹具存在 + mockcli 生产零命中 | ✅ | `src/__tests__/helpers/mockCliProfile.ts`；生产 grep 仅 `TerminalPanel.tsx:126` 注释（e2e 定位锚点说明） |
| S07-02 mock-cli-profile.test.tsx 五点用例存在 | ✅ | `mock-cli-profile.test.tsx:375-778`（OSC 133 命中/策略真实调用/历史聚合/hub 选择行/恢复注入）；绿态待基线 |
| S07-03 AC-5 守卫（七路径 + 三类违规 + import 豁免 + 防静默空扫） | ✅ | `no-claude-literals.test.ts:39-47`（七路径）/`:50-61`（10 事件名）/`:64`（豁免标记）/`:206-213`（扫描完整性断言） |
| S07-04 E2E helper 存在 + E2E_ENABLED 门控 | ✅ | `e2e-tests/helpers.ts:345`（installMockCliProfile）/`:372`（window 挂载）；L4 绿采信收尾报告 |
| S07-05 通用层消费方抽查（useCommandDetection） | ✅ | 见 Stage 01 行；其余消费方归维度「扩展性达成」抽查 |

### Stage 08（文档同步终验）

| 断言 | 结论 | 证据 |
|------|------|------|
| S08-01 模块索引三新行 + 两旧行移除 + MC 家族说明 | ✅ | 根 `CLAUDE.md` 索引含 cliProfiles/agentHistory/src-tauri agent_history 行；无 claudeHistory/claude_history 行；「未列入的编号家族免登记」段含 MC-* |
| S08-04 无「专为 claude / claude 定制」归属表述 | ✅ | grep 命中均为「claude 专属编辑器」合法注明（S08-07 要求保留）；pty/CLAUDE.md 仅历史动机描述 |
| S08-05 SLTERM_PANEL_ID 通用路由键记录 | ✅ | `pty/CLAUDE.md`「通用每终端路由键（MC-110）」段 + 门控语义归 reporter 说明 |
| S08-06 已知限制段两条（组键漂移/无 ticker）注明不修 | ✅ | `agentHistory/CLAUDE.md`「已知限制（MC-318，规格确认不修——决策 6）」 |
| S08-07 「claude 专属」两处注明 | ✅ | `features/hooksConfig/CLAUDE.md:9`、`panels/CLAUDE.md:146/150` |
| S08-08 模块 CLAUDE.md 旧命名零残留 | ⚠️ 见 YS-3 | `hook-event`/`onHookEvent`/`claude_history_*`/`claudeStatus` 零命中；`TabTitleRegistry` 作模式先例引用 7 处 |
| S08-09 CONTEXT.md 术语一致 | ✅ | `CONTEXT.md:260-261` 更名对照表（旧名→新名映射，合法） |
| S08-10 文档与代码终态一致性抽查 | ⚠️ 1 处失实 | 见 YS-1 |

## 问题条目

### YS-1（P2）panels/CLAUDE.md 仍有退役 API 的功能描述，与同文件新描述自相矛盾

- 位置：`src/panels/CLAUDE.md:249`
- 问题：该行描述 `useCommandDetection` 为「`logo: cliIconRegistry.match(command)`」——`CliIconRegistry.match` 是 Stage 01 已退役删除的 API（且方法名 `match` 从未存在于新注册表，新 API 为 `matchByCommand`）；同文件 `:275` 已是正确的新描述（`cliProfileRegistry.matchByCommand`）。同一文件两处描述互相矛盾，读者无法判断哪个是真。
- 修复建议：`:249` 改为与 `:275` 一致的描述（`cliProfileRegistry.matchByCommand` 命中后 `logo = profile.iconSrc`、`icon = STATUS_EMOJI.attention`），或直接删除重复行。
- 来源：验收核对 S08-10（文档不撒谎抽查）

### YS-2（P3）两处测试注释残留退役 API 名

- 位置：`src/__tests__/terminal.test.tsx:292`、`src/__tests__/use-xterm-lifecycle.test.ts:1363`
- 问题：注释仍写 `CliIconRegistry.match("claude")` 字样（退役 API + 字面量形态），与现行 `cliProfileRegistry.matchByCommand` 消费方式不符；仅注释、不影响运行。
- 修复建议：两处注释改为现行 API 名，或删除过时注释。
- 来源：验收核对 Stage 01/02（更名零残留断言的注释层残留）

### YS-3（P3）7 处文档/注释引用已删除的 TabTitleRegistry 作「模式先例」

- 位置：`src/features/sideViews/CLAUDE.md:5`/`:28`/`:46`/`:125`、`src/theme/CLAUDE.md:19`、`src/theme/schemeRegistry.ts:4-5`、`src/features/cliProfiles/CLAUDE.md:17`/`:29`
- 问题：TabTitleRegistry 已在 Stage 01 删除，上述文件仍以「照 TabTitleRegistry 模式」「同 TabTitleRegistry 的 register/getAll/_reset 模式」作模式先例——读者无法查阅被引用者的源码，先例失效。（`panels/CLAUDE.md:275`「Stage 01 退役 TabTitleRegistry 后」与 `cliProfiles/CLAUDE.md:29`「原 TabTitleRegistry.ts 两份拷贝收敛于此」属迁移溯源交代，合法，不计。）
- 修复建议：模式先例统一改指现存注册表（`CliProfileRegistry` 或 `ShortcutRegistry`），或删除先例引用只留自身机制描述。
- 来源：验收核对 S08-08（语义式抽查）

### YS-4（P3）schemes/types.ts 注释引用已更名的 claudeHistory 视图

- 位置：`src/theme/schemes/types.ts:46`
- 问题：注释「agentStatus/claudeHistory/commit 视图借用」——claudeHistory 视图已更名 agentHistory（Stage 05），注释描述的借用方不存在；且属 S05-02「claudeHistory 零残留」断言的字面口径残留（同断言下 `strategies.ts:10` 为迁移溯源注释，可接受）。
- 修复建议：`claudeHistory` → `agentHistory`。
- 来源：验收核对 S05-02

### YS-5（P3）AgentStatusRow 注释引用退役 API 作历史对照

- 位置：`src/features/agentStatus/AgentStatusRow.tsx:55`
- 问题：注释「与原 cliIconRegistry.getSrc 语义一致」——引用已删除 API 作语义对照，读者无法查证；信息价值低。
- 修复建议：删除该对照半句，或改为自含描述（「按行 cliId 查 profile.iconSrc，未注册不报错」）。
- 来源：验收核对 Stage 02（独立发现）

## 核对覆盖说明

- 以上为我直接复核的静态可证断言；「依 npm test / npm run e2e」类断言的绿态：L1/L2/L3 以本次基线门禁实跑为准（汇总于 review-00），L4 未复跑、采信收尾报告 b294c67。
- L4 层断言（S05-10/S06-09/S07-04 绿态部分）与人工验证点（completion-report 全部「待实测」）不在本报告判定范围，统一归 review-00「待人工确认」节。
