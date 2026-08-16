# UI 重设计 Review 修复清单（ui-redesign-review-fix）

> 真值源：`docs/ui-redesign-review/` 7 份报告（去重后 47 条）+ 本清单头部的决策留痕。
> 范围约定：**全部问题均修复，不按优先级裁剪**（用户决策 2026-08-17）；条目中的 P 级别仅作溯源留痕。
> 编号：SPEC=规范修订 / FE=前端代码 / TE=测试 / DOC=文档 / VER=核实项（不改码）。Stage 划分按 ID 引用。
> 每条代码改动项附带常驻回归用例要求（硬约束 #11）；用例增删同步 `.claude/test-inventory.md`（归 Stage 09 统一登记）。

## 决策留痕（用户澄清结论，2026-08-17）

| # | 决策点 | 结论 | 落点 |
|---|--------|------|------|
| D1 | 字号矛盾（review P2-1/2/3：终端/编辑器/错误页默认 14px vs 阶梯 11–13px） | **改规范**：内容区默认 14px 登记为例外，代码不动 | SPEC-01 |
| D2 | 文件树行高 24px vs 规范树行 28px（review P2-14） | **改规范**：登记导航树 28/会话行 30/文件树 24 分档，代码不动 | SPEC-02 |
| D3 | HistorySessionList/Row 退役组件 | **删除**，测试迁移 NavHistoryRow；连带消解 hover 残留与反查重复实现 | FE-25 |
| D4 | navTree 穿透 TerminalRegistry（review P1-5） | 反查逻辑**上提 workspace/pageApis** | FE-09 |
| D5 | 规范修订方式 | 直接修订 requirements.md/design.md 原文，本清单留痕 | SPEC-01/02 |
| D6 | review 未实机验证 6 项 | 纳入 execution-plan 收尾人工验证清单 | stages.md 末节 |
| D7 | 产出结构 | 沿用 impl 四件套（checklist+stages+execution-plan+workflows） | 本目录 |
| D8 | 执行时机 | 本任务仅出计划；执行由 `/systematic-changes-execute` 触发 | — |

## 核实修正（对 review 判定的一手证据复核）

- **VER-01**：`panel.view?.contentComponent`（review 03 P2 判「非公开 API」）——**核实为 dockview 公开类型成员**：`IDockviewPanel.view: IDockviewPanelModel`（dockviewPanel.d.ts:12）→ `IDockviewPanelModel.readonly contentComponent: string`（dockviewPanelModel.d.ts:7,20）。**不改代码**；`src/workspace/CLAUDE.md` F8 段已记载「panel.component 不存在」的判据说明，Stage 09 补一句公开性注记即闭环。
- **main.tsx:28-31 错误页硬编码色**（review 02 列入 P1-4）——**属已登记 fail-safe**（先于方案加载，无法走 token）：`linear.ts:8-10` 文件头交叉引用 + `theme/CLAUDE.md` 启动链节均有登记。**不改代码**。
- **FileIcon 六色盘 + NavProjectRow 项目蓝**（review 02 列入 P1-4）——**属已登记硬编码例外**（IC-04 契约，`explorer/CLAUDE.md` 文件表 + `navTree/CLAUDE.md` 硬约束节）。真实缺陷是**根 CLAUDE.md 硬约束 #6 的例外指向错位**（写「既定例外见 src/panels/CLAUDE.md」，而 panels/CLAUDE.md 无例外清单）→ DOC-09 修正指向。
- **同模式漏列**：`theme/CLAUDE.md` 的 tauri.conf.json 行号 `:20` 失实（实际 `:21`）——`linear.ts:9` 文件头注释同病（review 仅报前者），DOC-07 一并修正。

## SPEC（规范修订——直接改原文，代码不动）

- **SPEC-01**（D1，消解 review P2-1/2/3）字号阶梯登记内容区 14px 例外：`docs/ui-redesign/requirements.md:30`（UI-204「编辑器终端 12.5–13px」）与 `docs/ui-redesign/design.md:106-115`（字号阶梯表「编辑器/终端 12.5–13px」行）修订为「12.5–13px 为设计基准，终端/编辑器**内容区默认 14px**（用户可调 8–32，Ctrl+Wheel）属登记例外」；`src/App.css:9` 注释口径同步（组件 chrome 字号阶梯不变，内容区例外一句注记）。代码侧 `fontSize.ts:17`、`panels/terminal/theme.ts:12`、`useCodeMirror.ts:139,296`、`main.tsx:30` **保持 14 不动**。
- **SPEC-02**（D2，消解 review P2-14）树行高分档登记：`docs/ui-redesign/requirements.md:42`（UI-305「树行 28」）与 `docs/ui-redesign/design.md:112,121` 修订为「导航树行 28 / 会话行 30 不变；**文件树（explorer）行 24px**——紧凑列表档登记」；`src/features/explorer/CLAUDE.md` 文件表 FileTree 行补行高 24 登记。代码侧 `FileTree.tsx:202` **保持 24 不动**。

## FE（前端代码修复）

### 浮层收尾（Stage 01）

- **FE-01**（review P1-1）`src/panels/editor/useCodeMirror.ts` 原生弹窗 3 处：`:177` 保存失败 `window.alert` → `toast.show("error", ...)`；`:265`、`:392` 外部修改/脏确认 `window.confirm` → `await confirmDialog({...})`（调用处改 async，语义对照现状：确认=继续/取消=中止）。回归：use-code-mirror / editor-confirm 测试改 mock `confirmDialog`（不 mock window），断言 toast/confirmDialog 调用参数。
- **FE-02**（review P1-1）`src/panels/diff/DiffPanel.tsx:363,457` 两处 `window.confirm` → `confirmDialog`（语义同上）。回归：diff-panel 测试脏弹窗分支同步。
- **FE-03**（review P1-1）`src/features/navTree/NavTree.tsx:565-569` 项目删除 `window.confirm` → `confirmDialog({ title, message, danger: true })`（action 改 async）。回归：nav-tree 测试删除项目用例改 mock confirmDialog。E2E 无项目删除用例（grep e2e-tests 实证），L4 不受影响。
- **FE-09**（D4，review P1-5）反查函数上提：`findPanelForSession`/`findPageIdForPanelId`（NavTree.tsx:107-139）移至 `src/workspace/pageApis.ts` 导出（纯函数形态，依赖 TerminalRegistry/useProjects/parseTerminalPageId/keyOf/basename——workspace 引用 TerminalRegistry 为既有先例，见 PageDockviewHost）；NavTree 删 `:56` import 与本地实现，调用点（`:325,333,340`）改调 pageApis。回归：pageapis.test.ts 增两函数全分支用例（复合键命中/usageSourcePath 回退/未命中；前缀匹配/parse 兜底/null）。
- **FE-24**（review 03 P2）`NavTree.tsx:309` `handleNewPage` 死返回：删返回值（调用方不消费 pageId）。

### 配色单点收敛（Stage 02）

- **FE-07**（review P1-4）`TitleBar.tsx:22` 关闭 hover `#c04747` token 化：`linear.ts` ui 段新增标量 `titlebarCloseHover: "#c04747"`（UI-301 定值）→ `types.ts` UiTokens 槽位（含消费注释）→ `colors.ts` facade 导出 `TITLEBAR_CLOSE_HOVER_BG` → TitleBar 改引用。**消费方同步**：`scheme-registry.test.ts` 标量计数 26→27；`colors.test.ts` token 集合；`theme/index.ts:3` 注释 34→35；`theme/CLAUDE.md` 计数描述（Stage 09 统一）。
- **FE-08**（review P1-4）`App.css` 硬编码 4 处（`:59,63,66` 滚动条 rgba 三色 + `:75` focus `#6e9ff2`）→ `colors.ts` `ROOT_CSS_VARS` 扩 4 键并改 `var()` 引用。键名写死（跨 agent 契约）：`--sl-focus-border` ← `ui.focusBorder`；`--sl-scrollbar-slider` / `--sl-scrollbar-slider-hover` / `--sl-scrollbar-slider-active` ← `terminal.scrollbarSliderBackground/Hover/Active`（值与现硬编码逐字相同：rgba(255,255,255,0.10/0.20/0.28)、#6e9ff2，附录 A 一致）。`App.css:5,72-73` 「var() 不可行」注释重写。main.tsx 注入循环零改动（Object.entries 通用）。**消费方同步**：`colors.test.ts:241-258` 键集合断言。回归：colors 测试断言 6 键集合 + 值映射。
- **FE-20**（review 03 P2）`FileIcon.tsx:104-115` 文件夹分支忽略 `gitStatus`：`isDir` 分支 `color` 由 `statusColorMap[gitStatus] ?? EXPLORER_COLORS.fg` 取值（与文件分支同一映射）。回归：file-icon 测试增文件夹+gitStatus 着色用例。
- **FE-21**（review 03 P2）`TitleBar.tsx:50-67` 订阅整个 projects 对象：拆轻量 selector（activePageId + 按 activePageId 推导标题的窄 selector，或 titles 派生上提 store selector——执行期按最小改动定），任意项目变更不再触发 TitleBar 全量遍历重渲染。回归：title-bar 测试增「无关项目变更不重渲染/标题不变」用例。

### 右键菜单与树行（Stage 03）

- **FE-04**（review P1-2）`PageDockviewHost.tsx:262` `nextPanelId()` 菜单构建期消耗编号：调用移入「新建终端」action 闭包内（`const newTerminalId = nextPanelId()` 延迟到点击时）。回归：workspace-header-actions 增「连续两次构建菜单后执行新建，编号不跳号」用例。
- **FE-16**（review P2-11）`PageDockviewHost.tsx:109` Watermark「新建终端」按钮圆角 4→6（UI-306 按钮档）。
- **FE-05**（review P1-3）`FileTree.tsx:207-216` hover 直接改 DOM `style.background` → React state 驱动（参考 NavProjectRow/TabContextMenuItem 既有模式；选中态优先级语义不变）。回归：explorer 测试增 hover 背景 state 断言（fireEvent mouseEnter/Leave 后 style 正确）。
- **FE-06**（review P1-3）`NavContextMenu.tsx:101-106` 同上改 React state 驱动。
- **FE-15**（review P2-10）`NavPageRow.tsx:108` 重命名输入框圆角 4→8（UI-306 输入框档）。

### 对话框与通知形态（Stage 04）

- **FE-12**（review P2-6/7）`ConfirmDialog.tsx:162,163,179`：主/次按钮圆角 4→6（UI-306）；删次按钮多余 1px 描边（UI-803 只规定底色/字色）。回归：confirm-dialog 测试视觉断言同步。
- **FE-28**（review 03 P3）ConfirmDialog 焦点管理：挂载后主按钮 `autoFocus`/`focus()`；Enter 确认、Esc 取消语义保持；Tab 在取消/确认两钮间循环（焦点陷阱）。**data-e2e="confirm-ok"/"confirm-cancel" 选择器不变**（history.e2e.ts:602 依赖）。回归：confirm-dialog 测试增聚焦/Enter/Tab 循环用例。
- **FE-13**（review P2-8）`TerminalRenameDialog.tsx:141,173,188`：输入框圆角 4→8、确定/取消钮 4→6。回归：terminal-rename-dialog 测试断言同步。
- **FE-14**（review P2-9）`SessionActionDialog.tsx:122,144`：按钮圆角 4→6。
- **FE-29**（review 03 P3）`toast.tsx:93-125` ToastHost 容器加 `role="status" aria-live="polite"`。回归：toast 测试增属性断言。

### 侧栏与活动栏（Stage 05）

- **FE-17**（review P2-12）`ActivityBar.tsx:65` 删 `transition: background-color 0.15s`（UI-110 硬约束无动效）。回归：activityBar 测试增「style 不含 transition」负断言。
- **FE-23**（review 03 P2）`ActivityBar.tsx:226-231` `onDragLeave` 误清指示线：改 `relatedTarget` 判断真正离开容器（或统一 dragend/drop 清理——执行期取最小改动）。回归：activityBar 测试增容器→子元素转移不清指示线用例。
- **FE-19**（review 03 P2）`SideBarArea.tsx:58-63` 双开时 `setSplitRatio(0.5)` 无条件重置：改仅「首次进入双视图」或「持久化值越界」时回退默认；正常单↔双切换保留用户调节值。回归：sideBar 测试增切换保留 splitRatio 用例。
- **FE-22**（review 03 P2）`sideBarState.ts:184,190-194` `reconcileZones` mutate 入参数组：`top.push` 前先 `[...top]` 复制，保纯函数语义。回归：sideBarState 测试增入参不被 mutate 断言。

### 杂项收敛（Stage 06）

- **FE-10**（review P2-4）`App.tsx:214-216` 启动加载页：`fontFamily: "monospace"` → 全局字体栈（UI-201）；色误用 `INPUT_BORDER` → `DIM_FG`（说明文字 fg-3 档）。回归：App 加载页测试断言同步。
- **FE-11**（review P2-5）`ErrorBoundary.tsx:63,125` `fontFamily: "monospace"` → 全局字体栈（两处：全屏 variant + inline variant）。回归：error-boundary 测试断言同步。
- **FE-18**（review P2-13）`GitShowPanel.tsx:133` 大文件警告 `⚠` → `icons.tsx` 新增 `IconAlertTriangle`（lucide 对应导出，执行期按 lucide-react 实际导出名定）替换（IC-08 emoji 禁令）。回归：gitshow-panel 测试断言改 svg 存在性；emoji-scan 守卫不受影响（⚠ 本就不应在 src/ 出现）。
- **FE-26**（review 03 P3）`App.tsx:171` + `src/ipc/window.ts:56-69`：`registerCloseHandler` 清理函数 `unlisten.then(fn=>fn())` 补 `.catch(() => {})`（窗口已销毁时 reject 吞掉）。落点执行期定（优先 window.ts 内部收口）。回归：App/window 测试增 unlisten reject 不抛用例。
- **FE-27**（review P3）`ExplorerPanel.tsx:446-452` 文件树容器 `tabIndex={-1}` 却 `outline:"none"`：删 outline 抑制，由全局 `:focus-visible` 接管（UI-808——鼠标点击本就不匹配 :focus-visible，视觉无变化；键盘编程聚焦时可见）。回归：explorer-focus 测试增样式断言。

### 退役组件删除（Stage 07）

- **FE-25**（D3，消解 review P3-1 + P1-3 的 HistorySessionList 处 + 03 反查重复实现）删除 `HistorySessionList.tsx` + `HistorySessionRow.tsx`（生产零消费方，仅测试引用）：
  1. 删两文件；`agentHistory/index.ts:7,8,12,13` 删导出
  2. `src/__tests__/agent-history-row.test.tsx` 迁移：仍有独立语义的用例改写为 `NavHistoryRow` 面向（归入 nav-tree-history 测试或同目录新文件），已被 nav-tree-history.test.tsx 覆盖的重复语义删除
  3. 孤儿 helper 清理（仅限本次删除所孤儿者）：执行期 grep `groupByCwd` 等仅 List 消费的导出，确认零引用后删
  4. 代码注释历史引用更新：`lib/panelId.ts:8,69`、`NavTree.tsx`（多处「照 HistorySessionList」）、`useNavTree.ts:137`、`NavHistoryRow.tsx:8`、`NavContextMenu.tsx:54`、`historyContextMenu.ts:11,13`——迁移注记改写为「原 HistorySessionList（已删）」口径或就近重写；`lib/CLAUDE.md:61` 的解析调用点防御分层描述同步（Stage 09 统一）
  5. 回归：L2 全量绿即门禁；test-inventory 用例迁移登记（Stage 09）

## TE（测试质量——Stage 08，本 Stage 只改测试/e2e 辅助，不动生产代码）

- **TE-01**（review 05 P2）`nav-tree.test.tsx:605-618` 「父节点因子」假守卫：查询词 `"Beta"` 同命中项目名与页面名——改仅命中项目名的查询（如 `"项目Beta"`），断言页面行因父命中而显示。
- **TE-02**（review 05 P2）`nav-tree-history.test.tsx:289-294` 重扫次数 `toBeGreaterThanOrEqual(2)` 过宽：改精确次数差值断言（展开前后调用次数严格递增的精确值）。
- **TE-03**（review 05 P2）`e2e-tests/specUtils.ts:250` `waitForPanelTabIcon` 更名 `waitForPanelTabStatus`（IC-03 字段已改 tabStatus）+ 注释术语统一；调用点同步：`mockcli.e2e.ts:39,255`、`hooks.e2e.ts:23,118,134,204`。**本 Stage 门禁补 `npm run e2e`**（e2e-tests 不在根 tsconfig include 内，构建级兜底）。
- **TE-04**（review P3）`sideBarState.test.ts:24-53` 测试数据用已退役 id `"projects"`：改 `"nav"` 或测试专用 id，与生产默认不混淆。
- **TE-05**（review P3）`activityBar.test.tsx:276-280,503-512,514-528` 「不抛异常即通过」用例：补 dropIndicator DOM 状态/事件调用次数/清理后样式实断言。
- **TE-06**（review P3）`workspace-page-dockview.test.tsx:276-285` FileIcon 页签测试粒度：断言 svg 确为 FileIcon（特征 path/色块 fill）+ 增非文件面板（terminal）不渲染 FileIcon 反向用例。

## DOC（文档同步——Stage 09 统一执行，反映全部代码 Stage 完成后终态）

- **DOC-01**（review P1-6）`.claude/test-inventory.md:370` 删除已失实的 `⚠️` 警告段（a7b0e90 已改 ConfirmDialog 形态），history.e2e.ts 覆盖描述更新为 `data-e2e="confirm-ok"` 点击语义。
- **DOC-02**（review P1-7）`e2e-tests/CLAUDE.md:36` history.e2e.ts「8 条 active」→「7 条 active」，移除「孤儿行 ✗」条目；`:38` 附近 `waitForPanelTabIcon` 术语随 TE-03 改 `waitForPanelTabStatus`。
- **DOC-03**（review P1-8）`src/stores/CLAUDE.md:36` 侧栏默认态改三槽 `nav/explorer/commit`（`sideBarState.ts:33-42` 现状），删 `projects`/`agent-status` 四槽描述。
- **DOC-04**（review 06 P2）`src/features/shortcuts/CLAUDE.md:126` hooks 配置入口描述改「活动栏底部配置钮 → `openHooksConfigFromActivityBar`」，删侧栏右键菜单/SidebarTree 引用。
- **DOC-05**（review 06 P2）`src/ipc/CLAUDE.md:24` 「六个 wrapper」→「七个 wrapper」（onFocusChanged/requestUserAttention/setFocus/registerCloseHandler/minimizeWindow/toggleMaximizeWindow/closeWindow）。
- **DOC-06**（review 06 P2）`src/panels/CLAUDE.md:270` 附近 index.ts 文件表补 `HooksConfigPanel` 导出行。
- **DOC-07**（review 06 P3 + 同模式漏列）`src/theme/CLAUDE.md:64` 与 `src/theme/schemes/linear.ts:9` 的 `tauri.conf.json:20` → `:21`（一手证据：backgroundColor 实在 :21；index.html:10 / main.tsx:28 经核实无误）。
- **DOC-08**（review 06 P3）`src/workspace/CLAUDE.md:44` index.ts 文件表导出项补全（PANEL_TERMINAL/PANEL_EDITOR/PANEL_HTML_VIEWER/FILE_PANEL_TYPES/isValidPanelType/isAlwaysRenderPanel 等，照 `src/workspace/index.ts` 现状核对）；pageApis.ts 行补 FE-09 新增两导出 + VER-01 公开性注记（contentComponent 为 dockview 公开类型成员）。
- **DOC-09**（核实修正项）根 `.claude/CLAUDE.md`：① 硬约束 #6「既定例外见 ../src/panels/CLAUDE.md」指向修正——例外实际登记于 `theme/CLAUDE.md`（fail-safe 三处 + 终端 adapter）、`explorer/CLAUDE.md`（六色盘）、`navTree/CLAUDE.md`（项目蓝），措辞改为汇总指向或例外清单全表；② 模块索引 agentHistory 行随 FE-25 更新（删 HistorySessionList/Row 表述）；③ 需求编号索引如需补 FIX 族条目（执行期定）。
- **DOC-10**（伴随同步）`agentHistory/CLAUDE.md`（FE-25 后：:19 保留段删除、:58/:68/:72/:80-81 文件表与决策更新）；`navTree/CLAUDE.md`（删除项目改 confirmDialog 描述 + 反查上提 pageApis 登记 + 硬约束节 TerminalRegistry 引用表述更新）；`lib/CLAUDE.md:61`（HistorySessionList 提及删除）；`explorer/CLAUDE.md`（FE-27 outline 修复 + 行高 24 档登记——**该文件归 Stage 09 spec-revise agent 承担**，避免与 claudemd-c 文件重叠）。
- **DOC-11** `.claude/test-inventory.md` 全量同步：本修复全部新增/修改/删除用例逐条登记（硬约束 #11 + 用例清单同步规则）；含 TE-03 更名、FE-25 迁移、各 Stage 新增回归用例。

## 附：修复→Stage 映射速查

| Stage | 包含 ID | 性质 |
|---|---|---|
| 01 浮层收尾 | FE-01/02/03/09/24 | fix |
| 02 配色收敛 | FE-07/08/20/21 | fix |
| 03 菜单与树行 | FE-04/05/06/15/16 | fix |
| 04 对话框形态 | FE-12/13/14/28/29 | fix |
| 05 侧栏状态 | FE-17/19/22/23 | fix |
| 06 杂项收敛 | FE-10/11/18/26/27 | fix |
| 07 退役删除 | FE-25 | refactor |
| 08 测试强化 | TE-01~06 | test |
| 09 规范与文档 | SPEC-01/02 + DOC-01~11 + VER-01 注记 | docs |
