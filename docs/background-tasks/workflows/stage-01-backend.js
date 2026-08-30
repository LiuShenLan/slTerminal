// =====================================================================
// Stage 01 后端骨架（BE-01 ~ BE-05）
// =====================================================================
// 跨边界契约（checklist.md「跨边界契约」节，双端不各自推断）：
// - IPC 命令：background_tasks_list() -> Vec<BackgroundTaskInfo>（无参）；
//   background_tasks_set_config(task_id, enabled?, interval_sec?) -> Vec<BackgroundTaskInfo>
// - 事件：background-tasks-updated，payload = BackgroundTaskInfo[]
// - DTO 六键（serde camelCase）：taskId/title/enabled/intervalSec/intervalMin/intervalMax，无 default 字段
// - taskId 值集 = ["planBalance", "sessionRefresh"]
// - settings.json 顶层键 "backgroundTasks"（background_tasks::SETTINGS_KEY），子键 per taskId {enabled, intervalSec}
// - 衔接点：set_config_core 调 crate::settings::save_settings_blocking（backend-wiring 提供）
// - 锁序单向：CONFIG_WRITE_LOCK → SETTINGS_SAVE_LOCK，无环
// fix-loop constraints（execution-plan.md）：本 Stage 无特殊纪律，传空串
export const meta = {
  name: 'stage01-backend',
  description: 'F12 后端骨架：background_tasks 注册表/poller 骨架/配置命令 + plan_balance 执行体下沉 + settings 写通道抽取 + 三处注册',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入。
背景：修复要点详见 checklist 对应 ID 条目（先读 docs/background-tasks/checklist.md 再动手）。
补充纪律：并行 agent 不跑资源共享型测试（cargo 系共享 target 目录锁会排队属正常）——本 Stage 两个 agent 均为后端，只做源码改动，测试统一由全量测试 agent 单点跑；Rust 代码遵循 clippy -D warnings 与 rustfmt；临时目录隔离用 tempfile::tempdir + AppDataDirGuard（照 plan_balance 先例）。`

// === Phase 1: 并行重构（agent 间文件零重叠）===
phase('并行重构')
const parallelAgents = [
  {
    label: 'backend-tasks',
    prompt: `你负责 BE-01 / BE-02 / BE-03，先读 docs/background-tasks/checklist.md 中三个 ID 的完整条目（含可照抄代码块），逐条实现：

【BE-01】新建 src-tauri/src/background_tasks/registry.rs：
- 照 checklist BE-01 3.1 完整代码块原样写入：TaskExecutor/TaskDef/TaskRuntime 结构、SETTINGS_KEY 常量、TASKS 静态切片（planBalance: 默认 10s 区间 10-3600 enabled 默认 true 执行体 Some(poll_once_executor)；sessionRefresh: 默认 3s 区间 2-300 enabled 默认 true 执行体 None）、RUNTIMES 数组（与 TASKS 同序）、find()、ResolvedConfig + resolve_task_config（逐字段独立钳制回退）。
- 3.2 同文件 #[cfg(test)] mod tests：tasks_registry_key_set_locked（键集精确 ["planBalance","sessionRefresh"] + 两任务六字段逐条断言）、runtimes_same_length_as_tasks（等长守卫 + 静态初值==def 默认值）、find_hit_and_miss、resolve_task_config 系列 6 例（无文件默认/合法采用/越界回退保留 enabled/非 bool 回退/段非对象回退/损坏 JSON 回退——每例 tempdir + AppDataDirGuard）。
- 注意：registry.rs 引用 crate::plan_balance::poll_once_executor 与 crate::app_dir::app_data_dir——poll_once_executor 由 BE-03 提供，app_dir 已有。若 BE-03 尚未完成导致编译失败属预期，全量测试阶段汇合。

【BE-02】新建 src-tauri/src/background_tasks/mod.rs：
- 照 checklist BE-02 3.1 完整代码块原样写入：pub mod registry;、BackgroundTaskInfo DTO（serde camelCase 六字段）、current_list()、spawn_poller（首轮立即执行 + 每轮末按内存间隔 sleep + 轮首 enabled 检查退出 + running 置位）、start_background_tasks（读盘初始化内存 → enabled 且 executor Some 才 spawn）、CONFIG_WRITE_LOCK、set_config_core（同步函数！顺序写死：白名单校验 → 双 None 拒绝 → 边界校验 → 子键合并读-改-写落盘（调 crate::settings::save_settings_blocking）→ 落盘成功才更新内存 → 返回 current_list()）、background_tasks_list 命令、background_tasks_set_config 命令（spawn_blocking 包 set_config_core + enabled=true 且 executor Some 且 !running 时重 spawn + emit "background-tasks-updated" + 返回清单）、reset_runtimes_for_test。
- 3.2 同文件 #[cfg(test)] mod tests（9 例，每个 set_config 用例首行 reset_runtimes_for_test()）：background_task_info_serde_key_set（六键精确 ["enabled","intervalMax","intervalMin","intervalSec","taskId","title"]）、list_returns_registry_order_with_defaults、set_config_valid_interval_persists_and_updates_memory、set_config_valid_enabled_persists、set_config_subkey_merge_preserves_sibling、set_config_out_of_range_rejected、set_config_unknown_task_rejected、set_config_no_fields_rejected、set_config_disk_memory_consistent。
- 契约红线：事件名逐字 "background-tasks-updated"；SETTINGS_KEY 引用 registry::SETTINGS_KEY 不写字面量；禁止自建 settings.json 第二写通道（只调 save_settings_blocking）。

【BE-03】改造 src-tauri/src/plan_balance/mod.rs：
- 3.1 删除：POLL_INTERVAL_SEC、reset_poll_interval_for_test、resolve_poll_interval、SETTINGS_KEY、INTERVAL_SEC_KEY、DEFAULT/MIN/MAX_INTERVAL_SEC、start_plan_balance_poller、plan_balance_set_interval 命令；同步清理不再使用的 import（AtomicU64/Ordering 等，若无其他消费一并删）。
- 3.2 apply_snapshot 之后新增 poll_once_executor（pub fn，AppHandle 参数，调 poll_once_production(unix_now()) + apply_snapshot）。
- 3.3 poll_once_production 维持 pub(crate) fn 即可（同模块内调用）。
- 3.4 头注释更新：模块职责段删「轮询间隔」描述，注明「轮询编排骨架已上提 background_tasks（F12），本模块保留套餐语义执行体与快照存储」。
- 3.5 删测试 10 例：resolve_poll_interval_* 4 例、set_interval_* 4 例、poll_interval_memory_default_is_60、settings_key_constants_value；保留 14 例零改动（serde 4 + merge_slot 4 + poll_once 5 + get_plan_balance 1）。

完成后报告：每个 ID 的修改摘要 + 文件清单。`,
  },
  {
    label: 'backend-wiring',
    prompt: `你负责 BE-04 / BE-05，先读 docs/background-tasks/checklist.md 中两个 ID 的完整条目，逐条实现：

【BE-04】改造 src-tauri/src/settings.rs：
- 3.1 白名单第 5 键从 crate::plan_balance::SETTINGS_KEY 改 crate::background_tasks::SETTINGS_KEY（数组仍 5 项）；头注释 SEC-11 行与「后端消费型域键名归域模块（plan_balance::SETTINGS_KEY 先例）」注释改写为 background_tasks 口径。
- 3.2 抽取同步写通道 save_settings_blocking（pub(crate) fn，参数 serde_json::Value，返回 Result<(), AppError>）：validate_settings_input → app_data_dir → SETTINGS_SAVE_LOCK 持锁 → create_dir_all → 读现有合并 → to_string_pretty → MAX_PERSIST_BYTES 上限 → NamedTempFile 原子写 + .bak 备份 → persist。内容照 checklist BE-04 3.2 完整代码块原样平移（原 save_settings spawn_blocking 闭包本体）。
- 3.3 save_settings async 命令瘦身为 spawn_blocking 包装（照 checklist 3.3 代码块）；删「spawn_blocking 前快速失败」头注释表述。
- 3.4 测试：save_accepts_plan_balance_key（:568-578）改名 save_accepts_background_tasks_key，payload 改 { "backgroundTasks": { "planBalance": { "enabled": true, "intervalSec": 120 } } }；新增 save_rejects_plan_balance_key（{"planBalance": {...}} → Validation 含「白名单」且不落盘）。其余 26 例零改动。
- 契约红线：save_settings_blocking 是全仓唯一 settings.json 写通道（background_tasks::set_config_core 是第二调用方），禁止在别处再建写通道。

【BE-05】三处注册：
- src-tauri/src/lib.rs：mod app_dir; 后插 mod background_tasks;（字母序）；setup 中 plan_balance::start_plan_balance_poller(app.handle().clone()); 整行替换为 background_tasks::start_background_tasks(app.handle().clone()); // F12 后台定时任务骨架（含套餐余量 poller）；generate_handler 删 plan_balance::plan_balance_set_interval,，在 agent_history::agent_history_read_title, 后插 background_tasks::background_tasks_list, 与 background_tasks::background_tasks_set_config,。
- src-tauri/build.rs：commands 数组删 "plan_balance_set_interval"，"agent_history_read_title", 后插 "background_tasks_list", "background_tasks_set_config",；:16 注释「当前 36 条」改「当前 37 条」。
- src-tauri/capabilities/default.json："allow-agent-history-read-title", 后插 "allow-background-tasks-list", "allow-background-tasks-set-config",；删 "allow-plan-balance-set-interval"。
- 注意：lib.rs 引用 background_tasks 模块由 BE-01/BE-02 提供——编译失败属预期，全量测试阶段汇合。

完成后报告：每个 ID 的修改摘要 + 文件清单。`,
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

// === Phase 4: 逐项验证（断言清单唯一真值源：docs/background-tasks/workflows/verify/stage-01.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 01 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/background-tasks/workflows/verify/stage-01.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
