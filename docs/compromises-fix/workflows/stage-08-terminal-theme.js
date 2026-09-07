// =====================================================================
// Stage 08 终端与主题体验
// 并行 4 路：CP-020 Ctrl+C 中断 ∥ CP-018 SwiftShader ∥ CP-039+002 合并体
// （039 先于 002）∥ CP-009 ConPTY 开关配置化（禁区豁免仅此 agent）
// fix-loop constraints 取值唯一真值源 = execution-plan.md「fix-loop 调用规范」表 S08 行
//（本脚本 PREAMBLE 禁区行已内含 CP-009 豁免口径，与表值同源）
// =====================================================================

export const meta = {
  name: 'stage-08-terminal-theme',
  description: 'S08 终端与主题体验（CP-020/018/039/002/009）',
  phases: [
    { title: '并行修复' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入。【本 Stage 例外】该禁区仅对 cp009-conpty-modes agent 解除（CP-009 即该禁区的配置化翻案，ADR-0007 已裁决）；其余 agent 不得触碰 ConPTY flags。
背景：修复步骤六段式真值源 = docs/compromises-fix/checklist.md 对应 CP 条目（先读再动手，照抄步骤不另起方向）；大代码块按条目内「照抄 review-NN 步骤 X.Y」指针读 docs/compromises-fix/ 下对应 review 文件。
并行纪律：本阶段只做编译级检查（npx tsc --noEmit / cargo test --no-run），真实测试由全量测试 agent 单点跑。`

// === Phase 1: 并行修复（4 路文件零重叠；package.json 归 CP-039+002 agent）===
phase('并行修复')
const parallelAgents = [
  {
    label: 'cp020-terminal-interrupt',
    prompt: `你负责 CP-020（Ctrl+C 本地中断事件源，working 显式置 attention；不新增 interrupted 态，理由见条目首段）：
照抄 checklist CP-020 步骤 1-6：commandCatalog.ts 在 terminal.newline 后追加 terminal.interrupt 条目（照抄代码块，defaultKey Ctrl+KeyC）；activeTerminal.ts TerminalActions 加 interrupt?（幂等 JSDoc）；TerminalPanel.tsx 新增 handleInterrupt（照抄代码块）+ useXterm 调用补 onInterrupt；useXterm.ts props 加 onInterrupt? + terminalActions 改造（照抄代码块）；keyboard.ts 追加 commandFromMeta("terminal.interrupt", ...)（handler 返回 false 透传——SIGINT 语义不变）+ 头注释改口径；src/lib/agentStatus.ts:12-14 已知行为注释改 CP-020 口径。
测试：command-catalog.test.ts（EXPECTED_IDS 加 terminal.interrupt；保留键守卫改显式豁免形态）；terminal-shortcuts.test.ts 新增 CP-020 组三例；新增 terminal-interrupt-status.test.tsx；use-xterm-lifecycle.test.ts 补 interrupt 存在性断言。
文档：shortcuts/CLAUDE.md 与 panels/CLAUDE.md 两处「Ctrl+C 保留为中断」改写。
自查：npx tsc --noEmit 绿；条目验证段 grep 断言满足。
完成后报告修改文件清单。`,
  },
  {
    label: 'cp018-swiftshader',
    prompt: `你负责 CP-018（WebGL SwiftShader 探测 + 一次性 toast）：
照抄 checklist CP-018 步骤 1-4：webgl.ts 顶部补 toast import；detectWebgl 原样不动；新增 isSwiftShaderRenderer + swiftShaderCache/swiftShaderNotified + resetSwiftShaderCache（照抄代码块）；setupWebglWithRetry tryLoad 成功路径追加一次性 toast 块（照抄代码块）。
测试：detect-webgl.test.ts 新增 isSwiftShaderRenderer 组四例（每例前 resetSwiftShaderCache）；webgl-setup.test.ts 新增「仅提示一次」用例。
文档：panels/CLAUDE.md FE-26 条目末追加 CP-018 口径。
自查：npx tsc --noEmit 绿；条目验证段 grep 断言满足。
完成后报告修改文件清单。`,
  },
  {
    label: 'cp039-002-editor-theme-jsonlint',
    prompt: `你负责 CP-039+CP-002（editorTheme Compartment 热切换 + 摘除 codemirror-json-schema 自绘 lint/hover；039 先行为 002 基线，共碰 JsonMode.tsx）：
【先 CP-039】照抄 checklist 步骤 1-5：schemeRegistry.ts 增 onDidChange 订阅机制（setActive 两分支 + _reset 触发 notifyChange）；overrides.ts:40 editorTheme 常量改 getEditorTheme() 函数；theme/index.ts re-export 改名；新建 src/theme/editorThemeSlot.ts（照抄代码块：editorThemeBundle/createEditorThemeSlot，Compartment 一 view 一槽红线）；4 消费点改造（useCodeMirror.ts / JsonMode.tsx / GitShowPanel.tsx / DiffPanel.tsx 双栏双槽，模式照条目）。
【后 CP-002】照抄 checklist 步骤 1-5：新建 jsonSchemaCm.ts 自绘层（完整全文照抄 review-01 CP-002 步骤 3.1，约 170 行）；JsonMode.tsx import 收窄 + extensions 三行替换（stateExtensions 行删）；package.json 删 codemirror-json-schema 与 json-schema 两行 + npm install；vitest.config.ts 与 vitest.l3.config.ts 的 server.deps.inline 块整删；schema/index.ts:22-24 TE-15 注释替换。顺手项（章一附注 5）：knip.json ignoreIssues 旧路径 src/panels/hooksConfig/* 清理（:156-184，F11 迁址遗留）。
测试：theme-overrides.test.ts / theme-scheme-registry.test.ts 改写与新增；消费点四测试补 setActive reconfigure 用例 + EditorView 不重建防复发；新建 hooks-json-schema-cm.test.ts 14 用例；hooks-config-jsonmode.test.tsx mock 重写（删 codemirror-json-schema mock，改 jsonSchemaCm sentinel）。
文档：theme/CLAUDE.md :33/:65/:28 改写；panels/editor/CLAUDE.md ACC-05 节落点改述；adr.md ADR-0002 追加后果条目 + TE-15 段末追加消解记录；cliProfiles/CLAUDE.md TE-15 段替换。
自查：npx tsc --noEmit 绿；grep -rn "codemirror-json-schema" src/ package.json vitest.config.ts vitest.l3.config.ts 零命中；npm ls json-schema-library 单实例。
完成后报告修改文件清单。`,
  },
  {
    label: 'cp009-conpty-modes',
    prompt: `你负责 CP-009（ConPTY 模式能力矩阵配置化；compute_conpty_flags 禁区对你解除——ADR-0007 已裁决；人工验证点）：
照抄 checklist CP-009 步骤 1-6：spawn.rs 增 SETTINGS_KEY = "conptyInputModes" + ConptyInputModes DTO（serde + ts-rs 双边，Default = 现状三态等价，照抄代码块；#[ts(export)] 路径按 S05 落地形态）；compute_conpty_flags 加 modes 参（0x4 门控维持 bundled || build >= CONPTY_WIN11_MIN_BUILD；0x8 末行矩阵位）；pty_spawn 接线 read_existing_settings 读段（缺失/解析失败 → default + debug 日志，不阻塞 spawn）；settings.rs SETTINGS_ALLOWED_KEYS 六键改七键；前端 settingsCenter 新增「终端输入模式」页（四开关 + passthrough 常驻警示文案）。
测试：compute_conpty_flags 既有 7 例注入默认矩阵适配（期望 0x7/0x7/0x3 不变）；新增 conpty_flags_default_matrix_matches_legacy_tristate（防复发主用例）+ passthrough_mode_adds_0x8 + win32_input_still_gated_by_build；settings 白名单用例 6→7；L2 新页用例（四开关渲染 + 警示文案断言）。
文档：pty/CLAUDE.md PASSTHROUGH 节改写 + 红线改默认矩阵口径；adr.md ADR-0007 后果节追加；settings.rs 白名单注释补先例；test-exemptions.md 增人工门禁行。
自查：cargo test --no-run；npx tsc --noEmit。
完成后报告：修改文件清单 + 「人工验证点待办：ADR-0007 门禁第 3 条（默认矩阵零漂移时豁免执行，报告注明）」。`,
  },
]
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 2: 全量测试 ===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:/data/learn/code/slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. npx tsc --noEmit
2. npx eslint src/
3. npm test
4. npm run test:l3
5. cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
6. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
7. npx knip --production
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证 ===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 08 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/compromises-fix/workflows/verify/stage-08.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
