# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 职责

侧栏（SidebarTree）——左侧项目/操作页面二级树组件。管理项目 CRUD、操作页面 CRUD、页面切换导航。

## 架构决策

### 新建页面不自动创建终端

`makeEmptyLayout()` 返回空布局 `{}`，新建项目/操作页面时不包含任何默认面板。`PageDockview.handleReady` 在空布局时不做兜底终端创建。空白页面由 Dockview watermarkComponent 接管显示（"打开终端或编辑器开始工作"）。

**涉及路径**：
- `handleAddProject` — 新建项目时 page.layout 为空
- `handleNewPage` — 新建操作页面时 page.layout 为空
- `__slterm_e2e_createProject` — E2E 辅助，同上

**旧 `makeDefaultLayout` 已删除**——早期版本硬编码 `contentComponent: "terminal"` 到新建页面布局中。`layoutSerde.patchLegacyLayout` 仍保留对旧格式的兼容修补（不影响新页面创建）。

### 项目创建不自动切换页面

`handleAddProject` 创建项目后调用 `addProject(project)`（store 内展开项目节点），但**不调 `useLayout.setActivePage()`**。用户需手动点击页面行切换。这与 E2E 辅助（`__slterm_e2e_createProject`）不同——后者显式设 `activePageId` 自动切换。

## 文件

| 文件 | 职责 |
|------|------|
| `index.ts` | barrel export：SidebarTree 组件 + makeEmptyLayout |
| `SidebarTree.tsx` | 侧栏主组件：二级树渲染、工具栏、右键菜单、handleAddProject/handleNewPage/handleDeletePage |

## 关键集成点

- **`src/stores/projects.ts`** — `addProject` / `addPage` / `removeProject` / `removePage`（CRUD）
- **`src/ipc/dialog.ts`** — `open()` 原生文件夹选择对话框
- **`src/workspace/Workspace.tsx`** — `switchToPage` / `onDeletePage` 回调（通过 props 传入）
- **`src/workspace/panelRegistry.ts`** — PANEL_TERMINAL 常量（**仅 Watermark/RightHeader 使用，SidebarTree 不再引用**）

## 测试模式

测试文件：`src/__tests__/sidebar-actions.test.ts`（25 用例 → 33 用例）。

- 使用真实 Zustand stores（`setState` 种子数据），mock IPC dialog
- `handleAddProject` 测试：mock dialog.open → fireEvent.click → waitFor store 更新 → 验证 project/pages/layout
- `handleNewPage` 测试：populateStore → 右键菜单 → 验证 page 数量 +1 + layout 为空
- 25 条测试覆盖：树结构渲染、展开折叠、右键菜单交互、内联重命名、Escape/空白/相同名称边界分支、项目删除 confirm、布局 CSS 自适应
