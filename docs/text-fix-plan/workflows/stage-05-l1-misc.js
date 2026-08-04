// =====================================================================
// Stage 05 L1-外围：fs/notify/history/settings
// =====================================================================
// 清单 docs/text-fix-plan/checklist.md（逐 ID 对照原文）| Stage 划分 docs/text-fix-plan/stages.md
// 断言 docs/text-fix-plan/workflows/verify/stage-05.md（与 fix-loop 同一真值源）
// fix-loop constraints: 本 Stage 生产代码改动仅限 checklist 标注的最小可测性重构（D2 零行为变更，如 EventEmitter trait、ScanRootGuard、app_data_dir 注入、命令内核纯函数化）；其余只改测试
// =====================================================================

export const meta = {
  name: 'stage05-l1-misc',
  description: 'L1 fs/notify/history/settings 命令层与异常分支覆盖',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：
1. compute_conpty_flags 固定 0x7（含 4 条守卫测试），任何 agent 不得修改 ConPTY flags——本 Stage 不涉，仅作提示
2. L4 E2E 不得触碰真实 ~/.claude/projects/（SLTERM_CLAUDE_PROJECTS_DIR env 隔离 fixture）——测试经该 env 时必须在 --test-threads=1 下且用 RAII 恢复
3. C10 契约不可改：slterm-hook-reporter.js 任何代码路径必须 process.exit(0)——本 Stage 不涉，仅作提示
4. cargo test 恒 --test-threads=1（ConPTY 并发 spawn 死锁；env 测试同样依赖）
背景：修复要点详见 checklist 对应 ID 条目（先读再动手）。生产代码改动仅限 checklist 标注的最小可测性重构（D2 零行为变更），其余一律只改测试。settings/projects 命令写 exe 同级 JSON——L1 测试必须经 app_data_dir 注入 tempdir，绝不写真实 exe 目录。并行 agent 各自文件零重叠；重构阶段只做编译级检查（cargo test --no-run），真实执行由全量测试 agent 单点跑。`

// === Phase 1: 并行重构（4 agent，文件零重叠）===
phase('并行重构')
const parallelAgents = [
  {
    label: 'l1-fs',
    prompt: `你负责 HFN-01、HFN-04、HFN-08，触碰文件：src-tauri/src/fs/mod.rs。逐 ID 对照 checklist 原文实施：

【HFN-01】fs write_file_tests 与实现同构。位置 fs/mod.rs:492-632。测试重写 use_crlf 检测与行尾转换逻辑，生产改为恒 CRLF/LF 时期望跟随变（循环断言）。改直接调 fs_write_file 命令，用固定输入/输出字节断言（CRLF 保持/LF 保持/新文件平台默认/混合归一）。

【HFN-04】fs 异常路径未覆盖。位置 fs/mod.rs:221（fs_delete 不存在）、create_dir/delete 沙箱拒绝、TaskJoin panic 映射。补删除不存在路径、root 外拒绝、spawn_blocking panic → AppError 映射三用例。

【HFN-08】fs 测试 as_tauri_state transmute。位置 fs/mod.rs:285-288。测试用 transmute 构造 State<AppState>，UB 风险且脆弱。抽命令内核为纯函数（State 仅做提取），测试调纯函数；或改安全构造。测试区 transmute 必须消除。

完成后报告：每项改动摘要 + 修改文件清单。`,
  },
  {
    label: 'l1-notify',
    prompt: `你负责 HFN-02、HFN-03、HFN-07、HFN-09①，触碰文件：src-tauri/src/notify/mod.rs、src-tauri/src/notify/pool.rs。逐 ID 对照 checklist 原文实施：

【HFN-02】pool.rs:66 替换分支未真正覆盖。位置 pool.rs:66（测试在 pool.rs 测试模块）。p10 测试先手动 pool.remove(&path) 再 insert，insert 内部"已存在→stop 旧 watcher"分支未执行。去掉手动 remove，同 path 直接两次 insert，断言旧 watcher 被 stop。

【HFN-03】FileWatcher::start / notify_watch 零 L1——抽 EventEmitter trait 补测。位置 notify/mod.rs:62-157、214-270。debouncer 创建、watch 注册、事件循环、pause/resume、emit 全部无 L1（原豁免：无 AppHandle）。按 D6 抽 EventEmitter trait（生产实现包 AppHandle emit），L1 用 mock emitter 驱动事件循环；notify_watch 的沙箱校验/pool 交互分支补用例。

【HFN-07】notify Drop 测试固定 sleep(100ms)。位置 notify/mod.rs:567。改轮询等待 thread.is_finished()（2s 超时），消除慢 CI flaky。

【HFN-09①】pool p9_drop 无断言。位置 pool.rs:303-307。drop 测试补线程退出/stop 断言（测试 watcher 记录 stop 调用次数或断言线程退出）。

完成后报告：每项改动摘要 + 修改文件清单。`,
  },
  {
    label: 'l1-history',
    prompt: `你负责 HFN-05、HFN-06、HFN-09②③，触碰文件：src-tauri/src/claude_history/scan.rs、src-tauri/src/claude_history/ops.rs。逐 ID 对照 checklist 原文实施：

【HFN-05】claude_history 命令包装 + IO 降级路径未覆盖。位置 scan.rs:42,49,54,58、ops.rs:43,48,73。命令包装层（spawn_blocking/参数透传）与 metadata 失败 → mtimeMs=0 等 IO 降级分支未测（原豁免"命令包装不直测"）。按 D6 补包装层最小用例 + IO 降级用例（不可读文件 → 降级条目）。

【HFN-06】scan.rs env 无 RAII 清理。位置 scan.rs:163-169。SLTERM_CLAUDE_PROJECTS_DIR set 后 panic 会残留污染环境变量。引入 ScanRootGuard（Drop 时恢复 env）替换手动 set/unset。

【HFN-09②③】scan 命名误导 + ops 空串恒真断言。位置 scan.rs:240-260、ops.rs:139-148。②scan_multiple_sessions_sorted_input_order 改名（不验证顺序——扫描顺序无契约，排序是前端职责）；③ops 空串 UUID 用例改断言错误消息含具体校验文案（msg.contains(bad) 空串恒真）。

完成后报告：每项改动摘要 + 修改文件清单。注意：env 测试依赖 --test-threads=1 门禁（全量测试 agent 保证）；测试内设/测毕恢复（ScanRootGuard）。`,
  },
  {
    label: 'l1-settings',
    prompt: `你负责 SPE-01~06，触碰文件：src-tauri/src/settings.rs、src-tauri/src/projects.rs、src-tauri/src/error.rs。逐 ID 对照 checklist 原文实施：

【SPE-01】settings 全部核心用例未调真实命令。位置 settings.rs:114-498。.bak 备份恢复、原子写入、浅合并、spawn_blocking、TaskJoin 全在 inline 重写测试中虚构，真实 save_settings/load_settings 命令从未被调用。tokio::runtime::Runtime::block_on 调真实命令；app_data_dir() 抽为可注入（测试注 tempdir）；覆盖备份恢复/浅合并不擦他段/原子写。

【SPE-02】projects.rs 命令包装层未覆盖。位置 projects.rs:64-81。新增用例直接 block_on 调 save_projects/load_projects（app_data_dir 注入 tempdir）。

【SPE-03】error.rs 三个 From 实现未覆盖。位置 error.rs:49-63。补 serde_json::Error/git2::Error/tokio::task::JoinError → AppError 三转换用例（变体 + 消息契约）。

【SPE-04】app_data_dir() 错误分支未覆盖。位置 settings.rs:10-20。current_exe 失败、exe 无父目录两分支未测。路径解析抽纯函数注入可失败点，补两错误分支。

【SPE-05】persist 失败映射未覆盖。位置 projects.rs:25-28、settings.rs:63-64。补 NamedTempFile::persist 失败 → AppError 映射用例（目标路径只读/冲突构造）。

【SPE-06】settings 边界 + current_exe 依赖说明。位置 settings.rs:482-497。①补并发写/只读文件/超大 JSON 边界用例（可行范围内）；②app_data_dir 依赖真实 current_exe 的测试加注释说明；lib.rs run() 维持豁免 → DOC-01（本 Stage 无需动作）。

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

// === Phase 4: 逐项验证（断言清单唯一真值源：docs/text-fix-plan/workflows/verify/stage-05.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage05 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/text-fix-plan/workflows/verify/stage-05.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
