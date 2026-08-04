// =====================================================================
// Stage 15 L3：生产配置覆盖与断言精确化
// =====================================================================
// 清单 docs/text-fix-plan/checklist.md（逐 ID 对照原文）| Stage 划分 docs/text-fix-plan/stages.md
// 断言 docs/text-fix-plan/workflows/verify/stage-15.md（与 fix-loop 同一真值源）
// fix-loop constraints: 本 Stage 只改 L3 测试（test/terminal/ 下），不改生产代码；L3 定位 = 网格状态正确性，非渲染正确性（渲染由 L4/人工验收）
// =====================================================================

export const meta = {
  name: 'stage15-l3',
  description: 'L3 keyboard 降级标注 + 生产 theme/OSC 覆盖 + 断言精确化',
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
背景：修复要点详见 checklist 对应 ID 条目（先读再动手）。本 Stage 单 agent（L3 同配置同目录串行）；只改 test/terminal/ 下测试，不改生产代码。L3 定位 = 网格状态正确性（headless 不跑 WebGL/GPU）。重构阶段只做编译级检查（npx tsc --noEmit），真实执行由全量测试 agent 单点跑。`

// === Phase 1: 并行重构（单 agent）===
phase('并行重构')
const parallelAgents = [
  {
    label: 'l3-terminal',
    prompt: `你负责 E2E-01、E2E-02、E2E-03、E2E-07、E2E-08、E2E-14，触碰文件：test/terminal/keyboard.test.ts、terminal-serialize.test.ts、ansi-correctness.test.ts、osc.test.ts、新增 production 配置/OSC/negative 测试文件。逐 ID 对照 checklist 原文实施：

【E2E-01】L3 keyboard ~30 条同义反复降级标注。位置 keyboard.test.ts:72-317。term.input('\x01') → 断言 onData 收到 \x01，等价"输入=输出"（一手证据：xterm input() = triggerDataEvent 纯透传，node_modules/@xterm/xterm/src/common/CoreTerminal.ts:183-185），不经生产 attachCustomKeyEventHandler → ShortcutRegistry 链路。按 D4 文件头 + describe 标注降级为"xterm.js 基础行为回归（非 slTerminal 键盘链路）"，用例保留。

【E2E-02】L3 未覆盖生产 theme.ts。位置 src/panels/terminal/theme.ts（colors/cursorStyle/scrollback/vtExtensions/drawBoldTextInBrightColors）。新增用生产 terminalOptions 创建 headless Terminal 的用例：16 色 ANSI 与主题色板一致、CSI>1u 可激活 Kitty、scrollback 容量生效、drawBoldTextInBrightColors 亮色映射。

【E2E-03】L3 未覆盖生产 OSC 52/133/8 handler。位置 src/panels/terminal/useClipboardHandler.ts、useCommandDetection.ts、useXterm.ts（OSC 8）。headless 触发：①\x1b]52;c;<base64>\x07 → mock src/ipc/clipboard writeText 断言调用 + CJK 解码；②\x1b]133;C;<cmd>\x1b\\ → onTabStateChange 参数（icon/title）；③\x1b]8;;<url>\x1b\\ → mock src/ipc/shell openUrl。

【E2E-07】L3 断言粒度过粗（文本存在 → 行列精确）。位置 terminal-serialize.test.ts:83-95,146-174,188-222,297-306。CUP/reflow/SGR 用例改 term.buffer.active.getLine(y).translateToString() 按行断言 + getCell(x,y).getFgColorMode() 单元格属性断言。

【E2E-08】256 色用例名实不符修正。位置 ansi-correctness.test.ts:70-81。按 D3 对齐实现：一手证据 node_modules/@xterm/addon-serialize/src/SerializeAddon.ts:259-262——palette 0-15 优化为基本 SGR（30+(c&7) / 90+(c&7)），补断言 \x1b[30m/\x1b[97m 等优化后序列 + 删除/修正误导注释。

【E2E-14】L3 反向/异常 ANSI 用例缺失。补非法 ANSI、截断多字节序列、嵌套 OSC、异常 resize（0×0）等负面用例（headless 不崩溃 + 状态可恢复）。

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
2. npm run test:l3
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。
`, { label: 'full test suite' })

// === Phase 4: 逐项验证（断言清单唯一真值源：docs/text-fix-plan/workflows/verify/stage-15.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage15 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/text-fix-plan/workflows/verify/stage-15.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
