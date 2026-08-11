// =====================================================================
// Stage 04 — hub 编辑器分派 + 配置层抽象（KZ-1/KZ-4/KZ-5 消解验证）
// =====================================================================
// 真值源: docs/review-fix/checklist.md + docs/review-fix/stages.md（Stage 04 节）
// 断言清单: docs/review-fix/workflows/verify/stage-04.md（本脚本与 fix-loop 共用同一真值源）
// 编排说明: 本 Stage 为 pipeline 串行 2 agent——types.ts / profiles/claude/index.ts /
//   cli-profile-claude.test.ts / hooks-config-panel.test.tsx / panels/CLAUDE.md 共享，
//   editor-dispatch 产出供 layers 使用（模板串行块：前序未返回则下游短路不跑）
// 跨边界契约（stages.md 契约 2 原文，写死——两 agent 不各自推断）：
//   HooksConfigEditorProps { profile: CodingCliProfile; onDirtyChange?: (dirty: boolean) => void;
//     askGuardRef?: React.MutableRefObject<boolean> }（泛化自 ClaudeHooksConfigEditorProps）
//   HooksCapability 追加：configEditor?: React.ComponentType<HooksConfigEditorProps>（hasConfigEditor=true 时必填）
//     + configLayers?: { id: string; label: string; hint: string }[]（hasConfigEditor=true 时必填）
//   HooksLayer = string（值集由 profile.capabilities.hooks.configLayers 声明）
//   hub 分派：const Editor = selectedProfile?.capabilities.hooks?.configEditor
//     → Editor ? <Editor key={selectedProfile.id} profile={selectedProfile} onDirtyChange={...} askGuardRef={...} /> : 空态占位
//   claude profile 挂载 configEditor: ClaudeHooksConfigEditor + configLayers: 现 LAYERS 三层值
//   后端零改动（trait layer 参数本为字符串；parse_layer 三层校验是 claude provider 内部知识）
// fix-loop args: { stage: 4, failedItems, fixContext,
//   verifyFile: 'docs/review-fix/workflows/verify/stage-04.md',
//   constraints: stages.md「禁区」六条原样 }
//   ——本 Stage 无 L4 门禁（hub claude 渲染列入人工验证点 + Stage 06 收尾 L4 兜底），testCommands 缺省即可
// =====================================================================

export const meta = {
  name: 'stage04-hub-editor-dispatch',
  description: 'Stage 04: hub 编辑器分派 + 配置层抽象入 profile（KZ-1/KZ-4/KZ-5）',
  phases: [
    { title: '串行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区（不可违背）：
1. compute_conpty_flags 固定 0x7 勿动（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮，无法自动化守卫
2. C10 契约：reporter 任何路径 exit(0)、不写 stderr——改 payload 键时勿削弱
3. watcher 轮询补漏（3s）勿削弱——win10 实证防线
4. SEC-05 等价：sessionId 校验 + 定位不信托前端——改 ops.rs 时勿削弱（is_symlink 是加防御不是松校验）
5. E2E 不得触碰用户真实 ~/.claude/——AQ-4 正是强化此防线，改 run-wdio.cjs 时勿引入新降级路径
6. E2E_ENABLED 保持内联 import.meta.env 字面量形态（rolldown DCE 红线）——改 helpers.ts 时勿动
背景：先读 docs/review-fix/checklist.md 中你负责 ID 的条目原文 + docs/review-fix/stages.md 契约 2 与 Stage 04 节实现要点，再动手。
本 Stage 纪律：
- 串行执行：editor-dispatch 先完成，layers 在其产出上继续（types.ts / profiles/claude/index.ts / 测试文件共享）
- 不跑资源共享型测试——编译级检查 npx tsc --noEmit；允许跑自己改动的单文件 vitest；全量测试由专门 agent 统一执行
- .claude/test-inventory.md 归 layers 单点负责——editor-dispatch 禁止触碰（你的用例变化由 layers 在最终态一并登记）`

// === Phase 1: 串行重构（共享文件依赖，前序 agent 的产出供后序使用）===
phase('串行重构')
const sequentialAgents = [
  {
    label: 'editor-dispatch',
    prompt: `你负责 KZ-1（configEditor 入 profile + hub 分派）——契约 2（写死）：

【types.ts】src/features/cliProfiles/types.ts：
- 新增 export interface HooksConfigEditorProps { profile: CodingCliProfile; onDirtyChange?: (dirty: boolean) => void; askGuardRef?: React.MutableRefObject<boolean> }（泛化自 ClaudeHooksConfigEditorProps；React 仅类型 import）
- HooksCapability 追加：configEditor?: React.ComponentType<HooksConfigEditorProps>（注释注明 hasConfigEditor=true 时必填；缺失 → hub 空态防御）

【claude profile 挂载】src/features/cliProfiles/profiles/claude/index.ts：capabilities.hooks.configEditor = ClaudeHooksConfigEditor——import 自 src/panels/hooksConfig/ClaudeHooksConfigEditor（新增 features→panels 依赖方向，合法化理由：profiles/claude/ 是 claude 合法领地，编辑器组件是 claude 专属资产；types.ts 仅类型 import 不构成运行循环——理由写入就近文档）。

【hub 分派】src/panels/hooksConfig/HooksConfigPanel.tsx：
- :32 import ClaudeHooksConfigEditor 移除（唯一直接引用消亡）
- :253-260 编辑器槽改分派渲染：const Editor = selectedProfile?.capabilities.hooks?.configEditor → Editor 存在则渲染 <Editor key={selectedProfile.id} profile={selectedProfile} onDirtyChange={...} askGuardRef={...} />，缺失则空态占位（不渲染 claude 编辑器）
- 选择行过滤条件（hasConfigEditor === true）不变

测试：
- src/__tests__/cli-profile-claude.test.ts：configEditor 挂载断言（= ClaudeHooksConfigEditor）
- src/__tests__/hooks-config-panel.test.tsx：hub 分派用例（经 profile.configEditor 渲染；hasConfigEditor=true 但 configEditor 缺失 → 空态占位）
就近同步：
- src/features/cliProfiles/CLAUDE.md（configEditor 字段 + features→panels 依赖方向合法化说明）
- src/panels/CLAUDE.md（hub 段：编辑器槽经 profile.configEditor 分派 + 文件表随行）
禁止触碰 .claude/test-inventory.md——归 layers 单点负责。
循环依赖自查：npx tsc --noEmit 通过；你的改动完成后由全量测试 agent 跑 npx vite build 验证打包图无循环报错。
单文件自查：npx vitest run src/__tests__/cli-profile-claude.test.ts src/__tests__/hooks-config-panel.test.tsx 通过。`,
  },
  {
    label: 'layers',
    prompt: `你负责 KZ-4（configLayers 入 profile + HooksLayer 泛化）+ KZ-5 消解验证配合——契约 2（写死）。editor-dispatch 已完成 KZ-1（types.ts 已有 HooksConfigEditorProps/configEditor，claude profile 已挂载编辑器），你在其产出上继续：

【types.ts】src/features/cliProfiles/types.ts：HooksCapability 追加 configLayers?: { id: string; label: string; hint: string }[]（注释注明 hasConfigEditor=true 时必填；claude 值 = user/project/local 三层现值）。
【HooksLayer 泛化】src/types/hooksConfig.ts:8：export type HooksLayer = string（注释注明：值集由 profile.capabilities.hooks.configLayers 声明；claude = "user"|"project"|"local"）。
【claude profile】src/features/cliProfiles/profiles/claude/index.ts：capabilities.hooks.configLayers = 现 LAYERS 三层值（含 label/hint 文案，从 ClaudeHooksConfigEditor.tsx:58-66 迁入）。
【编辑器改造】src/panels/hooksConfig/ClaudeHooksConfigEditor.tsx：模块级 LAYERS 常量退役——层切换器数据源改 profile.capabilities.hooks.configLayers 渲染；PRIORITY_HINT 处置（由 layers 派生或编辑器内部保留——claude 领地内可硬编码，形态自决）；project/local 需 rootPath 的禁用判定是 claude 知识——保留在编辑器内部（合法领地，形态自决）。
【useHooksConfig】src/panels/hooksConfig/useHooksConfig.ts：layer 状态类型随行（HooksLayer = string）；初始层 = configLayers?.[0]?.id ?? "user"（防御缺省）。
【KZ-5 消解配合】不改 ClaudeHooksConfigEditor 内部 ~/.claude/settings.json 错误文案——分派落地后该文件整文件属 claude 专属编辑器合法领地（verify 阶段断言 hub 无直接引用即闭环）。
【后端零改动】trait config_read/write 的 layer 本为字符串；parse_layer 三层校验是 claude provider 内部知识——不碰 src-tauri/。

测试：
- src/__tests__/cli-profile-claude.test.ts：configLayers 断言（三层现值）
- src/__tests__/hooks-config-panel.test.tsx：层渲染用例随行（数据源 = profile.configLayers）
- src/__tests__/hooks-config-sync.test.tsx：若涉层类型则随行（注意扩展名为 .tsx）
就近同步：src/types/CLAUDE.md（HooksLayer 泛化）、src/panels/CLAUDE.md（层切换器数据源）、src/features/hooksConfig/CLAUDE.md（层声明入 profile）。
.claude/test-inventory.md 由你单点负责——登记本 Stage 全部用例变化（含 editor-dispatch 的 configEditor 挂载断言/hub 分派用例）。
mock 夹具波及提示：src/__tests__/helpers/mockCliProfile.ts 与 e2e-tests/helpers.ts 的 mockcli 声明在 Stage 05 才补 configEditor/configLayers——本 Stage 不动；字段可选故 tsc 不报错，中间态合法。
自查：npx tsc --noEmit 通过；npx vitest run src/__tests__/cli-profile-claude.test.ts src/__tests__/hooks-config-panel.test.tsx 通过。`,
  },
]
const sequentialResults = []
for (const a of sequentialAgents) {
  const r = await agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label })
  if (!r) break  // 前序失败短路，不跑下游
  sequentialResults.push(r)
}

// === Phase 2: 全量测试（命令相互独立，并行启动执行，收集全部结果）===
phase('全量测试')
const TEST_COMMANDS = [
  'npx tsc --noEmit',
  'npx eslint src/',
  'cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings',
  'cargo fmt --manifest-path src-tauri/Cargo.toml -- --check',
  'cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1',
  'npm test',
  'npm run test:l3',
  'npx vite build',
]
const testResult = await agent(`
在项目根目录 D:/data/learn/code/slTerminal 执行全量验证。
执行前确认：无运行中的 slterminal.exe（Windows 文件锁会致 cargo 链接 os error 5）。
以下命令相互独立，并行启动执行，收集全部结果：
${TEST_COMMANDS.map((c, i) => `${i + 1}. ${c}`).join('\n')}
第 8 条 npx vite build 专责验证 KZ-1 新增 features→panels 依赖方向的打包图——报告是否有 circular dependency 警告/报错。
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证（断言清单唯一真值源：verify/stage-04.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 04 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/review-fix/workflows/verify/stage-04.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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

return { sequentialResults, testResult, verifyResult }
