// =====================================================================
// Stage 02 — toast 改设计·最小
// =====================================================================
// 编排：单 agent 修复（实现+测试一体串行）→ 全量测试 → 逐项验证
//
// 跨边界契约（写死，agent 不各自推断；真值源 = checklist.md「跨边界契约」段）:
//   契约 4 sendToastNotification: sendToastNotification(title: string,
//     options: { body: string }): void——Tauri 原生 sendNotification 通道，
//     无 onClick 参数（点击路由放弃）。ensureNotificationPermission 保留不变。
//
// 决策基线（用户拍板）: toast 仅提示不可点击；点击路由诉求由任务栏闪烁承担
//   （toast 失去点击能力后，闪烁是唯一回窗引导通道，三类事件必须全覆盖）；
//   不做应用内通知列表。
//
// fix-loop args.constraints 应传值（单一出处，勿手写第三份）:
//   本 Stage 特殊纪律：单 agent 同时改生产代码与测试（3 文件强耦合串行），
//   可跑 npm test 自验（无并行冲突）。
//
// 人工验证点（不属本脚本自动化范围，收尾前必须完成）: Win11 真实环境 Tauri
//   原生 sendNotification banner 可见性实测——真实 claude 触发权限请求（或
//   Stop），Alt+Tab 失焦观察；不弹则 toast 退化为通知中心条目+任务栏闪烁主职，
//   接受（决策基线 1），结果写入 Stage 05 文档。
// =====================================================================

export const meta = {
  name: 'stage-02-toast',
  description: 'Stage 02 toast 改设计·最小——Tauri 原生 sendNotification + 去路由化 + 三类均闪烁',
  phases: [
    { title: '修复实现' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
背景：修复要点先读 docs/hooks-dev/phase2-fix/checklist.md 对应 ID 条目 + 「跨边界契约」段（契约取值唯一真值源，禁止各自推断）。
本 Stage 特殊纪律：单 agent 同时改生产代码与测试（3 文件强耦合串行），可跑 npm test 自验（无并行冲突）。`

// === Phase 1: 修复实现（A：toast 全链 3 文件）===
phase('修复实现')
const fixResult = await agent(`${PREAMBLE}

你负责 PF2-FE-08、PF2-FE-09、PF2-FE-10、PF2-TE-04（toast 全链改设计·最小）：

【PF2-FE-08】src/ipc/notification.ts：sendClickableNotification → sendToastNotification
- 位置：:40-62（new Notification 主路径 + onclick 绑定 + catch 回退 Tauri sendNotification）
- 按契约 4：替换为 sendToastNotification(title, { body })——主路径 Tauri 原生 sendNotification；删 Web Notification 路径与 onclick 参数
- ensureNotificationPermission（:20-26）保留不变
- 失败 catch 补 console.error，不静默（DBG-7 教训）
- :33 的「委托 OS 原生通知中心」注释替换为 AUMID 平台限制结论：未打包 Win32 WebView2 无 AUMID——banner 抑制 + onclick 不路由 + shim 无 close + 构造不抛（catch 回退永不触发）；探针实测证据 {"created":true,"permission":"granted","thrown":"TypeError: n.close is not a function"}

【PF2-FE-09】src/features/notifications/useClaudeNotifications.ts：去路由化
- 删 routeToPanel（:106-110）、findPanelTitle（:89-101）、toast onClick 绑定（:193-198）
- 相关 import 清理（setFocus/switchToPageAndFocus/getPageApi 等如无其他使用一并删除）
- 三类事件（permission/done/error，即 classifyEvent 全分类）均触发任务栏闪烁（现状仅 permission 闪 :188-190）——决策：toast 失去点击能力后，任务栏闪烁是唯一回窗引导通道
- 失焦门控（:148）、60s 去重（:155-163）、classifyEvent 三分类保留不变
- toast body 不再含面板标题查找（项目名 + 事件类文案即可）

【PF2-FE-10】flashTaskbar 静默 catch 补可观测性
- useClaudeNotifications.ts:119-121（requestUserAttention 的 .catch 静默吞错）→ catch 内补 console.error（DBG-7 教训）

【PF2-TE-04】src/__tests__/notifications.test.ts 重写（808 行 31 用例）
- mock ../ipc/notification 的 sendClickableNotification → sendToastNotification（两参数无 onClick）
- 删「toast onClick 路由」describe 整块（:536-714，6 用例——路由功能移除）
- 「任务栏闪烁细分」（:720-808）反转：Stop/StopFailure/PostToolUseFailure 由「不触发 requestUserAttention」改为触发（三类均闪烁）
- 保留：失焦门控、60s 去重、classifyEvent 过滤、正文文案断言（去面板标题后的新文案）
- 完成判据：npx tsc --noEmit 通过 + npm test 全绿（你单 agent 串行，可跑 npm test 自验）。
`, { label: 'A:toast-redesign' })

// === Phase 2: 全量测试（命令相互独立，并行启动执行，收集全部结果）===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:/data/learn/code/slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. npx tsc --noEmit
2. npx eslint src/
3. npm test
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证（断言清单唯一真值源：verify/stage-02.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 02 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/hooks-dev/phase2-fix/workflows/verify/stage-02.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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

return { fixResult, testResult, verifyResult }
