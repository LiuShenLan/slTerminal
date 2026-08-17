// =====================================================================
// stage-11-frontend-security.js — S11 前端安全（SEC-04/06/10）
// =====================================================================
// 跨边界契约：无（纯前端 Stage；SEC-06 为 D6 决策的守卫测试，权限本身不动）。
// fix-loop 调用约定：args.testCommands 省略（默认基础五条）；constraints 传空。
// =====================================================================

export const meta = {
  name: 'stage11-frontend-security',
  description: 'Stage 11: HTML 预览 postMessage nonce 校验 + fail-safe 页去 innerHTML + 剪贴板权限消费点守卫测试（SEC-04/06/10）',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
背景：修复要点详见 docs/review-fix/checklist.md 对应 ID 条目 + docs/review-fix/stages.md S11 节（先读再动手）。
测试纪律：本阶段禁止跑资源共享型全量测试（cargo test / npm test / npm run test:l3）——只做编译级检查（cargo check / npx tsc --noEmit），全量执行由独立测试 agent 单点跑（跨进程并发会死锁，cargo 排队属正常）。`

// === Phase 1: 并行重构（agent 间文件零重叠；不跑资源共享型测试）===
phase('并行重构')
const parallelAgents = [
  {
    label: 'frontend-security',
    prompt: `你负责 SEC-04、SEC-06、SEC-10，只许改 src/panels/html/HtmlPanel.tsx、src/main.tsx、新建 src/__tests__/ 下测试文件：
【SEC-04】HtmlPanel.tsx（:96-125）iframe 内任意脚本可伪造 slterm_key postMessage 触发全局快捷键。修复：面板挂载时用 crypto.getRandomValues 生成随机 nonce，经注入脚本传入 iframe（注入脚本随文档内联，nonce 拼入）；父窗口 message 监听校验消息 nonce，不符静默丢弃。补 L2 测试：伪造消息（无 nonce/错误 nonce）不触发快捷键，合法 nonce 触发。
【SEC-10】main.tsx:28-31 fail-safe 页用 innerHTML 拼接（msg 为变量）。改 createElement + textContent + style 赋值（视觉效果保持一致）。
【SEC-06】按 D6 补契约守卫测试（grep 级 L2）：断言 clipboard-manager 的 readText 仅出现于 src/ipc/clipboard.ts、src/panels/terminal/keyboard.ts 及测试文件（测试实现：fs 遍历 src/ 读文件 grep，路径集合硬编码断言）。权限本身不动。
只做 npx tsc --noEmit 编译级检查，禁止 npm test。`
  },
]
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 2: 全量测试（命令相互独立，并行启动执行，收集全部结果）===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:/data/learn/code/slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. npx tsc --noEmit
2. npx eslint src/
3. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
4. npm test
5. cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证（断言清单唯一真值源）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 11 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/review-fix/workflows/verify/stage-11.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
