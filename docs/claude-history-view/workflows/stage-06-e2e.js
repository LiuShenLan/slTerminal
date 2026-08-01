// =====================================================================
// Stage 06: E2E fixture 与关键路径用例
// 覆盖项: TE-01、TE-02、TE-03、TE-04
// 安全红线（SEC-02）:
//   任何用例不得触碰用户真实 ~/.claude/projects/——扫描根必须经
//   SLTERM_CLAUDE_PROJECTS_DIR 指向 e2e-tests/.tmp-claude-projects/ 副本
// 特殊纪律: 本 Stage 门禁含 npm run build:e2e + npm run wdio 实跑；
//   run-wdio.cjs 不在根 tsconfig include 内——改动后必须经 build:e2e 全链路验证
// fix-loop 调用本 Stage 时 args.constraints 传:
//   "安全红线：任何用例不得触碰用户真实 ~/.claude/projects/，写操作只允许作用于 .tmp-claude-projects 副本"
// =====================================================================

export const meta = {
  name: 'stage06-e2e',
  description: 'Stage 06: E2E fixture 与关键路径用例（env 隔离 + 副本写断言）',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
安全红线（SEC-02）：任何用例不得触碰用户真实 ~/.claude/projects/——扫描根必须经 SLTERM_CLAUDE_PROJECTS_DIR 指向 e2e-tests/.tmp-claude-projects/ 副本；写操作（删除/重命名）只允许作用于副本。
背景：修复要点详见 checklist 对应 ID 条目（先读再动手）。先读 docs/claude-history-view/checklist.md 中 TE-01..04 条目 + docs/claude-history-view/stages.md Stage 06 实现要点 + e2e-tests/CLAUDE.md（helper 机制与键盘输入限制），再动手。`

// === Phase 1: 并行重构（单 agent）===
phase('并行重构')
const parallelAgents = [
  {
    label: 'e2e-agent',
    prompt: `你负责 TE-01、TE-02、TE-03、TE-04：

【TE-01 e2e-tests/fixtures/claude-projects/ fixture 目录】
- 新建 fixture：≥2 个编码目录（模拟 ~/.claude/projects/ 一级子目录形态），内含合成 .jsonl 文件，覆盖全部 7 形态：
  1. 含 custom-title 行的会话（标题取 custom-title）
  2. 含 ai-title 行的会话（标题取 ai-title）
  3. 无标题行 → 回退首条可见 prompt 的会话（含 isMeta/content 数组/< 开头的干扰行，验证跳过规则）
  4. 无 cwd 字段的会话
  5. cwd 指向不存在路径的孤儿会话
  6. agent-*.jsonl 平铺文件（应被扫描排除）
  7. <id>/subagents/agent-*.jsonl（应被扫描排除）
- 会话 id 用合法 UUID 形态；其中一个会话的 cwd 指向 E2E 临时项目目录——该路径在 fixture 中写占位符（如 __E2E_PROJECT_DIR__），由 run-wdio.cjs 复制时替换为真实路径（先 Read stages.md Stage 06 占位符约定逐字对齐）。
- 各 .jsonl 行内容最小可用（type/cwd/message.content 等必要字段），并注释标注每文件覆盖的形态。

【TE-02 run-wdio.cjs env 注入 + 工作副本】
- 先 Read e2e-tests/run-wdio.cjs 现状（settings.json 备份还原机制 FIX-TE-04 保持不动）。
- 启动 wdio 前：删除并重建 e2e-tests/.tmp-claude-projects/（从 fixtures/claude-projects/ 递归复制），复制过程中将 fixture 内占位符替换为真实路径（E2E 临时项目目录的真实绝对路径，照既有用例创建项目的路径约定）。
- process.env.SLTERM_CLAUDE_PROJECTS_DIR 指向 .tmp-claude-projects/ 绝对路径（子进程继承——被测 slterminal.exe 后端扫描时读取）。
- .gitignore 追加 .tmp-claude-projects 条目（先 Read 现状照格式追加）。

【TE-03 e2e-tests/test.e2e.ts 新 describe】
- 先 Read 既有用例模式（__slterm_e2e_* helper 用法、agent-status 相关 describe 的选择器断言、__e2e_getTerminalText 终端缓冲断言）。
- 新 describe（如 describe("Claude 历史会话视图")），用例覆盖：
  1. 展开「全部项目历史会话」区 → fixture 会话行展示（data-e2e="agent-history-section-all" / agent-history-row；展开触发扫描）
  2. 标题回退展示：custom-title / ai-title / 首条 prompt 三会话行各显示预期标题；agent-*.jsonl 与 subagents 形态不出现在列表
  3. 搜索过滤：输入关键词 → 仅匹配行保留（data-e2e="agent-history-search"）
  4. 复制恢复命令：右键行 → 菜单点击「复制恢复命令」→ 剪贴板内容断言（browser.execute 读剪贴板或 helper；命令格式 cd '<cwd>' && claude --resume <id>）
  5. 重命名：右键 → 「重命名」→ InputDialog 输入新名提交 → 列表行标题更新 + Node 侧断言副本 .jsonl 文件尾部追加了 {"type":"custom-title",...} 行（Node fs 读 .tmp-claude-projects 副本）
  6. 删除：右键 → 「删除」→ 确认 → 列表行消失 + Node 侧断言副本 .jsonl 文件已不存在
  7. 孤儿行 ✗ 展示 + 双击无反应（无新面板/无页面切换）
  8. 恢复编排：双击普通行（cwd 指向 E2E 临时项目的会话）→ 项目入列断言 + 页面切换断言 + 终端页签出现 + __e2e_getTerminalText() 终端缓冲含 claude --resume <id>；**不断言 claude 成功进入会话**（fixture id 非真实会话，真实成功属人工验证）
- ask 弹窗处理（执行期决策点）：先 grep 既有用例（hooks 注入/删除类）如何处理 dialog.ask；有先例照抄；无先例则在页面内 browser.execute 预置拦截（如覆盖 window 确认逻辑或经 mock 层），并在注释标注方案来源。
- 键盘输入限制照 e2e-tests/CLAUDE.md：InputDialog 文本输入用 .setValue()/.addValue()（WebDriver 元素级输入可行，禁 browser.keys 依赖 OS 级投递）。

【TE-04 用例清单同步】
- .claude/test-inventory.md 更新 E2E 类目：新 describe 条目 + 用例数更新（与实际 it( 计数一致）。

约束：不改 src/ 与 src-tauri/src/ 下任何代码；不改 wdio.conf.ts / helpers.ts（确需改动时停下来报告理由，不顺手改）；fixtures 内 JSONL 内容逐字构造，不运行真实 claude 生成。`,
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
6. npm run build:e2e
7. npm run wdio
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
注意：npm run wdio 实跑耗时较长（含 build:e2e 后的完整 E2E），耐心等待，勿中止；报告中须明确既有用例（含 Agent Status 相关用例）是否全部通过。
`, { label: 'full test suite' })

// === Phase 4: 逐项验证（断言清单唯一真值源）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 6 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/claude-history-view/workflows/verify/stage-06.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
