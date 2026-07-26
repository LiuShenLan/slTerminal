# 阶段 2 Stage 划分

> 目标：F4 通知 + F5 Agent Status 侧栏视图。
> 前置：阶段 1 已完成（`src-tauri/src/hooks/`、`src/ipc/hooks.ts`、`src/lib/claudeStatus.ts`）。
> 划分原则：Stage 内文件零重叠；跨前后端单一任务独立成 Stage；文档同步固定为最后 Stage。

---

## Stage 1：后端 notification 插件 + hooks_context_usage 命令

### 负责项 ID
P2-BE-01 / P2-BE-02 / P2-BE-03 / P2-BE-04 / P2-BE-05 / P2-BE-06

### agent 文件分工表

| label | 负责项 | 文件 |
|-------|--------|------|
| backend-deps | P2-BE-01 | `src-tauri/Cargo.toml` |
| backend-plugin | P2-BE-02 / P2-BE-03 | `src-tauri/src/lib.rs`、`src-tauri/capabilities/default.json` |
| backend-usage | P2-BE-04 / P2-BE-05 / P2-BE-06 | `src-tauri/src/hooks/` 内相关文件（含 mod.rs/usage.rs/signal.rs/DTO）、`src/types/hooks.ts` |

### 实现要点
- `Cargo.toml` 追加 `tauri-plugin-notification = "2"`。
- `lib.rs`：`.plugin(tauri_plugin_notification::init())` 加入 Builder 链；`generate_handler!` 追加 `hooks_context_usage`。
- `capabilities/default.json` 追加 `"notification:default"`。
- hooks 模块新增命令 `hooks_context_usage(transcriptPath)`：
  - `spawn_blocking` 内 tail 读取 JSONL（建议一次性读最后 64KB 再按行分割，避免逐行 seek）。
  - 从最后一行逆行扫描，遇到 `message.usage.input_tokens` 与 `output_tokens` 均存在的行即返回。
  - 任何解析失败返回 `Ok(None)`。
- ContextUsage DTO：Rust `input_tokens/output_tokens`（camelCase serde），JS `inputTokens/outputTokens`。
- signal 解析确认 `transcriptPath` 字段透传（阶段 1 应已含本字段，本 Stage 做回归）。

### 验证项
- `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings` 通过。
- `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1` 通过（P2-TE-05 测试随本 Stage 或 Stage 4 落地，此处先保证编译通过）。

### 人工验证点
- 无（本 Stage 为纯后端基建）。

### commit message
`feat: 后端 notification 插件接入 + hooks_context_usage 命令`

---

## Stage 2：前端 F4 通知调度（toast + 任务栏闪烁）

### 负责项 ID
P2-FE-01 / P2-FE-02 / P2-FE-03 / P2-FE-04 / P2-FE-05 / P2-FE-06

### agent 文件分工表

| label | 负责项 | 文件 |
|-------|--------|------|
| fe-deps | P2-FE-01 | `package.json` |
| fe-ipc | P2-FE-02 / P2-FE-03 | `src/ipc/notification.ts`、`src/ipc/index.ts`、`src/ipc/hooks.ts`、`src/types/hooks.ts` |
| fe-focus | P2-FE-04 | `src/App.tsx` |
| fe-notify | P2-FE-05 / P2-FE-06 | `src/features/notifications/useClaudeNotifications.ts`（新建） |

### 实现要点
- package.json 追加 `@tauri-apps/plugin-notification`。
- `src/ipc/notification.ts`：re-export `isPermissionGranted` / `requestPermission` / `sendNotification`；新增 `sendClickableNotification(title, options, onClick)` 工厂，内部 `new Notification(title, options)` 并绑定 `onclick`。
- `src/ipc/hooks.ts`：追加 `contextUsage(transcriptPath)` wrapper。
- `App.tsx`：
  - 引入 `getCurrentWindow`（或从 `src/ipc/window` 封装）。
  - `useEffect` 订阅窗口焦点变化（`onFocusChanged(({ payload: focused }) => ...)`），写入模块级 `window.__slterm_windowFocused` 或 ref。
  - 挂载 `NotificationListener`（基于 `useClaudeNotifications` 的组件）。
- `useClaudeNotifications.ts`：
  - 订阅 `onHookEvent`。
  - 门控：仅 `!windowFocused` 时触发。
  - 三类事件：权限请求（`PermissionRequest` / `Notification`+`permission_prompt`）、完成（`Stop`）、错误（`StopFailure`/`PostToolUseFailure`）。
  - 任务栏闪烁：注意态期间调用 `getCurrentWindow().requestUserAttention(UserAttentionType.Critical)`（值 `1`，Windows 上等价于任务栏闪烁）；窗口聚焦后调用 `requestUserAttention(null)` 停止。**不使用 `flashFrame`**（Tauri v2 JS API 未暴露）。
  - toast 内容：项目名 + 页签标题 + 类别 + 时间。
  - toast 点击：通过 `sendClickableNotification` 创建 `Notification` 实例并绑定 `onclick`；onclick 中聚焦窗口 → 解析 pageId/projectId → `switchToPage` → `dockviewApi.getPanel(panelId)?.focus()`。
  - **禁止**在 `sendNotification` Options 上写 `onClick`（该字段不存在）。

### 验证项
- `npx tsc --noEmit` 通过。
- `npx eslint src/` 通过。
- L2 测试（P2-TE-01）通过（可在本 Stage 或 Stage 4 落地，但门禁需全绿）。

### 人工验证点
- 真实 claude 场景：窗口失焦 + 权限请求 → 系统通知中心出现 slTerminal toast + 任务栏闪烁；点击 toast 本体 → 窗口聚焦并跳到正确页签。
- 窗口聚焦状态下三类事件 → 无 toast、无闪烁。

### commit message
`feat: F4 通知调度——失焦门控 + toast + 任务栏闪烁 + 点击路由`

---

## Stage 3：前端 F5 Agent Status 侧栏视图

### 负责项 ID
P2-FE-07 / P2-FE-08 / P2-FE-09 / P2-FE-10 / P2-FE-11 / P2-FE-12 / P2-FE-13 / P2-FE-14

### agent 文件分工表

| label | 负责项 | 文件 |
|-------|--------|------|
| fe-registry | P2-FE-07 / P2-FE-08 | `src/features/sideViews/sideViewDefs.ts`、`src/features/sideViews/sideBarState.ts` |
| fe-view | P2-FE-09 | `src/features/agentStatus/AgentStatusView.tsx`（新建） |
| fe-hook | P2-FE-10 / P2-FE-13 | `src/features/agentStatus/useAgentStatus.ts`（新建）、`src/features/agentStatus/consts.ts`（新建） |
| fe-row | P2-FE-11 / P2-FE-12 / P2-FE-14 | `src/features/agentStatus/AgentStatusRow.tsx`（新建）、`src/theme/colors.ts` |
| fe-barrel | P2-FE-09 | `src/features/agentStatus/index.ts`（新建） |

### 实现要点
- `sideViewDefs.ts` 注册 `agent-status`（id/title="Agent 状态"/icon="🤖"/component=AgentStatusView）。
- `sideBarState.ts`：`DEFAULT_ZONES.top` 追加 `"agent-status"`。
- 新建 `src/features/agentStatus/`：
  - `AgentStatusView.tsx`：接受 `SideViewComponentProps`（`switchToPage`、`onDeletePage`），三态（no-root / empty / ready），标题栏 "AGENT STATUS"，样式照 CommitView。
  - `useAgentStatus.ts`：
    - 从 `useLayout` + `useProjects` 推导当前项目。
    - 初始扫描 `TerminalRegistry.getAll()` 获取 panelId 列表。
    - 订阅 `onHookEvent`，按 panelId 更新行状态（`working/attention/done/error` 来自 `claudeStatus.ts`）。
    - `Stop` → 状态改为 `done`（**保留在行列表**）。
    - `SessionEnd`/exit 事件移除行；面板关闭/TerminalRegistry.remove 同样移除。
    - 过滤：仅保留 panelId 所属 pageId 在当前项目内的会话。
    - 排序：`lastEventAt` 倒序。
    - 事件驱动调用 `hooksContextUsage(transcriptPath)` 更新用量。
  - `consts.ts`：单点导出 `CLAUDE_CONTEXT_LIMIT = 200_000`。
  - `AgentStatusRow.tsx`：
    - 标题：优先从 titleManager 查找（无则回退 panelId 中 pageId 或 "终端"）。
    - 四态图标：调用 `claudeStatus.ts` 映射。
    - 用量条：`percent = (inputTokens + outputTokens) / CLAUDE_CONTEXT_LIMIT * 100`，超限 clamp 100%。
    - 颜色：从 `theme/colors.ts` 的 `AGENT_STATUS_USAGE_COLORS` 读取 low/medium/high 三段色，禁止硬编码。
    - 不可用态：无 transcriptPath 或 usage 为 null → 灰条 + "--"。
    - 点击：解析 pageId → `switchToPage` → `dockviewApi.getPanel(panelId)?.focus()`。
  - `index.ts`：barrel export。
- `src/theme/colors.ts`：新增 `AGENT_STATUS_USAGE_COLORS` token 组。

### 验证项
- `npx tsc --noEmit` 通过。
- `npx eslint src/` 通过。
- sideBarState 默认值测试更新通过。

### 人工验证点
- 当前项目开多个 claude 页签 → Agent Status 视图显示多行，四态随工作变化。
- 切换项目 → 视图清空并加载新项目会话。
- 点击行 → 跨页面跳转并聚焦页签。
- 未注入 hooks 的 claude 会话 → 行存在、🟡、用量条不可用。

### commit message
`feat: F5 Agent Status 侧栏视图——运行中会话列表 + 四态 + 用量条`

---

## Stage 4：测试补全（L1/L2 + L4 用例骨架）

### 负责项 ID
P2-TE-01 / P2-TE-02 / P2-TE-03 / P2-TE-04 / P2-TE-05 / P2-TE-06

### agent 文件分工表

| label | 负责项 | 文件 |
|-------|--------|------|
| test-l2-notify | P2-TE-01 | `src/__tests__/notifications.test.ts`（新建） |
| test-l2-view | P2-TE-02 / P2-TE-04 | `src/__tests__/agent-status-view.test.tsx`（新建） |
| test-l2-hook | P2-TE-03 | `src/__tests__/agent-status-hook.test.ts`（新建） |
| test-l1 | P2-TE-05 | `src-tauri/src/hooks/` 内测试模块（或 `tests/hooks_context_usage_tests.rs`） |
| test-l4 | P2-TE-06 | `e2e-tests/test.e2e.ts` 追加用例 |

### 实现要点
- L2 notification 测试：mock `@tauri-apps/plugin-notification`、`getCurrentWindow`、焦点状态、`onHookEvent` 回调、`sendClickableNotification` 工厂，断言 toast/任务栏闪烁触发规则、点击路由。
- L2 agent-status 测试：mock `TerminalRegistry`、`onHookEvent`、`hooksContextUsage`、dockviewApi、stores，断言三态/行渲染/点击跳转/用量降级/Stop→done/SessionEnd→remove。
- L1 context_usage 测试：使用 `tempfile::NamedTempFile` 构造 JSONL，覆盖正常/损坏/无 usage/空文件/大文件 tail。
- L4 E2E：追加 describe 块，覆盖视图行渲染与 toast 链路；若通知中心无法稳定断言，则将 toast 断言标记为“人工验证点”。

### 验证项
- `npm test` 通过。
- `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1` 通过。
- `npx tsc --noEmit` 通过。
- `npx eslint src/` 通过。

### 人工验证点
- L4 E2E 中 toast 真实出现与点击跳转需在真实 Windows 环境人工复核（参见 Stage 2 人工验证点）。

### commit message
`test: 阶段 2 L1/L2/L4 测试补全`

---

## Stage 5：文档同步

### 负责项 ID
P2-DOC-01 / P2-DOC-02 / P2-DOC-03 / P2-DOC-04 / P2-DOC-05

### agent 文件分工表

| label | 负责项 | 文件 |
|-------|--------|------|
| doc-ipc | P2-DOC-01 | `src/ipc/CLAUDE.md` |
| doc-hooks | P2-DOC-02 | `src-tauri/src/hooks/CLAUDE.md` |
| doc-sideviews | P2-DOC-03 | `src/features/sideViews/CLAUDE.md` |
| doc-inventory | P2-DOC-04 | `.claude/test-inventory.md` |
| doc-contract | P2-DOC-05 | `docs/hooks-dev/contract.md` |

### 实现要点
- `src/ipc/CLAUDE.md`：追加 `notification.ts` wrapper 说明（含 `sendClickableNotification`）与 `hooks.ts` 的 `contextUsage`。
- `src-tauri/src/hooks/CLAUDE.md`：追加 `hooks_context_usage` 命令、DTO、实现要点、测试位置。
- `src/features/sideViews/CLAUDE.md`：扩展指南示例加入 `agent-status`；更新 DEFAULT_ZONES 描述。
- `.claude/test-inventory.md`：按实际用例数追加阶段 2 测试条目。
- `docs/hooks-dev/contract.md` C12：回填 `hooks_context_usage` 命令名、参数、ContextUsage 字段。

### 验证项
- 文档中命令名/DTO 字段与代码一致（grep 核对）。
- `npx tsc --noEmit`、`npx eslint src/`、`cargo clippy`、`npm test`、`cargo test` 全绿。

### 人工验证点
- 无。

### commit message
`docs: 同步阶段 2 IPC/hooks/侧栏视图文档与契约`

---

## 跨边界契约汇总（各 Stage 脚本头部须复制）

```
命令名：hooks_context_usage
参数：{ transcriptPath: string }
返回：{ inputTokens: number, outputTokens: number } | null
事件名：hook-event
侧栏视图 id：agent-status
上下文上限：CLAUDE_CONTEXT_LIMIT = 200_000（前端单点 src/features/agentStatus/consts.ts）
通知权限：notification:default
任务栏闪烁：getCurrentWindow().requestUserAttention(UserAttentionType.Critical)（值 1）
toast 点击实现：new Notification(title, options) + onclick（sendNotification Options 无 onClick）
```

## 人工验证点汇总

1. 真实 claude 场景：窗口失焦 + 权限请求 → toast 出现 + 任务栏闪烁；点击 toast 本体 → 窗口聚焦并落到正确页签。
2. 窗口失焦 + 长任务 Stop → toast 出现；窗口聚焦时不发。
3. Agent Status 视图：当前项目多 claude 页签 → 多行、四态正确、点击跨页面跳转聚焦正确。
4. 切换项目 → 视图内容切换到新项目。
5. 未注入 hooks 的会话 → 行存在、🟡、用量条不可用。
6. 上下文用量随 claude 工作推进而更新（事件驱动，非轮询）。
7. Stop 事件后对应行状态为 done 并保留，SessionEnd/exit 后行移除。
