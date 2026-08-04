// =====================================================================
// Stage 13 L2-ipc/html：盲区收口与参数化
// =====================================================================
// 清单 docs/text-fix-plan/checklist.md（逐 ID 对照原文）| Stage 划分 docs/text-fix-plan/stages.md
// 断言 docs/text-fix-plan/workflows/verify/stage-13.md（与 fix-loop 同一真值源）
// fix-loop constraints: 本 Stage 只改测试与 src/ipc/CLAUDE.md 文档（mockIPC 盲区说明）；若发现生产代码缺陷，报告主 agent 后另行处理
// cross-boundary 契约（写死，勿推断）：
//   - ipc-ping 必须改调 src/ipc/index.ts 导出的 ping()（barrel 出口），不得裸 invoke
//   - 四契约文件（ipc-contract/ipc-hooks-contract/ipc-hooks-config-contract/ipc-claude-history-contract）统一走新工厂 helpers/ipc-contract.ts，四维断言（命令名/参数/返回/异常）不丢
//   - E2E_ENABLED 必须保持内联 import.meta.env 字面量表达式（DCE 前提），不得包函数调用
// =====================================================================

export const meta = {
  name: 'stage13-l2-ipc-html',
  description: 'L2 ipc mockIPC 盲区收口 + notification/postMessage 负面用例',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：
1. compute_conpty_flags 固定 0x7（含 4 条守卫测试），任何 agent 不得修改 ConPTY flags——本 Stage 不涉，仅作提示
2. L4 E2E 不得触碰真实 ~/.claude/projects/——本 Stage 不涉，仅作提示
3. C10 契约不可改：slterm-hook-reporter.js 任何代码路径必须 process.exit(0)——本 Stage 不涉，仅作提示
4. cargo test 恒 --test-threads=1——本 Stage 不涉，仅作提示
背景：修复要点详见 checklist 对应 ID 条目（先读再动手）。本 Stage 只改测试与 src/ipc/CLAUDE.md（mockIPC 盲区文档化）。并行 agent 文件零重叠（ih-ipc 碰 ipc-*.test.ts + notification.test.ts（新建）+ helpers/ipc-contract.ts（新建）+ e2e-build-config/csp-config 测试 + src/ipc/CLAUDE.md；ih-html 碰 html-panel/error-boundary 测试）。重构阶段只做编译级检查（npx tsc --noEmit），真实执行由全量测试 agent 单点跑。`

// === Phase 1: 并行重构（2 agent，文件零重叠）===
phase('并行重构')
const parallelAgents = [
  {
    label: 'ih-ipc',
    prompt: `你负责 IHE-01、IHE-02、IHE-04、IHE-06、IHE-07①④，触碰文件：src/__tests__/ipc-*.test.ts（contract/hooks-contract/hooks-config-contract/claude-history-contract）、notification.test.ts（新建）、src/__tests__/helpers/ipc-contract.ts（新建）、e2e-build-config.test.ts、csp-config.test.ts、src/ipc/CLAUDE.md。逐 ID 对照 checklist 原文实施：

【IHE-01】mockIPC 结构性盲区——文档化 + wrapper 行为契约。位置 ipc-contract.test.ts、ipc-hooks-contract.test.ts、ipc-claude-history-contract.test.ts。mockIPC 只守 JS 侧形状——camelCase/snake_case 真实转换、Channel 序列化、Uint8Array 处理、listen 回调运行时解包均不在 mock 层验证。①文件头注释 + ipc/CLAUDE.md 文档化"契约测试只防 wrapper 写错命令名/参数结构，真实序列化由 L4 守卫"；②补"wrapper 行为契约"用例（listen 回调解包 event.payload 的模拟驱动断言——onFsEvent/onHookEvent 回调收到解包后 payload）。

【IHE-02】notification.ts 14.3% 分支未测。位置 src/ipc/notification.ts:38-47。新建 notification.test.ts：mock @tauri-apps/plugin-notification 拒绝/异常，验证 sendToastNotification catch 静默、ensureNotificationPermission 拒绝路径。

【IHE-04】E2E_ENABLED tree-shake 字面量断言。位置 src/lib/e2eEnabled.ts、e2e-build-config.test.ts。补 AST/正则断言 E2E_ENABLED 定义为内联 import.meta.env 字面量表达式（不得调用 computeE2eEnabled——函数调用阻碍 DCE）。

【IHE-06】四 IPC 契约文件参数化。抽 src/__tests__/helpers/ipc-contract.ts 工厂（声明式 schema：命令名/参数/返回/异常），四文件重走工厂；四维断言不丢。

【IHE-07①④】①ipc-ping 改调 src/ipc/index.ts 导出的 ping() wrapper（非裸 invoke）；④CSP 测试扩展 style-src/connect-src/img-src 关键字段快照。

完成后报告：每项改动摘要 + 修改文件清单。`,
  },
  {
    label: 'ih-html',
    prompt: `你负责 IHE-03、IHE-05、IHE-07②③、IHE-08，触碰文件：src/__tests__/html-panel.test.tsx、src/__tests__/error-boundary.test.tsx。逐 ID 对照 checklist 原文实施：

【IHE-03】HTML postMessage 负面用例缺失 + jsdom 局限标注。位置 src/panels/html/HtmlPanel.tsx:117-156、html-panel.test.tsx。补负面用例（origin ≠ "null"、source ≠ contentWindow、type ≠ "slterm_key"、未知 fingerprint 均不 dispatch）；用例标注"jsdom 模拟，真实 WebView2 由 L4 验收"。

【IHE-05】error-boundary inline variant 未覆盖。位置 src/lib/ErrorBoundary.tsx:51-62。补 variant="inline" 渲染用例。

【IHE-07②③】②注入脚本断言关键控制流（postMessage 字段构造/preventDefault/监听绑定），非仅字符串包含；③复跑确认 HtmlPanel err instanceof Error false 分支命中（html-panel.test.tsx E12/E13），未中则修用例。

【IHE-08】html-panel waitFor helper 提取。提取 waitForLoaded/waitForError 局部 helper 消除重复。

完成后报告：每项改动摘要 + 修改文件清单。`,
  },
]
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 3: 全量测试（命令相互独立，并行启动执行，收集全部结果）===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:\\data\\learn\\code\\slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. npx tsc --noEmit
2. npx eslint src/
3. npm test
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。
`, { label: 'full test suite' })

// === Phase 4: 逐项验证（断言清单唯一真值源：docs/text-fix-plan/workflows/verify/stage-13.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage13 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/text-fix-plan/workflows/verify/stage-13.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
