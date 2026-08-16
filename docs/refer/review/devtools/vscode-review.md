# vscode 事实核查报告

> 核查日期: 2026-08-15
> 核查方式: WebSearch + WebFetch 外部验证 + 内部交叉比对
> 核查范围: 全量声称逐一核实
> 方向: devtools

---

## 错误 1: 右键菜单色值与 Dark+ 实际值不符，且与本文档 2.1.3 表自相矛盾

- **文件+行号**: `docs/refer/vscode.md` (第 211 行，4.2 输入框/下拉/按钮段)
- **原声称**: "下拉 `#3C3C3C` / `#F0F0F0`；右键菜单 `#3C3C3C`，选中项 `#04395E`，分隔线 20% 透明。"
- **错误类型**: 事实错误 + 内部矛盾
- **正确信息**: Dark+（dark_vs.json 显式覆盖）下菜单实际为：背景 `menu.background #252526`、选中项 `menu.selectionBackground #0078d4`、分隔线 `menu.separatorBackground #454545`（不透明）。`#04395E` 仅是 `menuColors.ts` 的注册默认（`listActiveSelectionBackground`），被 Dark+ 显式覆盖；`#3C3C3C` 是 `input.background` 的值，与本句"输入框 #3C3C3C"同值，疑为字段混淆。该句与本文档 2.1.3 表"`menu.selectionBackground | #0078d4`（Dark+ 显式值）[3]"直接矛盾。
- **反证来源**:
  - https://raw.githubusercontent.com/microsoft/vscode/main/extensions/theme-defaults/themes/dark_vs.json → `"menu.background": "#252526"`、`"menu.selectionBackground": "#0078d4"`、`"menu.separatorBackground": "#454545"`
  - https://raw.githubusercontent.com/microsoft/vscode/main/src/vs/platform/theme/common/colors/menuColors.ts → `registerColor('menu.selectionBackground', listActiveSelectionBackground, ...)`（即 #04395E 仅为无覆盖时的默认）

## 无法验证项

- **"1.91 起默认暗色主题改为 Dark Modern"**（第 9 行，概览）：引用来源 [14]（themes 官方文档）与 [16]（dark_modern.json）均不含版本号——现行 themes 文档只写 "Workbench: Preferred Dark Color Theme - defaults to Dark Modern"；v1.91–v1.93 官方 release notes（vscode-docs 仓库冻结版）均无该变更记载；且 dark_modern.json 最早提交为 2023-05-22（"Rename new default theme files (#183001)"），主题文件本身早于 1.91 约一年。版本归属无法确认，标注待阶段 3 裁决。

## 核查通过项摘要

以下声称经外部验证全部正确（未单列错误条目）：
- 主题 JSON 部分（dark_vs.json/dark_plus.json/dark_modern.json/hc_black.json，main 分支）：`editor.background #1E1E1E`、`editor.foreground #D4D4D4`、`editor.inactiveSelectionBackground #3A3D41`、`editorIndentGuide.background1 #404040`、`editorIndentGuide.activeBackground1 #707070`、`editor.selectionHighlightBackground #ADD6FF26`、`activityBarBadge.background #007ACC`、`sideBarTitle.foreground #BBBBBB`、`menu.background #252526`、`menu.foreground #CCCCCC`、`statusBarItem.remoteBackground #16825D`、`tab.selectedBackground #37373D`、`tab.selectedForeground #FFFFFF`、`widget.border #303031`、`input.placeholderForeground #A6A6A6`；dark_plus include dark_vs、语法 tokenColors 全表（keyword `#569cd6`、string `#ce9178`、comment `#6A9955`、number `#b5cea8`、type `#4EC9B0`、function `#DCDCAA`、variable `#9CDCFE`、control `#C586C0`、constant `#4FC1FF`、invalid `#f44747` 等）；hc_black `editor #000000`/`#FFFFFF`、`selection.background #008000`、`contrastBorder #6FC3DF`（dark 为 null）；dark_modern 全表（editor `#1F1F1F`、工作台统一 `#181818`、无文件夹状态栏 `#1F1F1F`、页签条 `#181818`/边框 `#2B2B2B`、强调统一 `#0078D4`、`tab.activeBorderTop #0078D4`、quickInput `#222222`、输入框/下拉/复选框 `#313131`（边框 `#3C3C3C`）、通知 `#1F1F1F`（header 同色、边框 `#2B2B2B`）、`statusBar.debuggingBackground #0078D4`、badge `#616161`）——全部逐字一致。
- 颜色注册源码部分：`focusBorder #007FD4`、`textLink.foreground #3794FF`、`foreground #CCCCCC`、`descriptionForeground` = foreground 70% 透明、`icon.foreground #C5C5C5`、`errorForeground #F48771`、`editor.selectionBackground #264F78`、`editor.findMatchBackground #515C6A`、`editor.findMatchHighlightBackground #EA5C0055`、`editor.hoverHighlightBackground #264f7840`、`editorError #F14C4C`、`editorWarning #CCA700`、`editorInfo #59a4f9`、`editorHint #eeeeee 70% 透明`、inputValidation error/warning/info（`#5A1D1D`/`#BE1100`、`#352A05`/`#B89500`、`#063B49`/`#007acc`）、`button.background #0E639C`（hover = lighten 0.2）、`progressBar.background #0E70C0`、`scrollbarSlider #797979` 40% 透明（hover 70%、active `#BFBFBF` 40%）、`scrollbar.shadow #000000`、`list.activeSelectionBackground #04395E`、`list.inactiveSelectionBackground #37373D`、`list.hoverBackground #2A2D2E`、`list.dropBackground #062F4A`、`list.highlightForeground #2AAAFF`、`tree.indentGuidesStroke #585858`（inactive 40%）、`input.background #3C3C3C`、`inputOption.activeBorder #007ACC`、`pickerGroup.foreground #3794FF`/`border #3F3F46`、`dropdown #3C3C3C`/`#F0F0F0`、`button.secondaryBackground = listHoverBackground #2A2D2E`、`activityWarningBadge #B27C00`、`activityErrorBadge #F14C4C`、`badge.background #4D4D4D`——逐字一致。
- workbench theme.ts 部分：`tab.activeBackground = editorBackground`、`tab.inactiveBackground #2D2D2D`、`tab.hoverBackground null`、`tab.activeModifiedBorder #3399CC`（inactive 50% 透明）、`tab.border #252526`、`tab.selectedBorderTop = focusBorder #007FD4`、`tab.activeForeground 白`/`inactiveForeground 白 50%`、`editorGroupHeader.tabsBackground #252526`、`statusBar #007ACC`/`noFolder #68217A`、`statusBarItem.hoverBackground 白 12%`/`activeBackground 白 18%`/`prominentBackground 黑 50%`、`activityBar #333333`、`activityBarBadge #007ACC`、`activityBar.activeBorder = 白`、`sideBar #252526`、`sideBarSectionHeader #808080 20% 透明`、`titleBar #3C3C3C`/`#CCCCCC`（失焦 60%）、`notifications.background = editorWidgetBackground #252526`（注释原文 "Notifications slide in from the bottom right of the window" 一致）、`notificationCenterHeader = lighten(背景, 0.3)`、`panel.background = editorBackground`、`WORKBENCH_BACKGROUND()` 存在——逐字一致。
- CSS 部分：statusbarpart.css（`height: 22px`、`font-size: 12px`、`.monaco-workbench.monaco-enable-motion .part.statusbar { transition: background-color 0.15s ease-out }`、`font-variant-numeric: tabular-nums`、`flex-grow: 1 /* left items push right items to the far right end */`、row-reverse 换行）；part.css（`.part > .title height 35px`、`padding 0 8px`、title-label `padding-left 12px`、h2 `font-size 11px`、链接 13px、progress `top: 33px /* at the bottom of the 35px height title container */`）；sidebarpart.css（h2 `text-transform: uppercase`、动作按钮 `width: 0 → initial` 悬停展开、`width 28px`/`16px` 图标）；activitybarpart.css（`width: var(--activity-bar-width, 48px)`、nosidebar/activitybar-right 时 `box-shadow: var(--vscode-shadow-md)`、`flex-direction: column; justify-content: space-between`）；list.css 无 transition/animation 声明——全部一致。
- 编辑器字体 [12] fontInfo.ts：`DEFAULT_WINDOWS_FONT_FAMILY = 'Consolas, \'Courier New\', monospace'`、macOS `Menlo, Monaco...`、Linux `'Droid Sans Mono'`、`fontSize: isMacintosh ? 12 : 14`、`lineHeight: 0`——逐字一致。
- 官方文档 [14]：themes 页含 `themes_hero.gif` 与 `theme-activitybar.gif`（主题预览/活动栏主题化演示截图源）——一致。

## 备注（未列为错误，供参考）

- "页签拖放插入指示线为白色"未能在颜色注册中找到对应 token（tab 拖放相关为 `editorGroup.dropBackground` 等），系 CSS 层细节，未列为错误。
- 徽标句"远程/警告/错误徽标另有 #B27C00 / #F14C4C 变体"：警告/错误徽标值已核实，远程徽标在 Dark+ 下为 `statusBarItem.remoteBackground #16825D`（另一语境），表述松散但未到错误程度。
