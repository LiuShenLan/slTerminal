// =====================================================================
// Stage 03：后端安全（SEC-15、SEC-17、BE-22、BE-24、BE-25）
// 编排：并行 4（文件零重叠：shell.rs / state.rs / notify/mod.rs / hooks config.rs）
// verify 真值源：docs/review-phase2-fix/workflows/verify/stage-03.md
// 人工验证点：真实 claude spawn 三 shell 实测（Win11 本机 + Win10 另一台）
// =====================================================================

export const meta = {
  name: 'stage03-backend-security',
  description: 'S03 后端安全：shell fallback 收窄 + watcher 校验异步化/大小写 + 锁中毒可观测 + user 层审计（SEC-15/17、BE-22/24/25）',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入。
背景：修复要点详见 \`docs/review-phase2-fix/checklist.md\` 对应 ID 条目（先读再动手——条目内含精确 file:line、现状代码与可照抄的修改后代码块）。
测试纪律：本阶段禁止跑 cargo test（资源共享型，全量测试 agent 单点跑）；编译级自查用 \`cargo check --manifest-path src-tauri/Cargo.toml\`。`

// === Phase 1: 并行重构（agent 间文件零重叠）===
phase('并行重构')
const parallelAgents = [
  {
    label: 'A-shell',
    prompt: `你负责【SEC-15】shell 白名单 paths_match fallback 收窄（D15 决策）。先读 \`docs/review-phase2-fix/checklist.md\` 第 3 节 SEC-15 条目——条目内含可照抄的三臂 match 完整代码块。

触碰文件：\`src-tauri/src/pty/shell.rs\`、\`src-tauri/src/pty/CLAUDE.md\`

步骤：
1. \`src-tauri/src/pty/shell.rs\` 的 \`paths_match\`（约 :105-122）整体替换为 checklist 条目中的三臂 match 代码块：双成功精确比较 / 双侧失败字符串回退 / 单侧失败 \`_ => false\` 拒绝
2. 函数文档注释（约 :98-104）改写：删「安全语义不弱化」失实表述，按 checklist 条目写「双侧失败才回退（残余风险登记）+ 单侧失败即拒绝」
3. 测试：shell.rs 现有 paths_match 相关用例逐一核对——断言「单侧失败放行」的用例改为断言 false；新增 \`paths_match_single_side_failure_rejected\`（一侧真实存在路径、一侧不存在路径 → false）
4. \`src-tauri/src/pty/CLAUDE.md\`「白名单真实路径校验」段按 checklist 条目改写（双侧失败才回退 + 残余风险登记）
5. \`cargo check --manifest-path src-tauri/Cargo.toml\` 编译通过

完成后报告：改动摘要 + 修改的既有用例清单 + 新增用例名。`,
  },
  {
    label: 'B-state',
    prompt: `你负责【BE-24】SEC-14 锁中毒分支可观测化。先读 \`docs/review-phase2-fix/checklist.md\` 第 3 节 BE-24 条目——条目内含可照抄的 match 代码块。

触碰文件：\`src-tauri/src/state.rs\`、\`src-tauri/src/CLAUDE.md\`

步骤：
1. \`src-tauri/src/state.rs\` 的 \`apply_project_root\` Err 臂（约 :283-288）：\`if let Ok(mut root) = project_root.write() { *root = None; }\` 替换为 checklist 条目中的 match 代码块（Ok 臂清空 / Err 臂 tracing::warn!「写锁中毒」）
2. 注意：本 Stage 只改这一处 Err 臂——\`set_project_root\`/\`set_project_root_impl\` 主体由后续 S04 改动，禁止顺手改
3. \`src-tauri/src/CLAUDE.md\`「std Mutex 中毒保持现状」节末按 checklist 条目补 BE-24 例外登记
4. \`cargo check --manifest-path src-tauri/Cargo.toml\` 编译通过

完成后报告：改动摘要。`,
  },
  {
    label: 'C-notify',
    prompt: `你负责【BE-22】notify_watch 前置校验移 spawn_blocking +【BE-25】排除目录大小写不敏感。先读 \`docs/review-phase2-fix/checklist.md\` 第 3 节 BE-22/BE-25 条目——BE-22 含可照抄的 spawn_blocking 代码块。

触碰文件：\`src-tauri/src/notify/mod.rs\`（两处改动同文件）

步骤：
1. 【BE-22】\`notify_watch\` 前置校验块（约 :356-363）替换为 checklist 条目中的代码：读锁内取 root.clone() 快照 → 校验整体移 \`tokio::task::spawn_blocking\`（照 :379-391 BE-04 先例，TaskJoin 错误映射）；\`validate_watch_path\` 函数本体不动
2. 【BE-25】\`is_excluded_path\`（约 :199-203）的 \`.any(|seg| WATCH_EXCLUDE_DIRS.contains(&seg))\` 改为 checklist 条目中的 \`eq_ignore_ascii_case\` 版本
3. 测试适配：\`is_excluded_path_matches_all_seven_dirs\` 增补大小写变体断言（Node_Modules、TARGET、DIST 命中）；函数头注释「整分量比较」段同步补大小写说明
4. \`cargo check --manifest-path src-tauri/Cargo.toml\` 编译通过

完成后报告：两处改动摘要 + 新增断言清单。`,
  },
  {
    label: 'D-hooks',
    prompt: `你负责【SEC-17】hooks user 层写入后端审计 + 威胁模型登记。先读 \`docs/review-phase2-fix/checklist.md\` 第 3 节 SEC-17 条目——条目内含可照抄的审计代码块。

触碰文件：\`src-tauri/src/hooks/claude/config.rs\`、\`src-tauri/src/hooks/CLAUDE.md\`、\`.claude/test-inventory.md\`

步骤：
1. \`src-tauri/src/hooks/claude/config.rs\` 的 \`config_write_sync\`（约 :271-295）：\`let path = resolve_config_path(...)?;\` 之后、\`write_hooks_subtree\` 之前插入 checklist 条目中的审计代码块（\`matches!(l, Layer::User)\` → \`tracing::warn!(target: "audit", ...)\`）
2. \`src-tauri/src/hooks/CLAUDE.md\`「写入语义校验（SEC-05，S17，D9）」段末按 checklist 条目补威胁模型登记（二次确认 = UX 层非安全边界；后端审计日志兜底）
3. \`.claude/test-inventory.md\` 既定豁免清单补一行：SEC-17 审计日志输出（豁免原因：tracing 副作用 L1 不可测；兜底：人工可观测）
4. \`cargo check --manifest-path src-tauri/Cargo.toml\` 编译通过

完成后报告：三处改动摘要。`,
  },
]
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 2: 全量测试 ===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:/data/learn/code/slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
2. cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
3. cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证 ===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 3 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/review-phase2-fix/workflows/verify/stage-03.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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

const verifyResult = rawVerify ?? { allFixed: false, failedItems: ['verify-agent-no-return'], details: { 'verify-agent-no-return': { status: 'not_fixed', evidence: 'verify agent 未返回（被跳过或 API 错误）' } } }

return { refactorResults, testResult, verifyResult }
