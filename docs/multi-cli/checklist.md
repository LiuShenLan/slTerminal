# multi-cli profile 重构 — 逐 ID 清单

> 真值源：`docs/spec/00~06`（已定稿）。本清单逐 MC-ID 枚举，Stage 划分见 `stages.md`。
>
> **组织约定**：
> - 优先级不用 P0–P4——**由 Stage 依赖顺序表达**（建议序细化见 stages.md 头部）
> - 每条含：位置（Glob/grep 实查路径，非凭记忆）+ 要点 + grilling 决策结论（凡涉及）
> - **衍生项 D-NN**：无 MC 编号但由共享字面量消费方 grep 全仓扫描产生（含测试文件 / L3 / E2E / 全局 mock），单列末段
> - 位置列「退役」= 文件删除；「新建」= 新增文件；行号为实查锚点

## 0. 总规格项（MC-1~8）

| ID | 位置 | 要点 | Stage |
|----|------|------|-------|
| MC-1 | 前端 `src/features/cliProfiles/`（新建）；后端 `src-tauri/src/hooks/` + `agent_history/`（注册表） | 前后端双侧 CLI profile 注册表，cliId 公共键；能力可选（未声明=该域不可用，消费方优雅降级） | 01/03/04 |
| MC-2 | 全仓 | claude 迁移为首个 profile，全部现有行为经 profile 驱动且零回归（AC-1：L1–L4 全绿） | 全 Stage 门禁 |
| MC-3 | 全仓 | 领域机制（OSC 133 消费/信号通道/恢复编排/聚合 UI）保持实现现状，改 profile 数据/策略驱动；**不为抽象重写已验证机制** | 全 Stage 纪律 |
| MC-4 | 通用层 | 消费方一律经 profile 注册表取能力；禁止 profile 体系外新增 claude 字面量（字符串/事件名/路径） | 07（AC-5 守卫） |
| MC-5 | spec 00 §6 迁移点总清单 | 命名全面去 claude 化（类型/字段/文件/目录/命令/事件名/用户可见文案/E2E 断言） | 各 Stage 分摊 |
| MC-6 | 测试夹具 | mock profile 全链路验证接入清单（AC-4，非真实 CLI） | 07 |
| MC-7 | 全仓 | 遵守根 CLAUDE.md 硬约束 #1–#11（IPC 边界/DTO 双边/面板封闭/配色单点/测试覆盖等） | 全 Stage 纪律 |
| MC-8 | `.claude/test-inventory.md`、各模块 CLAUDE.md、`CONTEXT.md` | 用例增删改同步 test-inventory；模块结构变动同步子路径 CLAUDE.md；术语变动同步 CONTEXT.md | 08（各 Stage 就近同步模块 CLAUDE.md） |

## 1. 身份识别与终端适配（MC-101~110，来源 spec 01）

| ID | 位置 | 要点 | Stage |
|----|------|------|-------|
| MC-101 | 新建 `src/features/cliProfiles/CliProfileRegistry.ts` | 模块级单例（项目第 6 个注册表，照 TabTitleRegistry 模式）：`register(profile)`（同 id 覆盖）/ `get(id)` / `getAll()`（注册序）/ `matchByCommand(commandLine)` / `_reset()`（仅测试） | 01 |
| MC-102 | `src/lib/cliIcons.ts:30-33` + `src/panels/terminal/TabTitleRegistry.ts:41-44` | **首 token 解析单点化**：`trim().split(/\s+/)[0]` 两份拷贝收敛为注册表内部唯一实现；`matchByCommand` 对 profile.commands 逐键精确查表；空命令行/仅空白 → null；不 toLowerCase | 01 |
| MC-103 | profile 类型定义 | `commands: string[]` 支持多首 token（如 `["claude","cc"]`）；claude 当前仅 `["claude"]` | 01 |
| MC-104 | 新建 `src/features/cliProfiles/profiles/claude/` + `profiles/index.ts`；退役 `src/lib/cliIcons.ts`、`src/panels/terminal/tabRules.ts` | claude profile 身份域：`commands:["claude"]`、`iconSrc:"/cli-icons/claude.png"`、`tabTitle:"claude"`；注册文件照 tabRules side-effect 先例（`profiles/index.ts` 追加 `import "./claude"`）；`public/cli-icons/claude.png` 保留原位 | 01 |
| MC-105 | `src/panels/terminal/useCommandDetection.ts:53-58`（OSC 133 C 消费点） | 改经 `matchByCommand` 取 profile：title=profile.tabTitle、logo=profile.iconSrc；未命中零副作用（现状 `rule==null` 分支保留）；`TabState.logo` 保留（TerminalPanel 消费链不变，值来源改 profile.iconSrc） | 01 |
| MC-106 | `useCommandDetection.ts:56` | `icon: "🟡"` 字面量 → `STATUS_EMOJI.attention` 引用（「四态映射单点」例外点收敛） | 01 |
| MC-107 | `useCommandDetection.ts`（setClaudeSession 调用点） | 命中后 `setAgentSession(panelId, { cliId: profile.id, matchedCommand })`——cliId 从匹配 profile 取；依赖 TerminalRegistry 更名（MC-402），故落 Stage 02 | 02 |
| MC-108 | `src/__tests__/cli-icons.test.ts:77-84` | logo 资源守卫泛化：遍历注册表全部 profile 断言 iconSrc 磁盘存在 + PNG 魔数（img 404 无报错通道，资源缺失靠此守卫）；随三注册表合并迁入 profile 测试 | 01 |
| MC-109 | `src-tauri/src/pty/CLAUDE.md`、`src/panels/CLAUDE.md` | **代码零改动**，仅文档重新归类：DA1/COLORTERM/合帧/resize/OSC 52/Kitty/Ctrl+Enter 等「claude 定制」表述 →「终端平台能力（设计动机 Ink 系 TUI，对全部子进程生效）」；触发点描述（「供 claude 取消」等）保留 | 08 |
| MC-110 | `src-tauri/src/pty/spawn.rs`（env 注入点） | `SLTERM_PANEL_ID` 保留为通用每终端路由键，不移除；「无此变量 exit(0)」门控语义归各 CLI reporter 实现——**文档记录项** | 08 |

## 2. hooks 信号链路（MC-201~223，来源 spec 02）

### 通道层

| ID | 位置 | 要点 | Stage |
|----|------|------|-------|
| MC-201 | `src-tauri/src/hooks/signal.rs` | `HookEventPayload` → `AgentEventPayload`：8 字段（panelId/event/timestamp/sessionId/transcriptPath/cwd/toolName/notificationType）语义不变 + **新增可选 `cliId`**（serde `default`，缺省前端按 "claude" 兼容）；serde 键集合测试同步 | 03 |
| MC-202 | `src-tauri/src/hooks/signal.rs`（emit 点）；`src/ipc/hooks.ts`（listen 封装）；三消费方 `useXterm.ts` / `useAgentStatus.ts` / `useClaudeNotifications.ts`；`src/__tests__/setup.ts:92-93`（全局 mock，**衍生 D-01**） | 广播 `hook-event` → `agent-event`；前端 `onHookEvent` → `onAgentEvent`（`listen<AgentEventPayload>("agent-event")` 照 onFsEvent 模式）；三消费方同步迁移；**setup.ts 全局 mock 路径同步**（漏则全测试炸） | 03 |
| MC-203 | `src-tauri/src/hooks/watcher.rs` | watcher 双通道（notify 50ms debounce + 3s 轮询补漏 + 目录自动重建）**零行为改动**；信号目录 `~/.slterminal/hooks-events/` 单目录全 CLI 共用（路由靠 payload.panelId + cliId，不分目录）；**勿削弱轮询补漏**（win10 实证防线） | 03（仅核对，零改动） |
| MC-204 | `src-tauri/src/hooks/signal.rs`（process_signal_file） | 读→emit→删契约不变；解析失败/emit 失败仍删文件的容错语义不变 | 03（仅核对，零改动） |
| MC-205 | 前端事件消费点（useXterm / useAgentStatus / 通知调度） | profile 解析顺序：`payload.cliId`（显式）→ `TerminalRegistry.get(panelId)?.agentSession?.cliId`（反查）→ `"claude"`（缺省兼容旧信号）；**缺省值经 `profiles/claude/` 导出常量（如 `FALLBACK_CLI_ID`）引用，通用层不留 "claude" 字面量**（AC-5 兼容） | 02 落地三级解析（payload.cliId 恒 undefined 向后兼容）→ 03 后端加字段后自然生效 |
| MC-206 | 前端事件消费点 | 未知 cliId（未注册）→ `console.warn` + 跳过（不建行/不置图标/不通知），不抛异常 | 02 |

### 能力层（后端命令泛化）

| ID | 位置 | 要点 | Stage |
|----|------|------|-------|
| MC-210 | 新建 `src-tauri/src/hooks/provider.rs`（trait + 注册表）；`src-tauri/src/hooks/claude/`（下沉 `inject.rs`/`usage.rs`/`config.rs`） | `CliHooksProvider` trait 六方法：`inject()/uninstall()/injection_status()/context_usage(transcript_path)/config_read(layer, project_path)/config_write(layer, hooks, project_path)`；静态注册表以 cliId 为键；claude 为首个实现（行为零改动） | 03 |
| MC-211 | `src-tauri/src/hooks/mod.rs`（命令层）+ `lib.rs`（注册） | 6 命令泛化：`agent_hooks_inject(cliId)` / `agent_hooks_uninstall(cliId)` / `agent_hooks_injection_status(cliId)` / `agent_context_usage(cliId, transcriptPath)` / `agent_hooks_config_read(cliId, layer, projectPath?)` / `agent_hooks_config_write(cliId, layer, hooks, projectPath?)`；未知 cliId → `AppError::Validation`；无 hooks 能力 cliId → `Validation`（消息含「不支持 hooks 能力」语义）；旧命令不保留兼容（D10） | 03 |
| MC-212 | `src/ipc/hooks.ts` → `src/ipc/agentHooks.ts`；`src/types/hooks.ts` → `src/types/agent.ts`；`src/__tests__/ipc-hooks-contract.test.ts`（22 用例） | wrapper 加 cliId 首参；**决策 3：类型更名** `AgentEventPayload`/`ContextUsage`/`InjectionStatus`→`AgentInjectionStatus`/`HookInjectionStatus`→`AgentHookInjectionStatus`；契约测试四维同步（命令名/参数含 cliId camelCase/返回/异常） | 03 |
| MC-213 | `src-tauri/src/hooks/claude/`（下沉后） | provider 内部全部保留 claude 命名与 claude 知识：`HOOK_EVENTS` 10 事件、`~/.claude/settings.json`、matcher 结构、SCRIPT_VERSION 检测、reporter 模板、三层配置路径——**provider 内部是 claude 合法领地**（D11） | 03 |
| MC-214 | `src-tauri/src/hooks/claude/usage.rs`（下沉）；`src/features/agentStatus/consts.ts`（退役） | `ContextUsage` DTO 四字段保留（input/output/cacheRead/cacheCreation，cache serde default 0）；用量口径 `/contextLimit` 中 contextLimit 由 `profile.hooks.contextLimit` 提供（claude=200_000）；`CLAUDE_CONTEXT_LIMIT` 退役（前端落点 MC-413） | 03（后端）+ 02（前端） |
| MC-215 | `src-tauri/assets/slterm-hook-reporter.js` → 随 `hooks/claude/` 迁移（include_str! 路径同步） | reporter 归 claude provider 资产；**决策 7：payload 显式写 `cliId: "claude"`，接受 SCRIPT_VERSION 递增 → 已注入用户变「版本过旧」需重新注入，测试锁死此形态**；C10 契约（任何路径 exit 0 不写 stderr）与 E2E-06 端到端守卫不得削弱 | 03 |

### 配置面板联动（与 05 域衔接）

| ID | 位置 | 要点 | Stage |
|----|------|------|-------|
| MC-220 | `src/panels/hooksConfig/useHooksConfig.ts`（IPC 调用点） | `readHooksConfig/writeHooksConfig` 加 cliId 首参（值 = hub 面板当前选中 CLI）；依赖 Stage 03 泛化命令 | 06 |
| MC-221 | `src/panels/hooksConfig/HooksConfigPanel.tsx`（注入/卸载按钮） | 调 `agent_hooks_inject/uninstall(selectedCliId)`；注入后自动重读 user 层（C13-8）语义不变 | 06 |
| MC-222 | `HooksConfigPanel.tsx`（保存提示条） | 「hooks 改动需重启 claude 会话生效」→ `profile.hooks.restartHint` 驱动（claude 值同现状文案）；`data-e2e="hooks-restart-hint"` 选择器保留 | 06 |
| MC-223 | `src/panels/hooksConfig/` + `src/features/hooksConfig/` | claude hooks 协议知识（eventsCatalog 30 事件/matcherEngine/5 种 handler 字段矩阵/schema 内嵌/Draft07 校验）**不抽象**；**决策 2：文件物理位置保留现状**，模块 CLAUDE.md 注明「claude 专属编辑器」语义 | 06（仅文档注明） |

### E2E 基建（衍生自 spec 02 §5 L4 行）

| ID | 位置 | 要点 | Stage |
|----|------|------|-------|
| （决策 4） | `e2e-tests/run-wdio.cjs` | E2E-05 用户目录隔离备份集合（`~/.claude/settings.json` + `~/.slterminal/hooks/` + `hooks-events/`）**保持 claude 硬编码 + 注释「随第二 CLI 接入扩展」**（规格「二选一」取后者降范围） | 03 |

## 3. 历史会话（MC-301~318，来源 spec 03）

### 后端

| ID | 位置 | 要点 | Stage |
|----|------|------|-------|
| MC-301 | `src-tauri/src/claude_history/`（4 文件）→ `src-tauri/src/agent_history/`（聚合层 + trait + `claude/` provider） | 聚合层（provider 注册表 + 命令）+ `CliHistoryProvider` trait（`scan() -> Vec<AgentHistorySession>` / `delete(session_id) -> Result<()>` / `validate_session_id(id) -> Result<()>`）+ `claude/` provider（scan.rs/jsonl.rs/ops.rs 整体下沉，行为零改动；`is_uuid_filename` 作为可复用工具保留） | 04 |
| MC-302 | `agent_history/mod.rs`（DTO）+ `src/types/agentHistory.ts` | `AgentHistorySession`：七字段（sessionId/cwd/title/titleSource/firstPrompt/mtimeMs/cwdExists）+ **新增 `cliId`**（provider 打标，serde camelCase）；`titleSource` 五变体枚举 → **开放字符串**（claude 值集 customTitle/aiTitle/summary/firstPrompt/none；UI 不消费具体值） | 04 |
| MC-303 | `agent_history/`（命令层）+ `lib.rs`（注册） | `agent_history_scan()` **无参聚合**——遍历全部已注册 provider，单 provider 失败不阻塞其他（降级条目契约的语义层级提升）；`agent_history_delete(cliId, sessionId)`：未知 cliId → Validation；delete 前经该 provider `validate_session_id` 前置 | 04 |
| MC-304 | `agent_history/claude/`（ops.rs 下沉） | SEC-05 保留：UUID 形态校验 + `locate_session_jsonl` 遍历定位（前端不传任何路径）；trait 契约写明「validate_session_id 是 delete 的强制前置」——未来 provider 等价校验强制 | 04 |
| MC-305 | `agent_history/claude/scan.rs`（resolve_projects_root） | env 覆盖 `SLTERM_CLAUDE_PROJECTS_DIR` 留 provider 内部（每次调用读 env 不缓存）；聚合层不假设 env 命名——未来 `SLTERM_<CLI>_PROJECTS_DIR` 同款模式自管 | 04 |
| MC-306 | `src/ipc/claudeHistory.ts` → `agentHistory.ts`；`src/types/claudeHistory.ts` → `agentHistory.ts`；`src/__tests__/ipc-claude-history-contract.test.ts`（8 用例） | wrapper/类型更名；契约同步（scan 无参 / delete 参数 `{cliId, sessionId}` camelCase） | 04 |

### 前端聚合与恢复

| ID | 位置 | 要点 | Stage |
|----|------|------|-------|
| MC-310 | `src/features/claudeHistory/`（9 文件）→ `src/features/agentHistory/` | 聚合 UI 泛化为 CLI 无关；**组件/Hook 同步更名**（MC-5）：`ClaudeHistorySections`→`AgentHistorySections`、`useClaudeHistory`→`useAgentHistory`；barrel `index.ts` 同步（衍生 D-05）；宿主 `AgentStatusView.tsx` import 同步 | 05 |
| MC-311 | `HistorySessionRow.tsx:54` | `cliIconRegistry.getSrc("claude")` → 按 `session.cliId` 查 profile.iconSrc；logo 仍「仅随 status emoji」（孤儿 ✗ 后不加图）；未注册 cliId → 无 logo 不报错 | 05 |
| MC-312 | `historyModel.ts`（groupByCwd） | 分组维度保持 cwd 不变；同目录不同 CLI 同组、行级 logo 区分；「全部项目」区聚合全部 provider 数据（scan 无参聚合直达 UI，无前端二次过滤） | 05（仅核对，零改动） |
| MC-313 | `historyModel.ts:123-137`（deriveActiveSessionStatuses）+ `HistorySessionList.tsx:192-204`（findPanelForSession） | **复合键 `cliId\|sessionId`**（防跨 CLI sessionId 理论冲突）；transcriptPath basename 回退兼容保留（旧数据无 cliId 按 claude——经 FALLBACK_CLI_ID 常量，不留字面量） | 05 |
| MC-314 | `AgentStatusView.tsx:126-141`（titleBySessionId） | 键同步改 `cliId|sessionId`；claude `/rename`→custom-title→scan 联动语义保留在 claude provider + claude 数据路径 | 05 |
| MC-315 | `restoreSession.ts`（:130 字面量） | 四步框架零改动；第 4 步注入内容 = `profile.history.buildRestoreInput(session, { fork })`（claude = `claude --resume <id>` + fork 追加 ` --fork-session`）；addPanel `title: "claude"` → `profile.tabTitle` | 05 |
| MC-316 | `historyContextMenu.ts:51-54` | `buildResumeCommand` → `profile.history.buildResumeCommand(session)`；操作矩阵禁用态（孤儿/无 cwd 禁分支恢复、运行中禁删除）通用语义保留；`supportsFork=false` 不展示「分支恢复」；cwd 单引号限制（PowerShell `''` 转义缺失）留 claude buildResumeCommand 内部注释 | 05 |
| MC-317 | `HistorySessionList.tsx`（删除/双击分派） | 删除（ask → `agent_history_delete(cliId, sessionId)` → removeLocal 不重扫）与双击三分派（普通→恢复 / 孤儿·无 cwd→无操作 / 运行中→SessionActionDialog）语义不变 | 05 |
| MC-318 | 文档记录项 | 组键漂移（expandedGroups 键随组内最大 mtime 会话漂移）+ 历史区相对时间无 ticker——**决策 6：纳入为已知限制文档记录项，不修**（记录于 agentHistory 模块 CLAUDE.md） | 08 |

## 4. UI 状态与通知（MC-401~422，来源 spec 04）

### 状态模型与注册表

| ID | 位置 | 要点 | Stage |
|----|------|------|-------|
| MC-401 | `src/lib/claudeStatus.ts` → `src/lib/agentStatus.ts`；`src/lib/index.ts` barrel（衍生 D-02）；`src/__tests__/claude-status.test.ts`（32 用例） | 保留 `AgentStatus` 类型（四态值集不变）、`STATUS_EMOJI`、`getStatusIcon`；**`eventToStatus` 与 `ATTENTION_NOTIFICATION_TYPES` 移出**，作为 claude profile `hooks.eventToStatus` 实现（迁入 `profiles/claude/`，用例语义不丢、落点改 profiles/claude/ 测试）；lib 层不再含 claude 事件名字面量 | 02 |
| MC-402 | `src/panels/terminal/TerminalRegistry.ts`；测试 `terminal-registry.test.ts` + `terminal-registry-subscribe.test.ts`（衍生 D-12） | `ClaudeSessionInfo`→`AgentSessionInfo`：字段 sessionId/transcriptPath/matchedCommand/status/lastEventAt 保留 + **新增 `cliId: string`**（OSC 133 C 命中时写入，MC-107）；`claudeSession`→`agentSession`、`setClaudeSession`→`setAgentSession`（merge 语义/null 清空/undefined 不覆盖/缺 lastEventAt 自动填/register 幂等保留旧值——全保留）；「存在即运行中」二态模型不变 | 02 |
| MC-403 | `src/panels/terminal/useXterm.ts`（hook-event 消费，~348-357） | 按 MC-205 解析 profile → `profile.hooks.eventToStatus(event, notificationType)`；无 hooks 能力 profile → `console.warn` + 跳过；SessionEnd 清图标、Exit 清会话分支语义不变；`setAgentSession` 携 sessionId/transcriptPath/status + **payload 空串归一 `|| undefined` 防御保留** | 02 |
| MC-404 | `src/features/notifications/`（CATEGORY_EMOJI）+ `src/lib/agentStatus.ts`（STATUS_EMOJI） | 两常量集**不合并**——通知类别 emoji（🔐❌✅）vs 会话状态 emoji（⚡🟡✅❌），值有重叠但语义不同；注释互相指引 | 02 |
| MC-405 | `src/workspace/PageDockviewHost.tsx`（createGetContextMenu 内 F8 禁用判定，**实查落点**）；测试 `workspace-header-actions.test.tsx`（衍生 D-06） | `TerminalRegistry.get(panelId)?.agentSession != null`（字段更名后语义不变）；任何 CLI 活跃会话均禁用重命名；OSC 133-only 会话（matchedCommand-only）同样命中 | 02 |
| MC-406 | `src/workspace/PageDockviewHost.tsx`（DefaultTab） | 渲染链（tabIcon emoji / tabLogo 16×16 / `tabIcon && tabLogo` 双条件 / inactive 双清）零改动；值来源变化仅在生产侧（OSC 133 C logo 来自 profile.iconSrc，MC-105） | 02（仅核对，零改动） |

### AgentStatus 行与用量

| ID | 位置 | 要点 | Stage |
|----|------|------|-------|
| MC-410 | `src/features/agentStatus/useAgentStatus.ts` | 行建模双通道建行/三通道删行语义不变；hook 事件通道建行按 MC-205 解析 cliId 写入行；OSC 133 通道建行经 setAgentSession 的 sessionChange 自然驱动；初始扫描携 transcriptPath 拉 contextUsage 保留 | 02 |
| MC-411 | `src/features/agentStatus/AgentStatusRow.tsx` | `cliIconRegistry.getSrc("claude")` → 按 `row.cliId` 查 profile.iconSrc；图标列 40px flex 簇 /「logo 仅随 emoji」/ 空列占位布局语义不变；OSC 133-only 行同样有 cliId（logo 可展示） | 02 |
| MC-412 | `AgentStatusRow.tsx`（用量计算） | 用量口径 `(inputTokens + cacheReadInputTokens + cacheCreationInputTokens) / contextLimit`：**contextLimit 由行 cliId 对应 `profile.hooks.contextLimit` 提供**；无 hooks 能力行无 transcriptPath → 用量 `--`（现状语义保留） | 02 |
| MC-413 | `src/features/agentStatus/consts.ts`（**删除文件**）；`useAgentStatus.ts` / `AgentStatusRow.tsx` | consts.ts 随 CLAUDE_CONTEXT_LIMIT 退役删除；行组件/hook 残留 claude 命名（类型/注释/变量）全面去 claude 化（D11），行为零改动 | 02 |
| MC-414 | `src/features/agentStatus/AgentStatusView.tsx`（空态）；`e2e-tests/agent.e2e.ts`（断言同步） | 空态文案「无运行中的 claude 会话」→「**无运行中的编码 CLI 会话**」（本域唯一用户可见文案变动）；E2E 红线 `data-e2e="agent-status-view"`/`agent-status-row`/标题栏 "AGENT STATUS"/「选择一个项目」**逐字保留** | 02 |

### 通知调度

| ID | 位置 | 要点 | Stage |
|----|------|------|-------|
| MC-420 | `src/features/notifications/useClaudeNotifications.ts` → `useAgentNotifications.ts`；`index.ts` barrel（衍生 D-04）；`src/__tests__/notifications.test.ts` | `classifyEvent` 纯函数**两段分解**：通用门控（失焦门控/去重/seenRef 截断，CLI 无关保留模块内）+ **类别判定委托 `profile.hooks.classifyNotification(payload)`**（按 MC-205 解析 profile；无 hooks 能力 → 不通知）；返回类别 permission/error/done/null 语义不变 | 02 |
| MC-421 | `useAgentNotifications.ts` | toast 正文项目名反查（panelId → parseTerminalPageId → useProjects）与任务栏闪烁零改动；类别 emoji 用 CATEGORY_EMOJI | 02 |
| MC-422 | claude 类别判定知识 → `profiles/claude/`（classifyNotification 实现） | 五映射迁入：PermissionRequest / Notification+permission_prompt / StopFailure / PostToolUseFailure / Stop；行为零改动 | 02 |

## 5. hooks 配置面板（MC-501~508，来源 spec 05）

| ID | 位置 | 要点 | Stage |
|----|------|------|-------|
| MC-501 | `src/workspace/pageApis.ts`（openHooksConfigPanel）+ `src/features/sidebar/SidebarTree.tsx`（菜单） | 面板 id `hooksConfig-{activePageId}` 同页单例语义不变（C13-7）；侧栏右键菜单「打开 Hooks 配置」保持单一入口（先切页 → 查重聚焦/新建，流程**零改动**）；面板标题「Hooks 配置」已 CLI 中立不改 | 06（仅核对，零改动） |
| MC-502 | `src/panels/hooksConfig/HooksConfigPanel.tsx`（改造为 hub 容器） | **顶部 CLI 选择行**：遍历 `CliProfileRegistry.getAll()` 过滤 `capabilities.hooks?.hasConfigEditor === true`，渲染按钮（iconSrc 16×16 logo + displayName）；选中态背景高亮走 theme token（硬约束 #6）；点击切换下方编辑器；**单 CLI 也渲染选择行**（边界 1，防布局跳动） | 06 |
| MC-503 | hub 容器 | **选中态持久化**：`params.selectedCli` 随布局 JSON 持久化（照 F8 customTitle 先例——`api.updateParameters({...params, selectedCli})` + **显式 `onLayoutChange(saveLayout(api))`**，updateParameters 不触发 onDidLayoutChange 须显式保存）；挂载时读 params 恢复；**缺省/失效回退**首个有能力 CLI | 06 |
| MC-504 | hub 容器 + claude 编辑器组件（现有 HooksConfigPanel 内容整体下移一层） | 选择行下方渲染选中 CLI 的配置编辑器：claude = 现有全部内容（层级切换/GUI·JSON 双模式/注入状态条/保存/重启提示条）作为 claude 编辑器组件迁入 hub；IPC 经泛化命令携 selectedCliId（MC-220/221） | 06 |
| MC-505 | hub 容器 | 切换 CLI = 卸载当前编辑器并重挂载目标编辑器（照 ADR-0001 先例——dirty/选中态丢弃）；**dirty 守卫**：dirty 时切换需 `dialog.ask` 确认丢弃（照切层/visibilitychange ask 守卫先例，askGuard 防循环复用） | 06 |
| MC-506 | hub 容器 | 保存成功提示条 `profile.hooks.restartHint` 驱动（MC-222）；`data-e2e="hooks-restart-hint"` 保留；注入状态条三态语义不变，数据源 `agent_hooks_injection_status(selectedCliId)` | 06 |
| MC-507 | hub 容器 | 选择行空态：无任何 hasConfigEditor profile → 渲染「无可配置 CLI」占位，不渲染编辑器（防御分支） | 06 |
| MC-508 | `src/panels/hooksConfig/` + `src/features/hooksConfig/`（11+1 文件） | claude 编辑器内部（层级切换/GUI/JSON/eventsCatalog/matcherEngine/schema/注入段保护/F2 注入按钮）**行为零改动**；文件物理位置保留（决策 2）；模块 CLAUDE.md 注明「claude 专属」 | 06 |

## 6. mock profile 与守卫（来源 spec 06 + 00 §5）

| ID | 位置 | 要点 | Stage |
|----|------|------|-------|
| AC-4 | 测试夹具 | mock profile 约定：id `"mockcli"`、commands `["mockcli"]`、hooks+history 全能力（恒等/桩策略）、hasConfigEditor=true + 桩编辑器组件；**仅测试环境注册**（vitest register + `_reset`；L4 经 E2E helper，`E2E_ENABLED` 门控红线不变）；**决策 5：真实最小 PNG 放 `public/cli-icons/mockcli.png`**（资源守卫统一零特例）。验证点全表：① OSC 133 命中页签/logo/agentSession.cliId ② eventToStatus/classifyNotification 被真实调用 ③ 历史聚合条目+logo ④ hub 选择行两枚按钮+切换+持久化 ⑤ 恢复注入内容=mock 策略输出 | 07（PNG 资源 Stage 01 先行放入） |
| AC-5 | 新建 `src/__tests__/no-claude-literals.test.ts`（L2 grep 守卫形态） | 通用层（`src/lib`、`src/panels/terminal`、`src/features/agentStatus`、`src/features/agentHistory`、`src/features/notifications`、`src/ipc`、`src/types`）不出现 claude 事件名字面量（SessionStart/PreToolUse 等）与 `~/.claude` 路径、"claude" 字符串字面量；claude 字面量仅存在于 `profiles/claude/` 与后端 claude provider；**豁免写法**：缺省回退经 profiles/claude 导出的常量引用（MC-205/313） | 07 |

## 7. 衍生项（D-NN，grep 全仓消费方实查产生，含测试文件）

> 来源：任务期 grep 14 组共享字面量全仓扫描（`src/`、`test/`、`e2e-tests/`）。每项注明触发更名的 MC 条目。

| ID | 位置 | 要点 | 触发源 | Stage |
|----|------|------|--------|-------|
| D-01 | `src/__tests__/setup.ts:92-93` | **全局 mock 路径同步**：`vi.mock("../ipc/hooks")` → `vi.mock("../ipc/agentHooks")`，`onHookEvent` → `onAgentEvent`——漏改则全局 mock 失效、L2 大面积炸 | MC-202/212 | 03 |
| D-02 | `src/lib/index.ts`（barrel） | `cliIcons` 导出退役（Stage 01）；`claudeStatus` → `agentStatus` 导出名更名（Stage 02） | MC-104/401 | 01/02 |
| D-03 | `src/types/index.ts`（barrel） | `hooks` → `agent`（Stage 03）；`claudeHistory` → `agentHistory`（Stage 04）导出名更名 | MC-212/306 | 03/04 |
| D-04 | `src/features/notifications/index.ts`（barrel） | `useClaudeNotifications` → `useAgentNotifications` 导出名更名；`App.tsx` 的 `NotificationListener` import 路径同步 | MC-420 | 02 |
| D-05 | `src/features/claudeHistory/index.ts`（barrel）+ `src/features/agentStatus/AgentStatusView.tsx`（import） | 目录更名后 barrel 与宿主 import 同步（`useClaudeHistory`→`useAgentHistory`、`ClaudeHistorySections`→`AgentHistorySections`） | MC-310 | 05 |
| D-06 | `src/workspace/PageDockviewHost.tsx` + `src/__tests__/workspace-header-actions.test.tsx` | F8 禁用判定的 `claudeSession` 引用更名（与 MC-405 同文件同测试） | MC-402/405 | 02 |
| D-07 | `src/workspace/Workspace.tsx` | side-effect import `tabRules` 退役 → 改 import `features/cliProfiles/profiles`（注册触发点） | MC-104 | 01 |
| D-08 | `test/terminal/production-osc.test.ts`（L3，8 用例） | OSC 133 复刻段按生产实现改写（原复刻 TabTitleRegistry/CliIconRegistry 匹配逻辑 → 复刻 matchByCommand/profile 取值）；逐段来源行号注释同步 | MC-105 | 01 |
| D-09 | `e2e-tests/helpers.ts` | E2E helper `__slterm_e2e_injectHooks` 等调 `hooks.inject()` 系列 → 泛化 wrapper（cliId 参数，helper 内固定 "claude"）；`E2E_ENABLED` 内联门控红线不动 | MC-211/212 | 03 |
| D-10 | `e2e-tests/specUtils.ts` | hooks 注入辅助（`hooks_inject` 等命令名/ wrapper 引用）同步泛化 | MC-211/212 | 03 |
| D-11 | `e2e-tests/run-wdio.cjs` | E2E-05 备份集合硬编码 + 注释（决策 4，与 spec 02 §5 L4 行同源） | MC-211 | 03 |
| D-12 | `src/__tests__/terminal-registry-subscribe.test.ts` | TerminalRegistry 订阅测试（register/remove/sessionChange）随 MC-402 更名同步 | MC-402 | 02 |
| D-13 | `src/__tests__/e2e-gating-terminal.test.ts` | E2E helper 门控断言（useXterm/useTerminalInstance 引用点）随 Stage 01/02 改动同步核对 | MC-105/403 | 01/02 |
| D-14 | `e2e-tests/agent.e2e.ts` / `hooks.e2e.ts` / `history.e2e.ts` | E2E 断言随命名迁移分 Stage 同步：agent.e2e（空态文案 MC-414 → Stage 02；事件名 → Stage 03）；hooks.e2e（命令名 → Stage 03；hub 面板用例 → Stage 06）；history.e2e（命令名 → Stage 04；恢复注入 → Stage 05） | MC-5 | 02/03/04/05/06 |
| D-15 | `src/__tests__/open-hooks-config-panel.test.ts` + `sidebar-actions.test.ts` + `default-layout-format.test.ts` | hub 面板入口相关测试——面板 id/入口零改动（MC-501），**预期零改动**；若 hub 改造波及面板注册参数则同步核对 | MC-501 | 06 |

## 8. 消费方 grep 实查底账（备查）

以下字面量已完成全仓扫描（2026-08-09），文档类命中（docs/、CLAUDE.md、test-inventory、CONTEXT.md）归 Stage 08 同步，不逐条列：

- `getSrc("claude")`：`AgentStatusRow.tsx`、`HistorySessionRow.tsx`、`cli-icons.test.ts`
- `claudeSession|setClaudeSession|ClaudeSessionInfo`：`TerminalRegistry.ts`、`useXterm.ts`、`useCommandDetection.ts`、`PageDockviewHost.tsx`、`historyModel.ts`、`HistorySessionList.tsx`、`useAgentStatus.ts` + 测试 9 文件 + L3 `production-osc.test.ts` + `agent.e2e.ts`
- `onHookEvent|"hook-event"`：`ipc/hooks.ts`、`useXterm.ts`、`useAgentStatus.ts`、`useClaudeNotifications.ts`、`signal.rs`、`setup.ts` + 测试 9 文件 + `agent.e2e.ts`
- `claude_history_scan|delete`：`lib.rs`、`claude_history/{mod,scan,ops}.rs`、`ipc/claudeHistory.ts`、`ipc-claude-history-contract.test.ts`
- `CLAUDE_CONTEXT_LIMIT`：`agentStatus/consts.ts`（定义）、`AgentStatusRow.tsx`（消费）
- `claudeStatus|eventToStatus|STATUS_EMOJI`：`lib/claudeStatus.ts`、`lib/index.ts`、`useXterm.ts`、`TerminalRegistry.ts`、claudeHistory 5 文件、agentStatus 2 文件 + 测试 4 文件
- `HistorySession|TitleSource`：`types/claudeHistory.ts`、`types/index.ts`、`ipc/claudeHistory.ts`、claudeHistory 8 文件、`claude_history/{mod,scan,jsonl}.rs` + 测试 7 文件 + `history.e2e.ts`
- hooks 6 命令名：`lib.rs`、`hooks/{config,inject,mod,usage}.rs`、`ipc/{hooks,hooksConfig}.ts`、`slterm-hook-reporter.js` + 契约测试 2 文件 + `hooks.e2e.ts`/`agent.e2e.ts`/`run-wdio.cjs`
- `readHooksConfig|writeHooksConfig`：`ipc/hooksConfig.ts`、`panels/hooksConfig/{useHooksConfig,HooksConfigPanel}.tsx` + 测试 3 文件 + `hooks.e2e.ts`
- `cliIconRegistry|CliIconRegistry`：`lib/cliIcons.ts`、`useCommandDetection.ts`、`TabTitleRegistry.ts`、`AgentStatusRow.tsx`、`HistorySessionRow.tsx` + 测试 4 文件 + L3
- `TabTitleRegistry|tabTitleRegistry|tabRules`：`panels/terminal/{TabTitleRegistry,tabRules,useCommandDetection,useXterm,TerminalPanel,usePtyOutput}.ts(x)`、`Workspace.tsx` + 测试 6 文件 + L3
- `ContextUsage`：`types/hooks.ts`、`types/index.ts`、`ipc/hooks.ts`、`usage.rs`、`useAgentStatus.ts` + 测试 3 文件
- `HookEventPayload|HookInjectionStatus|InjectionStatus`：`hooks/{inject,mod,signal}.rs`、`useClaudeNotifications.ts`、`HooksConfigPanel.tsx`、`ipc/hooks.ts` + 测试 6 文件 + `hooks.e2e.ts`/`specUtils.ts`/`helpers.ts`
- `useClaudeHistory|ClaudeHistorySections`：claudeHistory 4 文件、`AgentStatusView.tsx` + 测试 3 文件 + `history.e2e.ts`
- `getStatusIcon`：`lib/claudeStatus.ts`、`lib/index.ts`、`AgentStatusRow.tsx` + 测试 2 文件
- `slterm-hook-reporter|isSltermManaged`（**保留项，不泛化**——MC-213/223 claude 合法领地）：`inject.rs`、`slterm-hook-reporter.js`、`panels/hooksConfig/{configModel,EventTree,GuiMode,HandlerForm}.tsx` + 测试 3 文件 + `hooks.e2e.ts`
- `openHooksConfigPanel|hooksConfig-`（**零改动**——MC-501）：`pageApis.ts`、`SidebarTree.tsx` + 测试 3 文件 + `hooks.e2e.ts`
