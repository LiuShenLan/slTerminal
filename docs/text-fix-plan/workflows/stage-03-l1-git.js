// =====================================================================
// Stage 03 L1-GIT：命令层重写 + 拆分
// =====================================================================
// 清单 docs/text-fix-plan/checklist.md（逐 ID 对照原文）| Stage 划分 docs/text-fix-plan/stages.md
// 断言 docs/text-fix-plan/workflows/verify/stage-03.md（与 fix-loop 同一真值源）
// fix-loop constraints: 本 Stage 生产代码改动仅限 checklist 标注的最小可测性重构（D2 零行为变更，如抽 rollback_in_spawn_blocking 同步函数）；其余只改测试
// pipeline 说明（stages.md 偏离规则豁免）：GIT-12 改变测试文件布局，后续项依赖新布局——git-restructure 先行串行，git-commands/git-units 随后并行
// =====================================================================

export const meta = {
  name: 'stage03-l1-git',
  description: 'L1 git 五命令命令层测试重写 + 单文件拆分 + 弱断言精确化',
  phases: [
    { title: '串行重构' },
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：
1. compute_conpty_flags 固定 0x7（含 4 条守卫测试），任何 agent 不得修改 ConPTY flags——本 Stage 不涉，仅作提示
2. L4 E2E 不得触碰真实 ~/.claude/projects/（SLTERM_CLAUDE_PROJECTS_DIR env 隔离 fixture）——本 Stage 不涉，仅作提示
3. C10 契约不可改：slterm-hook-reporter.js 任何代码路径必须 process.exit(0)——本 Stage 不涉，仅作提示
4. cargo test 恒 --test-threads=1（ConPTY 并发 spawn 死锁；SLTERM_CLAUDE_PROJECTS_DIR 类 env 测试同样依赖）
背景：修复要点详见 checklist 对应 ID 条目（先读再动手）。生产代码改动仅限 checklist 标注的最小可测性重构（D2 零行为变更），其余一律只改测试。8.3 短名坑：init_temp_repo 保持 dunce::canonicalize（CI runner 的 %TEMP% 是短名，git2 workdir 返回长名，strip_prefix 必须统一）。git 测试依赖系统 git CLI——测试自包含（init_temp_repo 内设仓库局部 config）。重构阶段只做编译级检查（cargo test --no-run），真实执行由全量测试 agent 单点跑。`

// === Phase 1: 串行重构（git-restructure 先行：拆分测试文件布局，下游依赖）===
phase('串行重构')
const restructureResult = await agent(`${PREAMBLE}

你负责 GIT-12、GIT-11、GIT-06（先行拆分），触碰文件：src-tauri/src/git/mod.rs（测试区拆分）、拆分新测试文件（tests/ 下 status/diff/at_head/rollback/unstage 独立文件）、src-tauri/tests/ci_config_tests.rs（新建）、共享 test_utils。逐 ID 对照 checklist 原文实施：

【GIT-12】88 条单文件拆分 + 工厂提取。位置 git/mod.rs:584-2718。按命令拆分为独立测试文件（status/diff/at_head/rollback/unstage），init_temp_repo/commit_file 提取共享 test_utils。本项先行——后续 git-commands/git-units 的新测试落位到拆分后新文件。

【GIT-11】ci_l1_uses_single_test_thread 领域污染迁移。位置 git/mod.rs:2000-2011。迁移至 src-tauri/tests/ci_config_tests.rs（新建），git 域测试文件只留 git 用例。

【GIT-06】测试未隔离系统 git 全局配置。位置 git/mod.rs:594-636（init_temp_repo/commit_file）。init_temp_repo 内设仓库局部 core.autocrlf=false、core.safecrlf=false、init.defaultBranch=main。需要 autocrlf=true 场景的测试再单独覆盖。

完成后报告：拆分文件清单 + 工厂位置 + 每项改动摘要。`, { label: 'git-restructure' })

if (!restructureResult) {
  // 前序失败短路，不跑下游——记录空结果继续走测试/verify 让全量测试暴露问题
  console.log('git-restructure 未返回（被跳过或 API 错误），下游并行仍执行')
}

// === Phase 2: 并行重构（文件零重叠：commands 碰命令层测试文件 + mod.rs 源码最小抽函数；units 碰 status/diff 测试文件）===
phase('并行重构')
const parallelAgents = [
  {
    label: 'git-commands',
    prompt: `你负责 GIT-01、GIT-02、GIT-03、GIT-09、GIT-10，触碰文件：src-tauri/src/git/mod.rs（源码最小抽函数）、拆分后 rollback/unstage/at_head 测试文件。逐 ID 对照 checklist 原文实施：

【GIT-01】五命令补命令层测试 + 重写 inline 假测试。位置 git/mod.rs:127-582（五命令）、2136-2703（git_rollback_*/git_unstage_*/git_file_at_head_* 测试）。①git_status/git_diff/git_file_at_head/git_rollback/git_unstage 的 State 注入、路径沙箱、spawn_blocking、错误消息契约零命令层测试——构造最小 AppState + block_on await 真实命令（每命令 ≥3 条：happy/沙箱拒绝/错误契约）；②inline 重写 git2 调用序列的测试改为调真实命令，保留的 git2 行为测试标注"底层原语"。

【GIT-02】git_rollback_two_step_* 7 条验证已废弃实现。位置 git/mod.rs:2421-2621。生产已改为 std::fs::write(blob) + index.add_path + index.write，测试仍验证废弃的 reset_default + checkout_index 两步法。删除或重写为当前命令路径（D3 测试对齐实现）。

【GIT-03】git_status_non_renamed_old_path_is_none 假测试。位置 git/mod.rs:1700-1739。循环内 continue 跳过 renamed 条目后再断言 oldPath 为 none——条件恒真，永不可失败。重写为构造非 renamed 条目断言 oldPath === null、renamed 条目断言 oldPath 为旧路径（一手证据：生产已开 renames 检测，git/mod.rs:145-151）。

【GIT-09】git_file_at_head_unborn_branch_err 未调被测函数。位置 git/mod.rs:2145-2157。只验证 git2::Repository::head() 返回 UnbornBranch。改调真实 git_file_at_head 命令，断言 AppError::Git 消息含"HEAD 中不存在"。

【GIT-10】五命令沙箱拒绝分支未覆盖。位置 git/mod.rs:139、229-231、407、472、550。随 GIT-01 命令层测试补齐五命令 validate_path_within_root 拒绝用例（SEC-01，D7）。

完成后报告：每项改动摘要 + 修改文件清单。`,
  },
  {
    label: 'git-units',
    prompt: `你负责 GIT-04、GIT-05、GIT-07、GIT-08，触碰文件：拆分后 status/diff 测试文件（不碰 git/mod.rs 源码）。逐 ID 对照 checklist 原文实施：

【GIT-04】status_to_str conflict 分支未覆盖。位置 git/mod.rs:42-43。补 git2::Status::CONFLICTED → "conflict" 表驱动用例。

【GIT-05】compute_diff_hunks 三处边界未覆盖。位置 git/mod.rs:315-320（修改后多余新增行）、361-363（prev_was_del flush）、264-265（非 UnbornBranch HEAD 错误）。三处各补一条精确 hunk 断言用例：替换+插入场景断言 added hunk 的 old_start=0；连续删除组后接新增触发 flush；corrupt refs 或锁文件模拟 HEAD 读取失败断言 Err。

【GIT-07】git_status 弱断言 any(...) 精确化。位置 git/mod.rs:691-862（五条）。改精确断言（路径集合 + 状态串 + 条目数，D7 payload 键集合精确匹配同款思路）。

【GIT-08】名实不符改名 + .gitignore 时序 + git 版本声明。位置 git/mod.rs:1109-1262（git_diff_returns_hunks 等四条）、771-862（.gitignore 磁盘时序）。①四条 diff 测试名暗示精确验证实为存在性断言——改名或补精确断言；②.gitignore 用例改 git2 内存 ignore 规则（add_ignore_rule）消除磁盘写入时序；③系统 git CLI 最低版本声明由 Stage 17 DOC-04 收编到模块 CLAUDE.md，本 Stage 在测试注释标注即可。

完成后报告：每项改动摘要 + 修改文件清单。`,
  },
]
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 4: 全量测试（命令相互独立，并行启动执行，收集全部结果）===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:\\data\\learn\\code\\slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
2. cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 5: 逐项验证（断言清单唯一真值源：docs/text-fix-plan/workflows/verify/stage-03.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage03 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/text-fix-plan/workflows/verify/stage-03.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
总则：不仅核对字面断言，还须 Read 代码判断实现是否达成断言意图——字面通过但意图未达判 partial 并说明理由。注意 git 测试已拆分至新文件——断言中的文件路径以拆分后布局为准。
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

return { restructureResult, refactorResults, testResult, verifyResult }
