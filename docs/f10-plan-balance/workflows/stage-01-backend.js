// =====================================================================
// F10 编码套餐余量展示 — Stage 01 后端 plan_balance 模块（PB-BE-01~08）
// =====================================================================
// 清单真值源：docs/f10-plan-balance/checklist.md（六段式 + 决策记录 D1-D16/U1-U3）
// 跨边界契约（写死，见 stages.md 头部）：
//   命令 get_plan_balance / refresh_plan_balance（无参，返回 Vec<PlanBalanceInfo>）
//   事件 plan-balance-updated，payload = PlanBalanceInfo[]
//   三处注册：lib.rs generate_handler! + build.rs commands + capabilities allow-<cmd>
// Stage 特殊纪律：无（fix-loop 调用时 args.constraints 传空串）
// =====================================================================

export const meta = {
  name: 'f10-stage01-backend',
  description: 'F10 Stage 01：后端 plan_balance 模块（来源/查询注册表 + 轮询推送 + 双命令）',
  phases: [
    { title: '后端实现' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
背景：实现依据 = docs/f10-plan-balance/checklist.md 对应 ID 条目（先读再动手）；头部「决策记录」D1-D16 全部写死，不做设计判断。`

// === Phase 1: 后端实现（单 agent 顺序——模块全新建，DTO/trait 被全部文件引用，
//     并行拆分无法各自编译验证，见 stages.md Stage 01 分工说明）===
phase('后端实现')
const parallelAgents = [
  {
    label: 'backend',
    prompt: `你负责 PB-BE-01 ~ PB-BE-08（顺序执行）。先 Read docs/f10-plan-balance/checklist.md 全文（决策记录 D1-D16 + 八条条目六段式），条目内代码骨架**照抄适配**，不另行设计；骨架中 \`#[cfg(test)]\` 测试块按各条目「测试同步」节补齐全部用例。

【PB-BE-01】src-tauri/Cargo.toml：tokio features 加 "time"；dependencies 段尾加 ureq = { version = "3", features = ["json"] }。cargo build 验证拉取编译通过。
【PB-BE-03】新建 src-tauri/src/plan_balance/source.rs（照 checklist 骨架）：PlanSource trait（source_id + resolve）、ClaudeUserSettingsSource、静态切片 SOURCES、resolve_env 纯函数、home_dir + HomeDirGuard（照 hooks/claude/mod.rs 模式自建，**禁止跨模块调用** hooks::claude，D2）。测试 8 例。
【PB-BE-05】新建 src-tauri/src/plan_balance/deepseek.rs（照骨架）：DeepSeekQuery + parse_deepseek_balance 纯函数。测试 6 例。
【PB-BE-06】新建 src-tauri/src/plan_balance/kimi.rs（照骨架）：KimiQuery + parse_kimi_usages + remaining_percent + parse_window。测试 10 例。
【PB-BE-04】新建 src-tauri/src/plan_balance/query.rs（照骨架）：PlanQuery trait、QUERIES 静态切片（注册序 deepseek,kimi）、normalize_base_url、find_query_by_url 参数化、http_agent 工厂、query_err（消息只含 planId+类别）。测试 9 例。
【PB-BE-02】新建 src-tauri/src/plan_balance/mod.rs（照骨架）：DTO 四结构 + FetchOutcome、模块级静态 SNAPSHOT、merge_slot、poll_once_with、resolve_poll_interval（AppDataDirGuard 可测）、start_plan_balance_poller、命令 get_plan_balance / refresh_plan_balance。测试含 serde 键集合精确匹配（token 红线守卫）+ merge_slot 4 例 + poll_once 5 例 + resolve_poll_interval 4 例 + get_plan_balance 1 例。
【PB-BE-07】三处注册 + settings 白名单：
  1. src-tauri/src/lib.rs——mod 区 projects 后插 mod plan_balance;；setup 块 hooks::reinject_statusline_on_startup(); 后加 plan_balance::start_plan_balance_poller(app.handle().clone());；generate_handler! 末尾加两条命令
  2. src-tauri/build.rs——commands 数组末尾加两条，注释「当前 34 条」改「当前 36 条」
  3. src-tauri/capabilities/default.json——permissions 末尾加 allow-get-plan-balance / allow-refresh-plan-balance
  4. src-tauri/src/settings.rs:17——白名单 [&str; 4] 改 [&str; 5] 加 "planBalance"，:13-16 注释同步；tests 加 save_accepts_plan_balance_key 1 例
【PB-BE-08】新建 src-tauri/src/plan_balance/CLAUDE.md（按根 CLAUDE.md「子文件模板」成文，覆盖 checklist PB-BE-08 列出的全部要点：存在理由/U2 静态切片偏离理由/D4/D5/D6/D2 决策、token 红线、URL 归一化不加 trim、kimi 字符串口径、测试模式与豁免指向）；.claude/CLAUDE.md 模块索引表 src-tauri/src/notify 行后加一行 | src-tauri/src/plan_balance | ../src-tauri/src/plan_balance/CLAUDE.md |。

【红线（违反即返工）】
- token 不出后端：DTO 无 token 字段；plan_balance 模块内所有 tracing!/Err 构造消息禁止插值 token 与 Authorization 头
- ureq 3 API 以 checklist D10 为准（Agent::config_builder().timeout_global / .header() / .call() / into_body().read_json()，json feature）
- URL 归一化只小写化 + 去尾斜杠（不加 trim，规格字面）
- kimi 数值字段（used/limit/totalQuota.used）按**字符串**解析（规格 §5.2 口径）

【测试纪律】实现过程只跑模块级验证：cargo test --manifest-path src-tauri/Cargo.toml plan_balance 与 cargo test --manifest-path src-tauri/Cargo.toml settings；全量门禁由后续测试 agent 统一执行，你不要跑全量。cargo 系命令共享 target 目录锁，排队属正常勿中止。`,
  },
]
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 2: 全量测试（命令相互独立，并行启动执行，收集全部结果）===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:\\data\\learn\\code\\slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. npx tsc --noEmit
2. npx eslint src/
3. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
4. npm test
5. cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
中间态说明：本 Stage 只动后端——tsc/eslint/npm test 应零变化全绿；clippy/cargo test 须含 plan_balance 新模块全部用例通过。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证（断言清单唯一真值源：verify/stage-01.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 01 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/f10-plan-balance/workflows/verify/stage-01.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
