# terminals-other 事实核查报告

> 核查日期: 2026-08-15
> 核查方式: WebSearch + WebFetch 外部验证 + 内部交叉比对
> 核查范围: 全量声称逐一核实
> 方向: terminal

---

## 错误 1: iTerm2 PR #627 合并日期错误——2026-03-25 是 PR 提交日，实际合并为 2026-04-09

- **文件+行号**: `docs/refer/terminals-other.md` (5.3 iTerm2, 第 223 行; 6 节来源清单 [48], 第 306 行)
- **原声称**: 「tab color 在 macOS 明暗切换时丢失（PR #627，2026-03-25 合并，修复为按「当前 appearance 键→相反键→共享无后缀键」顺序读取）」
- **错误类型**: 事实错误（日期归因错误）
- **正确信息**: PR #627 由 schneidermayer 于 2026-03-25 提交（GitHub 时间线「schneidermayer commented Mar 25, 2026」）；实际合并发生在 2026-04-09（gnachman 于 2026-04-09 评论「Merged as 4ed704d with additional changes in eaefa14」并关闭 PR）。修复顺序（当前 appearance 键 `iTermAmendedColorKey2(baseKey, YES, dark)` → 相反键 `!dark` → 无后缀 `baseKey`）经 PR diff 核实属实
- **反证来源**: https://github.com/gnachman/iTerm2/pull/627 — 时间线「gnachman commented Apr 9, 2026... Owner Merged as 4ed704d with additional changes in eaefa14」+「gnachman closed this Apr 9, 2026」; https://github.com/gnachman/iTerm2/pull/627.diff — `NSString *key = iTermAmendedColorKey2(baseKey, YES, dark); if (profile[key]) return key; NSString *fallbackKey = iTermAmendedColorKey2(baseKey, YES, !dark); if (profile[fallbackKey]) return fallbackKey; return baseKey;`

## 错误 2: Tabby「Windows/Linux 兜底 monospace」与源码不符（平台默认是具体字体）

- **文件+行号**: `docs/refer/terminals-other.md` (2.2 Tabby 字体排版, 第 55 行)
- **原声称**: 「终端 `fontSize: 14`，macOS 默认 Menlo、Windows/Linux 兜底 monospace」
- **错误类型**: 事实错误
- **正确信息**: macOS 默认 Menlo 属实；但 Windows 平台默认字体为 **Consolas**、Linux 为 **Liberation Mono**（均为具体字体名，非 monospace 兜底）——tabby-terminal/src/config.ts 的 platformDefaults 逐字定义
- **反证来源**: https://github.com/Eugeny/tabby/blob/master/tabby-terminal/src/config.ts — `[Platform.Windows]: { terminal: { font: 'Consolas', ... } }`、`[Platform.Linux]: { terminal: { font: 'Liberation Mono', ... } }`、`[Platform.macOS]: { terminal: { font: 'Menlo' } }`

## 错误 3: alacritty-theme README 主题目录条数「168」与实际 170 不符

- **文件+行号**: `docs/refer/terminals-other.md` (5.4 Alacritty 仓库结构, 第 236 行; 6 节来源清单 [56], 第 319 行)
- **原声称**: 「README.md 安装说明 + 按字母序主题目录（**168 条**带预览图）」
- **错误类型**: 事实错误（数字不精确）
- **正确信息**: README「Color Schemes」表格实际含 170 个唯一 `images/*.png` 预览引用行（2026-08-15 实测）；`themes/` 目录 176 个 TOML（此数正确）
- **反证来源**: https://github.com/alacritty/alacritty-theme/blob/master/README.md — `grep -oE "images/[A-Za-z0-9_.-]+\.png" | sort -u | wc -l` = 170

## 核查通过项摘要

以下声称经外部验证全部正确（未单列错误条目）：
- **Ghostty 配色**：StyleDark 全套色值逐字一致（discussion #5390：background `#292c33`、foreground/cursor-color/selection-background `#ffffff`、cursor-text `#363a43`、selection-foreground 作者修正值 `#292c33`、palette 0–15 全表含 `7=#c5c8c6`）；来源 [12] stvhay fork conf「7=#ffffff 与 ghostty 实际 #c5c8c6 有差异」逐字核实（fork 文件 `kern.vt.color.7.rgb="#ffffff"`）；「StyleDark 为 community 命名」属实（collaborator rhodes-b 原文「I've only seen other people call it that... I cant find any reference to it」）
- **Ghostty 配置**：Config.zig 逐项核实——background 默认 `#282C34`、font-size macOS 13/其它 12（注释「On macOS we default a little bigger」逐字）、window-padding-x/y 默认 2、background-blur 默认 false（macOS 26.0+ 玻璃值、X11 仅 KWin 注释逐字）、background-opacity 1.0、background-image null、palette-generate false（注释「The default value is false (disabled)」+ 6×6×6 色立方/24 步灰度）、quick-terminal-position top + 「There is no default keybind for toggling the quick terminal」逐字、macos-titlebar-style transparent、window-decoration/window-theme auto（auto→system 逻辑）、alpha-blending macOS native/其它 linear-corrected、window-padding-color background、split-divider-color 存在、window-show-tab-bar auto、palette 语法（0–255、0b/0o/0x 前缀、hex 可省 #、X11 命名色）、全文检索无 corner-radius、无 statusline 键
- **Ghostty 主题/文档**：theme.zig 查找目录（user XDG config → resources）+ 绝对路径；ghostty.org theme 页（iterm2-color-schemes 每周同步、+list-themes、`dark:X,light:Y` 语法）；config 页（零配置哲学、「embeds a default font (JetBrains Mono)」逐字、config.ghostty/1.2.3 前 config、双路径、ctrl+shift+, / cmd+shift+, 重载）
- **Ghostty 动画**：235 帧逐帧核实（src/build/framegen/frames 目录 235 个 frame_XXX.txt，frame_001/frame_120 实测含 `<span class="b">` 字符画）；+boo commit 9cb297202b（2025-01-08、commit message 逐字「All files are concatenated together using \x01 as a combining byte」「The overall addition to the binary size is 348k」、@embedFile、framedata 模块）；discussion 7010（「If you have a tip build, ghostty +boo」「tip is the nightly version」）；PR #10903「macos: optimize secure input overlay animation」；1.3.0（2026-03-09：scrollback search、原生滚动条默认 system、overlay 不占网格引文「aesthetically unpleasant gap or gutter」逐字、shader uniforms「cursor shape, position, previous position, time since change, color scheme」、macOS 右键页签设色 #9784、GHOSTTY_QUICK_TERMINAL #9673）；1.3.1（2026-03-13、progress-style、set_tab_title、双击重命名全屏修复 #11353）；discussion 7297（background 亮暗双值语法为请求且回答仅给 workaround，无支持证据）/5595（「Ghostty looks dimmed compared to kitty」）/2763（window-theme auto/system 行为）；deepwiki GTK（SplitTree 自定义实现确认；AdwTabView/GtkPaned 未在页面出现，文档已标注「待核」合理）；social-share-card.jpg 实测 3200×1800
- **Tabby**：colorSchemes.ts「Tabby Default」全色值逐字（bg `#171717`/fg `#cacaca`/cursor `#bbbbbb`/16 ANSI 全表）；v1.0.190 config.ts「Material」逐字（bg `rgba(38, 50, 56, 1)`/fg `#eceff1`/cursor `#FFCC00`/亮黑 `rgba(255,255,255,0.2)`）；configDefaults.yaml（theme 'Follow the color scheme'、colorSchemeMode 'dark'、tabsLocation top、opacity 1.0、vibrancy false、animations true）；master config.ts（colorScheme=Tabby Default、lightColorScheme、customColorSchemes、minimumContrastRatio 4、frontend 'xterm-webgl'、fontSize 14）；themes.service.ts（`luminosity() <` 暗亮判定、--theme-bg/fg-more(-2) 0.25/0.5 阶梯、--bs-body-bg/--bs-{color} 映射、minimumContrastRatio 对比度、`document.body.classList.toggle('no-animations', ...)`）；全部动效参数（fadeIn 0.5s ease-out、进度条 1s ease-out width、页签进入 250ms ease-out 1px→200px 逐字、离开 250ms ease-in-out、页签宽度 0.125s ease-out、拖拽 250ms cubic-bezier(0,0,0.2,1)、悬停按钮组 0.25s opacity、分屏 0.125s all + opacity .75、窗口背景 0.25s、resizing 禁过渡）；titlebar 30px（含 macOS inset 36px line-height）、tab 38px * spaciness、侧边 200px * spaciness、content-tab left -1000%/active 0 无过渡；dropdown 阴影逐字、maximized 阴影 + blur(10px) + 圆角 10px；theme.vars.scss（Source Sans Pro/Source Code Pro/$font-size-base 14px/line-height 1.6/$border-radius .4rem）；README 插件清单（hype/relaxed/gruvbox/windows10/altair/catppuccin/noctis）；tabby-theme-hype 插件结构逐字（`class HypeTheme extends Theme { name = 'Hype'; css = require('./theme.scss'); terminalBackground = '#010101' }` + NgModule）
- **iTerm2**：ColorPresets.plist「Dark Background」全色值逐字（bg `#000000`/fg `#BBBBBB`/Bold `#FFFFFF`/Cursor `#BBBBBB`/Cursor Text `#FFFFFF`/Selected Text `#000000`/ANSI 0–15 全表；Selection 由 float 0.7098/0.8353/1.0 换算约 `#B5D5FF`，文档取值合理）；11 个 preset 名逐一核实（含 Smoooooth）；High Contrast `#000000/#C7C7C7`、Smoooooth `#15191F/#DCDCDC`、Solarized Dark `#002B36/#839496` 全对；DefaultBookmark.plist「Normal Font = Monaco 12」；Minimum Contrast（「At 100, all text will be pure black or pure white. Minimum contrast never modifies background」逐字）、Cursor Boost、Use separate colors 明暗分离、Tab Color；2.1 文档「Animate dimming」「magenta→red→gray 标签色」「no annoying animation」「1-pixel border」「ESC]0;string ^G」逐字；3.5.12 changelog「Cursor movement can now be animated... Enable Animate Movement in profiles text settings」；general-usage 蓝点/活动指示器/`⃠` 图标；appearance 页 blue circle、七主题选项（Regular/Minimal/Compact/Light/Dark/Light High Contrast/Dark High Contrast）、「Flash tab bar when switching tabs in fullscreen」、「Show line under title bar... Turn this off for a sleek appearance with the dark theme」、Minimal/Compact 标签入标题栏；状态栏组件全集 21 项逐字；GitLab #12870 closed；.itermcolors 结构（Ansi N Color + RGB Component 0–1 float，solarized 示例实测）；SE #181825（macOS 10.10 窗口越界移除滑动动画）、#48757（Advanced→Tab→outline around selected tab 0–3：largest (3)）；mbadolato README（iTerm2 v3.4.19 port 声明逐字、screenshots 目录）；iterm2colors.com 实测连接失败（HTTP 000）；python API 页含 `async_get` 与「Dark Background」
- **Alacritty**：config-alacritty.html 全默认值逐字（primary `#181818/#d8d8d8`、normal/bright/dim 24 色全表、font Windows Consolas/macOS Menlo/Linux monospace + size 11.25、builtin_box_drawing true、decorations_theme_variant、footer_bar 用途、opacity 1.0、blur false 仅 macOS/KDE Wayland）；alacritty_0_12.toml「Alacritty's default color scheme pre-0.13 (based on tomorrow_night)」+ `#1d1f21/#c5c8c6` 逐字；alacritty-theme 仓库（2023-01-19 创建、2889 stars、Apache-2.0、themes/ 176 个 TOML、根结构 LICENSE/README/images/print_colors.sh/themes、README 安装说明 clone + `[general] import = ["~/.config/alacritty/themes/themes/{theme}.toml"]` + Manual 复制全文逐字、dracula.toml 结构 [colors.primary]/[colors.normal]/[colors.bright] 语义色键无数字下标、nord.toml `#2E3440/#D8DEE9`、内置暗色主题代表清单 15 项全部在 README）；issue #2053「Smooth Scrolling」open + chrisduerr 引文「annoy the living crap out of most users」与 nixpulvis「"cost" something, both at runtime, and in code complexity」逐字；issue #1360（README 引文「The simplicity goal means that it doesn't have features such as tabs or splits...」逐字 + chrisduerr 2025-10-24「Alacritty didn't add tabs.」逐字）；issue #8285（chrisduerr 唯一评论「Not interested in adding a configuration option for such a niche feature at this time.」）；CHANGELOG 版本线 0.18.0-dev/0.17.0、tabs 条目均 macOS 系统 tabbing；features.md 六功能全集（Vi Mode/Search/Hints/Selection expansion/Opening URLs with the mouse/Multi-Window）与 Multi-Window 用法逐字；alacritty-smooth-cursor 确为 fork（desc「Fork with a basic cursor movement animation」）

## 备注（未列为错误，供参考）

- Ghostty「双击页签重命名（1.3.1）」：1.3.1 发布说明原文是「Fix fullscreen tab title rename hit testing so you can now double-click to rename tabs in **native fullscreen**」（#11353）——双击重命名是修复全屏 hit-test 的语境，文档表述略泛化，功能本身存在
- Ghostty「主题在用户配置之前加载，冲突时用户配置覆盖」：theme.zig 中 Location 优先级已确认（user → resources），但加载时序的具体调用路径未在本次核查的源码文件中直接验证
- iTerm2 热键窗口「3.1 起自动判定：窗口贴边且不跨屏→滑动，否则淡入淡出」：issue #2288 状态与标题确认（open），但评论需认证（401），「3.1 起自动判定」细节未获一手证据
- iTerm2「MinimalTabStyleOutlineStrength（0–3）」键名：SE #48757 确认设置存在与 0–3 范围（largest (3)），键名本身未在公开来源中出现
- iTerm2「Animate dimming 现行文档已无此项（待核）」：2.1 文档确有此项；现行文档是否移除未逐一验证，标注待核合理
- Ghostty social-share-card「2024-12-22 生成」：图片尺寸 3200×1800 已实测，生成日期无法从图片直接验证
- mbadolato「450+ 主题」：README 未含该数字，无法验证
- Tabby「社区配色以 Xresources 格式提交（PR #7158）」「Material 切换版本区间 v1.0.191~v1.0.206」：前者仅搜索摘要、后者已标注「精确版本待核」，均维持原标注
- 同组交叉比对未发现矛盾：Warp default_dark.yaml 与 Alacritty 默认配色背景/前景相同（#181818/#d8d8d8）但 16 色不同，分属 base16 default dark 与 Alacritty 自身方案，文档各自引用正确
