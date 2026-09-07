// =====================================================================
// Stage 11 Dockview 重设计（CP-004 单项 Stage；前置 = S07 + S10 完成）
// fix-loop constraints 值：panelId 页前缀协议先行落地+测试，再改消费点
// =====================================================================

export const meta = {
  name: 'stage-11-dockview-redesign',
  description: 'S11 Dockview 共享宿主+页组模型，MAX_PAGES 消亡（CP-004）',
  phases: [
    { title: '修复' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入。
【Stage 特殊纪律】panelId 页前缀协议（pageGroups.ts 纯函数族）先行落地+测试，再改消费点；布局存取只经 layoutSerde.ts（硬约束 #7 不破）；不得另起方向。
背景：修复步骤六段式真值源 = docs/compromises-fix/checklist.md CP-004 条目（先读再动手；本条为模块/契约级设计，协议与生命周期契约写死于条目）。`

// === Phase 1: 修复（单 agent 大重构）===
phase('修复')
const fixResult = await agent(`${PREAMBLE}

你负责 CP-004（多 Dockview 实例 + MAX_PAGES=20 → 共享宿主 + 页组分组模型）：
照抄 checklist CP-004 步骤 0-6：
0. 笔误先行：workspace/CLAUDE.md:15 ADR-0001 → ADR-0009。
1. 共享宿主结构：Workspace 渲染单一 DockviewReact；删 initializedPages 与 ensurePageInitialized；PageDockviewHost 改造为页组渲染组件或删除；window.__dockviewApi 收敛「宿主唯一」。
2. 页组分组模型：新建 src/workspace/pageGroups.ts 纯函数族（pageGroupId/panelIdInPage/pageOfPanelId/panelsOfPage，照抄代码块）；panelId 页前缀协议逐点核改（冲击面：titleManager 终端编号 local 契约/tabClose settings- 判据/TerminalRegistry 键/SEC-08 PtySession.panel_id 归属校验/DefaultTab params/findExistingEditor 查重键——逐点 grep 实查）；跨页组拖拽禁止（onDidAddPanel 校验越界回迁，panelBelongsToGroup 纯函数）。
3. 面板生命周期契约（条目写死）：addPanel 显式 group；切页 = 页组容器 display 切换；终端不随切页卸载（xterm open 一次，#4978 约束不变）；renderer="always" 白名单语义不变；页面删除先 kill 页组内终端再移除页组；restoreGuardRef 语义不变。
4. layoutSerde 契约演进：单宿主 JSON 一份 + 页组 id 索引；OperationPage.layout 语义改页组子树切片；loadPageGroup；patchLegacyLayout 增旧多实例格式迁移（无法归组面板丢弃 + console.error，不阻断启动）。
5. MAX_PAGES 消亡：删常量与 addPage 上限判定（含 toast 调用）；理由写进 stores/CLAUDE.md。
6. e2e 适配：wdio.conf.ts 双 reset 保留；e2e-tests/CLAUDE.md:58 句改「跨 spec 状态隔离」；helpers.ts 三 helper 改 getPageApi 页组查询。
测试：workspace 系全量适配（多实例/CSS 显隐/惰性初始化用例改页组语义）；projects 上限用例删并换「addPage 无上限」；layoutSerde 旧格式用例保留 + 迁移两分支新用例；pageGroups 纯函数用例 + 跨页拖拽回迁用例；settings 跨页单例（F11）回归；L4 terminal.e2e.ts H6 跨页存活与 settings.e2e.ts 跨页计数适配。
文档：workspace/CLAUDE.md H6 节整节重写；stores/CLAUDE.md FE-01/FE-36 节改写；adr.md ADR-0009 FE-01 行改写；panels/CLAUDE.md 白名单措辞核修。
自查：rg "MAX_PAGES|initializedPages|ensurePageInitialized" src/ 零命中；npx tsc --noEmit 绿。
完成后报告：修改文件清单 + 人工验证点待办（布局/面板全场景演练）。`, { label: 'cp004-dockview-redesign' })

// === Phase 2: 全量测试 ===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:/data/learn/code/slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. npx tsc --noEmit
2. npx eslint src/
3. npm test
4. npm run test:l3
5. npm run e2e
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证 ===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 11 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/compromises-fix/workflows/verify/stage-11.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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

return { fixResult, testResult, verifyResult }
