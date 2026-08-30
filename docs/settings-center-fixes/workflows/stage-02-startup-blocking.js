// =====================================================================
// Stage 02 启动链阻断 + 错误页（FE-02 / FE-03 / TE-01）
// =====================================================================
// fix-loop 调用约定：args.constraints 无需传值（无特殊纪律）
// 跨 agent 契约（写死，三方不各自推断）：
//   - markLoadSucceeded() / markPersistenceReady() 均从 stores/projects 导入
//     （App.tsx 用 "./stores/projects"，__tests__ 用 "../stores/projects"）——
//     两函数 Stage 01 已由 FE-01 落地/既有，本 Stage 只消费不改
//   - 错误页 data-e2e 三值：projects-load-error / projects-load-retry /
//     projects-load-continue-empty
//   - 「slTerminal 启动中…」文本节点保留在 projectsLoadError === null 分支
//     （startup-restore 用例 4/8 依赖）

export const meta = {
  name: 'stage02-startup-blocking',
  description: 'Stage 02 启动链阻断：加载失败阻断写门控 + 错误页 + 设置面板水合门控 + 测试适配',
  phases: [
    { title: '并行修复' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：禁止改 src-tauri/src/pty/ 任何 ConPTY flags（compute_conpty_flags 固定 0x7，含其 4 条守卫测试——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入）；禁止前端 src/ipc/ 外出现 invoke；禁止硬编码颜色（经 theme/colors.ts token）；禁止 npm run tauri dev 验证；禁止写入真实凭据值（SEC-18，仅允许 sk-test 假值占位符）。
背景：修复要点详见 docs/settings-center-fixes/checklist.md 对应 ID 条目（先读再动手）。
测试纪律：你只做编译级检查（npx tsc --noEmit 单文件不可行则跳过），禁止各自跑 npm test——真实执行统一由后续全量测试 agent 单点跑。
跨 agent 契约：markLoadSucceeded() / markPersistenceReady() 均从 stores/projects 导入（Stage 01 已落地/既有，只消费不改）；错误页 data-e2e 三值：projects-load-error / projects-load-retry / projects-load-continue-empty；「slTerminal 启动中…」文本节点保留在 projectsLoadError === null 分支。`

// === Phase 1: 并行修复（agent 间文件零重叠）===
phase('并行修复')
const parallelAgents = [
  {
    label: 'fe-02-app-error-page',
    prompt: `你负责 FE-02（照抄 docs/settings-center-fixes/checklist.md 的 FE-02 条目）：
【FE-02】src/App.tsx 启动链阻断 + 错误页（重试 / 以空状态继续）
- 位置：src/App.tsx init 加载段（约 :75-91）、\`if (!ready)\` 加载页（约 :257-277）
- 步骤（可照抄代码见 checklist FE-02 条目，逐项落实）：
  ① state 区新增 \`const [projectsLoadError, setProjectsLoadError] = useState<string | null>(null);\`
  ② 抽取 loadProjectsAndRestore() async 函数：E2E 分支（VITE_E2E === "1"）改调 markLoadSucceeded()；成功路径 markPersistenceReady() + lastPage 恢复逻辑原样保留（DBG-6 顺序不动：先 await setProjectRoot 再 setActivePage）
  ③ init 的 catch 改为：console.error("[App] 加载项目数据失败:", err); setProjectsLoadError(err instanceof Error ? err.message : String(err)); return;——不调 markPersistenceReady()、不 setReady(true)（阻断写门控）
  ④ 新增 retryProjectsLoad：setProjectsLoadError(null) → 重跑 loadProjectsAndRestore() → 成功 setReady(true)
  ⑤ 新增 continueWithEmptyProjects：markLoadSucceeded(); markPersistenceReady(); setReady(true);
  ⑥ 加载页 JSX 条件渲染：projectsLoadError === null 时原「slTerminal 启动中…」分支逐字不变（文本节点与 div 样式不动）；否则渲染错误页：容器 data-e2e="projects-load-error"，文案「项目数据加载失败」+ 错误详情 + 说明「可选择重试，或以空项目状态继续（磁盘上的项目数据不会被覆盖）」；两按钮 data-e2e="projects-load-retry"（「重试」）/ data-e2e="projects-load-continue-empty"（「以空状态继续」）；样式经 theme token（PANEL_BG 底 + SECONDARY_BG 按钮底 + SEPARATOR_BG 1px 边框 + DIM_FG 13px），禁止硬编码颜色
- markLoadSucceeded 从 ./stores/projects 导入（Stage 01 FE-01 已落地）。
- 你只改 src/App.tsx。`,
  },
  {
    label: 'fe-03-hydration-gate',
    prompt: `你负责 FE-03（照抄 docs/settings-center-fixes/checklist.md 的 FE-03 条目）：
【FE-03】src/panels/settings/SettingsPanel.tsx SC-FE-08 effect 水合门控
- 位置：src/panels/settings/SettingsPanel.tsx 自动关闭 effect（约 :350-398）
- 步骤：在 firstRun 消费之前插入（约 :360 后、:363 前）：
\`\`\`tsx
// ownProjectId===null && projects 空 = 未水合，保留 firstRun 待重跑
// （防误消费为「变化触发」致 dirty 分支误弹窗）
if (ownProjectId === null && Object.keys(projects).length === 0) return;
\`\`\`
判据语义：ownProjectId===null 且 projects 非空 = 项目已删，维持现状逻辑不拦截。
- 测试同步：src/__tests__/settings-panel-autoclose.test.tsx 新增用例：空 projects 首轮评估不关闭不消费 firstRun；水合后重跑走初始评估静默关闭。
- 你只改 src/panels/settings/SettingsPanel.tsx 与 src/__tests__/settings-panel-autoclose.test.tsx。`,
  },
  {
    label: 'te-01-startup-tests',
    prompt: `你负责 TE-01（照抄 docs/settings-center-fixes/checklist.md 的 TE-01 条目）：
【TE-01】启动链测试适配（语义反转 + 五文件 mock 补全）
- 位置：src/__tests__/startup-restore.test.ts（mock 工厂约 :120-123、用例 3 约 :186-205）、startup-store-fail-warn.test.tsx（约 :96-99）、close-handler.test.ts（约 :172-175）、error-boundary.test.tsx（约 :22-24）、e2e-clipboard-helper.test.ts（约 :44-46）
- 步骤：
  ① 五处 vi.mock projects store 工厂各补 \`markLoadSucceeded: vi.fn()\`（startup-restore 的在 :120-123 工厂对象内；先 Read 各文件确认工厂对象现状再改）
  ② startup-restore 用例 3 语义反转：loadAllProjects mockRejectedValueOnce 后断言——不 ready / markPersistenceReady 未被调用 / console.error 被调 / 渲染出 data-e2e="projects-load-error" 错误页
  ③ startup-restore 新增 3 例：错误页两按钮渲染（projects-load-retry / projects-load-continue-empty）；点重试成功进 ready（重试时 loadAllProjects mockResolvedValue）；点「以空状态继续」进 ready 且 markLoadSucceeded + markPersistenceReady 被调
- 契约：错误页 data-e2e 三值 projects-load-error / projects-load-retry / projects-load-continue-empty；markLoadSucceeded / markPersistenceReady 从 ../stores/projects mock 工厂导出。
- 你只改上述五个测试文件。`,
  },
]
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 2: 全量测试（命令相互独立，并行启动执行，收集全部结果）===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:\\data\\learn\\code\\slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. npx tsc --noEmit
2. npx eslint src/
3. npm test
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证（断言清单唯一真值源：verify/stage-02.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 02 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/settings-center-fixes/workflows/verify/stage-02.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
