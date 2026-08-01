// =====================================================================
// Stage 07: 文档同步（决策 22-26 回写 + 模块文档登记 + 用例清单）
// 覆盖项: DOC-01、DOC-02、DOC-03、DOC-04、DOC-05、DOC-06
// 特殊纪律: 纯文档 Stage——禁改任何 .ts/.tsx/.rs 代码文件
// fix-loop 调用本 Stage 时 args.constraints 传:
//   "纯文档 Stage：禁改任何 .ts/.tsx/.rs 代码文件"
// =====================================================================

export const meta = {
  name: 'stage07-docs',
  description: 'Stage 07: 决策 22-26 回写 + 模块文档登记 + 用例清单同步',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
特殊纪律：纯文档 Stage——禁改任何 .ts/.tsx/.rs 代码文件（含测试代码）。
背景：修复要点详见 checklist 对应 ID 条目（先读再动手）。先读 docs/claude-history-view/checklist.md 中 DOC-01..06 条目 + docs/claude-history-view/stages.md Stage 07 实现要点，再动手。文档不撒谎：引用的文件路径/命令名/函数名逐一 grep 命中真实代码后再写入。`

// === Phase 1: 并行重构（单 agent）===
phase('并行重构')
const parallelAgents = [
  {
    label: 'docs-agent',
    prompt: `你负责 DOC-01..06（全部文档同步）：

【DOC-01 docs/claude-history-view/README.md 升 v1.1】
- 版本头改 v1.1 并注明变更日期与来源（决策 22–26 回写）。
- 决策记录表追加第 22–26 行（内容照 stages.md「跨 Stage 契约」段前置的决策表：22 重命名写 custom-title 且标题回退链 custom-title > ai-title > summary > 首条 prompt；23 E2E 隔离 = SLTERM_CLAUDE_PROJECTS_DIR env 覆盖；24 当前项目匹配 = normalizePath + 忽略大小写精确相等；25 恢复注入 = addPanel → 轮询 TerminalRegistry → pty.write，零后端改动；26 时间口径 = 文件 mtime）。
- 决策 10 行标注「已被决策 22 推翻」（保留原文加标注，不删除）。
- 修订三处正文：4.2 标题回退链加入 custom-title 首优先级；4.4 重命名操作写 custom-title（原 ai-title 表述替换）；4.3.2 恢复步骤 4 改 pty.write 注入表述（原任何后端注入表述替换）。

【DOC-02 src-tauri/src/claude_history/CLAUDE.md 新建】
- 四段式（照 src-tauri/src/hooks/CLAUDE.md 结构）：职责 → 架构决策（扫描根单点 resolve_projects_root + env 覆盖「生产不设置，仅测试用途」标注 + 标题回退链 + 头 512KB/尾 64KB 轻量解析 + SEC-01 定位不信托前端 + home 目录绕过沙箱先例）→ 文件表（mod.rs/scan.rs/jsonl.rs/ops.rs）→ 测试模式（用例分类与运行命令，计数与实跑一致）。
- 根 .claude/CLAUDE.md 模块索引表追加 src-tauri/src/claude_history 行（照既有行格式，详情列链接）。

【DOC-03 src/features/claudeHistory/CLAUDE.md 新建】
- 四段式：职责 → 架构决策（双行式行 + 三下拉框 + 四步恢复编排 + ⚡ 派生机制与其局限——仅本应用 spawn 且有 transcriptPath 的会话可标记 + 操作矩阵：普通/孤儿/⚡/无 cwd × 复制/分支/删除/重命名/双击）→ 文件表 → 测试模式。
- 根 .claude/CLAUDE.md 模块索引表追加 src/features/claudeHistory 行。
- src/ipc/CLAUDE.md 模块映射表追加 claudeHistory.ts 行（三命令名逐字：claude_history_scan / claude_history_delete / claude_history_rename；与 hooksConfig.ts 行同格式）。

【DOC-04 根 .claude/CLAUDE.md 需求编号索引】
- 表尾追加：| F7 | 特性 | claude 历史会话查询与恢复（历史区三下拉框 + 扫描/恢复/删除/重命名） |

【DOC-05 src/features/sideViews/CLAUDE.md】
- AgentStatusView.tsx 行描述更新为三下拉框结构（活跃会话 + 当前项目历史会话 + 全部项目历史会话），替换旧「渲染 Agent 会话状态列表」单一描述。

【DOC-06 .claude/test-inventory.md 一致性】
- L1 类目追加 claude_history 模块条目（计数与 cargo test 实跑一致——先跑 cargo test --manifest-path src-tauri/Cargo.toml claude_history -- --test-threads=1 取数，或静态 grep 测试文件 #[test] 计数，口径写入条目注释）。
- L2 类目追加 claude-history-* 测试文件条目（计数与实跑一致）。
- E2E 类目的新 describe 计数若与 Stage 06 后实际 it( 计数有出入，一并修正。

【文档不撒谎核查（收尾必做）】
- 两份新 CLAUDE.md 与 README 修订处引用的每个文件路径/命令名/函数名（如 claude_history_scan、resolve_projects_root、restoreHistorySession、HistorySessionRow、TRANSCRIPT_TAIL_BYTES 先例引用），逐一 Grep 真实代码确认命中后再定稿；引用先例描述（hooks/usage.rs 尾部扫描、commitContextMenu 策略模式、openHooksConfigPanel 轮询、SidebarTree 建页模式）Read 对照属实。`,
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
2. npx eslint src/
3. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
4. npm test
5. cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 4: 逐项验证（断言清单唯一真值源）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 7 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/claude-history-view/workflows/verify/stage-07.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
