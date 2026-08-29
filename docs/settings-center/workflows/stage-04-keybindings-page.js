// =====================================================================
// Stage 04 — 快捷键页（SC-FE-09）
// =====================================================================
// 改动项: SC-FE-09 快捷键可视化页 + setCaptureSuspended/getEffectiveKeystroke 两 API
// 分工: 单 agent（ShortcutRegistry.ts + KeybindingsPage.tsx + pages.ts 追加 + 2 测试）
// 门禁: tsc + eslint + npm test
// fix-loop 调用约束: args.constraints 传
//   "ShortcutRegistry 只加两 API 与捕获态短路，既有 resolve/effectiveKeystroke 语义零变更"
// =====================================================================

export const meta = {
  name: 'stage04-keybindings-page',
  description: 'Stage 04: 快捷键可视化配置页 + 录制屏蔽 API（F11）',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
背景：修复要点详见 checklist 对应 ID 条目（先读再动手）。动手前先 Read docs/settings-center/checklist.md 中 SC-FE-09 条目全文，严格按「修复步骤」执行（两 API 代码块为照抄级，禁止自行另设计）。
【Stage 特殊纪律】ShortcutRegistry 只加 setCaptureSuspended/getEffectiveKeystroke 两公共 API 与捕获态短路（handleKeyDown/resolve 起始 return false），既有 resolve/effectiveKeystroke 语义零变更。`

// === Phase 1: 并行重构（单 agent Stage；重构 agent 不跑全量测试）===
phase('并行重构')
const parallelAgents = [
  { label: 'fe-keybindings', prompt: `你负责 SC-FE-09（照 docs/settings-center/checklist.md 条目执行）：

1. src/features/shortcuts/ShortcutRegistry.ts（:181-199 effectiveKeystroke 旁）新增两公共 API（照 checklist 代码块逐字）：
   - private captureSuspended = false + setCaptureSuspended(suspended)；handleKeyDown 与 resolve 起始加 if (this.captureSuspended) return false
   - getEffectiveKeystroke(id: string): string | null（commands.get → effectiveKeystroke → formatKeystroke；无命令/无键返回 null）
2. 新建 src/panels/settings/pages/KeybindingsPage.tsx：
   - listCommands() 按 category 分组（目录序 global/terminal/editor/explorer）；行 = title + 生效键（hasOwnProperty(overrides,id) → 高亮 + ↺ + 默认键小字；getEffectiveKeystroke(id)===null → 「未绑定」占位）
   - 录制态：行点击进入（行高亮「按下新键位…Esc 取消」）；录制中挂 window keydown capture 监听：isComposing 跳过 / Escape 取消 / Backspace|Delete → setBinding(id, null) 解绑 / 纯修饰键（code 为 Control*/Shift*/Alt*/Meta*）忽略 / 其余构造 KeyStroke → isReserved(ks, cmd.context) → 行内红字拒绝 / findConflict（同 context 他命令 getEffectiveKeystroke 相同）→ 警告「与 XX 冲突，生效按优先级派发」但允许写入 / 合法 → setBinding(id, formatKeystroke(ks))
   - 录制开始 setCaptureSuspended(true)，结束/取消/卸载 false（卸载兜底必须覆盖）
   - findConflict(commands, getEffective, id, keystroke) 页内纯函数导出（单测）
   - 配色全走 theme/colors.ts token（硬约束 #6）
3. src/features/settingsCenter/pages.ts 追加注册 { id: "keybindings", title: "快捷键", group: "global", order: 10 }
4. 测试：
   - 新建 src/__tests__/settings-keybindings.test.tsx（分组渲染 / override 高亮+默认小字 / 未绑定占位 / 录制 Esc 取消 / Backspace 解绑 / 纯修饰键忽略 / 保留键红字不写入 / 冲突警告放行 / 合法写入 setBinding / ↺ clearBinding / 卸载清 suspended）
   - src/__tests__/shortcuts.test.ts 加 2 例（suspended 时 handleKeyDown 不消费 / resolve 返回 false）
   - src/__tests__/command-catalog.test.ts 不动（9 条无增删）
自查：npx tsc --noEmit 零错 + npx eslint src/features/shortcuts/ src/panels/settings/ src/features/settingsCenter/ 零警告。` },
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
3. npm test
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证（断言清单唯一真值源：docs/settings-center/workflows/verify/stage-04.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 04 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/settings-center/workflows/verify/stage-04.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
