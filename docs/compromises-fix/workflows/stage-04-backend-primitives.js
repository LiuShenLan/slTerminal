// =====================================================================
// Stage 04 后端基础原语
// 4a CP-005 parking_lot 全仓换装（单 agent 先行）→ 4b 五路并行
// （CP-008/010/011+034/014/043）→ 4c 命令注册收口（lib.rs/build.rs 防并行撞车）
// fix-loop constraints：本 Stage 无特殊纪律（省略 args.constraints）
// =====================================================================

export const meta = {
  name: 'stage-04-backend-primitives',
  description: 'S04 后端基础原语（CP-005/008/010/011/034/014/043）',
  phases: [
    { title: '4a parking_lot 换装' },
    { title: '4b 并行修复' },
    { title: '4c 命令注册收口' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入。
背景：修复步骤六段式真值源 = docs/compromises-fix/checklist.md 对应 CP 条目（先读再动手，照抄步骤不另起方向）；大代码块按条目内「照抄 review-NN 步骤 X.Y」指针读 docs/compromises-fix/ 下对应 review 文件。
L1 命令形态 = cargo test <filter> -- --test-threads=1（S02 后形态）。
并行纪律：本阶段只做编译级检查（cargo test --no-run / cargo check），真实测试由全量测试 agent 单点跑。`

// === Phase 1: 4a CP-005 全仓单 agent 先行 ===
phase('4a parking_lot 换装')
const wave4aResult = await agent(`${PREAMBLE}

你负责 CP-005（std::sync::Mutex/RwLock 全仓换装 parking_lot）：
照抄 checklist CP-005 步骤 1-5：
1. Cargo.toml ureq 行后加 parking_lot = "0.12"（含 CP-005 注释）。
2. 全仓类型替换（std::sync::Mutex → parking_lot::Mutex、RwLock 同）——站点清单以 rg "std::sync::(Mutex|RwLock)" src-tauri/ 实查为准（登记 8 处为低估，生产实含 notify/mod.rs；含 use 语句、全路径签名、#[cfg(test)] 槽位、测试局部）。
3. 锁获取三形态清除（map_err/match 中毒分支/裸 unwrap）逐点按条目清单执行（settings.rs/background_tasks/hooks/state.rs/spawn.rs/reader.rs/git/scan.rs 逐一点名）。
4. 注释同步删改（锁中毒相关注释随分支删除清理）。
5. 文档：src-tauri/src/CLAUDE.md:53 节整节重写；adr.md ADR-0009 09#14 行改写；pty/git/CLAUDE.md 豁免表 Mutex 中毒行删除；grep "锁中毒" 全仓清理。
测试适配逐一点名按条目执行（state_tests/settings 内嵌/plan_balance/notify/spawn 测试 import 等）；新增防复发 hooks/mod.rs watcher_tests 用例 start_signal_watcher_locks_without_poison_path。
自查（编译级）：cargo check + cargo test --no-run；rg "std::sync::(Mutex|RwLock)" src-tauri/ 零命中。
完成后报告修改文件清单。`, { label: 'cp005-parking-lot' })

// === Phase 2: 4b 五路并行（文件零重叠；lib.rs/build.rs 注册一律不做，归 4c）===
phase('4b 并行修复')
const wave4bAgents = [
  {
    label: 'cp008-git-dead-branch',
    prompt: `你负责 CP-008（git_status is_ignored 死分支删除）：
照抄 checklist CP-008 步骤 1-3：git/mod.rs 删 is_ignored 两分支行 + :26 注释删 "| ignored" + status_to_str doc 补 IGNORED 落 None 注记；src/types/git.ts:7 注释值集删 "| ignored"；前端零改动。
测试：git_status_tests.rs:48 改 (IGNORED, None)；新增 status_to_str_ignored_returns_none + git_status_ignored_file_never_emitted 两用例。
文档：git/CLAUDE.md:21 节末句改写；:65 红线保留并追加引用。
自查：cargo test --no-run；rg "is_ignored" src-tauri/src 零命中。
完成后报告修改文件清单。`,
  },
  {
    label: 'cp010-conpty-status',
    prompt: `你负责 CP-010（Win10 conpty 状态暴露 + 启动 toast，跨端）：
照抄 checklist CP-010 步骤 1-3：conpty_api.rs 增 ConptyStatus DTO + STATUS OnceLock + conpty_status() 查询入口（warn 文案与 fallback_reason 同一 format!("{e:#}") 变量）；pty_conpty_status 命令函数本体写好但【三处注册（lib.rs/build.rs/capabilities）不由你执行，归 4c 收口】；前端 src/types/pty.ts 增 ConptyStatus 双边 + src/ipc/pty.ts 增 getConptyStatus() + App.tsx 启动序列一次性 toast。
测试：conpty_api_tests 增三用例（win11_not_attempted/bundled_on_win10/fallback_reason_matches_warn，STATUS 单例污染按条目防御口径处理）；L2 ipc-pty-contract.test.ts 增命令名 + 键集合断言。
文档：pty/CLAUDE.md ADR-0005 节补 + 豁免表行补兜底；src/types/CLAUDE.md 补 ConptyStatus；src/ipc/CLAUDE.md pty 通道同步。
自查：cargo test --no-run；npx tsc --noEmit。
完成后报告：修改文件清单 + 移交 4c 的注册签名（命令名 pty_conpty_status、无参、返回 ConptyStatus）。`,
  },
  {
    label: 'cp011-034-pty-kill-ring',
    prompt: `你负责 CP-011+CP-034（pty kill 超时显式清理 + ring buffer 删除，同 agent，先 034 后 011；基线 = CP-005 换装后）：
【先 CP-034】照抄 checklist 步骤 1-6：PtySession 删 channel/output_ring 两字段；state.rs 删 RING_BUFFER_CAPACITY/ring_buffer_append；reader_loop 收敛 Channel 直写断开退出（签名与三分支照条目代码块；E1 头注释/函数文档/M11 分析块同步删 RwLock/ring/reattach 措辞）；spawn.rs 四处适配（:1239/:1256/:1270/:1281 + :2091 测试构造）；pty/CLAUDE.md :58 删行 + :115 豁免措辞改；test-exemptions.md:13 措辞删「channel 锁/ring buffer」；state_tests ring 六用例删。
【后 CP-011】照抄 checklist 步骤 0-5：join_with_timeout 与两常量上提 reader.rs；CLOSE_PSEUDO_CONSOLE_TIMEOUT 3s 常量；pty_kill/pty_kill_all 超时分支监督线程（照抄代码块，pty-cleaner 命名）；state.rs Drop 带超时 join（照抄代码块）；plan_cleanup_after_join_timeout 纯函数 + CleanupPlan 枚举；四处失真注释修正；pty/CLAUDE.md :52-54 节重写 + 豁免表增行；test-exemptions.md 增监督线程豁免行。
测试：reader_tests 增 cleanup_plan 两用例 + join_with_timeout_timeout_returns_false；spawn.rs 涉 join 用例改 import。
自查：cargo test --no-run；rg "output_ring|ring_buffer_append|RING_BUFFER_CAPACITY" src-tauri/src 零命中；rg ".join()" src-tauri/src 仅剩 join_with_timeout 内部。
完成后报告修改文件清单。`,
  },
  {
    label: 'cp014-file-identity',
    prompt: `你负责 CP-014（shell 路径比对改 Win32 句柄级文件身份）：
照抄 checklist CP-014 步骤 1-3（代码全量照抄 review-03 CP-014 步骤 3）：Cargo.toml windows features 加 Win32_Storage_FileSystem；shell.rs 新增 file_identity（CreateFileW 不带 OPEN_REPARSE_POINT + GetFileInformationByHandle，身份 = volume serial + file index）；paths_match 三分支改写（双成功精确比较/双失败 fallback_identity_match 不降级字符串/单侧失败即拒绝）；normalize_for_compare 收编 #[cfg(not(windows))]。
测试：三 fallback 用例合并改写为 fallback_both_unopenable_rejected（同名同串也拒绝）；新增 hardlink 三例（same_file_via_hardlink_equal/distinct_files_unequal/missing_file_none，Windows 条件编译）。
文档：pty/CLAUDE.md:75 改写；adr.md:229 D15 行尾追加销记。
自查：cargo test --no-run；grep "eq_ignore_ascii_case(&b)" shell.rs 零命中。
完成后报告修改文件清单。`,
  },
  {
    label: 'cp043-statusline-confirm',
    prompt: `你负责 CP-043（SEC-12 statusline 可疑命中即暂停 + 用户确认二次注入，跨端）：
照抄 checklist CP-043 步骤 1-7（代码全量照抄 review-03 CP-043 步骤 3）：warn_if_suspicious_statusline 改 audit_suspicious_statusline 返 Option（审计通道 target: "audit"）；inject_impl 加第三参 skip_suspicious_review + 审查闸（命中 → PendingConfirmation + suspicious_command，settings.json 零写盘）；reinject_statusline_impl 命中即跳过 + 审计；DTO 改造（AgentInjectionStatus 第四态 + AgentHookInjectionStatus 增 suspicious_command 带 skip_serializing_if）；provider trait 加 confirm_inject 默认实现（claude override = inject_impl(..., true)）；agent_hooks_confirm_inject 命令函数本体写好但【三处注册（lib.rs/build.rs/capabilities）不由你执行，归 4c 收口】；前端 types/agent.ts 双边 + ipc/agentHooks.ts confirmInject + ClaudeHooksConfigEditor 确认流内联确认条（data-e2e hooks-confirm-inject/hooks-cancel-confirm）。
测试：inject_tests 两改写 + 两新增（含 settings.json 逐字节零写盘断言 + traced_test 审计断言）；hooks/mod.rs roundtrip 新增 pendingConfirmation；既有 inject_impl 调用点逐名补第三参 false；L2 ipc-agent-hooks-contract + settings-hooks-page 新增用例 + 全局 mock 补 confirmInject（setup.ts:108/:98/:77 等逐一点名）。
文档：hooks/CLAUDE.md SEC-12 节改写 + :91 trait 先例补；src/ipc/CLAUDE.md 6 命令表改 7；src/types/CLAUDE.md 四态同步。
自查：cargo test --no-run；npx tsc --noEmit；grep -rn "warn_if_suspicious_statusline" src-tauri/src/ 零命中。
完成后报告：修改文件清单 + 移交 4c 的注册签名（命令名 agent_hooks_confirm_inject、参数 cliId、spawn_blocking 形态）。`,
  },
]
const wave4bResults = await parallel(
  wave4bAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 3: 4c 命令注册收口（防 4b 多 agent 并行撞 lib.rs/build.rs）===
phase('4c 命令注册收口')
const wave4cResult = await agent(`${PREAMBLE}

你负责 4c 命令注册收口（4b 各 agent 已完成命令函数本体，签名见各自报告）：
1. src-tauri/src/lib.rs generate_handler! 增 pty_conpty_status 与 agent_hooks_confirm_inject。
2. src-tauri/build.rs AppManifest::new().commands(...) 同步增两条（清单计数注释按实际改）。
3. src-tauri/capabilities/default.json 增 allow-pty_conpty_status 与 allow-agent-hooks-confirm-inject（权限最小化口径，不追加通配）。
4b 报告（签名出处）：
---
CP-010：${wave4bResults[1] ?? '（未返回——Read src-tauri/src/pty/conpty_api.rs 自取签名）'}
CP-043：${wave4bResults[4] ?? '（未返回——Read src-tauri/src/hooks/ 自取签名）'}
---
自查：cargo check 绿；rg "pty_conpty_status" src-tauri/src/lib.rs src-tauri/build.rs src-tauri/capabilities/default.json 三处全命中；agent_hooks_confirm_inject 同。
完成后报告修改文件清单。`, { label: 'cp04c-command-registration' })

// === Phase 4: 全量测试 ===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:/data/learn/code/slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
2. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
3. cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
4. npx tsc --noEmit
5. npx eslint src/
6. npm test
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 5: 逐项验证 ===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 04 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/compromises-fix/workflows/verify/stage-04.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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

return { wave4aResult, wave4bResults, wave4cResult, testResult, verifyResult }
