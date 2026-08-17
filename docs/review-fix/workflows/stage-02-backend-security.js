// =====================================================================
// stage-02-backend-security.js — S02 后端安全 P0（SEC-01/02、BE-01）
// =====================================================================
// 跨边界契约：无（纯后端 Stage）。
// fix-loop 调用约定：args.testCommands 省略（默认基础五条）；constraints 传空。
// =====================================================================

export const meta = {
  name: 'stage02-backend-security',
  description: 'Stage 02: shell 白名单真实路径校验 + 信号文件 symlink 过滤 + PTY 会话上限 32（SEC-01/02、BE-01）',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
背景：修复要点详见 docs/review-fix/checklist.md 对应 ID 条目 + docs/review-fix/stages.md S02 节（先读再动手）。
测试纪律：本阶段禁止跑资源共享型全量测试（cargo test / npm test / npm run test:l3）——只做编译级检查（cargo check / npx tsc --noEmit），全量执行由独立测试 agent 单点跑（跨进程并发会死锁，cargo 排队属正常）。`

// === Phase 1: 并行重构（agent 间文件零重叠；不跑资源共享型测试）===
phase('并行重构')
const parallelAgents = [
  {
    label: 'pty-security',
    prompt: `你负责 SEC-01、BE-01，只许改 src-tauri/src/pty/shell.rs、src-tauri/src/pty/spawn.rs：
【SEC-01】shell.rs 的 validate_shell_allowlist 当前仅 file_name 比对（shell.rs:29-58），传 C:\\project\\cmd.exe 可绕过。修复：用户传入 shell 含路径分隔符时——canonicalize 用户路径，与 which_full_path(文件名) 解析结果比对，一致才放行（即只信任 PATH 解析出的真实路径）；纯文件名输入维持现状。spawn.rs:979 调用点按需适配。补 L1 测试（mod tests）：伪造绝对路径拒绝 / PATH 解析出的合法绝对路径放行。
【BE-01】spawn.rs 的 pty_spawn（:958）无会话总数上限。加 const MAX_PTY_SESSIONS: usize = 32，spawn 前检查 sessions.len() >= 32 返回 AppError::Validation；检查必须在 SPAWN_LOCK 持锁区间内（防并发超发）。补 L1 测试（上限判定逻辑抽可测纯函数或构造满员状态验证 Err）。
只做 cargo check 编译级检查，禁止 cargo test。`
  },
  {
    label: 'signal-security',
    prompt: `你负责 SEC-02，只许改 src-tauri/src/hooks/signal.rs、src-tauri/src/hooks/watcher.rs：
信号目录 .json 未过滤符号链接，fs::metadata/read_to_string 跟随 symlink 可越界读取经 agent-event 泄露。修复：process_signal_file_with（signal.rs:82-127）与 collect_signal_files（watcher.rs:155-174）改用 fs::symlink_metadata + is_symlink() 检查；symlink 文件仅删除不读取。补 L1 测试：Windows symlink 需管理员/developer mode——测试内创建失败则 skip 并用注释注明该前提。
只做 cargo check 编译级检查，禁止 cargo test。`
  },
]
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 2: 全量测试（命令相互独立，并行启动执行，收集全部结果）===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:/data/learn/code/slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. npx tsc --noEmit
2. npx eslint src/
3. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
4. npm test
5. cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证（断言清单唯一真值源）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 02 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/review-fix/workflows/verify/stage-02.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
