# Phase 2 开发验证 — 问题与根因分析

> 范围：对照 `docs/hooks-dev/phase2/`（checklist 31 项 + stages 验证项）的静态符合性 review + 五个人工验证问题的根因。只记录问题，符合项从略。
> 日期：2026-07-28
> 测试基线：L1 351+8 全过；L2 1668/1669（唯一失败 `diff-panel.test.tsx > .cm-scroller exists` 单独复跑 30/30 通过，为 pre-existing flake，与 Phase 2 无关）；tsc / eslint / clippy 全绿。
> 验证手段：静态核对 + 测试基线 + claude 信号链实验（真实 claude 写信号文件）+ VITE_E2E 二进制内 WDIO 实测复现（R1–R4 场景 spec + 通知探针，临时 spec 用后已删）。

## 一、不符合开发计划的项

### 1. [P2-FE-10] F5 行建模偏离 feature-plan：行 = 全部终端面板，非「运行中的 claude 会话」（严重度：高）

- **计划要求**（feature-plan 拍板，用户已确认以此为准）：
  - 视图总览「当前项目所有**运行中的 claude 会话**」（`docs/hooks-dev/feature-plan/phase2-notify-overview.md:62`）；
  - 边界 3：会话退出（SessionEnd / OSC 133 D）→ 该行**移除**，视图是运行中会话总览、非历史列表（同文件 :90）；
  - 边界 5：未注入 hooks 的 claude 会话——行存在（**OSC 133 C 检测到启动**建行）、四态 🟡、用量条不可用态（同文件 :92）。
- **实际**：行生命周期绑定 TerminalRegistry——PTY spawn（register）即建行（**任何终端**，含从未跑 claude 的）、关页签（remove）删行、切项目时从 `TerminalRegistry.getAll()` 全量初始扫描重建（`src/features/agentStatus/useAgentStatus.ts:186-221,224-252`）。OSC 133 C 检测建行未实现。
- **证据**：`useAgentStatus.ts:186-252`；`phase2-notify-overview.md:62,90,92`；checklist P2-FE-10 按 TerminalRegistry 语义描述（与 feature-plan 冲突，用户拍板以 feature-plan 为准）。
- **连带**：人工验证问题 1（`--resume` 无行变化）、问题 3（行复活 / 残留）均源于此建模。
- **修复方向**：行建模改为「claude 会话」——OSC 133 C（命令匹配）+ SessionStart（hook 事件）双通道建行，SessionEnd / OSC 133 D 删行；初始扫描不能直接照 TerminalRegistry 重建（注册表无会话信息），需引入会话状态跟踪（TerminalRegistry 扩展记录最近匹配命令，或 useCommandDetection 上报命令运行态），使切项目重建时能区分「活着的 claude 会话」与「纯 shell 终端」。

### 2. [P2-FE-02 / P2-FE-06] toast 技术路径平台级失效：Web Notification API 在未打包 Win32 WebView2 上功能不全（严重度：高）

- **计划要求**：`sendClickableNotification` 用 `new Notification()` 发可点击 toast（注释假设「WebView2 环境下 Notification API 委托 OS 原生通知中心」，`src/ipc/notification.ts:33`）；onclick 聚焦窗口 + 路由面板（P2-FE-06）；构造抛异常时 catch 回退 Tauri `sendNotification`。
- **实际**（VITE_E2E debug 二进制内探针实测 + 用户双机实测）：
  - 探针：`Notification.permission === "granted"`、`new Notification()` 构造**不抛异常**（→ catch 回退路径永不触发）、实例**无 `close` 方法**（`TypeError: n.close is not a function`）——WebView2 的 Notification 是残缺 shim。
  - Win11（用户实测）：横幅不弹，静默进通知中心（banner 抑制）。
  - Win10（用户实测）：通知可见但点击完全无反应（onclick 未路由回应用）。
- **根因**：未打包 Win32 应用无 AUMID（无开始菜单快捷方式 / AppUserModelID 注册），Windows 通知的 banner 展示与激活回调（COM activator）无法路由回 WebView2 宿主。计划「委托 OS 原生通知中心」的假设在此形态下不成立。
- **证据**：`src/ipc/notification.ts:40-62`；探针实测输出 `{"created":true,"permission":"granted","thrown":"TypeError: n.close is not a function"}`；用户双机实测。
- **性质**：平台级限制 + 计划假设失实，非实现 bug。
- **修复方向**（候选，拍板留 fix 阶段）：
  - a) 改走 Tauri 原生 `sendNotification`——需先实测未打包环境下是否同样受 AUMID 限制（大概率同样受限，且 Tauri v2 通知无 onclick 契约，C12 已核实）；
  - b) 注册 AUMID + 开始菜单快捷方式——与当前免安装 zip 发布形态冲突；
  - c) 改设计：toast 仅作提示（放弃点击路由），路由诉求由任务栏闪烁（`requestUserAttention`，独立路径可用）+ 应用内通知列表承担；
  - d) Rust 侧自实现 WinRT toast + COM activator 注册（工作量最大）。

### 3. [P2-FE-08 连带] 用量刷新单通道缺口：初始扫描不带 transcriptPath/usage，切项目后无新事件则永远「--」（严重度：中）

- **计划要求**：更新时机「新信号事件到达时顺带更新该行的四态与上下文用量（事件驱动，不做定时轮询）」（`phase2-notify-overview.md:84`）；验收 2「上下文用量随 claude 工作推进而更新」（同文件 :105）。
- **实际**：usage 唯一来源是「hook 事件含 transcriptPath 时拉取」（`useAgentStatus.ts:158-172`）；切项目后初始扫描重建的行 `usage: undefined` 且**无 transcriptPath**（:239-247）——会话此后无新 hook 事件（如 idle 等输入）时，用量条永远「--」。「事件驱动」在切项目场景下无事件可驱动。
- **证据**：`useAgentStatus.ts:158-172,224-252`；R2 实测复现（切项目往返后行 `🟡...--`）。
- **说明**：实现符合「不做轮询」约束，但「切项目重建后用量恢复」路径计划与实现双双缺失。
- **修复方向**：初始扫描 / register 建行时，若会话状态跟踪（不符合项 #1 的修复）携带最近已知 transcriptPath，主动 `contextUsage` 拉取一次。

### 4. [C12 契约] ContextUsage 语义低估：仅 input_tokens + output_tokens，不含 cache tokens（严重度：中）

- **计划要求**：C12 定义 `ContextUsage { inputTokens, outputTokens }`，transcript 尾部逆行扫描 `message.usage`（`docs/hooks-dev/contract.md:124`）。
- **实际**：实现照契约取两字段（`src-tauri/src/hooks/usage.rs:75-85`）；但 claude transcript 的 `message.usage` 还含 `cache_read_input_tokens` / `cache_creation_input_tokens`——真实会话中 cache tokens 占上下文窗口大头（常 >90%）。当前百分比 = (input+output)/200K，严重低估实际占用（显示 10% 时实际可能已 80%+）。
- **证据**：`usage.rs:75-85`；`contract.md:124`。
- **说明**：契约层缺口（C12 定义即漏 cache 字段），非实现偏离；但直接导致 F5 核心卖点「上下文还剩多少」数值失实。
- **修复方向**：`ContextUsage` 增 cache 两字段，百分比按四字段总和计；需改 C12 + `usage.rs` + `AgentStatusRow` 三处（DTO 双边对应，硬约束 #4）。

### 5. [P2-FE-10 连带] 关页签删行事件丢失：TerminalRegistry.remove 与 effect 重订阅同 commit 交错（严重度：中）

- **计划要求**：checklist P2-FE-10「TerminalRegistry subscribe remove → 删行」。
- **实际**（R4 实测）：关页签 4s 后行仍残留，且行时间戳 = 建行时刻（证明**从未被删**，非删后重建）。
- **机制**（高置信推断，与实测一致）：关页签触发 Dockview 面板卸载 + `onDidLayoutChange` → `updatePageLayout` → projects 不可变更新 → `activeProject` 新引用 → `projectPageIds` 新 Set → useAgentStatus registry effect deps 变化需重订阅（`useAgentStatus.ts:221`）。React 18 同 commit 的 passive destroy 按 fiber 顺序执行：SideBarArea 子树（unsub，pane2）先于主区子树（useXterm cleanup 中 `TerminalRegistry.remove` 同步 notify，pane3）→ remove 事件发出时 listener 已退订 → 事件丢失 → 行残留。
- **证据**：`useAgentStatus.ts:186-221`（effect deps :221）；useXterm cleanup（`TerminalRegistry.remove`）；R4 实测输出（行 `🟡...--` 残留，时间戳未更新）。
- **修复方向**：删行不依赖订阅时序——初始扫描 / 事件处理时以 TerminalRegistry 现值对账（reconcile：行在 registry 中不存在则移除），或将 registry listener 挂到不随 deps 重建的 ref/模块级订阅上。随不符合项 #1 的行建模重设计一并解决。

## 二、人工验证问题根因

### 问题 1：`claude --resume xxxx` 不触发页签改名与 agent 行变化

**现象**：终端执行 `claude --resume xxxx` 恢复历史会话，页签不改名（停留 terminal-N），agent 视图无对应新记录。

**根因**：命令匹配过窄，两层失效——

1. **TabTitleRegistry.match 是整行精确匹配**（`src/panels/terminal/TabTitleRegistry.ts:37-39`，`this.rules.get(command)`）；useCommandDetection 把 OSC 133 C 携带的**完整命令行**喂入（`useCommandDetection.ts:45-46`）；`tabRules.ts` 仅注册 `command: "claude"`。`claude --resume xxxx`（及任何带参形式 `claude -p` / `claude --model xx`）≠ `"claude"` → match 返回 null → 不改名、不置 `isCommandRunningRef`、不发 🟡。
2. **agent 行标题不刷新**：行标题经 `resolveTitle` 读页签标题（`useAgentStatus.ts:50-59`）——页签不改名 → 行永远显示 terminal-N。行本身在终端 spawn 时已按「行=终端」建模建出（不符合项 #1），用户感知的「没有添加记录」实为行标题停留 terminal-N、无可见变化。
3. **hook 事件链本身正常**：SessionStart/UserPromptSubmit 等照发（信号链实验证实），四态 ⚡/✅ 流转不受影响——仅 OSC 133 命令检测通道失效。

**性质**：实现 bug（匹配逻辑只覆盖裸命令，未考虑带参形式）。

**修复方向**：`match` 取命令行首 token（空白分割）再精确匹配；或规则引擎支持前缀/正则匹配（需同步 TabTitleRegistry 测试与 tabRules 语义）。

### 问题 2：切视图 / 切项目后用量条显示空

**现象**：场景 a（agent-status → 其他视图 → 切回）、场景 b（切项目再切回）后用量条显示「--」。

**根因**（分场景）：

- **场景 a（纯点击切视图）：实测不复现**（R1 PASS）——SideBarArea 槽位 `display:none` 保挂载，rows state 与 usage 保持。用户观察大概率命中以下两个真实子场景之一：① 曾**拖拽按钮换区**（ADR-0001 已知行为：换区触发组件卸载重建 → 初始扫描 → 与场景 b 同机制丢用量）；② 与切项目操作混合。
- **场景 b（切项目）：实锤**（R2 复现）——切项目后初始扫描重建行 `usage: undefined` 且无 transcriptPath（`useAgentStatus.ts:239-247`），usage 唯一来源是 hook 事件顺带拉取（:158-172），会话 idle 无新事件 → 永远「--」。即不符合项 #3。

**性质**：实现缺口（切项目/换区重建路径无用量恢复机制）。

**修复方向**：同不符合项 #3——初始扫描建行时携带 transcriptPath 并主动拉取一次用量。

### 问题 3：退出 claude 后 agent 行不删除（三场景均存在）

**现象**：切到 B 退 B 的 claude / 切回 A 退 A 的 claude / 关页签退出——行均不消失。

**根因**（分层）：

1. **claude 侧信号链正常**（信号链实验实锤）：真实 claude（含 `-p` 模式）完整写出 SessionStart/UserPromptSubmit/Stop/SessionEnd 四个信号文件（SessionEnd 在 Stop 后 168ms）——失效点在前端，不在 claude/hook 脚本/后端 watcher。
2. **基本删行链路有效**：E2E 已有用例 + R3 复测均证实 SessionEnd → 行消失（单项目、无后续切换场景）。
3. **场景 1/2（/exit）实锤机制——删后复活**：SessionEnd 删行成功，但用户随后的切项目往返触发初始扫描，从 TerminalRegistry 全量重建行——注册表无「claude 会话已结束」信息，终端面板（shell）还活着即重建为 🟡 行（`useAgentStatus.ts:224-252`）。R3 实测复现。用户操作时序（退出 → 切换查看）必然踩中。
4. **场景 3（关页签）实锤机制——remove 事件丢失**：行从未被删（R4 实测行时间戳 = 建行时刻），机制见不符合项 #5（同 commit destroy 顺序致 TerminalRegistry.remove 事件无人接收）。
5. **干扰项排除**：页签图标消失不能证明 SessionEnd 到达前端——清图标有 OSC 133 D（shell integration 本地产生）与 SessionEnd（hook 事件）两条路径，前者不依赖 hooks。

**性质**：不符合项 #1（行建模偏离）的直接后果 + effect 时序竞态（不符合项 #5）。

**修复方向**：随不符合项 #1 行建模重设计解决（会话状态跟踪使初始扫描能识别已结束会话）；竞态以 reconcile 对账消除（不符合项 #5）。

### 问题 4：toast 点击无反应 / banner 不弹

**现象**：Win10 点击 toast 完全无反应（不跳转）；Win11 屏幕不弹 banner，仅通知中心可见条目。

**根因**：见不符合项 #2——未打包 Win32 WebView2 应用无 AUMID，Web Notification API 为残缺 shim：banner 被系统抑制（Win11 现象）、激活回调不路由回应用（Win10 现象）、catch 回退永不触发（构造不抛异常）。计划「委托 OS 原生通知中心」假设不成立。

**性质**：平台级限制 + 计划假设失实，非实现 bug。

**修复方向**：不符合项 #2 候选 a–d。另注：F4 的任务栏闪烁（`requestUserAttention`）为独立路径，不受此影响。

### 问题 5：终端输入 claude 后启动慢 1-3s

**现象**：键入 `claude` 回车到 UI 可交互约 1-3 秒。

**根因**（实测量化）：hooks 非主因——

1. **hook 脚本开销实测 36-44ms/次**（5 次测量：44/37/36/37/36ms；裸 node 基线 35ms）。启动路径仅 SessionStart 一个 hook 触发（提交 prompt 才到 UserPromptSubmit）→ hooks 总贡献 ~0.1s 量级，**无法解释 1-3s**。
2. **无重复注入**：`~/.claude/settings.json` 每事件恰 1 条 slterm hook（Notification 事件另有用户自装 `claude-notify.exe` 并存，不在启动路径）。
3. **主体是 claude 自身启动成本**：信号链实验中进程 spawn → SessionStart 信号写盘间隔 1.4s（含 claude node bundle 加载 + 一次 hook）；1-3s 的感知主要是 claude 在 Windows 的 node 模块加载 + Ink 初始化，跨终端普遍存在，非 slTerminal hooks 引入。
4. **slTerminal 特有路径未见秒级阻塞点**：DA1 模拟即时响应（若失效 Ink 会卡 ~60s 而非 1-3s）；CPR 注入一次性；shell integration 影响 shell 启动而非 claude 启动。

**性质**：非 hooks 回归；感知慢主因是 claude 自身启动成本（hooks 贡献 ~0.1s）。

**修复方向**：接受现状；如需压榨 hooks 的 0.1s，可将 per-event node spawn 改为编译 exe 或持久守护进程（收益有限）。若要做根治性定位需 claude 启动 profile，超出本次 review 范围。
