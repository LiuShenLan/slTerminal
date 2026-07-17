// Stage 1 — 路径沙箱启用 + capabilities 收紧（SEC-01、SEC-06、DOC-05c）
// 用法：主 agent 以 Workflow({ scriptPath: 本文件 }) 调用
// 2 个并行 agent：backend-agent（Rust 侧）| frontend-agent（TS 侧）
// 跨侧契约（写死，两侧必须一致）：
//   Rust 命令  set_project_root(path: String) -> Result<(), AppError>
//   JS wrapper setProjectRoot(path: string): Promise<void>  → invoke('set_project_root', { path })

export const meta = {
  name: 'stage1-sandbox',
  description: 'Stage1: 启用路径沙箱（set_project_root）+ 移除生产 wdio 权限',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目：slTerminal（Tauri 2 + React/TS 前端 + Rust 后端，Windows 终端模拟器），根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改分配给你的文件，不顺手改无关代码（surgical changes）；代码注释用中文，遵循各语言注释规范；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何情况下不得修改 ConPTY flags。
背景文档：修复要点详见 docs/refacotr/refactoring-checklist.md 的 SEC-01/SEC-06/DOC-05c 条目（先读再动手）。
跨侧契约（另一侧 agent 并行实现，必须严格一致，不得自行改名）：
- Rust 命令：set_project_root(path: String) -> Result<(), AppError>
- JS wrapper：setProjectRoot(path: string): Promise<void>，invoke('set_project_root', { path })`

phase('并行重构')
const refactorResults = await parallel([
  () => agent(`
${PREAMBLE}

你负责 Rust 后端侧（SEC-01 后端 + SEC-06 + DOC-05c）：

【SEC-01 后端】
1. src-tauri/src/state.rs — 修复 validate_path_within_root：
   - 相对路径先以 project_root 为基准 join 成绝对路径，再 dunce::canonicalize
   - 目标不存在时拒绝（不再做"上溯到存在祖先 + join + starts_with"的词法比较）
   - 移除 project_root 为 None 时跳过的逻辑（保留 #[cfg(test)] 豁免）
   - 保留现有 15 条 sandbox 测试并使其适配新语义（测试内需要设置 project_root 的先设置）
2. src-tauri/src/state.rs — 新增 Tauri 命令 set_project_root(path: String, state: State<AppState>)：
   - canonicalize 后写入 AppState.project_root；返回 Result<(), AppError>
3. src-tauri/src/fs/mod.rs — fs_read_dir 增加 state: State<'_, AppState> 参数并调用 validate_path_within_root（与其余 fs_* 命令一致）
4. src-tauri/src/lib.rs — 在 generate_handler! 注册 set_project_root

【SEC-06 capabilities 收紧】
- src-tauri/capabilities/default.json：放行 set_project_root（按现有条目格式）；移除 "wdio-webdriver:default" 条目

【DOC-05c JobHandle 跨平台一致性】
- state.rs 的 PtySession.job_object 字段与 pty/spawn.rs 的初始化处：消除 #[cfg(windows)] 条件初始化——非 Windows 平台用 JobHandle::new_dummy() 无条件初始化（对齐 pty/CLAUDE.md 零占位设计，消除非 Windows 编译错误 E0063）

改完自查：cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings && cargo test --manifest-path src-tauri/Cargo.toml state -- --test-threads=1
`, { label: 'backend-agent' }),

  () => agent(`
${PREAMBLE}

你负责前端侧（SEC-01 前端接入）：

1. src/ipc/fs.ts — 新增 setProjectRoot(path) wrapper（遵循本层 thin wrapper 约定，契约见上）
2. src/stores/projects.ts — 打开/切换项目时调用 setProjectRoot(rootPath)
3. e2e-tests/helpers.ts — E2E 辅助路径（__slterm_e2e_createProject 等）同样接入 setProjectRoot
4. rootPath 为 null（未打开项目）时 fs 操作将被沙箱拒绝——确认 explorer 在 rootPath=null 时不发 IPC（现有逻辑已是，核实即可，不改）

注意：Rust 侧由 backend-agent 并行实现；本侧仅写前端代码，invoke 命令名/参数严格按契约。
改完自查：npx tsc --noEmit && npx eslint src/（e2e-tests/helpers.ts 的正确性由收尾 L4 构建覆盖）
`, { label: 'frontend-agent' }),
])

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

// === Phase 3: 逐项验证（断言清单唯一真值源：verify/stage-01.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 1 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/refacotr/workflows/verify/stage-01.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
