# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 模块状态：已退役（NAV-06，2026-08）

`src/features/sidebar/` 目录仅剩本文件——`SidebarTree.tsx` 及其配套已于 NAV-06 整体删除。本目录待清理：目录删除时本文件一并删除（登记于 `.claude/CLAUDE.md` 模块索引）。

## 历史职责与迁移去向

SidebarTree 曾是左侧项目/操作页面二级树组件（项目 CRUD、操作页面 CRUD、页面切换导航）。NAV-01~09 以 `src/features/navTree/` 统一导航树接管其全部职责：

| 原职责 | 迁移去向 |
|--------|---------|
| 二级树渲染（项目/页面） | `navTree/NavTree.tsx`（+ 活跃会话行 + 历史折叠节点，决策 5 层级扩展） |
| 项目 CRUD / 页面 CRUD / 内联重命名 | `NavTree.tsx`（行为不变迁入） |
| `makeEmptyLayout()` | `navTree/NavTree.tsx` 导出（restoreSession 等消费点改引用，NAV-06 承接约定） |
| 宿主（sideViewDefs `projects` 视图） | sideViews 三槽重组——`projects` 视图注销，`nav` 视图注册（NAV-05） |
| 右键菜单「打开 Hooks 配置」 | 决策 4 入口唯一化——菜单项删除，配置钮移至活动栏底部 |
| 相关测试 | `sidebar-actions.test.ts` 语义迁入 `nav-tree.test.tsx` / `nav-tree-history.test.tsx`（NAV-08 测试迁移） |

## 防误用提示

- 消费方不得再 `import` 本目录（无导出文件）；历史引用一律改指 `../navTree`。
- 侧栏相关文档请读 @../sideViews/CLAUDE.md（视图系统）与 @../navTree/CLAUDE.md（导航树）。
