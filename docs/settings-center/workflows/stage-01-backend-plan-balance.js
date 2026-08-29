// =====================================================================
// Stage 01 — 后端 plan_balance 动态间隔（SC-BE-01..04）
// =====================================================================
// 改动项: SC-BE-01 模块级间隔原子量+动态轮询循环 / SC-BE-02 plan_balance_set_interval
//         专用命令 / SC-BE-03 三处注册 / SC-BE-04 域键名常量归域
// 分工: 单 agent（mod.rs 同文件串行 + 三处注册）
// 门禁: clippy + rustfmt --check + cargo test（--test-threads=1）
// fix-loop 调用约束: args.constraints 传
//   "Rust 后端 Stage：只改 src-tauri/ 下五个文件（plan_balance/mod.rs、settings.rs、
//    lib.rs、build.rs、capabilities/default.json）"
// =====================================================================

export const meta = {
  name: 'stage01-backend-plan-balance',
  description: 'Stage 01: plan_balance 模块级间隔原子量 + set_interval 命令 + 三处注册 + 域键名归域（F11 后端）',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
背景：修复要点详见 checklist 对应 ID 条目（先读再动手）。动手前先 Read docs/settings-center/checklist.md 中你负责的条目全文，严格按「修复步骤」执行（代码块为照抄级，禁止自行另设计）。`

// === Phase 1: 并行重构（单 agent Stage；重构 agent 不跑全量测试，统一由全量测试 agent 单点跑）===
phase('并行重构')
const parallelAgents = [
  { label: 'be-plan-balance', prompt: `你负责 SC-BE-01..04（全部在 src-tauri/ 下，逐 ID 照 docs/settings-center/checklist.md 条目执行）：

【SC-BE-01】src-tauri/src/plan_balance/mod.rs：
- :66 SNAPSHOT 旁新增 static POLL_INTERVAL_SEC: AtomicU64（初值 DEFAULT_INTERVAL_SEC；import std::sync::atomic::{AtomicU64, Ordering}）
- :186-200 start_plan_balance_poller 改造：启动时从 resolve_poll_interval() 初始化内存值；弃 tokio::time::interval，改 loop { poll; sleep(内存值) }——首轮立即语义（D8）保留；改造后完整代码照 checklist 抄写
- 测试模块加 #[cfg(test)] pub(crate) fn reset_poll_interval_for_test()（照 :68-71 reset_snapshot_for_test 先例）+ 新用例 poll_interval_memory_default_is_60（用例内 store 回 60 防串扰）

【SC-BE-02】mod.rs refresh 命令（:210-220）后新增 plan_balance_set_interval（#[tauri::command] async）：顺序写死 校验 10-3600 → crate::settings::save_settings 复用落盘 → POLL_INTERVAL_SEC.store——完整代码照 checklist 抄写（越界 Err 文案、serde_json::json! payload 形态均照抄）
- 测试模块新增 4 例（AppDataDirGuard 注入 tempdir 直调命令 fn，每例首行 reset_poll_interval_for_test()）：set_interval_valid_persists_and_updates_memory（120 → 磁盘+内存双断言）/ set_interval_below_min_rejected（5 → Validation+磁盘无文件+内存不变）/ set_interval_above_max_rejected（9999 同）/ set_interval_disk_memory_consistent

【SC-BE-03】三处注册：
- src-tauri/src/lib.rs generate_handler! 尾部 "plan_balance::refresh_plan_balance," 后加 "plan_balance::plan_balance_set_interval,"
- src-tauri/build.rs 尾部 "refresh_plan_balance", 后加 "plan_balance_set_interval",
- src-tauri/capabilities/default.json :53 "allow-refresh-plan-balance" 行末补逗号后加 "allow-plan-balance-set-interval"（原 :53 为数组末位无逗号，必须先补）

【SC-BE-04】域键名归域：
- mod.rs 常量区加 pub(crate) const SETTINGS_KEY: &str = "planBalance"; 与 const INTERVAL_SEC_KEY: &str = "intervalSec";（SC-BE-02 命令代码引用这两个常量——按 checklist 保持引用一致）
- resolve_poll_interval 内 root.get("planBalance")?.get("intervalSec") 改 root.get(SETTINGS_KEY)?.get(INTERVAL_SEC_KEY)
- src-tauri/src/settings.rs :18-24 白名单 "planBalance" 改 crate::plan_balance::SETTINGS_KEY（数组仍 5 项），注释按 checklist 决策口径更新（前端消费型四键保留字面量集中于此，后端消费型归域先例）
- mod.rs 测试新增 settings_key_constants_value（两常量值断言防漂移）

自查（单 agent Stage 允许跑测试）：cargo check --manifest-path src-tauri/Cargo.toml 编译过 + cargo test --manifest-path src-tauri/Cargo.toml plan_balance -- --test-threads=1 绿 + cargo test --manifest-path src-tauri/Cargo.toml settings -- --test-threads=1 绿。` },
]
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 2: 全量测试（命令相互独立，并行启动执行，收集全部结果）===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:\\data\\learn\\code\\slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
2. cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
3. cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证（断言清单唯一真值源：docs/settings-center/workflows/verify/stage-01.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 01 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/settings-center/workflows/verify/stage-01.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
