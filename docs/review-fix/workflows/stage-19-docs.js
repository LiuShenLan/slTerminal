// =====================================================================
// stage-19-docs.js — S19 文档同步（DOC-01~10 + 全模块 CLAUDE.md + test-inventory）
// =====================================================================
// 固定最后 Stage：文档反映 S01~S18 完成后的最终状态。
// 特殊纪律：本 Stage 只改文档（.md/CONTEXT.md/README.md），禁改任何代码文件。
// 门禁：npx tsc --noEmit 静态兜底 + git diff 仅文档类文件（代码零改动）。
// fix-loop 调用约定：args.testCommands 传 ["npx tsc --noEmit"]；
//   args.constraints 传「只改文档，禁改代码」。
// =====================================================================

export const meta = {
  name: 'stage19-docs',
  description: 'Stage 19: 约束修订 + 豁免/决策汇总登记 + 模块 CLAUDE.md/test-inventory 全量同步（DOC-01~10）',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
背景：修复要点详见 docs/review-fix/checklist.md 对应 ID 条目 + docs/review-fix/stages.md S19 节（先读再动手）；全量变更事实来源 = docs/review-fix/stages.md + git log（S01~S18 各 commit）。
本 Stage 特殊纪律：只改文档（.md / CONTEXT.md / README.md），禁止改任何代码文件；文档描述须对照当前代码核实——禁凭 stages.md 转述，写每个事实前 Read 代码确认（防文档撒谎）；不跑任何测试（纯文档 Stage）。`

// === Phase 1: 并行重构（agent 间文件零重叠）===
phase('并行重构')
const parallelAgents = [
  {
    label: 'docs-root',
    prompt: `你负责 DOC-01、DOC-02、DOC-03、DOC-04、DOC-05、DOC-06、DOC-07、DOC-08、DOC-09、DOC-10（root 侧），只许改 .claude/CLAUDE.md、CONTEXT.md、新建 README.md、改 .claude/adr.md：
【DOC-01】约束 #11 修订：可自动化部分必须覆盖；不可自动化部分须在 test-inventory 既定豁免清单登记并注明原因与兜底层级。
【DOC-02】约束 #9 修订（D5）：业务 cfg 仅 pty/conpty_api/shell/win_build；测试 cfg 原则上改 cfg!()，例外须模块 CLAUDE.md 登记。
【DOC-03】新增约束：store 只存状态不存业务逻辑、持久化经指定 IPC、禁止跨 store 隐式依赖。
【DOC-04】新增约束：注册表家族通用契约——模块级单例、register/getAll/_reset 接口形态、side-effect import 触发、_reset 测试隔离。
【DOC-05】约束 #5 补充合法形态条款：hub 容器 + 注册表分派子编辑器模式（hooksConfig 先例）。
【DOC-06】约束 #4 补充：字段类型泛化后语义值集须在 profile 与后端 provider 同步登记并配合同步测试。
【DOC-07】约束 #6：完整例外清单写入约束正文；规定新增例外须同步登记对应模块 CLAUDE.md。
【DOC-08】新建 README.md：项目定位、构建/测试命令、文档链接（CONTEXT.md/adr.md/test-inventory）。
【DOC-09】CONTEXT.md:27 面板类型 html 改 htmlviewer（对照 src/panelRegistry.ts 核实注册 id）。
【DOC-10 root 侧】adr.md 汇总登记：FE-01（Workspace 多实例豁免 + MAX_PAGES=20，对照 src/stores/projects.ts 核实上限值）、SEC-09（CSP unsafe-inline 保留——srcdoc 继承父 CSP，对照 src-tauri/tauri.conf.json 核实）、09#14（Mutex 中毒保持现状，parking_lot/catch_unwind 仅作未来预案）、其余 BE-21/FE-31/SEC-06/TE-03/TE-04 交叉引用对应模块 CLAUDE.md 与 S16 已写 ADR 条目（避免重复登记——先 Read adr.md 现状）。`
  },
  {
    label: 'docs-fe-modules',
    prompt: `你负责前端模块 CLAUDE.md 随动，只许改 src/ 下各模块 CLAUDE.md：
先 git log --oneline 看 S01~S18 commit + 对照代码现状，逐模块同步：
- src/ipc/CLAUDE.md：SEC-06 剪贴板消费点登记（src/panels/terminal/keyboard.ts 的 terminal.paste，Ctrl+Shift+V 手势）；命令数终态 34；appError.ts 新文件登记
- src/types/CLAUDE.md：HooksLayer 收窄登记（"user"|"project"|"local"，未来 CLI 加层再泛化）
- src/workspace/CLAUDE.md：FE-01 豁免（多实例 + MAX_PAGES=20）；FE-26 AbortSignal
- src/panels/CLAUDE.md：SEC-04 nonce 校验（HTML 预览 postMessage）
- src/panels/editor 对应 CLAUDE.md（按实际路径）：FE-31 决策登记（CM 不虚拟化——D3 分块削峰 + 10MB 上限 + 1MB 警告）
- src/features/explorer/CLAUDE.md：FE-30 虚拟化、FE-15 子树刷新、BE-10 stopWatch
- src/features/agentHistory/CLAUDE.md：force 参数/缓存（BE-19）、FE-27 AbortSignal
- src/features/hooksConfig/CLAUDE.md：SEC-05 user 层确认
- src/features/sideViews/CLAUDE.md：FE-21 按需卸载
- src/stores/CLAUDE.md：FE-09 保存失败 toast、FE-11 corrupted 消费
- 其余按 git diff --name-only 触及模块随动（先确认该 CLAUDE.md 存在再改）
登记措辞遵循各文件既有风格；渐进式披露——只登记触碰该模块才需要的细节。`
  },
  {
    label: 'docs-be-modules',
    prompt: `你负责后端模块 CLAUDE.md + 测试清单，只许改 src-tauri/ 下各模块 CLAUDE.md、.claude/test-inventory.md：
先 git log --oneline 看 S01~S18 commit + 对照代码现状，逐模块同步：
- src-tauri/src/CLAUDE.md：app_dir 新模块登记（BE-16）；09#14 Mutex 中毒保持现状登记（parking_lot/catch_unwind 仅作未来引入高风险外部代码时的预案）；命令数终态 34；模块索引 app_dir 行
- src-tauri/src/pty/CLAUDE.md：MAX_PTY_SESSIONS=32（BE-01）；reader 微批编排（BE-05——「读到即续读」64KB）；kill 加固（BE-06）；pty_reattach 删除（SEC-03）；pty_kill_all（BE-08）；shell 白名单真实路径校验（SEC-01）
- src-tauri/src/notify/CLAUDE.md：WATCH_EXCLUDE_DIRS 七元素（BE-02）；池容量 8（BE-11）；notify_stop_watch（BE-10）；symlink 过滤（SEC-08）；fs-event 合并上限（BE-07）
- src-tauri/src/fs/CLAUDE.md：BE-21 豁免登记（read_dir 不分页——懒加载 + FileTree 虚拟化覆盖渲染侧）；fs_read_file Channel 分块（BE-03）
- src-tauri/src/hooks/CLAUDE.md：SEC-02 symlink 过滤、SEC-05 语义校验、SEC-12 信任边界、SEC-13 哈希比对、BE-18 Layer 枚举、BE-20 allow 移除
- src-tauri/src/agent_history/CLAUDE.md：缓存键 (目录 mtime, 文件数) + force 参数（BE-19）
- src-tauri/src/git/CLAUDE.md：git_repo_cache LRU 8（BE-09）
- 测试 cfg 豁免残留处（S14 报告列出的保留点）在对应模块 CLAUDE.md 登记豁免（BE-17/DOC-02）
- .claude/test-inventory.md：DOC-01 豁免表更新（reader_loop I/O 编排项 1 因 S06 变动——对照 src-tauri/src/pty/reader.rs 现状重写豁免描述）；S01~S18 全部新增/修改用例登记（逐 Stage grep 新增测试函数，按既有格式登记）`
  },
]
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 2: 全量测试（文档 Stage：静态兜底 + 代码零改动门禁）===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:/data/learn/code/slTerminal 执行文档 Stage 门禁：
1. npx tsc --noEmit（静态兜底，确认文档 Stage 未误伤类型）
2. git diff --name-only HEAD——输出必须只含文档类文件（S19 范围 = .claude/*.md、src/**/CLAUDE.md、src-tauri/**/CLAUDE.md、CONTEXT.md、README.md）；发现任何代码文件（.ts/.tsx/.rs/.toml/.json/.yml）变更即判失败并列出文件名
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证（断言清单唯一真值源）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 19 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/review-fix/workflows/verify/stage-19.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
总则：不仅核对字面断言，还须 Read 代码判断实现是否达成断言意图——文档类断言必须对照真实代码核实（防文档撒谎），文档与代码矛盾判 not_fixed。
以下为测试 agent 的门禁执行结果，测试类断言据此判定（无需重跑）：
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
