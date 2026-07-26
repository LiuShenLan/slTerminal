// =====================================================================
// Stage 01 Workflow: 后端 hooks 模块骨架 + 信号 watcher
// =====================================================================
// 契约头部：
//   - 新增 IPC 命令：hooks_inject / hooks_uninstall / hooks_injection_status
//   - Tauri Event：hook-event
//   - 信号目录：~/.slterminal/hooks-events/
//   - Hook 脚本：~/.slterminal/hooks/slterm-hook-reporter.js
//   - 用户配置路径：~/.claude/settings.json
//   - DTO：HookInjectionStatus { status: "injected"|"notInjected"|"outdated", version: number|null }
//   - HookEventPayload { panelId, event, timestamp, sessionId, transcriptPath, cwd, toolName, notificationType }
//   - 信号并发策略：单事件单文件 + 原子 rename
// =====================================================================

export const meta = {
  name: 'stage01-backend',
  description: '后端 hooks 模块骨架 + 信号 watcher',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入。
背景：修复要点详见 docs/hooks-dev/phase1/checklist.md 对应 ID 条目（先读再动手）。`

// === Phase 1: 并行重构 ===
phase('并行重构')
const parallelAgents = [
  {
    label: 'be-hooks-dto',
    prompt: `你负责 P1-BE-01、P1-BE-02：
【P1-BE-01】新建 src-tauri/src/hooks/mod.rs：模块入口；定义并导出 HookEventPayload、HookInjectionStatus、InjectionStatus 三个 DTO；暴露 pub fn start_signal_watcher(app_handle: AppHandle)；管理 static Mutex<Option<HookSignalWatcher>> 持有 watcher 实例（避免 state.rs 与 hooks 循环依赖）。
【P1-BE-02】新建 src-tauri/src/hooks/signal.rs：纯函数 parse_signal_file(content: &str) -> Option<HookEventPayload>；HookEventPayload 字段使用 #[serde(rename_all = "camelCase")]；处理逻辑：读文件 -> 解析 -> app_handle.emit("hook-event", payload) -> 删除文件；缺 panelId、解析失败、读失败均 tracing::warn! 并仍尝试删除文件，不 panic。
完成后跑 cargo check --manifest-path src-tauri/Cargo.toml。`
  },
  {
    label: 'be-hooks-watcher',
    prompt: `你负责 P1-BE-03：
新建 src-tauri/src/hooks/watcher.rs：HookSignalWatcher 结构体；使用 notify + notify-debouncer-full，debounce 50ms，监听 ~/.slterminal/hooks-events/（dirs::home_dir()），RecursiveMode::NonRecursive；线程名 "hook-signal-watcher"；实现 start(app_handle, signal_dir)、stop()、Drop 清理；Create/Modify 事件触发时调用 signal::process_signal_file。
注意：watcher.start 内部若目录不存在则 create_dir_all。完成后跑 cargo check。`
  },
  {
    label: 'be-hooks-inject',
    prompt: `你负责 P1-BE-04、P1-BE-05：
【P1-BE-05】新建 src-tauri/assets/slterm-hook-reporter.js：Node 单文件脚本。行为：读 stdin 全部内容 -> JSON.parse -> 检查 process.env.SLTERM_PANEL_ID，缺失则 process.exit(0)；按契约 C1 组装 payload（panelId, event, timestamp, sessionId, transcriptPath, cwd, toolName, notificationType）；写入 ~/.slterminal/hooks-events/ 下单事件文件：先写 *.tmp 再 fs.renameSync 成 *.json；任何异常（解析失败、目录不可写、写失败）均静默 process.exit(0)，不输出 stderr。脚本顶部含常量 SCRIPT_VERSION = 1。
【P1-BE-04】新建 src-tauri/src/hooks/inject.rs：实现 #[tauri::command] hooks_inject / hooks_uninstall / hooks_injection_status。inject：用 include_str!("../../assets/slterm-hook-reporter.js") 取模板，原子写 ~/.slterminal/hooks/slterm-hook-reporter.js；读 ~/.claude/settings.json，JSON 非法则返回 AppError 且不改动文件；否则 merge 追加 10 事件 matcher 组（command = node "<脚本绝对路径>"，timeout: 5），按 command 含 "slterm-hook-reporter" 子串识别并替换旧段；原子写回。uninstall：移除所有含 "slterm-hook-reporter" 的 matcher 组，空数组事件键清理，删脚本目录，清空信号目录。injection_status：检查脚本存在+可读、settings 中 matcher 组存在、脚本版本与模板一致，返回 injected/outdated/notInjected。
完成后跑 cargo check。`
  },
  {
    label: 'be-lib-register',
    prompt: `你负责 P1-BE-06、P1-BE-07：
修改 src-tauri/src/lib.rs：在 generate_handler! 宏中追加 hooks_inject、hooks_uninstall、hooks_injection_status；在 builder 链上添加 .setup(|app| { hooks::start_signal_watcher(app.handle().clone()); Ok(()) })，确保 watcher 在应用启动时运行。注意 setup 闭包返回 Ok(())，不要阻塞。完成后跑 cargo check。`
  }
];
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 3: 全量测试 ===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:\\data\\learn\\code\\slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
2. cargo test --manifest-path src-tauri/Cargo.toml hooks -- --test-threads=1
3. cargo check --manifest-path src-tauri/Cargo.toml
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 4: 逐项验证 ===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 01 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/hooks-dev/phase1/workflows/verify/stage-01.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
