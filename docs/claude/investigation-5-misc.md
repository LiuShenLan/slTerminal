# 调查方向五：Claude Code CLI 在 slTerminal 中的终端兼容性问题

> 状态：调查完成 | 日期：2026-07-07

## 调查概览

本文档从 9 个方向系统性调查 Claude Code CLI 在 slTerminal（xterm.js 6.0.0 + ConPTY）中可能存在的终端兼容性问题。

---

## 1. Kitty 键盘协议 (CSI u)

### 1.1 协议背景

Kitty keyboard protocol 通过渐进增强（progressive enhancement）机制解决传统终端键盘处理的严重缺陷：

| 问题 | 传统行为 | Kitty 协议解决方式 |
|------|----------|-------------------|
| Ctrl+I = Tab 无法区分 | 都发 0x09 | 各自编码为不同 CSI u 序列 |
| Ctrl+M = Enter 无法区分 | 都发 0x0d | 各自编码为不同 CSI u 序列 |
| Shift+Enter = Enter | 都发 0x0d | Shift+Enter 发独立 CSI u 序列 |
| Escape 键 vs 转义前缀 | 需要超时试探 | 明确编码区分 |

**五种渐进增强标志位**（通过 `CSI = flags ; mode u` 设置）：

| 位 | 十进制 | 标志 | 效果 |
|----|--------|------|------|
| 0b1 | 1 | Disambiguate | Esc/Alt/Ctrl 统一走 CSI u，Ctrl+C 不再发 SIGINT |
| 0b10 | 2 | Report events | 上报按下/重复/释放事件 |
| 0b100 | 4 | Report alternates | 同时上报 Shift 后字符和基准键值 |
| 0b1000 | 8 | Report all keys | 所有按键（含普通字母）编码为 CSI u |
| 0b10000 | 16 | Report associated text | 附加上下文 Unicode 文本 |

**Push/Pop 栈机制**：`CSI > flags u` 压栈 + 设置，`CSI < number u` 弹栈恢复。主屏幕和备用屏幕各自维护独立栈。

按键编码格式：`CSI unicode-key-code[:alternates];modifiers[:event-type];text-as-codepoints u`

来源：https://sw.kovidgoyal.net/kitty/keyboard-protocol/

### 1.2 Claude Code 对 CSI u 的使用

Claude Code CLI 主动 push `CSI > 1u`（启用 Disambiguate）以区分 Shift+Enter vs Enter。这是 Claude Code 多行输入功能的核心依赖——不带 Shift 的 Enter 提交消息，Shift+Enter 插入换行。

### 1.3 xterm.js 支持状态

xterm.js 从 **v6.1.0-beta.x** 开始原生支持 Kitty keyboard protocol（PR [#5600](https://github.com/xtermjs/xterm.js/pull/5600)）。支持方式是被动的——终端内应用发送 `CSI > flags u` 来启用协议，xterm.js 按协议规范编码后续键盘事件。

### 1.4 slTerminal 当前状态

**风险等级：中高**

- slTerminal 使用 **xterm.js 6.0.0**，**不支持** Kitty keyboard protocol
- 当 Claude Code 发送 `CSI > 1u` 启用 CSI u 时，xterm.js 6.0.0 的 DEC 解析器进入 `csi_ignore` 状态，序列被**静默消费**，不产生任何效果
- 后果：Shift+Enter 被当作普通 Enter，Claude Code 的**多行输入功能失效**

**修复方案**：升级 xterm.js 到 >= 6.1.0。运行 `npm install @xterm/xterm@^6.1.0`。

---

## 2. 终端能力宣告 (TERM/COLORTERM)

### 2.1 环境变量规范

| 变量 | 推荐值 | 作用 |
|------|--------|------|
| `TERM` | `xterm-256color` | 终端能力标识，决定 terminfo 数据库使用哪个条目 |
| `COLORTERM` | `truecolor` | 宣告 24-bit 真彩色支持（de facto 标准） |
| `TERM_PROGRAM` | `slTerminal` | 终端自标识（可选但推荐） |

### 2.2 slTerminal 当前状态

**风险等级：高**

当前 slTerminal **未设置任何终端能力环境变量**：

- `shell.rs` 的 `CommandBuilder` 不调用 `.env("TERM", ...)` 也不调用 `.env("COLORTERM", ...)`
- `shell-integration.ps1` 仅设置 UTF-8 编码（`[Console]::OutputEncoding` 等），不设 TERM/COLORTERM
- 子进程继承 Windows 全局环境变量（可能为 `TERM=` 空值或不存在）

### 2.3 影响分析

| 缺失 | 后果 |
|------|------|
| `TERM=xterm-256color` | Claude Code 无法通过 terminfo 查询终端能力，可能回退到保守的 VT100 模式（8 色） |
| `COLORTERM=truecolor` | Claude Code 回退到 256 色调色板，无法使用 24-bit 真彩色 |

Claude Code 的颜色检测优先级（从高到低）：
1. `NO_COLOR` 已设置 → 无颜色
2. `CI=true` → 无颜色
3. `TERM=dumb` → 无颜色
4. Agent 环境变量 → 无颜色
5. stdout 非 TTY → 无颜色
6. 默认 → 富文本 + 颜色

**在 Docker/devcontainer 中硬编码 `TERM=xterm-256color` 的实际教训**：会阻止宿主机终端类型传入容器，导致 Shift+Enter 等功能失效。正确做法是终端模拟器在 spawn 时注入，不在 shell rc 中覆盖。

### 2.4 修复方案

在 `shell.rs` 的 `CommandBuilder` 中添加环境变量：

```rust
cmd.env("TERM", "xterm-256color");
cmd.env("COLORTERM", "truecolor");
cmd.env("TERM_PROGRAM", "slTerminal");
```

来源：
- https://code.claude.com/docs/zh-CN/env-vars
- https://wezterm.org/config/lua/config/term.html
- https://github.com/termstandard/colors

---

## 3. OSC 8 超链接

### 3.1 协议格式

```
ESC ] 8 ; [params] ; URI ST  可见文本  ESC ] 8 ; ; ST
```

- `ST` 可以是 `BEL`（`\x07`）或 `ESC \`（`\x1b\\`）
- `params` 可选，支持 `id=xxx` 参数（同 URL 不同 id 的链接分别高亮）
- URI 遵循 RFC 3986 百分号编码
- 安全 scheme：`http:`、`https:`、`ftp:`、`file:`

### 3.2 Claude Code 的使用

Claude Code 通过 OSC 8 序列将 Markdown 链接渲染为可点击的终端超链接。但存在多个已知问题：

- [#29188](https://github.com/anthropics/claude-code/issues/29188)：特定格式的 Markdown 链接（`[#number](url)`）会被 GitHub issue autolink 覆盖
- [#59212](https://github.com/anthropics/claude-code/issues/59212)：文件路径不发射 `file://` 超链接
- [#59100](https://github.com/anthropics/claude-code/issues/59100)：Windows Terminal 需 `FORCE_HYPERLINK=1` 绕过自动检测

### 3.3 xterm.js 支持状态

xterm.js **从 5.x 起原生支持 OSC 8**（PR [#4005](https://github.com/xtermjs/xterm.js/issues/4005)），无需加载 `@xterm/addon-web-links`。

- `WebLinksAddon` 是纯文本 URL 正则匹配器，不处理 OSC 8——**两者互补**
- xterm.js 内置处理：解析 OSC 8 序列，通过 `linkHandler` 回调处理点击
- 正在讨论的自报能力序列（[#4982](https://github.com/xtermjs/xterm.js/issues/4982)）：xterm.js 可向应用宣告自身支持 `"hyperlinks"`

### 3.4 slTerminal 当前状态

**风险等级：中**

- xterm.js 6.0.0 原生支持 OSC 8 解析和渲染
- 但 slTerminal **未配置 `linkHandler`**——超链接会被正确渲染（显示为可点击样式，由 xterm.js 内置处理），但**点击后无响应**（未调用 `shell.open` 打开浏览器）
- `@xterm/addon-web-links` 未加载——裸文本 URL（如 `https://example.com` 未用 OSC 8 包裹）不可点击

### 3.5 修复方案

1. 在 `theme.ts` 的 `terminalOptions` 中配置 `linkHandler`：

```typescript
linkHandler: {
  activate: (_event, text) => {
    // 通过 Tauri shell.open 打开链接
    import("@tauri-apps/plugin-shell").then(({ open }) => {
      open(text).catch(() => {});
    });
  },
},
```

2. 可选：加载 `WeblinksAddon` 作为兜底（覆盖未用 OSC 8 的裸 URL）

来源：
- https://github.com/xtermjs/xterm.js/issues/4005
- https://gist.github.com/egmontkob/eb114294efbcd5adb1944c9f3cb5feda

---

## 4. DA1 终端查询与 60 秒启动延迟

### 4.1 DA1 机制

**请求**：`ESC [ c`（CSI c），主机查询终端设备属性。

**响应格式**：`ESC [ ? Ps1 ; Ps2 ; ... ; Psn c`
- Ps1 = 设备一致性级别（62=VT220, 63=VT320, 64=VT420, 65=VT520）
- Ps2..Psn = 扩展能力（1=132列, 4=Sixel, 6=选择性擦除, 22=ANSI颜色等）

### 4.2 Claude Code 使用 DA1 作为启动哨兵

Claude Code 的 Ink 渲染器在启动时发送两条查询：
1. **XTVERSION**（`ESC [ > 0 q`）——请求终端标识
2. **DA1**（`ESC [ c`）——作为同步**哨兵**，Promise 等待响应才 resolve

Ink 的 `flush()` 方法将 DA1 响应用作"前序查询已完成"的确认信号。

### 4.3 ConPTY 吞掉 DA1 的问题

**ConPTY 会拦截 DA1 查询（`ESC[c]`），内部处理后不向 stdout 返回任何响应。** 这导致：

- XTVERSION 能正常通过输出管道（因为当前 slTerminal reader 线程透传 OSC/CSI 序列）
- DA1 被 ConPTY 拦截后，子进程的 stdout 中**永远不会出现 DA1 响应**
- Claude Code 的 waitFor 永不 resolve → **阻塞约 60 秒**直到 Node.js 事件循环兜底

Confirmed issues：
- [Claude Code #36328](https://github.com/anthropics/claude-code/issues/36328)：ConPTY 60 秒启动延迟
- [Claude Code #34014](https://github.com/anthropics/claude-code/issues/34014)：VTE 终端上 XTVERSION 查询回显为可见文本

### 4.4 slTerminal 当前状态

**风险等级：高**

- `reader.rs` 的 `strip_conpty_startup()` 仅剥离首轮读取的 ConPTY 注入序列（OSC 标题/清屏/光标/DSR），**不注入 DA1 响应**
- xterm.js 作为前端渲染层支持 DA1 响应（VT420 级），但 xterm.js 的响应只会留在前端 DOM，不会回灌到 PTY 管道——因此对 ConPTY 吞 DA1 问题**无帮助**
- `reader.rs` 的 `match_csi_startup()` 中 `b's' | b'u' => Some(3)` 会误剥离普通的保存/恢复光标序列（ESC[s/ESC[u]），这是**潜在 bug**（虽然只在首轮读取触发）

### 4.5 修复方案

**最务实的方案**：在 Rust backend 的 reader 线程中，检测子进程发送 `ESC[c`（DA1 查询）后，主动向子进程 stdin 注入模拟的 DA1 响应。

实现思路：
1. `reader_loop` 在每次读取中扫描 `ESC[?1049h` 或 `ESC[c` 模式
2. 检测到 DA1 查询 `ESC[c` 时，立即向子进程 stdin 写入 `ESC[?64;22c`（VT420 + ANSI 颜色）
3. 此举模拟 ConPTY + conhost 的一致行为

**备选方案**（A/B 测试确认有效性再采用）：
- 在 `shell.rs` 的 `CommandBuilder` 中设置 `CLAUDE_CODE_SKIP_DA1=1`（如果 Claude Code 支持此变量跳过 DA1）

注意：`reader.rs` 中 `match_csi_startup` 的 `'s'|'u'` 剥离应修正为 `'s' => Some(2)`（ESC[s 只有 2 字节，不包含 `[`），或直接移除这两条——保存/恢复光标不是 ConPTY 专属，不应被当作启动序列剥离。

---

## 5. ConPTY 隐式鼠标注入

### 5.1 问题机制

当终端应用发送 `ESC[?1049h`（smcup / 进入交替屏幕）时，ConPTY **在未收到显式 DECSET 1000/1003 的情况下**，自动开启鼠标跟踪。用户在终端窗口内移动鼠标或滚轮时，ConPTY 向应用 stdin 注入原始鼠标转义序列（如 `<35;col;rowM`），形成"幽灵鼠标序列"。

三种鼠标模式：
- DECSET 1000：Normal Tracking（按下+释放）
- DECSET 1002：Button-Event Tracking（按下+释放+拖拽）
- DECSET 1003：Any-Event Tracking（任何移动均报告）

### 5.2 slTerminal 影响分析

**风险等级：低（对 Claude Code 主交互流程）**

- Claude Code CLI 是**行模式 REPL**，不进入交替屏幕，不触发 smcup
- 因此在 slTerminal 中运行 `claude` 的日常交互**不受此问题影响**
- 但如果用户在终端中运行 `vim`、`less`、`htop` 等 TUI 程序，理论上可能遇到此问题

### 5.3 防御性措施

1. 在 `pty/spawn.rs` 的 spawn 后注入 `ESC[?1000l` + `ESC[?1002l` + `ESC[?1003l`，全局禁用鼠标跟踪（不预期交互式程序使用鼠标）
2. 如未来需要支持鼠标 TUI 程序，可在 `reader_loop` 中检测 `ESC[?1049h` 后自动注入 `ESC[?1000l`

Microsoft Terminal 在较新版本（Windows 11 22H2+, build 22523+）修复了此问题。

来源：
- https://github.com/microsoft/terminal/issues/376
- https://github.com/NousResearch/hermes-agent/pull/15488

---

## 6. IME 组合与 Ctrl 组合键

### 6.1 xterm.js 键盘事件处理机制

xterm.js 通过三层键盘处理：

| 层级 | 机制 | 示例 |
|------|------|------|
| 1. `attachCustomKeyEventHandler` | 浏览器层面 capture 拦截 | IME 组合、Ctrl+W 被浏览器吞掉时 |
| 2. 内置键盘处理器 | xterm.js 内部 keydown 解析 | 字母键、Enter、Tab 等 |
| 3. `onData` 回调 | 最终字节流 | 应用层接收（如 PTY write） |

`attachCustomKeyEventHandler` 允许在浏览器层拦截按键事件，返回 `true` 交由 xterm.js 处理，返回 `false` 阻止传递。主要用于：
- IME 组合期间（compositionstart 时设 flag 阻止 xterm.js 输入双份字符）
- 浏览器/WebView2 吞掉的组合键补救

### 6.2 Claude Code 的 readline 快捷键

Claude Code 支持标准 readline 编辑快捷键：
- `Ctrl+W`：删除前一个词（发送 `\x17`）
- `Ctrl+A`：行首（发送 `\x01`）
- `Ctrl+E`：行尾（发送 `\x05`）
- `Ctrl+K`：删除至行尾（发送 `\x0b`）
- `Ctrl+U`：删除至行首（发送 `\x15`）
- `Ctrl+D`：删除光标处字符 / EOF（发送 `\x04`）

### 6.3 slTerminal 当前状态

**风险等级：低**

- `useXterm.ts` **未调用 `attachCustomKeyEventHandler`**
- `keyboard.ts` 仅处理 `Ctrl+Shift+C/V`（复制/粘贴），不在 capture 阶段拦截其它按键
- xterm.js 6.0.0 的默认键盘处理器将这些 Ctrl 组合键编码为对应的控制字符，通过 `onData` 回调发送到 PTY
- **结论：未实现 `attachCustomKeyEventHandler` 对 readline 快捷键不构成障碍**——Ctrl 组合键的默认编码行为完全满足 Claude Code 需求

### 6.4 IME 中文输入评估

xterm.js 使用隐藏 `<textarea>` 接收键盘输入，IME 组合通过标准 DOM 事件三步处理：

1. **compositionstart** -- `_isComposing = true`，不发送任何内容到 PTY
2. **compositionupdate** -- 仅更新 textarea 值，不触发终端输出
3. **compositionend** -- 提取 textarea 中选定文字，通过 `onData` 发送到 PTY，清空 textarea

WebView2 (Edge Chromium) 对 IME 支持完整，但需关注版本：
- **WebView2 >= 120（Edge 120+）** 候选窗口定位和 composition 事件处理正确
- 较老版本（Edge < 100）存在候选窗口定位偏移问题（textarea 置于视口外，IME 无法获取正确屏幕坐标）

中文 Windows 的编码问题（GBK/936 ↔ UTF-8）已在 `shell-integration.ps1` 中通过 `chcp 65001` 处理。

### 6.5 已知 IME 问题

| 问题 | 原因 | 缓解措施 |
|------|------|----------|
| IME 组合期间 Ctrl 键被跳过 | `keyCode = 229`, xterm.js 内部 keyDown 跳过 | 检查 `event.isComposing` flag |
| 候选窗口位置偏移 | textarea 在视口外（`left:-9999px`），旧版 IME 无法获取坐标 | WebView2 120+ 自动修复 |
| 输入法切换乱码 | 未完成的 composition 发送到 PTY | `compositionend` 完整处理即可 |

### 6.6 与 WebView2 被吞组合键的关系

**WebView2 吞掉的组合键**（如 `Ctrl+N`、`Ctrl+Tab`）作用于浏览器/窗口层级，不经过 xterm.js 文本输入管道。Claude Code 通过 PTY stdin 接收控制字符（如 Ctrl+W → `\x17`），xterm.js 的 `onData` 是独立的数据通道，**不受 WebView2 窗口级快捷键拦截影响**。这解释了为何未实现 `attachCustomKeyEventHandler` 不影响 Claude Code -- readline 快捷键的 ASCII 控制字符始终能到达 PTY。

### 6.7 修复建议

目前无紧急修复需求。如果未来需要添加 `attachCustomKeyEventHandler`（例如处理 IME 组合期间的特殊按键），参考实现：

```typescript
term.attachCustomKeyEventHandler((event: KeyboardEvent) => {
  // 只处理 keydown，避免 keypress 重复触发
  if (event.type !== 'keydown') return true;

  // IME 组合期间不拦截任何按键（keyCode=229 或 isComposing=true）
  if (event.isComposing || event.keyCode === 229) {
    return true; // 交由 xterm.js 默认处理
  }

  return true; // 其他按键交由 xterm.js 默认处理
});
```

**关键约束**：
- handler 被 keydown 和 keypress 各触发一次，务必检查 `event.type === 'keydown'`
- `return false` 不阻止浏览器默认行为，需同时 `event.preventDefault()`
- handler 内部的 Terminal ref 必须通过 getter 访问，不能闭包捕获

来源：
- https://xtermjs.org/docs/guides/hooks/
- https://github.com/xtermjs/xterm.js/blob/master/src/browser/input/CompositionHelper.ts
- https://github.com/xtermjs/xterm.js/issues/3801
- https://learn.microsoft.com/en-us/microsoft-edge/webview2/releasenotes/

---

## 7. 焦点事件 (DECSET 1004)

### 7.1 协议机制

DECSET 1004（Focus Event Mode）xterm patch #224 扩展，非原始 DEC VT 特性：

| 序列 | 含义 |
|------|------|
| `ESC[?1004h` | 启用焦点事件报告 |
| `ESC[?1004l` | 禁用焦点事件报告 |
| `ESC[I`（CSI I） | 终端获得焦点 |
| `ESC[O`（CSI O） | 终端失去焦点 |

默认关闭。支持的终端：Windows Terminal、iTerm2、WezTerm、Kitty、Ghostty、Alacritty、tmux（`set -g focus-events on`）。

来源：https://invisible-island.net/xterm/ctlseqs/ctlseqs.html

### 7.2 Claude Code 使用焦点事件

Claude Code 基于 Ink（React-for-CLI 框架）的焦点感知架构：

- `TerminalFocusContext` + `useTerminalFocus()`：终端失焦时**暂停所有动画**、**取消时钟订阅**，降低 CPU 使用
- `useAnimationFrame(callback)`：失焦时 intervalMs 设为 null
- `useBlink(enabled)`：失焦时始终返回可见状态

聚焦事件用于：
- 恢复动画渲染循环
- 从"静默/暗色"状态恢复到正常渲染

已知相关 issues：
- [#36418](https://github.com/anthropics/claude-code/issues/36418)：多代理工作流在失焦标签页/窗格中暂停——Ink React reconciliation 与终端渲染耦合，未聚焦表面渲染被 QoS 降级后状态机冻结
- [#30199](https://github.com/anthropics/claude-code/issues/30199)：Ink focus-out 重渲染触发 iTerm2 活动指示器
- [#11391](https://github.com/anthropics/claude-code/issues/11391)：DECSET 1004 序列泄漏到输入字段（v2.0.67 已修复）

### 7.3 xterm.js 焦点事件支持

**xterm.js 原生支持 DECSET 1004**——VT Features 页面标记为 "Supported"。

工作机制：
1. PTY 端应用写入 `ESC[?1004h` 时，xterm.js 的 InputHandler 追踪 `sendFocus` 模式
2. 浏览器 focus/blur 事件触发时，xterm.js 通过 `onData` 回调生成 `ESC[I]`/`ESC[O`
3. PTY 端写入 `ESC[?1004l` 时停止生成焦点序列

xterm.js v4.6.0 起移除了 `terminal.onFocus()`/`onBlur()` 公共 API（issue #3057），改为推荐通过 DOM 事件在 `terminal.textarea` 上监听：

```js
terminal.onDidAttach(() => {
  terminal.textarea.addEventListener('focus', () => { /* ... */ });
  terminal.textarea.addEventListener('blur',  () => { /* ... */ });
});
```

来源：https://xtermjs.org/docs/api/vtfeatures/

### 7.4 WebView2 环境下的焦点事件

Tauri + WebView2 存在多个已知焦点相关问题：

| 问题 | 状态 |
|------|------|
| `tauri://focus`/`blur` 导致 Windows 应用冻结 | tauri #4533 |
| `WindowEvent::Focused` 在 `App::run` 回调中不触发 | tauri #6460 (需 unstable feature) |
| 拖拽窗口区域导致 focus/blur 快速切换 | tauri #10767 |
| WebView2 无 "unfocus/blur" API | wry discussion #1227 |

但 xterm.js **内部 textarea** 仍能接收 DOM focus/blur 事件——只要 WebView2 将 OS 焦点传递到 DOM 输入元素。因此 DECSET 1004 的焦点序列生成在正常情况下应能工作。

### 7.5 slTerminal 当前状态

**风险等级：低**

- xterm.js 6.0.0 内置支持 DECSET 1004
- `onData` 回调将 `ESC[I]`/`ESC[O` 序列透传到 PTY
- 无需额外代码——Claude Code 发送 `ESC[?1004h` 后，xterm.js 自动处理焦点事件
- WebView2 焦点问题属于 Tauri/Win32 窗口事件层，xterm.js textarea 层级不受影响

**需关注的场景**：分屏/多终端面板时，焦点切换可能触发多个面板的焦点事件——Claude Code 全屏模式正确响应焦点变化以避免多会话混淆。

---

## 8. 多行输入

### 8.1 Claude Code 的多行输入方式

Claude Code 默认行为：**Enter 提交**，**Shift+Enter 插入换行**。其他触发方式：

| 方法 | 适用终端 | 说明 |
|------|----------|------|
| **Shift+Enter** | 支持 Kitty 协议的终端 | 行业标准做法，需 CSI u 支持 |
| **Ctrl+J** | 所有终端，零配置 | 发送 ASCII 0x0A (LF)，通用回退方案 |
| **\\ + Enter** | 所有终端，零配置 | 行末输入 `\` 再按 Enter 即插入换行 |
| **Alt+Enter** | macOS 需配置 | 发送 `ESC + \r`，备选方案 |
| **自定义键位** | `~/.claude/keybindings.json` | 重映射 `chat:newline` / `chat:submit` |

Claude Code 的终端配置工具 `/terminal-setup` 可检测终端能力并推荐最佳多行输入方式。

**tmux 用户**必须在 `~/.tmux.conf` 中添加：
```
set -g allow-passthrough on
set -s extended-keys on
set -as terminal-features 'xterm*:extkeys'
```

来源：https://code.claude.com/docs/en/terminal-config

### 8.2 xterm.js 对 Enter 变体的处理

xterm.js **默认无法区分 Shift+Enter 与普通 Enter**——两者都通过 `onData` 发送 `\r`（0x0D）。这是 ASCII 控制码的历史遗留问题（Enter = Ctrl+M = 0x0D，Shift 无法参与编码）。

| 按键 | xterm.js 6.0.0（无 CSI u） | Kitty 协议（CSI u 启用后） |
|------|---------------------------|---------------------------|
| Enter | `\r`（0x0d） | `ESC[13u` |
| Shift+Enter | `\r`（0x0d，**无法区分**） | `ESC[13;2u`（修饰键位 = 2/Shift） |
| Ctrl+Enter | `\r`（0x0d） | `ESC[13;5u`（修饰键位 = 5/Ctrl） |
| Alt+Enter | `\x1b\r` | `ESC[13;3u`（修饰键位 = 3/Alt） |
| Ctrl+J | `\n`（0x0a） | `ESC[106;5u` |

**Kitty 协议编码解析**：
- `13` = Enter 键 Unicode 码点（CR = 0x0D = 13）
- `2` = `1 + shift_bitmask`（Shift = bit 0）
- `u` = 终止字节 0x75

**xterm.js 6.0.0 的变通方案**：使用 `attachCustomKeyEventHandler`：

```typescript
terminal.attachCustomKeyEventHandler((event: KeyboardEvent) => {
  if (event.key === 'Enter' && event.shiftKey) {
    event.preventDefault();
    terminal.write('\r\n');  // 插入换行，不提交
    return false;
  }
  return true;
});
```

### 8.3 Bracketed Paste 与多行输入

Bracketed Paste Mode（DECSET 2004）是区分"用户粘贴"与"手动输入"的机制：
- 启用：`ESC[?2004h`
- 粘贴内容包裹于：`ESC[200~` ... (粘贴文本) ... `ESC[201~`
- **无括号粘贴**：粘贴 50 行代码 = 触发 50 次提交
- **有括号粘贴**：所有内容作为单次粘贴处理，不触发提交

Claude Code 中：
- 大文本粘贴（>10,000 字符）折叠显示 `[Pasted text]` 占位符
- 图片粘贴检测依赖 bracketed paste 包裹
- 不支持 bracketed paste 的终端使用去抖/合并启发式作为回退

xterm.js 6.0.0 原生支持 DECSET 2004。slTerminal 的 `onData` 透传机制完整支持 bracketed paste——用户粘贴的内容会正确地被 `ESC[200~...ESC[201~` 包裹。

### 8.4 slTerminal 影响分析

**风险等级：中高**

| 多行输入方式 | slTerminal 支持状态 | 优先级 |
|-------------|-------------------|--------|
| Shift+Enter | **不支持**（xterm.js 6.0.0 无 CSI u） | 最高 |
| Ctrl+J | 支持（发送 `\x0a`） | 最高（零配置回退） |
| \\ + Enter | 支持（应用层转义） | 最高（零配置回退） |
| Bracketed Paste | 支持（xterm.js 原生 + onData 透传） | 正常 |

**结论**：
1. **升级 xterm.js 到 6.1.0+ 是解决 Shift+Enter 多行输入的唯一方案**
2. 在升级完成前，用户可通过 `Ctrl+J` 或 `\Enter` 绕过
3. 也可使用 `attachCustomKeyEventHandler` 作为临时补丁（手动检测 Shift+Enter）

---

## 9. 全屏模式 (CLAUDE_CODE_NO_FLICKER=1)

### 9.1 Claude Code 全屏模式启用的特性

`CLAUDE_CODE_NO_FLICKER=1`（Claude Code v2.1.89+, 2026 年 3 月底）启用全屏 TUI 模式，标记为"研究预览"：

| 特性 | 序列 | 作用 |
|------|------|------|
| 备用屏幕缓冲区 | DEC 1049 (`ESC[?1049h/l`) | 独立的绘制区域，退出后恢复主屏幕 |
| 同步更新 | DEC 2026 (`CSI?2026h/l`) | 帧边界同步，消除撕裂 |
| 鼠标事件 | DECSET 1000/1003/1006 | 点击定位光标、滚轮滚动、Ctrl+点击打开链接 |
| 虚拟化 scrollback | 仅渲染可见消息 | 旧消息通过 `Ctrl+O` 抄本模式查看 |

关键行为变化：
- 输入框固定在底部，不再随输出跳动
- 不再写入终端常规 scrollback
- 支持鼠标点击展开/折叠工具结果

来源：https://code.claude.com/docs/zh-CN/fullscreen

### 9.2 DEC 2026 (BSU/ESU) 同步更新

**规范**：
- BSU（Begin Synchronized Update）：`CSI ? 2026 h` -- 开始缓冲渲染
- ESU（End Synchronized Update）：`CSI ? 2026 l` -- 结束缓冲，一次性渲染
- 特性检测：`CSI ? 2026 $ p` 查询，终端回复 `CSI ? 2026 ; 1 $ y`（支持且已设置）

**工作原理**：BSU 后终端停止渲染，将所有网格变更缓存。ESU 后一次性 flush 并绘制完整帧。多次 write 合并为一次 GPU 绘制调用。

**嵌套行为**：安全——多层嵌套 BSU/ESU 正确计数，外层 ESU 才真正渲染。

**终端支持矩阵**：

| 终端 | 支持状态 |
|------|----------|
| **xterm.js 6.0.0** | **内置原生支持**（PR #5453） |
| Kitty | 早期即支持 |
| Ghostty | 1.0.0 起支持 |
| WezTerm | 完整支持 |
| foot | 完整支持 |
| iTerm2 | 已从 DCS 迁至 CSI |
| VS Code 集成终端 | Anthropic 贡献了补丁 |
| xterm (原生) | 安全忽略（不报错） |
| Alacritty | 仍未支持 |

来源：
- https://contour-terminal.org/vt-extensions/synchronized-output/
- https://github.com/xtermjs/xterm.js/issues/5453

### 9.3 xterm.js DEC 2026 支持状态

**结论：已原生支持，无需 addon，开箱即用。**

- xterm.js v6.0.0 通过 PR [#5453](https://github.com/xtermjs/xterm.js/issues/5453) 添加了 DEC 2026 支持
- 核心功能，非 addon，`Terminal` 类型内置
- slTerminal 使用的 xterm.js 6.0.0 **正好命中** DEC 2026 支持的最低版本

因此 Claude Code 全屏模式在 slTerminal 中理论上可以获得 DEC 2026 同步更新的帧同步效果。但需确认：
- `reader_loop` + Channel + IPC + `onData` + xterm.js 的完整管道是否真正利用了 DEC 2026（Ink 的 BSU/ESU 经 ConPTY + reader 线程 + Channel IPC 再到达 xterm.js）
- 管道延迟可能导致 BSU/ESU 的同步窗口被稀释（IPC 序列化开销）

### 9.4 DEC 1049 备用屏幕的 scrollback 行为

**xterm.js 行为**：
- 切换到 alt buffer 时，xterm.js 切换 `BufferSet.activateAltBuffer()`
- alt buffer **不保存 scrollback**——超出屏幕顶部的行被丢弃
- 退出 alt buffer 切回 normal buffer 时，normal buffer 的 scrollback **被保留**
- xterm.js 历史 issue #802（alt buffer 内容泄漏到主 scrollback）已在当前版本修复

**已知风险点**：
- resize 在交替屏幕期间可能破坏主 buffer grid 尺寸
- 分屏/页签切换场景下，alt buffer 状态可能不跨面板一致

**对 slTerminal 的影响**：Claude Code 退出全屏后，之前的终端输出应完整可见。resize 期间需关注——如果 Claude Code 全屏模式中用户调整窗口大小或分屏，需确认主 buffer scrollback 是否被保护。

### 9.5 全屏模式下的鼠标支持

Claude Code 全屏模式使用的鼠标协议：

| DECSET | 模式 | 说明 |
|--------|------|------|
| `9` | X10 鼠标 | 仅按键按下 |
| `1000` | 普通跟踪 | 按下 + 释放 |
| `1003` | 任意事件跟踪 | 所有移动均报告 |
| `1006` | SGR 扩展模式 | 坐标 > 223 |

Claude Code 在 JS 层捕获和处理鼠标事件——点击坐标映射到 UI 元素。xterm.js 通过 `onData` 将鼠标序列传递给 PTY，Claude Code 通过 stdin 读取。

**潜在问题**：ConPTY 隐式鼠标注入（第 5 节）在 alt screen 切换时可能干扰。Claude Code 明确启用鼠标模式（发送 `ESC[?1003h`），ConPTY 看到显式命令后应正确传输所有鼠标事件。

禁用鼠标：`CLAUDE_CODE_DISABLE_MOUSE=1`（完全禁用）或 `CLAUDE_CODE_DISABLE_MOUSE_CLICKS=1`（保留滚轮）

### 9.6 退出全屏后终端状态恢复

DEC 1049 保存/恢复范围：
- 保存：光标位置、光标样式、SGR 属性、屏幕内容
- 不保存：scrollback 内容（在主 buffer 中冻结）、键盘模式、窗口尺寸

Claude Code 退出全屏时发送 `ESC[?1049l`，恢复主屏幕和光标。

**已知风险**：
- [#35580](https://github.com/anthropics/claude-code/issues/35580)：DEC 2026 同步块内 CSI 2J（清屏）导致滚动跳跃
- xterm.js history #2745：GNU screen + alt buffer 退出时光标未正确恢复

**slTerminal 额外风险**：`reader.rs` 的 `match_csi_startup()` 中 `b's'|b'u'` 会剥离 `ESC[s`/`ESC[u`（光标保存/恢复）——如果首轮读取中出现这些序列，会导致光标状态丢失。详细见附录 A1。

### 9.7 slTerminal 全屏模式评估总结

| 特性 | 支持状态 | 风险 |
|------|---------|------|
| DEC 2026 (同步更新) | **已支持**（xterm.js 6.0.0 内置） | 管道延迟可能稀释同步窗口 |
| DEC 1049 (alt screen) | **已支持**（xterm.js 内置） | 分屏 resize 可能破坏主 buffer |
| 鼠标支持 | **已支持**（onData 透传） | ConPTY 隐式鼠标注入需关注 |
| 退出状态恢复 | **支持但需验证** | reader.rs s/u 误剥离; resize 边界 |
| scrollback 完整性 | **当前正确** | xterm.js 历史 issue 已修复 |

**核心风险**：reader.rs 的 `ESC[s`/`ESC[u` 误剥离是已知代码缺陷，虽仅影响首轮读取但应在使用全屏模式前修复。

**建议测试流程**：
1. `export CLAUDE_CODE_NO_FLICKER=1`
2. 启动 claude，观察进入 alt screen 是否平滑
3. 发送一条消息，观察 Ink 渲染是否有闪烁/撕裂
4. 调整窗口大小，观察内容是否错位
5. `Ctrl+D` 或 `/exit` 退出 claude，确认主屏幕恢复完整
6. 检查之前终端内容是否在 scrollback 中可见

---

## 附录：当前 slTerminal 代码发现的问题汇总

### A1. reader.rs 误剥离 ESC[s/ESC[u (SAVE/RESTORE CURSOR)

`src-tauri/src/pty/reader.rs` 第 201 行：

```rust
b's' | b'u' => Some(3),
```

此代码将 `ESC[s`（SCP/Save Cursor Position）和 `ESC[u`（RCP/Restore Cursor Position）标记为 ConPTY 启动序列并剥离。**但这两者不是 ConPTY 启动序列**——它们是标准 VT100 光标保存/恢复命令，任何终端应用都可能使用。

虽然此剥离仅在首轮读取（`startup_drained` 为 false）期间生效，但存在理论风险：如果 shell 的启动输出恰好以 `ESC[s` 开头（虽然极端罕见），会导致光标状态丢失。

**建议**：移除 `b's'|b'u'` 分支。

### A2. TERM/COLORTERM 完全未设置

如第 2 节详述，`shell.rs` 未设置任何终端能力环境变量。

### A3. xterm.js 版本过旧

xterm.js 6.0.0 不支持 Kitty 键盘协议，导致 Shift+Enter 无法区分（影响 Claude Code 多行输入）。

### A4. linkHandler 未配置

OSC 8 超链接点击无响应。

---

## 附录：所有来源

- Kitty keyboard protocol spec: https://sw.kovidgoyal.net/kitty/keyboard-protocol/
- xterm.js #5600 (Kitty keyboard): https://github.com/xtermjs/xterm.js/pull/5600
- xterm.js #4005 (OSC 8): https://github.com/xtermjs/xterm.js/issues/4005
- OSC 8 spec (egmontkob): https://gist.github.com/egmontkob/eb114294efbcd5adb1944c9f3cb5feda
- xterm.js VT Features: https://xtermjs.org/docs/api/vtfeatures/
- Claude Code env vars: https://code.claude.com/docs/zh-CN/env-vars
- Claude Code #36328 (DA1 60s delay): https://github.com/anthropics/claude-code/issues/36328
- Microsoft Terminal PR #17741 (ConPTY DA1): https://github.com/microsoft/terminal/pull/17741
- Microsoft Terminal PR #17833 (ESC drop): https://github.com/microsoft/terminal/pull/17833
- Microsoft Terminal #376 (mouse tracking): https://github.com/microsoft/terminal/issues/376
- xterm.js #4107 (XTGETTCAP): https://github.com/xtermjs/xterm.js/issues/4107
- xterm.js #5790 (lock-key CSI u fix): https://github.com/xtermjs/xterm.js/pull/5790
- xterm.js #4198 (Kitty protocol request): https://github.com/xtermjs/xterm.js/issues/4198
- xterm.js #802 (alt buffer scrollback leak): https://github.com/xtermjs/xterm.js/issues/802
- xterm.js #5453 (DEC 2026 support): https://github.com/xtermjs/xterm.js/issues/5453
- xterm.js Hooks Guide: https://xtermjs.org/docs/guides/hooks/
- Claude Code fullscreen: https://code.claude.com/docs/zh-CN/fullscreen
- Claude Code terminal-config: https://code.claude.com/docs/en/terminal-config
- Claude Code #11391 (DECSET 1004 leak): https://github.com/anthropics/claude-code/issues/11391
- Claude Code #36418 (focus-out pause): https://github.com/anthropics/claude-code/issues/36418
- Claude Code #35580 (DEC 2026 + CSI 2J): https://github.com/anthropics/claude-code/issues/35580
- Contour DEC 2026 spec: https://contour-terminal.org/vt-extensions/synchronized-output/
- Kitty keyboard protocol: https://blog.fsck.com/agent-blog/2026/02/26/terminal-keyboard-protocol/
- WezTerm TERM config: https://wezterm.org/config/lua/config/term.html
- termstandard/colors: https://github.com/termstandard/colors
- xterm CTLSEQS: https://invisible-island.net/xterm/ctlseqs/ctlseqs.html
- terminalguide.namepad.de Primary DA: https://terminalguide.namepad.de/seq/csi_sc/
