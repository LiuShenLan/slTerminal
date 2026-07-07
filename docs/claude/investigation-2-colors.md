# slTerminal 颜色显示异常 —— 深度调查

## 问题概述

Claude Code CLI 基于 Ink (React-for-terminal) 渲染引擎，硬编码输出 24-bit RGB SGR 序列（`ESC[38;2;R;G;Bm` / `ESC[48;2;R;G;Bm`）。当前 slTerminal 中存在颜色显示异常（高亮丢失、色偏）。

## 调查结论速览

| # | 调查项 | 严重级别 | 当前状态 | 需修复 |
|---|--------|:--------:|----------|:------:|
| 1 | COLORTERM 未设置 | **高** | spawn 不注入，子进程看不到 truecolor 能力 | **是** |
| 2 | TERM 环境变量 | **高** | 继承自父进程，无显式设置 | **是** |
| 3 | ConPTY 颜色缩窄 | 中 | 修复后 Win10/11 原样透传 24-bit；旧版可能降级 | 检查 |
| 4 | xterm.js ANSI 调色板与 True Color | **低** | theme.ts 的 16 色调色板**不影响** True Color 渲染 | 否 |
| 5 | drawBoldTextInBrightColors | 中 | 默认 true，但只影响 ANSI 16 色（0-7->8-15）；不影响 True Color | 考虑显式 |
| 6 | 256 色退化 | **高** | 缺 COLORTERM 时 Chalk 降级到 Level 1（16 色） | **是** |

---

## 1. COLORTERM 环境变量

### 1.1 问题分析

**当前 slTerminal 不设置 `COLORTERM`**。`pty_spawn` 中 `CommandBuilder` 只设置 `cwd`，不注入任何环境变量。子进程（pwsh/powershell）完全继承 slTerminal 进程的环境变量。

### 1.2 对 Claude Code 的影响

Claude Code 的渲染栈依赖 Chalk（通过 Ink）检测终端颜色能力：

```
Chalk level 检测优先级（从高到低）：

1. --no-color / --color CLI flag
2. FORCE_COLOR 环境变量（3=truecolor）
3. COLORTERM=truecolor 或 24bit -> Level 3 (24-bit TrueColor)
4. TERM=xterm-kitty / wezterm / alacritty 等已知终端 -> Level 3
5. TERM_PROGRAM (iTerm, Apple_Terminal) -> Level 2-3
6. TERM 后缀 -256color -> Level 2 (256 色)
7. Windows build >= 14931 -> Level 1 (16 色)
8. CI 环境 -> Level 1 或 3
9. 非 TTY -> Level 0
```

**关键事实**：Chalk 的 `supports-color` 在 Windows 上若没检测到 `COLORTERM`，不会自动判定 Level 3。Windows build 号检测只给 Level 1（基本色）。这意味着 Claude Code 发出的 hex/RGB 颜色会被量化到最近的 16 色调色板值，产生**明显色偏**。

### 1.3 主流终端做法

| 终端 | COLORTERM 值 |
|------|-------------|
| Windows Terminal (ConPTY) | `truecolor`（自动注入） |
| Alacritty | `truecolor`（硬编码于 spawn） |
| Kitty | `truecolor`（硬编码） |
| WezTerm | `truecolor`（term 选项控制） |
| iTerm2 | `truecolor`（v3+） |
| VTE (GNOME Terminal) | `truecolor` |
| Konsole | `truecolor` |

**无一例外，所有现代终端模拟器都设置 `COLORTERM=truecolor`。**

### 1.4 修复方案

在 `pty_spawn` 中，`cmd.cwd()` 之后、`pair.slave.spawn_command(cmd)` 之前添加：

```rust
cmd.env("COLORTERM", "truecolor");
cmd.env("TERM", "xterm-256color");
```

`portable-pty` 的 `CommandBuilder` 已提供完整的 `env()` API。内部实现为 push 追加模式，最后写入的同名 key 胜出。

### 1.5 来源

- [termstandard/colors -- 终端颜色标准化](https://github.com/termstandard/colors)
- [Windows Terminal issue #11057 -- 请求设置 COLORTERM](https://github.com/microsoft/terminal/issues/11057)
- [Chalk supports-color 检测算法](https://github.com/chalk/supports-color)
- [Alacritty issue #1526 -- 添加 COLORTERM](https://github.com/alacritty/alacritty/issues/1526)
- [portable-pty CommandBuilder 文档](https://docs.rs/portable-pty)

---

## 2. TERM 环境变量

### 2.1 当前状态

slTerminal 的 `pty_spawn` 不显式设置 `TERM`。子进程继承父进程的环境值。在大多数 Windows 环境中，`TERM` 可能未被设置（值为空或 `dumb`），导致许多 CLI 工具完全禁用颜色输出。

### 2.2 推荐值：`xterm-256color`

`TERM=xterm-256color` + `COLORTERM=truecolor` 是当前最佳实践：

- 旧应用走 `xterm-256color` 的 terminfo（256 色兼容路径）
- 新应用检查 `COLORTERM` 开启真彩
- **不推荐 `TERM=xterm-direct`** -- 它把颜色 8+ 重定义为 24-bit RGB，导致所有使用 256 色调色板的旧应用看到几乎全黑的像素

### 2.3 其他推荐变量

```rust
cmd.env("TERM", "xterm-256color");
cmd.env("COLORTERM", "truecolor");
cmd.env("TERM_PROGRAM", "slTerminal");
cmd.env("TERM_PROGRAM_VERSION", env!("CARGO_PKG_VERSION"));
```

`TERM_PROGRAM` 提供终端身份标识，允许应用做终端特定的 workaround（如 Neovim 检测 iTerm、WezTerm 等）。

### 2.4 注意事项

- **不要覆盖已有 TERM**（如用户运行在 tmux 内时 `TERM=screen-256color`）
- **终端模拟器在 spawn 阶段设变量，不是 shell**（shell rc 硬编码会导致切换终端时类型不匹配）
- `COLORTERM` 不被 SSH 默认转发 -- 远端需自行配置

### 2.5 来源

- [termstandard/colors](https://github.com/termstandard/colors)
- [weechat issue #1364 -- xterm-direct vs xterm-256color](https://github.com/weechat/weechat/issues/1364)
- [Neovim PR #26407 -- 自动启用 termguicolors](https://github.com/neovim/neovim/pull/26407)
- [TrueColour.md (XVilka) -- 终端真彩支持权威清单](https://gist.github.com/XVilka/8346728)

---

## 3. ConPTY 24-bit 颜色透传

### 3.1 核心发现

ConPTY 不是透明管道。子进程的 Console API 调用被 conhost 转换为 VT 序列，此过程会主动改写 SGR 序列。

### 3.2 历史 Bug：颜色缩窄（Narrowing，Issue #2661）

修复前的 ConPTY 存在严重的颜色带化问题 -- 渲染器提前将颜色转为 `COLORREF`（RGB），VT 引擎再从 RGB 反向推导 SGR。这是有损操作：

| 子进程输出 | ConPTY 改写为 | 原因 |
|---|---|---|
| `ESC[38;2;22;198;12m`（真彩绿） | `ESC[92m`（16 色亮绿） | RGB(22,198,12) 匹配 Campbell 色表索引 10 |
| `ESC[38;5;127m`（256 色） | `ESC[38;2;175;0;175m` | 展开为 RGB，丢失 256 色语义 |
| `ESC[38;2;12;12;12m`（近黑真彩） | `ESC[30m`（ANSI 黑） | 匹配索引 0，丢失 RGB 语义 |

### 3.3 修复（PR #5834 + #6506）

- **PR #5834**：引入 `ColorType` 枚举（`IsIndex16`/`IsIndex256`/`IsTrueColor`），区分颜色来源
- **PR #6506**：延迟颜色转换，Xterm256Engine 按优先级分派：
  1. 默认颜色 -> `SGR 39/49`
  2. 16 色索引 -> ANSI/AIX 序列
  3. 256 色索引 -> `SGR 38;5;n` / `SGR 48;5;n`
  4. **24-bit RGB -> `SGR 38;2;r;g;b` / `SGR 48;2;r;g;b` 原样透传**

### 3.4 修复覆盖范围

| Windows 版本 | ConPTY 真彩透传 |
|-------------|:---:|
| Win10 1703 前 | 不支持 VT 序列 |
| Win10 1703+（修复前） | 支持但会缩窄 |
| Win10 1903+（回植修复） | 原样透传 |
| Win11 22H2+（PASSTHROUGH_MODE） | 完全透传（绕过 conhost 翻译层） |

### 3.5 ConPTY 其他已知颜色问题

| 问题 | GitHub Issue | 状态 |
|------|-------------|:----:|
| `ESC[49m` 被优化为 `ESC[m`（丢失默认背景语义） | [#362](https://github.com/microsoft/terminal/issues/362) | 已知 |
| 不转发 OSC 4（控制台色表变更） | [#2985](https://github.com/microsoft/terminal/issues/2985) | 已知 |
| SGR 7 反色渲染不可见 | [#5498](https://github.com/microsoft/terminal/issues/5498) | 已知 |
| DCS 序列被注入 SGR reset | [#17117](https://github.com/microsoft/terminal/issues/17117) | 已修复 |
| SGR 序列计为可打印字符导致提前换行 | [#380](https://github.com/microsoft/terminal/issues/380) | 已知 |

### 3.6 对 slTerminal 的影响

- 在现代 Windows 上（Win10 1903+），ConPTY 已正确透传 24-bit true color SGR 序列
- `ESC[49m` 被优化为 `ESC[m` 可能导致背景色 reset 行为异常 -- xterm.js 需要额外处理此差异
- 若将来使用 `PSEUDOCONSOLE_PASSTHROUGH_MODE`，需 Win11 22H2+，否则输出损坏

### 3.7 ConPTY 启动序列注入

ConPTY 初始化时注入以下序列（`VtIo::StartIfNeeded`）：

- `ESC[6n`（DSR -- 查询光标位置）
- `ESC[c`（DA1 -- 查询终端属性）
- ConPTY 阻塞等待 DA1 回复，超时 3000ms

slTerminal 的 reader 线程通过 `strip_conpty_startup()` 剥离这些注入序列。

### 3.8 来源

- [microsoft/terminal Issue #2661 -- ConPTY narrowing bugs](https://github.com/microsoft/terminal/issues/2661)
- [microsoft/terminal PR #6506 -- 改善 ConPTY 颜色属性传递](https://github.com/microsoft/terminal/commit/ddbe370)
- [24-bit Color in the Windows Console (Microsoft DevBlog)](https://devblogs.microsoft.com/commandline/24-bit-color-in-the-windows-console/)
- [PSEUDOCONSOLE_PASSTHROUGH_MODE (portable-pty-psmux)](https://docs.rs/crate/portable-pty-psmux)
- [ConPTY and VT I/O (DeepWiki)](https://deepwiki.com/microsoft/terminal/2.4-conpty-and-console-host-integration)

---

## 4. xterm.js 主题色表与 True Color 冲突

### 4.1 核心结论：不冲突

**xterm.js 的 ANSI 16 色调色板（theme）完全不影响 True Color（24-bit RGB）渲染。**

`theme.ts` 中配置的 16 色调色板只控制以下序列的颜色映射：

| 颜色模式 | CSI 序列 | 受 theme 影响？ |
|----------|----------|:---:|
| ANSI 16 (SGR 30-37, 40-47) | `ESC[31m`, `ESC[92m` | **是** |
| ANSI 256 (SGR 38;5) 索引 0-15 | `ESC[38;5;1m` | **是** |
| ANSI 256 (SGR 38;5) 索引 16-255 | `ESC[38;5;200m` | 否（固定 6x6x6 cube + 灰度） |
| **True Color (SGR 38;2)** | `ESC[38;2;255;128;0m` | **否** |

### 4.2 内部机制

xterm.js 内部使用统一数值编码区分颜色模式：

| 颜色模式 | 存储格式 | `isFgRGB()` | `isFgPalette()` |
|----------|---------|:---:|:---:|
| Default | `0` | false | false |
| ANSI 16/256（调色板索引） | `0-255` | false | true |
| True Color | `0xRRGGBB`（24-bit hex） | true | false |

**关键**：不能根据数值范围判断模式。`0x0000FF`（RGB 蓝色）= 十进制 255，与调色板索引 255 重叠，必须用 `isFgRGB()`/`isFgPalette()` 区分。

True Color 渲染路径：SGR 解析器 -> 设置 `fgColor=0xRRGGBB` + `fgColorMode=RGB` -> DOM/WebGL 渲染器直接输出 `#RRGGBB` CSS 颜色，**完全绕过 `ColorManager` 的调色板查找**。

xterm.js 内部 cell 属性编码（每 cell 占 3 个 Uint32Array slot）：

```
[fg_and_flags, char_code, bg_and_flags]
```

属性以位掩码打包。BgFlags 中的 `HAS_EXTENDED` 标记指向溢出属性数组（overline、strikethrough 等）。

### 4.3 Claude Code 发出的序列

Claude Code 的 Ink 渲染器在 Level 3（TrueColor）下输出的是 24-bit RGB SGR 序列：
- `ESC[38;2;R;G;Bm` 前景色
- `ESC[48;2;R;G;Bm` 背景色

这些序列**不受** theme.ts 中定义的 ANSI 调色板影响。

### 4.4 但需注意的边界情况

- `drawBoldTextInBrightColors`：仅影响 ANSI 16 色 palette 模式，不影响 True Color
- `minimumContrastRatio`：v4.19+ 的渲染后处理，会调整最终渲染颜色（包括 True Color），但其目的是 WCAG 无障碍，不是调色板覆盖
- **SGR 39/49（默认色 reset）**：会恢复为 `theme.foreground`/`theme.background`，Claude Code 频繁使用此码
- **SGR 7（反色）**：前景/背景互换，如果一方是 True Color，直接交换实际 RGB 值

### 4.5 xterm.js 扩展调色板硬编码

256 色调色板结构（不可通过 theme 自定义索引 16-255）：

- 0-15：16 种 ANSI 色（可通过 ITheme 自定义）
- 16-231：6x6x6 RGB 立方体（硬编码：`index = 16 + 36*r + 6*g + b`，r/g/b 取值 0/95/135/175/215/255）
- 232-255：24 级灰度（硬编码：`value = 8 + 10*(index-232)`）

Issue #3601 提议 `ITheme.extendedAnsi` API 自定义此范围，尚未实现。

OSC 4（运行时修改调色板）不被 xterm.js 支持。

### 4.6 来源

- [xterm.js PR #1895 -- True color support](https://github.com/xtermjs/xterm.js/pull/1895)
- [xterm.js Issue #484 -- Terminal does not render true color](https://github.com/xtermjs/xterm.js/issues/484)
- [xterm.js Issue #2603 -- 改善属性数据命名](https://github.com/xtermjs/xterm.js/issues/2603)
- [xterm.js Issue #3601 -- extendedAnsi 支持](https://github.com/xtermjs/xterm.js/issues/3601)
- [Oliver Roick -- Setting Colours in Xterm.js](https://oliverroick.net/learnings/2024/setting-colours-in-xterm-js.html)
- [xterm.js ThemeService.ts 源码](https://github.com/xtermjs/xterm.js)

---

## 5. drawBoldTextInBrightColors

### 5.1 当前状态

slTerminal 的 `theme.ts` 未显式设置 `drawBoldTextInBrightColors`。xterm.js 的默认值为 `true`。

### 5.2 行为机制

当 `drawBoldTextInBrightColors=true` 时，xterm.js 对 **bold + 前景色索引 0-7** 自动 +8（映射到亮色变体 8-15）：

```typescript
// DomRendererRowFactory.ts
if (drawBoldTextInBrightColors && cell.isBold() && fg < 8) {
    fg += 8;
}
```

### 5.3 对 Claude Code 的影响

**对 True Color 无影响**。此选项仅作用于 ANSI 16 色 palette 模式。

如果 Claude Code 发出 bold + True Color 组合（`ESC[1;38;2;R;G;Bm`），xterm.js 不会对 RGB 颜色做任何 palette 重新映射。

### 5.4 建议

- 保持默认 `true` 不显式设置（行为正确，无负面影响）
- 不需要设为 `false`（会影响使用 ANSI 16 色的传统 TUI 应用）

### 5.5 来源

- [xterm.js DomRendererRowFactory.ts 源码](https://github.com/xtermjs/xterm.js)
- [xterm.js ITerminalOptions 文档](https://xtermjs.org/docs/api/terminal/interfaces/iterminaloptions/)

---

## 6. 256 色退化

### 6.1 问题机制

当 Claude Code（Chalk/Ink）检测不到终端 TrueColor 能力时，自动降级：

| Chalk Level | 颜色模式 | SGR 输出 | 视觉效果 |
|:---:|---------|---------|---------|
| 3 | 24-bit TrueColor | `ESC[38;2;R;G;Bm` | 原始颜色 |
| 2 | 256 色 | `ESC[38;5;Nm`（量化到最近 256 色调色板值） | 轻微色偏 |
| 1 | 16 色（ANSI） | `ESC[31m` 等（量化到 16 基本色） | **严重色偏** |
| 0 | 无颜色 | 纯文本 | 所有高亮消失 |

### 6.2 当前 slTerminal 的降级路径

```
slTerminal 不设 COLORTERM
  -> Chalk supports-color 检测：
    1. FORCE_COLOR 未设 -> 跳过
    2. COLORTERM 未设 -> 跳过
    3. TERM 未知/未设 -> 跳过
    4. TERM_PROGRAM 未设 -> 跳过
    5. Windows build >= 14931 -> Level 1 (仅 16 色)
  -> Claude Code 所有 hex/RGB 颜色 -> 量化到 16 色
  -> 严重色偏 + 高亮丢失
```

### 6.3 修复

设置 `COLORTERM=truecolor` 即可。Chalk 检测到后直接给 Level 3，Claude Code 所有颜色输出为 24-bit RGB。

用户也可手动 `FORCE_COLOR=3` 覆盖（优先级最高），但不推荐作为默认方案。

### 6.4 来源

- [Chalk supports-color 检测算法](https://github.com/chalk/supports-color)
- [Claude Code 架构分析 (GraphScope)](https://graphscope.io/blog/tech/2026/04/02/Dissecting-Claude-Code)

---

## 7. SGR 序列组合解析

### 7.1 Ink 发出的复杂 SGR 组合

Ink 嵌套 `Text` 组件时维护样式栈，退出嵌套恢复外部样式：

```
// 嵌套文本（深色代码块）
\x1b[38;2;0;0;0m           // 前景: 黑
\x1b[48;2;230;230;230m     // 背景: 浅灰
\x1b[1m                     // bold ON
\x1b[0m                     // 全量重置
\x1b[38;2;0;200;0m         // 恢复外层: 绿字
```

Ink 的退出嵌套模式：`ESC[0m`（全量重置）+ 重新发出外层完整样式。避免 ANSI 属性累积漂移。

### 7.2 常见组合模式

Claude Code 典型输出包括：

```
// 1. 单属性 + TrueColor
\x1b[38;2;0;200;0m Bold Green Text\x1b[0m

// 2. 多属性叠加 + TrueColor（高亮输出）
\x1b[1;3;38;2;255;200;0m Bold Italic Amber\x1b[0m

// 3. 前景 + 背景 TrueColor
\x1b[38;2;255;255;255;48;2;30;30;46m White on Dark\x1b[0m

// 4. 下划线颜色（SGR 58）
\x1b[4;58;2;255;0;0m Red underline\x1b[0m

// 5. OSC 8 超链接（Markdown 链接渲染）
\x1b]8;;https://example.com\x1b\\Link text\x1b]8;;\x1b\\
```

### 7.3 xterm.js 内部 SGR 属性模型

xterm.js 将每个 cell 的属性以**位掩码**形式压缩到 `Uint32Array`，每列占 3 个 slot：

```
[fg_and_flags, char_code, bg_and_flags]   // 每 cell
```

| Flag 层级 | 包含属性 |
|----------|---------|
| **FgFlags**（打包进 fg 高位） | `BOLD`, `ITALIC`, `DIM`, `INVISIBLE`, `UNDERLINE`, `BLINK` |
| **BgFlags**（打包进 bg 高位） | `HAS_EXTENDED`（溢出标记，指向单独 `_extendedAttrs` 数组：overline/strikethrough） |

`InputHandler.parseSgr()` 按顺序遍历 CSI 参数，每个参数修改当前游标属性（curAttr），后续写入的字符继承该游标属性。未知 SGR 码遵循 ECMA-48：忽略（switch 无匹配分支 -> no-op）。

### 7.4 xterm.js 完整 SGR 支持矩阵

#### 基本文本属性

| SGR | 属性 | 关闭码 | 备注 |
|:---:|------|:---:|------|
| 0 | 全量重置 | -- | 一键复位所有属性到默认值 |
| 1 | Bold | 22 | 与 dim 互斥（SGR 22 同时关闭两者） |
| 2 | Dim | 22 | CSS opacity 或专用 color |
| 3 | Italic | 23 | font-style: italic，需 italic 字体变体已加载 |
| 4 | 单下划线 | 24 | text-decoration: underline |
| 5 | 慢闪烁 | 25 | |
| 7 | 反色 | 27 | 前景/背景互换 |
| 8 | 隐藏/不可见 | 28 | |
| 9 | 删除线 | 29 | v4.14.0+ |
| 21 | 双下划线 | 24 | ECMA-48 标准行为，历史争议已收敛 |
| 53 | 上划线 | 55 | v5.x（PR #4526），可与下划线叠加 |

#### 16 色 ANSI 前景/背景

SGR 30-37（前景）、40-47（背景）、90-97（亮前景）、100-107（亮背景）。受 theme 调色板控制。

#### 256 色

`CSI 38;5;Nm`（前景）、`CSI 48;5;Nm`（背景）。索引 0-15 系统色（theme 可控）、16-231 6x6x6 RGB 立方体（硬编码）、232-255 24 级灰度（硬编码）。

#### 24-bit TrueColor

`CSI 38;2;R;G;Bm`（前景）、`CSI 48;2;R;G;Bm`（背景）。完全绕过调色板，直接渲染为 CSS `#RRGGBB`。

#### 下划线样式子参数（v5.0.0+，仅冒号语法）

| 序列 | 样式 |
|------|------|
| `CSI 4:0m` | 无下划线 |
| `CSI 4:1m` | 单下划线 |
| `CSI 4:2m` | 双下划线 |
| `CSI 4:3m` | 波浪线 (curly/wavy) |
| `CSI 4:4m` | 点线下划线 |
| `CSI 4:5m` | 虚线下划线 |

#### 下划线颜色（v5.0.0+，仅冒号语法）

| 序列 | 含义 |
|------|------|
| `CSI 58:5:Nm` | 256 色下划线颜色 |
| `CSI 58:2::R:G:Bm` | TrueColor 下划线颜色（需空颜色空间 ID 的双冒号） |
| `CSI 59m` | 恢复默认下划线颜色（跟随前景色） |

**重要**：SGR 58 只接受冒号语法（`58:5:N`）。发送分号形式 `58;5;N` 会被错误解析为"SGR 58（未定义）、SGR 5（慢闪烁）、SGR N"。Claude Code/Chalk 不发出下划线颜色，故 slTerminal 不受影响。

### 7.5 分号 vs 冒号标记深度分析

ITU T.416 / ISO 8613-6 规范的完整格式为：

```
ESC [ 38 : 2 : <颜色空间ID> : <R> : <G> : <B> : <未使用> : <CS容差> : <容差颜色空间> m
```

共 8 个参数元素。xterm 1999 年首次实现时误用分号（`38;2;R;G;B`），成为事实标准。

| 格式 | 示例 | 支持 |
|------|------|:---:|
| 分号（事实标准） | `\e[38;2;255;0;0m` | xterm.js 始终支持（PR #1895） |
| 冒号（法律标准） | `\e[38:2:255:0:0m` | xterm.js PR #2241（2019.6） |
| 混合语法 | `\e[38;2:R:G:Bm` | xterm.js 支持（宽松解析器） |
| 颜色空间 ID | `\e[38:2:id:R:G:Bm` | 解析后忽略（极少有应用使用） |
| 省略颜色空间 ID | `\e[38:2::255:0:0m` | 支持（双冒号） |

xterm 的向后兼容策略：检查 "2" 之后的参数元素数量 -- 恰好 3 个按 R/G/B 处理（兼容旧版），4+ 个取第一个为颜色空间 ID。

### 7.6 已知 SGR 边界问题

#### 粗体字形宽度偏移

某些等宽字体的 bold 变体 glyph 宽度与 regular 不一致，破坏终端网格对齐。slTerminal 使用 JetBrains Mono（严格等宽），bold 变体宽度一致，不受此问题影响。

#### 下划线不连续

相邻单元格颜色不同时，CSS `text-decoration` 会被分割出断点（Issue #4059）。WebGL 渲染器通过手动绘制下划线绕过此限制。

#### 空格 + 下划线

仅空格字符带下划线时可能不被渲染 -- DOM 渲染器用 `<span>` 包裹字符组，纯空格 span 高度为 0（Issue #4901）。

#### SGR 21 历史争议

ECMA-48 定义 SGR 21 为"双下划线"，历史上很多终端实现为"关闭粗体"。现代终端已收敛到标准行为。xterm.js 中 SGR 24 同时关闭单下划线和双下划线。

#### 未实现特性

| 特性 | Issue |
|------|-------|
| XTPUSHSGR / XTPOPSGR（`CSI # {` / `CSI # }`） | [#2570](https://github.com/xtermjs/xterm.js/issues/2570) |
| 256 色索引超出范围（>= 256）静默截断 | 已知 |
| OSC 4 运行时修改调色板 | 不支持 |

### 7.7 Claude Code 已知 ANSI 输出 Bug

| Bug | 描述 |
|-----|------|
| 背景色泄漏（#1341） | chalk.bgRed 缺少 ESC[0m 重置，背景色渗出到后续 UI |
| 状态行原始码（#6635） | 自定义状态行中的 ANSI 码显示为文字而非颜色 |
| 末尾字符截断（#13942） | 重置码前一个字符被吃掉 |

slTerminal 的 xterm.js 解析器对这些异常输出的防御性处理有限 -- 主要依赖 xterm.js 自身的容错能力。

### 7.8 来源

- [xterm.js PR #1895 -- True color support](https://github.com/xtermjs/xterm.js/pull/1895)
- [xterm.js PR #2241 -- 冒号语法支持](https://github.com/xtermjs/xterm.js/pull/2241)
- [xterm.js Issue #4059 -- 下划线不连续](https://github.com/xtermjs/xterm.js/issues/4059)
- [xterm.js Issue #4901 -- 空格下划线](https://github.com/xtermjs/xterm.js/issues/4901)
- [xterm.js Issue #2570 -- XTPUSHSGR](https://github.com/xtermjs/xterm.js/issues/2570)
- [Ink DeepWiki](https://deepwiki.com/vadimdemedes/ink)
- [Claude Code Issues](https://github.com/anthropics/claude-code/issues)
- [Trail of Bits -- ANSI 安全分析](https://blog.trailofbits.com/2025/04/29/deceiving-users-with-ansi-terminal-codes-in-mcp/)

---

## 8. WebGL Addon 颜色渲染

### 8.1 WebGL vs DOM 颜色差异

xterm.js 的 WebGL addon 和 DOM renderer 使用相同的 `ColorManager` 和 `IColorSet` 构建实际颜色值。理论颜色应一致，但可能存在：

- **WebGL shader 精度限制**（8-bit per channel），在渐变过渡区域可能产生极轻微色偏
- **WebGL 纹理 atlas 缓存策略**：True Color 字符（16.7M 颜色可能）不进入 glyph cache，直接通过 shader 带颜色参数渲染
- **字体渲染引擎差异**：WebGL 用纹理渲染字形，DOM 依赖浏览器字体 rasterizer -- bold 字形可能有亚像素差异

### 8.2 WebGL 上下文丢失

slTerminal 的 `useXterm.ts` 中 `setupWebglWithRetry()` 实现了 context loss 自动重建。每次重建会重新创建 WebGL 上下文和纹理 atlas，中间有短暂白屏。

### 8.3 CSS 属性叠加映射

xterm.js DOM 渲染器将 SGR 属性映射为叠加的 CSS 类：

| SGR 组合 | CSS 类链 |
|----------|---------|
| Bold | `.xterm-bold { font-weight: bold; }` |
| Dim | `.xterm-dim { opacity: 0.5; }` |
| Italic | `.xterm-italic { font-style: italic; }` |
| Underline | `.xterm-underline-1 { text-decoration: underline; }` |
| Strikethrough | `.xterm-strikethrough { text-decoration: line-through; }` |
| Overline | `.xterm-overline { text-decoration: overline; }` |
| Bold + Italic + Underline | `.xterm-bold.xterm-italic.xterm-underline-1` |

Overline 可与全部 5 种下划线样式叠加（PR #4526）。

### 8.4 来源

- [xterm.js PR #1895](https://github.com/xtermjs/xterm.js/pull/1895)
- [xterm.js WebGL addon 文档](https://github.com/xtermjs/xterm.js/tree/master/addons/addon-webgl)
- [xterm.js PR #4526 -- overline 支持](https://github.com/zyztek/xterm/commit/cdec1e5b)

---

## 9. 其他颜色相关优化方向

### 9.1 OSC 4（调色板变更）不支持

ConPTY 不转发 OSC 4（Issue #2985）。运行时动态改色的 CLI 工具（如 Colorful.Console）在 ConPTY 下显示异常。**这不是 slTerminal 的问题**，是 ConPTY 的限制。xterm.js 本身也不支持 OSC 4。

### 9.2 SGR 7 反色不可见

ConPTY Issue #5498：SGR 7（反色）与 ConPTY 颜色重映射冲突可能导致文本不可见。**这是 ConPTY 层的问题**，但 slTerminal 的 xterm.js 渲染器本身正确支持 SGR 7。

### 9.3 minimumContrastRatio 无障碍

xterm.js v4.19.0 引入 `minimumContrastRatio` 选项，用于 WCAG 无障碍对比度。影响渲染时的最终颜色值（包括 True Color）。当前 slTerminal 未设置，使用默认值（可能为 1，即不调整）。

### 9.4 来源

- [microsoft/terminal Issue #362 -- ESC[49m 优化](https://github.com/microsoft/terminal/issues/362)
- [microsoft/terminal Issue #2985 -- OSC 4 不转发](https://github.com/microsoft/terminal/issues/2985)
- [microsoft/terminal Issue #5498 -- SGR 7 渲染](https://github.com/microsoft/terminal/issues/5498)

---

## 10. 修复优先级建议

### 立即修复（3 行代码）

在 `src-tauri/src/pty/spawn.rs` 的 `pty_spawn` 函数中，第 96-98 行 `cmd.cwd()` 之后添加：

```rust
// 宣告终端能力，使子进程（Claude Code 等）能检测 truecolor
cmd.env("TERM", "xterm-256color");
cmd.env("COLORTERM", "truecolor");
cmd.env("TERM_PROGRAM", "slTerminal");
```

这消除了根因中的绝大部分颜色退化问题。

### 后续改进

1. **Probe 当前 TERM 值**：若已设置（如 tmux 内 `screen-256color`），不覆盖，只追加 `COLORTERM`
2. **shell-integration.ps1 中加 COLORTERM**：作为第二层保障（但不应依赖 shell rc 设终端能力）
3. **验证修复**：在 slTerminal 中运行 `echo $env:COLORTERM` 和 `echo $env:TERM`，确认变量正确注入
4. **考虑 `TERM_PROGRAM_VERSION`**：将来用于兼容性判断

---

## 附录：信息来源索引

### 终端颜色标准化
- [termstandard/colors](https://github.com/termstandard/colors)
- [TrueColour.md (XVilka)](https://gist.github.com/XVilka/8346728)

### Windows / ConPTY
- [microsoft/terminal Issue #2661 -- ConPTY narrowing](https://github.com/microsoft/terminal/issues/2661)
- [microsoft/terminal PR #6506 -- 颜色属性改善](https://github.com/microsoft/terminal/commit/ddbe370)
- [microsoft/terminal Issue #11057 -- COLORTERM](https://github.com/microsoft/terminal/issues/11057)
- [microsoft/terminal Issue #362 -- ESC[49m](https://github.com/microsoft/terminal/issues/362)
- [microsoft/terminal Issue #2985 -- OSC 4](https://github.com/microsoft/terminal/issues/2985)
- [microsoft/terminal Issue #5498 -- SGR 7](https://github.com/microsoft/terminal/issues/5498)
- [microsoft/terminal Issue #380 -- SGR 计数为打印字符](https://github.com/microsoft/terminal/issues/380)
- [24-bit Color in Windows Console](https://devblogs.microsoft.com/commandline/24-bit-color-in-the-windows-console/)
- [microsoft/vscode Issue #124427 -- CONPTY_ENABLED](https://github.com/microsoft/vscode/issues/124427)
- [ConPTY and VT I/O (DeepWiki)](https://deepwiki.com/microsoft/terminal/2.4-conpty-and-console-host-integration)

### xterm.js
- [xterm.js PR #1895 -- True color support](https://github.com/xtermjs/xterm.js/pull/1895)
- [xterm.js PR #2241 -- 冒号语法支持](https://github.com/xtermjs/xterm.js/pull/2241)
- [xterm.js Issue #484 -- true color rendering](https://github.com/xtermjs/xterm.js/issues/484)
- [xterm.js Issue #2603 -- 属性数据命名](https://github.com/xtermjs/xterm.js/issues/2603)
- [xterm.js Issue #3601 -- extendedAnsi](https://github.com/xtermjs/xterm.js/issues/3601)
- [xterm.js Issue #4059 -- 下划线不连续](https://github.com/xtermjs/xterm.js/issues/4059)
- [xterm.js Issue #4901 -- 空格下划线](https://github.com/xtermjs/xterm.js/issues/4901)
- [xterm.js Issue #2570 -- XTPUSHSGR](https://github.com/xtermjs/xterm.js/issues/2570)
- [xterm.js Issue #4800 -- BufferLine 提案](https://github.com/xtermjs/xterm.js/issues/4800)
- [xterm.js ITerminalOptions 文档](https://xtermjs.org/docs/api/terminal/interfaces/iterminaloptions/)
- [Oliver Roick -- Setting Colours in Xterm.js](https://oliverroick.net/learnings/2024/setting-colours-in-xterm-js.html)

### Chalk / Ink
- [Chalk supports-color](https://github.com/chalk/supports-color)
- [Ink DeepWiki](https://deepwiki.com/vadimdemedes/ink)
- [Claude Code 架构分析 (GraphScope)](https://graphscope.io/blog/tech/2026/04/02/Dissecting-Claude-Code)

### Claude Code
- [Anthropic/claude-code Issues](https://github.com/anthropics/claude-code/issues)
- [Trail of Bits -- ANSI 终端码安全](https://blog.trailofbits.com/2025/04/29/deceiving-users-with-ansi-terminal-codes-in-mcp/)

### portable-pty
- [portable-pty CommandBuilder 文档](https://docs.rs/portable-pty/0.8.1/i686-pc-windows-msvc/portable_pty/cmdbuilder/struct.CommandBuilder.html)
- [portable-pty-psmux PASSTHROUGH_MODE](https://docs.rs/crate/portable-pty-psmux)
- [wezterm pseudocon.rs (PASSTHROUGH_MODE 实现)](https://github.com/wezterm/wezterm)

### 终端模拟器参考
- [Alacritty Issue #1526](https://github.com/alacritty/alacritty/issues/1526)
- [Neovim PR #26407 -- termguicolors](https://github.com/neovim/neovim/pull/26407)
- [weechat Issue #1364 -- xterm-direct 兼容性](https://github.com/weechat/weechat/issues/1364)
- [Ruby Reline PR #604 -- COLORTERM fallback](https://github.com/ruby/reline/pull/604)
- [Kitty COLORTERM 讨论](https://github.com/kovidgoyal/kitty/discussions/8790)
- [WezTerm term 配置](https://wezterm.org/config/lua/config/term.html)
