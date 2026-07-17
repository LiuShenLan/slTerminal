// Stage 6 — 终端 hook 架构（约束 #8）（FE-23、FE-08、TE-16、FE-17、FE-14、FE-15、FE-12）
// 用法：主 agent 以 Workflow({ scriptPath: 本文件 }) 调用
// 2 个并行 agent：session-agent（useXterm/usePtyOutput/usePtyResize/TerminalRegistry）
//                instance-agent（useTerminalInstance + 删 useFontSizeBridge）

export const meta = {
  name: 'stage6-terminal-hooks',
  description: 'Stage6: 会话元数据单点化（约束 #8）+ 终端 hook 清理',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目：slTerminal（Tauri 2 + React/TS 终端模拟器），根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改分配给你的文件，不顺手改无关代码；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7（前端 agent 不涉及，声明备查）。
背景文档：修复要点详见 docs/refacotr/refactoring-checklist.md 对应 ID 条目（先读再动手）；架构约束 #8 见根 .claude/CLAUDE.md。
测试约束：npm test；use-xterm 系列测试（use-xterm-output.test.ts 37 条 + use-xterm-lifecycle.test.ts 71+ 条）必须全绿——这是本 Stage 门禁。`

phase('并行重构')
const refactorResults = await parallel([
  () => agent(`
${PREAMBLE}

你负责会话元数据单点化（高风险重构，约束 #8），涉及 4 个文件：
src/panels/terminal/useXterm.ts、usePtyOutput.ts、usePtyResize.ts、TerminalRegistry.ts

【FE-23】删除 sessionIdRef 副本
- 现状：useXterm.ts:140 自建 sessionIdRef 并透传 usePtyOutput/usePtyResize；TerminalRegistry 已存 sessionId——两处真值源违反约束 #8
- 修复：删除 useXterm 的 sessionIdRef；pty.write/pty.resize 处改从 TerminalRegistry.get(panelId)?.sessionId 读取；读不到（未 spawn/已退出）时跳过并 debug 日志
- usePtyOutput/usePtyResize 的入参签名同步改（不再接收 sessionId ref，改收 panelId 或自行查 Registry）

【FE-08】usePtyOutput retry disposable 未清理（usePtyOutput.ts:156-178）
- useXterm cleanup 中补 retryDisposableRef.current?.dispose()（Terminal dispose 前的显式路径）

【TE-16】usePtyOutput 直写路径不检查 visibleRef（usePtyOutput.ts:199-201）
- <64 字节直写路径加 visibleRef 门控：visible=false 时也走累积缓冲（对齐设计文档"非焦点仅累积不渲染"）
- use-xterm-output.test.ts 的 NF3 用例断言同步修正（标题与行为一致：非焦点直写被抑制）

【FE-17】TerminalRegistry.getAll 返回可写 Map（TerminalRegistry.ts:39-41）
- 返回 new Map(registry) 副本

注意：Registry 读取的时序——spawn 完成前 write/resize 调用应安全跳过（现有空 sessionId 分支语义保留）。
改完跑：npm test，use-xterm 全部测试 + keyboard/activeTerminal 测试全绿。
`, { label: 'session-agent' }),

  () => agent(`
${PREAMBLE}

你负责 src/panels/terminal/useTerminalInstance.ts 与 useFontSizeBridge.ts 的 3 项改动：

【FE-14】document.fonts.ready Promise 无取消（useTerminalInstance.ts:123-135）
- cleanup 置取消标志；rAF 回调双重检查取消状态 + isDisposedRef

【FE-15】dispose/cleanup 逻辑重复（useTerminalInstance.ts:159-180 与 208-224）
- 提取内部 performDispose()，effect cleanup 与 dispose 回调共用

【FE-12】useFontSizeBridge.ts 死代码删除
- 删除 src/panels/terminal/useFontSizeBridge.ts 整个文件（生产代码实际用 src/lib/useFontSizeWheel.ts）
- grep 确认无 import 残留（含 terminal/index.ts 导出与注释引用，注释引用一并清理）
- 注意：terminal/index.ts 若 re-export 它也删除该导出

改完跑：npx tsc --noEmit && npx eslint src/ && npx vitest run use-xterm terminal 全绿。
`, { label: 'instance-agent' }),
])

// === Phase 2: 全量测试 ===
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

// === Phase 3: 逐项验证（断言清单唯一真值源：verify/stage-06.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 6 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/refacotr/workflows/verify/stage-06.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
