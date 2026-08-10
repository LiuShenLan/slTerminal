// =====================================================================
// Stage 06 — hub 面板
// 条目：MC-501(核对)/502/503/504/505/506/507/508 + MC-220/221/222 + MC-223(核对段) + D-15 + D-14(hooks.e2e hub 段)
//       + Stage 03 中间态回收（CLAUDE_CLI_ID 临时代理 → selectedCliId）
// 真值源：docs/multi-cli/checklist.md（逐 ID 条目）+ docs/multi-cli/stages.md（Stage 06 分工表与实现要点）
// commit message：refactor(hooks-config): hub 面板 + CLI 选择行（MC-501~508）
// fix-loop 调用约定：args.constraints 传 stages.md「禁区」六条原样
// test-inventory 独占：本 Stage 仅 panel-tests agent 可改 .claude/test-inventory.md，其余 agent 禁改。
// 事实留痕：stages.md 称 hooks-config-* 测试 11 文件，2026-08-10 Glob 实查 9 个
//   （catalog/matcher/model/jsonmode/handlerform/gui/sync/schema/panel）——以 glob 全匹配覆盖为准。
// =====================================================================

export const meta = {
  name: 'stage-06-hub-panel',
  description: 'Stage 06：hooks 配置 hub 面板 + CLI 选择行 + selectedCli 持久化 + 中间态回收（MC-501~508 + MC-220~222）',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区（红线，触碰即返工）：
1. compute_conpty_flags 固定 0x7 勿动（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮，无法自动化守卫
2. C10 契约：reporter 任何路径 exit(0)、不写 stderr——勿削弱
3. watcher 轮询补漏（3s）勿削弱——win10 实证防线
4. SEC-05 等价：sessionId 校验 + 定位不信托前端——勿削弱
5. E2E 不得触碰用户真实 ~/.claude/（env 覆盖 + fixture 隔离）
6. E2E_ENABLED 保持内联 import.meta.env 字面量形态（rolldown DCE 红线）
背景：先读 docs/multi-cli/checklist.md 中你负责的 MC-ID 条目原文 + docs/multi-cli/stages.md 的 Stage 06 实现要点，再动手。
【跨边界契约（写死，不各自推断）】
1. 组件拆分形态：HooksConfigPanel.tsx 改造为 hub 容器（选择行 + 编辑器槽）；现状全部内容（层级切换/GUI·JSON 双模式/注入状态条/保存/重启提示条）下移为 ClaudeHooksConfigEditor 组件（同文件或新文件 ClaudeHooksConfigEditor.tsx，行为零改动）；features/hooksConfig/（schema 单点）零改动。
2. selectedCli 持久化照 F8 先例：api.updateParameters({ ...params, selectedCli }) + 显式 onLayoutChange(saveLayout(api))——updateParameters 不触发 onDidLayoutChange（dockviewPanel.js:84-95 实证），必须显式保存；挂载读 params 恢复；缺省/失效回退首个 hasConfigEditor CLI。
3. 泛化命令实参：hub 化后 readHooksConfig/writeHooksConfig/agent_hooks_inject/agent_hooks_uninstall/agent_hooks_injection_status 的 cliId 实参 = hub 选中态 selectedCliId——Stage 03 的 CLAUDE_CLI_ID 临时代理本 Stage 回收；panels/hooksConfig/ 内 ipc 调用实参不得残留 CLAUDE_CLI_ID（claude 编辑器组件内部 claude 知识合法，但 ipc 实参必须来自选中态）。
4. 选择行：cliProfileRegistry.getAll().filter(p => p.capabilities?.hooks?.hasConfigEditor)；按钮 = iconSrc 16×16 + displayName；选中态背景高亮走 theme token（硬约束 #6，禁硬编码色值）；单 CLI 也渲染（边界 1，防布局跳动）；空态「无可配置 CLI」占位不渲染编辑器。
5. 入口零改动：面板 id hooksConfig-{pageId}、侧栏右键菜单流程、pageApis 全部不动（C13-7 同页单例语义不变）。
【测试纪律】你不跑资源共享型测试；只做编译级检查 npx tsc --noEmit——若报错来自非你分工的文件，属其它并行 agent 中间态，忽略；真实执行由全量测试 agent 单点跑。除 panel-tests 外禁止改 .claude/test-inventory.md。`

// === Phase 1: 并行重构（3 agent，文件零重叠）===
phase('并行重构')
const parallelAgents = [
  {
    label: 'hub-panel',
    prompt: `你负责 Stage 06 的 hub 面板改造：MC-502、MC-503、MC-504、MC-505、MC-506、MC-507 + MC-220、MC-221、MC-222（面板侧）+ MC-501/508（核对）+ MC-223（核对段）+ 中间态回收。禁止改 .claude/test-inventory.md（归 panel-tests agent）。

【MC-502】改 src/panels/hooksConfig/HooksConfigPanel.tsx 为 hub 容器：顶部 CLI 选择行——遍历 cliProfileRegistry.getAll() 过滤 capabilities?.hooks?.hasConfigEditor === true，渲染按钮（iconSrc 16×16 logo + displayName）；选中态背景高亮走 theme token（硬约束 #6）；点击切换下方编辑器；单 CLI 也渲染选择行（边界 1，防布局跳动）。
【MC-503】选中态持久化：params.selectedCli 随布局 JSON 持久化——api.updateParameters({ ...params, selectedCli }) + 显式 onLayoutChange(saveLayout(api))（照 F8 customTitle 先例，updateParameters 不触发 onDidLayoutChange 须显式保存）；挂载时读 params 恢复；缺省/失效回退首个有能力 CLI。
【MC-504】选择行下方渲染选中 CLI 的配置编辑器：claude = 现有 HooksConfigPanel 全部内容（层级切换/GUI·JSON 双模式/注入状态条/保存/重启提示条）整体下移一层为 ClaudeHooksConfigEditor 组件（同文件或新文件 src/panels/hooksConfig/ClaudeHooksConfigEditor.tsx，行为零改动）。
【MC-505】切换 CLI = 卸载当前编辑器并重挂载目标编辑器（照 ADR-0001 先例——dirty/选中态丢弃）；dirty 守卫：dirty 时切换需 dialog.ask 确认丢弃（照切层/visibilitychange ask 守卫先例，askGuard 防循环复用）。
【MC-506 + MC-222】保存成功提示条文案 = profile.capabilities?.hooks?.restartHint 驱动（claude 值同现状文案）；data-e2e="hooks-restart-hint" 选择器保留；注入状态条三态语义不变，数据源 agent_hooks_injection_status(selectedCliId)。
【MC-507】选择行空态：无任何 hasConfigEditor profile → 渲染「无可配置 CLI」占位，不渲染编辑器（防御分支）。
【MC-220/221】src/panels/hooksConfig/useHooksConfig.ts 与编辑器 IPC 调用点：readHooksConfig/writeHooksConfig 与 agent_hooks_inject/uninstall 的 cliId 实参 = hub 选中态 selectedCliId（回收 Stage 03 的 CLAUDE_CLI_ID 临时代理）；注入后自动重读 user 层（C13-8）语义不变。
【MC-501/508 核对】面板 id hooksConfig-{pageId} / 侧栏菜单 / pageApis 零改动；panels/hooksConfig/ 其余 9 文件（configModel.ts、EventTree.tsx、GuiMode.tsx、HandlerForm.tsx、eventsCatalog.ts、matcherEngine.ts、MatcherTester.tsx、JsonMode.tsx、index.ts）与 features/hooksConfig/ 行为零改动（claude hooks 协议知识不抽象、文件物理位置保留，MC-223 决策 2）——仅核对。
【中间态回收】grep 确认 panels/hooksConfig/ 内无 CLAUDE_CLI_ID 临时代理残留（ipc 实参来自选中态）。

文件清单（只许碰这些）：改 src/panels/hooksConfig/HooksConfigPanel.tsx（拆 hub 容器）；可新建 src/panels/hooksConfig/ClaudeHooksConfigEditor.tsx；改 src/panels/hooksConfig/useHooksConfig.ts。`,
  },
  {
    label: 'panel-tests',
    prompt: `你负责 Stage 06 的 hooks 配置面板测试同步：D-15 + hub 新用例 + test-inventory 就近登记（本 Stage 独占）。

【hub 用例改造】改 src/__tests__/hooks-config-*.test.ts(x) 全部匹配文件（当前 9 个：catalog/matcher/model/jsonmode/handlerform/gui/sync/schema/panel）——hub 容器内挂载路径变化断言语义不丢（编辑器下移一层后渲染断言同步）；新增用例：选择行能力过滤（hasConfigEditor=false 不出现）/ logo+displayName 渲染 / 选中高亮 token / 单 CLI 也渲染选择行 / 点击切换 → 编辑器重挂载且 IPC 携新 cliId / selectedCli 持久化（updateParameters 写入 + 显式 onLayoutChange 调用，照 customTitle 测试先例）/ 挂载恢复 / 失效回退首个有能力 CLI / dirty 守卫 ask 确认与取消两分支 / 非 dirty 直接切换 / 空态「无可配置 CLI」占位不渲染编辑器 / restartHint 由 profile 驱动（文案来源断言）。
【D-15 核对】核对 src/__tests__/{open-hooks-config-panel.test.ts, sidebar-actions.test.ts, default-layout-format.test.ts}：入口零改动预期（MC-501），应零改动通过；若 hub 改造波及面板注册参数则同步核对并在报告中说明。
【test-inventory 独占登记】改 .claude/test-inventory.md，就近登记本 Stage 全部用例变动（静态清单，含其它 agent 负责的部分）：hooks-config-* 9 文件 hub 改造同步 + 新增 hub 用例（选择行/持久化/dirty 守卫/空态/restartHint 来源）；open-hooks-config-panel / sidebar-actions / default-layout-format 核对（预期零变动）；E2E hooks.e2e hub 用例同步。

文件清单（只许碰这些）：改 src/__tests__/hooks-config-*.test.ts(x)（9 文件）；核对（必要时改）src/__tests__/{open-hooks-config-panel.test.ts, sidebar-actions.test.ts, default-layout-format.test.ts}；改 .claude/test-inventory.md。`,
  },
  {
    label: 'e2e-hub',
    prompt: `你负责 Stage 06 的 E2E hub 用例同步：D-14（本 Stage 段）。禁止改 .claude/test-inventory.md（归 panel-tests agent）。

【D-14 本 Stage 段】改 e2e-tests/hooks.e2e.ts：hub 面板用例同步——选择行渲染（单 CLI 也有选择行 + claude logo/displayName）/ project 层保存写盘 + merge 保留字段经 hub 全绿 / 注入按钮三态经 hub / data-e2e="hooks-restart-hint" 选择器断言保留；断言随 hub 结构调整（编辑器下移一层），断言语义不丢。
【门禁说明】e2e-tests/ 不在根 tsconfig include——你的改动正确性由全量测试 agent 的 npm run e2e（L4）运行时兜底。

文件清单（只许碰这些）：改 e2e-tests/hooks.e2e.ts。`,
  },
]
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 2: 全量测试（1-7 并行收集，8 最后单独串行）===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:/data/learn/code/slTerminal 执行全量验证。命令清单：
1. npx tsc --noEmit
2. npx eslint src/
3. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
4. cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
5. cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
6. npm test
7. npm run test:l3
8. npm run e2e
执行纪律：命令 1-7 相互独立，并行启动执行，收集全部结果；待 1-7 全部结束后，再单独串行执行命令 8（npm run e2e 内部 = build:e2e + wdio 串行；它会重新构建并占用 slterminal.exe，与其他命令并行会构建失败——禁拆分、禁与其他命令并行）。
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证（断言清单唯一真值源：verify/stage-06.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 06 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/multi-cli/workflows/verify/stage-06.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
