# 阶段 2 清单：通知 + 会话总览

> 范围：F4 任务栏闪烁/toast + F5 Agent Status 侧栏视图。
> 前置：阶段 1 已完成（`src-tauri/src/hooks/`、`src/ipc/hooks.ts`、`src/lib/claudeStatus.ts` 已落地）。
> ID 前缀：P2-BE（后端）、P2-FE（前端）、P2-TE（测试）、P2-DOC（文档）。

---

## 后端（P2-BE）

### P2-BE-01 Cargo.toml 添加 tauri-plugin-notification 依赖
- 位置：`src-tauri/Cargo.toml`
- 要点：在 `[dependencies]` 追加 `tauri-plugin-notification = "2"`，与现有 `tauri-plugin-dialog = "2"` 对齐。
- 验证：`cargo tree -p slterminal | grep notification` 命中。

### P2-BE-02 lib.rs 初始化 notification 插件并注册 hooks_context_usage
- 位置：`src-tauri/src/lib.rs`
- 要点：
  - `.plugin(tauri_plugin_notification::init())` 加入 `Builder` 链（在 dialog/clipboard 等插件旁）。
  - `generate_handler!` 追加 `hooks_context_usage`（阶段 2 新增命令）。
- 验证：grep `tauri_plugin_notification` 与 `hooks_context_usage` 均在 `lib.rs` 命中。

### P2-BE-03 capabilities/default.json 追加 notification 权限
- 位置：`src-tauri/capabilities/default.json`
- 要点：在 `permissions` 数组追加 `"notification:default"`（硬约束 #10：显式放行，不加 `*`）。
- 验证：capabilities JSON 解析通过，`notification:default` 存在。

### P2-BE-04 hooks 模块新增 `hooks_context_usage` 命令
- 位置：`src-tauri/src/hooks/`（具体文件由阶段 1 结构决定，推荐 `usage.rs` 或并入 `mod.rs`）
- 契约：
  - 命令名：`hooks_context_usage`
  - 参数：`(transcript_path: String)`（Rust）/ `{ transcriptPath: string }`（JS）
  - 返回：`Result<Option<ContextUsage>, AppError>`，成功时返回 `Some({ input_tokens, output_tokens })`，失败降级为 `Ok(None)`
- 实现要点：
  - `spawn_blocking` 内执行（阻塞 I/O）。
  - 打开 `transcript_path` 文件，从尾部读取（避免加载数百 MB 全文件）。
  - 逆行扫描最后 N 行（建议 64 行），逐行 `serde_json::from_str`；遇到第一个含 `message.usage.input_tokens` 且 `output_tokens` 存在的行即返回。
  - 解析失败/无 usage/文件不存在 → `Ok(None)`，不 panic。
- 验证：L1 测试覆盖正常/损坏/无 usage/空文件/大文件 tail 读取。

### P2-BE-05 定义 ContextUsage DTO（双边对应）
- Rust 位置：`src-tauri/src/hooks/` DTO 模块（或 `src-tauri/src/types/hooks.rs` 若阶段 1 已建）
- JS 位置：`src/types/hooks.ts`（阶段 1 应已创建，追加 `ContextUsage`）
- 字段：
  - Rust：`input_tokens: u64`, `output_tokens: u64`，serde camelCase。
  - JS：`inputTokens: number`, `outputTokens: number`。
- 验证：ipc-contract.test.ts 扩展覆盖 `hooks.contextUsage` 命令名与参数结构。

### P2-BE-06 signal 解析确保 transcriptPath 透传
- 位置：`src-tauri/src/hooks/signal.rs`（阶段 1 结构）
- 要点：C1 字段 `transcriptPath` 已约定；本项仅做回归断言——信号解析后 `HookEventPayload` 含非空 `transcript_path`，缺失时该条事件仍透传但 F5 用量条标记不可用。
- 验证：grep `transcript_path`/`transcriptPath` 在 signal 解析与事件 DTO 中均存在。

---

## 前端（P2-FE）

### P2-FE-01 package.json 添加 @tauri-apps/plugin-notification
- 位置：`package.json`
- 要点：`dependencies` 追加 `"@tauri-apps/plugin-notification": "^2"`，与 dialog/opener 对齐。
- 验证：`npm install` 后 `node_modules/@tauri-apps/plugin-notification` 存在。

### P2-FE-02 src/ipc/notification.ts 作为 thin wrapper + 可点击 toast 工厂
- 位置：`src/ipc/notification.ts`（新建）
- 要点：照 `src/ipc/clipboard.ts` 先例，直接 re-export 官方插件：
  - `isPermissionGranted`
  - `requestPermission`
  - `sendNotification`
- 额外新增 `sendClickableNotification(title, options, onClick)`：
  - 内部使用 `new Notification(title, options)`（Tauri v2 notification 插件 guest-js 底层即调用 `new window.Notification(...)`）。
  - 绑定实例 `onclick = onClick`，把点击路由逻辑收敛到 IPC 层。
  - 返回创建的 `Notification` 实例，便于测试。
- 原因：Tauri v2 `sendNotification` 的 `Options` 接口**不含 JS `onClick` 回调**；要实现“点击 toast 聚焦并跳转页签”，必须直接使用 Web Notification API 的 `onclick`。
- 验证：`src/ipc/index.ts` barrel export 含 `notification`；不存在前端其它文件直接 `import @tauri-apps/plugin-notification`。

### P2-FE-03 src/ipc/hooks.ts 追加 contextUsage wrapper
- 位置：`src/ipc/hooks.ts`（阶段 1 创建）
- 要点：新增 `async function contextUsage(transcriptPath: string): Promise<ContextUsage | null>`，调用 `invoke("hooks_context_usage", { transcriptPath })`。
- 验证：ipc-contract.test.ts 覆盖命令名、参数结构、返回值透传。

### P2-FE-04 App.tsx 集成 notification 插件初始化与窗口焦点状态
- 位置：`src/App.tsx`
- 要点：
  - `useEffect` 中初始化窗口焦点监听（`getCurrentWindow().onFocusChanged(({ payload: focused }) => ...)`）。
  - 焦点状态写入模块级原子（如 `window.__slterm_windowFocused`）或导出 ref，供 notification dispatcher 读取。
  - 挂载 `<NotificationListener />`（见 P2-FE-05）。
- 验证：grep `onFocusChanged` / `__slterm_windowFocused` 命中；notification 模块在失焦门控中读取该状态。

### P2-FE-05 新建通知调度模块 useClaudeNotifications
- 位置：`src/features/notifications/useClaudeNotifications.ts`（新建）
- 要点：
  - 订阅 `onHookEvent`（阶段 1）。
  - 门控：仅 `windowFocused === false` 时触发通知。
  - 三类事件映射（C1 event 字段 + notificationType）：
    - 权限请求：`PermissionRequest` 或 `Notification` 且 `notificationType === "permission_prompt"`。
    - 任务完成：`Stop`。
    - 错误：`StopFailure` / `PostToolUseFailure`。
  - 任务栏闪烁：仅“注意态🟡”期间调用 `getCurrentWindow().requestUserAttention(UserAttentionType.Critical)`（Tauri v2 在 Windows 上等效于任务栏闪烁；窗口聚焦后调用 `requestUserAttention(null)` 停止）。**不依赖 `flashFrame`**（Tauri v2 JS API 未暴露 `flashFrame`）。
  - toast 内容：项目名 + 页签标题 + 事件类别 + 时间。
  - toast 创建：调用 `src/ipc/notification.ts` 的 `sendClickableNotification`，将 `panelId`/`pageId`/`projectId` 通过闭包或 `data` 传入 onclick。
  - 多会话：各自独立 toast，不聚合。
- 验证：L2 测试 mock 焦点状态、mock `sendClickableNotification`/`requestUserAttention`、断言三类事件触发/聚焦时不触发/权限请求才闪烁。

### P2-FE-06 toast 点击路由
- 位置：`src/features/notifications/useClaudeNotifications.ts`（通过 `sendClickableNotification` 的 `onClick`）
- 要点：`Notification` 实例的 `onclick` 中：
  - 调用 `getCurrentWindow().setFocus()` 聚焦窗口。
  - 通过已保存的 `panelId` 解析 `pageId`（格式 `terminal-{pageId}-{seq}`）。
  - 查找所属 `projectId`，调用 `switchToPage(projectId, pageId)`。
  - 调用 `window.__dockviewApi?.getPanel(panelId)?.focus()`；页签已关闭时仅聚焦窗口，不报错。
- 约束：**禁止**在 `sendNotification` 的 Options 上写 `onClick`（该字段不存在）。
- 验证：L2 mock window.__dockviewApi 与 switchToPage，断言调用参数；断言未直接调用 `sendNotification({ onClick })`。

### P2-FE-07 sideViewDefs.ts 注册 agent-status 视图
- 位置：`src/features/sideViews/sideViewDefs.ts`
- 要点：追加 `sideViewRegistry.register({ id: "agent-status", title: "Agent 状态", icon: "🤖", component: AgentStatusView })`。
- 验证：grep `agent-status` 命中，sideViewRegistry.getAll() 测试包含该 id。

### P2-FE-08 sideBarState.ts DEFAULT_ZONES 追加 agent-status
- 位置：`src/features/sideViews/sideBarState.ts`
- 要点：`DEFAULT_ZONES.top` 追加 `"agent-status"`（位于 `"commit"` 之后）。
- 验证：sideBarState.test.ts 默认值断言更新。

### P2-FE-09 新建 AgentStatusView 组件
- 位置：`src/features/agentStatus/AgentStatusView.tsx`（新建目录）
- 要点：
  - 接受 `SideViewComponentProps`（`switchToPage`、`onDeletePage`），与 `SidebarTree`/`ExplorerPanel`/`CommitView` 对齐。
  - 状态机（优先级自上而下）：`no-root` / `empty` / `ready`。
  - 标题栏 "AGENT STATUS"（28px、大写、样式照 CommitView）。
- 验证：L2 渲染测试覆盖三态与 props 透传。

### P2-FE-10 新建 useAgentStatus hook
- 位置：`src/features/agentStatus/useAgentStatus.ts`
- 要点：
  - 从 `useLayout` + `useProjects` 推导当前项目 rootPath/cwd。
  - 维护 `Map<panelId, AgentSessionRow>` 状态。
  - 事件源：`TerminalRegistry.getAll()` 的 panelId 列表 + `onHookEvent` 事件驱动更新。
  - 过滤：仅保留 panelId 所属 pageId 在当前项目内的会话。
  - 生命周期：
    - `SessionStart` / `UserPromptSubmit` / `PreToolUse` / `PostToolUse` / `PermissionRequest` / `Notification` 等工作/注意事件 → 插入或更新行。
    - `Stop` → 行状态置 `done`（**保留在行列表**，视图仍展示任务完成态）。
    - `SessionEnd` / OSC 133 D（exit 事件）→ 立即移除。
  - 排序：按 `lastEventAt` 倒序。
- 验证：L2 测试断言 Stop 后状态为 done 且仍在列表，SessionEnd/exit 才移除。

### P2-FE-11 新建 AgentStatusRow 组件
- 位置：`src/features/agentStatus/AgentStatusRow.tsx`
- 要点：
  - 显示页签标题（从 panelId 在 titleManager 无直接注册时，退化为 panelId 中的 pageId 或 “未命名终端”）。
  - 四态图标：复用 `src/lib/claudeStatus.ts` 的 `getStatusIcon(status)`。
  - 上下文用量条：百分比 = `(inputTokens + outputTokens) / CLAUDE_CONTEXT_LIMIT * 100`，`CLAUDE_CONTEXT_LIMIT = 200_000`（单点常量，见 P2-FE-13）。
  - 不可用态：无 transcriptPath 或 `contextUsage` 返回 null → 用量条置灰 + 显示 “--”。
  - 颜色：用量条分段色从 `theme/colors.ts` 的 `AGENT_STATUS_USAGE_COLORS` 读取，禁止硬编码。
- 验证：L2 断言图标字符、用量条宽度、颜色 token 引用、"--" 降级态。

### P2-FE-12 点击行切页面聚焦页签
- 位置：`src/features/agentStatus/AgentStatusRow.tsx` 或 `AgentStatusView.tsx`
- 要点：点击行时：
  - 解析 panelId → pageId。
  - 调用 `switchToPage(projectId, pageId)`（props 透传）。
  - 调用 `window.__dockviewApi?.getPanel(panelId)?.focus()`。
- 验证：L2 mock `window.__dockviewApi` 与 `switchToPage` 回调。

### P2-FE-13 用量条降级与事件驱动刷新 + 常量单点
- 位置：`src/features/agentStatus/useAgentStatus.ts` / `src/features/agentStatus/consts.ts`
- 要点：
  - `CLAUDE_CONTEXT_LIMIT = 200_000` 在前端单点定义（推荐 `src/features/agentStatus/consts.ts` 并 export），`AgentStatusRow` 仅引用不复制。
  - 新 hook-event 到达且含 `transcriptPath` 时，异步调用 `hooksContextUsage(transcriptPath)` 更新该行的 `usage`。
  - 解析失败不抛异常，行仍显示，用量条进入不可用态。
  - 不轮询：仅在事件到达时触发一次读取。
- 验证：L2 mock `hooksContextUsage` reject/null/正常值，断言 UI 状态；grep 确认 `200000`/`200_000` 只出现在 consts 文件。

### P2-FE-14 新增 Agent Status 配色 token
- 位置：`src/theme/colors.ts`
- 要点：新增 `AGENT_STATUS_USAGE_COLORS` 对象，含 `low` / `medium` / `high` 三段色值（阈值 50%/80%），供 `AgentStatusRow` 用量条使用。
- 验证：`src/__tests__/colors.test.ts` 更新断言；`src/features/agentStatus/` 中不存在硬编码十六进制色值。

---

## 测试（P2-TE）

### P2-TE-01 L2 notification 门控测试
- 文件：`src/__tests__/notifications.test.ts`
- 覆盖：窗口聚焦时三类事件均不发 toast；失焦时权限请求/Stop/StopFailure 发 `sendClickableNotification`；flashFrame 仅在权限请求时调用 `requestUserAttention(UserAttentionType.Critical)`；点击 `Notification.onclick` 路由到 switchToPage + focus。

### P2-TE-02 L2 AgentStatusView 渲染测试
- 文件：`src/__tests__/agent-status-view.test.tsx`
- 覆盖：`no-root` / `empty` / `ready` 三态；多行渲染；行点击调用 switchToPage + focus；props 透传 `SideViewComponentProps`。

### P2-TE-03 L2 useAgentStatus 状态联动测试
- 文件：`src/__tests__/agent-status-hook.test.ts`（或合并入 view test）
- 覆盖：事件到达插入/更新行；Stop 事件状态变 done 且不立即移除；SessionEnd/exit 移除；切换项目清空旧行；按时间倒序排序。

### P2-TE-04 L2 用量条降级测试
- 文件：`src/__tests__/agent-status-view.test.ts`（或独立文件）
- 覆盖：`contextUsage` 返回 null 时行仍显示且用量条不可用；正常值时百分比按 `CLAUDE_CONTEXT_LIMIT` 计算正确；颜色 token 来自 `AGENT_STATUS_USAGE_COLORS`。

### P2-TE-05 L1 hooks_context_usage 解析测试
- 文件：`src-tauri/src/hooks/` 内 `#[cfg(test)] mod tests`（或 `tests/hooks_context_usage_tests.rs`）
- 覆盖：正常 JSONL 含 usage、无 usage、JSON 损坏、空文件、大文件 tail 读取只读尾部。

### P2-TE-06 L4 E2E 关键路径用例
- 文件：`e2e-tests/test.e2e.ts` 追加 describe
- 覆盖：
  - Agent Status 视图行渲染（真实 claude 会话触发事件后视图出现行）。
  - toast 触发链路（失焦 + 权限请求/Stop/错误时系统通知中心出现 slTerminal 通知）。
- 注意：toast 点击跳转依赖真实 WebView2/OS 通知行为，若无法稳定自动化则列为“人工验证点”。

---

## 文档（P2-DOC）

### P2-DOC-01 更新 src/ipc/CLAUDE.md
- 增加 `notification.ts` 条目（thin wrapper 先例 + `sendClickableNotification` 工厂说明）。
- 更新 `hooks.ts` 条目，追加 `contextUsage` 命令说明。

### P2-DOC-02 更新 src-tauri/src/hooks/CLAUDE.md
- 追加 `hooks_context_usage` 命令说明、DTO 字段、测试位置。
- 说明 tail 读取 + 逆行扫描 + 降级 null 的实现要点。

### P2-DOC-03 更新 src/features/sideViews/CLAUDE.md
- 在“侧栏视图扩展指南”示例中追加 `agent-status`。
- 更新 DEFAULT_ZONES 描述。

### P2-DOC-04 更新 .claude/test-inventory.md
- 追加阶段 2 新增测试文件与用例数（执行 Stage 4 后按实际计数回填）。

### P2-DOC-05 回填 docs/hooks-dev/contract.md C12
- 将 `hooks_context_usage` 命令名、参数、ContextUsage DTO 字段写入 C12，与 stages.md 一致。

---

## 跨边界契约（写死）

| 契约项 | 值 |
|--------|-----|
| 后端命令名 | `hooks_context_usage` |
| 参数 | `{ transcript_path: string }` → JS `{ transcriptPath: string }` |
| 返回 DTO（Rust） | `ContextUsage { input_tokens: u64, output_tokens: u64 }`，serde camelCase |
| 返回 DTO（JS） | `ContextUsage { inputTokens: number, outputTokens: number }` \| `null` |
| 事件名 | `hook-event`（阶段 1 已约定） |
| 侧栏视图 id | `agent-status` |
| 上下文上限常量 | `CLAUDE_CONTEXT_LIMIT = 200_000`（前端单点：`src/features/agentStatus/consts.ts`） |
| 通知权限 | `notification:default` |
| 任务栏闪烁 API | Tauri `getCurrentWindow().requestUserAttention(UserAttentionType.Critical)`（值 `1`） |
| toast 点击实现 | 通过 Web Notification API `new Notification(...)` + `onclick`（Tauri v2 `sendNotification` Options 无 `onClick`） |
