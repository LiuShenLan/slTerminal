# zed 事实核查报告

> 核查日期: 2026-08-15
> 核查方式: WebSearch + WebFetch 外部验证 + 内部交叉比对
> 核查范围: 全量声称逐一核实
> 方向: devtools

---

## 错误 1: 语法高亮表中 selector 色值错误

- **文件+行号**: `docs/refer/zed.md` (第 103 行，2.1 语法高亮表)
- **原声称**: "number / boolean / selector | `#bf956aff`（橙）"
- **错误类型**: 事实错误
- **正确信息**: `number` 与 `boolean` 确为 `#bf956aff`，但 `selector` 实际为 `#dfc184ff`（与 `constant` 同值），非 `#bf956aff`。
- **反证来源**: https://raw.githubusercontent.com/zed-industries/zed/main/assets/themes/one/one.json → `"selector": {"color": "#dfc184ff", "font_style": null, "font_weight": null}`（number/boolean 为 `#bf956aff`）

## 错误 2: "语义状态色 13 组"计数错误（实际 14 组），与同段自列清单矛盾

- **文件+行号**: `docs/refer/zed.md` (第 207 行，5.1 style 分区)
- **原声称**: "语义状态色 13 组 ×3 级（主色/background/border）：conflict、created、deleted、error、hidden、hint、ignored、info、modified、predictive、renamed、success、unreachable、warning"
- **错误类型**: 内部矛盾（兼事实错误）
- **正确信息**: 实际为 **14 组**（conflict、created、deleted、error、hidden、hint、ignored、info、modified、predictive、renamed、success、unreachable、warning），同段枚举本身即列出 14 个名称，与"13 组"自相矛盾。
- **反证来源**: https://raw.githubusercontent.com/zed-industries/zed/main/assets/themes/one/one.json（style 下 14 个语义状态色键，每组含主色/`.background`/`.border` 三级）

## 错误 3: 命令面板触发键位错误（cmd-k/ctrl-k → 实际 cmd-shift-p/ctrl-shift-p）

- **文件+行号**: `docs/refer/zed.md` (第 140 行动效表 + 第 159 行图注，均称 "`cmd-k` / `ctrl-k` 打开"命令面板，引 [32][35])
- **原声称**: "命令面板 | 无动画，即时出现 | — | `cmd-k` / `ctrl-k`"；"命令面板：`cmd-k`/`ctrl-k` 打开，模糊过滤全部命令与动作，键盘全程导航 [35]。"
- **错误类型**: 事实错误
- **正确信息**: 官方文档 [35]（zed.dev/docs/command-palette）现行键位为 **cmd-shift-p / ctrl-shift-p**（原文 "Its keybinding is one of the first shortcuts to learn: cmd-shift-p|ctrl-shift-p"）。`cmd-k` 是主题选择器等命令的前缀键（appearance 文档 "Press cmd-k cmd-t|ctrl-k ctrl-t to open the Theme Selector"），不是命令面板键位。
- **反证来源**: https://zed.dev/docs/command-palette → "The Command Palette is the main way to access actions in Zed. Its keybinding is one of the first shortcuts to learn: cmd-shift-p|ctrl-shift-p ."

## 错误 4: "One Dark 暗色共约 250 键"与主题文件实测不符

- **文件+行号**: `docs/refer/zed.md` (第 199 行，5.1 style 分区)
- **原声称**: "**style 分区**（One Dark 暗色共约 250 键）[7]"
- **错误类型**: 事实错误
- **正确信息**: one.json 的 One Dark style 分区实测 **141 个顶层键**（含嵌套叶值共 278 个颜色/参数值）。"约 250"既不符合顶层键数，也高于官方博客的 "200+ 颜色 token" 口径（[6]）。
- **反证来源**: https://raw.githubusercontent.com/zed-industries/zed/main/assets/themes/one/one.json（`themes[0].style` 顶层键数 141）；https://zed.dev/blog/theme-builder（"hunting through 200+ options"）

## 错误 5: reduce_motion 注释的引用文件错误（[34] 指向 editor.rs，注释实际在 animation.rs）

- **文件+行号**: `docs/refer/zed.md` (第 148 行要点 3 + 第 267 行来源清单 [34])
- **原声称**: "另有 `App::reduce_motion` 全局减动效开关（装饰性动效会遵循它，GPUI 侧注释明确）[34]"；来源 [34] 标注 "crates/editor/src/editor.rs（reduce_motion 注释）"
- **错误类型**: 来源不支撑
- **正确信息**: 引用的 `crates/editor/src/editor.rs` 全文不含 `reduce_motion`；该注释实际位于 `crates/gpui/src/elements/animation.rs`（"Animations rendered through this trait automatically respect [`App::reduce_motion`]: when it is set, the element is rendered in a static state..."）。声称内容本身正确，但引用 URL 错误。
- **反证来源**: https://raw.githubusercontent.com/zed-industries/zed/main/crates/gpui/src/elements/animation.rs（第 76-82 行注释）vs https://raw.githubusercontent.com/zed-industries/zed/main/crates/editor/src/editor.rs（grep 无 reduce_motion）

## 错误 6: [10] PR #40035 从未合并，不能作为"主题模式切换 action"依据

- **文件+行号**: `docs/refer/zed.md` (第 3 行头部 + 第 231 行 5.3 表 + 第 269 行来源 [10])
- **原声称**: "One Dark 同时是官方文档中动态主题的默认 dark 值 [2][10]"；"`theme`（动态模式）| `{ "mode": "system", "light": "One Light", "dark": "One Dark" }` [2][10]"；来源 [10] "PR #40035 主题模式切换 action（One Dark/One Light 为内置兜底）"
- **错误类型**: 来源不支撑
- **正确信息**: PR #40035（"Theme mode switch action: switch between light and dark themes"）于 2025-12-02 **closed、未合并**（merged: false）。默认值与默认主题对的声称本身正确（由 [2] appearance 官方文档支撑），但 [10] 不支撑任何内容，应删除或替换为实际合入的 PR。
- **反证来源**: https://api.github.com/repos/zed-industries/zed/pulls/40035 → `"state": "closed", "merged": false, "merged_at": null`；https://zed.dev/docs/appearance → `{ "theme": { "mode": "system", "light": "One Light", "dark": "One Dark" } }`

## 错误 7: [20] PR #8241 从未合并，且所列状态栏设置名已过时/不存在

- **文件+行号**: `docs/refer/zed.md` (第 181 行 4.4 状态栏 + 第 279 行来源 [20])
- **原声称**: "可控显隐：`show_status_bar`、`show_active_language`、`show_cursor_position`、`show_feedback_icon`、`status_bar.language_server_button`、`line_endings_button`、`active_encoding_button` [5][20]"；来源 [20] "PR #8241 状态栏项显隐设置"
- **错误类型**: 来源不支撑 + 过时信息
- **正确信息**: PR #8241（"Add setting items to control items in status bar"）于 2024-03-22 **closed、未合并**（merged: false）。现行官方文档（2026-08）中状态栏设置已重构为 `status_bar.active_language_button`（默认 true）、`status_bar.cursor_position_button`（true）、`status_bar.line_endings_button`（false）与实验性 `status_bar.experimental.show`——原声称的 7 个设置名中 `show_status_bar`、`show_active_language`、`show_cursor_position`、`show_feedback_icon`、`status_bar.language_server_button`、`active_encoding_button` 均不存在于现行文档。
- **反证来源**: https://api.github.com/repos/zed-industries/zed/pulls/8241 → `"state": "closed", "merged": false`；https://zed.dev/docs/reference/all-settings → "Setting: status_bar Default: { "status_bar": { "active_language_button": true, "cursor_position_button": true, "line_endings_button": false } } ... "status_bar": { "experimental.show": false }"

## 无法验证项

- **macOS 统一标题栏的 "GPUI 全尺寸内容视图 / NSFullSizeContentViewWindowMask 式融合" 实现机制**（第 163 行，引 [23][21]）：[23] jsmestad/minga#707 仅佐证"统一标题栏"概念（"like Zed's unified title bar"），未涉及实现机制；PR #46641 正文也未描述该机制。
- **PR #46641 引入的项目下拉"因'设计与最近项目选择器差异过大'而回退收敛"的原因引语**：当前 main 代码确实已无 project_dropdown（workspace.rs/dock.rs 均 0 命中，回退事实成立），PR 本身 2026-01-13 已合入；但"设计差异过大"的引语出处未定位。后续事件（2026-02-12 issue "workspace: Improve recent projects picker for multi-project" cross-reference）佐证了"收敛到最近项目选择器"的方向。

## 核查通过项摘要

以下声称经外部验证全部正确（未单列错误条目）：
- 主题源码 [7] one.json（One Dark）全部色值：background/toolbar/tab.active_background `#282c33ff`、surface/panel/tab_bar `#2f343eff`、element `#2e343eff`、title_bar.inactive `#2e343eff`、text/icon `#dce0e5ff`、muted `#a9afbcff`、placeholder/disabled `#878a98ff`、accent `#74ade8ff`、editor.foreground `#acb2beff`、五档边框（`#464b57ff`/`#363c46ff`/`#47679eff`/`#293b5bff`/`#00000000`）、hover `#363c46ff`、active/selected `#454a56ff`、行号 `#4e5a5f`/当前行 `#d0d4da`、active_line `#2f343ebf`（75%）、wrap_guide `#c8ccd40d`、document_highlight.read `#74ade81a`、scrollbar thumb `#c8ccd44c`/轨道全透明、语义色 14 组主色（error/deleted `#d07277ff`、warning/conflict/modified `#dec184ff`、success/created `#a1c181ff`、info/renamed `#74ade8ff`、hint `#788ca6ff`、predictive `#5a6a87ff`、hidden/ignored `#878a98ff`，含 background/border 变体如 `error.background #d072771a`/`error.border #4c2b2cff`）、ANSI 16 色全表（red `#e06c75ff` ... bright_white `#fafafaff`，背景 `#282c34ff` 与编辑器 `#282c33ff` 差异属实）、players 8 组光标色全对（selection 为同色 24% 透明度）、syntax 46 键中除 selector（见错误 1）外全部正确（keyword/preproc `#b477cfff`、string/text.literal `#a1c181ff`、function/constructor/variant `#73ade9ff`、type/enum/operator/link_uri `#6eb4bfff`、constant `#dfc184ff`、comment `#5d636fff`、comment.doc/string.escape `#878e98ff`、property/variable.parameter/title `#d07277ff`、punctuation/variable/primary `#acb2beff`、bracket/delimiter `#b2b9c6ff`、link_text/predictive 斜体、emphasis.strong 700 字重）——一致。另"全文无 shadow 键"属实（grep 0 命中）。
- 官方文档部分：[5] all-settings（ui_font_size 16、ui_font_weight 400、buffer_font_size 15、buffer_font_weight 400、buffer_line_height "comfortable"、cursor_blink true、cursor_shape "bar"、centered_layout 左右 0.2）；[4] visual-customization（"comfortable" (1.618)、"standard" (1.3)、terminal line_height 默认 "standard"、".ZedSans" → IBM Plex、".ZedMono" → Lilex）；[2] appearance（动态主题默认 `{mode: system, light: One Light, dark: One Dark}`、icon_theme 暗色 "Zed (Default)"）；[36] tab-switcher（ctrl-tab、"sorted by recent usage"、按住 ctrl 保持/松键确认）；[37] project-panel（树视图、toggle 键）；[35] 命令面板唯一不符项为键位（见错误 3）；[6] theme-builder 博客（2026-02-12 发布、16 类 UI 色、10 类语法色、200+ token、Inspector 右键、color linking、Tree-sitter、CSS 自定义属性 + undo/redo + localStorage）——一致。
- 源码部分：animation.rs（缓动集 linear/quadratic/ease_in_out/ease_out_quint/bounce/pulsating_between，无独立 ease_out；pulsating_between "sine and cubic" 注释；reduce_motion 注释见错误 5）；blink_manager.rs（pause_blinking 500ms）+ editor.rs `CURSOR_BLINK_INTERVAL: Duration::from_millis(500)`（293 行，恒定不可配置）与 commit 18df6158（"terminal_view: Reuse editor's blink manager (#43351)"，2025-11-25）；style.rs（`corner_radii: Corners<AbsoluteLength>`、BoxShadow blur/spread 默认 0px）；workspace.rs/dock.rs/command_palette.rs 三文件 grep `with_animation` 均为 0 命中——一致。
- GitHub 状态部分：PR #31671 "editor: Smooth scroll" closed 未合并（2025-06-03），ConradIrwin 评论 "A core goal of Zed is speed; and currently this makes Zed feel much slower."（引语逐字一致）；PR #13596 "Move from Zed fonts to IBM Plex" merged 2024-06-27，团队评论 "saves us about 8Mb of space, and fixes some font fallback issues on Linux"（"省约 8MB、修复 Linux 字体回退"逐字一致），v0.143.0-pre 提交于 2024-07-03（晚于合入，"v0.143 起"版本归属成立）；issue #4991 光标动画 open；issue #4355 smooth scrolling open；PR #11137 closed 未合并（协作体验语境存在于讨论中）；PR #46641 merged 2026-01-13（"introduces a project dropdown" 逐字一致，回退见无法验证项）；issue #4629 标题即含 "in 0.94.3"（v0.94.3 版本归属成立）；issue #5460（"Restart to update Zed should be on the left side (with the project browser toggle, and language server/diagnostic information)" 支撑状态栏左右侧声称）；issue #7955 透明标题栏 closed 未实现；zed-fonts README（Zed Sans/Zed Mono 为 Iosevka 定制、quasi-proportional、现默认 IBM Plex/Lilex、别名 .ZedSans/.ZedMono）——全部一致。
- 第三方部分：[8] discussion #8499（JosephTLyons 2024-02-27 "Our UI is very intentionally minimal" 逐字一致；sketch/minimalism/engineering/retro/disappear 关键词出自社区成员评论）；[19] zed.tips（"language server indicator in the status bar (bottom-right)"、Restart/Stop/版本 hover 逐字一致）；[23] minga#707（"like Zed's unified title bar"）——一致。
- 交叉比对：zed.md 与同组其它三份文件无同一事实点冲突。

## 备注（未列为错误，供参考）

- "你的编辑器应该消失（Your editor should disappear）"并非官方团队在该讨论中的原话：讨论中该短语出自社区成员转述（"Zed team's concept of Your editor should disappear is correct"），官方原话仅为 "very intentionally minimal"。
- "syntax 约 40 键"：实际 46 键，"约"字偏差 15%，未列为错误。
- syntax 键 `title` 含显式 `font_weight: 400`（数值上与 normal 相同），与"除斜体/700 外全部无字重修饰"的表述存在字面上的微小出入，视觉上无差异。
- "style 分区约 250 键"若按全部叶值计数（278）则接近"约 250"，文档计数口径不明确（见错误 4 按顶层键口径处理）。
