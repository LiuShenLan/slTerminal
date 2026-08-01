// =====================================================================
// Stage 01 — F5 行建模重设计
// =====================================================================
// 编排：契约层 A → 并行实现 B ∥ C → 测试重写 D → 全量测试 → 逐项验证
//
// 跨边界契约（写死，agent 不各自推断；真值源 = checklist.md「跨边界契约」段）:
//   契约 1 claudeSession: ClaudeSessionInfo { transcriptPath?: string;
//     matchedCommand?: string; lastEventAt: number }——存在即运行中（二态，
//     无 running 布尔）。RegisteredTerminal.claudeSession?: ClaudeSessionInfo | null
//     （可选字段）。setClaudeSession(panelId, patch: Partial | null)：merge 语义
//     （undefined 键不覆盖旧值）、null 清空、panelId 不存在 no-op 不 notify、
//     缺 lastEventAt 自动填 Date.now()。RegistryEvent.type 增 "sessionChange"
//     （payload 仅 { type, panelId } 裸结构，不带 session 数据——listener 经
//     get() 读现值）。register 幂等覆盖时 claudeSession 缺省保留旧值。
//   契约 2 match: command.trim().split(/\s+/)[0] 取首 token 后精确匹配。
//   契约 5 行生命周期: 建行 = sessionChange(session 非 null) ∨ hook 事件
//     （非 SessionEnd/Exit 且行不存在）；删行 = sessionChange(null) ∨
//     SessionEnd/Exit ∨ remove；初始扫描只建 claudeSession 非 null 行且携
//     transcriptPath 主动 contextUsage 拉一次；初始扫描/事件处理 reconcile 对账。
//
// fix-loop args.constraints 应传值（单一出处，勿手写第三份）:
//   本 Stage 特殊纪律：实现 agent 不跑资源共享型测试（PTY/端口/全局锁类），
//   只做编译级检查（npx tsc --noEmit 可跑）；npm test 由全量测试 agent 单点执行。
//   测试重写 agent（D）可跑 npm test（串行阶段，无并发冲突）。
// =====================================================================

export const meta = {
  name: 'stage-01-row-model',
  description: 'Stage 01 F5 行建模重设计——claudeSession 契约 + match 首 token + useAgentStatus 重设计 + L2 测试重写',
  phases: [
    { title: '契约层' },
    { title: '并行实现' },
    { title: '测试重写' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
背景：修复要点先读 docs/hooks-dev/phase2-fix/checklist.md 对应 ID 条目 + 「跨边界契约」段（契约取值唯一真值源，禁止各自推断）。
本 Stage 特殊纪律：实现 agent 不跑资源共享型测试（PTY/端口/全局锁类），只做编译级检查（npx tsc --noEmit 可跑）；npm test 由全量测试 agent 单点执行。测试重写 agent（D）可跑 npm test（串行阶段，无并发冲突）。`

// === Phase 1: 契约层（A：TerminalRegistry + TabTitleRegistry）===
phase('契约层')
const contractResult = await agent(`${PREAMBLE}

你负责 PF2-FE-01、PF2-FE-02（行建模契约层）：

【PF2-FE-01】TabTitleRegistry.match 改首 token 匹配
- 文件：src/panels/terminal/TabTitleRegistry.ts（match 在 :37-39，现为 this.rules.get(command) ?? null 整行精确匹配）
- 按契约 2：match 内先取 command.trim().split(/\\s+/)[0] 再查表。根因：useCommandDetection.ts:46 把 OSC 133 C 携带的完整命令行喂入，claude --resume xxxx ≠ claude 导致 match null（review 问题 1）。
- 调用方（useCommandDetection.ts）不改——match 内部消化首 token。
- src/panels/terminal/tabRules.ts 顶部注释同步首 token 语义。

【PF2-FE-02】TerminalRegistry 增 claudeSession 契约
- 文件：src/panels/terminal/TerminalRegistry.ts（RegisteredTerminal :11-16；RegistryEvent :19；register 幂等覆盖 :31-35）
- 按契约 1（全文照抄上文脚本头契约 1 取值）：
  1. 新增导出接口 ClaudeSessionInfo { transcriptPath?: string; matchedCommand?: string; lastEventAt: number }
  2. RegisteredTerminal 增可选字段 claudeSession?: ClaudeSessionInfo | null
  3. RegistryEvent.type 增 "sessionChange"
  4. 新增方法 setClaudeSession(panelId: string, patch: Partial<ClaudeSessionInfo> | null)：merge 语义（patch 中 undefined 键不覆盖旧值）、null 清空为 null、panelId 不存在时 no-op 不 notify、patch 缺 lastEventAt 时自动填 Date.now()；成功后 notify({ type: "sessionChange", panelId })
  5. register 幂等覆盖时 claudeSession 缺省保留旧值（调用方未传 claudeSession 字段时不清空）
- 验收锚点：src/__tests__/terminal-registry.test.ts:13-27 的 stub 工厂（四字段对象字面量，不含 claudeSession）必须编译不炸——可选字段设计的意义即在此。
- 完成判据：npx tsc --noEmit 通过（你只改 3 个文件，若其他文件报与本改动无关的既有错误则报告不处理）。`, { label: 'A:registry-contract' })

// === Phase 2: 并行实现（B ∥ C，文件零重叠）===
phase('并行实现')
const implResults = await parallel([
  () => agent(`${PREAMBLE}

你负责 PF2-FE-03、PF2-FE-04（终端侧写 session）。前置：契约层已在 src/panels/terminal/TerminalRegistry.ts 产出 ClaudeSessionInfo/setClaudeSession/sessionChange（先 Read 确认其真实签名再动手，禁凭本 prompt 摘要想象）。

【PF2-FE-03】useCommandDetection 写入会话状态
- 文件：src/panels/terminal/useCommandDetection.ts（签名现 (terminal, onTabStateChange?, sharedCmdRunningRef?)；OSC 133 C :43-50；OSC 133 D :51-55）
- 签名新增 panelId: string 参数（位置自定，建议首参后）。
- OSC 133 C 且 rule 命中时：追加 TerminalRegistry.setClaudeSession(panelId, { matchedCommand: rule.command })——此时无 transcriptPath（对应 feature-plan 边界 5：未注入 hooks 的会话行存在、四态 🟡、用量条不可用态）。
- OSC 133 D 且 isCommandRunningRef.current === true（注册命令退出）时：追加 TerminalRegistry.setClaudeSession(panelId, null)（feature-plan 边界 3：OSC 133 D 删行）。
- 现有 onTabStateChange/isCommandRunningRef 行为不变（正交追加）。
- 调用点 src/panels/terminal/useXterm.ts:205 同步传入 panelId（useXterm 作用域内现成值）。

【PF2-FE-04】useXterm hook 事件订阅写入会话状态
- 文件：src/panels/terminal/useXterm.ts（hooks.onHookEvent 订阅 :349-357）
- 订阅回调内（panelId 过滤之后）追加 session 写入，与页签 emoji 逻辑正交：
  - SessionEnd/Exit 事件 → TerminalRegistry.setClaudeSession(panelId, null)
  - 其他事件 → TerminalRegistry.setClaudeSession(panelId, { transcriptPath: payload.transcriptPath ?? undefined })（merge 语义：payload.transcriptPath 为 null 时传 undefined 不覆盖旧值）
- 不动 eventToStatus/onTabStateChange 现有链路。
- 完成判据：npx tsc --noEmit 通过（只改 2 个文件）。`, { label: 'B:terminal-session' }),

  () => agent(`${PREAMBLE}

你负责 PF2-FE-05、PF2-FE-06、PF2-FE-07（视图侧行建模重设计）。前置：契约层已在 src/panels/terminal/TerminalRegistry.ts 产出 claudeSession 字段/sessionChange 事件（先 Read 确认真实形态再动手）。

【PF2-FE-05】useAgentStatus 行建模重设计
- 文件：src/features/agentStatus/useAgentStatus.ts（handleHookEvent :89-175；SessionEnd/Exit 删行 :111-120；新行 :141-150；usage 拉取 :158-172；registry subscribe effect :186-221 + deps :221；初始扫描 :224-252，usage:undefined 无 transcriptPath :239-247）
- 按契约 5（全文照抄上文脚本头契约 5）重设计：
  1. 行 = 运行中的 claude 会话（非全部终端）
  2. 建行双通道：sessionChange（经 TerminalRegistry.get() 读 session 非 null）∨ hook 事件（非 SessionEnd/Exit 且行不存在——保留现有 :141-150 建行能力；两通道独立幂等，订阅顺序不定）
  3. 删行三通道：sessionChange（session null）∨ SessionEnd/Exit hook 事件 ∨ remove 事件
  4. 初始扫描：遍历 getAll() 只建 claudeSession 非 null 的行；行携 transcriptPath 时主动 contextUsage 拉取一次（修复 review 问题 2b：切项目后 idle 会话用量永远 --）
  5. #5 竞态双保险：① registry/hook-event 双 listener 经 ref 读最新状态（照现有 rowsRef :69-70 模式），effect deps [] 订阅永不重建（remove 事件永不丢失——R4 根因：同 commit passive destroy 顺序 SideBarArea 先于主区，旧 deps 重订阅窗口内 remove 丢失）；② 初始扫描/事件处理 reconcile 对账（行在 registry 中不存在或 session 为 null → 移除）
  6. AgentSessionRow.usage 内联类型（:31 的 {inputTokens, outputTokens} | null）改为引用 ContextUsage（import type from src/types/hooks）——注意：Stage 01 时 ContextUsage 仍 2 字段，Stage 03 才扩 4 字段，本 Stage 按现状类型引用即可

【PF2-FE-06】AgentStatusView 空态文案核对
- 文件：src/features/agentStatus/AgentStatusView.tsx:94
- 核对现状文案是否已是「当前项目无运行中的 claude 会话」——预期零改动；若 drift 则对齐。报告中明确写出核对结论。

【PF2-FE-07】contextUsage 静默 catch 补可观测性
- 文件：src/features/agentStatus/useAgentStatus.ts:169-171（usage 拉取 .catch 静默吞错）
- catch 内补 console.error（DBG-7 教训），降级语义不变（usage 保持旧值/--）。
- 完成判据：npx tsc --noEmit 通过（只改 2 个文件）。`, { label: 'C:view-row-model' }),
])

// === Phase 3: 测试重写（D：按 A/B/C 最终真实形态重写 7 个测试文件）===
phase('测试重写')
const testRewriteResult = await agent(`${PREAMBLE}

你负责 PF2-TE-01、PF2-TE-02（行建模适配部分）、PF2-TE-03、PF2-TE-05、PF2-TE-08（L2 测试重写）。前置：A/B/C 已完成实现——先 Read 以下实现文件确认最终真实形态（禁凭计划想象 API）：src/panels/terminal/TerminalRegistry.ts、src/panels/terminal/TabTitleRegistry.ts、src/panels/terminal/useCommandDetection.ts、src/panels/terminal/useXterm.ts、src/features/agentStatus/useAgentStatus.ts。

【PF2-TE-01】src/__tests__/agent-status-hook.test.ts 全量重写（现 929 行 31 用例）
- mock TerminalRegistry 工厂（:37-72）扩展：setClaudeSession stub + sessionChange 通知能力 + entry 含 claudeSession 字段；registerTerminal 辅助（:147-154）支持 claudeSession 参数
- 语义反转：T1 初始扫描——claudeSession 为 null 的终端不建行，非 null 才建行；FE-03 register 插入 🟡 行——register 不建行（session null），sessionChange（非 null）建行
- 新覆盖（逐条成用例）：纯 shell 无行 / sessionChange 建行（matchedCommand）/ hook 事件建行（行不存在时）/ SessionEnd 删行 / sessionChange(null) 删行 / remove 删行（订阅 deps [] 稳定——remove 不丢失）/ 初始扫描只建活会话 / 初始扫描携 transcriptPath 主动拉 usage / reconcile 对账（行在 registry 不存在 → 移除）
- T7 usage mock 返回字面量按 Stage 01 现状（ContextUsage 2 字段）——Stage 03 扩 4 字段后由 Stage 03 补齐

【PF2-TE-02 行建模适配部分】src/__tests__/agent-status-view.test.tsx（现 410 行 11 用例）
- 「TerminalRegistry 含两个 panelId 时渲染两行」用例（:204-221）：makeTerminalMap 的 entry 补 claudeSession 非 null（行建模改后纯 shell 无行）
- 空态文案断言（:199-201）已符合——保留作回归
- 用量条断言（:263-370）本 Stage 不动（2 字段口径；cache 口径归 Stage 03）

【PF2-TE-03】tab-rules.test.ts + tab-title-registry.test.ts 首 token 语义
- src/__tests__/tab-rules.test.ts:43-46 断言反转：match("claude update") 由 toBeNull 改为命中（首 token claude）——用例改名/改写为新语义
- src/__tests__/tab-title-registry.test.ts（kebab-case，非驼峰）补首 token 用例：带参命中（claude --resume xxx）、空命令行、仅空白、首 token 无规则仍 null

【PF2-TE-05】terminal-registry 两测试扩展
- src/__tests__/terminal-registry.test.ts：stub 工厂（:13-27）不含 claudeSession 编译不炸验证；setClaudeSession 五分支（merge 部分键更新保留其余 / null 清空 / panelId 不存在 no-op 不 notify / 缺 lastEventAt 自动填充 / undefined 键不覆盖旧值）；register 幂等覆盖保留旧 session
- src/__tests__/terminal-registry-subscribe.test.ts：sessionChange 事件——setClaudeSession（非 null/null 均触发）→ listener 收到 { type: "sessionChange", panelId } 裸结构（不带 session 数据）

【PF2-TE-08】src/__tests__/use-xterm-lifecycle.test.ts OSC 133 适配
- useCommandDetection 签名加 panelId（实现见 useXterm.ts:205 调用点）——若测试 mock 了 useCommandDetection 则断言调用参数含 panelId；若真实调用则走真实 match
- 新增断言：OSC 133 C 命中 → TerminalRegistry.setClaudeSession 被调（{ matchedCommand: "claude" }）；OSC 133 D（命令运行中）→ setClaudeSession(panelId, null)
- hook 事件订阅新增断言：非 SessionEnd 事件 → setClaudeSession 携 transcriptPath；SessionEnd → null
- TerminalRegistry 在该文件的 mock 形态同步（增 setClaudeSession stub）
- 完成判据：npm test 全绿（你串行执行，无并发冲突）。`, { label: 'D:l2-test-rewrite' })

// === Phase 4: 全量测试（命令相互独立，并行启动执行，收集全部结果）===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:/data/learn/code/slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. npx tsc --noEmit
2. npx eslint src/
3. npm test
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。
`, { label: 'full test suite' })

// === Phase 5: 逐项验证（断言清单唯一真值源：verify/stage-01.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 01 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/hooks-dev/phase2-fix/workflows/verify/stage-01.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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

return { contractResult, implResults, testRewriteResult, testResult, verifyResult }
