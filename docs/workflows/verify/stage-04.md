# Stage 4 逐项验证断言（唯一真值源）

> stage-04 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。
> 覆盖清单 ID：SB-25（docs/sidebar-checklist.md「Stage 4 — E2E」）。

## 断言清单

- **SB-25-A（helpers 三函数）**：Read `e2e-tests/helpers.ts` 确认 `installAllE2eHelpers()` 内新增三个 window 全局（grep 命中）：`__slterm_e2e_getSideBarState`（返回 useSideBar store 的 plain 快照，去函数——Read 确认仅含 zones/open/width/splitRatio/loaded 数据键）、`__slterm_e2e_toggleSideView(id)`（走 `useSideBar.getState().toggleView(id)`）、`__slterm_e2e_moveSideViewButton(id, zone, index)`（走 `useSideBar.getState().moveButton(id, zone, index)`）；注入点与现有 helper 同处 `installAllE2eHelpers()` 函数体内（E2E_ENABLED 门控链路不变）。
- **SB-25-B（E2E-1 点击开关）**：Read `e2e-tests/test.e2e.ts` 确认 describe「侧栏视图」存在；用例 1 序列：createProject 后断言默认项目列表打开且侧栏区可见 → 真实 click `[data-e2e="activity-btn-projects"]` → 断言 open 双空（侧栏区隐藏）→ 再 click 恢复 → click `[data-e2e="activity-btn-explorer"]` → 断言 `open.top === "explorer"`（R1 替换）。
- **SB-25-C（E2E-2 拖拽跨区）**：Read 确认用例 2：JS 合成 DragEvent（`new DragEvent("dragstart"/"dragover"/"drop", { dataTransfer: new DataTransfer(), bubbles: true })`）驱动按钮跨区 → 断言 state zones 变化 + open 跟随（R6/R7）；**若合成事件在 WebView2 不可行**，允许降级为 `__slterm_e2e_moveSideViewButton` 驱动断言状态机，但测试文件内必须有注释注明降级原因（Read 确认注释存在），且收尾报告注明拖拽手感人工验收。
- **SRC-0（只改 e2e-tests/）**：`git diff HEAD -- src/` 输出为空（**语义式**：本 Stage 禁止改 src/ 任何文件）；`git diff HEAD -- e2e-tests/wdio.conf.ts e2e-tests/run-wdio.cjs` 输出为空。
- **SHAKE-0（生产 tree-shake）**：`npx vite build` 成功后，grep `__slterm_e2e_getSideBarState|__slterm_e2e_toggleSideView|__slterm_e2e_moveSideViewButton` 在 `dist/` 产物中为零（生产构建 DEV=false 且未设 VITE_E2E，helper 必须被 tree-shake）。
- **E2E-ALL（L4 全绿）**：`npm run build:e2e` 成功 + `npm run wdio` 全绿，报告共 14 用例（12 存量 + 2 新增），无 skip。

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `npm test`
4. `npx vite build`（生产构建，供 SHAKE-0 grep）
5. `npm run build:e2e`（= cross-env VITE_E2E=1 tauri build --debug --no-bundle；helpers.ts 不在根 tsconfig include，必须构建级验证）
6. `npm run wdio`（L4 全量，含 12 存量回归）

## 人工验证点（不属于本断言文件判定范围，主 agent 在收尾报告中提示用户实测）

真实 app 拖拽手感（合成事件验证状态机 ≠ 真实鼠标拖拽）：按钮拖拽视觉反馈、插入指示线位置、跨区落点正确。
