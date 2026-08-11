# review-fix 修复 — 逐 ID 清单

> 真值源：`docs/review/review-00-summary.md` ~ `review-08`（30 条：1 P0 + 6 P1 + 11 P2 + 12 P3）。
> 范围决策（review-00「决策与人工确认状态」，2026-08-11 拍板）：**全部 30 条列入本期修复**。
> Stage 划分见 `stages.md`；编排参数见 `execution-plan.md`。

## 组织约定

- ID 沿用 review 原编号（YS/ZQ/AQ/CS/WD/KZ 前缀 + 序号），Stage 划分按 ID 引用
- 每条含：级别、位置（Glob/Read 实查锚点，非凭记忆）、修复要点、Stage 归属
- 优先级保留 review 原级（P0–P3）；执行序由 Stage 依赖表达（stages.md 头部）
- **去重合并留痕**：
  - KZ-5 → 并入 KZ-1（同源消解：编辑器分派落地后 claude 路径字面量留在 claude 专属编辑器合法领地，Stage 04 验证消解，不独立改代码）
  - ZQ-1 + ZQ-7 → 合并修复形态（`keyOf` 复合键单点：回退 + 转义一处生效，三处拼接/消费点统一），两条均列、同一 agent 落地
  - ZQ-2 三处同构（useXterm/useAgentStatus/useAgentNotifications）→ 抽 `resolvePayloadCliId` helper 单点，一条三处

## 决策结论（grilling 拍板，2026-08-11）

| # | 决策点 | 结论 | 影响条目 |
|---|--------|------|---------|
| 1 | KZ-2/KZ-3 更名范围 | **全链路统一更名 `usageSourcePath`**——事件 DTO 字段 + trait 参数 + 命令参数 + 前端内部状态字段（TerminalRegistry.agentSession）+ reporter payload 键（SCRIPT_VERSION 2→3，照决策 7 先例：已注入用户显示「版本过旧」需重新注入）。**不加 serde alias**——信号文件瞬态（亚秒~3s 存活），旧键信号降级为 None（仅丢该事件用量拉取），版本门控引导重新注入 | KZ-2、KZ-3 |
| 2 | ZQ-3 修复形态 | **null 映射事件建行但 status 置 null**（无图标）——兼顾「感知会话存活」与「不误标 attention」；与 deriveActiveSessionStatuses「status 为 null 不产出键」语义一致 | ZQ-3 |
| 3 | ZQ-5 修复方向 | **后端写路径 hooks 入参 null 视作 {}** 进行 merge——与 read 返回 null 对称，与既有「原文件内容为 null 视作空对象」语义一致；语义 = 清空该层 hooks | ZQ-5 |
| 4 | L4 门禁策略 | **按需 + 收尾全量**——静态+L1/L2/L3 每 Stage 全量；L4 在 Stage 01（run-wdio.cjs）、03（e2e spec 更名波及）、05（CS-3 新增用例）跑；Stage 06 收尾统一全量复跑（含 L1——届时须关闭运行中的 slterminal.exe） | 全 Stage 门禁 |

另有两处计划期自查决策（技术细节，不单列问题）：

- **CS-1 修复强度 = 报违规**（review 首选）：词法器对含 `${}` 的模板字符串提取全部字面量片段拼接（忽略表达式），拼接结果命中禁令词表（`claude` 精确 / 10 事件名 / `~/.claude` 路径）即违规
- **E2E spec 更名波及口径**：`transcriptPath`（camelCase 信号文件构造）随行更名；`transcript_path`（snake_case，hooks.e2e.ts:170 模拟 claude hook **stdin 协议**字段）**不动**——claude hook stdin 协议是 claude 领地知识，reporter 读取后映射为中性 `usageSourcePath`

## 1. 安全（AQ-1~4，来源 review-04）

| ID | 级别 | 位置 | 修复要点 | Stage |
|----|------|------|---------|-------|
| AQ-1 | P1 | `src/features/cliProfiles/profiles/claude/strategies.ts:109-112`（buildResumeCommand） | cwd 单引号按 PowerShell 规则转义为 `''`（两个单引号）：`session.cwd.replace(/'/g, "''")`；删除 :106-107「原样保留」自述注释改为转义说明；**同步更新 L2「输出与迁出源逐字一致」断言**（cli-profile-claude.test.ts）并新增 cwd 含单引号回归用例；E2E history.e2e 复制恢复命令断言同步（fixture cwd 无单引号，预期不变，跑通即证） | 01 |
| AQ-2 | P2 | `src-tauri/src/hooks/signal.rs:72-78`（process_signal_file_with） | 读取前 `metadata()` 限制大小（上限 1MB，常量 `MAX_SIGNAL_FILE_BYTES`），超限 `tracing::warn!` + 删除文件返回（与既有「解析失败仍删」容错语义一致）；L1 新增超限用例（构造 >1MB 文件 → emit 不被调用且文件已删） | 01 |
| AQ-3 | P2 | `src-tauri/src/agent_history/claude/ops.rs:43-56`（locate_session_jsonl）、:77-81（同名目录删除） | 符号链接拒跟随：一级子目录 `dir_path.is_dir()` 后补 `!dir_path.is_symlink()`；命中文件 `candidate.is_file()` 后补 `!candidate.is_symlink()`；同名 `<id>/` 目录 :79 判定同款补 `!is_symlink()`；L1 新增符号链接越界用例（symlink 指向外部目录 → 定位不命中/拒绝删除；Windows 建目录符号链接需权限——测试用 `std::os::windows::fs::symlink_dir`，失败时 skip 语义注释说明） | 01 |
| AQ-4 | P2 | `e2e-tests/run-wdio.cjs:148-152`（fixture 缺失 else 分支） | fixture 缺失即终止不降级：`console.error` 明确文案 + `process.exit(1)`（wdio 启动前）；verify 半自动断言（rename fixture → 非零退出 → 恢复）+ L4 正常路径全绿回归 | 01 |
| ZQ-5 | P2 | `src-tauri/src/hooks/claude/config.rs:109-112`（write_hooks_subtree）、:179-181（config_write_sync） | **决策 3**：写路径 hooks 入参 null 视作 {}——`config_write_sync` 入口校验改为「null → 空对象 {}，其余非 object → Validation」；write_hooks_subtree 收到的恒为 object；L1 新增「hooks=null → 视作清空该层」用例；模块 CLAUDE.md write 语义段就近同步 | 01 |

## 2. 正确性（ZQ-1~7，来源 review-02；ZQ-5 已列安全族）

| ID | 级别 | 位置 | 修复要点 | Stage |
|----|------|------|---------|-------|
| ZQ-1 | P2 | `src/features/agentHistory/HistorySessionList.tsx:278`（rowFlags 消费无回退）；拼接方 `historyModel.ts:138`、`AgentStatusView.tsx:133/144`、`HistorySessionList.tsx:196/206` | 与 ZQ-7 合并形态：**新建 `keyOf(cliId, sessionId)` 单点**（落点 `historyModel.ts` 导出——FE-05 纯函数模型单点），内部统一 `cliId ?? CLAUDE_CLI_ID` 回退 + `\|` 转义；三处拼接方 + 两处消费方（rowFlags get / findPanelForSession 比较键）全部改经 keyOf；findPanelForSession 入参侧（scan 数据 cliId）同样经 keyOf 归一 | 02 |
| ZQ-7 | P3 | 同 ZQ-1 全部位置 | 并入 keyOf 单点：键内 `\|` 转义（`replaceAll('|', '\\|')`，cliId 与 sessionId 两侧均转义）——复合键构造/解析唯一口径；L2 新增「cliId/sessionId 含 `\|` 时生产消费两侧键一致」用例 | 02 |
| ZQ-2 | P2 | `src/panels/terminal/useXterm.ts:358-361`、`src/features/agentStatus/useAgentStatus.ts:134-137`、`src/features/notifications/useAgentNotifications.ts:65-68` | **抽 `resolvePayloadCliId(payload)` helper 单点**（建议落点 `src/panels/terminal/resolvePayloadCliId.ts`——与 TerminalRegistry 同域，三消费方均有 import panels/terminal 先例）：空串与 null/undefined 同等回退（`payload.cliId \|\| agentSession?.cliId \|\| CLAUDE_CLI_ID`，含 trim 后空串判定）；三处改经 helper；L2 新增空串 cliId 回退用例（三消费方各一） | 02 |
| ZQ-3 | P1 | `src/features/agentStatus/useAgentStatus.ts:187-199`（建行 :194 写死 `newStatus ?? "attention"`） | **决策 2**：null 映射事件建行但 `status: null`（无图标）——`newStatus ?? "attention"` 改为 `newStatus`（AgentSessionRow.status 类型随行放宽 null）；更新已有行逻辑（:177 null 不覆盖）不变；L2 新增「null 映射事件首达建行 status=null 无图标」用例 + 「SessionStart 丢失感知存活」场景注释 | 02 |
| ZQ-4 | P2 | `src/features/agentHistory/restoreSession.ts:133` | panelId 加自增序号防毫秒碰撞：`terminal-${targetPageId}-${Date.now()}-${++restoreSeq}`（模块级计数器）；L2 新增同毫秒两次恢复 panelId 相异用例 | 02 |
| ZQ-6 | P3 | `src/panels/terminal/useXterm.ts:390-392` | `status === null` 分支清图标条件扩为 `SESSION_END_EVENT \|\| EXIT_EVENT`（:376 删 agentSession 已含 EXIT_EVENT，清图标对齐）；L2 新增 Exit 事件清图标用例 | 02 |

## 3. 测试质量（CS-1~3，来源 review-06）

| ID | 级别 | 位置 | 修复要点 | Stage |
|----|------|------|---------|-------|
| CS-1 | P1 | `src/__tests__/no-claude-literals.test.ts:129-140`（extractStringLiterals 模板分支） | 含 `${}` 模板字符串不再整体跳过：提取全部字面量片段拼接（忽略表达式）后走既有三类判定；新增自检用例（`cl${''}aude` 形态样例源码 → 守卫必报违规） | 01 |
| CS-2 | P1 | `src/__tests__/no-claude-literals.test.ts:39-47`（SCAN_DIRS） | 扫描范围加 `src/features/cliProfiles`（第 8 路径）+ **目录级豁免 `profiles/claude/`**（collectTsFiles 或扫描循环按路径前缀排除——该目录是 claude 合法领地）；扫描范围完整性断言同步（七→八路径）；describe 标题与头部注释「七路径」表述随行 | 01 |
| KZ-7 | P2 | `src/__tests__/helpers/mockCliProfile.ts:25-57`、`src/__tests__/mock-cli-profile.test.tsx:605-720`（AC-4④ 段）、:244-245（JsonMode mock） | mockCliProfile 声明自有 `configEditor` 桩组件（可识别渲染标记，如 `data-e2e="mockcli-config-editor"`）；AC-4④ 改为断言「选中 mockcli → 渲染 mock 编辑器桩、**未渲染** ClaudeHooksConfigEditor（JsonMode mock 零调用）」；选中 claude → 仍渲染 claude 编辑器（JsonMode 被调用）——双向分派断言 | 05 |
| CS-3 | P2 | `e2e-tests/mockcli.e2e.ts`（现仅 1 条 OSC 133 用例）、`e2e-tests/helpers.ts:345-375`（installMockCliProfile 补 configEditor 桩 + configLayers） | L4 补 mockcli 关键路径用例：① agent-event 信号注入（cliId="mockcli"）→ 页签 emoji + 活跃区行建模（信号文件 cliId 任意可写，全链路真实）；② hub 选择行渲染 mockcli 按钮 + 点击渲染 mock 编辑器桩 + 保存链路携带 mockcli cliId（断言后端「未知 cliId: mockcli」错误透传——证明 cliId 全链携带）。**review 建议的「历史条目展示/双击恢复注入」两条 L4 不可行（豁免）**：历史条目由后端 provider 打标产出，生产二进制仅 claude provider（cliId 恒 "claude"），无 mockcli 后端 provider 则造不出 mockcli 历史行；为测试在生产留后门代价过大——该链路 CLI 无关性由 L2 AC-4③/⑤ 覆盖（mock-cli-profile.test.tsx），豁免理由同步记入 test-inventory | 05 |

## 4. 扩展性（KZ-1~7，来源 review-08）

| ID | 级别 | 位置 | 修复要点 | Stage |
|----|------|------|---------|-------|
| KZ-1 | P0 | `src/panels/hooksConfig/HooksConfigPanel.tsx:32`（import）/:253-260（编辑器槽无条件渲染）；`src/features/cliProfiles/types.ts:12-25`（HooksCapability）；`src/features/cliProfiles/profiles/claude/index.ts`（挂载点） | **组件入 profile**（形态决策已定）：HooksCapability 新增 `configEditor` 组件字段（签名 = 现 ClaudeHooksConfigEditorProps 泛化：`{ profile; onDirtyChange?; askGuardRef? }`，hasConfigEditor=true 时必填）；claude profile 挂载 ClaudeHooksConfigEditor；hub 编辑器槽改 `selectedProfile.capabilities.hooks.configEditor` 分派渲染（缺失防御 → 空态占位）；选择行过滤条件（hasConfigEditor===true）不变 | 04 |
| KZ-2 | P1 | `src/types/agent.ts:26`；`src-tauri/src/hooks/signal.rs:33`；`src-tauri/src/hooks/claude/slterm-hook-reporter.js:50`；消费方 `useAgentStatus.ts`（:179-180/:196/:207-208/:264/:272-273/:340/:346-347）、`useXterm.ts:384`、`TerminalRegistry.ts:16/95`、`historyModel.ts:133-135`、`HistorySessionList.tsx:201-203` | **决策 1 全链路更名**：事件 DTO `transcriptPath: string` → `usageSourcePath?: string \| null`（可选）；后端 `transcript_path: String` → `usage_source_path: Option<String>`（serde default，**不加 alias**）；reporter payload 键更名 + SCRIPT_VERSION 2→3；前端内部状态 `AgentSessionInfo.transcriptPath` → `usageSourcePath` 随行；basename 回退逻辑语义不变；L1 serde 键集合 + L2 契约/全量 mock 字段 + L3 注释 + E2E 信号构造（camelCase 键）全部随行 | 03 |
| KZ-3 | P1 | `src-tauri/src/hooks/provider.rs:33`（trait 参数）；`src-tauri/src/hooks/mod.rs`（agent_context_usage 命令参数）；`src/ipc/agentHooks.ts:42-44`（wrapper）；`src-tauri/src/hooks/claude/usage.rs`（实现随行） | trait `context_usage(transcript_path)` → `context_usage(usage_source_path)`，trait 文档注明「路径语义由具体 CLI 解释」；命令参数 `transcriptPath` → `usageSourcePath`（Tauri camelCase 双边）；wrapper `contextUsage(cliId, usageSourcePath)`；契约测试参数结构断言同步 | 03 |
| KZ-4 | P1 | `src/types/hooksConfig.ts:8`（HooksLayer union）；`src/features/cliProfiles/types.ts`（HooksCapability 加 layers）；`src/panels/hooksConfig/ClaudeHooksConfigEditor.tsx:58-66`（LAYERS + PRIORITY_HINT）；`src/panels/hooksConfig/useHooksConfig.ts`（layer 状态/初始层/读写透传） | **profile 声明层集合**（形态决策已定）：HooksCapability 新增 `configLayers: { id: string; label: string; hint: string }[]`（hasConfigEditor=true 时必填；claude 值 = 现 LAYERS 三层）；`HooksLayer` 泛化为 `string`（注释注明值集由 profile 声明）；编辑器层切换器按 `profile.capabilities.hooks.configLayers` 渲染（LAYERS 常量退役，claude 三层值迁入 profile 定义）；useHooksConfig layer 状态类型随行（初始层 = configLayers[0]）；后端不动（trait layer 参数本为字符串，parse_layer 是 claude provider 内部校验） | 04 |
| KZ-5 | P2 | `src/panels/hooksConfig/ClaudeHooksConfigEditor.tsx:269/:285`（错误文案 `~/.claude/settings.json`） | **不独立改代码**——KZ-1 分派落地后该字面量位于 claude 专属编辑器合法领地（MC-223 决策 2），codex 接入时用自有编辑器；Stage 04 verify 消解断言（hub 无无条件 claude 编辑器渲染）即闭环 | 04（消解验证） |
| KZ-6 | P2 | `src/features/cliProfiles/CLAUDE.md:88-90`（「新增 CLI 步骤」） | 补四步：后端 hooks provider 注册（`hooks/provider.rs` REGISTRY）、后端 history provider 注册（`agent_history/provider.rs` REGISTRY）、`.claude/test-inventory.md` 用例数同步、`hasConfigEditor=true` 时新增编辑器组件并挂入 profile `configEditor`（含 configLayers 声明）；按 Stage 03/04 终态撰写 | 06 |

## 5. 文档一致性（YS-1~5 来源 review-01；WD-1~4 来源 review-07）

| ID | 级别 | 位置 | 修复要点 | Stage |
|----|------|------|---------|-------|
| YS-1 | P2 | `src/panels/CLAUDE.md:249`（F3 节 useCommandDetection 描述 `cliIconRegistry.match`） | 改为与同文件 :275 文件表一致的新描述（`cliProfileRegistry.matchByCommand` 命中后 `logo = profile.iconSrc`）；同文件矛盾消除 | 06 |
| YS-2 | P3 | `src/__tests__/terminal.test.tsx:292`、`src/__tests__/use-xterm-lifecycle.test.ts:1363` | 两处注释 `CliIconRegistry.match("claude")` 字样 → 现行 API（`cliProfileRegistry.matchByCommand`）或删除过时注释 | 06 |
| YS-3 | P3 | `src/features/sideViews/CLAUDE.md:5/:28/:46/:125`、`src/theme/CLAUDE.md:19`、`src/theme/schemeRegistry.ts:4-5`、`src/features/cliProfiles/CLAUDE.md:17/:29` | 7 处「TabTitleRegistry 模式先例」改指现存注册表（CliProfileRegistry 或 ShortcutRegistry），或删先例引用只留自身机制描述（迁移溯源交代两处合法保留：panels/CLAUDE.md:275、cliProfiles/CLAUDE.md:29 后半——逐处甄别） | 06 |
| YS-4 | P3 | `src/theme/schemes/types.ts:46` | 注释 `claudeHistory` → `agentHistory` | 06 |
| YS-5 | P3 | `src/features/agentStatus/AgentStatusRow.tsx:55` | 删除「与原 cliIconRegistry.getSrc 语义一致」对照半句，或改自含描述（按行 cliId 查 profile.iconSrc，未注册不报错） | 06 |
| WD-1 | P3 | `.claude/CLAUDE.md:178-183`（需求编号索引表） | 追加 F9 行：`\| F9 \| 特性 \| 终端页签/侧栏 CLI 品牌 logo（按命令行首 token 匹配 profile.iconSrc）\|` | 06 |
| WD-2 | P3 | `src/panels/CLAUDE.md:152`、`src/features/hooksConfig/CLAUDE.md:21`、`src/features/hooksConfig/schema/index.ts:15` | 三处「补全/悬停/波浪线」→「悬停/波浪线」（JSON 模式无自动补全——2026-08-01 验收后决策删除，JsonMode.tsx:6-8 注释为准） | 06 |
| WD-3 | P3 | `CONTEXT.md:75-76` | 改为「前端为统一的 `CliProfileRegistry`；后端按能力拆分为 hooks/history 两个 cliId 键注册表（分别见 `hooks/provider.rs` 与 `agent_history/provider.rs`）」 | 06 |
| WD-4 | P3 | `src/features/agentHistory/AgentHistorySections.tsx:49` | 注释 `Map<sessionId, status>` → `Map<cliId\|sessionId, status>`（复合键 MC-313） | 06 |

## 计核对

| 族 | 条数 | Stage 分布 |
|----|------|-----------|
| 安全 AQ + ZQ-5 | 5 | 01 |
| 测试守卫 CS-1/2 | 2 | 01 |
| 正确性 ZQ（除 ZQ-5） | 6 | 02 |
| 扩展性 KZ-2/3 | 2 | 03 |
| 扩展性 KZ-1/4/5 | 3 | 04 |
| 测试验收 KZ-7/CS-3 | 2 | 05 |
| 文档 YS×5 + WD×4 + KZ-6 | 10 | 06 |
| **合计** | **30** | |
