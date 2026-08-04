// =====================================================================
// Stage 04 L1-hooks：信号链与注入命令
// =====================================================================
// 清单 docs/text-fix-plan/checklist.md（逐 ID 对照原文）| Stage 划分 docs/text-fix-plan/stages.md
// 断言 docs/text-fix-plan/workflows/verify/stage-04.md（与 fix-loop 同一真值源）
// fix-loop constraints: 本 Stage 生产代码改动仅限 checklist 标注的最小可测性重构（D2 零行为变更，如 emit 抽注入参数、路径可注入 impl、run_one_tick 拆分、#[cfg(test)] 重置钩子）；assets/slterm-hook-reporter.js 零改动（C10）
// =====================================================================

export const meta = {
  name: 'stage04-l1-hooks',
  description: 'L1 hooks 信号链/注入命令/watcher 事件循环覆盖',
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
2. L4 E2E 不得触碰真实 ~/.claude/projects/（SLTERM_CLAUDE_PROJECTS_DIR env 隔离 fixture）——本 Stage 不涉，仅作提示
3. C10 契约不可改：slterm-hook-reporter.js 任何代码路径必须 process.exit(0)——绝对不要修改 assets/slterm-hook-reporter.js（含 SCRIPT_VERSION）
4. cargo test 恒 --test-threads=1（ConPTY 并发 spawn 死锁）
背景：修复要点详见 checklist 对应 ID 条目（先读再动手）。生产代码改动仅限 checklist 标注的最小可测性重构（D2 零行为变更），其余一律只改测试。注入/卸载命令走真实 ~/.claude 路径——L1 测试必须用 tempdir 注入路径，绝不读写真实用户 home。并行 agent 各自文件零重叠；重构阶段只做编译级检查（cargo test --no-run），真实执行由全量测试 agent 单点跑。`

// === Phase 1: 并行重构（3 agent，文件零重叠）===
phase('并行重构')
const parallelAgents = [
  {
    label: 'hooks-signal',
    prompt: `你负责 HUK-01、HUK-04、HUK-09，触碰文件：src-tauri/src/hooks/signal.rs、src-tauri/src/hooks/mod.rs。逐 ID 对照 checklist 原文实施：

【HUK-01】process_signal_file 全流程零覆盖。位置 signal.rs:52-79。读文件 → parse → emit("hook-event") → 删文件全链路（含 emit 失败仍删文件）未测。按 D6 将 emit 抽为注入参数（闭包/trait），tempdir 构造信号文件验证读→emit→删全流程 + emit 失败仍删除 + 非法 JSON 降级。

【HUK-04】start_signal_watcher 全局启动零 L1。位置 mod.rs:63-84。幂等启动（已启动跳过）、WATCHER 静态实例管理未测。加 #[cfg(test)] 重置钩子，补首次启动/重复启动幂等用例。

【HUK-09】serde camelCase contains 弱断言。位置 mod.rs:98-144、signal.rs:144-174。json.contains("panelId") 不防字段值/类型错误。改序列化→反序列化往返精确断言 + 键集合精确匹配（HookInjectionStatus/HookEventPayload 全字段）。

完成后报告：每项改动摘要 + 修改文件清单。`,
  },
  {
    label: 'hooks-inject',
    prompt: `你负责 HUK-02、HUK-03、HUK-08、HUK-11，触碰文件：src-tauri/src/hooks/inject.rs、src-tauri/src/hooks/watcher.rs。逐 ID 对照 checklist 原文实施：

【HUK-02】hooks_inject/uninstall/injection_status 三命令零 L1。位置 inject.rs:191-274、280-351、358-423。settings.json merge、非法 JSON 中止、版本比对、目录删除等核心逻辑无 L1 回归。按 D2/D6 抽 inject_impl(settings_path, script_dir) 等路径可注入的同步函数，tempdir 驱动三命令场景（注入/幂等/非法中止/卸载混组保用户 handler/状态三态）。

【HUK-03】HookSignalWatcher::start 双通道事件循环零 L1。位置 watcher.rs:46-136。notify 实时 + 3s 轮询补漏、notify 降级 warn、目录删除重建恢复等 win10 实证兜底逻辑未测。按 D6 拆 run_one_tick 可测单元或写临时目录真实启动 watcher 的集成测试（轮询补漏消费残留文件、目录重建后恢复）。

【HUK-08】inject_adds_10_events 弱断言。位置 inject.rs:651-661。只检查事件键存在，未断言 handler 的 type/matcher/timeout/command 字段。改结构断言（每事件 handler 数组含 {type:"command", timeout:5, command 含 slterm-hook-reporter}，D7 键集合精确匹配）。

【HUK-11】watcher stop 无结束断言 + handler_contains_slterm 非字符串分支。位置 watcher.rs:337-348、inject.rs:98-102。①stop 测试补 thread.is_finished() 断言；②补 command 为非字符串（number/null）时 handler_contains_slterm 返回 false 用例。

完成后报告：每项改动摘要 + 修改文件清单。注意：绝对不要修改 assets/slterm-hook-reporter.js（C10）；勿削弱轮询补漏——它是 win10 实证 watcher 静默失效的兜底。`,
  },
  {
    label: 'hooks-usage-config',
    prompt: `你负责 HUK-05、HUK-06、HUK-07、HUK-10，触碰文件：src-tauri/src/hooks/usage.rs、src-tauri/src/hooks/config.rs。逐 ID 对照 checklist 原文实施：

【HUK-05】hooks_context_usage 命令包装未覆盖。位置 usage.rs:34-42。补命令包装层用例（参数透传 transcriptPath、None/Some 返回映射）。

【HUK-06】config 读写包装 + IO 异常分支未覆盖。位置 config.rs:66-68、94、121、145-147、154-176、182-207。home_dir() 失败、persist 失败、命令包装参数透传未测。补 IO 异常分支（注入失败点或用不可写路径）+ 包装层透传用例。

【HUK-07】config user 层测试依赖真实 home 目录。位置 config.rs:233-238。user 层路径解析测试读真实 dirs::home_dir()，环境污染风险。home 解析抽为可注入参数，测试注入 tempdir。

【HUK-10】P2-TE-05 与 scan_transcript_usage 用例重复。位置 usage.rs:371-483 vs 256-318。两组用例覆盖同一纯函数路径，重复维护。去重合并；保留的一组改为经命令包装层调用（与 HUK-05 协同）。

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

// === Phase 4: 逐项验证（断言清单唯一真值源：docs/text-fix-plan/workflows/verify/stage-04.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage04 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/text-fix-plan/workflows/verify/stage-04.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
