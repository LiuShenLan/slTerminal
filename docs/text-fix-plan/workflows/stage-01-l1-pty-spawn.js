// =====================================================================
// Stage 01 L1-PTY：spawn.rs 校验与可测性重构
// =====================================================================
// 清单 docs/text-fix-plan/checklist.md（逐 ID 对照原文）| Stage 划分 docs/text-fix-plan/stages.md
// 断言 docs/text-fix-plan/workflows/verify/stage-01.md（与 fix-loop 同一真值源）
// fix-loop constraints: 本 Stage 生产代码改动仅限 checklist 标注的最小可测性重构（D2 零行为变更）；compute_conpty_flags 及 4 条守卫测试零改动
// 人工验证点 M1：Stage 完成后主 agent 需 npx tauri build --debug --no-bundle 构建产物实测真实 claude 会话（滚轮/键盘输入/Ink 渲染无回归）
// =====================================================================

export const meta = {
  name: 'stage01-l1-pty-spawn',
  description: 'L1 pty spawn.rs 校验/SEC-08 归属/Job Object 覆盖 + 可测性重构',
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
背景：修复要点详见 checklist 对应 ID 条目（先读再动手）。本 Stage 单 agent 同文件串行（spawn.rs 一处文件，避免并行冲突）；生产代码改动仅限 checklist 标注的最小可测性重构（D2 零行为变更），其余一律只改测试。重构阶段只做编译级检查（cargo test --no-run / cargo build），真实执行由全量测试 agent 单点跑。`

// === Phase 1: 并行重构（单 agent）===
phase('并行重构')
const parallelAgents = [
  {
    label: 'pty-spawn',
    prompt: `你负责 PTY-01/02/03/07/08/09/13①，触碰文件：src-tauri/src/pty/spawn.rs、src-tauri/tests/pty_integration_tests.rs。逐 ID 对照 checklist 原文实施：

【PTY-01】Job Object 孤儿防护零 L1 覆盖。位置 spawn.rs:1185-1263（add_to_job_object/create_and_assign_job）、706-714（JobHandle::drop）。CreateJobObjectW/SetInformationJobObject/AssignProcessToJobObject 参数与 KILL_ON_JOB_CLOSE 设置、JobHandle Drop 全部无测试。按 D2 抽纯逻辑（job_name 构造、limit flags 计算）补 L1 单测；L4 部分（杀 slterminal.exe 后检查子进程残留）由 E2E-12 负责，本 Stage 不做。

【PTY-02】pty_spawn 校验路径零覆盖。位置 spawn.rs:756-970（762-767 尺寸超限、770-772 shell 白名单、775-781 cwd 沙箱）。抽 validate_spawn_request 纯函数（尺寸/白名单/cwd 三校验），补边界用例：cols/rows 超 i16::MAX 拒绝、非法 shell 拒绝、cwd 越界拒绝、cwd 在根内放行；命令层最小集成测试（构造 AppState + await 调用）或经该纯函数。

【PTY-03】pty_write/resize/kill/reattach + SEC-08 归属校验零覆盖。位置 spawn.rs:977-1183。四命令均含 panelId 归属校验。抽 validate_session_ownership 纯函数（SEC-08），补归属放行/拒绝用例（D7 防复发）。

【PTY-07】build_cmdline 引号处理未测。位置 spawn.rs:81-99。补含空格路径、含空格参数、含制表符参数、无空格不加引号、空 args 用例。

【PTY-08】spawn_conpty_child 仅集成覆盖。位置 spawn.rs:398-459。可纯化部分（命令行/环境块构造）抽函数补单测；纯 Win32 调用部分标注"由 pty_spawn_custom_conpty 集成测试 + CI 守卫"。

【PTY-09】ConPtyMaster::resize HPCON invalid 分支未覆盖。位置 spawn.rs:201-217。构造 invalid HPCON 状态断言 resize 静默成功且 size 更新（不调 Win32 API）。

【PTY-13①】spawn.rs 测试清理重复（1365-1371、1399-1403、1426-1430 三处相同清理块）。抽 cleanup_session(pty_state, sid) 测试辅助函数替换三处。

完成后报告：每项改动摘要 + 修改文件清单。注意：不得修改 compute_conpty_flags 及其 4 条守卫测试；新增 #[cfg(windows)] 只允许出现在 spawn.rs（硬约束 #9）。`,
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

// === Phase 4: 逐项验证（断言清单唯一真值源：docs/text-fix-plan/workflows/verify/stage-01.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage01 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/text-fix-plan/workflows/verify/stage-01.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
