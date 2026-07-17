// Stage 5 — 前端高危：编辑器竞态 + HTML 注入（FE-01、FE-19、FE-02、SEC-03）
// 用法：主 agent 以 Workflow({ scriptPath: 本文件 }) 调用
// 2 个并行 agent，文件不重叠（useCodeMirror.ts | HtmlPanel.tsx+injectScript.ts+测试）

export const meta = {
  name: 'stage5-frontend-critical',
  description: 'Stage5: 编辑器加载竞态修复 + HTML 注入面收敛',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目：slTerminal（Tauri 2 + React/TS 终端模拟器），根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改分配给你的文件，不顺手改无关代码；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7（本项目 ConPTY 禁区，前端 agent 通常不涉及，声明备查）。
背景文档：修复要点详见 docs/refacotr/refactoring-checklist.md 对应 ID 条目（先读再动手）。
测试约束：前端测试 npm test（Vitest+jsdom）；测试文件放 src/__tests__/，遵循 vi.hoisted() mock 模式（参考现有测试）。`

phase('并行重构')
const refactorResults = await parallel([
  () => agent(`
${PREAMBLE}

你负责 src/panels/editor/useCodeMirror.ts 的 2 项改动：

【FE-01】文件切换过期写入竞态（useCodeMirror.ts:232-289）
- 问题：filePath 快速切换时，旧 initEditor 的 await fs.readFile 完成后看到 mountedRef.current===true（已被新 effect 重置），旧 EditorView 覆盖新实例——用户看到旧文件内容 + 旧 view 永不 cleanup
- 修复：引入 generation 计数（参照 src/features/explorer/useFileTree.ts 的 genRef 模式）——effect 入口生成本次 gen，所有异步回调完成后先比对 genRef.current !== gen 则丢弃；旧 view 创建前已过期则直接 dispose
- 补测试：快速连续切换 filePath A→B，最终显示 B 内容且 A 的 EditorView 已 dispose（参考 editor-confirm.test.ts 的 renderHook 真实驱动模式）

【FE-19】handleSave 根路径 repoDir 计算有误（useCodeMirror.ts:166-171）
- "D:/file.txt" 现得 "D:"（无效 repo 路径）、"C:/" 得空串——gitDiff 失败被静默吞
- 用 src/lib/path.ts 的 dirname 语义处理（无现成 dirname 则用 normalizePath + 边界判断）；无有效父目录时跳过 gitDiff

改完跑：npx tsc --noEmit && npx eslint src/ && npx vitest run useCodeMirror editor-confirm 全绿。
`, { label: 'editor-agent' }),

  () => agent(`
${PREAMBLE}

你负责 src/panels/html/HtmlPanel.tsx 与 src/lib/injectScript.ts 的 2 项改动（含测试文件 src/__tests__/inject-script.test.ts、src/__tests__/html-panel.test.tsx）：

【FE-02】注入脚本未转义宿主 HTML 的 </script>（HtmlPanel.tsx:45-61）
- 用户 HTML 含 </script> 会提前闭合注入脚本，键盘转发/片段拦截失效甚至执行非预期脚本
- 修复在 injectScript 纯函数内完成：注入前将宿主 HTML 中的 "</script>" 转义为 "<\\/script>"（大小写不敏感）；注入脚本自身内容不受影响
- inject-script.test.ts 补对应用例

【SEC-03】postMessage 键盘桥未校验 origin（HtmlPanel.tsx:45-60/88-112）
- 注入脚本的 window.parent.postMessage 第二参数不用 "*"：srcdoc iframe 为 opaque origin，按规范其 origin 序列化为字符串 "null"，故目标 origin 用 "null"
- handleMessage 校验 e.origin === "null" + 校验 e.source === 本面板 iframe.contentWindow
- 重要：你无法启动真实 app 实测 WebView2 行为——按上述 opaque origin 规范实现，并在代码注释中明确记录："e.origin === 'null' 为 opaque origin 规范推断，未经真实 WebView2 实测，正确性由收尾 L4 验证"
- 重放到父 window 的合成 KeyboardEvent 加自定义信任标记（如自定义属性），ShortcutRegistry 分发前可识别
- html-panel.test.tsx 补用例：伪造 origin 的消息被忽略、非本 iframe source 的消息被忽略、本 iframe 消息正常转发

改完跑：npx tsc --noEmit && npx eslint src/ && npx vitest run html-panel inject-script 全绿。
`, { label: 'html-agent' }),
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

// === Phase 3: 逐项验证（断言清单唯一真值源：verify/stage-05.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 5 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/refacotr/workflows/verify/stage-05.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
