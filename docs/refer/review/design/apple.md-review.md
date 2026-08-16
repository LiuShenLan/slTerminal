# apple 事实核查报告

> 核查日期: 2026-08-15
> 核查方式: WebSearch + WebFetch 外部验证 + 内部交叉比对
> 核查范围: 全量声称逐一核实
> 方向: design

---

## 错误 1: 「macOS 定义 38 个动态系统色」应为 35 个

- **文件+行号**: `docs/refer/apple.md` (行 17、21–24 分类表)
- **原声称**: "macOS 定义 38 个动态系统色（semantic colors）"
- **错误类型**: 事实错误（兼内部矛盾）
- **正确信息**: 官方 HIG Color 页 macOS 动态系统色表格实际为 **35 个**（35 行数据 + 1 表头行）。文档自身第 21–24 行按四类列出的 API 恰好也是 35 项（17 前景 + 9 背景 + 5 控件 + 4 分隔/杂项），与「38 个」表述自相矛盾。
- **反证来源**: https://developer.apple.com/tutorials/data/design/human-interface-guidelines/color.json（2026-08-15 直抓）——`macOS` 小节表格 `rows` 共 36 行（含表头），数据行 35：Alternate selected control text color / Alternating content background colors / Control accent / Control background color / Control color / Control text color / Current control tint / Unavailable control text color / Find highlight color / Grid color / Header text color / Highlight color / Keyboard focus indicator color / Label color / Link color / Placeholder text color / Quaternary label color / Secondary label color / Selected content background color / Selected control color / Selected control text color / Selected menu item text color / Selected text background color / Selected text color / Separator color / Shadow color / Tertiary label color / Text background color / Text color / Under page background color / Unemphasized selected content background color / Unemphasized selected text background color / Unemphasized selected text color / Window background color / Window frame text color。官方页面全文无「38」与动态色数量的组合表述（"macOS defines the following dynamic system colors" 后即为该表）。

## 错误 2: 「NSFont.titleBarFont(ofSize:) 等 11 个」应为 12 个

- **文件+行号**: `docs/refer/apple.md` (行 136)
- **原声称**: "macOS 控件级字号（title bar/toolbar/sidebar/menu）官方只给 API（`NSFont.titleBarFont(ofSize:)` 等 11 个）不给 pt 数值"
- **错误类型**: 事实错误
- **正确信息**: 官方 HIG Typography 页 macOS 小节的 dynamic system font variants 表列出 **12 个** API：controlContentFont、labelFont、menuFont、menuBarFont、messageFont、paletteFont、titleBarFont、toolTipsFont、userFont、userFixedPitchFont、boldSystemFont、systemFont。
- **反证来源**: https://developer.apple.com/tutorials/data/design/human-interface-guidelines/typography.json（2026-08-15 直抓）——`macOS` 小节表格 rows 为 12 行数据：「Control content」「Label」「Menu」「Menu bar」「Message」「Palette」「Title」「Tool tips」「Document text (user)」「Monospaced document text (user fixed pitch)」「Bold system font」「System font」。

## 错误 3: svrnty_design_system 页面「无任何 Apple HIG 声明」与实际内容相反

- **文件+行号**: `docs/refer/apple.md` (行 166)
- **原声称**: "「150/250/350/500ms 时长 token」为第三方设计系统惯例，非 Apple 官方（svrnty_design_system 源码 `fast: 150ms / normal: 250ms / slow: 350ms / verySlow: 500ms`，页面无任何 Apple HIG 声明）"
- **错误类型**: 事实错误（来源内容与描述相反；「非 Apple 官方」的核心结论仍成立）
- **正确信息**: 该页面对 `standardValues` 常量的文档字符串明确声明基于 Apple HIG：「Standard animation duration values following Apple HIG. These values are based on Apple's motion guidelines and provide a good foundation for all brand themes.」。文档中「页面无任何 Apple HIG 声明」的描述与页面实际内容相反（Apple HIG Motion 页本身确实无这些数值，故「非官方数值」结论不变，但该句的支撑性描述错误）。
- **反证来源**: https://pub.dev/documentation/svrnty_design_system/latest/svrnty_design_system/RequiredAnimationTokens/standardValues-constant.html（2026-08-15 抓取）——页面正文：「Standard animation duration values following Apple HIG. These values are based on Apple's motion guidelines…」；token 值 `'fast': Duration(milliseconds: 150), 'normal': Duration(milliseconds: 250), 'slow': Duration(milliseconds: 350), 'verySlow': Duration(milliseconds: 500)` 与文档一致。

## 错误 4: colorarchive 笔记中「OKLCH」「prefers-reduced-motion 瞬切」两项不存在于来源

- **文件+行号**: `docs/refer/apple.md` (行 174)
- **原声称**: "主题切换最佳 150–200ms 全表面 crossfade（>300ms 感觉像页面加载）、感知均匀色彩插值（OKLCH/HSL）避免浑浊中间色、`prefers-reduced-motion` 时直接瞬切 [44]"
- **错误类型**: 来源不支撑
- **正确信息**: 该文（2026-07-09，Issue 034）实际表述为：「Theme transitions (light to dark) work best as fast, cross-fade transitions — ideally 200ms or under.」「At 150–200ms, this reads as a lighting shift. Longer than 300ms and it starts to feel like a page loading.」「transition in a perceptually uniform space — either HSL with hue locked and lightness transitioning, or a CSS color-mix() call with explicit interpolation settings」（无 OKLCH）；全文无 `prefers-reduced-motion` 相关表述（「瞬切」声称无出处）。文中还含 150–200ms crossfade、>300ms 页面加载感等正确部分，仅 OKLCH 与 prefers-reduced-motion 两点不支撑。
- **反证来源**: https://colorarchive.org/notes/july-2026-color-and-motion/（2026-08-15 抓取全文）——正文见上；全文检索 "OKLCH"、'reduced' 均 0 命中。

## 错误 5: Dracula 主题 string 色值 ≈#F1FA8C 与文件实际不符

- **文件+行号**: `docs/refer/apple.md` (行 231)
- **原声称**: "string `≈#F1FA8C`"
- **错误类型**: 事实错误
- **正确信息**: Dracula.xccolortheme（master）中 `xcode.syntax.string` = `0.956863 0.976471 0.615686 1` ≈ **#F4F99D**（R 244 / G 249 / B 157），并非 #F1FA8C（#F1FA8C 为 Dracula 官方调色板 string 色，该文件内实际值不同）。
- **反证来源**: https://raw.githubusercontent.com/dracula/xcode/master/Dracula.xccolortheme（2026-08-15 抓取）——`<key>xcode.syntax.string</key><string>0.956863 0.976471 0.615686 1</string>`（其余示例值 keyword `1 0.47451 0.776471 1`=#FF79C6、comment `0.384314 0.447059 0.643137 1`=#6272A4、plain `0.972549 0.972549 0.94902 1`=#F8F8F2、DVTSourceTextBackground `0.117658 0.122153 0.159778 1`=#1E1F29、DVTConsoleTextBackgroundColor `0.156863 0.164706 0.211765 1`=#282A36 均与文档一致）。

## 错误 6: stackoverflow 12025622 并无「两答数值不一致（0.35s / 0.505s）」

- **文件+行号**: `docs/refer/apple.md` (行 165)
- **原声称**: "UIKit 系统过渡时长 ≈ 0.35s：社区经 `transitionCoordinator.transitionDuration` 实测（iOS 7+，被描述为防 Apple 改值的 future-proof 做法）；iOS 7 前实测 0.35s、iOS 7 后变 0.505s（两答数值不一致）——…[41]"
- **错误类型**: 来源不支撑（数字归因错误）
- **正确信息**: https://stackoverflow.com/questions/12025622（StackExchange API 2026-08-15 核查）只有 **1 个答案**（score 4，2012-08-18）：「those durations do vary depending on the platform… A very common duration for many system animations seems to be around 0.35-0.40 seconds. Shorter animations often are exactly half that time. Longer animations often are exactly twice that time.」——无 0.35s 精确值、无 transitionDuration 实测、无「future-proof」描述、无第二个答案、全文无 0.505s。0.505s 数值在该帖不存在。
- **反证来源**: https://api.stackexchange.com/2.3/questions/12025622/answers?site=stackoverflow&filter=withbody（仅 1 条答案，正文见上；comments 4 条亦无 0.505）。

## 错误 7: photon issue #139 内容与「menupopup 圆角实测 6px」完全无关

- **文件+行号**: `docs/refer/apple.md` (行 146、来源表 [26])
- **原声称**: "Mozilla Photon：macOS menupopup 圆角实测 6px（仅搜索摘要，**待核**）[26]"，来源表 [26] 标注该 URL 为「menupopup 圆角实测 6px（反爬拦截，仅搜索摘要，待核）」
- **错误类型**: 来源不支撑（URL 存在但内容完全不涉及声称主题）
- **正确信息**: https://github.com/FirefoxUX/photon/issues/139 实际为 **「Doorhanger component: Understand」**（closed）：「Collect and compare doorhangers variations. Create a visual inventory and find possible edge cases.」——与 macOS menupopup 圆角无任何关系，6px 数值在该 issue 中不存在。若 6px 数值真实存在，其正确出处需要另行定位。
- **反证来源**: https://api.github.com/repos/FirefoxUX/photon/issues/139（2026-08-15 获取）：title "Doorhanger component: Understand"、state "closed"、body "Collect and compare doorhangers variations. Create a visual inventory and find possible edge cases."

## 错误 8: NSVisualEffectView 公开属性「仅 5 个」遗漏 interiorBackgroundStyle

- **文件+行号**: `docs/refer/apple.md` (行 103)
- **原声称**: "NSVisualEffectView 公开属性仅 material/blendingMode/state/isEmphasized/maskImage，模糊半径与着色由系统内部决定，无公开 API/数值"
- **错误类型**: 事实错误（遗漏）
- **正确信息**: 官方文档「Specifying the Effect Appearance」主题区除 material、blendingMode、state（State 枚举）、isEmphasized 外，还公开 **`interiorBackgroundStyle`**（只读属性，供内容视图按材质背景绘制）。「模糊半径/着色无公开数值」的结论不受影响。
- **反证来源**: https://developer.apple.com/tutorials/data/documentation/appkit/nsvisualeffectview.json（2026-08-15 直抓）——topicSections「Specifying the Effect Appearance」identifiers 含 `doc://com.apple.appkit/documentation/AppKit/NSVisualEffectView/interiorBackgroundStyle`，与 blendingMode/BlendingMode/isEmphasized 并列。

## 核查通过项摘要

以下声称经外部验证全部正确（未单列错误条目）：

- **官方 HIG 引言类（developer.apple.com JSON 直抓逐字核对）**：「Dark Mode is a systemwide appearance setting that uses a dark color palette…」（dark-mode.json）；「these colors aren't necessarily inversions of their light counterparts: while many colors are inverted, some are not」；「Avoid hard-coding system color values… Documented color values are for your reference…」；「Avoid offering an app-specific appearance setting.」；desktop tinting 定义（graphite → 窗口背景取桌面色）；visionOS/watchOS 不支持 Dark Mode；「macOS provides several standard materials with designated purposes, and vibrant versions of all system colors」；「Choose materials and effects based on semantic meaning and recommended usage. Avoid selecting a material or effect based on the apparent color it imparts…」；「SF Pro is the system font in macOS. NY is available for Mac apps built with Mac Catalyst. macOS doesn't support Dynamic Type.」；「prefer Regular, Medium, Semibold, or Bold font weights, and avoid Ultralight, Thin, and Light font weights…」；「If you need to display three or more lines of text, avoid tight leading」；「Add motion purposefully」「Make motion optional」「Aim for brevity and precision in feedback animations」；「Regardless of rendering mode, using system-provided colors ensures that symbols automatically adapt… Dark Mode.」
- **官方数值表（JSON 逐字比对）**：iOS dark 12 色 + 6 灰 + increased contrast 变体全部 36 个值（swatch alt RGB：red dark R-255,G-66,B-69=#FF4245、blue #0091FF、teal #00D2E0、mint #00DAC3、orange #FF9230、indigo #6D7CFF 等；contrast 列 #FF6165/#5CB8FF/#3BDDEC/#6DD9FF/#A7AAFF/#EA8DFF/#FF8AC4/#DBA679/#AEAEB2/#7C7C80/#545456/#444446/#363638/#242426 等）——与文档表格逐字一致；macOS built-in text styles 11 行全表（Large Title 26/32…Caption 2 Medium 10/13）；SF Pro tracking 表（6pt +41、12pt 0、17pt −26、20pt −23、28pt +14、48pt +8、96pt 0）；macOS 默认 13pt/最小 10pt（iOS 17/11）；color.json changelog「June 9, 2025 — Updated system color values」（支撑 2025-06-09 更新与 blue #0A84FF→#0091FF 说法）；motion changelog「2025-09-09 — Added guidance for Liquid Glass」；motion 全文零毫秒数值；HIG 独立 animation 章节 JSON 接口 404（壳页不存在）。
- **API 默认值（官方文档 JSON）**：SwiftUI spring(response:dampingFraction:blendDuration:) = 0.5/0.825/0；interactiveSpring = 0.15/0.86/0.25；spring(duration:bounce:blendDuration:) = 0.5/0.0/0；response「approximate duration in seconds… infinitely-stiff」、dampingFraction「fraction of…critical damping」、bounce「0…critically damped…1.0 undamped…-1.0 overdamped」等语义；Material 枚举 19 case 全名；blendingMode「Sheets and popovers use behind-window blending」「Toolbars always use in-window blending」；maskImage「the mask is applied in an appropriate way to the window's shadow」；followsWindowActiveState 语义。
- **macos/AppKit 实现级**：macios Enums.cs raw 值 0–22 全表（含 HeaderView=10/Sheet=11/FullScreenUI=15/ToolTip=17/UnderWindowBackground=21/UnderPageBackground=22）与「Use a semantic material instead」Advice；macOS 动态色表格 35 行的 API 名（含 disabledControlTextColor 官方用法）；window-vibrancy `radius: Option<f64>` 与 Liquid Glass `radius(26.0)` 示例（v0.8.0 源码+README）；objc2-app-kit 0.3.1 NSVisualEffectView 默认值。
- **第三方色值/仓库（raw 抓取逐字节比对）**：wezterm scheme_data.rs「Apple System Colors」background #1e1e1e、foreground/cursor #ffffff/#98989d、selection #3f638b、brights #0a84ff/#ff453a/#32d74b/#ffd60a/#bf5af2、ansi 首色 #1a1a1a；iTerm2-Color-Schemes commit d58e107 存在且 /schemes/Apple System Colors.itermcolors（sRGB 浮点）16 项色值与文档一致；sfthemes macos_dark_cols 11 色（#0a84ff/#98989d/#32d74b/#ff9f0a/#ff375f/#bf5af2/#ff453a/#ffd60a）+ 背景 #262626；TiddlyWiki CupertinoDark.tid（2f63abc1）background #282828、foreground #FFFFFF、external-link #32D74B、description「A macOS inspired dark palette」；colorpickercode 页面 Background #1e1e1e / Surface #2c2c2c / Accent #007aff（含「accent 混用亮色值」标注的合理性）；graphicdesign.se 18689 高分回答「corner radius seems to be 7px」（2013）；NSWindowStyles README `visualEffect.layer?.cornerRadius = 16.0`；stackoverflow 19940019 被采纳回答 cornerRadius 10.0 + NSShadow opacity 0.5/radius 5.0；three-philosophers「SF Pro Text…up to 20pt / SF Pro Display…20pt and above」；wwdcnotes WWDC22-110381「three new width styles: Condensed, Compressed, and Expanded」；yell0wsuit Apple-Fonts-Documentation 静态九字重 100–900、变量 weight 1–1000、optical 17–28；jasonm23/xcode-themes 62 个主题文件（含 Gruvbox/Monokai/Tomorrow Night，支撑「30+」）；MateoCerquetella/xcode-theme Default (Dark)/Classic (Dark)/Civic (Dark)/Midnight (Dark) 与 Midnight 编辑器 #10131A、侧栏 #0A0D14（5.2.0，2026-05-12），Default Dark 主题 JSON 全部界面/语法值（#1F1F24/#dfdfe0/#26282b/#9a9c9d/#1c1f21/#383a3d/#646f8366/#ffffff/#A0D07D/#FC6A5D/#D0BF69/#FF7AB2/#FFA14F/#6699FF/#5DD8FF/#E5CFFF/#A167E6）；Dracula.xccolortheme v1.2.5 plist/XML 结构、DVTFontAndColorVersion=1、r g b a 四浮点格式、SFMono-Regular - 14.0 字体格式、核心 token 键名全集、无 sourceText* 旧键；macos_ui 2.2.0 hueComponentToAccentColor 8 色映射（blue/purple/pink/red/orange/yellow/green/graphite）；fusengine app-icons.md iOS 26 icon-light/dark/tinted 与 #313131→#141414、AccentColor.colorset；danielmartin gist DVTUseTheme/IDEExtensionDebuggingHost/.dvttheme 机制；Xcode 10 release notes「Support for varying image and color assets by Light, Dark, and High Contrast appearances on macOS 10.14」；Xcode 13 release notes「Xcode 13 introduces Vim key bindings」「changing the line wrapping preference on a per-editor basis」；gist alemmar11（alemar11）CGS 材质表（appearanceBased 暗色→MacUltraDark、SelectionDark 变体、2015 Yosemite 语境）；NetNewsWire DarkMode.md（WWDC18 笔记：侧栏图标不 vibrant、vibrancy 上忌透明度、禁非语义材质、desktop-tinted 三材质）；hig-doctor motion.md/dark-mode.md 快照 2025-02-02 与零数值；svrnty token 值 150/250/350/500ms 本身；「38 动态色」外的四分类 API 名列表（35 项全部与官方表格一致）；SF Symbols「nine symbol weights」「three scales: small, medium (the default)」与 SF Symbols 7 Draw On/Draw Off 渐变（changelog 2025-07-28 + WWDC25 337）。

## 备注（未列为错误，供参考）

- 行 103「模糊半径与着色由系统内部决定，无公开 API/数值」结论正确，但「须实测截图反推」的表述为合理推断（无来源声明，未列为错误）。
- 行 232「旧键名 sourceTextBackgroundColor 属旧版 .dvtcolortheme」与「本次抓取的两个仓库文件均无 sourceText* 键」——Dracula.xccolortheme 中确实无该键；「两个仓库文件」的另一仓库文件未指实，无法复核。
- 行 47/231 colorpickercode 的「四色板」角色标签（Background/Surface/Accent）在页面技术表中逐字存在，色条区无标签，文档转述可接受。
- 无法验证项（文档已自标「待核」，验证受阻原因：Cloudflare 拦截）：[25] Apple 开发者论坛 765410（Security Verification 页）、[45] apple.stackexchange revision c4843ec5（Cloudflare）、[55] git.thauvin.net warp-themes apple_dark.yaml（Cloudflare）；[14] seedflip.co 仅搜索摘要（未抓取原文）。
