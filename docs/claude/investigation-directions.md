# slTerminal 针对 Claude Code CLI 优化 —— 调查方向

## Context

slTerminal 是 Windows 10/11 下的 Tauri 2 终端模拟器（xterm.js + ConPTY）。
Claude Code CLI 是基于 Ink (React-in-terminal) 的复杂 TUI 应用，对其运行的终端有多项能力要求。
当前在 slTerminal 中运行 `claude` 已确认存在以下问题：闪烁/撕裂、颜色异常、resize 画面错位、剪贴板乱码。
需要系统性地调查这些问题的根因并确定优化方向。

---

## 问题 1：流式输出画面闪烁/撕裂

Claude Code 的 Ink 渲染器在流式 LLM 输出时以约 60fps 全帧刷写 ANSI 序列，
管道为：`Ink → ConPTY → reader 线程 → Channel → IPC → rAF 合帧 → xterm.js → WebGL/DOM 渲染`。

### 调查方向

| # | 方向 | 说明 |
|---|------|------|
| 1a | ConPTY 吞吐瓶颈 | Ink 约 189KB/s + 4000-6700 次滚动/秒的输出量，ConPTY 的 VT 解析器是否形成瓶颈？reader 线程 4KB 缓冲区是否够？ |
| 1b | 前端 rAF 合帧策略 | 当前阈值 256 字节——Ink 高频小帧 (~每帧可能 <256B) vs 大帧 (>256B) 的分布比例？阈值是否需要调整？ |
| 1c | IPC Channel 序列化开销 | Tauri IPC Channel 在 WebView2 bridge 上的序列化/反序列化是否引入额外延迟/抖动？ |
| 1d | xterm.js WebGL 渲染器 | WebGL addon 在高频 `term.write()` 下的帧率表现。是否需要启用 DEC 2026 同步更新？ |
| 1e | 页面显隐 WebGL 切换 | 当前 visible=false 时 dispose WebGL addon，切回时重建。重建耗时是否导致可感知白屏？ |
| 1f | 备用屏幕缓冲区 | xterm.js 对 DEC 1049 (alternate screen) 的原生支持是否正常？Claude Code 全屏模式 (`CLAUDE_CODE_NO_FLICKER=1`) 的交替缓冲切换是否触发重绘？ |
| 1g | scrollback 行数 | 当前 5000 行，Ink 长期会话滚动积累对 DOM/WebGL 渲染性能的影响。 |

---

## 问题 2：颜色显示异常（高亮丢失、色偏）

Claude Code 硬编码 24-bit RGB SGR 序列 (`ESC[38;2;R;G;Bm` / `ESC[48;2;R;G;Bm`)。

### 调查方向

| # | 方向 | 说明 |
|---|------|------|
| 2a | `COLORTERM` 环境变量 | spawn 时是否设置了 `COLORTERM=truecolor`？Claude Code 依赖此变量启用 24-bit 颜色。 |
| 2b | ConPTY 24-bit 颜色通过率 | ConPTY 对 `ESC[38;2;...m` / `ESC[48;2;...m` 是否能完整透传？已知 ConPTY 对 256 色/true color 有选择性吞噬。 |
| 2c | xterm.js 主题色表冲突 | 当前 `theme.ts` 定义了 16 色 ANSI 调色板。xterm.js 的 256 色/true color 是否被 ANSI 调色板覆盖？ |
| 2d | `drawBoldTextInBrightColors` | 未显式设置此选项（使用 xterm.js 默认值），Claude Code 的粗体+颜色组合是否受影响？ |
| 2e | 256 色退化 | Claude Code 在某些条件下降级为 256 色模式。slTerminal 的终端能力宣告（`TERM` 变量、颜色深度检测）是否正确？ |
| 2f | SGR 序列组合 | Ink 发出的复杂 SGR 组合（粗体+斜体+颜色+下划线同时存在）是否能被 xterm.js 正确解析？ |

---

## 问题 3：调整窗口大小后画面错位/重叠

已知根因：Ink 在 SIGWINCH 后 Yoga 布局重算完成前就触发重新渲染，相对光标移动 (CUF/CUD) 按旧布局计算，写入新视口外。

### 调查方向

| # | 方向 | 说明 |
|---|------|------|
| 3a | ConPTY SIGWINCH 传递 | `pty_resize` 调用 `master.resize(PtySize)` 后，ConPTY 是否正确向子进程发送 `SIGWINCH`？子进程是否收到？ |
| 3b | resize 时序 | 前端 ResizeObserver (100ms debounce) → `pty.resize()` → ConPTY → 子进程的完整延迟是多少？是否会因 debounce 导致 resize 通知晚于实际渲染帧？ |
| 3c | resize 与 rAF flush 的竞态 | 如果 resize 事件与 rAF flush 同时到达，pendingBufferRef 中的旧尺寸数据是否会以错误的行宽写入终端？ |
| 3d | ConPTY reflow | ConPTY 在 resize 时自身会做 reflow——依赖 `windowsPty.buildNumber` 参数。当前动态获取的 buildNumber 是否准确？reflow 行为是否正确？ |
| 3e | fit 再布局 | `fitAddon.fit()` 后 `proposeDimensions()` 返回的 cols/rows 是否始终有效？NaN 回退的概率？ |
| 3f | 缩小→放大循环 | 用户拖拽分屏（先缩小再放大）时，Claude Code 的 Ink diff 引擎是否会因帧缓冲区尺寸反复变化而累积错位？ |

---

## 问题 4：从 Claude 复制内容到系统剪贴板乱码

Claude Code 使用 OSC 52 序列实现 `/copy` 命令（stdout 写入 base64 编码内容 + terminal 转发到系统剪贴板）。

### 调查方向

| # | 方向 | 说明 |
|---|------|------|
| 4a | OSC 52 前端拦截 | slTerminal 前端是否拦截/解析/转发 OSC 52 序列？当前键盘复制走 `writeText` (Tauri clipboard plugin)，不走 OSC 52。Claude Code 发出的 OSC 52 序列是直接写到 xterm.js 缓冲区（显示乱码）还是需要前端拦截处理？ |
| 4b | ConPTY 编码 | ConPTY 在 UTF-8 ↔ UTF-16 转换过程中，OSC 52 的 base64 负载（含 CJK 字符）是否会因代码页不匹配而损坏？（已知 Windows `/copy` Issue #50605：`clip.exe` + OSC 52 双重写入导致 CJK 乱码） |
| 4c | OSC 52 与 clipboard plugin 协调 | 如果前端拦截 OSC 52 并调用 `writeText`，与现有的 `Ctrl+Shift+C` → `writeText` 路径是否有冲突？ |
| 4d | tmux 透传 | 用户是否在 tmux 内运行 claude？是否需要 DCS 透传包装？ |
| 4e | 括号粘贴 | Claude Code 的括号粘贴模式 (`ESC[200~...ESC[201~`) 是否正常工作？大量文本粘贴 (>10000 字符) 的折叠行为？ |

---

## 其他潜在调查方向（预防性）

以下方向虽非用户当前反馈的问题，但搜索结果和代码探索表明值得关注：

| # | 方向 | 说明 |
|---|------|------|
| 5a | Kitty 键盘协议 (CSI u) | Claude Code 主动 push/pop CSI u 模式以区分 Shift+Enter/Enter。xterm.js 支持但 slTerminal 未显式处理。未被识别的 CSI u 序列是否会影响输入？ |
| 5b | 终端能力宣告 (`TERM`/`COLORTERM`) | spawn 时子进程的环境变量是否正确宣告终端能力？当前 shell-integration.ps1 只设 UTF-8 编码，未设 `TERM`/`COLORTERM`。 |
| 5c | OSC 8 超链接 | Claude Code 输出 Markdown 链接使用 OSC 8。xterm.js 支持但 slTerminal 未加载 `@xterm/addon-web-links`。 |
| 5d | DA1 终端查询 | Claude Code 可能查询 DA1 (`ESC[c]`) 来检测终端类型。ConPTY 吞掉 DA1 会导致 60s 超时。需确认 slTerminal 的 reader 线程是否透传 DA1 响应。 |
| 5e | ConPTY 隐式鼠标注入 | ConPTY 在 DEC 1049 (alternate screen) 切换时隐式启用鼠标跟踪，可能产生幽灵鼠标序列渗入 Claude Code 的 stdin。 |
| 5f | IME 组合与 Ctrl 组合键 | `attachCustomKeyEventHandler` 未实现。Claude Code 的 Ctrl+W (删词)、Ctrl+A/E (行首/尾) 等编辑快捷键是否能正确传递？ |
| 5g | 焦点事件 (DECSET 1004) | Claude Code 需要焦点事件来暂停/恢复动画。xterm.js 支持 `ESC[I`/`ESC[O`，但需确认 WebView2 焦点变化是否正确反映到 xterm.js。 |
| 5h | 多行输入 | Claude Code 支持 `\`+Enter、`Alt+Enter`、`Shift+Enter`、`Ctrl+J` 等多种多行输入方式。xterm.js 对 Shift+Enter 的组合键识别是否正常？`attachCustomKeyEventHandler` 是否需要接管此组合键？Kitty 键盘协议 (CSI u) 对 `Enter` vs `Shift+Enter` 的区分是否被正确透传？ |
| 5i | 全屏模式 (`CLAUDE_CODE_NO_FLICKER=1`) | Claude Code 全屏模式启用备用屏幕缓冲区 (DEC 1049)、DEC 2026 同步更新、鼠标事件。xterm.js 对 DEC 1049 切换是否平滑（无残影/内容泄漏到主缓冲区）？DEC 2026 (BSU/ESU) 是否被 xterm.js 支持？全屏模式 resize 时内容是否会泄漏到 scrollback？全屏模式下的鼠标滚轮/点击是否正常工作？退出全屏后终端状态是否正确恢复？ |

---

## 验证方式

完成调查后需逐一验证：

1. **闪烁/撕裂**：在 slTerminal 中运行 `claude`，触发长文本流式输出，肉眼观察 + 录制帧率
2. **颜色**：对比 slTerminal vs Windows Terminal 中 claude 的语法高亮、diff 着色
3. **resize**：反复拖拽分屏边界，观察 claude 输出是否错位
4. **剪贴板**：claude 内执行 `/copy`，粘贴到外部编辑器验证中文是否乱码
5. **多行输入**：测试 `Shift+Enter`、`Ctrl+J`、`\`+Enter 三种多行方式是否正常工作
6. **全屏模式**：`CLAUDE_CODE_NO_FLICKER=1 claude` 启动，测试全屏渲染、resize、鼠标交互、退出恢复
7. **预防性方向**：逐个检查环境变量宣告、键盘响应、超链接可点击性
