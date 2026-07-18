# Stage 3 逐项验证断言（唯一真值源）

> stage-03 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。
> 覆盖清单 ID：SB-21~SB-24（docs/sidebar-checklist.md「Stage 3 — Workspace 集成 + App 接线」）。

## 断言清单

- **SB-21-A（旧常量清除）**：grep `SIDEBAR_PREFERRED_SIZE|SIDEBAR_MIN_SIZE|SIDEBAR_MAX_SIZE|EXPLORER_PREFERRED_SIZE|EXPLORER_MIN_SIZE|EXPLORER_MAX_SIZE` 在 `src/workspace/Workspace.tsx` 为零。
- **SB-21-B（三 pane 结构）**：Read `Workspace.tsx` 确认 Allotment 三 pane 顺序为：活动栏 → 侧栏区 → 主区。pane1 渲染 `<ActivityBar />`，`preferredSize`/`minSize`/`maxSize` 均为 40（允许引用 `ACTIVITY_BAR_SIZE` 常量）；pane2 渲染 `<SideBarArea switchToPage={switchToPage} onDeletePage={onDeletePage} />`，`preferredSize={width}`（width 来自 useSideBar）、`minSize={160}`/`maxSize={500}`（允许引用 `WIDTH_MIN`/`WIDTH_MAX`）、`visible={anyOpen}`；pane3 主区逻辑不变。
- **SB-21-C（anyOpen 推导与宽度写回）**：Read 确认 `anyOpen` 由 `deriveLayout(open) !== "hidden"` 推导（**语义式**：不接受 `!!open.top || !!open.bottom` 之外引入第三份状态，也不接受本地 useState 镜像）；外层 Allotment `onChange={(sizes) => anyOpen && setWidth(sizes[1])}`（等价写法可接受，但必须含 anyOpen 守卫且写回 `sizes[1]`）。
- **SB-21-D（side-effect 注册）**：grep `import "../features/sideViews/sideViewDefs"` 命中 `Workspace.tsx` 顶部 import 区。
- **SB-21-E（逻辑不动）**：`git diff HEAD -- src/workspace/Workspace.tsx` 中 `switchToPage`/`onDeletePage`/SEC-01 effect/页面回调 map 的函数体无实质改动（**语义式**：Read diff 逐块确认，仅允许与三栏改造直接相关的行变更）。
- **SB-22-A（启动序列）**：Read `src/App.tsx` init() 确认 `await useSideBar.getState().loadFromDisk()` 位于 `useKeybindings.getState().loadFromDisk()` 之后、`loadAllProjects()` 之前，且在独立 try/catch 块中。
- **SB-22-B（关窗冲刷）**：Read `src/App.tsx` 关闭 handler 确认 `cancelKeybindingsSave()` 之后调用 sideBar 的取消保存（`import { cancelPendingSave as cancelSideBarSave } from "./stores/sideBar"` 或等价命名）。
- **SB-23（存量套件适配）**：`workspace-multi-instance.test.tsx` / `workspace-switch-order.test.tsx` / `workspace-e2e-ready.test.tsx` / `e2e-gating-workspace.test.tsx` 全绿（据测试结果）；适配方式为种子 sideBar store 默认态或 mock `../features/sideViews` 模块（Read diff 确认未删除存量用例——**语义式**：只允许新增种子/mock 与修正断言，不允许删除/跳过（`it.skip`/`xit`）任何存量用例）。
- **SB-24**：`src/__tests__/workspace-sideviews.test.tsx` 存在且全绿；覆盖：活动栏 pane 40 固定（preferredSize/min/max）、侧栏区 pane `visible=anyOpen` 四态（hidden→false、单开/双开→true）、onChange→setWidth、主区 pane 保持、`switchToPage` 透传 SideBarArea。
- **BODY-0（组件本体零改动）**：`git diff HEAD -- src/features/sidebar/SidebarTree.tsx src/features/explorer/ExplorerPanel.tsx` 输出为空。
- **HELPERS-0（本 Stage 不动 E2E 辅助）**：`git diff HEAD -- e2e-tests/` 输出为空（SB-25 属 Stage 4）。
- **REG-3（存量零回归）**：`npm test` 全量绿。

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `npm test`
4. `npx vite build`

## 人工验证点（不属于本断言文件判定范围，主 agent 在 commit 后、进 Stage 4 前提示用户实测）

`npm run tauri dev` 实测：① 默认项目列表打开；点击 📋 关闭 → 侧栏区消失主区占满；再点恢复且项目树展开状态保留（display:none 保挂载）；② 点击 📁 → 替换为文件浏览器；拖拽 📁 到下区 → 上下分区；拖分隔条调比例；③ 拖拽 📋 到下区（两视图打开中）→ 项目列表跟随到下区，文件浏览器被关闭；④ 重启 app → 打开状态/宽度/比例/按钮归属全部恢复；⑤ 终端 claude 启动正常（主区零改动冒烟）。
