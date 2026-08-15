# multi-cli profile 重构 — Stage 划分

> 清单真值源：`docs/multi-cli/checklist.md`（逐 MC-ID + 衍生项 D-NN）。编排参数：`docs/multi-cli/execution-plan.md`。
>
> **划分原则**：Stage 内文件不重叠（硬性）；Stage 间允许重复碰同一文件（串行 + 每 Stage commit）；并行 agent ≤ 5；文档同步固定末位 Stage。
>
> **依赖序**：前端身份/状态域先行（profile 注册表是一切消费的公共键载体）→ 后端 hooks/历史泛化（命令 + DTO + cliId）→ 前端历史聚合 UI（依赖 04 的 DTO.cliId）→ hub 面板（依赖 03 的泛化命令）→ mock 验收（依赖全部前序）→ 文档（终态反映）。
>
> **统一门禁（决策 9，每 Stage 全量）**：
> ```
> npx tsc --noEmit
> npx eslint src/
> cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
> cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
> cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1   # L1
> npm test                                                            # L2
> npm run test:l3                                                     # L3
> npm run e2e                                                         # L4（= build:e2e + wdio 串行，禁拆分/禁并行）
> ```
> ⚠️ L4 必须用 `npm run e2e`——config.json 的 `e2eBuild`（`npx tauri build --debug --no-bundle`）**不带 `VITE_E2E=1`**，直接用会 tree-shake 掉 E2E helper，wdio 全卡「Workspace 未就绪」。

## 跨边界契约（写死——各 Stage 脚本头部重复此段，agent 不各自推断）

### profile 接口（前端，spec 00 §3.1）

```ts
interface CodingCliProfile {
  id: string;                 // cliId 公共键，如 "claude"
  displayName: string;        // 展示名，如 "claude"
  commands: string[];         // 首 token 精确匹配键集
  iconSrc: string;            // 品牌 logo 根绝对路径，如 "/cli-icons/claude.png"
  tabTitle: string;           // OSC 133 C 命中页签标题
  capabilities: {
    hooks?: HooksCapability;
    history?: HistoryCapability;
  };
}
interface HooksCapability {
  eventToStatus(event: string, notificationType?: string | null): AgentStatus;
  classifyNotification(payload: AgentEventPayload): "permission" | "error" | "done" | null;
  contextLimit: number;         // claude = 200_000
  restartHint: string;          // claude = "hooks 改动需重启 claude 会话生效"
  hasConfigEditor: boolean;     // claude = true
}
interface HistoryCapability {
  supportsFork: boolean;        // claude = true
  buildResumeCommand(session: AgentHistorySession): string;
  buildRestoreInput(session: AgentHistorySession, opts: { fork: boolean }): string;
}
```

> Stage 01 落地时类型引用现状名（`ClaudeStatus`/`HookEventPayload`/`HistorySession`），Stage 02/03/04 更名时同步本文件——更名是全局机械同步，profile 类型文件随行。

### 泛化命令（后端，8 条全表）

```
agent_hooks_inject(cliId)                       agent_hooks_uninstall(cliId)
agent_hooks_injection_status(cliId)             agent_context_usage(cliId, transcriptPath)
agent_hooks_config_read(cliId, layer, projectPath?)   agent_hooks_config_write(cliId, layer, hooks, projectPath?)
agent_history_scan()                            agent_history_delete(cliId, sessionId)
```
- 未知 cliId → `AppError::Validation`；无 hooks 能力 cliId → `Validation`（消息含「不支持 hooks 能力」语义）
- 旧命令名（`hooks_*` / `claude_history_*`）不保留兼容（D10）

### 事件与 DTO

- 广播事件名：`agent-event`（旧 `hook-event` 零残留）
- `AgentEventPayload` = panelId/event/timestamp/sessionId/transcriptPath/cwd/toolName/notificationType + **可选 `cliId`**（serde `default`）
- `AgentHistorySession` = sessionId/cwd/title/titleSource/firstPrompt/mtimeMs/cwdExists + **`cliId`**；`titleSource` 开放字符串
- 复合键格式：`cliId|sessionId`（三处：deriveActiveSessionStatuses / findPanelForSession / titleBySessionId）
- **缺省回退不写字面量**：`profiles/claude/` 导出 `CLAUDE_CLI_ID = "claude"` 常量，通用层缺省回退（MC-205/313）一律 import 该常量（AC-5 字面量守卫兼容）

### 禁区（PREAMBLE 必含）

1. `compute_conpty_flags` 固定 0x7 勿动（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮，无法自动化守卫
2. C10 契约：reporter 任何路径 `exit(0)`、不写 stderr——勿削弱
3. watcher 轮询补漏（3s）勿削弱——win10 实证防线
4. SEC-05 等价：sessionId 校验 + 定位不信托前端——勿削弱
5. E2E 不得触碰用户真实 `~/.claude/`（env 覆盖 + fixture 隔离）
6. `E2E_ENABLED` 保持内联 `import.meta.env` 字面量形态（rolldown DCE 红线）

---

## Stage 01 — 前端 profile 注册表 + 身份域

**条目**：MC-101、MC-102、MC-103、MC-104、MC-105、MC-106、MC-108 + D-02（lib barrel cliIcons 部分）、D-07、D-08、D-13（核对）+ AC-4 资源先行（mockcli.png，决策 5）

**agent 分工表**（3 并行，文件无重叠）：

| label | 负责项 | 文件 |
|-------|--------|------|
| profile-registry | MC-101/102/103/104（注册表 + claude profile 身份域）、MC-108（守卫泛化迁入） | 新建 `src/features/cliProfiles/{types.ts, cliProfileRegistry.ts, index.ts}`、`profiles/{index.ts, claude/index.ts}`；新建 `src/__tests__/cli-profile-registry.test.ts` + `cli-profile-claude.test.ts`（含 logo 守卫遍历）；新建 `public/cli-icons/mockcli.png`（真实最小 PNG） |
| terminal-consumers | MC-105/106（OSC 133 消费点）、TabState 类型承接、终端域测试同步、D-08、D-13 | 改 `src/panels/terminal/{useCommandDetection.ts, TerminalPanel.tsx, useXterm.ts, usePtyOutput.ts}`（import/类型同步）；删 `TabTitleRegistry.ts`、`tabRules.ts`；改 `src/__tests__/{use-xterm-lifecycle.test.ts, use-xterm-output.test.ts, use-xterm-integration.test.ts, terminal.test.tsx, e2e-gating-terminal.test.ts}`；改 `test/terminal/production-osc.test.ts` |
| peripheral-consumers | cliIcons 退役 + 外围消费点过渡形态、D-02（cliIcons 段）、D-07 | 删 `src/lib/cliIcons.ts`；改 `src/lib/index.ts`；改 `src/workspace/Workspace.tsx`（side-effect import 改指 `features/cliProfiles/profiles`）；改 `src/features/agentStatus/AgentStatusRow.tsx` + `src/features/claudeHistory/HistorySessionRow.tsx`（过渡形态）；删 `src/__tests__/{tab-title-registry.test.ts, tab-rules.test.ts, cli-icons.test.ts}` |

**实现要点**：

1. **TabState 类型承接**：`TabTitleRegistry.ts` 退役后 `TabState`（含 `logo` 字段，spec 01 §6 保留）迁入 `useCommandDetection.ts` 顶部导出，TerminalPanel/useXterm import 同步——TerminalPanel 消费链（`logoRef`/双清）零行为改动。
2. **过渡形态（中间态设计）**：cliIcons.ts 退役后，`AgentStatusRow.tsx` / `HistorySessionRow.tsx` 的 `cliIconRegistry.getSrc("claude")` 改为 `cliProfileRegistry.get(CLAUDE_CLI_ID)?.iconSrc`（import profiles/claude 导出常量，**不写 "claude" 字面量**）——行 cliId 字段 Stage 02（MC-410）/Stage 05（MC-311 数据侧）才就绪，本 Stage 保留过渡；verify 白名单限此两处。
3. **profile 类型引用现状类型名**：`types.ts` 的 capabilities 签名引用 `ClaudeStatus`/`HookEventPayload`/`HistorySession`（Stage 02/03/04 更名时随行同步）。
4. **claude profile Stage 01 只含身份域**：`capabilities` 先为 `{}`（hooks 能力 Stage 02 迁入、history 能力 Stage 05 迁入）。
5. **注册触发点**：`Workspace.tsx` 的 `import "./tabRules"` 类 side-effect 改为 import `features/cliProfiles/profiles`（照 tabRules/schemes 先例）。
6. **L3 复刻段同步**（D-08）：`production-osc.test.ts` 的 OSC 133 段按生产 `matchByCommand` 取值逻辑复刻改写，逐段来源行号注释同步。
7. **退役测试语义迁移**：tab-title-registry（13 用例）/tab-rules（6）/cli-icons（12）的语义（首 token 匹配/带参变体/覆盖/单例/资源守卫）并入 cli-profile-registry.test.ts + cli-profile-claude.test.ts，用例数在 test-inventory 就近登记。
8. **mockcli.png**：1×1 透明 PNG（合法魔数），Stage 07 mock 夹具引用；本 Stage 仅放入资源 + Glob 断言。

**验证项**（详表落盘 `verify/stage-01.md`）：

1. `CliProfileRegistry` 五方法行为正确 + claude profile 身份域完整（L2 断言：register 覆盖/get/getAll 注册序/matchByCommand 多 commands·带参·空命令行·未命中/_reset）
2. Glob 断言：`cliIcons.ts`、`tabRules.ts`、`TabTitleRegistry.ts` 及三个退役测试文件不存在
3. grep 断言：`useCommandDetection.ts` 无 `"🟡"` 字面量、无 `TabTitleRegistry|CliIconRegistry` import；全仓无 `cliIconRegistry|tabTitleRegistry` 代码引用
4. 过渡形态白名单：`get(CLAUDE_CLI_ID)` 仅现于 `AgentStatusRow.tsx` + `HistorySessionRow.tsx`（grep 计数 = 2），且无 `"claude"` 字符串字面量
5. `Workspace.tsx` import `features/cliProfiles/profiles`；`lib/index.ts` 无 cliIcons 导出
6. `production-osc.test.ts` OSC 133 复刻段与生产 matchByCommand 一致（语义式断言）
7. Glob 命中 `public/cli-icons/mockcli.png`
8. 全量门禁绿（含 L4）

**commit message**：`refactor(cli-profiles): 前端 CliProfileRegistry + 身份域迁移（MC-101~108）`

**人工验证点**：真实 claude 会话页签冒烟——OSC 133 C 命中后页签标题 = `claude`、🟡 + 16×16 logo 显示正常、命令退出后恢复（L4 间接覆盖，视觉人工确认）。

---

## Stage 02 — 前端状态域

**条目**：MC-401、MC-402、MC-403、MC-404、MC-405、MC-406（核对）、MC-107、MC-205、MC-206、MC-410、MC-411、MC-412、MC-413、MC-414、MC-420、MC-421、MC-422 + D-02（claudeStatus 段）、D-04、D-06、D-12、D-14（agent.e2e 空态文案段）

**agent 分工表**（5 并行，文件无重叠）：

| label | 负责项 | 文件 |
|-------|--------|------|
| lib-status | MC-401（lib 迁移）、MC-422 + MC-214 前端半（claude profile hooks 能力：eventToStatus/classifyNotification/contextLimit/restartHint/hasConfigEditor + `CLAUDE_CLI_ID` 常量导出）、D-02（claudeStatus 段） | `src/lib/claudeStatus.ts` → `src/lib/agentStatus.ts`；改 `src/lib/index.ts`；改 `src/features/cliProfiles/profiles/claude/index.ts`（可拆 `strategies.ts`）+ `types.ts`（类型名随行同步）；删 `src/__tests__/claude-status.test.ts` → 新建 `agent-status-lib.test.ts`；扩 `cli-profile-claude.test.ts`（eventToStatus 32 用例语义 + classifyNotification 五映射迁入） |
| registry-xterm | MC-402、MC-107、MC-403、MC-205/206（三级解析落地）、D-12 + 更名连锁同步（claudeHistory 两消费文件仅字段更名，复合键留 Stage 05） | 改 `src/panels/terminal/{TerminalRegistry.ts, useXterm.ts, useCommandDetection.ts}`；改 `src/features/claudeHistory/{historyModel.ts, HistorySessionList.tsx}`（**仅 claudeSession→agentSession 字段更名同步**）；改 `src/__tests__/{terminal-registry.test.ts, terminal-registry-subscribe.test.ts, use-xterm-lifecycle.test.ts, use-xterm-output.test.ts, use-xterm-integration.test.ts, claude-history-model.test.ts, claude-history-view.test.tsx, claude-history-hook.test.tsx}` |
| notifications | MC-420/421/404（通知侧）、D-04 | `src/features/notifications/useClaudeNotifications.ts` → `useAgentNotifications.ts`；改 `index.ts` barrel；改 `src/App.tsx`（NotificationListener import）；改 `src/__tests__/notifications.test.ts` |
| agent-status-view | MC-410/411/412/413/414 + 过渡形态清扫（AgentStatusRow 的 `get(CLAUDE_CLI_ID)` → row.cliId） | 改 `src/features/agentStatus/{useAgentStatus.ts, AgentStatusRow.tsx, AgentStatusView.tsx}`；删 `consts.ts`；改 `src/__tests__/{agent-status-view.test.tsx, agent-status-hook.test.ts}`；改 `e2e-tests/agent.e2e.ts`（空态文案断言） |
| f8-rename | MC-405/406（核对）、D-06 | 改 `src/workspace/PageDockviewHost.tsx`（F8 判定 `agentSession`）；改 `src/__tests__/workspace-header-actions.test.tsx` |

**实现要点**：

1. **MC-205 三级解析落地形态**：`payload.cliId ?? TerminalRegistry.get(panelId)?.agentSession?.cliId ?? CLAUDE_CLI_ID`——本 Stage `HookEventPayload` 尚无 cliId 字段（Stage 03 后端加），前端类型层先写可选字段 `cliId?: string`（恒 undefined 向后兼容），三级解析一次写全；Stage 03 后端字段到达后自然生效。
2. **eventToStatus 迁移**：`profiles/claude/` 新增 hooks 策略实现（10 事件 + notificationType 子类型 + ATTENTION_NOTIFICATION_TYPES），现 32 用例语义不丢、落点改 `cli-profile-claude.test.ts`；lib 层仅保留四态类型/STATUS_EMOJI/getStatusIcon。
3. **useXterm 事件路径**：按解析结果取 profile → `profile.hooks?.eventToStatus(...)`；无 hooks 能力 → `console.warn` + 跳过；SessionEnd 清图标、Exit 清会话、payload 空串归一 `|| undefined` 等现状分支全保留；订阅函数名本 Stage 仍为 `onHookEvent`（Stage 03 统一换 `onAgentEvent`）。
4. **行建模 cliId**（MC-410）：hook 事件通道建行按三级解析写 cliId；OSC 133 通道经 setAgentSession 的 sessionChange 自然驱动；初始扫描/竞态双保险/reconcile 语义不变。
5. **用量口径**（MC-412）：`contextLimit` = `cliProfileRegistry.get(row.cliId)?.capabilities?.hooks?.contextLimit`，缺失 → `--`；CLAUDE_CONTEXT_LIMIT 退役删 consts.ts。
6. **空态文案**（MC-414）：仅此一处用户可见文案变动；E2E 红线（data-e2e/标题栏/「选择一个项目」）逐字保留；agent.e2e.ts 断言同步。
7. **CATEGORY_EMOJI**（MC-404）：通知模块内定义 🔐❌✅，与 STATUS_EMOJI 注释互引。
8. **更名连锁**：`ClaudeSessionInfo`→`AgentSessionInfo`(+cliId)、`claudeSession`→`agentSession`、`setClaudeSession`→`setAgentSession` 全仓同步（含 L3 production-osc、E2E helper 若引用）——merge 语义/null 清空/undefined 不覆盖/lastEventAt 自动填/register 幂等保留旧值，全保留。

**验证项**（详表 `verify/stage-02.md`）：

1. Glob：`lib/agentStatus.ts` 存在、`lib/claudeStatus.ts` 与 `agentStatus/consts.ts` 不存在
2. grep：`src/lib/` 无 claude 事件名字面量（SessionStart/PreToolUse 等零命中）
3. grep 全仓零残留：`claudeSession`、`setClaudeSession`、`ClaudeSessionInfo`、`CLAUDE_CONTEXT_LIMIT`（含测试/L3/E2E）
4. `AgentSessionInfo` 含 `cliId: string`；setAgentSession merge 语义用例全绿（L2）
5. useXterm 事件路径 L2：mock profile 的 eventToStatus 被真实调用（入参断言）；无 hooks 能力 warn+跳过；SessionEnd/Exit 清态用例保留
6. 三级解析三分支 L2 用例（显式/反查/缺省——显式分支经可选字段注入）
7. AgentStatusRow logo 按 row.cliId 查 profile.iconSrc；用量 contextLimit 来自 profile；`get(CLAUDE_CLI_ID)` 过渡形态在 AgentStatusRow 已消除（grep 计数 ≤1，仅 HistorySessionRow）
8. 空态文案「无运行中的编码 CLI 会话」（L2 + agent.e2e 断言同步）；E2E 红线逐字保留
9. CATEGORY_EMOJI 与 STATUS_EMOJI 注释互引（Read 断言）；F8 判定 agentSession（workspace-header-actions 绿）
10. 全量门禁绿（含 L4）

**commit message**：`refactor(agent-status): 前端状态域去 claude 化——四态策略入 profile（MC-401~422）`

**人工验证点**：真实 claude 会话四态全链路冒烟（⚡→✅ 流转、页签 emoji、AgentStatus 行/用量条）；F4 通知（失焦 toast + 任务栏闪烁）真实触发一次。

---

## Stage 03 — 后端 hooks 泛化 + 前端 ipc/types

**条目**：MC-201、MC-202、MC-203（核对）、MC-204（核对）、MC-210、MC-211、MC-212、MC-213、MC-214（后端半）、MC-215（决策 7）+ 决策 3（类型更名）+ 决策 4（E2E-05 硬编码+注释）+ D-01、D-03（types barrel hooks 段）、D-09、D-10、D-11、D-14（hooks.e2e/agent.e2e 事件名段）

**agent 分工表**（3 并行，文件无重叠）：

| label | 负责项 | 文件 |
|-------|--------|------|
| backend-hooks | MC-201/202（后端）、MC-210/211/213/214/215、决策 3 后端类型更名 | `src-tauri/src/hooks/`：改 `signal.rs`（AgentEventPayload+cliId、agent-event 广播）、`mod.rs`（命令层泛化 + WATCHER 保留 + InjectionStatus→AgentInjectionStatus 等更名）、`watcher.rs`（仅核对零改动）；新建 `provider.rs`（trait + cliId 键注册表）；下沉 `inject.rs`/`usage.rs`/`config.rs` → `claude/`；移动 `src-tauri/assets/slterm-hook-reporter.js` → `hooks/claude/slterm-hook-reporter.js`（显式 `cliId:"claude"` + SCRIPT_VERSION 递增）；改 `lib.rs`（6 命令更名注册） |
| frontend-ipc | MC-212、MC-202（前端订阅更名）、D-01、D-03（hooks 段）+ 前端调用点中间态同步 | `src/ipc/hooks.ts` → `agentHooks.ts`；`src/types/hooks.ts` → `types/agent.ts`；改 `src/ipc/{hooksConfig.ts, index.ts}`、`src/types/index.ts`；改 `src/__tests__/setup.ts`（D-01）；`ipc-hooks-contract.test.ts` → `ipc-agent-hooks-contract.test.ts`（22 用例四维同步）；改 `ipc-hooks-config-contract.test.ts`；调用点同步：`useHooksConfig.ts`/`HooksConfigPanel.tsx`（cliId 实参暂传 `CLAUDE_CLI_ID` 常量）、`useAgentStatus.ts`（contextUsage 传 row.cliId）、`useXterm.ts`/`useAgentNotifications.ts`（onAgentEvent 订阅更名）+ 相关测试 `use-xterm-*.test.ts`、`agent-status-*.test.ts(x)`、`notifications.test.ts`、`hooks-config-panel.test.tsx`、`hooks-config-sync.test.tsx` |
| e2e-infra | D-09、D-10、D-11、D-14（本 Stage 段） | 改 `e2e-tests/{helpers.ts, specUtils.ts, run-wdio.cjs, hooks.e2e.ts, agent.e2e.ts}`（事件名/命令名断言 + 备份集合注释） |

**实现要点**：

1. **trait 签名（写死）**：`CliHooksProvider`：`inject() / uninstall() / injection_status() / context_usage(transcript_path) / config_read(layer, project_path) / config_write(layer, hooks, project_path)`；注册表 = cliId 键静态映射（照前端模块级单例先例的 Rust 形态）。
2. **claude provider 下沉零行为改动**：inject/usage/config 三文件物理移动 + import 路径修正；133 条 L1 用例（inject 34/signal 14/watcher 20/usage 26/config 27/mod 12）全部保留迁移，`--test-threads=1` 纪律不变。
3. **reporter（决策 7）**：payload 显式写 `cliId: "claude"`；`SCRIPT_VERSION` 递增（已注入用户变「版本过旧」需重新注入——预期波及，测试锁死新形态）；注入目标路径 `~/.slterminal/hooks/slterm-hook-reporter.js` **不变**（E2E 零波及）；C10 契约不改；E2E-06 链路不削弱。
4. **命令层错误语义**：未知 cliId → `Validation("未知 cliId: ...")`；cliId 已注册但无 hooks 能力 → `Validation`（含「不支持 hooks 能力」语义）——为未来无 hooks 能力 CLI 预留（本期注册表仅 claude，走不到第二分支，但分支与测试要建好）。
5. **前端中间态（写死）**：`useHooksConfig.ts` / `HooksConfigPanel.tsx` 的泛化命令 cliId 实参**暂传 `CLAUDE_CLI_ID` 常量**（Stage 06 hub 化时改 selectedCliId 回收）；`useAgentStatus.ts` 的 `contextUsage` 传行 cliId（Stage 02 已建行字段）。
6. **DTO 双边**：`AgentHookInjectionStatus`（决策 3 更名）camelCase 三态契约不变；`AgentEventPayload` 九键（8+可选 cliId）serde 键集合测试。
7. **E2E-05（决策 4）**：run-wdio.cjs 备份集合保持 claude 硬编码，追加注释「随第二 CLI 接入扩展」。
8. **广播更名连锁**：`agent-event` 三消费方 + setup.ts 全局 mock + E2E 断言同 Stage 同步——漏 setup.ts 则 L2 全局 mock 失效大面积炸（D-01 红线）。

**验证项**（详表 `verify/stage-03.md`）：

1. `AgentEventPayload` 九键 serde 测试（含无 cliId 旧信号反序列化兼容用例）
2. grep 零残留：`"hook-event"`、`onHookEvent`、`HookEventPayload`（src/ + src-tauri/ + e2e-tests/ + test/）
3. `CliHooksProvider` trait 六方法 + claude provider 注册；L1 新增：注册表 get/未知 cliId Validation/命令 cliId 透传（block_on 直测）
4. lib.rs 注册 6 泛化命令（grep 命中）；旧命令名零残留（grep `hooks_inject|hooks_uninstall|hooks_injection_status|hooks_context_usage|hooks_config_read|hooks_config_write` 于 lib.rs/ipc/e2e-tests 零命中——注意命中 `agent_hooks_inject` 属正常，断言用精确词边界）
5. Glob：`hooks/claude/{inject,usage,config}.rs` 存在、`hooks/` 顶层同名不存在；reporter.js 位于 `hooks/claude/`；133 用例全绿
6. reporter 模板含显式 `cliId:"claude"` + SCRIPT_VERSION 已递增（L1 模板内嵌校验断言）；E2E-06 绿
7. Glob：`src/types/agent.ts`、`src/ipc/agentHooks.ts` 存在，旧 `types/hooks.ts`、`ipc/hooks.ts` 不存在；setup.ts mock 路径 = `../ipc/agentHooks`
8. 中间态语义式断言：useHooksConfig/HooksConfigPanel 的 cliId 实参来自 `CLAUDE_CLI_ID` 常量 import（非 "claude" 字面量）
9. run-wdio.cjs 含「随第二 CLI 接入扩展」注释
10. 全量门禁绿（L4 hooks.e2e 泛化命令 + agent.e2e 事件名全绿）

**commit message**：`refactor(hooks): 后端 hooks 信号链路泛化 + CliHooksProvider 下沉 claude（MC-201~215）`

**人工验证点**：真实环境 hooks 注入→状态三态→卸载全流程；**已注入用户升级后显示「版本过旧」→ 重新注入恢复**（决策 7 波及确认）；claude 会话事件链路（信号文件 → 页签 emoji/会话行）真实冒烟。

---

## Stage 04 — 后端历史泛化 + 前端 ipc/types

**条目**：MC-301、MC-302、MC-303、MC-304、MC-305、MC-306 + D-03（types barrel claudeHistory 段）、D-14（history.e2e 命令名段）

**agent 分工表**（3 并行，文件无重叠）：

| label | 负责项 | 文件 |
|-------|--------|------|
| backend-history | MC-301/302/303/304/305 | `src-tauri/src/claude_history/` → `src-tauri/src/agent_history/`：`mod.rs`（聚合层 + DTO + 命令）、新建 `provider.rs`（`CliHistoryProvider` trait + 注册表）、`claude/{scan,jsonl,ops}.rs` 下沉（`is_uuid_filename` 作可复用工具保留）；改 `lib.rs`（2 命令更名注册） |
| frontend-ipc-history | MC-306、D-03（claudeHistory 段）+ 调用点签名同步 | `src/ipc/claudeHistory.ts` → `agentHistory.ts`；`src/types/claudeHistory.ts` → `agentHistory.ts`；改 `src/ipc/index.ts`、`src/types/index.ts`；`ipc-claude-history-contract.test.ts` → `ipc-agent-history-contract.test.ts`（8 用例：scan 无参/delete 双参）；调用点同步：`useClaudeHistory.ts`、`HistorySessionList.tsx`、`historyContextMenu.ts`（删除链传 `session.cliId`——目录更名留 Stage 05，本 Stage 仅签名同步）+ `claude-history-*.test.ts(x)` 相关断言 |
| e2e-history | D-14（本 Stage 段） | 改 `e2e-tests/history.e2e.ts`（命令名断言） |

**实现要点**：

1. **trait 签名（写死）**：`CliHistoryProvider`：`scan() -> Vec<AgentHistorySession> / delete(session_id) -> Result<()> / validate_session_id(id) -> Result<()>`；契约注释写明「validate_session_id 是 delete 的强制前置」（SEC-05 等价强制，MC-304）。
2. **聚合语义**：`agent_history_scan()` 无参遍历全部已注册 provider 串行聚合；单 provider 失败不阻塞（照单文件降级条目契约的层级提升）；全部空 → 空数组。
3. **claude provider 下沉零行为改动**：63 条 L1 用例（jsonl 28/scan 19/ops 9/mod 7）全保留；env 覆盖 `SLTERM_CLAUDE_PROJECTS_DIR` 与 ScanRootGuard 模式留 provider 内部；`titleSource` 开放字符串（claude 值集不变）。
4. **DTO 打标**：claude provider 产出条目 `cli_id: "claude"`（provider 内部写字面量合法，MC-213 同理）；serde camelCase 八键。
5. **前端中间态**：`AgentHistorySession.cliId` 字段到达后，删除链调用 `deleteHistorySession(session.cliId, session.sessionId)`；features 目录更名与复合键改造属 Stage 05。
6. **lib.rs 与 Stage 03 同文件**：Stage 串行无冲突。

**验证项**（详表 `verify/stage-04.md`）：

1. Glob：`agent_history/{mod,provider}.rs` + `agent_history/claude/{scan,jsonl,ops}.rs` 存在；`claude_history/` 目录不存在
2. trait 三方法 + 注册表；L1 新增：聚合 scan 遍历/delete 未知 cliId Validation/validate 前置
3. `AgentHistorySession` serde 八键 camelCase + titleSource 开放字符串序列化用例
4. 63 用例迁移全绿；SEC-05（UUID 校验 + 定位不信托前端）用例保留；env 覆盖用例保留
5. lib.rs 注册 `agent_history_scan/delete`；旧名零残留（精确词边界 grep）
6. Glob：`src/types/agentHistory.ts`、`src/ipc/agentHistory.ts` 存在，旧名不存在；契约 8 用例四维绿
7. 删除链调用点传 `session.cliId`（Read 断言）
8. history.e2e 全绿（fixture 形态不变）
9. 全量门禁绿（含 L4）

**commit message**：`refactor(agent-history): 后端历史会话泛化 + CliHistoryProvider 下沉 claude（MC-301~306）`

**人工验证点**：历史区真实冒烟——当前项目/全部项目两区展示、删除会话、孤儿 ✗ 标记正常。

---

## Stage 05 — 前端历史聚合 UI

**条目**：MC-310、MC-311、MC-312（核对）、MC-313、MC-314、MC-315、MC-316、MC-317 + D-05、D-14（history.e2e 恢复注入段核对）

**agent 分工表**（3 并行，文件无重叠）：

| label | 负责项 | 文件 |
|-------|--------|------|
| features-migrate | MC-310/311/313/315/316/317（目录迁移 + 全部委托改造） | `src/features/claudeHistory/` → `src/features/agentHistory/`（9 文件：`index.ts`、`ClaudeHistorySections.tsx`→`AgentHistorySections.tsx`、`HistorySessionList.tsx`、`HistorySessionRow.tsx`、`SessionActionDialog.tsx`、`historyContextMenu.ts`、`historyModel.ts`、`useClaudeHistory.ts`→`useAgentHistory.ts`、`restoreSession.ts`） |
| claude-history-cap | claude profile history 能力 + 用例 | 改 `src/features/cliProfiles/profiles/claude/`（+ `capabilities.history`：supportsFork/buildResumeCommand/buildRestoreInput）；扩 `src/__tests__/cli-profile-claude.test.ts`（history 策略用例：resume 命令/fork 追加/`\r` 结尾） |
| host-and-tests | MC-314、D-05 + 测试迁移 | 改 `src/features/agentStatus/AgentStatusView.tsx`（import 更名 + titleBySessionId 复合键）；迁移 `src/__tests__/claude-history-{model,hook,restore,row,view,action-dialog}.test.ts(x)` → `agent-history-*`（断言同步）+ `agent-status-view.test.tsx`（import 断言同步）；核对 `e2e-tests/history.e2e.ts`（恢复注入断言——buildRestoreInput 输出与现状逐字一致则零改动） |

**实现要点**：

1. **跨边界依赖**：features-migrate 调 `profile.capabilities?.history?.buildResumeCommand(...)` 可选链——类型 Stage 01 已定义，实现由 claude-history-cap 同 Stage 交付，编译互不阻塞。
2. **复合键格式（写死）**：`` `${cliId}|${sessionId}` `` 三处——`deriveActiveSessionStatuses`（historyModel.ts:123-137）、`findPanelForSession`（HistorySessionList.tsx:192-204）、`titleBySessionId`（AgentStatusView.tsx:126-141）；transcriptPath basename 回退保留（旧数据无 cliId 按 `CLAUDE_CLI_ID`——常量引用非字面量）。
3. **恢复编排**（MC-315）：四步框架零改动；第 4 步 = `profile.history.buildRestoreInput(session, { fork })`；addPanel `title` = `profile.tabTitle`；防重入/失败 toast 保留。
4. **菜单策略**（MC-316）：`buildResumeCommand` 委托 profile；`supportsFork=false` 不展示「分支恢复」；孤儿/无 cwd 禁分支恢复、运行中禁删除的通用矩阵保留；cwd 单引号限制注释留 claude 实现内。
5. **行 logo**（MC-311）：`session.cliId` 查 `profile.iconSrc`；「仅随 status emoji」与孤儿 ✗ 不加图保留；未注册 cliId → 无 logo 不报错；HistorySessionRow 的 `get(CLAUDE_CLI_ID)` 过渡形态本 Stage 清扫。
6. **更名全表**：目录/文件/组件/Hook/barrel/测试文件名全同步（MC-5）；`data-e2e` 选择器与空态文案红线不动。
7. **E2E 恢复断言核对**：claude profile 的 buildRestoreInput 输出必须与现状逐字一致（`claude --resume <id>` + fork 追加 + `\r` 结尾）——history.e2e 恢复编排用例应零改动通过；若断言漂移即实现有误。

**验证项**（详表 `verify/stage-05.md`）：

1. Glob：`features/agentHistory/` 9 文件存在、`features/claudeHistory/` 不存在；测试文件更名同步
2. grep 零残留：`claudeHistory`、`useClaudeHistory`、`ClaudeHistorySections`（src/ 全仓含测试）
3. HistorySessionRow 行 logo 按 session.cliId（语义式断言：iconSrc 来源 = 注册表查 session.cliId）；`get(CLAUDE_CLI_ID)` 过渡形态全仓清零
4. 复合键三处语义式断言（Read 确认 `` `${cliId}|${sessionId}` `` 拼接形态）；transcriptPath basename 回退用例保留
5. restoreSession 无 `"claude"` 字面量；`title` 来自 profile.tabTitle；pty.write 注入内容 = claude profile 策略输出（L2 断言逐字一致）
6. supportsFork 菜单显隐 L2 用例（mock profile supportsFork=false → 无「分支恢复」项——可用 Stage 07 夹具前置简易 stub，或构造测试 profile；**允许用例内局部注册测试 profile**）
7. AgentStatusView 标题覆盖复合键；活跃区/历史区标题联动用例绿
8. **AC-5 预检**：grep `src/lib/`、`src/panels/terminal/`、`src/features/agentStatus/`、`src/features/agentHistory/`、`src/features/notifications/`、`src/ipc/`、`src/types/` 无 "claude" 字符串字面量与 claude 事件名（profiles/claude/ 与 panels/hooksConfig、features/hooksConfig 属 claude 合法领地除外）——为 Stage 07 守卫铺路
9. history.e2e 全绿（恢复注入断言零改动通过）
10. 全量门禁绿（含 L4）

**commit message**：`refactor(agent-history): 前端历史聚合 UI 泛化 + 复合键（MC-310~317）`

**人工验证点**：恢复编排真实冒烟——双击历史行恢复会话（四步：项目入列/页面/切换/终端注入 `claude --resume`）、分支恢复、复制恢复命令剪贴板内容正确。

---

## Stage 06 — hub 面板

**条目**：MC-501（核对）、MC-502、MC-503、MC-504、MC-505、MC-506、MC-507、MC-508 + MC-220、MC-221、MC-222、MC-223（文档注明段）+ D-15 + D-14（hooks.e2e hub 段）+ Stage 03 中间态回收（CLAUDE_CLI_ID 临时代理 → selectedCliId）

**agent 分工表**（3 并行，文件无重叠）：

| label | 负责项 | 文件 |
|-------|--------|------|
| hub-panel | MC-502/503/504/505/506/507 + MC-220/221/222（面板侧）+ 中间态回收 | 改 `src/panels/hooksConfig/HooksConfigPanel.tsx`（拆 hub 容器 + claude 编辑器组件整体下移一层）、`src/panels/hooksConfig/useHooksConfig.ts`（cliId 实参 = hub 选中态）；其余 9 文件零改动核对（MC-508） |
| panel-tests | 测试同步 | 改 `src/__tests__/hooks-config-*.test.ts(x)`（11 文件：hub 容器内挂载路径变化断言语义不丢 + 选择行/持久化/dirty 守卫/空态新用例）；核对 `open-hooks-config-panel.test.ts`、`sidebar-actions.test.ts`、`default-layout-format.test.ts`（D-15：入口零改动预期） |
| e2e-hub | D-14（本 Stage 段） | 改 `e2e-tests/hooks.e2e.ts`（hub 面板用例：project 层保存写盘 + merge 保留字段经 hub 全绿） |

**实现要点**：

1. **组件拆分形态（写死）**：`HooksConfigPanel.tsx` 改造为 hub 容器（选择行 + 编辑器槽）；现状全部内容（层级切换/GUI·JSON/注入状态条/保存/重启提示条）下移为 `ClaudeHooksConfigEditor` 组件（同文件或新文件 `ClaudeHooksConfigEditor.tsx`，行为零改动）；`features/hooksConfig/`（schema 单点）零改动。
2. **selectedCli 持久化照 F8 先例**：`api.updateParameters({...params, selectedCli})` + **显式 `onLayoutChange(saveLayout(api))`**——updateParameters 不触发 onDidLayoutChange（dockviewPanel.js:84-95 实证），必须显式保存；挂载读 params 恢复；缺省/失效回退首个 hasConfigEditor CLI。
3. **dirty 守卫**：切换 CLI 时当前编辑器 dirty → `dialog.ask` 确认丢弃；askGuard 防循环复用（照切层/visibilitychange 先例）；切换 = 卸载重挂载（ADR-0001 先例）。
4. **选择行渲染**：`cliProfileRegistry.getAll().filter(p => p.capabilities?.hooks?.hasConfigEditor)`；按钮 = iconSrc 16×16 + displayName；选中态背景高亮走 theme token（硬约束 #6，禁硬编码色值）；单 CLI 也渲染（边界 1）；空态「无可配置 CLI」（MC-507）。
5. **restartHint**（MC-222/506）：提示条文案 = `profile.hooks.restartHint`；`data-e2e="hooks-restart-hint"` 保留；注入状态条三态数据源 = `agent_hooks_injection_status(selectedCliId)`。
6. **中间态回收**：useHooksConfig/HooksConfigPanel 的 `CLAUDE_CLI_ID` 实参改为 hub 选中态 cliId——grep 确认 panels/hooksConfig/ 内无 CLAUDE_CLI_ID 残留（claude 编辑器组件内部属合法领地，但 ipc 调用实参必须来自选中态）。
7. **入口零改动**（MC-501）：面板 id `hooksConfig-{pageId}`、侧栏菜单流程、pageApis 全部不动。

**验证项**（详表 `verify/stage-06.md`）：

1. 选择行 L2：能力过滤（hasConfigEditor=false 不出现）/logo+displayName/选中高亮 token/单 CLI 渲染/点击切换 → 编辑器重挂载且 IPC 携新 cliId
2. 持久化 L2：selectedCli 写入 params + 显式 onLayoutChange 调用（照 customTitle 测试先例）；挂载恢复；失效回退首个有能力 CLI
3. dirty 守卫 L2：ask 确认/取消两分支；非 dirty 直接切换
4. 空态 L2：无 hasConfigEditor profile →「无可配置 CLI」占位不渲染编辑器
5. restartHint 由 profile 驱动（L2 断言文案来源）；`data-e2e="hooks-restart-hint"` 保留
6. claude 编辑器 11 测试文件全绿（hub 内）；grep `panels/hooksConfig/` 无 `CLAUDE_CLI_ID` 临时代理残留（ipc 实参来自选中态）
7. 入口零改动：open-hooks-config-panel/sidebar-actions/default-layout-format 测试绿（零改动通过）
8. hooks.e2e hub 用例全绿
9. 全量门禁绿（含 L4）

**commit message**：`refactor(hooks-config): hub 面板 + CLI 选择行（MC-501~508）`

**人工验证点**：hub 面板真实操作——选择行显示/切换/保存提示/注入按钮三态/层级切换与 GUI·JSON 双模式在 hub 内正常。

---

## Stage 07 — mock profile 全链路验收 + AC-5 守卫

**条目**：AC-4（全表）、AC-5、MC-4、MC-6

**agent 分工表**（2 并行，文件无重叠）：

| label | 负责项 | 文件 |
|-------|--------|------|
| mock-fixture | AC-4 五点 L2 全链路 | 新建 `src/__tests__/helpers/mockCliProfile.ts`（mockcli 定义 + 注册/清理辅助；iconSrc = `/cli-icons/mockcli.png` Stage 01 已放）；新建 `src/__tests__/mock-cli-profile.test.tsx`（五点全表用例） |
| ac5-guard | AC-5 grep 守卫 + E2E mock 冒烟 | 新建 `src/__tests__/no-claude-literals.test.ts`；改 `e2e-tests/helpers.ts`（`__slterm_e2e_registerMockCliProfile`，E2E_ENABLED 门控）+ mock 冒烟用例（追加进既有 e2e spec 或新建 `mockcli.e2e.ts`） |

**实现要点**：

1. **mock profile 约定（决策 5 + spec 06 §7，写死）**：id `"mockcli"`、commands `["mockcli"]`、displayName `"mockcli"`、tabTitle `"mockcli"`、iconSrc `/cli-icons/mockcli.png`；hooks 全能力（eventToStatus 恒等映射/classifyNotification 桩/contextLimit 任意值/restartHint 桩文案/hasConfigEditor=true）+ history 全能力（supportsFork=true/buildResumeCommand/buildRestoreInput 桩输出，输出带可识别前缀如 `mockcli --resume`）；**仅测试环境注册**（vitest 内 register + afterEach `_reset` 清理；L4 经 E2E helper 注册）。
2. **AC-4 五点用例**：① OSC 133 命中（`matchByCommand("mockcli --flag")` → 页签标题/logo/agentSession.cliId="mockcli"）② eventToStatus/classifyNotification 被真实调用（spy 断言入参）③ 历史聚合 UI 出现 mock 条目 + 行 logo ④ hub 选择行两枚按钮 + 切换渲染桩编辑器 + selectedCli 持久化恢复 ⑤ 恢复注入内容 = mock buildRestoreInput 输出（pty.write 断言）。
3. **AC-5 守卫形态**：L2 测试读源码文件文本做 grep 断言（照 `e2e-build-config.test.ts` AST/正则断言先例）——扫描 `src/lib/`、`src/panels/terminal/`、`src/features/agentStatus/`、`src/features/agentHistory/`、`src/features/notifications/`、`src/ipc/`、`src/types/` 的 `.ts(x)` 文件：无 `"claude"` 字符串字面量、无 claude 事件名（SessionStart/SessionEnd/UserPromptSubmit/Stop/StopFailure/PreToolUse/PostToolUse/PostToolUseFailure/Notification/PermissionRequest 作为**字符串字面量**出现）、无 `~/.claude` 路径；白名单：`profiles/claude/` 导出常量的 import 引用形态（`CLAUDE_CLI_ID`）合法。**断言写语义式**：守卫测试自身枚举目录扫描，新增文件自动纳入。
4. **E2E mock 冒烟**（1-2 条）：helper 注册 mockcli → 终端注入 OSC 133 C（`__e2e_writeToTerminal` 或 pty.write）→ 页签标题/logo 断言。helpers.ts 的 E2E_ENABLED 内联门控红线不动。

**验证项**（详表 `verify/stage-07.md`）：

1. mockCliProfile 夹具存在且仅测试注册（grep 生产代码无 mockcli 注册点）
2. AC-4 五点用例全绿（逐点断言用例存在且通过）
3. AC-5 守卫测试绿；人工复核守卫扫描目录清单 = 通用层七路径
4. E2E mock 冒烟绿（如落地）
5. 全量门禁绿（含 L4）

**commit message**：`test(cli-profiles): mock profile 全链路验收 + AC-5 字面量守卫（AC-4/AC-5）`

**人工验证点**：无（全自动化覆盖）。

---

## Stage 08 — 文档同步（固定末位）

**条目**：MC-8、MC-109、MC-110、MC-318、MC-223（CLAUDE.md 注明）、AC-6

**agent 分工表**（4 并行，文件无重叠）：

| label | 负责项 | 文件 |
|-------|--------|------|
| root-docs | 根 CLAUDE.md + CONTEXT.md + test-inventory 总核对 | `.claude/CLAUDE.md`（模块索引：增 `src/features/cliProfiles`、`src/features/agentHistory`、`src-tauri/src/agent_history`，删 `src/features/claudeHistory`、`src-tauri/src/claude_history`；需求编号索引补 MC 家族说明）、`CONTEXT.md`（术语核对——「CLI profile」等词条已按规格修订，核对与终态一致）、`.claude/test-inventory.md`（全量用例数与实跑对齐） |
| frontend-module-docs | 前端模块 CLAUDE.md（**不含 `src/panels/CLAUDE.md`**——整文件归 backend-module-docs，见下） | `src/{lib,ipc,types,workspace,stores,theme}/CLAUDE.md`、`src/features/{cliProfiles（新建）,agentStatus,agentHistory（新建，替代 claudeHistory）,notifications,hooksConfig,sideViews,sidebar,explorer,commit,shortcuts}/CLAUDE.md`；MC-223 注明（panels/hooksConfig 段在 `src/panels/CLAUDE.md` 内——由 backend-module-docs 一并写入；features/hooksConfig 段归本 agent）；MC-318 已知限制记录于 agentHistory CLAUDE.md |
| backend-module-docs | 后端模块 CLAUDE.md + `src/panels/CLAUDE.md`（含 MC-109/223 两处） | `src-tauri/src/CLAUDE.md`（顶层）、`src-tauri/src/{pty,hooks,agent_history（新建，替代 claude_history）,git,notify,fs}/CLAUDE.md`、`src/panels/CLAUDE.md`；MC-109：pty/panels CLAUDE.md 适配层段落「claude 定制」→「终端平台能力」改写；MC-223：panels/CLAUDE.md 的 hooksConfig 段注明「claude 专属」；MC-110 文档记录 |
| e2e-l3-docs | 测试基建文档 | `e2e-tests/CLAUDE.md`（helper 更名/mock 冒烟/E2E-05 注释说明）、`src/__tests__/CLAUDE.md`（测试文件更名映射 + 新测试文件登记） |

**实现要点**：

1. **模块索引更新**：根 CLAUDE.md 模块索引表增删行 + 详情链接；「需求编号索引」段 MC 家族免登记说明（阶段项目代号先例）。
2. **test-inventory 全量对齐**：各 Stage 已就近登记，本 Stage 做总数核对——L1 用例数（cargo test 输出统计）、L2（vitest 输出）、L3、L4 spec 条目数，与文档计数一致。
3. **文件表逐行 Glob 命中**：各模块 CLAUDE.md 的文件表与磁盘实态一致（删 claudeHistory/claude_history 行、增 agentHistory/agent_history/cliProfiles 行）。
4. **MC-109 改写红线**：机制注释中「供 claude 取消」「Ink 据此换行」等触发点描述**保留**（历史动机如实记录）；仅「定制/专为 claude」类归属表述改写为「终端平台能力」。
5. **MC-318 记录形态**：agentHistory CLAUDE.md「已知限制」段两条（组键漂移/历史区无 ticker），注明「规格确认不修（决策 6）」。

**验证项**（详表 `verify/stage-08.md`）：

1. 根 CLAUDE.md 模块索引含三个新模块行、无两个旧模块行（grep 断言）
2. 各模块 CLAUDE.md 文件表逐行 Glob 命中（语义式断言：Read 对照磁盘）
3. test-inventory 计数与实跑输出一致（取数口径：L1/L2/L3 门禁输出统计行 + L4 spec 文件用例计数）
4. MC-109：pty/CLAUDE.md + panels/CLAUDE.md 适配层段落无「claude 定制」归属表述（grep「专为 claude|claude 定制」零命中），触发点描述保留（grep「供 claude 取消」命中）
5. MC-318：agentHistory CLAUDE.md 含两条已知限制
6. MC-223：panels/hooksConfig 与 features/hooksConfig CLAUDE.md 含「claude 专属」注明
7. CONTEXT.md 术语与终态一致（Read 核对）
8. 全量门禁绿（终验 AC-1/AC-2）

**commit message**：`docs(multi-cli): 文档同步——模块索引/CLAUDE.md/test-inventory（MC-8/109/110/318）`

**人工验证点**：终验人工走查——真实 claude 会话全功能回归（终端/四态/通知/历史/hub 面板），确认零回归交付。
