# Stage 5 逐项验证断言（唯一真值源）

> stage-05 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。
> 覆盖清单 ID：SB-26（docs/sidebar-checklist.md「Stage 5 — 文档同步」）八处文档。
> **文档类断言总则（语义式，防文档撒谎）**：文档描述的签名/行为/用例数必须与当前真实代码、测试实际输出一致——verify agent 须对照代码抽查，不接受照抄计划文档。

## 断言清单

- **SB-26-1**：`src/features/sideViews/CLAUDE.md` 存在，含：职责、状态机单槽位模型（无历史记忆）、SideViewRegistry 扩展指南（register 一行注册）、HTML5 拖拽说明、文件表、测试模式章节（Read 确认章节齐全）；**抽查**：文中描述的 `sideBarState.ts` 导出函数名与真实代码一致（对照 `src/features/sideViews/sideBarState.ts`）。
- **SB-26-2**：`.claude/CLAUDE.md` 模块表新增 `src/features/sideViews` 行（grep `sideViews` 命中）。
- **SB-26-3**：`src/workspace/CLAUDE.md` 布局段重写：描述活动栏（40px 固定）+ 侧栏区（`visible=anyOpen`、内含垂直 Allotment 上下半区、SideBarArea 视图槽 display:none 保挂载）+ 主区三 pane 结构；宽度/比例持久化说明（grep `活动栏|侧栏区|SideBarArea` 命中）；**抽查**：与 `src/workspace/Workspace.tsx` 当前 pane 结构一致。
- **SB-26-4**：`src/stores/CLAUDE.md` Store 清单新增 `sideBar.ts` 条目：状态形状（zones/open/width/splitRatio）、settings.json `sideBar` 段、默认态（grep `sideBar` 命中）。
- **SB-26-5**：`src/features/sidebar/CLAUDE.md` 补充宿主变化说明（常驻栏 → 侧栏区视图槽，组件本体不变——grep `侧栏视图|视图槽|sideViews` 命中）。
- **SB-26-6**：`src/features/explorer/CLAUDE.md` 同 ⑤ 补宿主变化，且含**换区重建丢展开状态**的已知行为说明（grep `换区|重建|展开状态` 命中）。
- **SB-26-7**：`e2e-tests/CLAUDE.md` helpers 表新增 3 个 helper（grep `__slterm_e2e_getSideBarState|__slterm_e2e_toggleSideView|__slterm_e2e_moveSideViewButton` 命中）+ describe 清单加「侧栏视图」2 用例（grep `侧栏视图` 命中）；**抽查**：helper 名与 `e2e-tests/helpers.ts` 真实代码一致。
- **SB-26-8**：`.claude/test-inventory.md` L2 新增「侧栏视图」类目（6 个测试文件：sideBarState / sideViewRegistry / sideBar / activityBar / sideBarArea / workspace-sideviews——grep 命中），用例数与 `npm test` 实际输出一致（**以测试 agent 报告的数字为准核对**，不接受估算）；总数更新；L4 12→14。
- **DIFF-0（只改文档）**：`git diff HEAD --name-only` 输出仅含 `.md` 文件（**语义式**：本 Stage 禁止改任何代码/配置文件）。
- **REG-5（存量零回归）**：`npm test` 全量绿。

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `npm test`
