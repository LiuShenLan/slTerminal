// =====================================================================
// Stage 16 L4：隔离、真实链路与拆分
// =====================================================================
// 清单 docs/text-fix-plan/checklist.md（逐 ID 对照原文）| Stage 划分 docs/text-fix-plan/stages.md
// 断言 docs/text-fix-plan/workflows/verify/stage-16.md（与 fix-loop 同一真值源）
// fix-loop constraints: 本 Stage 只改 e2e-tests/ 下文件（run-wdio.cjs / test.e2e.ts 拆分 spec / helpers.ts / wdio.conf.ts / fixtures README）；不改生产代码；SLTERM_CLAUDE_PROJECTS_DIR 隔离 fixture 契约不可破坏
// 人工验证点 M2/M3：视觉回归基线人工确认；确认未触碰真实 ~/.claude/projects/ 且 ~/.claude/settings.json 已还原
// =====================================================================

export const meta = {
  name: 'stage16-l4-e2e',
  description: 'L4 hooks 隔离备份扩展 + 真实 reporter 链路 + test.e2e.ts 拆分',
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
2. L4 E2E 不得触碰真实 ~/.claude/projects/（SLTERM_CLAUDE_PROJECTS_DIR env 隔离 fixture）——任何用例不得触碰用户真实 ~/.claude/projects/
3. C10 契约不可改：slterm-hook-reporter.js 任何代码路径必须 process.exit(0)——新增 reporter 用例只执行脚本验证行为，不改脚本
4. cargo test 恒 --test-threads=1——本 Stage 不涉，仅作提示
背景：修复要点详见 checklist 对应 ID 条目（先读再动手）。本 Stage 单 agent（WDIO 单实例 + 拆分需全局视野）。只改 e2e-tests/ 下文件。注意：helpers.ts 不在根 tsconfig include——本 Stage 门禁靠 npx tauri build --debug --no-bundle 构建级验证兜底（E2E helper 由 VITE_E2E=1 门控，构建产物必须含 helper）。测试期间 run-wdio.cjs 会备份/还原 ~/.claude/settings.json 与 ~/.slterminal/settings.json——不要手工中断，避免残留。重构阶段只做编译级检查，真实执行由全量测试 agent 单点跑。`

// === Phase 1: 并行重构（单 agent）===
phase('并行重构')
const parallelAgents = [
  {
    label: 'e2e-full',
    prompt: `你负责 E2E-04、E2E-05、E2E-06、E2E-09、E2E-10、E2E-11、E2E-12、E2E-13、E2E-15，触碰文件：e2e-tests/test.e2e.ts（拆分）、拆分新 spec 文件、e2e-tests/helpers.ts、e2e-tests/run-wdio.cjs、e2e-tests/wdio.conf.ts、e2e-tests/fixtures/claude-projects/README.md（新建）。逐 ID 对照 checklist 原文实施：

【E2E-04】L4 真实 WebView2 视觉/功能回归（headless ≠ 生产渲染器）。全屏 TUI 输出后 resize、切页签往返、WebGL→DOM 回退不白屏——**M2 人工验证点**（截图基线人工确认，本 Stage 产出用例并标注人工确认项）。L3 定位声明（网格状态非渲染正确性）由 Stage 17 DOC-02 收编。

【E2E-05】L4 hooks 注入污染 ~/.claude/settings.json 无备份。位置 run-wdio.cjs:11-33（FIX-TE-04 仅备份 ~/.slterminal/settings.json）。按 D5 扩展备份：启动时备份 ~/.claude/settings.json（存在时），exit 还原；同时清理 ~/.slterminal/hooks/ 与 hooks-events/；三启动路径（node22 直跑/便携/fallback）均覆盖。

【E2E-06】L4 新增真实 hook reporter 链路用例。位置 test.e2e.ts（信号文件用例群：1378-1532/1688-1856/3019-3165 现有 Node 侧直接写 .json 绕过脚本）。新增 1 条：真实执行 node ~/.slterminal/hooks/slterm-hook-reporter.js 向 stdin 写 JSON（含 SLTERM_PANEL_ID env），断言信号文件产生且被消费（页签 emoji 变化）；另断言非法 JSON 输入脚本 exit 0（C10 守卫，D7）。

【E2E-09】test.e2e.ts 3236 行拆分 + setup 提取。提取 withProjectAndTerminal({ hooks?: boolean }) 等共享 setup 到 helpers.ts；按领域拆 spec（terminal/sidebar/agent/history/hooks 等），wdio.conf specs 通配覆盖。

【E2E-10】browser.pause(500) 固定等待替换。位置 test.e2e.ts:1669,2012,2018,2184,2190。替换为 browser.waitUntil 轮询具体状态（DOM/store/文件 mtime）。

【E2E-11】拖拽跨区改名 + 恢复编排标注部分端到端。位置 test.e2e.ts:1018-1143、3169-3235。①"拖拽跨区"标题改"侧栏视图跨区移动状态机（R6/R7）"（实际走 store helper）；②恢复编排用例注释标注"部分端到端（断言到 pty.write 命令注入，不含真实进入会话）"。

【E2E-12】L4 Job Object 杀父进程检查子进程残留。新增：spawn 终端（跑持久子进程）→ 强杀 slterminal.exe → 断言子进程无残留（Job Object KILL_ON_JOB_CLOSE 真实验证，PTY-01 的 L4 部分）。

【E2E-13】run-wdio 健壮性 + fixture 维护说明。①Node 22 便携版预置 .temp/node22 或 CI 固定 Node 22 跳过外网下载；②还原前先 rmSync(settingsPath, {force:true}) 再 rename/copy，防残留 bak 致还原失败；③fixtures/claude-projects/ 加 README 说明编码目录名/UUID 与 claude_history 排除规则的同步关系。

【E2E-15】L4 WDIO 无重试机制。位置 wdio.conf.ts。配置 spec/用例级重试（mocha retries 或 specFileRetries），单条 flaky 不拖垮整轮。

完成后报告：每项改动摘要 + 修改文件清单（含拆分后新 spec）。`,
  },
]
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 3: 全量测试（命令相互独立，并行启动执行，收集全部结果）===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:\\data\\learn\\code\\slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. npx tauri build --debug --no-bundle
2. npm run wdio
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。E2E 全量较慢（单实例串行 + 60s 超时），耐心等待勿中止。
`, { label: 'full test suite' })

// === Phase 4: 逐项验证（断言清单唯一真值源：docs/text-fix-plan/workflows/verify/stage-16.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage16 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/text-fix-plan/workflows/verify/stage-16.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
总则：不仅核对字面断言，还须 Read 代码判断实现是否达成断言意图——字面通过但意图未达判 partial 并说明理由。注意 test.e2e.ts 已拆 spec——断言文件路径以拆分后布局为准。
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
