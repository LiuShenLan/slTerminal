// Stage 2 — spawn.rs 安全与异步化（SEC-02、SEC-07、SEC-08、BE-01、BE-02、BE-11、BE-12、BE-14）
// 用法：主 agent 以 Workflow({ scriptPath: 本文件 }) 调用
// 串行执行：shell-agent 先（提供白名单函数），pty-async-agent 后（调用之）
// ——两 agent 并行会导致 spawn.rs 引用尚不存在的 shell.rs 新函数，编译断裂
// ——shell-agent 失败（返回 null）时短路返回，不再跑下游（避免必然编译失败）

export const meta = {
  name: 'stage2-pty-async',
  description: 'Stage2: PTY 命令异步化 + shell 白名单 + session 归属校验',
  phases: [
    { title: '串行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目：slTerminal（Tauri 2 + React/TS 前端 + Rust 后端，Windows 终端模拟器），根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改分配给你的文件，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何情况下不得修改 ConPTY flags（PASSTHROUGH_MODE 吞鼠标输入，见 pty/CLAUDE.md）。
背景文档：修复要点详见 docs/refacotr/refactoring-checklist.md 对应 ID 条目（先读再动手）。
测试约束：cargo test 必须 --test-threads=1；修改 shell.rs 后跑 shell::tests 全部 7 条；修改 spawn.rs 后跑 conpty_custom::tests 全部 13 条。`

// === Phase 1: 串行重构 ===
phase('串行重构')

const shellResult = await agent(`
${PREAMBLE}

你负责 src-tauri/src/pty/shell.rs 的 3 项改动：

【SEC-02 shell 白名单】
- 新增 pub(crate) 函数 validate_shell_allowlist(program: &str) -> Result<(), AppError>：
  仅允许 pwsh.exe / powershell.exe / cmd.exe（不区分大小写，取文件名比较），
  或经 which_full_path 在 PATH 中解析到上述三者之一的完整路径；其余一律拒绝
- resolve_shell_info 在返回前对最终 program 调用该校验

【SEC-07 resolve_shell 旧路径收敛】
- resolve_shell（旧 CommandBuilder 路径，非 Windows fallback 用）复用与 resolve_shell_info 相同的白名单校验，两套接口行为一致

【BE-11 which_full_path 跨平台】
- PATH 分割改用 std::env::split_paths（不再固定 ';'）

改完跑：cargo test --manifest-path src-tauri/Cargo.toml shell -- --test-threads=1（7 条全绿），并为 validate_shell_allowlist 补单元测试（白名单内/外、大小写、PATH 解析）。
`, { label: 'shell-agent' })

// shell-agent 失败短路：下游 pty-async-agent 依赖其新函数，必然编译失败，不再执行
if (!shellResult) {
  return {
    shellResult,
    ptyResult: null,
    testResult: null,
    verifyResult: { allFixed: false, failedItems: ['shell-agent-no-return'], details: { 'shell-agent-no-return': { status: 'not_fixed', evidence: 'shell-agent 未返回（被跳过或 API 错误），下游已短路' } } }
  }
}

const ptyResult = await agent(`
${PREAMBLE}

你负责 src-tauri/src/pty/spawn.rs 与 src-tauri/src/state.rs 的 6 项改动（shell-agent 已在 shell.rs 提供 validate_shell_allowlist，直接调用 crate::pty::shell::validate_shell_allowlist，不要自己实现）：

【BE-01 PTY 四命令异步化】（对齐 pty_kill 现有模式）
- pty_spawn / pty_write / pty_resize / pty_reattach 改 async fn，阻塞段（create_conpty_pair、CreateProcessW、take_writer、CPR 注入、write_all/flush、ResizePseudoConsole、ring buffer drain + Channel::send）包入 tokio::task::spawn_blocking
- SPAWN_LOCK 仍在 spawn_blocking 内获取（保持串行化语义）

【SEC-02 spawn 侧接入】
- pty_spawn 中对 SpawnRequest.shell 调 validate_shell_allowlist；cwd 调 crate::state::validate_path_within_root

【SEC-08 session 归属校验】
- state.rs 的 PtySession 新增 panel_id 字段；pty_spawn 写入；pty_write/pty_resize/pty_kill 校验调用方 panelId 与 session 归属一致（前端请求 DTO 已有 panelId，核实后使用）

【BE-02 Job Object 失败杀子进程】
- add_to_job_object 失败路径显式 child.kill()，不留孤儿

【BE-12 SPAWN_LOCK 粒度收窄】
- 锁仅保护 create_conpty_pair + spawn_conpty_child；take_writer、CPR 注入、add_to_job_object 移到锁外；同步更新注释

【BE-14 COORD 尺寸校验】
- SpawnRequest 反序列化后校验 cols/rows ≤ i16::MAX，超限返回 Err（不 as i16 回绕）

改完跑：cargo test --manifest-path src-tauri/Cargo.toml pty -- --test-threads=1（conpty_custom 13 条 + 集成测试全绿）。
`, { label: 'pty-async-agent' })

// === Phase 2: 全量测试（命令相互独立，并行启动执行，收集全部结果）===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:\\data\\learn\\code\\slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. npx tsc --noEmit
2. npx eslint src/
3. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
4. npm test
5. cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1（必须单线程，ConPTY 并发 spawn 死锁）
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证（断言清单唯一真值源：verify/stage-02.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 2 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/refacotr/workflows/verify/stage-02.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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

return { shellResult, ptyResult, testResult, verifyResult }
