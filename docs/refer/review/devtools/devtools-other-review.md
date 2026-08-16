# devtools-other 事实核查报告

> 核查日期: 2026-08-15
> 核查方式: WebSearch + WebFetch 外部验证 + 内部交叉比对
> 核查范围: 全量声称逐一核实
> 方向: devtools

---

## 错误 1: "ST3.2 起默认 UI 主题为 Adaptive"的版本归属与机制描述均不被引用来源支撑

- **文件+行号**: `docs/refer/devtools-other.md` (第 15 行，2.1 双层着色架构)
- **原声称**: "ST3.2 起默认 UI 主题为 **Adaptive**（自适主题）：它从当前配色方案的背景/前景主色中提取主色，自动给侧栏、页签、状态栏着上同色调（深色配色方案 → 深色 UI，浅色方案 → 浅色 UI）[5][7][8]"
- **错误类型**: 来源不支撑
- **正确信息**: 三个引用均不支撑"ST3.2 起默认"这一版本归属：[5]（sublimehq/sublime_text#3665）是 bug 报告（标题 "The Adaptive.sublime-theme is a mess"，closed as not planned），既未描述取色机制也未提版本；ST3.2 官方博客全文（含 Themes/UI 变更清单：variables 支持、extends 关键字、CSS 颜色语法）无 "Adaptive" 字样；[8] placeless 博文为 2017-2018 年（ST3.1 时代）内容，早于 ST3.2（2019-03），反而说明 Adaptive 在 3.2 之前已存在。取色机制仅可部分经 [8] 佐证（title_bar 用 `["background", 52, 61, 70, 0.4]` 从配色方案取色）。
- **反证来源**:
  - https://github.com/sublimehq/sublime_text/issues/3665（issue 正文为渲染故障报告，无机制/版本内容）
  - https://www.sublimetext.com/blog/articles/sublime-text-3-point-2（全文无 "Adaptive"）
  - https://placeless.net/blog/customized-adaptive-theme-of-sublime-text（2017-2018 年，ST3.1 语境）

## 错误 2: "ST4 将 Mariana 列为首推暗色方案"的引用 [6] 不支撑该说法

- **文件+行号**: `docs/refer/devtools-other.md` (第 15 行，2.1)
- **原声称**: "ST4 将 **Mariana**（蓝灰调）列为首推暗色方案[6]"
- **错误类型**: 来源不支撑（兼内部矛盾）
- **正确信息**: [6]（mbadolato/iTerm2-Color-Schemes）是终端配色移植仓库，不含任何 ST4 表述。正确出处是 ST4 官方博客 [4]（"UI Changed default color scheme to Mariana"）。同时该句与同段上一句"默认配色方案为 Monokai（官方文档原文…）[2]"形成内部张力——ST4 博客明确将默认配色方案改为 Mariana，两处声称需协调。
- **反证来源**: https://www.sublimetext.com/blog/articles/sublime-text-4 → changelog "UI Changed default color scheme to Mariana"；https://github.com/mbadolato/iTerm2-Color-Schemes（无 ST4 相关内容）

## 错误 3: "v0.15 起改为跟随系统默认字体"不被引用来源 [22] 支撑

- **文件+行号**: `docs/refer/devtools-other.md` (第 80 行，2.2 字体排版)
- **原声称**: "v0.15 之前默认 UI/编辑字体为 **Inter**（随安装包内置），v0.15 起改为跟随系统默认字体，可在 Appearance 设置中切回[22]"
- **错误类型**: 来源不支撑
- **正确信息**: 论坛帖 [22]（forum.obsidian.md/t/19301，2021-06-05）仅确认默认字体为 Inter（"It is called Inter"），全文无 "0.15" 或任何版本号——版本归属与"切换系统字体"变更均不出自该帖。
- **反证来源**: https://forum.obsidian.md/t/q-default-font-in-default-theme/19301（全文检索无 "0.15"/"v0.15"）

## 错误 4: "编辑区默认 10pt 等宽字体"不被引用来源 [1] 支撑

- **文件+行号**: `docs/refer/devtools-other.md` (第 36 行，2.1 字体排版)
- **原声称**: "编辑区默认 10pt 等宽字体（Consolas/Menlo 系）[1]"
- **错误类型**: 来源不支撑
- **正确信息**: [1]（sublimetext.com/docs/themes.html）的 "Font Sizes" 节只说明字号可用的书写格式（整数/浮点像素、px/rem 字符串），未给出任何默认字号；"10pt" 事实本身正确（Sublime 默认 `font_size: 10`），但引用出处不支撑。
- **反证来源**: https://www.sublimetext.com/docs/themes.html（"Font sizes may be specified in the formats: ..." 无默认值表述）

## 错误 5: Obsidian 页签"8 档文本色"计数错误（实际 6 个文本色变量）

- **文件+行号**: `docs/refer/devtools-other.md` (第 139 行，4.2 组件拆解-页签)
- **原声称**: "`--tab-text-color-focused-active` 等 8 档文本色区分聚焦/激活/高亮状态"
- **错误类型**: 事实错误
- **正确信息**: 官方 Tabs 文档中文本色变量共 **6 个**：`--tab-text-color`、`--tab-text-color-active`、`--tab-text-color-focused`、`--tab-text-color-focused-active`、`--tab-text-color-focused-highlighted`、`--tab-text-color-focused-active-current`。
- **反证来源**: https://raw.githubusercontent.com/obsidianmd/obsidian-developer-docs/main/en/Reference/CSS%20variables/Components/Tabs.md（完整变量表逐行核对）

## 无法验证项

- **[9] https://www.php.cn/faq/1841928.html 与 [10] https://www.php.cn/faq/2202182.html**：两页均被反爬拦截（仅返回 cookie 跳转脚本，curl 与 WebFetch 均无法取得正文），动画设置项（animation_enabled/scroll_animation_length/tree_animation_enabled/scroll_speed）出处与 Monokai 色值出处无法核实。文档对该两处已自行标注"第三方整理，数值官方文档未公开，待核"。Monokai 色值本身（bg #272822、fg #F8F8F2、comment #75715E）经 VS Code 官方 Monokai 移植主题独立交叉验证一致（事实正确，仅出处无法核实）。

## 核查通过项摘要

以下声称经外部验证全部正确（未单列错误条目）：
- Sublime Text 部分：官方文档 [1] themes.html 机制全对（layer0/1/2 + opacity/tint/texture、attributes 状态（如 `["hover"]`）、`file_tab_style: "rounded"`、tint 接受 `[r,g,b,a]`/CSS 颜色/`["foreground","grayscale",25]`/`["background",255,255,255,0.1]`/两元素透明度数组、字号整数 px 或 px/rem 字符串）；[2] color_schemes.html "the default Monokai color scheme" 与 `Packages/User/Monokai.sublime-color-scheme` 覆盖规则逐字一致；[3] ST3.2 博客（extends 关键字、Git 状态徽标、"Themes may customize the display of sidebar badges and status bar information"）逐字一致；Mariana 色值双源交叉一致（[6] iTerm2 Mariana.itermcolors 与 [7] Remmina Mariana.colors 均含 bg `#343d46`、fg `#d8dee9`、光标 `#fcbb6a`、高亮 `#4e5a65`、红 `#ec5f66`、绿 `#99c794`、黄/橙 `#f9ae58`、蓝 `#6699cc`、紫 `#c695c6`、青 `#5fb4b4`）；[8] placeless 博文含 `[ "background" , 52 , 61 , 70 , 0.4 ]` 标题栏 tint 示例（"RGBA [52,61,70,0.4] ≈ Mariana 背景"一致）。
- Obsidian 部分：官方文档 [11] Colors（--color-base-00~100 暗色 12 值全对：`#1c1c1c`/`#212121`/`#232323`/`#282828`/`#2e2e2e`/`#333333`/`#3f3f3f`/`#555555`/`#666666`/`#999999`/`#b3b3b3`/`#dadada`；accent-h/s/l `258/88%/66%` 与 --color-accent-1/-2 hover/active 位移；扩展色 8 色暗色值全对；"As of Obsidian 1.13, colors use the OKLCH color space" 与 color-mix(in oklch) 推荐逐字一致；语义色仅给用途不给映射值，与文档"映射值待核"口径一致）；[12] Typography（--font-text-size 16px、--font-ui-small 13px/medium 15px/large 20px、字重 100–900、--line-height-normal 1.5/tight 1.3）；[13] Radiuses（4/8/12/16px）；[14] Layers（--layer-popover 30、--layer-modal 50、--layer-menu 65、--layer-tooltip 70、--layer-status-bar 15）；[15] Borders（--border-width 1px）；[16] Tabs（除"8 档"计数外全对：--tab-background-active、--tab-curve、--tab-radius/--tab-radius-active、--tab-divider-color、--tab-outline-color/width、--tab-width/max-width、tab stacks 独立变量集含 --tab-stacked-shadow）；[17] Divider（--divider-color/--divider-width）；[18] Ribbon（--ribbon-width/--ribbon-background）；[19] Build a theme（`.obsidian/themes/` 目录、theme.css + manifest.json、body/.theme-dark/.theme-light 作用域）；[20] About styling（"hundreds of CSS variables"）；[21] Theme guidelines（变量优先、低特异性、"Avoid `!important` declarations" 逐字一致）；[22] 论坛帖 Inter 默认字体确认（仅版本归属见错误 3）；[23] obsidian-advanced-appearance README（--accent-h/s/l、--base-h/s/l、--base-d 导出到 body 逐字一致）；[24] deepwiki Primary 主题（--anim-popup: 0.3s slideUp forwards、--anim-popup-alt: 0.335s slideUpAlt forwards、--anim-popdown: 0.4s slideDown forwards 逐字一致）；文档站暗色实测 `#1e1e1e`/`#dadada` 与 docs.obsidian.md preload 样式逐字一致。
- 交叉比对：devtools-other.md 与同组其它三份文件无同一事实点冲突；内部矛盾（默认 Monokai vs ST4 改默认 Mariana）已在错误 2 记录。

## 备注（未列为错误，供参考）

- "Adaptive 主题默认 UI 各元素的具体 tint 色值"、"Obsidian 默认主题语义变量映射值与 hover 过渡时长"、"Obsidian 默认主题弹层阴影参数"等文档自标「待核」项，均无法从官方来源核实，文档已自行标注。
- `theme.css` "热重载，无需重启"（[19]）：build-a-theme 教程明确要求 manifest.json 变更需重启，theme.css 变更以"reload"体现，未发现与"热重载"直接矛盾或支撑的官方表述，未列为错误。
