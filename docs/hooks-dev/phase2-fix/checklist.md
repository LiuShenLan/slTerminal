# Phase 2 Fix 修复清单（PF2-）

> 输入：`docs/hooks-dev/phase2/review-findings.md`（5 不符合项 + 5 人工验证问题根因）。
> 组织方式：按模块分组（FE 行建模 / FE toast / BE 契约 / TE / DOC）。**不用 P0-P4 优先级——优先级由 `stages.md` 的 Stage 依赖顺序表达**（Stage 01 行建模 → 02 toast → 03 cache → 04 L4 → 05 DOC）。
> 合并关系：无去重合并——输入为单份 review 报告，22 项为修复动作维度直接展开。
> 事实核验：本清单引用的既有文件路径/行号均经 Read/Glob 实证（2026-07-28）；库行为前提附一手证据行号。

## 决策基线（用户已拍板）

1. **toast = 改设计·最小**：toast 仅提示不可点击，发送通道换 Tauri 原生 `sendNotification`（含 Win11 banner 人工实测点）；点击路由诉求由任务栏闪烁承担；不做应用内通知列表。
2. **范围 = 全修 5 不符合项**；问题 5（claude 启动慢 1-3s）接受现状，仅文档记录实测结论。
3. **E2E 防复发 = 纳入**：R2/R3/R4 变体固化为常驻 L4 用例（按行建模新语义重写）。

## 跨边界契约（写死，agent 不各自推断）

1. **claudeSession**：`ClaudeSessionInfo { transcriptPath?: string; matchedCommand?: string; lastEventAt: number }`——**存在即运行中**（二态模型，无 running 布尔）。`RegisteredTerminal.claudeSession?: ClaudeSessionInfo | null`（可选字段，既有 stub 工厂编译不炸）。`setClaudeSession(panelId, patch: Partial<ClaudeSessionInfo> | null)`：merge 语义（`undefined` 键不覆盖旧值）、`null` 清空、panelId 不存在 no-op 不 notify、缺 `lastEventAt` 自动填 `Date.now()`。`RegistryEvent.type` 增 `"sessionChange"`（payload 仅 `{ type, panelId }` 裸结构，**不带 session 数据**——listener 经 `get()` 读现值，防快照不一致）。`register` 幂等覆盖时 `claudeSession` 缺省**保留旧值**（StrictMode/重试场景不丢 session）。
2. **match 语义**：`TabTitleRegistry.match(command)` 取命令行**首 token**（`command.trim().split(/\s+/)[0]`）后精确匹配（覆盖 `claude --resume` / `claude -p` / `claude --model xx`）。
3. **ContextUsage 四字段**：`{ inputTokens, outputTokens, cacheReadInputTokens, cacheCreationInputTokens }`。用量口径 = `(inputTokens + cacheReadInputTokens + cacheCreationInputTokens) / 200_000`；`outputTokens` 不计占用，保留为信息字段。transcript 缺 cache 字段默认 0（serde `default`，兼容旧 transcript）；`input_tokens` 缺失仍整行 None（沿用现状）。真实铁证：transcript 尾行 `input_tokens:2745, cache_read_input_tokens:196096, output_tokens:81`——当前算法 1.4% vs 实际 99.4%。
4. **sendToastNotification**：`sendToastNotification(title: string, options: { body: string }): void`——Tauri 原生 `sendNotification` 通道，**无 onClick 参数**（点击路由放弃）。`ensureNotificationPermission` 保留不变。
5. **行生命周期**（双通道建行幂等 + 三通道删行 + reconcile 兜底）：
   - **建行** = `sessionChange`（session 非 null）∨ hook 事件（非 SessionEnd/Exit 且行不存在）——两通道独立幂等（订阅顺序不定，视图可能先于终端挂载）。
   - **删行** = `sessionChange`（session 为 null）∨ SessionEnd/Exit hook 事件 ∨ `remove` 事件。
   - **初始扫描**只建 `claudeSession` 非 null 的行；携 `transcriptPath` 的行主动 `contextUsage` 拉一次（问题 2b 修复）。
   - **reconcile 对账**：初始扫描与事件处理时以注册表现值对账（行在 registry 中不存在或 session 为 null → 移除），兜底任何事件丢失。

## 一、FE 组·行建模核心（不符合项 #1/#3/#5 + 问题 1/2/3）

### PF2-FE-01 TabTitleRegistry.match 改首 token 匹配

- **位置**：`src/panels/terminal/TabTitleRegistry.ts:37-39`（`match(command)` = `this.rules.get(command) ?? null` 整行精确匹配）。
- **修复要点**：按契约 2——`match` 内先取 `command.trim().split(/\s+/)[0]` 再查表。根因：`useCommandDetection.ts:45-46` 把 OSC 133 C 携带的**完整命令行**喂入，`claude --resume xxxx` ≠ `"claude"` → match null → 不改名、不置 isCommandRunning、不发 🟡（问题 1）。
- **波及面**：
  - 生产调用点唯一：`src/panels/terminal/useCommandDetection.ts:46`（无需改——match 内部消化首 token）。
  - **测试断言反转**：`src/__tests__/tab-rules.test.ts:43-46`「"claude update" 不匹配」断言 `match("claude update")).toBeNull()`——首 token 后 `"claude update"` 命中 → 语义反转必改（归 PF2-TE-03）。
  - `src/__tests__/tab-title-registry.test.ts`（kebab-case 实证；panels/CLAUDE.md 误写驼峰 `TabTitleRegistry.test.ts`，文档修订归 PF2-DOC-02）：补首 token 用例。
  - `src/panels/terminal/tabRules.ts` 注释（注册语义说明）同步。

### PF2-FE-02 TerminalRegistry 增 claudeSession 契约

- **位置**：`src/panels/terminal/TerminalRegistry.ts`（`RegisteredTerminal` :11-16；`RegistryEvent` :19；`register` 幂等覆盖 :31-35）。
- **修复要点**：按契约 1——`RegisteredTerminal` 增可选 `claudeSession`；新增 `setClaudeSession(panelId, patch | null)`（merge/no-op/自动 lastEventAt）；`RegistryEvent.type` 增 `"sessionChange"`（裸 panelId）；`register` 幂等覆盖保留旧 session。
- **波及面**：
  - `TerminalRegistry.getAll` 生产调用点：`src/App.tsx:105` + `src/features/agentStatus/useAgentStatus.ts:231`——返回副本含新可选字段，不读新字段的消费方（App.tsx）无影响。
  - 测试 stub 工厂：`src/__tests__/terminal-registry.test.ts:13-27`（四字段对象字面量——可选字段设计下编译不炸）+ `src/__tests__/terminal-registry-subscribe.test.ts:9-12`。
  - 新测试归 PF2-TE-05。

### PF2-FE-03 useCommandDetection 写入会话状态

- **位置**：`src/panels/terminal/useCommandDetection.ts`（签名 `(terminal, onTabStateChange?, sharedCmdRunningRef?)`；OSC 133 C :43-50；OSC 133 D :51-55）。
- **修复要点**：
  - 签名新增 `panelId: string` 参数（写 session 用）。
  - OSC 133 C 且 rule 命中 → `TerminalRegistry.setClaudeSession(panelId, { matchedCommand: rule.command })`（此时无 transcriptPath——对应 feature-plan 边界 5「未注入 hooks 的会话行存在、四态 🟡、用量条不可用态」）。
  - OSC 133 D 且 `isCommandRunningRef.current === true`（注册命令退出）→ `setClaudeSession(panelId, null)`（feature-plan 边界 3：OSC 133 D 删行）。
- **波及面**：调用点 `src/panels/terminal/useXterm.ts:205`（传 panelId——作用域内现成值）；OSC 133 测试用例适配归 PF2-TE-08。

### PF2-FE-04 useXterm hook 事件订阅写入会话状态

- **位置**：`src/panels/terminal/useXterm.ts:349-357`（`hooks.onHookEvent` 订阅，panelId 过滤 → eventToStatus → onTabStateChange）。
- **修复要点**：订阅回调内追加 session 写入（与页签 emoji 正交）：
  - 非 SessionEnd/Exit 事件 → `setClaudeSession(panelId, { transcriptPath: payload.transcriptPath ?? undefined })`（merge 语义——`undefined` 不覆盖旧值；payload.transcriptPath 为 null 时保留旧 transcriptPath）。
  - SessionEnd/Exit → `setClaudeSession(panelId, null)`。
  - 双通道幂等说明：useXterm 与 useAgentStatus 的 hook-event listener 订阅顺序不定（视图可能先于终端挂载），故 useAgentStatus 侧保留建行能力（PF2-FE-05）；sessionChange 同步 notify 保证本路径建行先于状态更新。
- **波及面**：use-xterm 测试新增 session 写入断言归 PF2-TE-08。

### PF2-FE-05 useAgentStatus 行建模重设计

- **位置**：`src/features/agentStatus/useAgentStatus.ts`（handleHookEvent :89-175；SessionEnd/Exit 删行 :111-120；新行 :141-150；usage 拉取 :158-172；registry subscribe effect :186-221 + deps :221；初始扫描 :224-252，**usage:undefined 无 transcriptPath :239-247**——问题 2b 根因）。
- **修复要点**：按契约 5 重设计——
  - 行 = **运行中的 claude 会话**（非全部终端）。`AgentSessionRow` 数据源从「TerminalRegistry 全量」改为「claudeSession 非 null 的注册项 + hook 事件」。
  - 建行双通道：`sessionChange`（经 `get()` 读 session 非 null）∨ hook 事件（非 SessionEnd/Exit 且行不存在——保留现有 :141-150 建行能力）。
  - 删行三通道：`sessionChange`（session null）∨ SessionEnd/Exit ∨ `remove`。
  - 初始扫描：遍历 `getAll()` 只建 `claudeSession` 非 null 的行；行携 `transcriptPath` 时**主动 `contextUsage` 拉取一次**（修复问题 2b：切项目后 idle 会话用量永远「--」）。
  - **#5 竞态根治（双保险）**：① registry/hook-event 双 listener 经 **ref 读最新状态**（照 rowsRef :69-70 模式），effect deps `[]` 订阅永不重建——remove 事件永不丢失（R4 根因：同 commit passive destroy 顺序 SideBarArea(pane2) 先于主区(pane3)，旧 deps 重订阅窗口内 remove 丢失）；② 初始扫描/事件处理 reconcile 对账兜底。
  - `AgentSessionRow.usage` 内联类型 `{inputTokens, outputTokens} | null`（:31）改为引用 `ContextUsage`（import from `src/types/hooks`）——与 PF2-FE-11 四字段口径对齐。
- **波及面**：`AgentStatusView.tsx`（rows 消费——状态机 no-root/empty/ready 不变）；测试全量重写归 PF2-TE-01。

### PF2-FE-06 AgentStatusView 空态文案核对

- **位置**：`src/features/agentStatus/AgentStatusView.tsx:94`。
- **修复要点**：feature-plan 边界 2 要求空态文案「当前项目无运行中的 claude 会话」——**实读 :94 现状已是该文案**（L4 用例 1 `test.e2e.ts:1603` 亦已按此断言）。**预期零改动，仅需核对确认**；若 drift 则对齐。
- **波及面**：无。

### PF2-FE-07 useAgentStatus contextUsage 静默 catch 补可观测性

- **位置**：`src/features/agentStatus/useAgentStatus.ts:169-171`（usage 拉取 `.catch` 静默吞错）。
- **修复要点**：catch 内补 `console.error`（DBG-7 教训：静默 catch 既是故障放大器也是定位障碍）。降级语义不变（usage 保持旧值/`--`）。
- **波及面**：无。

## 二、FE 组·toast 改设计·最小（不符合项 #2 + 问题 4）

### PF2-FE-08 sendClickableNotification → sendToastNotification

- **位置**：`src/ipc/notification.ts:40-62`（`new Notification` 主路径 + onclick 绑定 + catch 回退 Tauri sendNotification）；`:33` 注释假设「委托 OS 原生通知中心」已证伪。
- **修复要点**：按契约 4——替换为 `sendToastNotification(title, { body })`：主路径 Tauri 原生 `sendNotification`；**删 Web Notification 路径与 onclick 参数**（未打包 Win32 WebView2 无 AUMID：banner 抑制 + onclick 不路由 + shim 无 close + 构造不抛→catch 回退永不触发，探针实测 `{"created":true,"permission":"granted","thrown":"TypeError: n.close is not a function"}`）。`ensureNotificationPermission`（:20-26）保留。失败 catch 补 `console.error` 不静默。
- **波及面**：生产调用点唯一 `src/features/notifications/useClaudeNotifications.ts:193`（PF2-FE-09 同步改）；测试 mock 重写归 PF2-TE-04。
- **人工验证点**：Tauri 原生 `sendNotification` 在未打包 debug 二进制的 Win11 banner 可见性无法自动化（同受 AUMID 限制的可能性存在）——Stage 02 完成后人工实测；不弹则 toast 退化为通知中心条目 + 任务栏闪烁主职，接受（决策基线 1）。

### PF2-FE-09 useClaudeNotifications 去路由化

- **位置**：`src/features/notifications/useClaudeNotifications.ts`（classifyEvent :51-69；失焦门控 :148；去重 seenRef :155-163；findPanelTitle :89-101；routeToPanel :106-110；toast onclick → setFocus+routeToPanel :193-198；仅 permission 闪烁 :188-190）。
- **修复要点**：
  - 删 `routeToPanel`/`findPanelTitle`/onClick 绑定（决策基线 1：点击路由放弃）。toast body 不再含面板标题查找（项目名 + 事件类文案即可）。
  - **三类事件（permission/done/error）均触发任务栏闪烁**（现状仅 permission 闪）——决策写明：toast 失去点击能力后，任务栏闪烁是唯一的回窗引导通道，必须全覆盖。
  - 失焦门控（:148）、60s 去重（:155-163）、classifyEvent 三分类保留不变。
- **波及面**：`setFocus`/`routeToPanel`/`switchToPageAndFocus` import 清理；测试重写归 PF2-TE-04。

### PF2-FE-10 flashTaskbar 静默 catch 补可观测性

- **位置**：`src/features/notifications/useClaudeNotifications.ts:119-121`（`requestUserAttention` 的 `.catch` 静默吞错）。
- **修复要点**：catch 内补 `console.error`（同 PF2-FE-07 的 DBG-7 教训）。
- **波及面**：无。

## 三、BE/契约组·cache tokens（不符合项 #4）

### PF2-BE-01 usage.rs ContextUsage 增 cache 两字段

- **位置**：`src-tauri/src/hooks/usage.rs`（`ContextUsage {input_tokens, output_tokens}` :15-20 serde camelCase；`parse_usage_line` :79-88 两字段均 `?` 缺失整行 None）。
- **修复要点**：按契约 3——`ContextUsage` 增 `cache_read_input_tokens: u64` / `cache_creation_input_tokens: u64`（serde camelCase + `#[serde(default)]` 兼容缺失）；`parse_usage_line` 提取两字段（缺失 `unwrap_or(0)`），`input_tokens` 缺失仍整行 None（现状沿用）。
- **波及面**：
  - **serde 两测试构造字面量编译错必同步**：`usage.rs:255-273` `context_usage_serialize_camelcase` / `context_usage_deserialize_camelcase`（结构体字面量缺新字段 → 编译错）。
  - `parse_extra_fields_ignored`（:143-151）用 `cache_read`/`cache_write` 字段名（非 `cache_read_input_tokens`）→ 不受影响（实证）。
  - 前端 DTO 同步归 PF2-FE-11（硬约束 #4 DTO 双边对应）。
  - 新 L1 用例归 PF2-TE-06。

### PF2-FE-11 前端 ContextUsage 同步 + 用量口径

- **位置**：`src/types/hooks.ts`（`ContextUsage { inputTokens; outputTokens }` 2 字段）；`src/features/agentStatus/AgentStatusRow.tsx:31-35`（`total = inputTokens + outputTokens; percent = min(100, total/200_000*100)`）。
- **修复要点**：按契约 3——`types/hooks.ts` 增 `cacheReadInputTokens: number` / `cacheCreationInputTokens: number`（必填）；AgentStatusRow total 改 `inputTokens + cacheReadInputTokens + cacheCreationInputTokens`（output 不计占用）。
- **波及面**：
  - `AgentSessionRow.usage` 类型改引用 `ContextUsage`（随 PF2-FE-05）。
  - **测试字面量全量补字段**（四字段必填 → 旧两字面量编译错）：`src/__tests__/ipc-hooks-contract.test.ts:310`（`{ inputTokens: 1500, outputTokens: 800 }`）、`src/__tests__/agent-status-view.test.tsx:266,341,351,362`（makeRow usage 字面量——75%/low/medium/high 四断言按新口径重算）、`src/__tests__/agent-status-hook.test.ts` T7 mock 返回字面量。grep 口径：`inputTokens:` 全仓测试文件。
  - `src/ipc/hooks.ts:45-49`（`contextUsage(): Promise<ContextUsage | null>`）——类型引用自动跟随，签名无改。

### PF2-DOC-01 contract.md C12 回填四字段 + 口径

- **位置**：`docs/hooks-dev/contract.md:124`（C12 定义 `ContextUsage { inputTokens, outputTokens }`）。
- **修复要点**：回填四字段定义 + 用量口径（`(input + cacheRead + cacheCreation) / 200_000`，output 不计占用保留为信息字段）+ 缺 cache 默认 0 兼容约定。
- **波及面**：无。

## 四、TE 组

### PF2-TE-01 agent-status-hook.test.ts 重写（行建模新语义）

- **位置**：`src/__tests__/agent-status-hook.test.ts`（929 行 31 用例；mock TerminalRegistry 完整 API :37-72；registerTerminal 辅助 :147-154 直写 terminalMap 不触发通知）。
- **修复要点**：按契约 5 全量重写——
  - mock TerminalRegistry 工厂扩展：`setClaudeSession` + `sessionChange` 通知 + entry 含 `claudeSession` 字段。
  - **语义反转用例**：「T1 初始扫描生成行」→ claudeSession 为 null 的终端**不建行**；「FE-03 register 插入 🟡 行」→ register 不建行（session null），`sessionChange`（非 null）建行。
  - 新覆盖：纯 shell 无行 / OSC 133 C 通道建行（sessionChange 携 matchedCommand）/ hook 事件建行（行不存在时）/ SessionEnd 删行 / sessionChange(null) 删行 / remove 删行（deps [] 稳定订阅——remove 事件不丢失）/ 切项目初始扫描只建活会话 / 初始扫描携 transcriptPath 主动拉 usage / reconcile 对账（行在 registry 不存在 → 移除）。
  - T7 usage 字面量补 cache 字段（PF2-FE-11 波及）。
- **波及面**：无（测试文件自闭环）。

### PF2-TE-02 agent-status-view.test.tsx 更新（cache 口径 + 字面量）

- **位置**：`src/__tests__/agent-status-view.test.tsx`（410 行 11 用例；用量条断言 :263-370；makeRow :142-153）。
- **修复要点**：
  - usage 字面量补 cache 字段；75%/low/medium/high 四断言**按新口径重算**（total = input + cacheRead + cacheCreation）。
  - 「TerminalRegistry 含两个 panelId 时渲染两行」用例（:204-221）：makeTerminalMap 的 entry 补 `claudeSession` 非 null（行建模改后纯 shell 无行）。
  - 空态文案断言（:199-201「当前项目无运行中的 claude 会话」）已符合——保留作回归。
- **波及面**：无。

### PF2-TE-03 tab-rules + tab-title-registry 首 token 语义

- **位置**：`src/__tests__/tab-rules.test.ts:43-46`；`src/__tests__/tab-title-registry.test.ts`（71 行，kebab-case 实证）。
- **修复要点**：
  - `tab-rules.test.ts:43-46` 断言**反转**：`match("claude update")` 命中（首 token `"claude"`）——用例改名/改写为新语义（如「带参命令行按首 token 命中」）。
  - `tab-title-registry.test.ts` 补首 token 用例：带参命中、空命令行、仅空白、首 token 无规则仍 null。
- **波及面**：无。

### PF2-TE-04 notifications.test.ts 重写（toast 去路由化）

- **位置**：`src/__tests__/notifications.test.ts`（808 行 31 用例：F4 通知门控 19 + toast onClick 路由 6 + 任务栏闪烁细分 6）。
- **修复要点**：
  - mock `../ipc/notification` 的 `sendClickableNotification` → `sendToastNotification`（两参数无 onClick）。
  - **删「toast onClick 路由」describe 整块**（6 用例——路由功能移除）。
  - 「任务栏闪烁细分」反转：Stop/StopFailure/PostToolUseFailure 由「不触发 requestUserAttention」改为**触发**（三类均闪烁，PF2-FE-09）。
  - 保留：失焦门控、60s 去重、classifyEvent 过滤、正文文案（去面板标题后的新文案断言）。
- **波及面**：无。

### PF2-TE-05 terminal-registry 两测试扩展（claudeSession 契约）

- **位置**：`src/__tests__/terminal-registry.test.ts`（83 行）；`src/__tests__/terminal-registry-subscribe.test.ts`（55 行 3 用例）。
- **修复要点**：
  - claudeSession 可选字段：stub 工厂（:13-27）编译不炸验证（可选字段设计目标）。
  - `setClaudeSession` 全分支：merge（部分键更新保留其余）/ null 清空 / panelId 不存在 no-op 不 notify / 缺 lastEventAt 自动填充 / `undefined` 键不覆盖旧值。
  - `sessionChange` 事件：setClaudeSession（非 null/null 均触发）→ listener 收到 `{ type: "sessionChange", panelId }` 裸结构（不带 session 数据）。
  - `register` 幂等覆盖保留旧 session。
- **波及面**：无。

### PF2-TE-06 usage.rs L1 cache 分支

- **位置**：`src-tauri/src/hooks/usage.rs` `#[cfg(test)]`（测试 23 条：parse 9 + scan 6 + tail 1 + serde 2 + L1 5）。
- **修复要点**：
  - 新增 parse 分支：含 cache 两字段提取 / 缺 cache 字段默认 0（兼容旧 transcript）/ 仅 input+output 旧格式 / cache 为 0 显式值。
  - serde 两测试（:255-273）字面量补新字段 + 四字段 camelCase 断言（`cacheReadInputTokens`/`cacheCreationInputTokens`）。
  - **mock 边界盲区认知**：mockIPC 只守 JS 侧形状——cache 字段真实解析必须 L1 覆盖（后端真实反序列化），不得仅以 L2 mock 通过为据。
- **波及面**：无。

### PF2-TE-07 ipc-hooks-contract.test.ts ContextUsage 键集合守卫

- **位置**：`src/__tests__/ipc-hooks-contract.test.ts`（345 行 21 用例；contextUsage 合约 :275-344；`:310` 字面量）。
- **修复要点**：
  - `:310` mockUsage 字面量补 cache 两字段。
  - 新增 **ContextUsage 键集合精确匹配守卫**（DBG-4 模式，照 :240-268 HookEventPayload 8 字段先例）：`Object.keys(usage).sort()` 精确等于四字段——存在性断言防不住字段增删漂移。
- **波及面**：无。

### PF2-TE-08 use-xterm-lifecycle.test.ts OSC 133 适配

- **位置**：`src/__tests__/use-xterm-lifecycle.test.ts`（OSC 133 describe :1627 起：OSC133-1 :1663 OSC 133 C 匹配注册命令 → onTabStateChange 含 title + 🟡；OSC133-2 :1685 OSC 133 D → active:false；:109 registerOscHandler 捕获；:142 mock hooks.onHookEvent）。
- **修复要点**：
  - useCommandDetection 签名加 panelId（PF2-FE-03）→ useXterm.ts:205 调用点传参适配（若测试 mock 了 useCommandDetection 则断言调用参数含 panelId；若真实调用则走真实 match）。
  - 新增断言：OSC 133 C 命中 → `TerminalRegistry.setClaudeSession` 被调（`{ matchedCommand: "claude" }`）；OSC 133 D（命令运行中）→ `setClaudeSession(panelId, null)`。
  - hook 事件订阅（PF2-FE-04）→ 新增断言：非 SessionEnd 事件 → setClaudeSession 携 transcriptPath；SessionEnd → null。
  - match 首 token 语义波及：OSC 133 C 携 `claude --resume xxx` → 命中（若测试走真实 match）。
- **波及面**：TerminalRegistry 在该测试文件的 mock 形态需同步（增 setClaudeSession stub）。

### PF2-TE-09 test.e2e.ts L4 防复发用例（R2/R3/R4 变体）

- **位置**：`e2e-tests/test.e2e.ts`（Agent Status describe :1532 起；用例 2a 静态行渲染 :1615-1696；用例 2b 动态四态 :1713-1874）。
- **修复要点**：
  - **用例 2a 语义反转改写**：行建模改后纯 shell 终端无行——「创建终端 → 初始扫描生成 🟡 行」反转为「创建终端 → agent-status-row **不出现**（纯 shell 无行）」。
  - **用例 2b 流程适配**：删第 4 步「等待静态行出现」（:1777-1782）——首个 PreToolUse 信号文件到达即 hook 事件建行（断言 `agent-status-row` 出现且含 ⚡）；Stop→✅、SessionEnd→行消失断言保留。
  - **新增常驻 3 条（R2/R3/R4 变体防复发）**：
    1. **R2 变体（切项目用量保持）**：Node 端写假 transcript JSONL（含 `message.usage` 行，四字段）→ 信号文件携真实 transcriptPath 建行 + usage 拉取（行含量化百分比）→ 切项目往返 → 用量数值保持（初始扫描携 transcriptPath 主动拉取）。
    2. **R3 变体（SessionEnd 删行 + 切项目不复活）**：hook 事件建行 → SessionEnd 信号 → 行消失 → 切项目往返 → 行仍不存在（claudeSession 已 null，初始扫描不建行）。
    3. **R4 变体（会话终端关页签删行）**：hook 事件建行 → `__dockviewApi.removePanel(panel)` → 行消失（remove 事件 + ref 稳定订阅——R4 原始竞态不重现）。
  - E2E 辅助代码自查：helpers.ts 无行建模逻辑复制（只装钩子），无 DBG-8 类同型 bug。
- **波及面**：`e2e-tests/CLAUDE.md` 用例表同步（归 PF2-DOC-02）；`.claude/test-inventory.md` L4 段对账（归 PF2-DOC-03）。

## 五、DOC 组

### PF2-DOC-02 四 CLAUDE.md 同步

- **位置**：`src/features/sideViews/CLAUDE.md`（agentStatus 行描述段）、`src/panels/CLAUDE.md`（TerminalRegistry/TabTitleRegistry 行 + 测试模式驼峰误写）、`src/ipc/CLAUDE.md`（notification 行）、`e2e-tests/CLAUDE.md`（L4 用例表）。
- **修复要点**：
  - sideViews/CLAUDE.md：useAgentStatus 行语义改写——行 = 运行中 claude 会话（claudeSession 双通道建行/三通道删行/初始扫描只建活会话+携 transcriptPath 拉 usage/reconcile 对账）；AgentStatusRow 用量口径四字段。
  - panels/CLAUDE.md：TerminalRegistry 增 claudeSession/setClaudeSession/sessionChange 描述；TabTitleRegistry match 首 token 语义；**顺带修正测试模式表格 `TabTitleRegistry.test.ts` 驼峰误写为 kebab-case `tab-title-registry.test.ts`**（实证）。
  - ipc/CLAUDE.md：notification 行改写——`sendClickableNotification` → `sendToastNotification(title, {body})`（Tauri 原生通道，无 onClick；未打包 Win32 WebView2 无 AUMID 的平台限制结论一并记录）。
  - e2e-tests/CLAUDE.md：L4 用例表 agent-status 段改写（静态行反转 + 动态四态流程 + R2/R3/R4 变体 3 条新增）。
- **波及面**：无。

### PF2-DOC-03 test-inventory.md 对账

- **位置**：`.claude/test-inventory.md`（:183-189 通知/Agent 状态 3 文件 74 用例；:45 ipc-hooks-contract 21；:57-64 tab-title-registry 8/terminal-registry 7/tab-rules 6/terminal-registry-subscribe 3；:257-259 L4 agent-status 3 条）。
- **修复要点**：Stage 01-04 完成后以 `npm test` / `npm run wdio` 实际输出为准对账——重写文件的用例数（TE-01/02/03/04/05/08）+ 新增（TE-06/07/09）+ L4 用例描述语义（静态行反转、新增 3 条防复发）。
- **波及面**：无。

### PF2-DOC-04 src-tauri/src/hooks/CLAUDE.md 记录问题 5 结论

- **位置**：`src-tauri/src/hooks/CLAUDE.md`（Glob 实证存在）。
- **修复要点**：记录 hook 脚本性能实测结论（review-findings 问题 5）——hook 脚本 36-44ms/次（5 次测量：44/37/36/37/36ms；裸 node 基线 35ms）；启动路径仅 SessionStart 一个 hook 触发 → hooks 总贡献 ~0.1s 量级，非 claude 启动慢 1-3s 主因（主因 = claude 自身 Windows node 模块加载 + Ink 初始化）；接受现状，不做 per-event node spawn 优化。
- **波及面**：无。

---

## 修复类清单自检（五类必备项对照）

| 必备项 | 落点 |
|--------|------|
| 波及面全量调用点 | 各条目「波及面」段——match 调用点（useCommandDetection.ts:46 唯一）/ sendClickableNotification 调用点（useClaudeNotifications.ts:193 唯一）/ getAll 调用点（App.tsx:105 + useAgentStatus.ts:231）/ ContextUsage 字面量构造点（usage.rs:255-273 + 3 个测试文件，grep `inputTokens:`） |
| 契约守卫测试 | PF2-TE-07（ContextUsage 键集合精确匹配，DBG-4 模式）；PF2-TE-05（sessionChange 事件裸结构断言） |
| mock 边界盲区认知 | PF2-TE-06 条目内写明——cache 字段真实解析必须 L1 覆盖，mockIPC 只守 JS 侧形状；PF2-TE-09 R2 变体用真实 transcript JSONL 走后端真实解析 |
| 静默 catch 可观测性 | PF2-FE-07（contextUsage 拉取）+ PF2-FE-10（flashTaskbar）+ PF2-FE-08（sendToastNotification 失败 catch） |
| 前置条件前置化 + 辅助代码清扫 | 页面切换 setProjectRoot 前置已有（DBG-5 已修，本次不涉及）；E2E helpers.ts 无行建模逻辑复制（PF2-TE-09 条目内自查确认）；E2E 用例本身按新语义重写（TE-09） |
