// =====================================================================
// Stage 04：测试补全 + E2E 隔离
//   改动项：FIX-TE-02 / FIX-TE-03 / FIX-TE-04 / FIX-TE-05
// =====================================================================
// 结构：并行 2 agent（无文件重叠）→ 全量测试（含 E2E 实跑门禁）→ 逐项验证
//
// Agent 分工（文件全集 = prompt 触碰文件，无重叠）：
//   l2-tests  ：src/__tests__/colors.test.ts、src/theme/index.ts、
//               src/__tests__/agent-status-view.test.tsx、src/__tests__/diff-panel.test.tsx
//   e2e-tests ：e2e-tests/test.e2e.ts、e2e-tests/run-wdio.cjs
//
// 本 Stage 特殊纪律（PREAMBLE_EXTRA）：
//   1. 只改测试与测试辅助——l2-tests 唯一例外是 src/theme/index.ts 追加一行 re-export
//      （AGENT_STATUS_USAGE_COLORS 聚合导出），除此之外禁止改生产代码；
//   2. E2E 构建必须 npm run build:e2e（含 VITE_E2E=1）——裸 tauri build 会 tree-shake
//      helper，wdio 全部卡"Workspace 未就绪"（config.json 的 e2eBuild 命令不可用）；
//   3. E2E 实跑为本 Stage 门禁，不可跳过。
// fix-loop 调用本 Stage 时 args.constraints 传上述 3 条原文。
// =====================================================================

export const meta = {
  name: 'stage4-tests-e2e',
  description: 'Stage 04：颜色断言/L4 行渲染启用/动态四态/E2E settings 隔离/flaky 加固',
  phases: [
    { title: '并行补测' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
背景：修复要点详见 docs/hooks-dev/phase2-fix/checklist.md 对应 ID 条目（先读再动手）。
【Stage 特殊纪律】只改测试与测试辅助——唯一例外是 src/theme/index.ts 追加一行 re-export，除此之外禁止改生产代码；不跑测试，全量测试由独立 agent 统一执行。`

// === Phase 1: 并行补测（agent 间文件零重叠）===
phase('并行补测')
const parallelAgents = [
  {
    label: 'l2-tests',
    prompt: `你负责 FIX-TE-02（用量条颜色 token 断言）与 FIX-TE-05（diff-panel flaky 加固）。先读 docs/hooks-dev/phase2-fix/checklist.md 的 FIX-TE-02 与 FIX-TE-05 条目，再动手。

【FIX-TE-02】
1. src/theme/index.ts：re-export 列表追加 AGENT_STATUS_USAGE_COLORS（唯一允许的生产代码微改）。
2. src/__tests__/colors.test.ts：新增 describe——AGENT_STATUS_USAGE_COLORS 3 token
   （low / medium / high）合法 hex + 精确值 #629755 / #BBB529 / #F44747，从 ../theme 导入。
3. src/__tests__/agent-status-view.test.tsx：用量条 describe（约 :269-330）补分段颜色断言——
   mock contextUsage 返回使 percent 落 <50 / 50-80 / >80 三档，断言内层 div style.backgroundColor
   等于对应 token 值（usageBarColor：<50 low / ≤80 medium / >80 high）。

【FIX-TE-05】
4. src/__tests__/diff-panel.test.tsx：用例 12（约 :349-363）的 waitFor 增断言
   [data-e2e="diff-left"] .cm-content 存在后再取元素；顺带审计同文件用例 13-15 是否同模式等待不足，
   同标准修复。`,
  },
  {
    label: 'e2e-tests',
    prompt: `你负责 FIX-TE-03（L4 Agent Status 用例启用 + 动态四态）与 FIX-TE-04（E2E settings 隔离 + 侧栏前置重置）。先读 docs/hooks-dev/phase2-fix/checklist.md 的 FIX-TE-03 与 FIX-TE-04 条目，再动手。

【FIX-TE-04】（e2e-tests/run-wdio.cjs）
1. 启动时备份 ~/.slterminal/settings.json：存在则复制为 settings.json.e2e-bak。
2. process.on('exit', ...) 注册同步还原钩子：原文件不存在则删除 E2E 运行产物；存在则移回并清 .e2e-bak。
   exit 钩子天然覆盖本脚本三条 wdio 启动路径（缓存 node22 直跑 / 下载 node22 / fallback npx，
   runWdio 内 execSync + catch 内 process.exit 均触发 exit）。

【FIX-TE-04】（e2e-tests/test.e2e.ts 侧栏两用例前置重置）
3. 拖拽用例（约 :1027-1035 第 3 步）与开关用例（约 :970 附近）：前置经 __slterm_e2e_moveSideViewButton
   逐个将 projects/explorer/commit/agent-status 归位 top 对应序位、bottom 清空，
   再经 __slterm_e2e_toggleSideView 将 open 重置为已知态。不再只重置 open。

【FIX-TE-03】（e2e-tests/test.e2e.ts Agent Status 用例 2，约 :1612-1697 现 it.skip）
4. 拆两个 active 用例：
   a. 静态行渲染：复用现 it.skip 本体（panelId = terminal-{pageId}-0），断言行出现 + 🟡 + 用量条容器。
   b. 动态四态：照 :1357「信号文件驱动页签图标流转」先例，Node 侧原子写信号文件
      （~/.slterminal/hooks-events/，.tmp→.json rename，payload 8 字段：
      panelId/event/timestamp/sessionId/transcriptPath/cwd/toolName/notificationType）驱动
      PreToolUse → ⚡、Stop → ✅、SessionEnd → 行消失，逐态轮询 DOM 断言。panelId 同静态用例。
5. 删除"E2E 环境 hook 事件不可用"失实注释；toast 用例 3（约 :1733）维持 it.skip 不动。`,
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
5. npm run build:e2e
6. npm run wdio
重要纪律：
- 第 5 条必须 npm run build:e2e（含 VITE_E2E=1）——裸 tauri build 会 tree-shake helper，wdio 全部卡"Workspace 未就绪"。
- 第 6 条为本 Stage 实跑门禁，不可跳过；E2E 耗时长（构建 + 20 条用例），耐心等完勿中止。
- 快照比对任务：运行第 6 条前，先对 ~/.slterminal/settings.json 取一次内容快照（不存在则记录"不存在"）；
  wdio 结束后再取一次并 diff——结果（一致 / 不一致 + 差异摘要 / 原本不存在跑后是否存在）写入报告。
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证（断言清单唯一真值源：docs/hooks-dev/phase2-fix/workflows/verify/stage-04.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 04 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/hooks-dev/phase2-fix/workflows/verify/stage-04.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
总则：不仅核对字面断言，还须 Read 代码判断实现是否达成断言意图——字面通过但意图未达判 partial 并说明理由。
以下为测试 agent 的全量测试执行结果（含 wdio 实跑结果与 settings.json 快照比对结论），测试类断言据此判定（无需重跑）：
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
