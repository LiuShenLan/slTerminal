// =====================================================================
// Stage 07：错误处理一致性 + nonce 威胁模型（SEC-04、FE-08、FE-10、FE-42、FE-43、FE-44、FE-45）
// 编排：并行 3（文件零重叠：A=HtmlPanel/panels 文档/command-catalog 测试；B=keyboard/window/stores×4；C=DiffPanel/useCodeMirror）
// verify 真值源：docs/review-phase2-fix/workflows/verify/stage-07.md
// =====================================================================

export const meta = {
  name: 'stage07-error-handling',
  description: 'S07 静默 catch 可观测化 + getErrorMessage 统一 + nonce 威胁模型守卫（SEC-04、FE-08/10/42/43/44/45）',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入。
背景：修复要点详见 \`docs/review-phase2-fix/checklist.md\` 对应 ID 条目（先读再动手——条目内含精确 file:line 与可照抄的代码块）。
测试纪律：本阶段禁止跑 npm test / npm run test:l3（全量测试 agent 单点跑）；编译级自查用 \`npx tsc --noEmit\`。`

// === Phase 1: 并行重构（agent 间文件零重叠）===
phase('并行重构')
const parallelAgents = [
  {
    label: 'A-nonce',
    prompt: `你负责【SEC-04】nonce 威胁模型登记 + global 命令集守卫（D16 决策）。先读 \`docs/review-phase2-fix/checklist.md\` 第 7 节 SEC-04 条目（含威胁模型注释原文与守卫测试代码块）。

触碰文件：\`src/panels/html/HtmlPanel.tsx\`（仅注释）、\`src/panels/CLAUDE.md\`、\`src/__tests__/command-catalog.test.ts\`

步骤：
1. \`src/panels/html/HtmlPanel.tsx\` 的 \`buildInjectedScript\` 文档注释（约 :56-64）追加 checklist 条目中的威胁模型段（nonce 明文可读、真正防线 = global 命令集最小化）
2. 同文件 :132-134 注释修正：删「拿不到本面板 nonce，伪造消息在此被拦截」失实表述，按 checklist 条目改写
3. \`src/panels/CLAUDE.md\`「postMessage origin 校验与威胁模型（SEC-03/SEC-04）」节：修正失实描述（「攻击者 HTML 无法读取 buildInjectedScript 产出」→ 明文可读的准确表述）+ 补「防线分层：nonce（外部）+ global 命令集最小化（内部，守卫测试锁死）」结论
4. \`src/__tests__/command-catalog.test.ts\` 追加 checklist 条目中的守卫用例（global context 命令集恒为 [global.closeTab]）
5. \`npx tsc --noEmit\` 通过

纪律：本项只改注释/文档/测试——禁止改 HtmlPanel 任何运行时代码。
完成后报告：三处改动摘要。`,
  },
  {
    label: 'B-silent-catch',
    prompt: `你负责【FE-08】终端粘贴 readText 失败可观测 +【FE-42】关窗监听 cleanup 静默吞错 +【FE-45】stores loadFromDisk catch{} 补日志（5 处）。先读 \`docs/review-phase2-fix/checklist.md\` 第 7 节 FE-08/FE-42/FE-45 条目（各含可照抄代码块）。

触碰文件：\`src/panels/terminal/keyboard.ts\`、\`src/ipc/window.ts\`、\`src/stores/fontSize.ts\`、\`src/stores/keybindings.ts\`、\`src/stores/sideBar.ts\`、\`src/stores/projects.ts\`（仅 2 处 catch 行）、keyboard/startup 相关测试文件

步骤：
1. 【FE-08】\`src/panels/terminal/keyboard.ts:35-38\` 粘贴 \`.catch(() => {});\` 改为 checklist 条目中的 console.error 版本（照同文件 :25-27 copy 分支先例）
2. 【FE-42】\`src/ipc/window.ts:55-58\` cleanup \`.catch(() => {});\` 改为 console.warn 版本；:56 注释「兜底吞掉」改「兜底记录」
3. 【FE-45】5 处 \`} catch {\` 空块统一按 checklist 条目补 console.warn（保留原注释）：\`src/stores/fontSize.ts:64\`（store名=fontSize）、\`src/stores/keybindings.ts:71\`（keybindings）、\`src/stores/sideBar.ts:122\`（sideBar）、\`src/stores/projects.ts:254\`（projects）、\`src/stores/projects.ts:275\`（loadAllProjects）——注意 projects.ts 只改这 2 处 catch 行，不动其他任何代码
4. 测试：\`keyboard.test.ts\` 粘贴失败路径断言 console.error（无既有用例则补一条 mock readText reject）；\`startup-store-fail-warn.test.tsx\` 核对告警路径既有用例，必要时补 console.warn 断言
5. \`npx tsc --noEmit\` 通过

完成后报告：7 处改动清单 + 测试适配摘要。`,
  },
  {
    label: 'C-diff-editor',
    prompt: `你负责【FE-10】Diff 右栏外部修改重载失败提示条 +【FE-43】DiffPanel 保存失败 toast 统一 getErrorMessage +【FE-44】编辑器保存失败 toast 统一 getErrorMessage。先读 \`docs/review-phase2-fix/checklist.md\` 第 7 节 FE-10/FE-43/FE-44 条目（各含可照抄代码块）。

触碰文件：\`src/panels/diff/DiffPanel.tsx\`、\`src/panels/editor/useCodeMirror.ts\`、\`src/__tests__/diff-panel-stale-banner.test.tsx\`、\`src/__tests__/diff-panel.test.tsx\`、\`src/__tests__/use-code-mirror.test.ts\`

步骤：
1. 【FE-10】\`src/panels/diff/DiffPanel.tsx:478-491\` 两处外部修改重载 catch（约 :483、:490）各补 \`setDiffStale(true);\`（保留 console.warn，照 checklist 条目代码块——复用 :655-677 已有 diffStale 提示条）
2. 【FE-43】\`DiffPanel.tsx:372-376\` 保存失败 toast 由直接拼接 err 改为走 \`getErrorMessage(err)\`（:50 已 import，照 checklist 条目改法）
3. 【FE-44】\`src/panels/editor/useCodeMirror.ts:178-181\` 同法改（:33 已 import getErrorMessage）
4. 测试：\`diff-panel-stale-banner.test.tsx\` 增用例——外部修改重载 readFile reject → \`data-testid="diff-stale-banner"\` 出现；\`diff-panel.test.tsx\`/\`use-code-mirror.test.ts\` 保存失败用例断言文案为解析后消息
5. \`npx tsc --noEmit\` 通过

完成后报告：三处改动摘要 + 测试适配摘要。`,
  },
]
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 2: 全量测试 ===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:/data/learn/code/slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. npx tsc --noEmit
2. npx eslint src/
3. npm test
4. npm run test:l3
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证 ===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 7 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/review-phase2-fix/workflows/verify/stage-07.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
