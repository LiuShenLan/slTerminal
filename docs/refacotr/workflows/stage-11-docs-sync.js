// Stage 11 — 文档同步（DOC-01~09、DOC-11、DOC-12、DOC-13、TE-17；DOC-10 已在 Stage 7 完成）
// 用法：主 agent 以 Workflow({ scriptPath: 本文件 }) 调用
// 3 个并行 agent，文档文件不重叠（对齐 refactoring-stages.md Stage 11 分工表）
// 固定为最后一个 Stage——文档必须反映 Stage 1-10 完成后的最终代码状态

export const meta = {
  name: 'stage11-docs-sync',
  description: 'Stage11: CLAUDE.md 与 test-inventory 全量同步最终代码状态',
  phases: [
    { title: '并行文档同步' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目：slTerminal（Tauri 2 + React/TS 终端模拟器），根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改分配给你的文档文件，不改代码；中文写作；渐进式披露（根文件只留全局约束+索引，细节留子路径）。完成后报告修改的文档清单与每项改动摘要。
背景：Stage 1-10 已完成 77 项修复。先用 git log --oneline（最近 10+ 条）与各 commit diff 了解实际改动，再动笔——禁止凭记忆写文档。
核对基准：docs/refacotr/refactoring-checklist.md 对应 DOC ID 条目 + 当前代码实际状态。`

phase('并行文档同步')
const docResults = await parallel([
  () => agent(`
${PREAMBLE}

你负责根级文档（DOC-01 根部分 + DOC-03 + DOC-04 + TE-17）：

【DOC-01 根部分】根 .claude/CLAUDE.md 约束 #10
- 改述为"Tauri 2 自定义命令默认放行，capabilities 只管插件权限"（不再要求逐条放行自定义命令）

【DOC-03 + TE-17】.claude/test-inventory.md 重写
- 用 grep -cE '^\\s*(it|test)\\(' 实测各 L2 测试文件用例数、cargo test -- --list 或 grep '#\\[test\\]' 实测 L1
- L1/L2/L3/L4 数字全部以实测为准（含 Stage 9 新增用例）；统一计数口径为"vitest 展开用例数"并声明
- 标注 E2E 键盘局限（TE-17 并入）；文末声明本文档为用例数唯一真值源

【DOC-04】根 .claude/CLAUDE.md
- 移除不存在的 claude/ 模块引用（架构章节与相关描述）
- 测试策略表的 L1/L2 用例数改为引用 test-inventory.md（不再各自维护数字）
`, { label: 'root-docs-agent' }),

  () => agent(`
${PREAMBLE}

你负责 src/ 下前端文档（DOC-02 shortcuts 部分 + DOC-06/07/08/09/11/12）：

【DOC-02 shortcuts 部分】src/features/shortcuts/CLAUDE.md
- HTML iframe 键转发章节改述 postMessage 注入脚本机制（HtmlPanel 注入脚本 → postMessage → 父窗口 handleMessage → ShortcutRegistry）；注明 forwardGlobalShortcuts.ts 已删除（Stage 8 FE-13）

【DOC-06】src/workspace/CLAUDE.md
- Workspace.tsx "已废弃"描述失实——改述为主组件（三栏布局+页面生命周期），单页渲染委托 PageDockviewHost

【DOC-07】src/stores/CLAUDE.md
- 持久化规则改述：fontSize/keybindings 委托 ipc/settings；projects 委托 ipc/fs

【DOC-08】src/ipc/CLAUDE.md
- 模块映射表补 index.ts 的 ping()

【DOC-09】src/panels/CLAUDE.md
- 删除 useFontSizeBridge 引用（Stage 6 FE-12 已删文件，实际用 src/lib/useFontSizeWheel.ts）
- HTML 注入脚本改述三段式（CSS 注入+键盘转发 postMessage+片段拦截，无 location.hash 依赖）
- 用例数改为引用 test-inventory.md（DOC-11）

【DOC-11】src/__tests__/CLAUDE.md、src/features/shortcuts/CLAUDE.md
- 用例数同样改为引用 test-inventory.md；panels/CLAUDE.md 的 useXterm 行数描述删除

【DOC-12】src/panels/CLAUDE.md（sandbox 决策节）
- 补录 SEC-03 修复后的 postMessage origin 校验机制（e.origin === "null" + e.source 校验 + 信任标记）与威胁模型
`, { label: 'frontend-docs-agent' }),

  () => agent(`
${PREAMBLE}

你负责后端与 e2e 文档（DOC-01 pty 部分 + DOC-05 + DOC-02 e2e 部分 + DOC-13）：

【DOC-01 pty 部分】src-tauri/src/pty/CLAUDE.md
- 注意事项 #1（capabilities 放行）与根约束 #10 新表述对齐（自定义命令默认放行）

【DOC-05】src-tauri/src/pty/CLAUDE.md JobHandle 章节
- 描述与 Stage 1 后代码一致：非 Windows 零占位始终初始化（JobHandle::new_dummy()），无 #[cfg(windows)] 条件初始化

【DOC-02 e2e 部分 + DOC-13】e2e-tests/CLAUDE.md
- HTML Ctrl+W 用例描述改述 postMessage 注入脚本路径（forwardGlobalShortcuts 已删除）
- 用例数引用 test-inventory.md；与 DOC-02/DOC-03 结论一致
`, { label: 'backend-docs-agent' }),
])

// === Phase 2: 全量测试（命令相互独立，并行启动执行，收集全部结果）===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:\\data\\learn\\code\\slTerminal 执行全量验证（文档 Stage 不应影响任何测试，此步为回归确认）。以下命令相互独立，并行启动执行，收集全部结果：
1. npx tsc --noEmit
2. npx eslint src/
3. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
4. npm test
5. cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1（必须单线程，ConPTY 并发 spawn 死锁）
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。另报告 L1/L2 实际用例总数（供 test-inventory 核对）。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证（断言清单唯一真值源：verify/stage-11.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 11 的文档同步是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/refacotr/workflows/verify/stage-11.md 获取断言清单，用 Grep/Read/git status 逐条核实并给出证据。
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

return { docResults, testResult, verifyResult }
