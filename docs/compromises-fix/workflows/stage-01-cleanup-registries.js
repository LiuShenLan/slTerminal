// =====================================================================
// Stage 01 低风险清理与登记硬化
// CP-025 删 sidebar / CP-026+021 navTree 迁移 / CP-038+027 启动色生成物 /
// CP-032+CP-001 登记硬化（A4 串行尾位，消化 adr.md 与条件 package.json 冲突）
// fix-loop constraints：本 Stage 无特殊纪律（省略 args.constraints）
// =====================================================================

export const meta = {
  name: 'stage-01-cleanup-registries',
  description: 'S01 低风险清理与登记硬化（CP-001/025/026/021/027/038/032）',
  phases: [
    { title: '并行修复' },
    { title: '串行尾位' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入。
背景：修复步骤六段式真值源 = docs/compromises-fix/checklist.md 对应 CP 条目（先读再动手，照抄步骤不另起方向）；大代码块按条目内「照抄 review-NN 步骤 X.Y」指针读 docs/compromises-fix/ 下对应 review 文件。`

// === Phase 1: 并行修复（agent 间文件零重叠；不跑共享资源测试，只做编译级自查）===
phase('并行修复')
const parallelAgents = [
  {
    label: 'cp025-delete-sidebar',
    prompt: `你负责 CP-025（退役 sidebar 目录物理删除）：
1. git rm -r src/features/sidebar（仅含 CLAUDE.md 一个文件）。
2. 修 src/__tests__/agent-history-restore.test.ts:4 注释为 checklist 给定新文（mock 本体早已指向 navTree，纯注释修正）。
3. 删 .claude/skills/systematic-changes-plan/config.json 的 "src/features/sidebar/CLAUDE.md", 行（claudeMdFiles 数组内，约 :52）。
4. 自查：grep -rn "features/sidebar" src/ .claude/skills/ 应零命中；npx vitest run agent-history-restore 全绿。
完成后报告修改文件清单。`,
  },
  {
    label: 'cp026-021-navtree',
    prompt: `你负责 CP-026+CP-021（useAgentStatus 迁入 navTree + 返回面收窄 + 60s ticker 移交宿主，单 agent 合并体）：
照抄 checklist.md CP-026+CP-021 条目步骤 1-7：
1. git mv src/features/agentStatus/useAgentStatus.ts src/features/navTree/useAgentStatus.ts。
2. 返回面收窄为 AgentSessionRow[]——全文照抄 review-06 CP-026 步骤 3.2（约 390 行，含 CP-026 头注）。
3-4. 改接线 useNavTree.ts:24-25/:99 与 NavSessionRow.tsx:14。
5. ticker 移交 NavTree.tsx 宿主（:193 后插入 CP-021 ticker 块；renderHistory 渲染 NavHistoryRow 处传 now={now}）。
6. NavHistoryRow.tsx props 增必填 now: number，:38 改用 prop now。
7. 删 src/features/agentStatus/CLAUDE.md，有效约束随迁 navTree/CLAUDE.md（照抄 review-06 步骤 5）；agentHistory/CLAUDE.md:89 MC-318 改写。
测试适配四文件逐一按条目点名执行：agent-status-hook.test.ts、nav-tree.test.tsx（mock 形状收窄 10 处）、nav-tree-history.test.tsx（resetAll 收窄为 mockReturnValue([])——以收窄后真实签名校准；新增 fake timers 宿主 ticker 用例组）、nav-history-row.test.tsx。
knip.json 删 :22-24 agentStatus types 豁免条目。
自查：npx tsc --noEmit；npx vitest run agent-status-hook nav-tree nav-history-row 全绿；条目「验证」段全部 grep 断言满足。
完成后报告修改文件清单。`,
  },
  {
    label: 'cp038-027-startup-colors',
    prompt: `你负责 CP-038+CP-027（pin 改回 ^14.2.0 + 启动色构建期注入，单 agent 串行改 package.json）：
先 CP-038：package.json:67 改 "@types/markdown-it": "^14.2.0"，npm install 刷新 lockfile；若 tsc 报相关新错误走异常分支（ADR-0006 例外登记，照 checklist 口径），无错误则不登记。
后 CP-027（照抄 checklist 步骤 1-5）：
1. 新建 src/theme/startupColors.ts（生成物，头注勿手改，内容照 checklist 代码块）。
2. 新建 scripts/sync-startup-colors.mjs（完整全文照抄 review-06 CP-027 步骤 3.2；零依赖纯 Node ESM）。
3. package.json scripts 段追加 sync:startup-colors / predev / prebuild 三行。
4. src/main.tsx 三处字面量替换为常量 + import + :30 注释改口径。
5. linear.ts 删 :6-11 交叉引用块；src/theme/CLAUDE.md :54/:62 改写。
注意：.claude/adr.md 的 ADR-0002/0003 交叉引用清除不由你执行（移交串行尾位 agent，防同文件冲突）——完成后在报告中显式列出「待尾位 agent 执行的 adr.md 改动原文」。
测试：新建 src/__tests__/startup-colors-sync.test.ts（照抄 review-06 CP-027 步骤 4）。
自查：npm run sync:startup-colors 退出码 0 且输出「无改动」；npx vitest run startup-colors-sync 全绿；npx tsc --noEmit 绿；markdown 系六件回归 npx vitest run src/__tests__/markdown- 全绿。
完成后报告修改文件清单 + 移交尾位的 adr.md 改动原文。`,
  },
]
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 2: 串行尾位（adr.md 三处汇合 + CP-032 条件升级分支，须在 A3 完成后）===
phase('串行尾位')
const tailResult = await agent(`${PREAMBLE}

你负责 CP-032 + CP-001 + 上游 agent 移交的 adr.md 改动（串行尾位）：
1. CP-032：e2e-tests/CLAUDE.md 在「### E2E helper 命名与挂载位置」节之前插入「### wdio 版本矩阵与 overrides 对齐契约（CP-032）」节（内容照抄 review-01 CP-032 步骤 3.1，七 override 成因表 a027b17/1233336 全列）；当场执行 npm view @wdio/tauri-service@latest version dependencies --json 评估——latest > 1.3.0 且硬钉已放开才走升级分支（升 tauri-service、删 overrides 两条、npm install、npm ls 断言单实例、npm run e2e 全量）；latest = 1.3.0 维持不动。.claude/adr.md:153 后追加 CP-032 登记句。
2. CP-001：新建 scripts/check-ts7-trigger.mjs（完整全文照抄 review-01 CP-001 步骤 3.1；退出码 0=达成/1=未达成/2=查询失败）；.claude/adr.md ADR-0010 TE-07 段末句替换为机检口径（照抄 review-01）；docs/compromises.md CP-001 条目「修改方向」句替换为机检脚本口径（照抄 review-01）。新建 src/__tests__/deps-ts7-trigger.test.ts 五用例。不注册 npm script。
3. 上游移交：cp038-027 agent 报告中的 adr.md ADR-0002/0003 改动原文，落到 .claude/adr.md（其报告附在下方，若为 null 则自行按 checklist CP-027 步骤 5 执行 ADR-0002 :58/:63 与 ADR-0003 :80 三处改写）。
---
上游报告：${refactorResults[2] ?? '（未返回，按 checklist 自行执行 adr.md 三处）'}
---
自查：node scripts/check-ts7-trigger.mjs 退出码 ∈ 0/1/2（当前预期 1=未达成）；npx vitest run deps-ts7-trigger 全绿；grep -n "对齐契约" e2e-tests/CLAUDE.md 命中。
完成后报告修改文件清单（含 CP-032 走了哪个分支）。`, { label: 'cp032-001-tail' })

// === Phase 3: 全量测试（命令相互独立，并行启动执行，收集全部结果）===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:/data/learn/code/slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. npx tsc --noEmit
2. npx eslint src/
3. npm test
4. npx knip --production
5. npx tauri build --debug --no-bundle
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 4: 逐项验证（断言清单唯一真值源：verify/stage-01.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 01 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/compromises-fix/workflows/verify/stage-01.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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

return { refactorResults, tailResult, testResult, verifyResult }
