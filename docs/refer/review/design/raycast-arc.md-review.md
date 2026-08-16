# raycast-arc 事实核查报告

> 核查日期: 2026-08-15
> 核查方式: WebSearch + WebFetch 外部验证 + 内部交叉比对
> 核查范围: 全量声称逐一核实
> 方向: design

---

## 错误 1: Arc for Windows 的 Mica 材质引入日期「2024-03-14」有误（实际 2024-03-21）

- **文件+行号**: `docs/refer/raycast-arc.md` (行 177、来源表 [48])
- **原声称**: "**Windows 窗口材质**：Windows 11 上 Mica（2024-03-14 起）与 Acrylic（2024-03-21 起，Settings → Appearance 二选一）两种系统材质 [48]"
- **错误类型**: 事实错误（日期错误）
- **正确信息**: 官方 release notes（2026-08-15 经 Zendesk API 直抓全文）中 Mica 与 Acrylic 是**同一版本（2024-03-21 v.0.14.2）同时引入**：「You can select either Mica or Acrylic material background from Arc menu > Settings > Appearance」（该条目位于 March 21, 2024 v.0.14.2 版本段）。March 14, 2024 v.0.13.1 段全文无 Mica。「Settings → Appearance 二选一」的表述正确，Mica 的日期应改为 2024-03-21。
- **反证来源**: https://resources.arc.net/api/v2/help_center/en-us/articles/22513842649623.json（Arc for Windows - 2023-2026 Release Notes，Zendesk API）——「March 21, 2024 v.0.14.2」段含「You can select either Mica or Acrylic material background from Arc menu > Settings > Appearance」；「March 14, 2024 v.0.13.1」段（Videos Mini Player/键盘快捷键渲染/toasts 视觉打磨/Pinned Folder expand/collapse 动画改进等）无 Mica。

## 错误 2: 「搜索结果列表图标 16→20px（+25%）」归因 [17]，实际出处为 [2]

- **文件+行号**: `docs/refer/raycast-arc.md` (行 47)
- **原声称**: "官方 2022 焕新确认：搜索结果列表图标 16→20px（+25%）[17]"
- **错误类型**: 来源不支撑（归因错误）
- **正确信息**: [17]（raycast.com/changelog/macos/1-38-0，2022-07-19）为 v1.38 焕新 changelog（更大搜索栏/Action Bar/新 icon set/Compact Mode），**全文无 16px、20px、25%**；「increasing the size by 25%, from 16 to 20 pixels」出自 [2]（raycast.com/blog/launch-week-summary，2022-08-09）：「we wanted to bring more prominence to the command and application icons in a list view, increasing the size by 25%, from 16 to 20 pixels」。数值本身正确，引证归属错误。
- **反证来源**: https://www.raycast.com/changelog/macos/1-38-0（2026-08-15 抓取，无 16/20/25%）；https://www.raycast.com/blog/launch-week-summary（2026-08-15 抓取：「increasing the size by 25%, from 16 to 20 pixels」）。

## 错误 3: 来源表 [49] 描述「窗口点暗色默认」在该文中不存在

- **文件+行号**: `docs/refer/raycast-arc.md` (来源表 [49]，行 256)
- **原声称**: "[49] …窗口点暗色默认"
- **错误类型**: 来源不支撑
- **正确信息**: typefully.com/ridd_design/9-design-takeaways-from-using-arc（2026-08-15 抓取全文）包含「Library 卡片 hover 平滑带光照效果」（"These library cards FEEL excellent. Notice how smooth the hover is and how my mouse shines a light on the card itself"）与「URL 栏刷新细微动画」（"look at how the URL bar animates on refresh"）以及 12 小时自动归档，但**无任何「窗口默认暗色」相关表述**（全文 dark 仅出现在红绿灯「simple dark opacity」语境）。该描述项应删除或改述。
- **反证来源**: https://typefully.com/ridd_design/9-design-takeaways-from-using-arc-kTZD6kj（2026-08-15 抓取全文，检索 "dark" 仅 1 处且为按钮暗色不透明度语境）。

## 核查通过项摘要

以下声称经外部验证全部正确（未单列错误条目）：

- **Raycast 官方渠道**：ANSI 表 8 色（Red #FF6363 / Green #59D499 / Yellow #FFC531 / Blue #56C2FF / Magenta #CF2F98 / Cyan #52EEE5，OUTPUTMODES.md 逐字）；v1.38 博客（2022-07-19）三原则 fast/simple/delightful、更大搜索栏、Action Bar（左导航标题/toast、右上下文动作+快捷键）、icon set「The icons follow the same rules for stroke width and corner radii」、Compact Mode、新 app icon；launch-week-summary（2022-08-09）16→20px/25%、导航栏加高、400+ icon set；the-new-raycast（2026-05-14）公开 beta、macOS Tahoe、Liquid Glass、Root Search；changelog 1-38-0（2022-07-19）；theme-explorer nord-dark.json 与文档 JSON 逐字节一致（authorUsername/version/author/name/appearance + colors 12 键全值）；manual.raycast.com/themes（Last Updated 30 July 2026）Theme Studio、明暗分离主题、Copy as URL/Copy as JSON、themes.ray.so；开发者文档 Color.Dynamic（light/dark/adjustContrast）、Color.Raw 默认对比度调整、environment.appearance `"dark"|"light"`、List 组件（searchBarPlaceholder/isLoading/pagination/isShowingDetail/quickLook/keywords/List.Dropdown ⌘P/List.EmptyView「never displayed if the List's isLoading property is true and the search bar is empty」/Accessory/Detail CommonMark + Metadata）；raycast/raycast-macos 404（GitHub API "Not Found"）。
- **Raycast 第三方**：[11] loftlyy Raycast Red #FF6363、Raycast Dark #151515、Raycast Black #070A0B、Muted Gray #929292；[12] VoltAgent DESIGN.md 全部 token（canvas #07080a/surface #0d0d0d/surface-elevated #101111/surface-card #121212、hairline #242728 + rgba(255,255,255,0.08/0.16)、ink #f4f4f6/mute #9c9c9d/ash #6a6b6c/stone #434345、accent #57c1ff/#ff6161/#59d499/#ffc533 + 15% soft、圆角 0/4/6/8/10/16/9999、font-feature "calt","kern","liga","ss03"、字号阶梯 16/14/13/12/18–24/56–64、字距 0.1–0.4px、keycap 4px/按钮 8px/功能卡 10px/命令面板 16px/pill、「there are no drop shadows in the system」）；[13] opendesigner.io bg #07080a、fg #f9f9f9、accent #FF6363、Border radius 12px、多层 inset 阴影口径、GeistMono 用于代码；[24] blakecrosley vibrancy/NSVisualEffectView/跟随系统明暗/continuous 圆角/HUD 1.5s 后消失（asyncAfter .now() + 1.5）+ opacity+scale 过渡/confetti .spring()/Esc 逐级返回；[25] daintree #3818（closed）「macOS native sheets and Raycast both use spring-like easing for modal/palette entrances」+「~2-5% beyond the final position」+「Exit/close animations should be faster than enter animations」；[26] blakecrosley arc 指南 .easeOut(duration: 0.2) 侧栏宽度、.spring(response: 0.3, dampingFraction: 0.7) Space 切换（作者自写示意代码，非官方值——文档标注正确）。
- **Arc 官方（release notes 经 Zendesk API 直抓全文逐条核对）**：2022-05-19「New shimmer animation upon starting up Arc」、2022-06-02「New restart window animation」、2022-09-22「A new, more obvious animation on tabs using your microphone, camera, or both」、2022-05-26 Intensity/Graininess 高级主题选项、2022-09-01 v.0.66.0「Introducing Space Themes V2! Now with more customizability, more presets, and dark mode!」、2022-12-08 v.0.80.0「Boosts can now detect your theme color… Grab the CSS for your theme colors here: https://arc.net/colors.html」、2023-05-25 V0.105「Introducing Boosts V2」、Windows 2024-01-08 v.0.4.2「We've added Space theming」、2024-03-14 v.0.13.1「improvements to the Pinned Folder expand/collapse animations」、2024-03-21 v.0.14.2 Mica/Acrylic 二选一、2024-04-08 v.0.17.2「Dragging the Sidebar to resize is now much smoother」+「Dark Space Themes now appear darker」；帮助中心（Zendesk API）：[6] 默认主题恢复 = Theme Picker 的 (-) 按钮取消全部颜色、[18] Dark Reader 排查且官方不推荐、[27] Boosts（Color wheel/Invert lightness (lightbulb)/Contrast+Brightness+Original Saturation 滑块/Size 90%–150%/Zap/CSS and Javascript editors）、[32] 每 Space 独立 Pinned/Unpinned/Theme/Icon、[34] 快捷键表（Command-T/Command-D 等）、[38] Split View「Add Right/Left/Top/Bottom Split」逐字。
- **Arc 第三方**：[4] loftlyy 品牌 6 色全表 + Marlin Soft SQ(400/700)/Inter + 2023-07 公测 + 2025-05 停运 + 2025-09 Atlassian 收购；[8] supasidebar 侧栏四区 + 「auto-clears after 12 hours by default」+ 停运/收购时间线；[20] ArcWTF global-colors.css 全部 7 个色值（#3b3b3b/#2B2B2B/#383838/#444444/#4A4A4A/#CBCBCB/#93d0ff）；[21] open-design.ai Glass Dark rgba(20, 20, 25, 0.6)；[22] dev.to 8 要素配方（bg-white/40、backdrop-blur、rounded-l-2xl、10px 大写标题、2px 未读点、macOS 三色圆点）；[28] The Verge（2023-05-25）「Want to force a website to have a dark mode? Easy.」逐字 + Zaps + CSS/JavaScript；[31] Wikipedia API 版 Arc 条目「Command Bar is similar in functionality and design to Apple's Spotlight feature」；[33] 少数派 95844（2025-01-23）三类页签 + 「12 小时后自动消失」；[35] nateparrott 页面 Command Bar/Split View/Focus Mode/Little Arc/Notes/Easel/Previews/PIP/Mini Audio Player/Boosts 初始设计自述；[37] The Verge「The Arc browser arrives on Windows」；[39] techbeta Little Arc/Split View/侧栏；[44] unpkg use-arc-theme@0.0.1 17 个 CSS 变量全表（--arc-palette-foregroundPrimary/Secondary/Tertiary、--arc-palette-background、--arc-palette-backgroundExtra、--arc-background-simple-color、--arc-background-gradient-color0/1、--arc-background-gradient-overlay-color0/1、--arc-palette-maxContrastColor、--arc-palette-minContrastColor、--arc-palette-focus、--arc-palette-hover、--arc-palette-cutoutColor、--arc-palette-title、--arc-palette-subtitle）；[45] turtle-key/Arcitect 仓库存在（「A custom theme editor for the Arc Chromium based Browser」）；[50] darkreader #12266（open）「The Arc browser provides a set of exposed CSS variables matching the space's theme」；[53] TechCrunch（2023-05-25）Boosts 发布报道；[54] apis.io「Arc Boosts」CSS/JavaScript 条目；[43] 2022-12-08 release notes 中的 arc.net/colors.html 原文；[9] script-commands OUTPUTMODES.md；[29][46][47] 2022/2023/2024-2026 release notes 的其余动画条目（2023-02-16 新加载指示器等可通过全文检索确认的条目）。

## 备注（未列为错误，供参考）

- §4.1「Compact Mode 下仅剩搜索栏+Action Bar [1][17]」——官方博客/Changelog 仅描述 Compact Mode「blending all other elements for a minimal appearance」，无「仅剩搜索栏+Action Bar」的明确表述，属对产品行为的合理转述但来源支撑不足。
- §1 概述「Arc 浏览器……（2023 年 7 月公测）」与 loftlyy「Publicly launched in July 2023」一致；Wikipedia 口径为 2023-03 内测 / 7 月公测，一致。
- 无法验证项（文档已自标「待核」或验证受阻）：[5] 163.com 停运数据（5.52% Space / 4.17% Live Folders / 0.4% 日历悬浮预览）——163 页面反爬（仅 146 字节）、官方公告原文 thebrowsercompany.com/blog/a-message-from-the-browser-company 现 404、Wayback Machine 无快照（429 与 404），数值无法独立复核；[23] arc.net/theme（Cloudflare）；[51] start.arc.net/paint-the-internet（Cloudflare）；[52] zhihu（反爬）；[14] seedflip（仅搜索摘要）；[19] arstechnica 论坛帖（未抓取）。
