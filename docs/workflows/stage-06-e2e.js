// =====================================================================
// Stage 06: E2E 用例（CV-TE-01/02）
// =====================================================================
// 契约（写死）：
//   - 选择器一律 data-e2e（commit-view / commit-changes / commit-unversioned / commit-file-item / diff-left / diff-right）
//   - helpers 复用 __slterm_e2e_toggleSideView / __slterm_e2e_createProject / __dockviewApi，无需新前端 helper
//   - git 仓库搭建在 Node spec 侧（child_process），不在 WebView2 内
// 门禁说明：e2e-tests/ 不在根 tsconfig include——构建级门禁由 npm run e2e（build:e2e）承担
// fix-loop 调用时 args.constraints 传 "本 Stage 只改 e2e-tests/ 下文件，禁止改 src/ 与 src-tauri/ 生产代码"
// =====================================================================

export const meta = {
  name: 'stage06-e2e',
  description: 'Stage 06: commit 视图 E2E——列表渲染 + diff 页签打开 2 用例（CV-TE-01/02）',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
背景：修复要点详见 docs/commit-view/checklist.md 对应 ID 条目（先读再动手）。
本 Stage 只改 e2e-tests/ 下文件，禁止改 src/ 与 src-tauri/ 生产代码。
不自行跑 npm run e2e（耗时且由全量测试 agent 单点跑）；你只写用例代码。

契约（写死，不得偏离）：
- 选择器一律 data-e2e 属性，禁止 CSS 内联样式选择器
- 复用 helpers：__slterm_e2e_createProject(path)（async）、__slterm_e2e_toggleSideView(id)、__slterm_e2e_getSideBarState()、__dockviewApi
- git 仓库搭建在 Node spec 侧（child_process.execSync），用例间 tempdir 隔离 + after 清理`

// === Phase 1: 并行重构 ===
phase('并行重构')
const parallelAgents = [
  {
    label: "e2e-commit",
    prompt: `你负责 CV-TE-01 / CV-TE-02：

【CV-TE-02】git 仓库脚手架（先做，用例依赖）
- 新建 e2e-tests/gitScaffold.ts（Node 侧 util）
- export function makeGitRepo(scenario: { modified?: string[]; untracked?: string[] }): string
  - tempdir（fs.mkdtempSync + os.tmpdir）→ git init → git config user.email/name → 写基线文件 → git add + commit → 按 scenario 修改 tracked 文件 / 新建 untracked 文件 → 返回仓库路径
- export function cleanupGitRepo(path: string)：递归删除
- execSync 调用 git CLI（CI runner 有 git）

【CV-TE-01】E2E 用例（e2e-tests/test.e2e.ts 追加新 describe "commit 视图"）
- 用例 1（列表渲染）：makeGitRepo({ modified: ["a.txt"], untracked: ["new.txt"] }) → browser.execute 调 __slterm_e2e_createProject(repoPath)（await）→ __slterm_e2e_toggleSideView("commit") → 等待 [data-e2e="commit-changes"] 存在 → 断言 commit-changes 下 [data-e2e="commit-file-item"] 文本含 "a.txt"；[data-e2e="commit-unversioned"] 下含 "new.txt"
- 用例 2（diff 页签打开）：用例 1 基础上，双击（element.dblClick() 或 browser.execute dispatch dblclick）文本为 a.txt 的列表项 → 等待后 browser.execute 读 __dockviewApi.panels（或 toJSON）断言存在 title 含 "(git diff)" 的面板；断言页面存在 [data-e2e="diff-left"] 与 [data-e2e="diff-right"]
- after 钩子 cleanupGitRepo
- 照现有 test.e2e.ts 用例风格（waitUntil workspaceReady 等模式先读文件对齐）`,
  },
];
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 2: 全量测试 ===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:/data/learn/code/slTerminal 执行全量验证。命令按序执行（e2e 耗时长，放最后）：
1. npm test
2. npm run e2e
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。
注意：npm run e2e = build:e2e（VITE_E2E=1 tauri build --debug --no-bundle）+ wdio，总耗时可达 10 分钟以上，勿中止。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证 ===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 06 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/workflows/verify/stage-06.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
