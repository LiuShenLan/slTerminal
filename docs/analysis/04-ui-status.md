# UI 状态指示 — claude 定制优化盘点

> 只读现状盘点（2026-08-08）。领域：针对 claude（Claude Code CLI）的 UI 状态指示与品牌/展示类定制。
> 专属程度口径：**硬编码 claude**（显式 claude 字样或假设其行为）/ **通用机制但 claude 触发**（机制通用，当前仅被 claude 事件/数据激活）/ **完全通用**（与 claude 无关的底层能力）。
> OSC 133 的**注入侧**（`shell-integration.ps1`）属 01 领域，本文仅引用消费侧联动，不展开注入实现。

## 相关文件

| 文件 | 职责 |
|------|------|
| `src/lib/claudeStatus.ts` | F3 四态映射单点：ClaudeStatus 类型、STATUS_EMOJI、eventToStatus 纯函数 |
| `src/lib/cliIcons.ts` | F9 CLI 品牌 logo 注册表（CliIconRegistry）+ 内嵌注册 claude 条目 |
| `public/cli-icons/claude.png` | claude 品牌 logo 图片资源（32×32，随 frontendDist 内嵌 exe） |
| `src/panels/terminal/TabTitleRegistry.ts` | 命令→页签标题/图标映射注册表（Registry Pattern 单例） |
| `src/panels/terminal/tabRules.ts` | 规则注册 side-effect 文件（claude 规则） |
| `src/panels/terminal/useCommandDetection.ts` | OSC 133 消费侧：C/D 序列解析 → 页签状态切换 |
| `src/panels/terminal/useXterm.ts` | 编排层：hook-event 订阅（F3 页签 emoji 直接通道 + claudeSession 写入） |
| `src/panels/terminal/TerminalPanel.tsx` | handleTabStateChange（title/icon/logo 条件更新与双清）+ originalTitleRef/logoRef |
| `src/panels/terminal/TerminalRegistry.ts` | claudeSession 二态模型（四态同源数据层） |
| `src/panels/terminal/usePtyOutput.ts` | PTY Exit 时页签状态重置（isCommandRunningRef 联动） |
| `src/workspace/PageDockviewHost.tsx` | DefaultTab（tabIcon emoji/img + tabLogo 渲染）+ F8 重命名禁用 |
| `src/features/agentStatus/` | Agent 状态视图：AgentStatusView / AgentStatusRow / useAgentStatus / consts.ts |
| `src/features/notifications/useClaudeNotifications.ts` | F4 通知调度：classifyEvent + 失焦门控 + 任务栏闪烁 + toast |
| `src/features/claudeHistory/historyModel.ts` | 历史区四态同源派生（deriveActiveSessionStatuses）+ formatRelativeTime |
| `src/features/claudeHistory/HistorySessionRow.tsx` | 历史区行：四态 emoji + CLI logo 消费 |
| `src/features/claudeHistory/useClaudeHistory.ts` | 历史区数据 hook：TerminalRegistry 订阅重算四态映射（activeStatuses） |
| `src/features/claudeHistory/HistorySessionList.tsx` | 历史区列表：claudeSession 运行中会话反查（导航消费） |
| 文档 | `src/panels/CLAUDE.md`（F3 四态 / OSC 133 / 中断行为）、`src/lib/CLAUDE.md`、`src/workspace/CLAUDE.md`（DefaultTab / F8）、`src/features/agentStatus/CLAUDE.md`、`src/features/notifications/CLAUDE.md`、`src/features/claudeHistory/CLAUDE.md` |

## 优化项清单

| # | 优化 | 位置(file:line) | 机制 | 触发点（claude 哪个行为） | 专属程度 |
|---|------|----------------|------|--------------------------|----------|
| 1 | F3 四态状态机（eventToStatus） | `src/lib/claudeStatus.ts:41-75` | hook 事件名 → 四态纯函数 | claude hook 协议事件（SessionStart/UserPromptSubmit/PreToolUse/PostToolUse/Notification/PermissionRequest/Stop/PostToolUseFailure/StopFailure/SessionEnd） | 硬编码 claude |
| 2 | F3 四态类型与 emoji 常量 | `src/lib/claudeStatus.ts:8-26` | ClaudeStatus 类型 + STATUS_EMOJI（⚡🟡✅❌）+ getStatusIcon | claude 会话状态语义（working/attention/done/error） | 硬编码 claude |
| 3 | F9 CLI 品牌 logo 注册表机制 | `src/lib/cliIcons.ts:21-44` | 注册表单例（register/match 首 token/getSrc/_reset） | claude 命令行首 token（OSC 133 C 路径）/ 侧栏活跃、历史区恒 claude | 通用机制但 claude 触发 |
| 4 | F9 claude logo 内嵌注册 + 图标资源 | `src/lib/cliIcons.ts:48` + `public/cli-icons/claude.png` | 注册行 `{command:"claude", src:"/cli-icons/claude.png"}` + 32×32 PNG（渲染 16×16） | claude 命令行 | 硬编码 claude |
| 5 | 命令→页签标题注册表机制 | `src/panels/terminal/TabTitleRegistry.ts:31-53` | 首 token 精确匹配注册表（覆盖带参变体） | 注册命令启动（OSC 133 C） | 通用机制但 claude 触发 |
| 6 | claude 页签标题规则 | `src/panels/terminal/tabRules.ts:11` | `register({command:"claude", title:"claude"})`（icon 字段已移除，emoji 由 F3 接管） | claude 命令启动 | 硬编码 claude |
| 7 | OSC 133 消费侧页签联动 | `src/panels/terminal/useCommandDetection.ts:42-77` | 133 handler 解析 C/D → 标题/🟡/logo 匹配 + matchedCommand 写入 + active 重置 | pwsh 集成脚本 Enter hook（133 C）/prompt()（133 D）发射的序列（注入侧属 01 领域） | 通用机制但 claude 触发 |
| 8 | 页签状态应用（handleTabStateChange） | `src/panels/terminal/TerminalPanel.tsx:78-91` | active=true 时 title/logo/icon 条件更新；active=false 恢复原标题（customTitle 优先）+ 双清 tabIcon/tabLogo；logoRef/originalTitleRef 保持 | claude 命令开始/退出、hook 事件、PTY spawn/Exit 重置 | 通用机制但 claude 触发 |
| 9 | hook-event 页签 emoji 直接通道 + 会话写入 | `src/panels/terminal/useXterm.ts:349-373` | onHookEvent 按 panelId 过滤 → eventToStatus → setClaudeSession + onTabStateChange({icon: emoji})；SessionEnd/Exit → setClaudeSession(null)（`{active:false}` 仅 SessionEnd，PTY Exit 复位走 usePtyOutput.ts:243-246）；空串归一 `\|\| undefined` | claude hook 信号文件事件流（hooks 模块注入侧） | 硬编码 claude |
| 10 | TerminalRegistry.claudeSession 二态模型 | `src/panels/terminal/TerminalRegistry.ts:12-21,80-98` | 存在即运行中（无 running 布尔）+ merge 语义（undefined 键不覆盖）+ sessionChange 订阅 | claude 会话生命周期（hook 事件 / OSC 133 matchedCommand / 面板关闭） | 硬编码 claude |
| 11 | DefaultTab 页签 emoji/logo 渲染 | `src/workspace/PageDockviewHost.tsx:226-289` | tabIcon 含 `/` 走 img 否则 span 渲染 emoji；tabLogo 16×16 仅随 emoji（`tabIcon && tabLogo` 双条件）；onDidParametersChange 接收扁平 Parameters | F3 四态 emoji + F9 logo 数据（经 updateParameters） | 通用机制但 claude 触发 |
| 12 | F8 重命名 claude 运行中禁用 | `src/workspace/PageDockviewHost.tsx:163-175` | `TerminalRegistry.get(panel.id)?.claudeSession != null` → 「重命名」disabled | claude 会话运行中 | 硬编码 claude |
| 13 | Agent 状态视图行建模（F5） | `src/features/agentStatus/useAgentStatus.ts` | 建行双通道（sessionChange 非 null ∨ hook 事件非 SessionEnd/Exit）/ 删行三通道 + 初始扫描（只建 claudeSession 非 null 行）+ transcriptPath 主动拉 contextUsage | claude 会话开始/结束/工具事件、面板关闭 | 硬编码 claude |
| 14 | 上下文用量条 | `src/features/agentStatus/consts.ts:5` + `AgentStatusRow.tsx:37-43` | `(inputTokens+cacheReadInputTokens+cacheCreationInputTokens) / 200_000` 百分比 + 三档分段色；outputTokens 不计占用 | claude transcript usage 数据（hooks_context_usage） | 硬编码 claude |
| 15 | 活跃区行双行式展示 | `src/features/agentStatus/AgentStatusRow.tsx` | 行1 = 四态图标 + CLI logo（`getSrc("claude")`:50）+ 标题 12px；行2 = 用量条 + 百分比 + 相对时间 11px（48px 缩进） | claude 会话状态/用量/最后事件时间 | 硬编码 claude |
| 16 | 相对时间 60s ticker | `src/features/agentStatus/useAgentStatus.ts:77-80` | setInterval 60s 重算 now → formatRelativeTime 重渲染 | （修复动机）idle claude 会话无 hook 事件时时间文本冻结的修复——60s ticker 持续运行、对所有活跃行生效，非被 claude 事件激活 | 通用机制但 claude 触发 |
| 17 | F4 通知调度 | `src/features/notifications/useClaudeNotifications.ts:50-68,117-160` | classifyEvent 三类映射（permission/error/done）+ 失焦门控 + sessionId\|event\|timestamp 去重 + 任务栏闪烁 + toast 正文 `<项目名> · <emoji 类别> · <时间>` | claude hook 事件（PermissionRequest/Notification+permission_prompt/StopFailure/PostToolUseFailure/Stop） | 硬编码 claude |
| 18 | 历史区四态同源与消费 | `src/features/claudeHistory/historyModel.ts:123-137` + `HistorySessionRow.tsx:51-54,85-93` | deriveActiveSessionStatuses（sessionId 优先/transcriptPath basename 回退）+ STATUS_EMOJI 渲染 + `getSrc("claude")` logo（仅随 status emoji，孤儿 ✗ 后不加图） | claude 会话状态（TerminalRegistry 同源）+ transcript 数据 | 硬编码 claude |
| 19 | 相对时间格式化（formatRelativeTime） | `src/features/claudeHistory/historyModel.ts` | 六档相对时间纯函数（刚刚/N 分钟/N 小时/N 天/同年 MM-DD/跨年 YYYY-MM-DD），mtimeMs≤0 →「-」 | 会话文件 mtime / 最后事件时间展示 | 完全通用 |

三档分布：硬编码 claude 12 项（1/2/4/6/9/10/12/13/14/15/17/18）；通用机制但 claude 触发 6 项（3/5/7/8/11/16）；完全通用 1 项（19）。

**跨文件重叠**（清单表格未逐行标注，汇总去重时以本表为准）：04-7↔01-20（OSC 133 消费侧，机制列与 file:line 高度重合）、04-1/2/9↔01-21 与 02-15（F3 四态消费链，同一区间 `useXterm.ts:349-373` + 空串归一）、04-18↔03-17（四态同源）、04-19↔03-22（formatRelativeTime）、04-17↔05-15（失焦门控/任务栏闪烁焦点联动）、04-3/4/15/18↔03-18（CLI logo 消费）。各条本身无分类冲突。

---

## 详细机制描述

### 1. F3 四态状态机（eventToStatus）— 硬编码 claude

`src/lib/claudeStatus.ts:41-75`。纯函数 `eventToStatus(event, notificationType?)` 把 Claude Code hook 协议事件名映射为四态：`SessionStart`→attention、`UserPromptSubmit`/`PreToolUse`/`PostToolUse`→working、`Notification`（仅 `permission_prompt`/`idle_prompt`/`agent_needs_input` 三类子类型，`ATTENTION_NOTIFICATION_TYPES` 集合 `claudeStatus.ts:29-33`）→attention、`PermissionRequest`→attention、`Stop`→done、`PostToolUseFailure`/`StopFailure`→error、`SessionEnd`→null；未识别事件 → null（不改变状态）。事件名全部为 claude hook 协议字面量，是 F3 四态的**唯一映射单点**（文档约束：`src/lib/CLAUDE.md`「四态映射单点：claudeStatus.ts 是 F3 四态唯一映射，组件不得另建映射」）。**生产消费方两处**：`useXterm.ts:353`（页签通道，见 #9）与 `useAgentStatus.ts:123`（活跃区行建模入口，`const newStatus = eventToStatus(...)`）——事件映射的全部消费方对多编码抽象是必要输入。**守护测试**：`src/__tests__/claude-status.test.ts`（32 用例）全分支守卫，含 `:206`「eventToStatus 返回 non-null 时 STATUS_EMOJI[status] 必定合法」映射-常量交叉一致性用例——映射单点的守护测试（对照 #4 的资源存在性守卫）。文件头注释（`claudeStatus.ts:4-6`）记载 claude 行为假设：Ctrl+C 主动中断不发射任何 hook 事件，working 无中断出边为预期行为，依赖下一事件覆盖或 `idle_prompt`（~60s）衰减转 🟡——该假设同时文档化于 `src/panels/CLAUDE.md`「中断场景已知行为（Ctrl+C）」段（三特征：滞留自愈/内置衰减/已知局限，局限标注 `eventToStatus` 无中断类事件映射）。

### 2. F3 四态类型与 emoji 常量 — 硬编码 claude

`src/lib/claudeStatus.ts:8-26`。`ClaudeStatus = "working" | "attention" | "done" | "error" | null`（类型名含 claude 字样，语义即 claude 会话状态）；`STATUS_EMOJI` 四态→emoji（⚡🟡✅❌，null 不入表）；`getStatusIcon(status)` 返回 emoji 或空串。**直接消费方两处**：`useXterm.ts:372`（页签路径的 emoji 取值点，`STATUS_EMOJI[status]`）与 `HistorySessionRow.tsx:51`（直接索引 `STATUS_EMOJI[status]`）；另 `getStatusIcon` 内部亦消费 `STATUS_EMOJI`——第三条内部消费路径，供桥接消费方 AgentStatusRow 活跃区使用（`AgentStatusRow.tsx:47` 消费 `getStatusIcon`；DefaultTab 页签经 TerminalPanel 的 updateParameters 间接到达）——**展示点三处成立**（页签/活跃区/历史区），emoji 状态指示联动展示同一映射。

### 3. F9 CLI 品牌 logo 注册表机制 — 通用机制但 claude 触发

`src/lib/cliIcons.ts:21-44`。`CliIconRegistry` 模块级单例：`register`（同 command 覆盖）/`match`（命令行首 token `trim().split(/\s+/)[0]` 精确查表，覆盖 `claude --resume`/`claude -p` 等带参变体，`cliIcons.ts:30-33`）/`getSrc`（精确键查询）/`_reset`（测试）。注册表模式本身通用（文档注释 `cliIcons.ts:7-10`：新增编码 CLI 两步——`public/cli-icons/<命令>.png` 放图 + 追加一行 register），但当前**唯一实际注册条目为 claude**（见 #4），三处消费点（页签经 `CliIconRegistry.match`、活跃区与历史区经 `getSrc("claude")`）均为 claude 数据驱动。**首 token 解析双份实现**：与 #5 的 `TabTitleRegistry.match`（`TabTitleRegistry.ts:41-44`）为同一 `trim().split(/\s+/)[0]` 逻辑的两份拷贝（`cliIcons.ts:30-33` vs `TabTitleRegistry.ts:41-44`）——多 CLI 抽象时首 token 解析逻辑应单点化。文档约束：`src/lib/CLAUDE.md`「CLI 图标映射单点：cliIcons.ts 是 CLI → 品牌 logo 唯一映射（F9），新增 CLI 在此注册」。

### 4. F9 claude logo 内嵌注册 + 图标资源 — 硬编码 claude

`src/lib/cliIcons.ts:48`：模块加载副作用 `cliIconRegistry.register({ command: "claude", src: "/cli-icons/claude.png" })`——显式 claude 字样。`public/cli-icons/claude.png`：32×32 透明底 PNG（渲染 16×16），随 frontendDist 内嵌 exe，根绝对路径同源加载（CSP `img-src 'self'` 放行，注释见 `cliIcons.ts:8-9`）。展示点：DefaultTab 页签（`PageDockviewHost.tsx:274-277`）、AgentStatusRow 活跃区（`AgentStatusRow.tsx:88-91`）、HistorySessionRow 历史区（`HistorySessionRow.tsx:88-91`）。测试 `src/__tests__/cli-icons.test.ts` 以 claude 条目为断言数据；其中 `cli-icons.test.ts:77-84` 为**资源存在性守卫用例**——断言 `public/cli-icons/claude.png` 磁盘存在 + PNG 魔数校验（`0x89 0x50 0x4e 0x47…`），资源缺失无任何报错通道（img 404 静默），靠此用例守护；多 CLI 抽象时守卫需随注册条目泛化（每 CLI 一个资源守卫）。

### 5. 命令→页签标题注册表机制 — 通用机制但 claude 触发

`src/panels/terminal/TabTitleRegistry.ts:31-53`。`TabTitleRegistry` 模块级单例：`register(rule)`/`match(command)`（首 token 精确匹配）/`_reset`。`TabState` 接口（`TabTitleRegistry.ts:18-28`）含可选 `logo` 字段（F9：OSC 133 C 携带的 CLI 品牌 logo，hook 事件路径无 command 不传）。机制通用（注释 `TabTitleRegistry.ts:4-5`：通过注册表匹配命令，不硬编码单个命令名），但当前注册规则仅 claude 一条（#6），且 `match` 注释示例即 `claude --resume`。

### 6. claude 页签标题规则 — 硬编码 claude

`src/panels/terminal/tabRules.ts:11`：`tabTitleRegistry.register({ command: "claude", title: "claude" })`——显式 claude 字样。规则注册经 side-effect import（`useXterm.ts:40` `import "./tabRules"`）。`icon` 字段已移除（`tabRules.ts:10` 注释：emoji 表示由 F3 四态系统接管，不再硬编码图标）。匹配命中时页签标题切换为 "claude"（OSC 133 C 路径）。

### 7. OSC 133 消费侧页签联动 — 通用机制但 claude 触发

`src/panels/terminal/useCommandDetection.ts:42-77`。注册 `term.parser.registerOscHandler(133, ...)`（xterm 解析器剥离 133 前缀，handler 收到 `"C;claude"` 或 `"D;0"`）：

- **C 序列**（命令即将执行，`useCommandDetection.ts:46-61`）：提取命令行 → `tabTitleRegistry.match(command)` 命中则置 `isCommandRunningRef = true` → `onTabStateChange({ active: true, title: rule.title, icon: "🟡", logo: cliIconRegistry.match(command) })`（attention 态 + F9 logo）→ `TerminalRegistry.setClaudeSession(panelId, { matchedCommand: rule.command })`（`useCommandDetection.ts:60`——未注入 hooks 时无 transcriptPath，matchedCommand 仍可建会话行）。**`icon: "🟡"` 为字面量**（`useCommandDetection.ts:56`，不经 `STATUS_EMOJI.attention`）——OSC 133 路径不经 `eventToStatus`，是「四态映射单点」约束（`src/lib/CLAUDE.md`）的现状例外点；多 CLI 抽象时该点需统一
- **D 序列**（命令退出，`useCommandDetection.ts:62-68`）：`isCommandRunningRef` 为 true 时置 false → `onTabStateChange({ active: false })` → `setClaudeSession(panelId, null)` 清会话
- 返回 false 不消费序列（提示符仍渲染）；handler 回调经 `onTabStateChangeRef` 防闭包过期

序列发射侧为 `shell-integration.ps1` 的 Enter hook（133 C，携命令行）与 `prompt()`（133 D，携退出码）——**注入侧属 01 领域，本文仅记消费侧联动**。另两条重置路径：PTY spawn 成功后 `resetCommandState()`（`useCommandDetection.ts:80-83`，`useXterm.ts:291` 调用，覆盖布局 JSON 持久化残留）；PTY Exit 时 `usePtyOutput.ts:243-246` 若 `isCommandRunningRef` 为 true 同样重置页签。机制对任意命令通用（OSC 133 由 pwsh 集成脚本为一切命令发射），但当前规则/logo 命中仅 claude（文档记载于 `src/panels/CLAUDE.md`「OSC 133 命令边界检测 + 页签标题/图标动态切换」段，含「仅限于 pwsh/powershell——cmd.exe 无此能力」）。

### 8. 页签状态应用（handleTabStateChange）— 通用机制但 claude 触发

`src/panels/terminal/TerminalPanel.tsx:74-91`。`onTabStateChange` 桥接 useXterm → Dockview API（`api.setTitle` / `api.updateParameters`，不引入新 IPC 命令，`src/panels/CLAUDE.md`「IPC 边界」段）：

- `active=true`：仅 `state.title` 存在才 `setTitle`；`state.logo !== undefined` 才更新 `logoRef.current`（OSC 133 C 路径更新、hook 事件路径不传 logo 保持前值）；`state.icon !== undefined` 才 `updateParameters({ ...params, tabIcon: state.icon, tabLogo: logoRef.current })`
- `active=false`：`setTitle(originalTitleRef.current)`（`:87`）恢复原标题 + **双清** `tabIcon: null, tabLogo: null`（覆盖布局 JSON 持久化残留，`updateParameters` 实际在 `TerminalPanel.tsx:89`；`:88` 为注释行）

配套 ref：`originalTitleRef`（`TerminalPanel.tsx:74`）挂载时取 `params.customTitle ?? api.title ?? "terminal"`（F8 自定义标题优先，防 claude 运行中退出保存的瞬态 title），并订阅 `onDidParametersChange` 在 `customTitle !== undefined` 时同步（`TerminalPanel.tsx:95-103`）；`logoRef`（`TerminalPanel.tsx:77`）初始 `params.tabLogo ?? null`，照 originalTitleRef 模式保持当前 CLI logo。机制为通用页签状态应用，触发数据（title/emoji/logo）全部来自 claude 命令/hook 路径。

### 9. hook-event 页签 emoji 直接通道 + 会话写入 — 硬编码 claude

`src/panels/terminal/useXterm.ts:349-373`。订阅 `hooks.onHookEvent`，按 `payload.panelId === panelId` 过滤：`eventToStatus(payload.event, payload.notificationType)` 求四态 → `SessionEnd`/`Exit` 时 `setClaudeSession(panelId, null)`（`useXterm.ts:354-355`，**`:354` 的 Exit 条件为防御代码**——按当前注入的 C9 10 事件清单（02-1）不存在 Exit 事件（推断性依据：**注入侧**——C9 清单（inject.rs:16-27）与 reporter 脚本无 Exit 事件；消费侧注释有记载（useAgentStatus.ts:4-5、:126），准确表述为「注入侧无 Exit」），Exit 属 PTY 通道 `PtyEvent`，页签复位见 `usePtyOutput.ts:243-246`）；**`onTabStateChange({ active: false })` 仅 `SessionEnd` 触发**（`:368-370`，正常分支非防御代码）。其余事件 `setClaudeSession(panelId, { sessionId, transcriptPath, status })`（**payload 空串归一 `|| undefined`**，`useXterm.ts:361-364`——claude hook 输入缺字段时下游 derive/标题覆盖/usage 拉取全部失效的防御，`src/panels/CLAUDE.md`「useXterm」文件表行）→ 状态非 null 时 `onTabStateChange({ active: true, icon: STATUS_EMOJI[status] })`（hook 路径无 command，不传 logo，TerminalPanel 层保持前值）。事件源为 claude hook 信号文件事件流（注入侧属 hooks 模块 `src-tauri/src/hooks`，10 事件含 **8 个产生状态的事件**——按 `eventToStatus` 映射口径：SessionStart/UserPromptSubmit/PreToolUse/PostToolUse/Stop/PermissionRequest/PostToolUseFailure/StopFailure 恒产生状态，Notification 视子类型（仅三类 attention 子类型），SessionEnd 恒 null；「6 个」= F3 表格触发源列列举的 hook 事件数（PreToolUse/PostToolUse/Notification/Stop/PostToolUseFailure/StopFailure），**触发源列为非全量列举**——未列 attention 类 SessionStart/PermissionRequest 与 working 类 UserPromptSubmit）。

### 10. TerminalRegistry.claudeSession 二态模型 — 硬编码 claude

`src/panels/terminal/TerminalRegistry.ts`。`ClaudeSessionInfo`（`:12-21`：sessionId/transcriptPath/matchedCommand/status/lastEventAt）**存在即运行中**（无 running 布尔，`claudeSession` 为 null = 明确无会话、undefined = 未设置，`:28-29` 注释）。`setClaudeSession`（`:80-98`）merge 语义（patch 中 undefined 键不覆盖旧值、null 清空、缺 lastEventAt 自动填 `Date.now()`）+ `sessionChange` 事件通知；`register` 幂等覆盖时 `claudeSession` 缺省保留旧值（`:46-50`，StrictMode/重试场景不丢 session）。是 F3 四态同源架构的数据层——**四路消费**：活跃区行（useAgentStatus 订阅）、历史区四态（deriveActiveSessionStatuses 读取）、F8 重命名禁用（PageDockviewHost 读取）、**03 领域运行中会话导航反查**（`HistorySessionList.tsx:192-204` `findPanelForSession` 函数体——sessionId 精确匹配 + transcriptPath basename 回退 → `switchToPageAndFocus`，SessionActionDialog「切换到该会话操作页面」路径）；**写入方**为 useXterm（hook 事件，04-9）与 useCommandDetection（OSC 133 matchedCommand，04-7），非消费方。类型名与语义均硬编码 claude（文档：`src/panels/CLAUDE.md`「TerminalRegistry」文件表行——claudeSession 存在即运行中，二态模型）。

### 11. DefaultTab 页签 emoji/logo 渲染 — 通用机制但 claude 触发

`src/workspace/PageDockviewHost.tsx:226-289`。`tabParams.tabIcon` 含 `/` 或 `\` 或 `http:`/`data:` 前缀 → `<img>` 渲染，否则 `<span>` 渲染 emoji（`:257-272`）；`tabLogo`（`params.tabLogo`）在 emoji 后渲染 16×16 logo img（`alt="CLI 图标"`，与 URL tabIcon 的 `alt="页签图标"` 区分），**渲染条件 `tabIcon && tabLogo`（仅随 emoji，双清双保险）**（`:274-277`）。订阅 `api.onDidParametersChange`——**回调直接接收扁平 `Parameters` 对象（`event.tabIcon`，非 `event.params.tabIcon`）**（`:240-246`，文档「关键坑」段：错误写法导致始终读到 undefined，`workspace-defaulttab.test.tsx` 有漂移即失败的结构回归用例）。渲染机制本身为通用 UI 能力，但 tabIcon/tabLogo 的值域当前仅由 F3 四态 emoji + F9 claude logo 产生。

### 12. F8 重命名 claude 运行中禁用 — 硬编码 claude

`src/workspace/PageDockviewHost.tsx:163-175`。右键菜单「重命名」项（7 项结构 `[新建终端, sep, 重命名, sep, close, closeOthers, closeAll]`）的 `disabled` 判定：`TerminalRegistry.get(params.panel.id)?.claudeSession != null`（`:165`，claudeSession 存在即运行中二态模型），菜单每次右键重新构建判断实时。文档：`src/workspace/CLAUDE.md`「终端页签自定义重命名（F8，右键菜单）」段。联动：OSC 133 D / SessionEnd 恢复标题时用 `customTitle`（F8 与 F3 的标题恢复链路耦合，见 #8）。

### 13. Agent 状态视图行建模（F5）— 硬编码 claude

`src/features/agentStatus/useAgentStatus.ts`。行 = **当前活跃项目内**运行中的 claude 会话——**仅当前活跃项目过滤门控**：hook 通道 `if (!pageIds.has(pageId)) return`（`:118`）与 `TerminalRegistry.subscribe` 回调（sessionChange/remove 通道）`if (!pageIds.has(pageId)) return`（`:215`）各一处过滤，加初始扫描 `if (!projectPageIds.has(pageId)) continue`（`:295`）共三重过滤，行建模语义含「活跃项目维度」（非 claude 专属，抽象设计需保留该维度）；`claudeSession` 为 null/undefined 的纯 shell 终端**不建行**（`:291` 初始扫描 `if (!entry.claudeSession) continue`）：

- **建行双通道幂等**（`:160-176, 221-237`）：`sessionChange` 事件 session 非 null ∨ hook 事件非 SessionEnd/Exit 且行不存在——**三路径建行新行均默认 status=attention（🟡）**（`:166` `status: newStatus ?? "attention"`、`:230`/`:302` `status: "attention"`），待后续 hook 事件覆盖；多 CLI 抽象时保留该默认态语义（非 claude 会话的新行默认态需另行定义）
- **删行三通道**（`:126-135, 253-272`）：`sessionChange` session 为 null ∨ SessionEnd/Exit 事件 ∨ `remove`（面板关闭）——**Exit 通道为防御代码**（注入侧无 Exit 事件，见 #9；实际删行两通道：sessionChange null + remove）
- **更新已有行**（`:144-158`）：行已存在时仅更新 title/lastEventAt/transcriptPath/sessionId，**status 为 null 不覆盖旧值**（`...(newStatus !== null ? { status: newStatus } : {})`，与 #9 的 `status: status ?? undefined` 同一语义、两处实现），并按 lastEventAt 重排序
- **初始扫描**（`:280-327`）：只建 `claudeSession` 非 null 的行，携 `transcriptPath` 主动拉 `contextUsage`（修复切项目后 idle 会话用量永远 --）
- **竞态双保险**（`:83-106, 193`）：双 listener 经 ref 读最新状态 + deps `[]` 订阅永不重建 + 初始扫描对账兜底
- 事件含 transcriptPath 时异步拉取 `contextUsage` 填充 `row.usage`（`:179-191`）

视图状态机三态：`no-root`（「选择一个项目以查看 Agent 状态」）/`empty`（「当前项目无运行中的 claude 会话」，文案含 claude 字样，E2E 兼容红线）/`ready`（`AgentStatusView.tsx:171-190`）。行点击 → `parseTerminalPageId` → `switchToPageAndFocus` 跳转聚焦对应终端页签（`AgentStatusView.tsx:115-122`）。**活跃区标题覆盖**（问题 6 修复）：渲染行标题经 `titleBySessionId` 用历史区 scan 数据（claude transcript custom-title/ai-title 回退链产物）覆盖，`/rename` 后刷新即同步——机制完整描述见 03-23，本条仅标注链路归属。数据源与建模语义均硬编码 claude（文档：`src/features/agentStatus/CLAUDE.md`「行建模：建行双通道 / 删行三通道（F5）」段）。

### 14. 上下文用量条 — 硬编码 claude

`src/features/agentStatus/consts.ts:5`：`CLAUDE_CONTEXT_LIMIT = 200_000`——claude 上下文窗口上限常量（跨边界契约：仅此文件定义，其他文件引用不复制）。`src/features/agentStatus/AgentStatusRow.tsx:37-43`：用量口径 = `(inputTokens + cacheReadInputTokens + cacheCreationInputTokens) / CLAUDE_CONTEXT_LIMIT` 百分比（`outputTokens` 不计占用，保留为信息字段），clamp 100；`usageBarColor`（`:23-27`）三档分段色（<50 低 / ≤80 中 / >80 高，`AGENT_STATUS_USAGE_COLORS` token）；usage 为 null 时**进度条以 DIM_FG 填充 100% 宽**（`:127-138`）、**百分比文本区显示 `--`**（`:148`）。数据来自 claude transcript usage（`hooks_context_usage` 后端命令，尾部 64KB 逆行扫描）。200K 常量与三 token 口径均硬编码 claude（文档：`src/features/agentStatus/CLAUDE.md`「用量口径」段）。

### 15. 活跃区行双行式展示 — 硬编码 claude

`src/features/agentStatus/AgentStatusRow.tsx`。行1（`:74-106`）= 40px 图标列 flex 簇（四态 emoji + **CLI logo 16×16**，`getSrc("claude")`（`:50`，注释：当前侧栏会话均为 claude，未来按行 CLI 标识扩展；logo 仅随 emoji，icon 空时渲染空列占位防标题漂移））+ 标题 12px 粗体截断；行2（`:109-159`）= 用量条（80×6px）+ 百分比 + 相对时间（11px 灰，`paddingLeft: 48px` 对齐图标列）。（**边界说明**：null 态渲染（DIM_FG 100% 宽 + `--`）归属 #14 数据口径、行2 布局（80×6px/48px 缩进/11px 灰）归属本条展示布局——两节覆盖同一 DOM 区间 `:109-159`，非双重计数。）`getSrc("claude")` 为显式 claude 字样。

### 16. 相对时间 60s ticker — 通用机制但 claude 触发

`src/features/agentStatus/useAgentStatus.ts:77-80`：`setInterval(() => setNow(Date.now()), 60_000)` 驱动 `formatRelativeTime(row.lastEventAt, now)` 重算（真实调用在 `AgentStatusRow.tsx:51`）。机制为通用定时刷新，但动机为 claude 场景修复——idle 会话无 hook 事件时组件不重渲染、时间文本永久冻结（文档：`src/features/agentStatus/CLAUDE.md`「相对时间 60s ticker」段）。时间口径与历史区统一（`formatRelativeTime`，#19）。

### 17. F4 通知调度 — 硬编码 claude

`src/features/notifications/useClaudeNotifications.ts`。`classifyEvent` 纯函数（`:50-68`）事件名硬编码映射：permission（`PermissionRequest` ∨ `Notification`+`notificationType === "permission_prompt"`）/ error（`StopFailure` ∨ `PostToolUseFailure`）/ done（`Stop`）；其他事件（PreToolUse/PostToolUse/SessionStart/SessionEnd）不触发。调度链（`:117-160`）：失焦门控（`window.__slterm_windowFocused !== false` 时不触发）→ 去重（`sessionId|event|timestamp` 键，缓存超 200 条截断保留最近 100 条）→ 项目名反查（`parseTerminalPageId` → `useProjects`）→ 三类事件均 `flashTaskbar()`（`UserAttentionType.Critical`，持续闪烁到聚焦——toast 失去点击路由后唯一回窗引导通道）→ `sendToastNotification("slTerminal", { body })`，正文格式 **`<项目名> · <emoji 类别标签> · <时间>`**（`CATEGORY_EMOJI` 🔐✅❌ + `CATEGORY_LABEL` 权限请求/任务完成/错误，`:28-39, 147-153`）。**emoji 常量分裂**：`CATEGORY_EMOJI`（🔐✅❌，`useClaudeNotifications.ts:28-32`）与 `STATUS_EMOJI`（⚡🟡✅❌，#2）为两套独立维护的 emoji 映射（✅❌ 重复）——F4 与 F3 各自维护，抽象统一四态 emoji 需知此分裂。权限懒初始化（`permissionEnsured` 模块级标记，`:96, 110-115`）。**调度挂载点**：`NotificationListener` 组件（`useClaudeNotifications.ts:172-175`，App.tsx 挂载一次的无 UI 副作用包装，内部调用 `useClaudeNotifications()`）——F4 调度的唯一入口事实，抽象拆分通知模块时以此为挂载边界。事件源与分类映射均硬编码 claude（文档：`src/features/notifications/CLAUDE.md`「事件分类（classifyEvent 纯函数）」「失焦门控」「toast 正文」段）。

### 18. 历史区四态同源与消费 — 硬编码 claude

`src/features/claudeHistory/historyModel.ts:123-137`：`deriveActiveSessionStatuses()` 读 `TerminalRegistry.getAll()` 条目 → `Map<sessionId, ClaudeStatus>`（sessionId 优先，回退 transcriptPath basename 去 `.jsonl` 兼容旧数据；matchedCommand-only 会话无两者可定位——文档化局限；status 为 null 不产出键——与活跃区 null 无图标语义一致）。`useClaudeHistory.activeStatuses` 经 `TerminalRegistry.subscribe` 实时跟随（register/remove/sessionChange 任一事件重算，不重扫）。`src/features/claudeHistory/HistorySessionRow.tsx:51-54, 85-93`：`statusIcon = STATUS_EMOJI[status]`；**CLI logo 仅随 status emoji 渲染**（`statusIcon && logoSrc`，孤儿 ✗ 后不加图），`logoSrc = cliIconRegistry.getSrc("claude")`（`:54` 显式 claude）。历史区与活跃区**四态同源**（问题 2 修复，文档：`src/features/claudeHistory/CLAUDE.md`「四态同源（问题 2 修复）」段）。

### 19. 相对时间格式化（formatRelativeTime）— 完全通用

`src/features/claudeHistory/historyModel.ts`：六档相对时间纯函数——刚刚（<1min）/ N 分钟前 / N 小时前 / N 天前（<7d）/ 同年 `MM-DD` / 跨年 `YYYY-MM-DD`；mtimeMs ≤ 0 →「-」。零 claude 依赖的通用底层能力，被活跃区（`AgentStatusRow.tsx:51`）与历史区（`HistorySessionRow.tsx:50`）共用，口径统一（决策 26：时间口径 = 文件 mtime / 最后事件时间）。

## 跨项联动关系（现状）

- **F3 双源合成**：页签四态 = hook-event 路径（useXterm 订阅，#9）+ OSC 133 路径（useCommandDetection，#7）双源，**实际为事件覆盖（last-write-wins，后到事件直接覆盖前态），无运行时仲裁**——如 PreToolUse 之后的 permission_prompt 会覆盖 ⚡ 为 🟡；「优先级：working（PreToolUse/PostToolUse）> attention（OSC 133 C 或 Notification/PermissionRequest）> done（Stop）> error（PostToolUseFailure/StopFailure）」是 `src/panels/CLAUDE.md` F3 表格「优先级自上而下」的状态语义层级转述，非仲裁机制，`SessionEnd`/OSC 133 D 清图标。
- **四态同源数据层**：TerminalRegistry.claudeSession（#10）是活跃区/历史区/F8 禁用/03 领域导航反查四路消费的单一真值源（逐路明细见 #10 详述）；写入双源——hook 事件（04-9）与 OSC 133（04-7）（`src/features/claudeHistory/CLAUDE.md`「四态同源」段）。
- **F9 三处消费**：CliIconRegistry（#3/#4）被页签（OSC 133 C 经 `match`）、活跃区（`getSrc("claude")`）、历史区（`getSrc("claude")`）消费（消费明细见 #3 详述）；页签与活跃区 logo 双清路径独立（TerminalPanel inactive 双清 tabIcon+tabLogo；AgentStatusRow 行随删行消失）。
- **F8 与 F3 耦合**：重命名禁用依赖 claudeSession（#12）；OSC 133 D / SessionEnd 标题恢复用 customTitle（#8）。
