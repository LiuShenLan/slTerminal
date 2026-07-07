# 调查方向三：终端 resize 后画面错位/重叠

> 状态：调查完成 | 日期：2026-07-07

## 确认的根因

**Ink（Claude Code 的 React TUI 引擎）在 SIGWINCH 后 Yoga 布局重算完成前就触发重新渲染。** 渲染器按旧布局（旧行数）计算相对光标移动路径（CUF/CUD），将内容写入已缩小的视口之外，导致画面错位、重叠。详情见 [claude-code#60069][]。

本文档分六个方向逐一分析各环节的机制、时序和可靠性。

[claude-code#60069]: https://github.com/anthropics/claude-code/issues/60069

---

## 1. ConPTY SIGWINCH 传递

### 1.1 Windows 上 SIGWINCH 的本质

Windows 没有内核级 SIGWINCH。ConPTY 通过以下多层机制模拟：

| 机制 | 说明 | 延迟 |
|------|------|------|
| **Signal Pipe**（主路径） | `ResizePseudoConsole` 通过 signal pipe 发送二进制 `ResizeWindow` (type 8) 包到 conhost 的 `PtySignalInputThread`，调用 `ResizeWindow` 更新屏幕缓冲区 | < 1ms |
| **WINDOW_BUFFER_SIZE_EVENT**（遗留路径） | 放入控制台输入缓冲区，Unix 子系统（WSL/MSYS2/Cygwin）将其翻译为 SIGWINCH | 取决于应用是否在读输入 |
| **WinEvent hook**（libuv/Node.js 路径） | `SetWinEventHook(EVENT_OBJECT_LOCATIONCHANGE)` 全局钩子 | 被限流到 ~30Hz |

来源：[microsoft/terminal#15935][]（修复了视口移动时误发 WINDOW_BUFFER_SIZE_EVENT）、[microsoft/terminal#1811][]（WinEvents 不是 ConPTY 兼容性保证的一部分）。

[microsoft/terminal#15935]: https://app.semanticdiff.com/gh/microsoft/terminal/commit/741633ef7ae964a5765914dc181b8eccbec58d68
[microsoft/terminal#1811]: https://github.com/microsoft/terminal/issues/1811

### 1.2 已知可靠性问题

- **空闲时丢事件**：libuv 文档明确警告：如果应用不在主动读 TTY 或移动光标，resize 事件可能被完全错过（[libuv signal docs][]）。
- **启动竞态**：resize 信号可能在 conhost I/O 线程初始化缓冲区之后、确认完成之前到达，导致旧尺寸被应用（[microsoft/terminal#10449][]，已修复）。
- **SSH 放大**：网络延迟将偶发重复 SIGWINCH 放大为级联重绘——一个窗口 resize 触发多次往返协商（[microsoft/terminal#15935][]）。
- **Cygwin 限制**：bash 附加到 `/dev/cons0` 时完全不传播 SIGWINCH；需 `shopt -s checkwinsize` 轮询（[Cygwin SIGWINCH 讨论][]）。

[libuv signal docs]: https://docs.rs/libuv/2.2.0/libuv/handles/signal/struct.SignalHandle.html
[microsoft/terminal#10449]: https://app.semanticdiff.com/gh/microsoft/terminal/commit/1b79cc87c3740c3ca5c26e4a7d7152981dd46412
[Cygwin SIGWINCH 讨论]: https://sourceware.org/pipermail/cygwin/2021-February/247817.html

### 1.3 对 slTerminal 的影响

当前代码 `pty_resize` 调用 `master.resize(PtySize{rows, cols, pixel_width:0, pixel_height:0})`，底层 portable-pty 调用 Win32 `ResizePseudoConsole`。Signal pipe 路径延迟 < 1ms，可靠。但帧渲染延迟（~16-33ms）意味着 resize 实际生效需要 1-2 帧。见第 2 节详细时序分析。

---

## 2. Resize 时序分析

### 2.1 完整延迟链路

```
前端 ResizeObserver 触发
  → 100ms debounce (setTimeout)     ← slTerminal 当前实现
  → fitAddon.fit()                   ← 同步 layout (getBoundingClientRect)
  → proposeDimensions()              ← NaN guard
  → pty.resize(cols, rows)           ← Tauri invoke (IPC)
  → Rust pty_resize()                ← sessions.read() + master.resize()
  → Win32 ResizePseudoConsole        ← ConPTY signal pipe
  → conhost signal thread            ← <1ms
  → conhost I/O thread 完成当前帧    ← ~16-33ms (1-2 frame @ 60Hz)
  → 子进程收到 SIGWINCH              ← 取决于应用（见 1.2）
  → Ink Yoga layout 重算             ← WASM 调用，取决于组件树深度
  → Ink 重新渲染                     ← 相对/绝对光标定位
```

总延迟：**100ms (debounce) + IPC (~1ms) + ConPTY 帧 (~16-33ms) + 子进程响应 (不定) = ~117ms 起**。

### 2.2 100ms debounce 的问题

当前实现用一个 100ms timer 合并 `fit()` 和 `pty.resize()`。这有两个隐患：

1. **debounce 期间输出尺寸错配**：100ms 窗口内 PTY 可能产生大量输出（claude 流式响应），这些输出以旧 cols 渲染到 xterm.js——但此时容器实际已经变大/变小。xterm.js buffer 行宽与实际容器不匹配，表现为文字错位。
2. **不分 X/Y**：VS Code 的最佳实践是 Y（行数）变更立即生效（廉价，~5ms），仅 X（列数）变更 debounce 100ms（昂贵，需 re-wrap 所有行）（[daintree#540][]）。当前实现 X/Y 统一 debounce。

[daintree#540]: https://github.com/daintreehq/daintree/issues/540

### 2.3 debounce 晚于实际渲染帧

`ResizeObserver` 回调在 layout commit 后触发，但 100ms setTimeout 可能在多个渲染帧之后才执行。在此期间，如果用户的 claude 会话正在流式输出，xterm.js 会以**旧尺寸**渲染新到达的 PTY 输出——这些错误的行宽会留在 scrollback 中，即使后续 fit() 修正了尺寸也无法恢复。

---

## 3. Resize 与 rAF flush 的竞态

### 3.1 当前 rAF flush 机制

`useXterm.ts` 中 PTY 输出分两路写入 xterm.js：

- 小块（< 256 字符）→ 直接 `term.write(text)`（同步）
- 大块 → `pendingBufferRef.current.push(text)` → `requestAnimationFrame(flushBuffer)`（异步合帧）

### 3.2 竞态场景

```
时间线：
  t0: ResizeObserver 触发 → 开始 100ms debounce
  t1: PTY 输出大块 → pendingBufferRef.push(oldSizedData) → rAF 排队
  t2: rAF flushBuffer → term.write(oldSizedData)  ← 此时容器已变大，但 xterm.js 未 fit
  t3: debounce timer 触发 → fitAddon.fit()
```

**结果**：t1 到 t3 之间到达的输出，以旧尺寸写入。t3 的 fit 修正后，已写入的旧尺寸内容成为 scrollback 中的永久错位。

两个缓解方向：
- **resize 时清空 pendingBuffer**：在 `fit()` 和 `pty.resize()` 之前 flush 或丢弃 pendingBufferRef 中积累的数据（但这会丢输出）。
- **resize 前先 flush 再 fit**：确保 resize 前的数据以正确的当前尺寸写入，resize 后的数据以新尺寸写入。但 debounce 期间尺寸本身就是过渡态，无法确定"正确"尺寸。

### 3.3 缩小-放大循环的累积

分屏拖拽场景（先缩小再放大，反复多次）下：
- 每次缩小：debounce 100ms → fit + pty.resize（小尺寸）
- 放大的 debounce 触发时 PTY 可能已产生"以小尺寸渲染"的输出
- Ink 在 resize 后可能按旧布局重新渲染了一次（相对 CUD 越界）

反复多次后错位**逐次累积**，因为 Ink 每次 resize 后 emit 的 ANSI 序列都是以"当时"的（可能已过时的）行为基础计算。

---

## 4. ConPTY Reflow 行为

### 4.1 默认行为 vs PSEUDOCONSOLE_RESIZE_QUIRK

| 行为 | 无 QUIRK (默认) | 有 QUIRK (0x2) |
|------|----------------|----------------|
| resize 时重绘整个 buffer | 是 | 否 |
| 快速 resize 产生重复行 | 可能 | 避免 |
| scrollback 被破坏 | 可能被覆盖 | 保留 |
| resize 后光标同步 | 始终同步 | 可能漂移 |
| 适用终端 | 所有终端（兼容模式） | 仅 quirk-aware 终端 |

来源：[微软 Terminal 提交 f2d1d95][]，[microsoft/terminal#16911][]。

[微软 Terminal 提交 f2d1d95]: https://app.semanticdiff.com/gh/microsoft/terminal/commit/f2d1d9505540fe78276297589937963698a66b05
[microsoft/terminal#16911]: https://github.com/microsoft/terminal/issues/16911

**默认行为（无 QUIRK）对使用相对光标定位的 TUI（如 Ink/Claude Code）是灾难性的**：ConPTY 在 resize 时大量重放历史 buffer 内容，Ink 的 output.ts diff 引擎会将这些重放内容当作新帧 diff，产生叠加/重复输出。

### 4.2 当前 slTerminal 的 ConPTY flag

当前 `spawn.rs` 中使用 `portable_pty::native_pty_system()` 创建 PTY，**未设置任何自定义 ConPTY flag**（无 RESIZE_QUIRK，无 PASSTHROUGH_MODE）。

对比 wezterm 的做法：
```rust
// wezterm: 显式设置 RESIZE_QUIRK + WIN32_INPUT_MODE
PSEUDOCONSOLE_RESIZE_QUIRK | PSEUDOCONSOLE_WIN32_INPUT_MODE
```

当前使用默认 flag（= 0），意味着 ConPTY 在 resize 时会**重绘整个 buffer**。这对 claude 使用的 Ink TUI 会产生 resize 后内容重复/叠加的问题。

### 4.3 Build Number 依赖

当前 slTerminal 通过 `RtlGetNtVersionNumbers` 获取真实 build 号（`build.rs`），准确性没问题。

但该 build 号仅传给前端（用于 `windowsBuildNumber` prop），**未用于 ConPTY flag 选择**。PASSTHROUGH_MODE 需要 build >= 22621（Win11 22H2），可选择性启用。

### 4.4 缩小→放大截断问题

ConPTY 从 24 行 resize 到 50 行时，`ResizePseudoConsole` 会重放约 50 行 reflowed 内容——如果终端之前有 50 行内容在 24 行宽的 buffer 中，reflow 后可能超过 50 行，ConPTY 会**截断**超出部分（[microsoft/terminal#16879][]）。这意味着缩小→放大的循环中，内容可能被逐次裁剪。

[microsoft/terminal#16879]: https://github.com/microsoft/terminal/discussions/16879

---

## 5. xterm.js fit 再布局可靠性

### 5.1 proposeDimensions() 的 NaN 问题

xterm 5.1.0 回归：`proposeDimensions()` 返回 `{cols: NaN, rows: NaN}`，因为 `_renderService.dimensions.actualCellWidth` 在渲染器未初始化时为 `undefined`，`Math.floor(availableWidth / undefined)` 产生 `NaN`（[xtermjs#4338][]）。

**当前 slTerminal 已有 NaN guard**（`useXterm.ts:341`）：
```typescript
if (dims && Number.isFinite(dims.cols) && Number.isFinite(dims.rows)) {
  doSpawn(dims.cols, dims.rows);
  return;
}
```
但 **ResizeObserver 回调中的 NaN guard 不完整**（`useXterm.ts:384`）：
```typescript
if (dims && sessionIdRef.current) {
  pty.resize(sessionIdRef.current, dims.cols, dims.rows)
```
这里 `dims` 为真但 `dims.cols` / `dims.rows` 可能是 NaN，会传给后端 `pty.resize(NaN, NaN)` → Rust `PtySize { rows: NaN as u16, cols: NaN as u16 }`。需补 `Number.isFinite` 检查。

[xtermjs#4338]: https://github.com/xtermjs/xterm.js/issues/4338

### 5.2 fitAddon.fit() 的同步 layout 开销

`fitAddon.fit()` 内部调用 `container.getBoundingClientRect()`——强制同步 layout（forced reflow）。在窗口拖拽/分屏拖拽时，ResizeObserver 每帧触发，每次 fit 都同步读 DOM，造成 UI 卡顿。

推荐改进：用 `ResizeObserver` 的 `entry.contentRect` 推送尺寸（push model），用 `terminal.resize(cols, rows)` 替代 `fitAddon.fit()` 避免同步 DOM 读（[daintree#847][]）。

[daintree#847]: https://github.com/daintreehq/daintree/issues/847

### 5.3 WebGL 渲染器 resize 竞态

xterm.js 的 WebGL addon 在 resize 时涉及 canvas 重设尺寸 + GPU 纹理重分配，比 DOM 渲染器慢。如果 resize 与 WebGL context loss 同时发生，`webglAddon.dispose()` 可能在 terminal.dispose() 期间重新创建 DOM 渲染器——访问已半销毁状态（[xtermjs#5181][]）。

当前 slTerminal 在 `onContextLoss` 中 dispose webglAddon 并回退 DOM 渲染器，但未专门处理 resize 期间的 context loss 竞态。

[xtermjs#5181]: https://github.com/xtermjs/xterm.js/issues/5181

---

## 6. 缩小-放大循环中 Ink diff 引擎的累积错位

### 6.1 Claude Code Ink 的光标定位策略

Claude Code 的 Ink fork 使用 **frame diffing**（`output.ts`）来最小化 ANSI 输出。渲染流程：

1. React reconciler commit → `resetAfterCommit`
2. `calculateLayout()` → Yoga WASM 计算所有节点的 (x, y, width, height)
3. `renderNodeToOutput()` → 遍历 Yoga 树构建输出 buffer（二维字符网格）
4. `output.ts` → 比较新旧 frame，仅 emit 差异部分的 ANSI 序列

关键发现（[claude-code#60069][] 中的 byte-stream 分析）：

> SIGWINCH 后 emit 的 ANSI 序列中**只有相对移动**（CUF = Cursor Forward, CUD = Cursor Down），**零绝对定位**（CUP = Cursor Position, HVP）。

```ansi
\x1b[2C  \x1b[1B  \x1b[7C  \x1b[1B  \x1b[3C  \x1b[1B  \x1b[2C  \x1b[1B
  CUF 2    CUD 1    CUF 7    CUD 1    CUF 3    CUD 1    CUF 2    CUD 1
```

累积 CUD 次数 = 旧 frame 的行数。如果旧 frame 40 行，新视口 23 行，累积 CUD 40 次——最后的 17 次 CUD 将光标移到视口以下，后续输出全部写在可见区域之外。

### 6.2 错位累积机制

缩小→放大循环中：

1. **缩小**（40 行 → 23 行）：Ink SIGWINCH handler 触发，但 Yoga layout 可能尚未完成 → 渲染器按 40 行 frame 计算 CUD → 输出越界 → 视觉错位
2. **放大**（23 行 → 40 行）：Ink 再次收到 SIGWINCH，Yoga 重算 → 渲染 23 行 frame → CUD 23 次 → 但此时视口 40 行，内容偏上
3. **反复多次**：每次 resize 都带来一次"基于旧尺寸计算的光标移动"，误差逐次叠加

Ink 的 `output.ts` frame diff 机制加剧了问题：**只有变化区域 emit ANSI**，不会全量重绘。这意味着错位后无自动修复——除非 claude 主动发出全量 repaint（`Ctrl+L` 无效因为 corruption 在 Ink 的虚拟 buffer 而非终端 buffer）。

### 6.3 为什么 Ctrl+L 无效

Ctrl+L 发送 form feed / clear screen 到 PTY，但：
- Ink 渲染器的内部 buffer（前/后 frame）不知道终端被清了
- Ink 的 frame diffing 仍然做增量更新，不会发出全量重绘
- 只有 claude 的状态变更（新数据到达）才触发新的 render cycle

这解释了 [claude-code#43273][]（KVM 切换后 TUI 空白）和 [claude-code#29937][]（tmux 中文本重叠）——两者都是 Ink 内部虚拟 buffer 与实际终端状态同步丢失，且无自动重绘机制。

[claude-code#43273]: https://github.com/anthropics/claude-code/issues/43273
[claude-code#29937]: https://github.com/anthropics/claude-code/issues/29937

### 6.4 社区缓解方案

npm 包 [claudefix][] 在 PTY 包装层做三个缓解：
1. debounce SIGWINCH 事件（防 resize 风暴 crash 渲染器）
2. 周期性 clear scrollback（`\x1b[3J`）
3. 剥离 VTE 终端上有问题的 ANSI 背景色

但这些都是治标不治本。根因修复在 Ink 渲染层。

[claudefix]: https://www.npmjs.com/package/claudefix

### 6.5 可行的修复方向

按影响从大到小排列：

| # | 方案 | 作用层 | 难度 |
|---|------|--------|------|
| 1 | **clamp 累积相对移动**在 emit 时，累计 deltaY >= viewportRows 时回退到 CUP 绝对定位 | Ink output.ts | 低 |
| 2 | **SIGWINCH 后先 emit CUP home**（`\x1b[1;1H`），重置光标后再做相对移动 | Ink renderer | 低 |
| 3 | **await Yoga layout 完成后再排程渲染**——当前 `resetAfterCommit` 中 layout 和 render 都触发但未同步 | Ink reconciler | 中 |
| 4 | **debounce SIGWINCH 事件**——500ms 内仅处理最后一次 resize，避免中间态渲染 | Ink 输入层 | 低 |

方案 1+2 组合可以最低成本解决大部分问题。方案 3 是根本修复但改动最深。

来源：[claude-code#60069][] 评论区中的 fix proposal。

---

## 7. 对 slTerminal 的具体建议

基于以上六个方向的调查，slTerminal 可采取的改进：

### 7.1 启用 PSEUDOCONSOLE_RESIZE_QUIRK

当前默认 flag (=0) 导致 ConPTY 在 resize 时重放整个 buffer。对使用 Ink 的 claude 来说，这些重放内容会被 Ink 当作新帧 diff，造成叠加/重复。启用 RESIZE_QUIRK 可以消除 ConPTY 端的 buffer 重放。

```rust
// spawn.rs 中传递自定义 conpty flag
// 注意：需要 portable-pty 暴露 conpty_flags 接口，或使用底层 Win32 API
const RESIZE_QUIRK: u32 = 0x2;
// 若 build >= 22621，可选加 PASSTHROUGH_MODE (0x8)
```

### 7.2 分离 X/Y resize debounce

将当前统一的 100ms debounce 改为：Y（行数）立即 resize，X（列数）100ms debounce。参考 VS Code 的 `TerminalResizeDebouncer`。

### 7.3 补全 ResizeObserver 中的 NaN guard

```typescript
// useXterm.ts:384 — 当前:
if (dims && sessionIdRef.current) {
// 应补:
if (dims && Number.isFinite(dims.cols) && Number.isFinite(dims.rows) && sessionIdRef.current) {
```

### 7.4 resize 时 flush pendingBuffer

在 `fit()` 和 `pty.resize()` 之前调用 `flushBuffer()` 确保旧尺寸数据不出现在新视口中。但这可能导致短暂闪烁（数据先 flush 再 fit 再等新数据）。

### 7.5 resize 时不下发 pixel_width/pixel_height

当前 `pty_resize` 传 `pixel_width: 0, pixel_height: 0`——这是对的。ConPTY 不需要像素尺寸，仅 cols/rows 有意义。保持不变。

### 7.6 考虑 resize 后全量刷新

对于 claude 类的 TUI 应用，可以在 resize 后让 xterm.js 做一次 `term.clear()` + `term.reset()`，迫使 claude 重新绘制。但这会丢失 scrollback 且打断用户操作，仅适合作为 fallback 策略。

---

## 信息来源

- [claude-code#60069][] — Ink resize 渲染竞态根因分析（byte-stream evidence + fix proposal）
- [claude-code#55762][] — 缩小后"压扁"输出不恢复
- [claude-code#52304][] — Zellij 中 SIGWINCH 放大触发重复 transcript
- [claude-code#43273][] — KVM 切换后 TUI 空白
- [claude-code#29937][] — tmux 中文本重叠
- [microsoft/terminal#16911][] — PSEUDOCONSOLE_RESIZE_QUIRK 文档
- [microsoft/terminal#16879][] — ResizePseudoConsole 缩小→放大截断行
- [microsoft/terminal#15935][] — 修复视口移动时误发 WINDOW_BUFFER_SIZE_EVENT
- [microsoft/terminal#10449][] — 修复启动时 resize 竞态
- [微软 Terminal 提交 f2d1d95][] — RESIZE_QUIRK flag 引入
- [xtermjs#4338][] — proposeDimensions 返回 NaN
- [xtermjs#5181][] — WebGL disposal 竞态
- [daintree#540][] — VS Code 风格 resize debounce（X/Y 分离）
- [daintree#847][] — 单点 resize 权威 + push model
- [libuv signal docs][] — Windows 上 SIGWINCH 不可靠性
- [Cygwin SIGWINCH 讨论][] — Cygwin 中 SIGWINCH 不传播
- [wezterm conpty flags][] — RESIZE_QUIRK + WIN32_INPUT_MODE 的 wezterm 用法
- [DeepWiki: Ink Rendering Engine][] — Ink 渲染管线（Yoga layout + output diff）
- [DeepWiki: Flexbox Layout with Yoga][] — Yoga calculateLayout 时序
- [claudefix][] — 社区 PTY 包装缓解方案

[claude-code#55762]: https://github.com/anthropics/claude-code/issues/55762
[claude-code#52304]: https://github.com/anthropics/claude-code/issues/52304
[wezterm conpty flags]: https://github.com/wezterm/wezterm/blob/4f123a461bffdd8ac2e21c0697d02c5c992abd29/pty/src/win/psuedocon.rs#L51
[DeepWiki: Ink Rendering Engine]: https://deepwiki.com/Sachin1801/claude-code/4.1-ink-rendering-engine
[DeepWiki: Flexbox Layout with Yoga]: https://deepwiki.com/vadimdemedes/ink/6.1-flexbox-layout-with-yoga
