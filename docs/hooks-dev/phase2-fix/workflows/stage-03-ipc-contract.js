// =====================================================================
// Stage 03：IPC 契约对齐（FIX-FE-08 / FIX-FE-09 / FIX-TE-01）
// =====================================================================
// 结构：并行 2 agent（无依赖、无文件重叠）→ 全量测试 → 逐项验证
//
// 跨边界契约（写死，agent 不各自推断，原文见 stages.md C3）：
//   C3 `src/ipc/notification.ts`：
//     sendClickableNotification(title: string, options: { body: string }, onClick: () => void): Notification | null
//     ——成功路径返回 Notification 实例；catch 回退 Tauri sendNotification 路径返回 null
//     export { isPermissionGranted, requestPermission, sendNotification } from "@tauri-apps/plugin-notification";
//     ——ensureNotificationPermission 保留；sendSilentNotification 删除
//
// Agent 分工（文件全集 = prompt 触碰文件）：
//   notification-contract（FIX-FE-08）：src/ipc/notification.ts、
//     src/features/notifications/useClaudeNotifications.ts、src/__tests__/notifications.test.ts
//   hooks-contract（FIX-FE-09 + FIX-TE-01）：src/ipc/hooks.ts、
//     src/__tests__/setup.ts、src/__tests__/ipc-hooks-contract.test.ts
//
// 本 Stage 无特殊纪律（PREAMBLE_EXTRA 为空）——fix-loop 调用本 Stage 时
// args.constraints 无需传值（留空）。
// =====================================================================

export const meta = {
  name: 'stage3-ipc-contract',
  description: 'Stage 03：notification/hooks IPC 契约对齐——签名/返回值/re-export/别名清理/合约用例',
  phases: [
    { title: '并行修复' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
背景：修复要点详见 docs/hooks-dev/phase2-fix/checklist.md 对应 ID 条目（先读再动手）。`

// === Phase 1: 并行修复（agent 间文件零重叠）===
phase('并行修复')
const parallelAgents = [
  {
    label: 'notification-contract',
    prompt: `你负责 FIX-FE-08（sendClickableNotification 契约对齐）。先读 docs/hooks-dev/phase2-fix/checklist.md 的 FIX-FE-08 条目与 docs/hooks-dev/phase2-fix/stages.md 的 C3 契约，再动手。

契约（写死，不各自推断）：
- 签名：sendClickableNotification(title: string, options: { body: string }, onClick: () => void): Notification | null
  ——成功路径返回 Notification 实例；catch 回退 Tauri sendNotification 的路径返回 null。
- 同文件 re-export：export { isPermissionGranted, requestPermission, sendNotification } from "@tauri-apps/plugin-notification";
- 删除 sendSilentNotification（零调用方）；ensureNotificationPermission 保留（有调用方）。

步骤：
1. src/ipc/notification.ts：按上述契约改签名/返回值/re-export/删 sendSilentNotification。
2. src/features/notifications/useClaudeNotifications.ts：调用点（约 :226）第二参数由字符串改为对象 { body: bodyParts }。
3. src/__tests__/notifications.test.ts：同步 mock 与断言——第二参数断言改为对象 { body }；现有「第三参为函数」断言（:691-704 一带）新签名下应继续存活。
4. 自查：grep 全仓 sendSilentNotification 应为零命中（除你刚删除前的文件）。
不跑测试——全量测试由独立 agent 统一执行。`,
  },
  {
    label: 'hooks-contract',
    prompt: `你负责 FIX-FE-09（删 getContextUsage 别名）+ FIX-TE-01（contextUsage 合约四维用例）。先读 docs/hooks-dev/phase2-fix/checklist.md 的 FIX-FE-09 与 FIX-TE-01 条目，再动手。

步骤：
1. src/ipc/hooks.ts：删除 getContextUsage（约 :45-49），保留唯一 wrapper contextUsage。ipc/index.ts 为 namespace barrel，无需改。
2. src/__tests__/setup.ts：全局 mock（约 :98）键名 getContextUsage 改为 contextUsage（改名非删除）。
3. src/__tests__/ipc-hooks-contract.test.ts：追加 contextUsage 合约用例，四维验证——
   ① 命令名 hooks_context_usage；② 参数结构 { transcriptPath: string }；
   ③ 正常返回透传（ContextUsage | null 两种各覆盖）；④ 异常传播（mockIPC throw 不吞）。
   该文件现有 16 用例，追加后应为 20。
4. 自查：grep 全仓 getContextUsage 应为零命中。
不跑测试——全量测试由独立 agent 统一执行。`,
  },
]
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 2: 全量测试（命令相互独立，并行启动执行，收集全部结果）===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:\\data\\learn\\code\\slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. npx tsc --noEmit
2. npx eslint src/
3. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
4. npm test
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证（断言清单唯一真值源：docs/hooks-dev/phase2-fix/workflows/verify/stage-03.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 03 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/hooks-dev/phase2-fix/workflows/verify/stage-03.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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

// agent() 未返回（被跳过/API 错误）时返回 null——必须兜底，否则主 agent 拿到 undefined
const verifyResult = rawVerify ?? { allFixed: false, failedItems: ['verify-agent-no-return'], details: { 'verify-agent-no-return': { status: 'not_fixed', evidence: 'verify agent 未返回（被跳过或 API 错误）' } } }

return { refactorResults, testResult, verifyResult }
