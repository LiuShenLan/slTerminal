// =====================================================================
// stage-08-errors.js — S08 错误处理体系（FE-02/03/05~10、BE-13/15）
// =====================================================================
// 跨边界契约（写死）：
//   src/ipc/appError.ts 导出：
//     parseAppError(err: unknown): { variant: string; message: string } | null
//       ——按 camelCase 变体名解析后端 AppError 序列化形态
//     getErrorMessage(err: unknown): string ——提取用户可读消息，兜底 String(err)
//   src/lib/index.ts re-export 两函数。
//   后端新增 AppError::ConfigParse 变体（BE-15）——变体总数 10+1=11，FE-02 测试全覆盖。
//   toast 契约：toast.show(type: "success" | "warning" | "error", message: string)。
// fix-loop 调用约定：args.testCommands 省略（默认基础五条）；constraints 传空。
// =====================================================================

export const meta = {
  name: 'stage08-errors',
  description: 'Stage 08: 统一 AppError 解析器 + ConfigParse 变体 + 启动链/终端/编辑器错误可感知化（FE-02/03/05~10、BE-13/15）',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
背景：修复要点详见 docs/review-fix/checklist.md 对应 ID 条目 + docs/review-fix/stages.md S08 节（先读再动手）。
测试纪律：本阶段禁止跑资源共享型全量测试（cargo test / npm test / npm run test:l3）——只做编译级检查（cargo check / npx tsc --noEmit），全量执行由独立测试 agent 单点跑（跨进程并发会死锁，cargo 排队属正常）。
本 Stage 契约：src/ipc/appError.ts 导出 parseAppError(err): { variant, message } | null（camelCase 变体名解析）与 getErrorMessage(err): string（兜底 String(err)）；src/lib/index.ts re-export；后端新增 AppError::ConfigParse（变体总数 11）；toast.show("success"|"warning"|"error", message)。`

// === Phase 1: 并行重构（agent 间文件零重叠；不跑资源共享型测试）===
phase('并行重构')
const parallelAgents = [
  {
    label: 'error-infra',
    prompt: `你负责 FE-02，只许新建 src/ipc/appError.ts、改 src/lib/index.ts、新建 src/__tests__/ 下测试文件：
新建 src/ipc/appError.ts：parseAppError(err) 按契约解析后端 AppError 序列化形态（camelCase 变体名，形如 { configParse: "..." } 或既有序列化惯例——先 Read src-tauri/src/error.rs 确认 serde 形态）；getErrorMessage(err) 提取用户可读消息。src/lib/index.ts re-export。
补 L2 测试：全 11 变体（含新 ConfigParse）解析 + 非 AppError 输入兜底。
只做 npx tsc --noEmit 编译级检查，禁止 npm test。`
  },
  {
    label: 'error-backend',
    prompt: `你负责 BE-13、BE-15，只许改 src-tauri/src/error.rs、src-tauri/src/fs/mod.rs、src-tauri/src/settings.rs、src-tauri/src/projects.rs：
【BE-15】error.rs 新增 ConfigParse 变体（配置 JSON 损坏场景）；用户可见消息改业务语义（如「保存设置失败」），技术细节进 tracing 日志；Notify/IoKind 变体承载异构错误的问题随之一并梳理（消息语义化，不拆变体体系）。
【BE-13】From<std::io::Error> 本身不动；fs/settings/projects 命令内 map_err 在调用点注入路径上下文（错误消息含路径）。
补 L1 测试：ConfigParse 序列化形态（与前端 parseAppError 契约对齐）；含路径的错误消息断言。
只做 cargo check 编译级检查，禁止 cargo test。`
  },
  {
    label: 'error-consumers-app',
    prompt: `你负责 FE-03、FE-05、FE-06、FE-09，只许改 src/main.tsx、src/App.tsx、src/stores/fontSize.ts、src/stores/keybindings.ts、src/stores/sideBar.ts 及 src/__tests__/ 下对应测试：
【FE-03】main.tsx:38 与 App.tsx:44-69 启动链各 catch 至少 console.warn 带模块名（降级兜底逻辑不动；项目数据损坏的 corrupted 通道 toast 在 S09 做，本 Stage 不碰）。
【FE-05】App.tsx 关闭序列 pty.kill 失败收集 → 全部完成后统一一条 console.error 汇总（含失败数），替代逐条 console.error。
【FE-06】App.tsx:198 requestUserAttention 的 .catch(() => {}) 内补 console.warn（非关键路径，不 toast）。
【FE-09】fontSize/keybindings/sideBar 三 store 保存失败统一 toast.show("warning", "设置保存失败，重启后将丢失")（toast 经 src/lib 导入；错误消息统一经 getErrorMessage——契约已写死，直接从 src/lib import，不依赖其实现完成）。
补 L2 测试：各 catch 路径 console.warn/toast 断言。
只做 npx tsc --noEmit 编译级检查，禁止 npm test。`
  },
  {
    label: 'error-consumers-panels',
    prompt: `你负责 FE-07、FE-08、FE-10，只许改 src/features/explorer/useFileTree.ts、src/features/explorer/ExplorerPanel.tsx、src/panels/terminal/useXterm.ts、src/panels/editor/useCodeMirror.ts、src/panels/diff/DiffPanel.tsx 及 src/__tests__/ 下对应测试：
【FE-07】useFileTree 的 loadDirectory catch 返回空数组伪装空目录（:46-58）。修复：store 增加按路径 error 状态；ExplorerPanel 渲染错误占位（错误消息 + 重试按钮）。
【FE-08】useXterm 多处静默 catch（:188,272,309-329,374,445,498,547）：非关键路径（resize/kill/openUrl）保留 console.error；关键路径（spawn 失败、write 连续失败 ≥3 次）toast；错误消息统一经 getErrorMessage（从 src/lib import，契约已写死）。
【FE-10】DiffPanel（:470,477）git diff 失败加「内容可能过时」面板内提示条；useCodeMirror（:197,414,427）外部修改重载失败保留 console.warn + 编辑器状态条提示。
补 L2 测试：错误占位渲染、toast 触发条件、提示条显示。
只做 npx tsc --noEmit 编译级检查，禁止 npm test。`
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
逐项检查 Stage 08 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/review-fix/workflows/verify/stage-08.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
