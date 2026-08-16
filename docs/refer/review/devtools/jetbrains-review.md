# jetbrains 事实核查报告

> 核查日期: 2026-08-15
> 核查方式: WebSearch + WebFetch 外部验证 + 内部交叉比对
> 核查范围: 全量声称逐一核实
> 方向: devtools

---

## 错误 1: 表格行将 Table 的 compact 行高一并写成 22（实际 Table 无 compact 覆盖）

- **文件+行号**: `docs/refer/jetbrains.md` (第 66 行，2.3 间距)
- **原声称**: "基础行高: List/Tree/Table `rowHeight = 20`，New UI compact 模式 `22` [1]"
- **错误类型**: 事实错误
- **正确信息**: `List.rowHeight.compact` 与 `Tree.rowHeight.compact` 均为 22，但 `Table` 节点**未定义** `rowHeight.compact`（compact 模式下仍为 20，无覆盖）。
- **反证来源**: https://raw.githubusercontent.com/JetBrains/intellij-community/master/platform/platform-resources/src/themes/darcula.theme.json（`Table` 节点仅 `rowHeight: 20`，无 `rowHeight.compact` 键；`List`/`Tree` 均有 `rowHeight.compact: 22`）

## 核查通过项摘要

以下声称经外部验证全部正确（未单列错误条目）：
- 官方主题文件 [1] darcula.theme.json 部分：`*: background #3C3F41`、`*: foreground #BBBBBB`、`caretForeground #BBBBBB`、`selectionBackground` os.default `#2F65CA` / os.windows `#4B6EAF`、`selectionForeground #DEDEDE`、`selectionBackgroundInactive #0D293E`、`Content.background #2B2B2B`、TextField/TextArea/ComboBox 背景 `#45494A`、`Hyperlink.linkColor`/`Link.activeForeground #589DF6`、`Link.pressedForeground #BA6F25`、`TabbedPane.underlineColor #4A88C7`/`tabSelectionHeight 3`/`tabHeight 32（compact 36）`/`tabFillStyle "underline"`/`contentAreaColor #323232`/`hoverColor #2E3133`、`EditorTabs.underlinedTabBackground #4E5254`、`ToolWindow.Header.background #3B4754`/失焦 `#3C3F41`/`HeaderTab.selectedBackground #313B45`/`HeaderCloseButton.background #3F4146`、`Component.focusColor #3D6185`/`arc 5`、`Focus.color #FF0000`、进度条 `passedColor #008F50`/`failedColor #E74848`/`trackColor #555555`、书签 `#D9A343`（XML Darcula 节 `Bookmark.iconBackground`）、按钮 `#4C5052`/`#5E6060`/`disabledText #777777`/默认按钮 `#365880`→`#4c708c`/`shadowColor #36363680`/`shadowWidth 2`、`PopupMenu.borderInsets "6,1,6,1"`/`borderCornerRadius 8`、`Code.Inline/Block.borderRadius 10`、`Shortcut.borderRadius 7`、Tree `leftChildIndent 7`/`rightChildIndent 11`/`paintLines false`、SearchEverywhere 全部（`#45494A`/`#555A5E`/`#3C3F41`/`#646464`/`#505050`/`Advertiser.borderInsets.compact "4,20,5,20"`）、`MainWindow.Tab`（`#3C3F41`/`#CED0D6`/`#131314`/`#1A1A1B`/`#2B2D30`）、StatusBar（`"panel"`/`#464646`/`#4C5052`/`Widget.widgetInsets.compact "4,8,3,8"`）、`Ide.Shadow`（16px 内缩 + `#00000012`）、`Notification.Shadow`（5px + `#00000010`）、`Popup.borderColor #616161`、`grayFilter "-70,-70,100"`——逐字一致。
- High Contrast 主题 [2]：`parentTheme "Darcula"`、`editorScheme "High contrast"`、`* background #000000`/`foreground #FFFFFF`、`selectionBackground #3333FF`、`focusColor #1AEBFF`、`Link.activeForeground #D2F53C`——一致。
- 编辑器色板 [3] DefaultColorSchemesManager.xml Darcula 节：gutter `#313335`、行号 `#606366`、当前行行号 `#A4A3A3`、选中 `#214283`（失焦 `#4C4F56`）、缩进参考线 `#373737`/选中参考线 `#505050`、右边界线/方法分隔线 `#4D4D4D`、空白字符 `#606060`、关键字 `#CC7832`、字符串 `#6A8759`、数字 `#6897BB`、注释 `#808080`、文档注释 `#629755`、函数声明 `#FFC66D`、常量/实例字段 `#9876AA`、类引用 `#769AA5`、注解名 `#BBB529`、错误 `#BC3F3C`、警告底 `#52503A`、文件状态色（新增 `#629755`/修改 `#6897BB`/未知 `#D1675A`/删除 `#6C6C6C`/合并 `#9876AA`）——全部逐字一致；"默认文字色以动态引用定义、历史稳定值 #A8A8A8" 系文档自标「待核」，未列为错误。
- 动效源码部分：`UISettings.kt` `ANIMATION_DURATION = 300`（第 609 行）与 `animateWindows` ← `Registry.is("ide.animate.toolwindows", false)`（第 68-69 行，默认 false）——一致；`ToolWindowPane.kt` `addSlidingComponent`/`removeSlidingComponent`（744/800 行）、`Surface` 内部类、`offset += (distance - offset) / iterations` 公式、`RemoteDesktopService.isRemoteSession()` 强制关闭（745 行）——一致；`AnimatedIcon.java` 帧公式 `(cycleTime * totalFrames / cycleDuration) % totalFrames`（251 行）——一致。
- SDK 文档部分：typography.md（Inter 默认 13；H1 默认+5/H2 默认+3/Paragraph 行高默认+3/Medium 默认−1/Default semibold 用于弹窗与工具窗口标题；暗色语义色表 #8C8C8C/#787878/#777777/#FF5261）；layout.md "Always left-align labels... inconsistent with macOS guidelines"；tabs.md "place tabs on top of the content"、"no more than 8 tabs"、"hide under the dropdown component"；themes_getting_started.md 五项自定义能力；platform_theme_colors.md（color key 命名、LaF Defaults 对话框 internal 模式、themeMetadata.json 路径）；icons_style.md（16×16/1px 边/14×14 内容区、工具窗口 13×13、gutter/状态栏 12×12、弹窗 32×32/2px、2px 描边、0.5px 像素网格、iconName_dark.svg）——全部逐字一致。
- 官方博客部分：newui-blog（2022-05）"reduce visual complexity, provide easy access to essential features, and progressively disclose complex functionality" 与 "Simplified main toolbar with new VCS, Project, and Run widgets"；bridging-blog（2024-10）"2022.3 beta → 2023.2 新用户默认 → 2024.2 全员默认 → 经典 UI 作为插件提供"，Compact-mode/More-tool-windows/Disappearing-icons-on-hover 三个 GIF 均在文中存在——一致。
- 第三方 [21] colorpickercode："Created In 2012"、"primary background of #2b2b2b... eliminates the harsh contrast of pure black"——一致。

## 备注（未列为错误，供参考）

- "工具窗口条图标 hover 时显示文字标签"表述不精确：官方博客原文为"tool window header icons and code folding arrows ... appear on hover and disappear when you move the mouse away"（图标显隐，非文字标签）；Disappearing-icons-on-hover.gif 确实存在于文中（2024-10 上传）。
- "Table compact 行高 22"之外，其余自标「待核」项（工具窗口动画历史默认值、弹窗淡入参数、New UI 图标 20×20、编辑器默认文字历史值 #A8A8A8）均无法从引用来源核实，文档已自行标注，未列为错误。
