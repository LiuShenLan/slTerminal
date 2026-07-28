# Phase 2-fix 清单：review-findings 15 项修复

> 范围：`docs/hooks-dev/phase2/review-findings.md`（2026-07-28）全部 15 项（§一 8 项 P2 不符合 + §二 7 项 checklist 外发现），拆为 17 个修复项。
> 决策（用户已确认）：① 全量修复；② P2-FE-02 矛盾项改代码对齐 checklist 契约；③ E2E 隔离 = run-wdio.cjs 备份/还原 + 用例前置重置；④ 测试补写 = 计划明文 + 防复发守卫 + E2E 动态四态全做。
> ID 前缀：FIX-FE（前端）、FIX-TE（测试）、FIX-DOC（文档）。无后端改动。
> 优先级组织：不另设 P0-P4，由 Stage 依赖顺序表达（根因优先、文档收尾）。

---

## 前端（FIX-FE）

### FIX-FE-01 toast 点击路由复用共享 switchToPage（review §一-1，高）
- 位置：`src/features/notifications/useClaudeNotifications.ts:116-143`（routeToPanel）；基础设施落点 `src/workspace/pageApis.ts`（新建）、`src/workspace/Workspace.tsx:95-120,123-159,162-167`
- 要点：
  - 新建 `pageApis.ts`：模块级 `Map<pageId, DockviewApi>` + `registerPageApi` / `unregisterPageApi` / `getPageApi` + `switchToPageShared(pageId)` + `switchToPageAndFocus(pageId, panelId)`（契约见 stages.md C1）。
  - `Workspace.tsx` 三处改写经 pageApis：`switchToPage`（保留 ensurePageInitialized 后委托 `switchToPageShared`）、`onDeletePage`（unregister + 次页重指向经 `getPageApi`）、`handlePageApiReady`（register + activePageId 匹配时重指向 `window.__dockviewApi`）。`window.__dockviewApi` 重指向仍只允许出现在这三站点。
  - `routeToPanel` 改为：parsePageId → `await switchToPageAndFocus(pageId, panelId)`，删除自行复制的 setProjectRoot + setActivePage + 立即 focus。
  - 连带：`findPanelTitle`（:101-111）改经 `getPageApi(pageId)?.getPanel(panelId)?.title`（跨页面可查，不再依赖 __dockviewApi 恰好指向目标页）。
  - **波及面（E2E 辅助代码同步清扫）**：`e2e-tests/helpers.ts:211-227` `__slterm_e2e_switchToPage` 复制同一时序模式且同样不重指向 __dockviewApi——改为委托 `switchToPageShared`。
- 验证：`notifications.test.ts` 新增守卫——捕获 onClick 触发后断言 `switchToPageAndFocus` 被调用且 `useLayout.setActivePage` 未被 routeToPanel 直接调用；`workspace-switch-order.test.tsx` 14 用例不回归。

### FIX-FE-02 行点击走共享函数（review §一-4，中）
- 位置：`src/features/agentStatus/AgentStatusView.tsx:62-89`（handleFocus）
- 要点：handleFocus 改为 async——解析 pageId（Stage 02 前暂用现有内联解析）→ `await switchToPageAndFocus(pageId, panelId)`；删除"同步 props.switchToPage + 立即 getPanel().focus()"与项目查找循环（共享函数内部处理）。`SideViewComponentProps` 接口不动。
- 验证：`agent-status-view.test.tsx` 点击用例（:229）改写为 mock `pageApis.switchToPageAndFocus` 断言调用参数（panelId → 正确 pageId）。

### FIX-FE-03 TerminalRegistry 订阅联动（review §一-2，中）
- 位置：`src/panels/terminal/TerminalRegistry.ts`（新增 subscribe）、`src/features/agentStatus/useAgentStatus.ts`（消费）
- 要点：
  - `TerminalRegistry` 新增 `subscribe(listener): () => void`（契约见 stages.md C2），`register`/`remove` 内同步通知。
  - `useAgentStatus` 订阅：register 且 panelId 属当前项目 → 插入 🟡 行（同初始扫描语义）；remove → 移除对应行。
- 验证：新建 `src/__tests__/terminal-registry-subscribe.test.ts`（register/remove 通知 + 退订）；`agent-status-hook.test.ts` 新增用例——模拟 register 事件出现行、remove 事件行消失。

### FIX-FE-04 行标题查 dockviewApi（review §一-3，中）
- 位置：`src/features/agentStatus/useAgentStatus.ts:127,208`
- 要点：标题 = `getPageApi(pageId)?.getPanel(panelId)?.title ?? \`终端 ${pageId}\``（初始扫描与事件路径两处）；事件到达时顺带刷新已有行标题。
- 验证：`agent-status-hook.test.ts` 新增用例——mock getPageApi 返回带 title 的面板，断言行标题为页签标题；无面板时回退 `终端 {pageId}`。

### FIX-FE-05 null 状态不覆盖行（review §二-1，中）
- 位置：`src/features/agentStatus/useAgentStatus.ts:100-149`；测试 `src/__tests__/agent-status-hook.test.ts:46-67`
- 要点：
  - `eventToStatus` 返回 null 时不更新行状态（仅刷新 lastEventAt / transcriptPath / 触发用量拉取）；SessionEnd/Exit 移除逻辑不变；`Stop → done` 由真实 eventToStatus 映射，删除 `payload.event === "Stop" ? "done"` 特判。
  - 测试**删除 claudeStatus mock**，改用真实 `eventToStatus`/`getStatusIcon`；审计现有 21 用例对真实映射（SessionStart→attention 等）的断言。
- 验证：新增用例——⚡ 行收到 `Notification(auth_success)` 后状态仍为 ⚡；未知事件同理。

### FIX-FE-06 消除每渲染重订阅（review §二-2，低）
- 位置：`src/features/agentStatus/useAgentStatus.ts:83-85,91-187`
- 要点：`projectPageIds` 用 `useMemo`（deps `[activeProject]`）稳定标识；`handleHookEvent` 的 useCallback deps 随之稳定，`useEffect([handleHookEvent])` 不再每渲染重订阅。
- 验证：新增用例——行更新触发重渲染后，`onHookEvent` 调用次数不增（spy 计数）。

### FIX-FE-07 parsePageId 收敛 src/lib 单点（review §二-6，低）
- 位置：新建 `src/lib/panelId.ts`；替换三处：`useClaudeNotifications.ts:77-81`、`useAgentStatus.ts:48-62`、`AgentStatusView.tsx:65-72`
- 要点：`parseTerminalPageId(panelId): string | null`——≥3 段 + 首段 `terminal` + 末段全数字 → 中间段 join；否则 null（契约见 stages.md C4）。三处全部改调单点，删除本地副本。
- 验证：新建 `src/__tests__/panelId.test.ts`（正常/含连字符 pageId/非数字尾段/非 terminal 前缀/两段）；grep 全仓无 `parsePageId` 本地定义残留。

### FIX-FE-08 sendClickableNotification 契约对齐（review §一-6，低；决策：改代码对齐 checklist）
- 位置：`src/ipc/notification.ts`；调用点 `useClaudeNotifications.ts:226`；测试 `notifications.test.ts:40-42,691-704`
- 要点：
  - 签名改 `(title: string, options: { body: string }, onClick: () => void): Notification | null`（成功路径返回实例；catch 回退 Tauri sendNotification 路径返回 null）。
  - 同文件 re-export `isPermissionGranted` / `requestPermission` / `sendNotification`（直接 re-export 官方插件，契约 C3）。
  - 删除 `sendSilentNotification`（零调用方）。
  - `ensureNotificationPermission` 保留（有调用方）。
  - 调用点改传 `{ body: bodyParts }`。
- 验证：tsc 通过；`notifications.test.ts` 同步 mock 与第二参数断言（对象 `{ body }`）；grep 全仓无 `sendSilentNotification` 残留。

### FIX-FE-09 删除 getContextUsage 别名（review §一-5 连带，低）
- 位置：`src/ipc/hooks.ts:45-49`；`src/__tests__/setup.ts:98`
- 要点：删 `getContextUsage`（保留唯一 wrapper `contextUsage`）；setup.ts 全局 mock 条目 `getContextUsage` → `contextUsage`。ipc/index.ts 为 namespace barrel，无需改。
- 验证：grep 全仓 `getContextUsage` 零命中；npm test 全绿。

### FIX-FE-10 去重注释改真实机制（review §二-7，低）
- 位置：`src/features/notifications/useClaudeNotifications.ts:187`
- 要点：注释「60s 内同一 session + event + timestamp 不重复通知」改为真实机制——同一信号文件重复投递去重（sessionId+event+timestamp 键）+ 缓存超 200 条截断保留最近 100 条。
- 验证：Read 确认注释与 :188-196 实现一致。

---

## 测试（FIX-TE）

### FIX-TE-01 contextUsage 合约四维验证（review §一-5，低）
- 位置：`src/__tests__/ipc-hooks-contract.test.ts`
- 要点：追加 `contextUsage` 用例——命令名 `hooks_context_usage`、参数 `{ transcriptPath }`、返回值透传（`ContextUsage | null`）、异常传播。16 用例 → 20。
- 验证：npm test 中该文件 20 用例全绿。

### FIX-TE-02 用量条颜色 token 断言（review §一-7，低）
- 位置：`src/__tests__/colors.test.ts`、`src/__tests__/agent-status-view.test.tsx:269-330`；连带 `src/theme/index.ts`（re-export 追加 `AGENT_STATUS_USAGE_COLORS`）
- 要点：
  - colors.test.ts 新增 describe：`AGENT_STATUS_USAGE_COLORS` 3 token（low `#629755` / medium `#BBB529` / high `#F44747`）合法 hex + 值精确匹配（从 `../theme` 导入——先把 token 加进 index re-export）。
  - view 测试用量条 describe 补分段颜色断言：mock contextUsage 返回使 percent 落 <50 / 50-80 / >80，断言内层 div `style.backgroundColor` 等于对应 token 值。
- 验证：npm test 两文件全绿。

### FIX-TE-03 L4 Agent Status 行渲染用例启用 + 动态四态（review §一-8，低）
- 位置：`e2e-tests/test.e2e.ts:1592-1697`
- 要点：
  - 静态行部分拆为 active 用例：复用现 it.skip 本体（:1612-1697，panelId=`terminal-{pageId}-0`），断言行出现 + 🟡 + 用量条容器。
  - 新增动态用例：照 :1357「信号文件驱动页签图标流转」先例，Node 侧原子写信号文件（`~/.slterminal/hooks-events/`，.tmp→.json rename）驱动 `PreToolUse → ⚡`、`Stop → ✅`、`SessionEnd → 行消失`，逐态轮询 DOM 断言。panelId 用同一 `terminal-{pageId}-0`。
  - 更新用例注释（删除"E2E 环境 hook 事件不可用"的失实理由）；toast 用例 3（:1733）维持 it.skip 不动。
- 验证：`npm run build:e2e` + `npm run wdio` 两用例 active 且通过。

### FIX-TE-04 E2E settings 隔离 + 侧栏用例前置重置（review §二-4，中；决策：备份/还原 + 前置重置）
- 位置：`e2e-tests/run-wdio.cjs`；`e2e-tests/test.e2e.ts:1027-1035`（拖拽用例第 3 步）与 :970 附近（开关用例前置）
- 要点：
  - run-wdio.cjs：启动时备份 `~/.slterminal/settings.json`（存在则复制为 `settings.json.e2e-bak`）；`process.on('exit', ...)` 同步还原（原文件不存在则删除 E2E 产物；存在则移回并清 bak）。覆盖 node22 直跑/下载/fallback 三路径（exit 钩子天然全覆盖）。
  - 侧栏两用例前置：经 `__slterm_e2e_moveSideViewButton` 将 zones 重置为 `top=[projects,explorer,commit,agent-status]`、`bottom=[]`（逐个 moveButton），再经 toggle 将 open 重置为已知态。
- 验证：wdio 全量 20 active 全绿（含原 2 条确定性失败用例）；E2E 跑完后真实 `~/.slterminal/settings.json` 内容不变（diff 比对）。

### FIX-TE-05 diff-panel flaky 用例加固（review §二-5，低）
- 位置：`src/__tests__/diff-panel.test.tsx:349-363`
- 要点：用例 12 的 `waitFor` 增断言 `[data-e2e="diff-left"] .cm-content` 存在后再取元素；顺带审计同文件用例 13-15 是否同模式等待不足，同标准修复。
- 验证：全量 npm test 连跑 3 轮该文件无失败。

---

## 文档（FIX-DOC）

### FIX-DOC-01 test-inventory.md 失实重写 + 计数更新（review §二-3，中）
- 位置：`.claude/test-inventory.md:24,28,44,45,185,186,187,255,278`
- 要点：
  - 按各文件**实际内容**重写覆盖描述：mod.rs 行剔除 hooks_context_usage；usage.rs 行改正函数名（`parse_usage_line`/`scan_transcript_usage`）与覆盖项；ipc-contract/ipc-hooks-contract 行按 FIX-TE-01 落地后实况写；notifications 行剔 4 项不存在用例；agent-status-hook 行剔"轮询"改事件驱动；agent-status-view 行剔 tooltip/加载态/错误态；L4 行按 FIX-TE-03 落地后实况写；changelog 括注改真实增量构成。
  - 用例数更新：Stage 1-4 完成后的实际计数（新增 panelId.test.ts、terminal-registry-subscribe.test.ts；ipc-hooks-contract 16→20；notifications/agent-status-hook/view/colors 增量；L2 总数重算；L4 18 active→20 active + 1 skip）。
- 验证：grep 失实关键词（`parse_context_usage`、`read_context_usage_file`、`total_tokens`、`exit_code`、`轮询`、`tooltip`）零命中；计数经实跑核对（npm test 统计行 + cargo test 统计行）。

### FIX-DOC-02 CLAUDE.md 系列同步（review §一-6 连带 + Stage 1-4 全部代码变更）
- 位置：`src/ipc/CLAUDE.md`、`src/lib/CLAUDE.md`、`src/workspace/CLAUDE.md`、`src/panels/CLAUDE.md`、`src/features/sideViews/CLAUDE.md`、`e2e-tests/CLAUDE.md`、根 `.claude/CLAUDE.md`
- 要点：
  - ipc：notification.ts 条目写 sendClickableNotification 契约（含 re-export 三函数 + ensureNotificationPermission）；修正 :35「thin wrapper 直接 re-export 不添加额外逻辑」失实描述（notification 含 onclick 工厂逻辑）。
  - lib：文件表追加 `panelId.ts`（parseTerminalPageId）；测试模式段追加 panelId.test.ts。
  - workspace：文件表追加 `pageApis.ts`；「页面切换流」补共享 switchToPageShared/switchToPageAndFocus 与 __dockviewApi 三站点重指向不变量。
  - panels：TerminalRegistry 条目补 subscribe API。
  - sideViews：useAgentStatus 条目补订阅增删/null 跳过/标题查找语义。
  - e2e-tests：补 settings.json 备份/还原机制 + Agent Status 静态/动态用例描述。
  - 根 .claude/CLAUDE.md：workspace 模块行补 pageApis（如需）。
- 验证：逐文件 Read 对照真实代码核实（文档不撒谎）；grep 相关条目关键词命中。
