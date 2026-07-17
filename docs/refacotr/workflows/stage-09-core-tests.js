// Stage 9 — 核心路径补测（TE-01、TE-02、TE-03、TE-05、TE-06、TE-07）
// 用法：主 agent 以 Workflow({ scriptPath: 本文件 }) 调用
// 5 个并行 agent，测试文件不重叠（对齐 refactoring-stages.md Stage 9 分工表）
// 本 Stage 只补测试不改生产代码（#[cfg(test)] 块除外）——测试暴露真实 bug 只报告不修
// fix-loop 调用本 Stage 时 args.constraints 传："本 Stage 只改测试，禁止改生产代码"

export const meta = {
  name: 'stage9-core-tests',
  description: 'Stage9: 核心路径测试补齐（PTY 命令/useXterm 编排/titleManager/layoutSerde/shortcuts）',
  phases: [
    { title: '并行补测' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目：slTerminal（Tauri 2 + React/TS 终端模拟器），根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只新增/修改测试文件（Rust 侧限 #[cfg(test)] 块与 tests/ 目录），不改生产代码；若测试暴露真实 bug，只在报告中描述，不修复。代码注释用中文；完成后报告新增用例清单与用例数（供 Stage 11 更新 test-inventory）。
禁区：compute_conpty_flags 固定 0x7，不得为此改动任何 ConPTY flags 相关代码或测试。
背景文档：补测要点详见 docs/refacotr/refactoring-checklist.md 对应 ID 条目（先读再动手）；测试模式见各模块 CLAUDE.md「测试模式」章节。`

phase('并行补测')
const testResults = await parallel([
  () => agent(`
${PREAMBLE}

你负责 TE-01：PTY 命令补测（Rust）。
- 位置：src-tauri/src/pty/spawn.rs 的 #[cfg(test)] mod tests + src-tauri/tests/pty_integration_tests.rs
- spawn.rs 单测覆盖：env 注入（COLORTERM/TERM/TERM_PROGRAM 三变量存在且值正确）、cwd 反斜杠规范化、ring buffer 回放、session 移除不级联（一个 session 移除不影响其他）
- 集成测试补 reattach 用例（spawn → 写 → kill channel → reattach → ring buffer 内容回放）
- 约束：--test-threads=1；需要真实 spawn 的用 cmd.exe + SPAWN_LOCK 串行模式（参考现有集成测试）
跑：cargo test --manifest-path src-tauri/Cargo.toml pty -- --test-threads=1 全绿
`, { label: 'pty-test-agent' }),

  () => agent(`
${PREAMBLE}

你负责 TE-02：useXterm 轻 mock 集成测试（新增文件 src/__tests__/use-xterm-integration.test.ts）。
- 与现有 use-xterm-*.test.ts（mock 7 个子 hook 测编排）不同：本文件真实 Terminal/FitAddon，仅 mock src/ipc/pty
- 覆盖：rAF 轮询容器宽度失败回退 80×24；term.onData → pty.write 调用链；visible 切换时 WebGL addon 释放/重建
- 遵循 vi.hoisted() mock 模式（参考 use-xterm-lifecycle.test.ts）
跑：npx vitest run use-xterm-integration 全绿
`, { label: 'xterm-test-agent' }),

  () => agent(`
${PREAMBLE}

你负责 TE-03：titleManager.onDeletePage 补测（src/__tests__/title-manager.test.ts 追加）。
- 覆盖：删除页面后该页 registry 与 counters 条目清理（终端编号重算）；SaveAs 后同名冲突标题重算
- 参照现有 29 条用例的 createTitleManager 工厂模式
跑：npx vitest run title-manager 全绿
`, { label: 'title-test-agent' }),

  () => agent(`
${PREAMBLE}

你负责 TE-05 + TE-06：layoutSerde 测试（src/__tests__/layout-serde.test.ts）。
- TE-05：删除文件头部"覆盖率 0%"误导注释（layout-serde.test.ts:3 附近）
- TE-06：补嵌套 branch 修补用例（多层 grid 嵌套的 patchLegacyLayout）、activeGroup 保留断言（fromJSON 后 activeGroup 指向正确 leaf）
- 参照现有 16 条用例的纯函数测试模式
跑：npx vitest run layout-serde 全绿
`, { label: 'layout-test-agent' }),

  () => agent(`
${PREAMBLE}

你负责 TE-07：ShortcutRegistry 补测（src/__tests__/shortcuts.test.ts 追加）。
- 覆盖：addEventListener/removeEventListener spy 断言（refCount 归零移除监听）；setOverrides(undefined) 边界；exportContextBindings 受 overrides 影响；resolve 的 handler 返回 false 时不消费
- 参照现有 41 条用例模式（_reset() 隔离）
跑：npx vitest run shortcuts 全绿
`, { label: 'shortcut-test-agent' }),
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
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。另报告 L1/L2 各层实测用例总数（供 Stage 11 使用）。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证（断言清单唯一真值源：verify/stage-09.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 9 的补测是否实际落地（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/refacotr/workflows/verify/stage-09.md 获取断言清单，用 Grep/Read/git status 逐条核实并给出证据。
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

return { testResults, testResult, verifyResult }
