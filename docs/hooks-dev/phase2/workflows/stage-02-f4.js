// =====================================================================
// Stage 2 Workflow：前端 F4 通知调度（toast + 任务栏闪烁）
// =====================================================================
// 跨边界契约（本脚本头部写死，agent 不各自推断）：
//   事件名：hook-event
//   命令名：hooks_context_usage（Stage 1 已注册）
//   通知权限：notification:default
//   任务栏闪烁 API：Tauri getCurrentWindow().requestUserAttention(UserAttentionType.Critical)（值 1）
//   toast 点击实现：new Notification(title, options) + onclick（sendNotification Options 无 onClick）
//   toast 内容格式：<项目名> · <页签标题> · <事件类别>
// =====================================================================

export const meta = {
  name: 'stage2-f4-notifications',
  description: '阶段 2 Stage 2：前端 F4 通知调度',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:\data\learn\code\slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入。
背景：修复要点详见 checklist 对应 ID 条目（先读再动手）。
Stage 特殊纪律：本 Stage 只改前端代码；禁止改后端、禁止改 ConPTY；所有 Tauri 调用必须经 src/ipc/ 封装（invoke 单点）；焦点检测优先使用 Tauri Window API，不稳定则降级 DOM focus/blur；toast 点击必须走 Web Notification API 的 onclick，禁止在 sendNotification Options 上写不存在的 onClick。`

// === Phase 1: 并行重构 ===
phase('并行重构')
const parallelAgents = [
  {
    label: 'fe-deps',
    prompt: `你负责 P2-FE-01：在 package.json 添加 @tauri-apps/plugin-notification 依赖。

【P2-FE-01】在 dependencies 追加 \`"@tauri-apps/plugin-notification": "^2"\`，与现有 dialog/opener 插件对齐。

要求：
- 不要修改其他依赖。
- 完成后报告 package.json 修改行。`
  },
  {
    label: 'fe-ipc',
    prompt: `你负责 P2-FE-02 / P2-FE-03：新增 src/ipc/notification.ts thin wrapper（含可点击 toast 工厂），并在 src/ipc/hooks.ts 追加 contextUsage wrapper。

【P2-FE-02】src/ipc/notification.ts（新建）：
- 照 src/ipc/clipboard.ts 先例，直接 re-export 官方插件：
  \`\`\`ts
  export { isPermissionGranted, requestPermission, sendNotification } from "@tauri-apps/plugin-notification";
  \`\`\`
- 新增工厂函数 \`sendClickableNotification\`：
  \`\`\`ts
  export function sendClickableNotification(
    title: string,
    options: NotificationOptions,
    onClick: (this: Notification) => void
  ): Notification {
    const n = new Notification(title, options);
    n.onclick = onClick;
    return n;
  }
  \`\`\`
  原因：Tauri v2 \`sendNotification\` 的 Options 接口不含 JS \`onClick\` 回调（官方 guest-js/index.ts 实现为 \`new window.Notification(...)\`），要实现“点击 toast 聚焦并跳转页签”必须直接用 Web Notification API 的 onclick。
- 在 src/ipc/index.ts 追加 \`export * as notification from "./notification";\`。

【P2-FE-03】src/ipc/hooks.ts（阶段 1 已创建）：
- 新增函数：
  \`\`\`ts
  import type { ContextUsage } from "../types/hooks";
  export async function contextUsage(transcriptPath: string): Promise<ContextUsage | null> {
    return await invoke("hooks_context_usage", { transcriptPath });
  }
  \`\`\`
- 确保 src/types/hooks.ts 已定义 ContextUsage DTO（Stage 1 负责，本项仅确认或补齐）。

要求：
- 不要在前端其他位置直接 import @tauri-apps/plugin-notification。
- 完成后报告新增/修改文件清单。`
  },
  {
    label: 'fe-notify',
    prompt: `你负责 P2-FE-04 / P2-FE-05 / P2-FE-06：在 App.tsx 集成窗口焦点监听并新建通知调度模块 useClaudeNotifications，实现失焦门控、三类事件 toast、任务栏闪烁、点击路由。

前置：src/ipc/hooks.ts 的 onHookEvent 已可用（阶段 1）；src/ipc/notification.ts 已完成（本 Stage fe-ipc agent）。

【P2-FE-04】App.tsx：
- 引入 \`getCurrentWindow\`（来自 @tauri-apps/api/window）或封装到 src/ipc/window.ts。
- 新增 \`useEffect\` 监听窗口焦点变化：
  \`\`\`ts
  const appWindow = getCurrentWindow();
  appWindow.onFocusChanged(({ payload: focused }) => {
    window.__slterm_windowFocused = focused;
  });
  \`\`\`
  若该 API 不稳定，兜底使用 \`window.addEventListener("focus"/"blur")\`。
- 在 App 渲染的 Workspace 旁（或内部）挂载 \`NotificationListener\` 组件，该组件调用 \`useClaudeNotifications()\`。

【P2-FE-05】src/features/notifications/useClaudeNotifications.ts（新建）：
- 导出函数 \`useClaudeNotifications()\` 与组件 \`NotificationListener\`（或纯 hook，由 App.tsx 调用）。
- 订阅 \`onHookEvent\`。
- 门控：仅当 \`window.__slterm_windowFocused === false\` 时触发通知。
- 三类事件映射（依据 event 字段与 notificationType）：
  - 权限请求：\`event === "PermissionRequest"\` 或 (\`event === "Notification"\` 且 \`notificationType === "permission_prompt"\`)。
  - 任务完成：\`event === "Stop"\`。
  - 错误：\`event === "StopFailure"\` 或 \`event === "PostToolUseFailure"\`。
- 任务栏闪烁：注意态（权限请求）期间调用 \`getCurrentWindow().requestUserAttention(UserAttentionType.Critical)\`（值 1，Windows 上等价于任务栏闪烁）；窗口聚焦后调用 \`requestUserAttention(null)\` 停止。**不使用 flashFrame**（Tauri v2 JS API 未暴露）。
- toast 内容：\`sendClickableNotification({ title: "slTerminal", body })\`，其中 body 格式为 \`<项目名> · <页签标题> · <事件类别>\`；body 中追加当前时间字符串（如 \`new Date().toLocaleTimeString()\`）。
- 多会话各自独立 toast，不聚合。
- **禁止**在 \`sendNotification\` Options 上写 \`onClick\`。

【P2-FE-06】toast 点击路由：
- 在 \`sendClickableNotification\` 的 \`onClick\` 回调中：
  1. 调用 \`getCurrentWindow().setFocus()\` 聚焦窗口；
  2. 从保存的 panelId 解析 pageId（格式 \`terminal-{pageId}-{seq}\`，按 \`-\` 分割取中间段）；
  3. 遍历 useProjects.getState().projects 找到包含该 pageId 的 projectId；
  4. 调用 \`switchToPage(projectId, pageId)\`（从 App.tsx 或 workspace 透传过来的回调；若无法直接拿到，可改为导出 App 级路由辅助函数）；
  5. 调用 \`window.__dockviewApi?.getPanel(panelId)?.focus()\`；panel 已关闭时静默忽略。

要求：
- 所有系统调用必须经 src/ipc/ 封装。
- 不要阻塞 PTY 或 Dockview 主线程。
- 完成后报告新增/修改文件清单与核心状态流转。`
  }
]

const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 2: 全量测试 ===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:\data\learn\code\slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. npx tsc --noEmit
2. npx eslint src/
3. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
4. npm test
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证 ===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage2 的改动是否实际生效（项目根 D:\data\learn\code\slTerminal）。
先读 docs/hooks-dev/phase2/workflows/verify/stage-02.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
总则：不仅核对字面断言，还须 Read 代码判断实现是否达成断言意图——字面通过但意图未达判 partial 并说明理由。
以下为测试 agent 的全量测试执行结果，测试类断言据此判定（无需重跑）：
---
${testResult ?? '（测试 agent 未返回——测试类断言全部判 not_fixed）'}
---
返回 JSON：{ "allFixed": true/false, "failedItems": ["未通过项ID"], "details": { "<ID>": { "status": "fixed|not_fixed|partial", "evidence": "..." } } }
`, { label: 'verify all items', schema: {
  type: 'object',
  properties: {
    allFixed: { type: 'boolean' },
    failedItems: { type: 'array', items: { type: 'string' } },
    details: { type: 'object' }
  },
  required: ['allFixed', 'failedItems', 'details']
}})

const verifyResult = rawVerify ?? { allFixed: false, failedItems: ['verify-agent-no-return'], details: { 'verify-agent-no-return': { status: 'not_fixed', evidence: 'verify agent 未返回（被跳过或 API 错误）' } } }

return { refactorResults, testResult, verifyResult }
