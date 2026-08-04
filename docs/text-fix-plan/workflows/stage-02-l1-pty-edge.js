// =====================================================================
// Stage 02 L1-PTY：reader/shell/state 边界
// =====================================================================
// 清单 docs/text-fix-plan/checklist.md（逐 ID 对照原文）| Stage 划分 docs/text-fix-plan/stages.md
// 断言 docs/text-fix-plan/workflows/verify/stage-02.md（与 fix-loop 同一真值源）
// fix-loop constraints: 本 Stage 生产代码改动仅限 checklist 标注的最小可测性重构（D2 零行为变更）；compute_conpty_flags 及 4 条守卫测试零改动
// =====================================================================

export const meta = {
  name: 'stage02-l1-pty-edge',
  description: 'L1 pty reader strip/ring buffer/shell 回退/沙箱边界覆盖',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：
1. compute_conpty_flags 固定 0x7（含 4 条守卫测试），任何 agent 不得修改 ConPTY flags——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮
2. L4 E2E 不得触碰真实 ~/.claude/projects/（SLTERM_CLAUDE_PROJECTS_DIR env 隔离 fixture）——本 Stage 不涉 L4，仅作提示
3. C10 契约不可改：slterm-hook-reporter.js 任何代码路径必须 process.exit(0)——本 Stage 不涉，仅作提示
4. cargo test 恒 --test-threads=1（ConPTY 并发 spawn 死锁）
背景：修复要点详见 checklist 对应 ID 条目（先读再动手）。生产代码改动仅限 checklist 标注的最小可测性重构（D2 零行为变更），其余一律只改测试。并行 agent 各自文件零重叠；重构阶段只做编译级检查（cargo test --no-run），真实执行由全量测试 agent 单点跑。`

// === Phase 1: 并行重构（3 agent，文件零重叠：reader.rs / shell.rs / state.rs）===
phase('并行重构')
const parallelAgents = [
  {
    label: 'pty-reader',
    prompt: `你负责 PTY-04、PTY-12，触碰文件：src-tauri/src/pty/reader.rs。逐 ID 对照 checklist 原文实施：

【PTY-04】strip_conpty_startup 未覆盖分支。位置 reader.rs:166-235。非 Windows 原样返回分支（cfg!(windows) 运行时分支）标注"由 cfg 守护，Windows CI 不可达"并补 cfg!(windows) 常量断言；OSC 1/3/4/9 保留、CSI 3J 补保留用例。

【PTY-12】reader_loop I/O 编排——评估抽取 + 残余豁免文档化。位置 reader.rs:31-154。按 D6 评估可抽取的决策点（如"channel 断开→写 ring buffer"分流表、EOF 处理决策），能抽为注入参数的补 L1；确不可抽的残余分支在 src-tauri/src/pty/CLAUDE.md 补豁免标注草稿（Stage 17 统一收编为豁免表，本 Stage 只留草稿）。二选一必有其一产出。

完成后报告：每项改动摘要 + 修改文件清单。注意：strip_conpty_startup 修改后务必跑全部 strip 相关测试确认不误杀正常输出。`,
  },
  {
    label: 'pty-shell',
    prompt: `你负责 PTY-06、PTY-10、PTY-13③，触碰文件：src-tauri/src/pty/shell.rs。逐 ID 对照 checklist 原文实施：

【PTY-06】resolve_shell_info 自动检测回退顺序未测。位置 shell.rs:94-127。构造可控 PATH（tempdir 放假 exe：只有 pwsh、只有 powershell、都没有）验证三档回退顺序与命中（pwsh→powershell→cmd）。

【PTY-10】resolve_shell 回退 + 白名单 PATH 解析后仍非法。位置 shell.rs:68-86、283-322。补回退顺序用例 + 白名单拒绝用例（用户指定 shell 经 PATH 解析成功但非 pwsh/powershell/cmd 仍拒绝）。

【PTY-13③】which_full_path PATH 顺序未测。位置 shell.rs:173-183。补 PATH 多目录返回第一个匹配、大小写边界用例。

完成后报告：每项改动摘要 + 修改文件清单。注意：resolve_shell_info 返回的 ShellInfo.program 必须是完整路径——测试构造假 exe 时勿破坏该语义。`,
  },
  {
    label: 'pty-state',
    prompt: `你负责 PTY-05、PTY-11、PTY-13②，触碰文件：src-tauri/src/state.rs。逐 ID 对照 checklist 原文实施：

【PTY-05】ring_buffer_append 无换行长行淘汰边界。位置 state.rs:201-218。map_or 右侧 or 分支（1024 字节内无换行则按 1024 原量淘汰）未测。补无换行超长行淘汰用例（恰好 1024、超 1024、含换行三边界），断言 buf 长度 ≤ 容量、剩余尾部正确、未 panic。

【PTY-11】validate_path_within_root 相对路径 .. 穿越未测。位置 state.rs:138-177。补 .. 穿越沙箱根拒绝、相对路径正常放行两用例（D7 防复发）。

【PTY-13②】canonicalize_or_ancestor relative 路径分支未测。位置 state.rs:100-131。补直接传 relative path 的边界用例。

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
1. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
2. cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 4: 逐项验证（断言清单唯一真值源：docs/text-fix-plan/workflows/verify/stage-02.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage02 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/text-fix-plan/workflows/verify/stage-02.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
