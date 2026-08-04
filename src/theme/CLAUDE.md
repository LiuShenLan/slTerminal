# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 模块职责

配色 token 单点（硬约束 #6）——所有颜色只在 `theme/colors.ts` 定义为 token，组件引用 token，禁止硬编码颜色。配色源自 JetBrains IDEA 暗色主题（Darcula）。**既定例外**：xterm.js 终端主题在 `src/panels/terminal/theme.ts` 独立定义（历史遗留，见 @../panels/CLAUDE.md）。

## 架构决策

### token 按语义域分组

| 分组 | token 示例 | 消费方 |
|------|-----------|--------|
| 文件名 git 状态色 | `GIT_FILE_COLORS`（modified/added/untracked/deleted/renamed/conflict/ignored） | 文件浏览器、Commit 视图文件名着色 |
| 行内 diff 边栏色 | `GIT_GUTTER_COLORS`（modified/added/deleted/whitespaceOnly） | 编辑器 git diff gutter |
| 文件浏览器通用色 | `EXPLORER_COLORS`（bg/fg/hover/selected/arrowClosed/arrowOpen） | 文件树 |
| 通用 UI 色（背景/前景/边框/交互） | `PANEL_BG`/`SIDEBAR_BG`/`SIDEBAR_FG`/`ERROR_FG`/`FOCUS_BORDER`/`INPUT_BORDER`/`DROPDOWN_BG`/`BUTTON_FG`/`DIM_FG`/`PLACEHOLDER_FG` 等 | 全应用组件 |

### 新增 token 规则

- 新颜色按语义域追加到 `colors.ts` 对应分组，不新开散落常量
- 组件引用 token 名，禁止直接写色值（CSS 内联样式、style 对象、字符串常量一律禁止）
- 终端主题例外仅限 `panels/terminal/theme.ts`——新面板的终端类渲染器配色照此登记例外，不扩默认

## 文件

| 文件 | 职责 |
|------|------|
| `colors.ts` | 全部配色 token 定义（git 状态色 / diff gutter 色 / explorer 色 / 通用 UI 色分组） |
| `index.ts` | barrel export |

## 测试模式

无独立测试文件——token 正确性由消费方测试断言（如 `git-gutter.test.ts` 的 GutterMarker DOM 颜色断言、`explorer-git-status.test.tsx` 着色断言）。新增 token 时同步更新消费方断言。
