# notion-figma 事实核查报告

> 核查日期: 2026-08-15
> 核查方式: WebSearch + WebFetch 外部验证 + 内部交叉比对
> 核查范围: 全量声称逐一核实
> 方向: design

---

## 错误 1: §5.1 映射表列头「亮色内联值 | 暗黑内联值」方向误标——该映射实为暗色主题语境内的值替换

- **文件+行号**: `docs/refer/notion-figma.md` (行 157–169，映射表及其表头「亮色内联值 | 暗黑内联值」)
- **原声称**: 表格以「亮色内联值 → 暗黑内联值」方向呈现，如 "rgb(63,68,71) → rgb(32,32,32)（侧栏/弹窗背景）"、"rgb(47,52,55) → rgb(25,25,25)（页面背景）"、"rgba(255,255,255,0.73) → rgba(255,255,255,0.9)（正文文字）"
- **错误类型**: 事实错误（映射方向误标）
- **正确信息**: NotionThemes global.css 的注释（`/* background: rgb(63, 68, 71) => rgb(32, 32, 32) */` 等）记录的是 **Notion 暗色模式语境内的内联值替换规则**（.notion-dark-theme 下生效），且至少三行明确是**暗色主题 2022 迭代的旧值→新值**：rgb(47,52,55)→rgb(25,25,25) 即暗色页面背景从 #2F3437 改为 #191919（v2ex 帖子 2022-02 证实）；rgb(63,68,71) 同为 2022 前暗色侧栏值；rgba(255,255,255,0.73) 是暗色正文文字不透明度（亮色正文为暖黑 #37352F 系不透明值）。「亮色内联值」列标注整体不成立——这些值均非亮色模式内联值。§2.1 正文对同批数据的定性（"页面/编辑器背景 rgb(25,25,25)=#191919，2022 年由偏灰 rgb(47,52,55) 改"）是正确的，仅 §5.1 表格列头方向标错。
- **反证来源**: https://notionthemes.netlify.app/dark/global.css（2026-08-15 抓取全文，注释行 `background: rgb(47, 52, 55) => rgb(25, 25, 25)`、`background: rgb(63, 68, 71) => rgb(32, 32, 32)`、`rgba(255, 255, 255, 0.73) => rgba(255, 255, 255, 0.9)` 均位于暗色主题覆盖文件内）；https://global.v2ex.co/t/835835（wolfie，2022-02-23：「原来属于偏灰色（47,52,55）现在黑色（25,25,25）」——证实该对为暗色主题自身变更）。

## 错误 2: §2.1 边框行原生值「rgb(37,37,37)」应为「rgb(32,32,32)」

- **文件+行号**: `docs/refer/notion-figma.md` (行 26)
- **原声称**: "| 边框 / 底边框 | `rgb(37,37,37)` / `rgb(60,63,67)` → `rgb(37,37,37)` | [1] |"
- **错误类型**: 事实错误
- **正确信息**: [1]（NotionThemes global.css）中边框映射为 `border: rgb(32, 32, 32) => rgb(37, 37, 37)` 与 `border-bottom: rgb(60, 63, 67) => rgb(37, 37, 37)`——原生边框值是 **rgb(32,32,32)**（即 §5.1 表格中正确写出的「rgb(32,32,32) → rgb(37,37,37)（边框）」），底边框 rgb(60,63,67) 正确。§2.1 该行把边框原生值误写为 rgb(37,37,37)（与自身 §5.1 及来源均矛盾）。
- **反证来源**: https://notionthemes.netlify.app/dark/global.css（2026-08-15 抓取：「border:rgb(32,32,32) => rgb(37, 37, 37)」「boder-right: rgb(32, 32, 32) => rgb(63, 66, 69)」「border-bottom: rgb(60, 63, 67) => rgb(37, 37, 37)」）。

## 错误 3: §5.2 声称的 token 示例名「blue-500」「color-background-primary」不在 [27] 中

- **文件+行号**: `docs/refer/notion-figma.md` (行 190)
- **原声称**: "官方推荐 token 分层：primitive（原始值，如 blue-500）→ semantic（用途语义，如 surface、color-background-primary）→ component（如 button-primary-background-default，格式 asset-type-property-state）[27]"
- **错误类型**: 来源不支撑（示例值虚构）
- **正确信息**: [27]（Update 1: Tokens, variables and styles）的实际示例为 primitive「pink/400」、semantic「surface/brand-contrast」、component「button-primary-background-default」——页面全文无 "blue-500" 与 "color-background-primary"（"blue" 仅出现在「base color — pink, neutral, green, blue, red, purple, orange, and yellow」的基色列举中，无编号示例）。分层结构（primitive/semantic/component）与「asset-type-property-state」格式描述正确，仅两个示例名不实。
- **反证来源**: https://help.figma.com/hc/en-us/articles/18490793776023-Update-1-Tokens-variables-and-styles（2026-08-15 抓取全文：「the token surface/brand-contrast references a primitive token pink/400」「The token for this could look something like button-primary-background-default…asset-typepropertystate」）。

## 核查通过项摘要

以下声称经外部验证全部正确（未单列错误条目）：

- **Notion 背景层级色（[1][3] 抓取全文）**：#191919（页面/编辑器背景，2022 由 rgb(47,52,55)=#2F3437 变更，v2ex 2022-02-23 帖「原来属于偏灰色（47,52,55）现在黑色（25,25,25）」+「太黑了」回复）、#202020（侧栏/弹窗）、#252525（卡片）、rgb(47,47,47) 点击态（含旧值 rgb(61,66,69)）、rgba(255,255,255,0.055) hover、正文 rgba(255,255,255,0.73)、次级 rgb(150,150,150)=#969696、弱文字 rgba(255,255,255,0.39)、#2EAADC（fill 与文本选择色）；NotionThemes 映射表其余行（rgb(37,37,37)→rgb(55,60,63)、rgb(47,47,47)→rgb(88,91,93)、rgba(255,255,255,0.055)→rgba(255,255,255,0.1)、rgb(150,150,150)→rgb(159,164,169)、rgba(255,255,255,0.39)→rgba(25,23,17,0.6)）逐字一致；「*[style*="..."]」选择器证实内联 style 注入、无 CSS 变量体系。
- **标注色 10 组配对（[2] matthiasfrank.de 抓取全文）**：#D4D4D4/#191919、#9B9B9B/#252525、#A27763/#2E2724、#CB7B37/#36291F、#C19138/#372E20、#4F9768/#242B26、#447ACB/#1F282D、#865DBB/#2A2430、#BA4A78/#2E2328、#BE524B/#332523 全部逐字存在。
- **Notion 官方/其他**：[7] release 2021-01-27（27 de enero de 2021）「Better dark mode settings…Always light / Always dark / Use system setting」逐字；[4] NotionThemes README「Dark themes will work only if you switched to the dark Mode in Notion」+ githubDark theme.css 全部变量（--bg #161b22、--bg-light #222830、--bg-lighter #0d1117、--fg #ffffffe6、--main #1f6feb、--hover #30363d、--border #30363d、9 标注色 + 9 标注背景变量）；[5] sotion.so #191919 背景/#58A6FF 链接/prefers-color-scheme；[8] wisechecker Reduce motion（My Settings 内 Accessibility 分区、禁用 slide-in/fade/expand-collapse/侧栏面板滑出、保留 loading spinner/progress bar、即时生效、跨设备同步）；[9] mikenjuki「every block in Notion has a six-dot drag handle…Same handle, same position, same behavior…One signifier, one behavior, full stop」逐字；[10] super.so「Notion only has three fonts (Default, Serif, Mono) and you can't add custom fonts」；[29] gitcode 教程含 max-height 0.3s ease-out 侧栏折叠 CSS（用户注入值）；[32] justzix 页含 .notion-dark-theme 对比度 CSS + NotionNext PR #4411「fix(xuhome): improve dark mode and color contrast」closed/merged 2026-08-11。
- **Notion iOS 拆解（[6] 抓取全文）**：200ms easeOut 块动画、280ms easeInOut 抽屉、AI 下划线 800ms 淡出、侧栏 86% 宽 + 遮罩 rgba(15,15,15,0.4)、字号 token 全表（--page-title 30/700/-1.0px、--h1 26/700/-0.6px、--h2 22/700/-0.4px、--h3 18/700、--body 16/400、--caption 14/500、--meta 12/500、--code 13/400）、行高 1.45–1.5、圆角「almost never exceed 6px」（Cards 4/Inputs 5/Buttons 6）、Kanban 卡片 4px 圆角 + 1px 边框 + no shadow、激活页签单条黑色下划线、六点手柄 14×14pt + tap 目标 36pt、slash 面板 sheet radius 6pt/行高 44pt/图标 36pt、页面边距 18pt/块间距 4pt（标题上 8pt）、六种柔和色调仅用于高亮 pill 与 callout 背景。
- **Figma 官方（博客/帮助中心抓取全文）**：[13] 主题帮助「While in dark mode, they'll default to an off-black color #1E1E1E」+ 亮色 #F5F5F5 逐字、device-specific、Enhanced contrast；[14] 画布帮助「In dark mode, the background defaults to an off-black color: #1E1E1E」「the new page inherits its background color from the current page's background color」「The canvas background color cannot be changed while in Dev Mode」；[15] Illuminating dark mode（2022-07-21）「we shipped dark mode in May」「about 5100 instances」「350 semantic tokens」「$figmaBlue showed up as #0d99ff」「color: var(--color-text-brand, $figmaBlue)」逐字、5 级 schema（Type{bg,text,icon,border}×UI Element{default,menu,toolbar,tooltip}×Color role{default,brand,selected,design,figjam,component,assistive,danger,warning,success,disabled,info}×Prominence{default,secondary,tertiary,strong}×Interaction{default,hover,pressed}）、示例 color-bg-menu-secondary-hovered、「menu colors did not invert in dark mode」、Dark/Light(new)/Debug 三主题 + Legacy、CI linter、DTFM 导出 + CSS tokens/Swift/XML 目标输出、Dark Mode Week「forty engineers a single week…80% of surfaces that were in scope」；[16] figma-on-figma（2024-09-16）Figma Sans 与 Grilli Type 合作 + Figma Condensed/Figma Mono/Figma Hand（OH no Type）、bold primaries/bright neons/muted earthy tones、动效三原则 Craft（fast, snappy）/Interactivity/Freeform（camera moves + parallax）、variables 驱动扩展配色；[17] The birth of Inter（2019-08-08）「In late 2018, we had a project to do a redesign of our UI」「Roboto…difficult to read…when it was small」「Inter has a relative x-height of exactly 3/4th the cap height」逐字；[18] 五区域（Navigation bar/Left sidebar/Canvas/Right sidebar/Toolbar）；[19] UI3（2024-10-01）「available for all users on October 10」「Tim Van Damme hand-drew 200 icons…including these directions for dark and light mode」「Toolbars will float at the bottom of all Figma products」「reversing the change so that panels are fixed, but still resizable」+ Minimize UI；[20] spring 博客（2022-06-16）F=m·a/F=-k·x/F=-b·v、WebKit SpringSolver、「vertical position of the handle controls overshoot, and the horizontal position controls the relative speed」「duration handle to signal where an animation will officially stop」、Framer motion mass/damping/stiffness vs react-spring mass/friction/tension；[21] variables 帮助「You can create up to 5,000 variables per collection」「A mode is a list of values for a variable in a collection」、Timing「such as 500ms」、Easing 曲线或 spring、六类型（Color/Number/String/Boolean/Timing/Easing）；[26] 原型帮助 spring 预设 Gentle/Quick/Bouncy/Slow + Custom；[27] token 三层（Primitive/Semantic/Component-specific）+ surface/brand-contrast/pink/400/button-primary-background-default 示例 + asset-type-property-state 格式。
- **Figma 第三方**：[22] colorarchive 品牌五色 #A259FF/#F24E1E/#FF7262/#1ABCFE/#0ACF83（HTML 逐字确认）；[23] colorpickercode figma-dark 含 #2c2c2c/#1e1e1e；[24] 中文教程 #121212/#E0E0E0/#333333（搜索摘要口径）；[28] devanddeliver 页面可达（截图源）；[32] justzix/NotionNext PR（见上）；[8] 未核实项清单与文内「待核」标注一致。

## 备注（未列为错误，供参考）

- 无法验证项（文档已自标「待核」或验证受阻，与文档声明一致）：[11] notion.so Color Palette 页的 --notion-black/--notion-white（SPA JS 渲染，页面 HTML 无 token 文本；页面 CSS 中可见 body.dark{background:#191919} 佐证暗色背景）；[25] forum.figma.com 阻尼比公式与 0.1% 终止容差（页面 403 Cloudflare，仅搜索摘要）；[12] docs.super.site、[24] php.cn、[29] m.17golang（44 字节 404，gitcode 镜像已支撑同一条）为搜索摘要来源；Notion 桌面/Web 原生动画时长、Cmd/Ctrl+Shift+L 快捷键、Figma 编辑器 UI 字号/圆角/阴影/动画时长数值——官方未公开（文档自述）。
- §2.1「正文文字 rgba(255,255,255,0.73)」「弱文字 rgba(255,255,255,0.39)」「次级文字 #969696」等单值引用本身在来源中逐字存在（暗色语境值），仅 §5.1 表格的「亮色」列头方向有误（见错误 1），单值引用未单列错误。
- NotionThemes README 中 GitHub 仓库链接为 notionblog/NotionThemes（源文件 [4] 引用的 raw 路径为 mindkhichdi/NotionThemes，文件可访问且内容一致），fork/更名差异不影响引用有效性。
