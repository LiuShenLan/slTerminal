# windows-terminal 事实核查报告

> 核查日期: 2026-08-15
> 核查方式: WebSearch + WebFetch 外部验证 + 内部交叉比对
> 核查范围: 全量声称逐一核实
> 方向: terminal

---

## 错误 1: issue #8405「建议改 disablePaneAnimations」在 issue 全部评论中不存在

- **文件+行号**: `docs/refer/windows-terminal.md` (3.2 节「局限」条目, 第 102 行)
- **原声称**: 「官方在 issue #8405 承认该命名名不副实，建议改 `disablePaneAnimations`」
- **错误类型**: 来源不支撑
- **正确信息**: issue #8405 中官方（zadjii-msft）承认 `disableAnimations` 未禁用命令面板等 WinUI 组件自带动画（"I forgot that there were animations in that component... Good catch!"），但全文无任何「改名建议」——"disablePaneAnimations"、"rename" 等字样在 issue 正文与全部评论中均不存在；issue 状态为 open（截至 2026-08-15），未关闭
- **反证来源**: https://github.com/microsoft/terminal/issues/8405 — 全部 4 条评论逐条核对：zadjii-msft「Just to confirm: you are putting `disableAnimations` into the _profile_?... We released 1.5.3282 with a fix...」/ 报告者确认顶层设置后 pane 动画已禁用、palette 动画仍在（附 GIF）/ zadjii-msft「Oh! Wow, I forgot that there were animations in that component... Good catch!」/ zadjii-msft「I'm not sure that an app is actually able to disable implicit animations on itself...」

## 错误 2: Pane 进入动画「缓动曲线未公开」——源码明确为 QuadraticEase

- **文件+行号**: `docs/refer/windows-terminal.md` (3.1 动画清单 Pane 行, 第 89 行; 附录「无法核实项 ③」, 第 239 行)
- **原声称**: 「缓动：代码内 easing（具体曲线未公开，「待核」）」及附录「pane 动画具体缓动曲线（源码确认有 easing 但曲线名未公开）」
- **错误类型**: 事实错误（来源不支撑的反面——声称无法核实，实际来源明确）
- **正确信息**: PR #7364 代码中缓动曲线明确公开：`animation.EasingFunction(Media::Animation::QuadraticEase{})`（进入与退出动画均使用，diff 第 161、243 行）；时长 200ms（`AnimationDurationInMilliseconds = 200`）与「166ms 只可见一帧、300ms 过长」折中说明均属实
- **反证来源**: https://github.com/microsoft/terminal/pull/7364.diff — `+        animation.EasingFunction(Media::Animation::QuadraticEase{});`（进入/退出两处）; https://github.com/microsoft/terminal/pull/7364 — 评论「300ms felt too long, and 166ms felt like it was only visible for a single frame.」

## 错误 3: issue #14858 未标注已关闭状态（closed/completed，计划已于 1.18 执行）

- **文件+行号**: `docs/refer/windows-terminal.md` (5.2 表 `theme` 行, 第 168 行)
- **原声称**: 「issue #14858 有计划改回 `"system"`」
- **错误类型**: 过时信息
- **正确信息**: issue #14858「Plan for new default theme」已于 2023-07-05 关闭（state: CLOSED, stateReason: COMPLETED，milestone Terminal v1.18），清单中「revert default.json to "system"」「Change userDefaults theme to "Dark Seamless"」等条目均已完成（关联 PR #15108 已合并）；「有计划改回 system」应标注为已执行完毕的历史计划，而非进行中的计划（且当前 main 分支 defaults.json 实测 `"theme": "dark"`，说明后续又有变动）
- **反证来源**: https://github.com/microsoft/terminal/issues/14858 — `"state":"CLOSED","stateReason":"COMPLETED"`、milestone「Terminal v1.18 (closed)」、`closedByPullRequestsReferences: PR #15108 state MERGED`

## 错误 4: 删除按钮「按下色 #B3FFFFFF」实为前景（文字）按下色，非背景按下色

- **文件+行号**: `docs/refer/windows-terminal.md` (3.1 动画清单「删除按钮背景过渡」行, 第 95 行)
- **原声称**: 「删除按钮背景过渡……按下色 `#B3FFFFFF`（70% 白）」
- **错误类型**: 事实错误（字段混淆）
- **正确信息**: `#B3FFFFFF` 是 **DeleteButtonForegroundPressed**（按下时前景/文字色）；背景按下色是 `DeleteButtonBackgroundPressed`（Opacity 0.8 的 Firebrick，非 #B3FFFFFF）。`DeleteButtonBackgroundPointerOver/Pressed` 分别用 Opacity 0.9/0.8 表达 hover/按下
- **反证来源**: https://github.com/microsoft/terminal/blob/main/src/cascadia/TerminalSettingsEditor/CommonResources.xaml — Dark 字典 15-25 行：`<SolidColorBrush x:Key="DeleteButtonBackgroundPressed" Opacity="0.8" Color="{ThemeResource DeleteButtonColor}" />` 与 `<SolidColorBrush x:Key="DeleteButtonForegroundPressed" Color="#B3FFFFFF" />`（#B3FFFFFF 绑定的是 Foreground 键）

## 核查通过项摘要

以下声称经外部验证全部正确（未单列错误条目）：
- **官方文档部分**：Campbell 配色全表 22 项逐字一致（defaults.json 与 learn.microsoft.com 同值）；内置 dark 主题 JSON 逐字一致（themes 页含 `tab.unfocusedBackground #00000000`、`tabRow.unfocusedBackground #333333FF`）；`window.applicationTheme` 默认 `"dark"`；`theme` 顶层默认值两处冲突属实（defaults.json main=`"dark"` vs appearance 文档默认 `"system"`）；`useMica` 默认 false、需 Win11 ≥ 22621、Mica 下 opacity pane 透出、「无法同时获得无模糊透明背景 + Mica 页签栏」逐字；`useAcrylicInTabRow` 默认 false、开启 50% opacity；`showCloseButton` 四态（always 默认）；主题色语法 `#rgb/#rrggbb/#rrggbbaa` + `accent`/`terminalBackground` 特殊值；Seamless 主题 JSON（tab/tabRow 均 terminalBackground、未聚焦 `#2C2C2CFF`）
- **源码部分**：App.xaml 的 `TabViewBackground #2e2e2e`（含 GH #12356 对比度注释）、`SettingsUiTabBrush #0c0c0c`、`BroadcastPaneBorderColor = SystemAccentColorDark2`、`TabViewHeaderPadding 0,0,0,0`、`TabViewItemBorderThickness 1,1,1,0`、NoAnimationsPlease 仅留 ContentThemeTransition、TabViewListView 移除 Entrance/AddDelete 过渡、颜色按钮 hover 0.9/pressed 0.8 + KeyFrame 0；CommandPalette.xaml 的 `FlyoutPresenterBackground`、`OverlayCornerRadius`、`SharedShadow` + `Translation 0,0,32`、主网格 2*/6*/2* × 8*/2*、搜索框 Padding 18,8,8,8、列表 Margin 8,0,8,8、图标槽 16px + 滚动条留白 16px、副标题 10pt（SubtitleTextStyle/SystemBaseMediumColor）、快捷键徽章 CornerRadius 2/边框 1/SystemControlForegroundBaseMediumBrush、「无匹配」高 36px、ColumnSpacing 8；TerminalWindow.cpp `tabRowHeight = 32`；defaults.json 全部默认值（Cascadia Mono 12、padding 8,8,8,8、disableAnimations false、alwaysShowTabs/showTabsInTitlebar/showTerminalTitleInTitlebar true、tabWidthMode equal、内置 scheme 名含 One Half Dark/Tango Dark/Vintage/Dimidium、主题名 light/dark/system + legacy 变体）
- **GitHub 部分**：PR #7364（200ms + 166/300 折中 + 退出动画 EnableDependentAnimation）；commit 204c9d2（#8237 进入动画补 app 级门控）；commit 4d4111b（#15204 移除 TabView 滑入/图标弹出动画，消息含「shaves about 10% off of the ~500ms startup delay」）；commit ad6473d（#19351 暗色 fallback `#282828`）；commit ed7c716（#10864 标题栏亚克力 `#FF333333`）；commit f892e52（CommandPaletteBackground `#333333`）；commit 024b9fc（unfocusedBackground 设置，commit 消息逐字「fall back to the default tab row color, TabViewBackground from our App.xaml」——页签栏背景 fallback 链属实）；commit 0d2e8ce（#19604 tabRow 色作 acrylic tint）；commit 5c5b671（theming 注记 tabRow.acrylicOpacity + TintOpacity 语义）；commit 48b796f（#19001 WinUI brushes 改造）；commit c1df08e（1.16 default themes 系列，含 `unfocusedBackground: #333333FF`）；commit 6d63605；Theme.cpp（Theme/ThemeColor/TabTheme/TabRowTheme/WindowTheme 齐全）；spec #2046（覆盖全部 pane、水平居中、从 tab row 下拉、「find the overlay problem」）；issue #9539（正文含「remove the base layer page」计划）

## 备注（未列为错误，供参考）

- 来源 [29] issue #9539 实际标题为「Terminal v1.8」（release issue），正文内容含 base layer 移除计划——文档用内容概括代替了标题，事实内容成立但标题并非如此
- 「设置 UI 主题选择器仅两个实体项 Dark/Light（含 legacy 变体）」：内置主题名 light/dark/system + legacyDark/legacyLight/legacySystem 已在 defaults.json 确认存在，但设置 UI 选择器实际渲染的选项列表未直接核实（UI 层验证缺失）
- 附录「无法核实项 ② 命令面板显隐动画时长与缓动」：属实，spec 与源码均未提取到明确参数（WinUI 系统过渡）
- 附录「无法核实项 ① FlyoutPresenterBackground / OverlayCornerRadius 精确值」：WinUI 系统资源，无法从 Windows Terminal 仓库获取，标注合理
- 「主题选择器」「pane 动画曲线」已在错误 2 修正——曲线名 QuadraticEase 已公开，其余「待核」标注均合理
