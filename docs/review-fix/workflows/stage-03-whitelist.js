// =====================================================================
// stage-03-whitelist.js — S03 reattach 删除 + 命令白名单（SEC-03/07）
// =====================================================================
// 跨边界契约（写死）——S03 完成后命令清单终态 32 条：
//   ping, get_windows_build_number, set_project_root,
//   pty_spawn, pty_write, pty_resize, pty_kill,
//   fs_read_file, fs_write_file, fs_read_dir, fs_create_dir, fs_delete, fs_rename,
//   save_settings, load_settings, save_projects, load_projects,
//   git_status, git_diff, git_file_at_head, git_rollback, git_unstage,
//   notify_watch,
//   agent_hooks_inject, agent_hooks_uninstall, agent_hooks_injection_status,
//   agent_hooks_restore_statusline, agent_hooks_config_read, agent_hooks_config_write,
//   agent_history_scan, agent_history_delete, agent_history_read_title
//   权限命名 = allow- + 命令名 snake_case 原样（一手证据：tauri-build 2.6.3 acl.rs:100
//   注释）；执行期以 cargo build 产物 src-tauri/gen/schemas/*.json 实际生成为准核对。
// 串行理由：白名单（B）依赖 reattach 删除（A）后的最终 32 条清单。
// fix-loop 调用约定：args.testCommands 省略（默认基础五条）；constraints 传空。
// =====================================================================

export const meta = {
  name: 'stage03-whitelist',
  description: 'Stage 03: 删除无消费 pty_reattach + 32 条命令白名单化（SEC-03/07）',
  phases: [
    { title: '并行重构' },
    { title: '串行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
背景：修复要点详见 docs/review-fix/checklist.md 对应 ID 条目 + docs/review-fix/stages.md S03 节（先读再动手）。
测试纪律：本阶段禁止跑资源共享型全量测试（cargo test / npm test / npm run test:l3）——只做编译级检查（cargo check / npx tsc --noEmit），全量执行由独立测试 agent 单点跑（跨进程并发会死锁，cargo 排队属正常）。`

// === Phase 1: 并行重构（agent 间文件零重叠；不跑资源共享型测试）===
phase('并行重构')
const parallelAgents = [
  {
    label: 'remove-reattach',
    prompt: `你负责 SEC-03，只许改 src-tauri/src/pty/spawn.rs、src-tauri/src/lib.rs、src/ipc/pty.ts、src/__tests__/ipc-contract.test.ts、src-tauri/tests/pty_integration_tests.rs：
pty_reattach 无 panel_id 归属校验且无生产调用方——按决策 D1 整体删除：后端命令 pty_reattach（spawn.rs:1309-1367）+ lib.rs 的 generate_handler! 注册项 + 前端 wrapper（src/ipc/pty.ts:74-82）+ 关联测试（ipc-contract.test.ts 的 reattach 用例、pty_integration_tests.rs 的 reattach 用例）。
【保留红线】ring buffer / channel 替换机制保留（reader 内部仍用，E1 机制不动）——只删命令入口，不删 PtySession 的 ring/channel 设施。
完成后 grep -ri reattach src/ src-tauri/ 应零残留（ring buffer 相关注释除外）；前端 types/ 中 reattach 专用 DTO（若有）一并清理。
只做 cargo check / npx tsc --noEmit 编译级检查。`
  },
]
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 2: 串行重构（共享文件依赖，前序 agent 的产出供后序使用）===
phase('串行重构')
const sequentialAgents = [
  {
    label: 'command-whitelist',
    prompt: `你负责 SEC-07，只许改 src-tauri/build.rs、src-tauri/capabilities/default.json：
前置：上一 agent 已删除 pty_reattach，lib.rs 的 generate_handler! 现为 32 条命令（先 Read lib.rs 逐条抄录实际清单，以下方契约为校验基准——不一致先报告再按 lib.rs 实际为准）。
契约清单（32 条）：ping, get_windows_build_number, set_project_root, pty_spawn, pty_write, pty_resize, pty_kill, fs_read_file, fs_write_file, fs_read_dir, fs_create_dir, fs_delete, fs_rename, save_settings, load_settings, save_projects, load_projects, git_status, git_diff, git_file_at_head, git_rollback, git_unstage, notify_watch, agent_hooks_inject, agent_hooks_uninstall, agent_hooks_injection_status, agent_hooks_restore_statusline, agent_hooks_config_read, agent_hooks_config_write, agent_history_scan, agent_history_delete, agent_history_read_title。
【build.rs】配置 tauri_build::Attributes::new().app_manifest(AppManifest::new().commands(&[...32 条]))（一手证据：tauri-build 2.6.3 acl.rs:100 存在该 API），为每条命令生成 allow-<cmd> 权限。
【capabilities/default.json】32 条命令逐条 allow-<cmd>（权限名 = allow- + 命令名 snake_case 原样）；保留既有插件权限不动；删除 _p0-07-note 旧注释。
执行 cargo build --manifest-path src-tauri/Cargo.toml 触发 codegen，Read src-tauri/gen/schemas/ 下生成的权限 schema，核对 32 条 allow- 权限名与 capabilities 所写一致（不一致以 gen 产物为准修正 capabilities）。
只做编译级检查，禁止 cargo test。`
  },
]
const sequentialResults = []
for (const a of sequentialAgents) {
  const r = await agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label })
  if (!r) break  // 前序失败短路，不跑下游
  sequentialResults.push(r)
}

// === Phase 3: 全量测试（命令相互独立，并行启动执行，收集全部结果）===
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

// === Phase 4: 逐项验证（断言清单唯一真值源）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 03 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/review-fix/workflows/verify/stage-03.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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

return { refactorResults, sequentialResults, testResult, verifyResult }
