# 暗黑 UI 设计调研总览

> 调研日期：2026-08-15。本文件是对 `docs/refer/` 下 11 份软件调研文档的横向对比与共性提炼。**纯外部对比，不含任何特定项目的映射或建议。**
> 各文档结论均内联来源编号；本总览为跨文档归纳，具体数值的出处与「待核」状态以各分文档为准。

## 1. 文档索引

| 文档 | 软件 | 类别 | 数据可靠性 |
|------|------|------|-----------|
| [apple.md](apple.md) | macOS 系统 / Xcode | 系统设计规范 | 官方 HIG 原则为主，具体色值/动效数值官方不公开 |
| [jetbrains.md](jetbrains.md) | IntelliJ 平台（Darcula / New UI） | 开发者工具 | 主题 JSON 源码级，动效参数源码级 |
| [windows-terminal.md](windows-terminal.md) | Windows Terminal | 终端 | defaults.json / XAML 源码级 |
| [warp.md](warp.md) | Warp | 终端 | 官方文档 + 主题仓库；动效参数未披露 |
| [terminals-other.md](terminals-other.md) | Ghostty / Tabby / iTerm2 / Alacritty | 终端 | 各官方源码/文档级 |
| [vscode.md](vscode.md) | VS Code | 开发者工具 | 主题 JSON + 颜色注册源码级 |
| [zed.md](zed.md) | Zed | 开发者工具 | 主题 JSON + GPUI 源码级 |
| [devtools-other.md](devtools-other.md) | Sublime Text / Obsidian | 开发者工具 | 官方文档 + 社区交叉值 |
| [linear.md](linear.md) | Linear | 设计标杆 | 官方博客 + 社区逆向 token |
| [raycast-arc.md](raycast-arc.md) | Raycast / Arc | 设计标杆 | 官方渠道为主，UI 色值/动效未公开 |
| [notion-figma.md](notion-figma.md) | Notion / Figma | 设计标杆 | 官方博客（Figma token 体系）+ 第三方实测 |

## 2. 跨软件共性设计模式

### 2.1 暗色底色与「内容区最暗」分层

现代暗黑 UI 普遍**放弃纯黑**，但把明度最低层留给内容区，UI 壳层逐级提亮：

| 流派 | 代表 | 底色示例 |
|------|------|----------|
| 近纯黑内容区 | Warp Dark `#000000`–`#070707`、Notion `#191919`、Linear `#08090A`、Windows Terminal 内容层 `#0C0C0C` | 终端/文档内容区 |
| 深灰蓝 | JetBrains 编辑器 `#2B2B2B`、Ghostty `#292c33`、Zed 编辑器 `#282c33`、Sublime Mariana `#343D46` | 缓解纯黑刺眼对比 |
| 双色阶 UI/内容分离 | Windows Terminal（UI `#2e2e2e` / 内容 `#0C0C0C`）、VS Code（workbench `#252526` / editor `#1E1E1E`） | 工具类最典型 |

层级规律：**背景明度阶梯 3–4 档**是普适结构（如 VS Code L0–L3、Zed 3 档、Obsidian base-00→100 十二档、JetBrains 八级灰阶）。

### 2.2 层级表达三流派（暗色下阴影失效的三种替代）

暗色下投影几乎不可见，各产品用三种机制表达层级（可叠加）：

1. **明度阶梯**：最普适。VS Code、Zed、Linear、Raycast 的表面阶梯均靠背景亮度差。
2. **半透明白描边/发丝线**：Linear `rgba(255,255,255,0.05)–0.08`、Notion hairline border、Zed 1px border 体系、VS Code 列表缩进线。
3. **材质/玻璃/overlay**：Apple vibrancy、Windows Mica/Acrylic、Arc backdrop-blur + 渐变、Warp「UI surface = 主题背景 + 白色 overlay + outline 描边」（唯一有官方公式化描述的方案）。

### 2.3 强调色纪律：单点 + 极低占比

- **全 UI 单一强调色**是共同纪律：VS Code 蓝 `#007ACC`/`#0078D4`、Zed 蓝 `#74ade8`、Linear 紫 `#5E6AD2`（占界面像素 <5%）、Raycast 红 `#FF6363`、Notion 蓝 `#2EAADC`、JetBrains 低饱和蓝系 `#4A88C7`/`#589DF6`。
- 强调色只用于：焦点环、选中态、激活指示、CTA、徽标。
- 语义状态色四件套（红/黄/绿/蓝）普遍带**透明背景变体**（Zed `error.background/border` 两级、VS Code 输入校验背景 `#5A1D1D` 等）。

### 2.4 动效克制与参数带

**「官方不公开动画时长/缓动」是行业常态**——Apple HIG、Warp、Raycast、Arc、Figma、Notion 均只给原则不给数值；只有开源产品可拿到源码级参数。已核实的参数带：

| 参数带 | 数值 | 出处 |
|--------|------|------|
| 状态变更默认 | 180ms `cubic-bezier(0.22,1,0.36,1)` | Linear（签名曲线） |
| 微交互反馈 | 100–150ms ease-out | Linear / 社区惯例 |
| 面板/窗格滑入 | 200–300ms | Windows Terminal pane 200ms、JetBrains 工具窗口 300ms、Tabby 页签 250ms |
| 主题切换 crossfade | 150–200ms | 第三方 Web 惯例（>300ms 像页面加载） |
| 光标闪烁 | 500ms（方波） | Zed 唯一常驻动画 |
| 上限原则 | UI 动画 <300ms、键盘操作零动画、只动 transform/opacity | Linear / Emil Kowalski |

共性规则：**高频交互不动画；键盘路径零动画；动画可中断；全局减动效开关**（Apple Reduce Motion、VS Code `monaco-enable-motion`、Tabby `accessibility.animations`、Windows Terminal 双门控、Zed `reduce_motion`）。

两个极端案例：Zed 以速度为纲几乎无 UI 动画（平滑滚动 PR 被官方否决）；Alacritty 明确拒绝任何动画。

### 2.5 字体与排版

- **Inter 统治现代开发者工具 UI**：Linear、Raycast、Figma、Notion（NotionInter）、JetBrains UI 均用 Inter 或变体；品牌差异靠 OpenType 特性（Linear `cv01`/`ss03`、Raycast `ss03`）。
- 等宽终端字体：Cascadia（Windows Terminal）、JetBrains Mono（Ghostty 默认）、Hack（Warp 默认，官方文档明示）。
- UI 字号体系：侧栏/面板标题 11px 全大写（VS Code）、正文 13–16px、状态栏 12px；编辑器 12–16px。
- 字重克制：Linear 用 510/590 回避 700；Apple 建议暗色下避免细字重。
- 对比度硬指标：Apple 4.5:1（正文）/7:1（力求）、Tabby `minimumContrastRatio` 4、Warp「Enforce minimum contrast」默认开启、iTerm2 Minimum Contrast 推黑白。

### 2.6 主题 token 工程化

- **语义化 token 是主流**：Apple 38 动态系统色、VS Code `--vscode-*` 全量 CSS 变量、Figma 约 350 语义 token（5 级命名 schema）、Zed 200+ token、Obsidian 400+ CSS 变量。
- **感知均匀色彩空间迁移**：Linear 2024 起 LCH、Obsidian 1.13 起 OKLCH——主题切换/派生色避免浑浊中间色。
- 明暗双模式绑定：Figma collection modes、Ghostty `theme = light:X,dark:Y`、iTerm2 `xxx_color_light/dark`、Windows Terminal `"theme": {"dark": ..., "light": ...}`。
- 反例：Notion 无 CSS 变量、颜色内联注入（第三方主题须逐值匹配覆盖）——被普遍视为反面教材。

### 2.7 终端类特有模式

- **UI 层让位于内容层**：Windows Terminal「seamless」页签背景取 `terminalBackground`；Warp 全 UI 由主题单点 `accent` 推导。
- 终端内容区 ANSI 16 色与 UI 强调色**双轨并行**（Warp 主题 20 字段、Zed 终端独立 16 色组）。
- 页签栏极简：Windows Terminal 合入标题栏、Warp hover 才显示、Ghostty 仅多页签时显示。
- 状态栏两极：Windows Terminal/Alacritty 无状态栏；iTerm2 组件化状态栏（20+ 内置组件）。

## 3. 横向对比表

### 3.1 底色与层级

| 产品 | 内容区底色 | UI 壳层 | 层级机制 | 强调色 |
|------|-----------|---------|----------|--------|
| VS Code（Dark Modern） | `#1F1F1F` | `#181818` 系统一 | 明度阶梯 | `#0078D4` |
| VS Code（Dark+） | `#1E1E1E` | `#252526`→`#333333`→`#3C3C3C` | 明度阶梯 | `#007ACC` |
| JetBrains Darcula | `#2B2B2B` | `#3C3F41`→`#45494A` | 明度阶梯 + 弱阴影 | `#4A88C7` |
| Zed（One Dark） | `#282c33` | `#2f343e`→`#3b414d` | 明度阶梯 + 1px 边框 | `#74ade8` |
| Linear | `#08090A` | `#0F1011`→`#28282C` | 阶梯 + 半透明白边框 | `#5E6AD2` |
| Notion | `#191919` | `#202020`→`#252525` | 阶梯 + 发丝线 | `#2EAADC` |
| Raycast | `#070A0B`（品牌库） | `#0d0d0d`→`#121212` | 阶梯（+macOS 材质） | `#FF6363` |
| Windows Terminal | `#0C0C0C`（Campbell） | `#2e2e2e`（Mica 可开） | 双色阶 + 材质 | 系统 accent |
| Warp | 主题 background（默认近纯黑） | background + 白 overlay + outline | overlay 公式 | 主题 accent |
| Ghostty | `#292c33` | 系统原生控件 | 系统原生 | — |
| Obsidian | `#1c1c1c` | base-05→100 十二档 | CSS 变量阶梯 | HSL 258/88%/66% |

### 3.2 已核实动效参数（源码级）

| 产品 | 动画 | 时长 | 缓动/机制 | 开关 |
|------|------|------|-----------|------|
| Linear | 状态变更 | 180ms | `cubic-bezier(0.22,1,0.36,1)` | 键盘零动画 |
| Linear | 微交互 | 100ms | ease-out | — |
| Windows Terminal | pane 进入/退出 | 200ms | QuadraticEase | OS+app 双门控 |
| JetBrains | 工具窗口滑入 | 300ms | 指数逼近 ease-out | registry **默认关** |
| Tabby | 页签进出 | 250ms | ease-out / ease-in-out | `accessibility.animations` |
| Tabby | 页签宽度/聚焦 | 125ms | ease-out | 同上 |
| VS Code | 状态栏背景 | 150ms | ease-out | `monaco-enable-motion` |
| Zed | 光标闪烁 | 500ms | 方波（唯一常驻） | `cursor_blink` |
| Apple | SwiftUI spring 默认 | 500ms | response 0.5 / damping 0.825 | Reduce Motion |
| Notion（iOS 实测） | 块动画 / 侧栏抽屉 | 200ms / 280ms | easeOut / easeInOut | Reduce motion |

### 3.3 圆角与字体

| 产品 | 圆角体系 | UI 字体 |
|------|----------|---------|
| JetBrains | 5–10px（工具型小圆角） | Inter 13 |
| Linear | 2/4/6/8/12 + pill 9999px（上限 16px） | Inter Variable（510 字重） |
| VS Code | 0（HC 强制 0） | 系统字体 |
| Zed | 逐组件硬编码小圆角，无 token | IBM Plex Sans |
| Notion | ≤6px（Modal 8px） | NotionInter |
| Obsidian | 4/8/12/16px | 系统字体（原 Inter） |
| Raycast | 6–16px + pill | Inter + GeistMono |
| Sublime | 直角（可选圆角页签） | 系统字体 |

## 4. 设计趋势（2024–2026）

1. **暗色更暗、更暖/去饱和**：VS Code 1.91 换 Dark Modern `#181818` 系；Linear 2026 刷新由冷蓝灰转暖灰；Notion 2022 改近纯黑；Arc 2024 官方调暗暗黑主题。
2. **感知均匀色彩空间**：Linear LCH、Obsidian OKLCH——主题派生与明暗切换避免浑浊中间色。
3. **材质/玻璃回归**：macOS 26 Liquid Glass、Raycast 2026 重写拥抱 Liquid Glass、Windows Mica 成为工具类默认底色选项。
4. **动效制度化克制**：<300ms 原则、键盘零动画、全局 reduce motion 开关成为标配；速度成为卖点（Zed、Alacritty 以"无动画"为荣）。
5. **语义 token 工程化**：Figma 350 token 迁移、Zed Theme Builder（可视化 + Inspector + 颜色联动）、Linear 98 变量精简到 3 个——主题系统从"色值清单"走向"可推导体系"。
6. **界面消失论**：Zed「editor should disappear」、Arc 侧栏三合一、Windows Terminal seamless、Warp 全 UI 由 accent 单点推导——chrome 层最小化，内容区最大化的共同方向。

## 5. 方法论附注

- 调研中「官方只给原则、零数值」的软件：Apple HIG（动效/系统色）、Warp（动效）、Raycast（动效/应用内色值）、Arc（UI 色值/动效）、Figma（面板色/动效）、Notion（桌面端动效）。对应数值只能依赖源码（开源产品）或第三方逆向（标注「待核」）。
- 开源产品（VS Code、JetBrains、Zed、Tabby、Windows Terminal、Ghostty、Alacritty、iTerm2）的色值与参数均为源码级一手数据；闭源产品的第三方逆向值已按"双源互证"原则处理，冲突口径并存标注。
- 所有「待核」条目在各分文档文末有集中清单，引用前应逐条回查来源。
