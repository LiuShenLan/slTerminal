# warp 事实核查报告

> 核查日期: 2026-08-15
> 核查方式: WebSearch + WebFetch 外部验证 + 内部交叉比对
> 核查范围: 全量声称逐一核实
> 方向: terminal

---

## 错误 1: 默认字体 Hack 标注「官方未声明，待核」——官方文档已明确声明

- **文件+行号**: `docs/refer/warp.md` (2.2 字体排版, 第 72 行; 6 节「未核实项」清单, 第 225 行)
- **原声称**: 「默认字体为 Hack（第三方分析结论，官方未声明，**待核**）[26]」
- **错误类型**: 事实错误（来源不支撑的反面——声称官方未声明，实际官方文档明示）
- **正确信息**: 默认字体确为 Hack，且官方文档明确声明（Text, Fonts, & Cursor 页：「Warp's default font, Hack, doesn't yet have ligature support」），无需标注待核、也无需依赖第三方分析
- **反证来源**: https://docs.warp.dev/terminal/appearance/text-fonts-cursor/ — 「Enabling ligatures can reduce performance. **Warp's default font, Hack**, doesn't yet have ligature support. We recommend a font that supports ligatures (e.g. Fira Code)」

## 错误 2: 「内置主题未开源」断言不准确——官方主题仓库 warp_bundled/ 目录含 13 个内置主题 YAML

- **文件+行号**: `docs/refer/warp.md` (6 节「未核实项」清单, 第 224 行; 2.1 表 Warp Dark 行「官方无公开 YAML「待核」」, 第 46 行)
- **原声称**: 「Warp Dark（应用内置默认主题）官方 YAML 色值（**内置主题未开源**；本文给出的是官方预览图提取值）」
- **错误类型**: 来源不支撑
- **正确信息**: 官方主题仓库 warpdotdev/themes 的 `warp_bundled/` 目录公开了 13 个内置主题 YAML（cyber_wave、dark_city、dracula、fancy_dracula、gruvbox_dark、gruvbox_light、jellyfish、koi、leafy、marble、pink_city、red_rock、snowy）——其中 dracula.yaml 背景 `#282a36`（文档提取值 #292a35 系预览图采样误差，作者已标注「≈」）、gruvbox_dark.yaml 背景 `#282828`（与文档一致）。「内置主题未开源」作为整体断言不成立；仅 Warp Dark/Warp Light 及 Solarized/Willow Dream/Phenomenon/Solar Flare/Adeberry 等 8 个内置主题确实无公开 YAML（Warp Dark 行「待核」可保留，但应限定范围）
- **反证来源**: https://github.com/warpdotdev/themes/tree/main/warp_bundled — 目录含 dracula.yaml（`background: "#282a36"`）、gruvbox_dark.yaml（`background: "#282828"`）、cyber_wave.yaml（background 渐变 top `#002633`/bottom `#000000`）、dark_city.yaml（top `#0c252d`/bottom `#0c2c35`）等；README 明示「What are warp_bundled themes? These are the themes that ship directly with Warp.」

## 错误 3: 「滚动 1 万行日志实测约 120fps」在所引来源中无支撑

- **文件+行号**: `docs/refer/warp.md` (3 节动画清单「滚动」行, 第 95 行)
- **原声称**: 「滚动 1 万行日志实测约 120fps、输入延迟 5–6ms [20][21][24]」
- **错误类型**: 来源不支撑
- **正确信息**: 输入延迟 5–6ms 有 [24] lushbinary 支撑（对照表 Input Latency ~5ms）；但「120fps」在全部三个引用来源中均不存在：[20] how-warp-works 仅 60fps/144fps/400fps 表述，[21] 新闻稿正文仅「smoother typing, scrolling, and pane resizing」定性描述，[24] lushbinary 仅「sustains 60fps rendering during heavy output scrolling」（120 出现处为内存 120MB）。「约 120fps」无法定位到任何一手/二手来源
- **反证来源**: https://www.warp.dev/blog/how-warp-works — 「Warp should always be running at 60fps even on 4K or 8K monitors」「render at well over 144fps, even on a 4K monitor」; https://lushbinary.com/blog/warp-rust-gpu-architecture-technical-deep-dive/ — 指标表「Input Latency ~5ms」「Idle Memory ~120MB」、正文「sustains 60fps rendering during heavy output scrolling」

## 错误 4: 「137 个标准/暗色主题」与仓库实际不符（standard/ 目录 130 个 YAML）

- **文件+行号**: `docs/refer/warp.md` (5.1 节, 第 160 行; 6 节来源清单 [4], 第 197 行)
- **原声称**: 「官方仓库 `warpdotdev/themes` 提供 137 个标准/暗色主题」
- **错误类型**: 事实错误（数字不精确）
- **正确信息**: `standard/` 目录含 130 个主题 YAML（另有 README.md、previews 等非 YAML 项，`ls` 总条目恰为 137）；`base16/` 178 个、`warp_bundled/` 13 个。若「137」指 standard 目录全部条目（含 README/previews）则易误导为 137 个主题
- **反证来源**: https://github.com/warpdotdev/themes/tree/main/standard — `find standard -maxdepth 1 -name "*.yaml" | wc -l` = 130（2026-08-15 实测）

## 错误 5: 错误下划线「波浪」与官方「dashed」（虚线）不符

- **文件+行号**: `docs/refer/warp.md` (2.1 配色, 第 54 行)
- **原声称**: 「命令输入编辑器内的非法命令以**红色虚线波浪下划线**标注 [17]」
- **错误类型**: 事实错误（措辞不精确）
- **正确信息**: 官方原文为「a dashed red underline」（红色虚线），无「波浪」（wavy）语义
- **反证来源**: https://docs.warp.dev/terminal/editor/syntax-error-highlighting/ — 「Warp automatically underlines any invalid commands with a dashed red underline.」

## 核查通过项摘要

以下声称经外部验证全部正确（未单列错误条目）：
- **官方文档部分**：custom-themes 页 Solarized 示例逐字（background `#002b36`、foreground `#839496`、accent `#268bd2`、cursor `#95D886`、normal red `#dc322f`、green `#859900`）；YAML 字段全集（name/accent/cursor/background/foreground/details/terminal_colors，cursor 省略默认 accent，「Each color...must start with #」）；渐变子级（accent 的 left/right 或 top/bottom、background 渐变示例 `top: '#474747' / bottom: '#ffffff'` 逐字）；background_image（path 相对 themes 目录 + opacity 0–100，仅 .jpg/.jpeg/.JPEG）；三平台主题安装路径（macOS `$HOME/.warp`、Windows `$env:APPDATA\warp\Warp\data\`、Linux `${XDG_DATA_HOME:-$HOME/.local/share}/warp-terminal`）；standard vs base16 分类定义；内置 21 个主题清单逐一核对（Warp Dark…Adeberry，暗色 12 个）；「Sync with OS」亮/暗分别指定
- **主题仓库部分**：`standard/default_dark.yaml` 全 8 项色值逐字一致（background `#181818`、foreground `#d8d8d8`、accent `#7cafc2`、normal/bright 全表）
- **Blocks 部分**：block-basics（「Blocks group your command and command output」「Blocks grow from the bottom to the top」「non-zero exit code have a red background and red sidebar」逐字）；background-blocks（无关联命令、自动混排）；sticky-command-header（长输出钉顶、点击跳回块顶、UP/DOWN 最小化、git log 类全屏命令仅上滚后显示逐字）；block-actions（kebab/右键、网页 permalink 分享、书签指示器 + OPTION-UP/DOWN、块内查找、块内过滤）；blocks-behavior（dividers 默认开启、「horizontal lines...visual break」逐字、Compact mode 存在）
- **布局部分**：输入位置三模式（Start at the top Classic / Pin to the top Reverse / Pin to the bottom Warp mode）；tab bar（窗口模式默认可见、全屏隐藏、hover 唤出、Always/Only on hover/When windowed）；命令面板（macOS Cmd-P、Win/Linux Ctrl-Shift-P、workflows/prompts/notebooks/actions/sessions 过滤）；语法高亮分层（sub-commands/options/flags/arguments/variables 逐字）；Warp prompt context chips（cwd/git branch/svn/k8s/pyenv/date/time 逐字 + 右键 Edit prompt）；pane dimming（功能存在、三角指示器、Focus follows mouse）；透明度 1–100、macOS blur 半径滑块、Windows Acrylic
- **设计博客部分**：UI surface 机制逐字（暗色白色 overlay 对齐前景色、亮色黑色 overlay、background + overlay + outline）；accent 设计动机（tab indicator + block selection）；「Warp built entirely in Rust」「rendering directly on the GPU using Metal」「60fps even on 4K or 8K monitors」逐字；新闻稿「New GPU-accelerated rendering engine launches」2026-03-08 + 「smoother typing, scrolling, and pane resizing」逐字
- **DeepWiki 部分**：TuiAIBlock 分段（Input/RichText/ToolCall/Thinking/TodoList）与工具调用状态字形（`○` Dim / `●` Attention / `✓` Success / `×` Error / `■` Muted）逐字
- **GitHub 部分**：issue #15126（标题「Custom themes: add a key for link color (links are stuck on `foreground`)」、open、正文「Today there is no way to recolor links without recoloring something else」——链接恒取 foreground 属实）；warpdotdev/themes README（warp_bundled 定义、standard 源自 eendroroy/alacritty-theme、base16 源自 aarowill/base16-alacritty）
- **其它**：默认字体 Hack 的事实本身正确（见错误 1）；内置主题提取值与官方参照（Dracula 官方 `#282a36`、Solarized 官方 `#002b36`、Gruvbox `#282828` 核对一致，采样误差作者已标注）

## 备注（未列为错误，供参考）

- 「Warp Dark（默认）」：官方 themes 页将 Warp Dark 列于 21 个内置主题首位并称「By default, Warp ships with these themes」，但未以文字明示默认主题为 Warp Dark；该「默认」定位无法从官方文档直接证实
- 动画时长/缓动：官方全渠道未披露数值，文档全部标注「待核」，属实
- starlog 来源 [23]（Rect/Glyph/Image/Icon 四种图元、独立 hit-test）：页面正文为 JS 动态渲染，curl 无法抓取正文文本，未能验证
- aicoolies 来源 [25]：「fluid」「responsive」逐字确认；「分栏调整丝滑」为文档转述，页面原文未逐字对应
- 「Warp 主题由 20 个字段控制」：YAML 字段 6 个基础字段 + 16 ANSI 色 = 22 项（不含子级），「20」的口径未在官方来源中找到对应
- 内置主题背景提取值（Cyber Wave `#0b1b24`、Dark City `#16262f` 等）：为预览图采样近似，与 warp_bundled YAML 渐变端点（如 cyber_wave top `#002633`/bottom `#000000`、dark_city top `#0c252d`/bottom `#0c2c35`）存在偏差，作者已声明「程序化提取」方法，不构成错误
- 链接色声称引用 [27] 标注为「GitHub issue」——实际为 warpdotdev/warp 仓库 issue，URL 正确
