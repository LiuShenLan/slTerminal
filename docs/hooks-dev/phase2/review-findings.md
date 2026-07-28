# Phase 2 开发验证 — 问题清单

> 范围：对照 `docs/hooks-dev/phase2/`（checklist 31 项 + stages 验证项 + workflows/verify 断言 + contract.md C12）的静态符合性 review + 门禁实跑。只记录问题，符合项从略。
> 日期：2026-07-28

**验证方式**：静态逐项核对（含 P1↔P2 接缝）+ 实跑门禁 + 实跑 L4 E2E。

**门禁结果**：
- `cargo clippy -D warnings` ✓、`cargo test --test-threads=1` 359/359 ✓
- `npx tsc --noEmit` ✓、`npx eslint src/` ✓
- `npm test` 1637/1637 ✓（首跑 1 条 flaky 失败，见 §二-7）
- `npm run e2e` 18 条 active：16 过 2 败（均为侧栏视图旧用例，见 §二-4；P2 新增 Agent Status 用例通过）

**未覆盖**：stages.md 汇总的 7 条人工验证点（真实 claude 场景：失焦 toast/任务栏闪烁/点击路由/多会话四态/项目切换/未注入降级/Stop-SessionEnd 生命周期），需人工复核，不计入本报告不符合项。

---

## 一、不符合开发计划的项

### 1. [P2-FE-06] toast 点击路由未调 `switchToPage`——跨页面面板聚焦失败 + `__dockviewApi` 悬挂（严重度：高）

- **计划要求**：onclick 中「查找所属 projectId，调用 `switchToPage(projectId, pageId)`；调用 `window.__dockviewApi?.getPanel(panelId)?.focus()`」（checklist P2-FE-06）。
- **实际**：`routeToPanel` 未调 `switchToPage`，自行复制其部分步骤（`setProjectRoot` + `setActivePage`）。`switchToPage` 有三个关键副作用：ensurePageInitialized（有 Workspace effect 兜底，侥幸无害）、`setActivePage`（已复制）、**`window.__dockviewApi` 重指向目标页面 API**（未复制，无兜底——重指向仅存在于 `switchToPage`/`onDeletePage`/`handlePageApiReady` 三处）。
- **后果**：跨页面场景下 `setActivePage` 后 `__dockviewApi` 仍指**旧页面**的 DockviewApi，`getPanel(panelId)` 查无此面板 → focus 静默失败。feature-plan F4 验收要点 1「点击 toast → 落到正确页签」在跨页面时不成立。且 `__dockviewApi` 持续悬挂，`global.closeTab` 等一切依赖它的全局命令将作用于错误页面，直到下一次真正的 `switchToPage`。
- **证据**：`src/features/notifications/useClaudeNotifications.ts:116-143`；`src/workspace/Workspace.tsx:95-120`（switchToPage 三副作用）、`:163-166`（handlePageApiReady 仅在 pageId===activePageId 时重指向）。L2 测试用单 MockDockviewApi，结构上无法暴露此问题。
- **修复方向**：`routeToPanel` 改为复用 Workspace 的 `switchToPage`（或将其提为可导入的共享函数）并 `await` 后再 focus；消除复制实现。

### 2. [P2-FE-10] 行生命周期与 TerminalRegistry 无联动——关页签行滞留、新终端行不出现（严重度：中）

- **计划要求**：stages.md Stage 3「`SessionEnd`/exit 事件移除行；**面板关闭/TerminalRegistry.remove 同样移除**」；feature-plan F5 边界 3/5。
- **实际**：`useAgentStatus` 仅在挂载/项目切换时一次性 `TerminalRegistry.getAll()` 扫描，无任何增删订阅。两个后果：
  - a) 用户直接关闭终端页签（pty_kill）时 `SessionEnd` 不一定发出 → 行永久滞留；
  - b) 视图已挂载后新开的终端（未注入 hooks、无事件）永不出现行，直到切换项目触发重扫。
- **证据**：`src/features/agentStatus/useAgentStatus.ts` 全文（仅 :197 一次 `getAll()`，无订阅）；`src/panels/terminal/TerminalRegistry.ts` 无订阅 API（仅 `register/remove/get/getAll`）。
- **修复方向**：TerminalRegistry 增加变更订阅（或轮询式轻量事件桥），useAgentStatus 订阅增删：remove → 移除行；register 且属于当前项目 → 插入 🟡 行。

### 3. [P2-FE-11] 行标题未查 titleManager——恒为「终端 {pageId}」（严重度：中）

- **计划要求**：stages.md Stage 3「标题：优先从 titleManager 查找（无则回退 panelId 中 pageId 或 '终端'）」；feature-plan F5 行内容表「页签标题｜TerminalRegistry / titleManager」。
- **实际**：标题恒为 `` `终端 ${pageId}` ``（`useAgentStatus.ts:127` 事件路径、`:208` 初始扫描路径）。注释自称「简单标题回退：若 titleManager 无注册则用 pageId」，但代码中无任何 titleManager/dockviewApi 查询。同项目 `useClaudeNotifications.findPanelTitle`（`useClaudeNotifications.ts:101-111`）已有经 `dockviewApi.getPanel(panelId).title` 取真实页签标题的先例，未复用。
- **证据**：`src/features/agentStatus/useAgentStatus.ts:126-127,208`。
- **修复方向**：照 `findPanelTitle` 先例取 `getPanel(panelId).title`，回退 `终端 {pageId}`；事件到达时顺带刷新标题。

### 4. [P2-FE-12] 行点击未 `await` async `switchToPage`——跨页面 focus 竞态（严重度：中）

- **计划要求**：点击行「调用 `switchToPage(projectId, pageId)`；调用 `window.__dockviewApi?.getPanel(panelId)?.focus()`」（checklist P2-FE-12）。
- **实际**：`AgentStatusView.handleFocus` 同步顺序执行：`props.switchToPage(...)`（async，内部先 `await setProjectRoot` IPC）→ 立即读 `window.__dockviewApi` 调 focus。此刻 `switchToPage` 尚未完成，`__dockviewApi` 仍指旧页面 → 跨页面时 `getPanel(panelId)` 查无面板，focus 静默失败；目标页未初始化时更无面板可查。
- **证据**：`src/features/agentStatus/AgentStatusView.tsx:62-89`；`src/workspace/Workspace.tsx:95-120`（switchToPage 为 async）。L2 测试 `switchToPage` 是同步 `vi.fn()`，掩盖竞态。
- **修复方向**：`await props.switchToPage(...)` 后再 focus；必要时等待目标页面板挂载（轮询 `getPanel` 有限次）。

### 5. [P2-FE-03 / P2-BE-05] `contextUsage` 的 IPC 合约验证缺失 + 双 wrapper 增实体（严重度：低）

- **计划要求**：P2-FE-03 验证「ipc-contract.test.ts 覆盖命令名、参数结构、返回值透传」；P2-BE-05 验证「ipc-contract.test.ts 扩展覆盖 `hooks.contextUsage` 命令名与参数结构」。
- **实际**：`ipc-contract.test.ts`（65 用例）与 `ipc-hooks-contract.test.ts`（16 用例）均无任何 `contextUsage`/`hooks_context_usage` 用例——两个文件的 grep 零命中。另外 `src/ipc/hooks.ts` 同时导出 `getContextUsage` 与 `contextUsage`（互为别名），计划只要求 `contextUsage`；`getContextUsage` 生产代码零调用方（仅 `src/__tests__/setup.ts:98` 的 mock 条目），属计划外增实体。
- **证据**：`src/ipc/hooks.ts:45-56`；grep `contextUsage`/`hooks_context_usage` 于两个合约测试文件均无命中。
- **修复方向**：删 `getContextUsage`（或收敛为一处）；在 `ipc-hooks-contract.test.ts` 追加 `contextUsage` 四维验证（命令名/参数/透传/异常）。

### 6. [P2-FE-02] `sendClickableNotification` 签名/返回值与契约不符，官方三函数未 re-export（严重度：低）

- **计划要求**：checklist P2-FE-02「`sendClickableNotification(title, options, onClick)`……返回创建的 `Notification` 实例，便于测试」「直接 re-export 官方插件：`isPermissionGranted` / `requestPermission` / `sendNotification`」；verify/stage-02 同。
- **实际**：签名为 `(title, body, onClick)`（`body: string` 非 options 对象），返回 `void` 非 Notification 实例；三官方函数仅 import 未 re-export（包装为 `ensureNotificationPermission`/`sendSilentNotification`）。另：`sendSilentNotification` 零调用方，属计划外增实体。
- **证据**：`src/ipc/notification.ts:10-23,36-65`；grep `sendSilentNotification` 调用方为零。
- **影响说明**：功能等价路径存在（`ensureNotificationPermission` 被 useClaudeNotifications 使用），属契约形状偏离而非功能缺失。
- **修复方向**：返回值改为 Notification 实例；三函数补 re-export 或修订 checklist/verify 表述使其与包装风格一致（二选一，需拍板）；删 `sendSilentNotification`。

### 7. [P2-TE-04] 用量条颜色 token 断言缺失（严重度：低）

- **计划要求**：P2-TE-04「颜色 token 来自 `AGENT_STATUS_USAGE_COLORS`」；P2-FE-14 验证「`src/__tests__/colors.test.ts` 更新断言」。
- **实际**：`agent-status-view.test.tsx` 无任何颜色断言；`colors.test.ts` 无 `AGENT_STATUS_USAGE_COLORS` 条目（grep 零命中）。正面：`src/features/agentStatus/` 无硬编码 hex，`colors.ts` token 组存在且三段齐全。
- **证据**：`src/__tests__/agent-status-view.test.tsx`（用量条 describe 仅断宽度与文案）；`src/__tests__/colors.test.ts` import 列表无该 token。
- **修复方向**：colors.test.ts 补 token 组合法性断言；view 测试补 usageBarColor 分段（<50/50-80/>80）颜色引用断言。

### 8. [P2-TE-06] L4 行渲染用例 `it.skip` 理由与内容不符——可自动化覆盖被放弃（严重度：低）

- **计划要求**：P2-TE-06「Agent Status 视图行渲染（真实 claude 会话触发事件后视图出现行）」，toast 链路允许降级人工。
- **实际**：用例 2（`test.e2e.ts:1612`）整体 `it.skip`，注释理由「E2E 环境无 claude 进程，hook 事件不可用」。但该用例本体只断言 TerminalRegistry 初始扫描产生的静态行（🟡 + 用量条容器），**全程不需要 claude 或 hook 事件**，skip 理由与用例实际内容不符。且动态四态部分本可照阶段 1「信号文件驱动页签图标流转」（`test.e2e.ts:1357`）先例，Node 端写信号文件合成事件驱动。结果：P2 行渲染 L4 自动化覆盖为零（仅视图打开用例 active）。
- **证据**：`e2e-tests/test.e2e.ts:1593-1689`（skip 注释与用例本体）；对比 `:1357-1420`（信号文件合成事件先例）。
- **说明**：verify/stage-04 允许「it.skip + 人工验证注释」，故定为低。
- **修复方向**：静态行部分拆为 active 用例直接启用；动态部分写信号文件合成事件后断言四态流转。

---

## 二、checklist 外发现

### 1. [useAgentStatus] `eventToStatus` 返回 null 时覆盖行状态——与 useXterm 语义不一致（严重度：中）

- **问题**：`useXterm` 对 null 状态跳过（`useXterm.ts:351-355`：`if (status === null) { ...; return; }`，图标保持）；`useAgentStatus` 直接写入行（`useAgentStatus.ts:123-124,143-149`）。`eventToStatus` 对 Notification 非 attention 子类（如 `auth_success`）与未识别事件返回 null（语义为「不改变状态」），F5 行却将其作为新状态 → 图标消失。
- **场景**：会话 ⚡ 工作中收到一条 `Notification(auth_success)` → 行图标从 ⚡ 变空白。
- **测试缺口**：`agent-status-hook.test.ts:46-56` 的 claudeStatus mock 永不返回 null（未知事件兜底 `"working"`），且 `SessionStart` 映射为 `working` 与真实实现（`attention`）不符——该 bug 结构上无法被现有测试捕获。
- **证据**：`src/features/agentStatus/useAgentStatus.ts:100-149`；`src/panels/terminal/useXterm.ts:349-357`；`src/lib/claudeStatus.ts:60-61,71-73`。
- **修复方向**：null 时不更新行状态（仅刷新 lastEventAt 与否需拍板）；测试改用真实 `eventToStatus` 或对齐映射表。

### 2. [useAgentStatus] 每次渲染重订阅 onHookEvent（严重度：低）

- **问题**：`handleHookEvent` 的 `useCallback` 依赖含 `projectPageIds`——每渲染新建的 `Set`，标识每渲染变化 → callback 每渲染重建 → `useEffect([handleHookEvent])` 每次行更新都退订+重订阅。Tauri `listen()` 注册是异步的，重订阅窗口期内到达的事件会丢失（概率低但随事件频率放大）。
- **证据**：`src/features/agentStatus/useAgentStatus.ts:83-85,91-187`。
- **修复方向**：`projectPageIds` 用 `useMemo`（依赖 `activeProject`）稳定标识，或事件回调内以 ref 读最新项目集合。

### 3. [P2-DOC-04] test-inventory.md 阶段 2 新增行覆盖描述多处失实（严重度：中）

- **背景**：计数数字已随 2026-07-28 对账修正且口径自洽（grep 口径实测 L2=1552、L1=359、L3=116、L4=18 均复核吻合）。问题集中在**覆盖描述列与 changelog 括注**，多处虚构：

| 位置 | 描述声称 | 实际 |
|------|---------|------|
| `:24` mod.rs | 含「hooks_context_usage 命令」 | 该命令测试全在 usage.rs，mod.rs 仅 serde + parse 冒烟 |
| `:28` usage.rs | 函数 `parse_context_usage`/`read_context_usage_file`；覆盖「total_tokens 计算/exit_code 解析/transcript_path 为空」 | 函数名为 `parse_usage_line`/`scan_transcript_usage`；三项覆盖内容均不存在（疑从他模块复制） |
| `:44` ipc-contract | 「hooks_context_usage 四维验证」 | 零命中（见 §一-5） |
| `:45` ipc-hooks-contract | 「hooks_context_usage 四维验证（命令名/参数结构/返回值/异常传播）」 | 零命中，16 用例全为 inject/uninstall/status/onHookEvent |
| `:185` notifications | 「transcript_path 提取/应用失焦预检/应用重新聚焦后 flush 积压/并发竞态」 | 均不存在的用例 |
| `:186` agent-status-hook | 「context_usage 轮询」 | 实现与计划均明令事件驱动禁轮询，描述与事实相反 |
| `:187` agent-status-view | 「四态 UI/tooltip/加载态/错误态」 | 组件为三态（no-root/empty/ready），无 tooltip/加载/错误态 |
| `:255` L4 Agent Status 行 | 「显示上下文用量 + hooks_context_usage IPC 返回 ContextUsage 数据」 | 用例仅断言视图打开与标题，无 IPC 调用 |
| `:278` changelog | 「+12：notification + hooks_context_usage 合约」「+8：hooks_context_usage」 | ipc-contract 无 notification/hooks describe（+12 实为 window/git 等）；+8 实为 onHookEvent 等 |

- **证据**：上表行号对应 `.claude/test-inventory.md`；各被引文件 grep 实查。
- **修复方向**：按各文件实际内容重写覆盖描述；changelog 括注改为真实增量构成。

### 4. [E2E] 侧栏视图两条旧用例确定性失败——与真实用户 settings.json 共享状态的隔离缺陷（严重度：中）

- **现象**：`npm run e2e` 两连跑均失败同两条：`侧栏视图.点击活动栏按钮开关/替换侧栏视图`（`test.e2e.ts:982`，「explorer 未替换 projects」）与 `侧栏视图.拖拽跨区`（`:1042`，`expect(zones.top).toContain('explorer')` 失败）。P2 新增 Agent Status 用例及其余 15 条通过。
- **根因**：E2E 二进制加载**真实用户** `~/.slterminal/settings.json` 的 sideBar 段。两条用例断言 explorer 初始在**上区**（DEFAULT_ZONES 布局），而持久化状态中 explorer 在下区（历史运行/用户手动拖拽残留）→ 断言确定性失败。拖拽用例第 3 步（`:1027-1035`）只重置 `open` 不重置 `zones`，无法自愈。
- **性质**：非 P2 引入（P2 未动这两用例），属 E2E 隔离缺陷——E2E 与真实用户共享 settings 文件，任何历史状态都可使断言崩盘；且 E2E 运行本身会回写该文件，反向污染用户配置。
- **证据**：wdio 日志 state dump（`zones.bottom: ['explorer']`）；`test.e2e.ts:975-985,1027-1045`；`src/stores/sideBar.ts` 持久化路径 `~/.slterminal/settings.json`。
- **修复方向**：E2E 启动时备份/隔离 settings.json（或注入测试专用 settings 路径）；拖拽/开关用例前置重置 zones 为 DEFAULT_ZONES。

### 5. [L2] diff-panel 一条用例全量运行偶发失败（flaky）（严重度：低）

- **现象**：首轮全量 `npm test` 1 失败：`diff-panel.test.tsx > left panel does NOT use editable.of(false)`（`:359` `leftContent` 为 null）；隔离单跑 30/30 通过；全量重跑 1637/1637 通过。
- **根因**：用例仅 `waitFor` `[data-e2e="diff-panel"]` 容器出现（`:352-354`），未等左栏 `.cm-content` 挂载即查询；全量运行 CPU 竞争下窗口期放大。
- **证据**：`src/__tests__/diff-panel.test.tsx:346-362`。
- **修复方向**：`waitFor` 增加 `[data-e2e="diff-left"] .cm-content` 存在断言后再取元素。

### 6. [风格] `parsePageId` 三处重复实现且行为分歧（严重度：低）

- **问题**：同一 panelId→pageId 解析逻辑存在于 `useClaudeNotifications.ts:77-81`（无条件剥最后段）、`useAgentStatus.ts:48-62`（仅数字后缀剥离）、`AgentStatusView.tsx:65-72`（内联，同后者）。两种行为对非数字尾段的 panelId 解析结果不同。
- **修复方向**：收敛为 `src/lib` 单点（建议保留数字后缀剥离语义），三处引用。

### 7. [风格] useClaudeNotifications 去重注释与实现不符（严重度：低）

- **问题**：注释称「60s 内同一 session + event + timestamp 不重复通知」（`useClaudeNotifications.ts:187`），实现无任何时间机制——仅 key 集合 + 超 200 条截断（`:188-196`）。且 timestamp 本即每信号唯一，去重实际只在同一信号文件被重复投递时生效，注释夸大其词。
- **修复方向**：修正注释为真实机制（同信号去重 + 200 条上限）。

---

## 附：人工验证点（本次未覆盖，需人工复核）

照 stages.md「人工验证点汇总」7 条原样保留：失焦权限请求 toast+闪烁+点击路由、失焦 Stop toast、聚焦零通知、多会话四态与跨页跳转、项目切换、未注入会话降级（🟡+用量条不可用）、用量事件驱动更新。
