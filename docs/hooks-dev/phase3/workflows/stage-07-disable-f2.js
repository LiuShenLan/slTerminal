// =====================================================================
// Stage 07 Workflow — 单条启停（ADR-0002）与 F2 并入
// =====================================================================
// 跨边界契约：
//   - 禁用四元组: { layer, event, matcher, command }
//   - 禁用状态存 ~/.slterminal/settings.json 的 disabledHooks 段（ADR-0002）
//   - 注入段条目（isSltermManaged）不渲染禁用 checkbox（C13-8 禁禁用）
//   - F2 复用 C6 wrapper 实际命名（src/ipc/hooks.ts，经 hooks namespace）:
//     inject() / uninstall() / getInjectionStatus()（无 Hooks 后缀）
//   - inject/uninstall 完成后自动重读 user 层配置（操作改写 ~/.claude/settings.json）
// =====================================================================

export const meta = {
  name: 'stage-07-disable-f2',
  description: '单条 hook 启停 + 失效禁用记录 + F2 注入/卸载/状态（wrapper 真名）',
  phases: [
    { title: '串行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
背景：修复要点详见 docs/hooks-dev/phase3/checklist.md 对应 ID 条目（先读再动手）。串行阶段：disable UI 先完成，F2 状态条后接入。`

// === Phase 1: 串行重构 ===
phase('串行重构')
const sequentialResults = []
const sequentialAgents = [
  {
    label: 'frontend-disable',
    prompt: `你负责 P3-FE-19/20 与 P3-TE-15/16。

【P3-FE-19】实现禁用 UI：
- 在 HandlerForm.tsx 或 EventTree.tsx 中为每条 handler 添加启停 checkbox——注入段条目（isSltermManaged 命中）除外，不渲染禁用 checkbox（C13-8 禁禁用）。
- 禁用条目在事件树中视觉区分（置灰 + 文字删除线）。
- 常驻提示文案：「禁用条目由 slTerminal 托管，不出现在配置文件中」。
- 失效禁用记录：当四元组（layer+event+matcher+command）在配置中找不到匹配时，UI 标记为「失效的禁用记录」。

【P3-FE-20】保存时过滤禁用条目：
- 在 useHooksConfig.ts 的保存逻辑中，先将 configJson 经 filterDisabled(config, disabledHooks) 剔除禁用条目，再 writeHooksConfig。
- 重新启用时按四元组将条目插回原位置（原位置因外部修改不存在则标记失效）。

【收尾】更新 src/stores/hooksConfig.ts 以支持失效记录展示所需的状态（如 staleDisabledKeys 派生）。

【P3-TE-15/16】新建 src/__tests__/hooks-config-disable.test.tsx（两 ID 同文件分 describe）：
- TE-15：禁用 → 保存时 IPC 调用 hooks 不含禁用条目 → store 持久化 disabledHooks → 重载后禁用状态保留 → 重新启用后条目恢复。
- TE-16：手动修改 JSON 使四元组失配 → UI 显示失效标记 → 重新启用或删除失效记录后标记消失。
- 注入段条目无禁用 checkbox。`
  },
  {
    label: 'frontend-f2',
    prompt: `你负责 P3-FE-21/22。

【P3-FE-21】在 HooksConfigPanel.tsx 顶部工具栏（或独立状态条）集成 F2 注入/卸载按钮：
- 复用 src/ipc/hooks.ts（阶段 1 已完成）经 hooks namespace 导出的 inject() / uninstall()——注意实际命名无 Hooks 后缀。
- 按钮：「注入 Hooks」/「卸载 Hooks」。
- 注入/卸载操作完成后自动重读 user 层配置（操作会改写 ~/.claude/settings.json，C13-8）。
- 本 Stage 不改 src/ipc/hooks.ts 的实现，仅调用其接口。

【P3-FE-22】显示注入状态：
- 调用 getInjectionStatus() 获取状态，状态条显示：已注入 / 未注入 / 版本过旧（HookInjectionStatus.status: injected / notInjected / outdated）。
- 注入/卸载操作后刷新状态。`
  }
];
for (const a of sequentialAgents) {
  const r = await agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label })
  if (!r) break
  sequentialResults.push(r)
}

// === Phase 2: 全量测试 ===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:\\data\\learn\\code\\slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. npx tsc --noEmit
2. npx eslint src/
3. npm test -- hooks-config-disable
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证 ===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 07 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/hooks-dev/phase3/workflows/verify/stage-07.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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

return { sequentialResults, testResult, verifyResult }
