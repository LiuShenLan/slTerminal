# 调查 4：从 Claude Code 复制内容到系统剪贴板乱码

## 概览

Claude Code `/copy` 命令通过 OSC 52 序列实现剪贴板写入。当前 slTerminal 对整个 OSC 52 管道无任何拦截或处理——序列从 ConPTY 经 reader 线程、IPC Channel 直通 xterm.js，xterm.js 核心解析器虽识别但不执行剪贴板操作。结果：Claude Code 的 `/copy` 对 slTerminal 用户完全无效，剪贴板无任何更新。

## 数据流全景

```
Claude Code (Ink/TUI)
  │ setClipboard(text):
  │   路径 A: copyNative(text) → PowerShell Set-Clipboard (fire-and-forget)
  │   路径 B: OSC 52 → process.stdout
  │
  ▼ OSC 52 序列: ESC ] 52 ; c ; <base64-UTF8> BEL
ConPTY (Windows Pseudo Console)
  │ 透传（非启动注入序列，不被 strip_conpty_startup 剥离）
  │ 风险: ConPTY OSC 重排 (microsoft/terminal#17314，未解决)
  │
  ▼ raw bytes (4096-byte chunks)
PTY Reader 线程 (reader.rs/reader_loop)
  │ 首轮: strip_conpty_startup() 仅剥离 OSC 0/2（窗口标题），OSC 52 不受影响
  │ 后续: 全部原样透传，无任何过滤或解析
  │
  ▼ PtyEvent::Output { bytes: Vec<u8> }
Tauri IPC Channel (WebView2 bridge)
  │ 序列化/反序列化 — 字节保真
  │
  ▼ Uint8Array → TextDecoder.decode()
前端 handlePtyOutput (useXterm.ts:171-202)
  │ text < 256 bytes → term.write(text) 立即写入
  │ text >= 256 bytes → rAF 合帧后 term.write()
  │
  ▼
xterm.js 6.0 核心解析器
  │ 内建 OSC 52 handler: parser.registerOscHandler(52, ...)
  │ → manipulateSelectionData(data) — 仅发出事件，无后端时静默丢弃
  │ → 不写剪贴板，不显示乱码（序列被 '已处理' 标记消费）
  │
  └─ 结果: OSC 52 数据完全丢失
```

## 当前 slTerminal 剪贴板现状

### 现有复制路径（唯一有效路径）

| 触发方式 | 流程 | 终点 |
|----------|------|------|
| Ctrl+Shift+C | `term.getSelection()` → `writeText(selection)` | Tauri clipboard plugin → arboard → Win32 `SetClipboardData(CF_UNICODETEXT)` |
| Ctrl+Shift+V | `readText()` → `term.paste(text)` | xterm.js 粘贴处理 + 括号粘贴自动包裹 |

### xterm.js 配置分析

```typescript
// src/panels/terminal/theme.ts
export const terminalOptions: ITerminalOptions = {
  allowProposedApi: true,  // ← 启用，但未使用任何 proposed API
  // ... 其他配置
};
```

- `allowProposedApi: true` 已启用——使 `parser.registerOscHandler()` 可用
- **无任何 OSC handler 注册**：全文搜索 `registerOscHandler`/`registerCsiHandler`/`registerLinkProvider` 返回零结果
- **未加载 `@xterm/addon-clipboard`**：package.json 中无此依赖，npm registry 中该包名为 `@xterm/addon-clipboard`
- **未加载 `@xterm/addon-web-links`**：无 OSC 8 超链接支持

### 现有依赖

| 包 | 版本 | 用途 |
|----|------|------|
| `@xterm/xterm` | ^6.0.0 | 核心终端模拟器 |
| `@xterm/addon-fit` | ^0.11.0 | 自适应容器尺寸 |
| `@xterm/addon-webgl` | ^0.19.0 | GPU 渲染 |
| `@tauri-apps/plugin-clipboard-manager` | ^2.3.2 | 系统剪贴板读写 |

---

## 调查方向详细分析

### 4a. OSC 52 前端拦截

**问题**：xterm.js 6.0+ 核心解析器内建 OSC 52 支持（PR [#4220](https://github.com/xtermjs/xterm.js/pull/4220)），在 `InputHandler` 中注册：

```typescript
this._parser.registerOscHandler(52, new OscHandler(data => this.manipulateSelectionData(data)));
```

`manipulateSelectionData` 解析 OSC 52 payload（`<clipboard-selector>;<base64-data>`），发出事件，返回 `true` 表示序列已处理。**无 `@xterm/addon-clipboard` addon 或自定义 handler 时，数据静默丢弃，不写入剪贴板，也不在屏幕上显示乱码。**

这意味着：
1. Claude Code 的 OSC 52 序列到达 xterm.js 后被消费但不产生效果
2. Claude Code 的 `copyNative()` 路径（PowerShell `Set-Clipboard`）独立运行在 PTY 子进程侧，若成功则剪贴板有内容，但该路径历史上存在编码问题
3. 用户在 slTerminal 中用 `/copy` 后剪贴板无内容（或内容乱码来自 `copyNative` 路径的编码问题）

**解决方案**：加载 `@xterm/addon-clipboard` 并提供自定义 `IClipboardProvider`，将 OSC 52 桥接到 Tauri 剪贴板插件：

```typescript
import { ClipboardAddon, IClipboardProvider, ClipboardSelectionType } from "@xterm/addon-clipboard";
import { writeText, readText } from "../../ipc/clipboard";

const clipboardProvider: IClipboardProvider = {
  readText(selection: ClipboardSelectionType): Promise<string> {
    // OSC 52 读请求 — 出于安全考虑，返回空字符串
    if (selection === "c") return readText();
    return Promise.resolve("");
  },
  writeText(selection: ClipboardSelectionType, text: string): Promise<void> {
    if (selection === "c") return writeText(text);
    return Promise.resolve();
  },
};

const clipboardAddon = new ClipboardAddon(clipboardProvider);
term.loadAddon(clipboardAddon);
```

或更轻量的方案——直接在 `useXterm.ts` 中注册自定义 OSC handler：

```typescript
term.parser.registerOscHandler(52, (data: string) => {
  // data 格式: "c;<base64-encoded-text>"
  const semicolonIdx = data.indexOf(";");
  if (semicolonIdx === -1) return true; // 无效格式，吞噬

  const selector = data.substring(0, semicolonIdx);
  const payload = data.substring(semicolonIdx + 1);

  // 仅处理系统剪贴板 (c) 和空选择器 (默认)
  if (selector && selector !== "c") return true;

  // "?" 为查询请求 — 出于安全不响应
  if (payload === "?") return true;

  try {
    const decoded = atob(payload);
    writeText(decoded).catch((err) =>
      console.error("[OSC52] 写剪贴板失败:", err),
    );
  } catch {
    // base64 无效，忽略
  }
  return true; // 返回 true = 序列已处理，不写入终端显示
});
```

**两种方案对比**：

| 维度 | `@xterm/addon-clipboard` | 自定义 `registerOscHandler` |
|------|--------------------------|---------------------------|
| 依赖 | 需安装 npm 包 | 零依赖，xterm.js 6.0+ 内建 API |
| base64 校验 | addon 内置严格校验 | 需自行实现 |
| 查询响应 | 内置 `?` 响应 | 需自行处理 |
| 1MB 限制 | 内置限制（可配置） | 无限制 |
| 安全控制 | Provider 接口清晰 | 完全自主控制 |
| 推荐 | 如需完整 OSC 52 实现 | **推荐** — 更轻量，slTerminal 只需写剪贴板 |

**信息来源**：
- [xterm.js PR #4220 — ANSI OSC52 支持](https://github.com/xtermjs/xterm.js/pull/4220)
- [xterm.js PR #4863 — OSC 52 剪贴板操作](https://github.com/xtermjs/xterm.js/pull/4863)
- [xterm.js Parser Hooks 文档](https://xtermjs.org/docs/guides/hooks/)
- [daintree Issue #3703 — OSC 52 安全讨论](https://github.com/daintreehq/daintree/issues/3703)

---

### 4b. ConPTY 编码

**Base64 字符集**（A-Z、a-z、0-9、+、/、=）全部为 ASCII（0x2B-0x7A），在 UTF-8 中为单字节且与 ASCII 一致。**Base64 负载本身在 UTF-8 ↔ UTF-16 转换中不会被损坏。**

但存在以下真实损坏风险：

#### 风险 1：ConPTY OSC 序列重排（未解决）

[microsoft/terminal#17314](https://github.com/microsoft/terminal/issues/17314)：ConPTY 重新解析和序列化 ANSI 转义序列。OSC 标题设置命令会从文本中提取并移到输出末尾。如果 OSC 52 与其他文本输出交错，序列可能被延迟或完全错位。

> 输入：`"hello <OSC 52 clipboard> world"`
> ConPTY 输出可能变为：`"hello world <OSC 52 clipboard>"` 或更糟

**影响**：如果 Claude Code 在流式输出中嵌入 OSC 52（非独占行），ConPTY 重排可能破坏序列。

#### 风险 2：clip.exe 编码不匹配（历史问题，已修复）

Claude Code v2.1.89-v2.1.97 期间，`copyNative()` 使用 `clip.exe`（使用系统 OEM 代码页，如 CP936/GBK），与 OSC 52 的 UTF-8 正确写入并行执行。`clip.exe` 启动慢，后到达的乱码覆盖 OSC 52 的正确内容。

- **v2.1.97 修复**：Windows 改用 `PowerShell Set-Clipboard`（UTF-8 感知）
- **WSL2 仍存问题**：[#50605](https://github.com/anthropics/claude-code/issues/50605) — OSC 52 路径在 WSL2 的 ConPTY 层仍可能因 CP932 转换损坏

#### 风险 3：ConPTY 换行注入

当 PTY 宽度小于 OSC 52 序列长度时，ConPTY 在字符单元边界注入 `\r\n` 换行符。这会断开行尾的 base64 负载（base64 中不含换行符，任何换行注入都将破坏解码）。

**缓解**：确保 PTY 宽度足够大（>= 标准 80 列），OSC 52 序列不会被换行。Claude Code 通常将 OSC 52 作为独占行输出，不会与其他文本混排。

#### 风险 4：非阻塞 PTY EAGAIN 截断

[Neovim #26688](https://github.com/neovim/neovim/issues/26688) 记录：OSC 52 负载超过约 1024 字节时，`write()` 可能因 PTY 缓冲区满返回 `EAGAIN`，只部分写入。slTerminal 使用 `portable-pty`，其 Windows 实现使用阻塞写入——此风险较低。

#### 风险 5：DA1 报告缺失

Windows Terminal 仅在 `allowOSC52=true` 时在 DA1 响应中包含 `52` 扩展。ConPTY 本身不透传 DA1 查询结果——如果 Claude Code 通过 DA1 检测 OSC 52 支持，可能因 slTerminal 不响应 DA1 而被误判为不支持，导致 Claude Code 走其他（可能有问题）的路径。

**结论**：ConPTY 层面的主要风险是序列重排（microsoft/terminal#17314）和过时 `clip.exe` 双写。Base64 负载本身不会被 UTF-8/UTF-16 转换损坏。

**信息来源**：
- [microsoft/terminal#17314 — ConPTY OSC 重排](https://github.com/microsoft/terminal/issues/17314)
- [microsoft/terminal#4116 — ConPTY 转义序列截断](https://github.com/microsoft/terminal/issues/4116)
- [WezTerm #3624 — ConPTY 吞噬未知序列](https://github.com/wezterm/wezterm/issues/3624)
- [Eclipse CDT Bug 573796 — ConPTY 始终 UTF-8](https://bugs.eclipse.org/bugs/show_bug.cgi?id=573796)
- [Neovim #26688 — OSC 52 EAGAIN 截断](https://github.com/neovim/neovim/issues/26688)

---

### 4c. OSC 52 与剪贴板插件协调

#### 两条路径对比

| 维度 | 现有 Ctrl+Shift+C | 拟议 OSC 52 handler |
|------|-------------------|---------------------|
| 触发方 | 用户键盘快捷键 | Claude Code stdout 输出 |
| 数据源 | xterm.js 选区 | OSC 52 base64 payload |
| 写入口 | `writeText()` (Tauri) | `writeText()` (Tauri) |
| 焦点要求 | 终端面板聚焦 | 无（终端输出异步到达） |
| 安全上下文 | 用户主动操作 | 无用户手势 |

两条路径在 `writeText()` 汇聚——Tauri 剪贴板插件通过 `arboard` 库直接走 Win32 `SetClipboardData(CF_UNICODETEXT)`，不存在编码问题（UTF-16）。

#### 潜在冲突

1. **竞态覆盖**：用户在 Claude Code 输出 OSC 52 的同时按 Ctrl+Shift+C 复制选区——最后到达 `writeText()` 的调用者获胜。概率极低（OSC 52 在 Claude Code 响应完成后才发出，用户此时通常不复制）。

2. **安全检查缺失**：OSC 52 handler 不区分 Claude Code 和任意 `curl attacker.com` 输出。任意写入 stdout 的进程都能通过 OSC 52 设置系统剪贴板。**缓解**：可限制 OSC 52 仅处理焦点终端面板的输出（检查 `document.hasFocus()` 或面板可见状态）。但这可能破坏 tmux/SSH 场景（用户焦点可能在别处）。

3. **多次写入**：如果 Claude Code 并行调用 `copyNative` 和 OSC 52，两条路径都会到达 `writeText()`——但内容相同（都是正确的 UTF-8 文本），后者覆盖前者无影响。

#### 推荐的协调策略

- OSC 52 写入无需用户确认（与 Windows Terminal 行为一致——`allowOSC52` 默认 true）
- 不响应 OSC 52 读请求（`Pd = ?`）——安全考虑
- 可选：焦点检查门控（仅活跃面板的 OSC 52 生效）
- 可选：视觉反馈（短暂 toast "已复制到剪贴板"）

**信息来源**：
- [Tauri Clipboard Plugin 文档](https://tauri.app/plugin/clipboard/)
- [arboard (Tauri 使用的 Rust 剪贴板库)](https://github.com/rerun-io/arboard)
- [Windows Terminal PR #18449 — allowOSC52](https://github.com/microsoft/terminal/pull/18449)

---

### 4d. tmux 透传

#### Claude Code 的 tmux 检测与 DCS 包装

Claude Code 在检测到 `$TMUX` 环境变量时，自动将 OSC 52 包装为 DCS 透传序列：

```
原始 OSC 52:  ESC ] 5 2 ; c ; <base64> BEL

tmux DCS 包装: ESC P t m u x ; ESC ESC ] 5 2 ; c ; <base64> BEL ESC \
               \___________/  \__________________________________/  \___/
                DCS 引入符      内部 ESC 加倍后的 OSC 52           DCS ST
```

**字节级格式**：
| 字节序列 | 含义 |
|----------|------|
| `\x1b P t m u x ;` | DCS 引入符 `ESC P` + `tmux;` 前缀 |
| `\x1b \x1b` | OSC 52 本身的 `ESC` 被加倍（模板中的第一个 `\x1b` 是 DCS 前缀延续，第二个是 OSC 开始的 `\x1b` 的加倍） |
| `] 5 2 ; c ; <base64>` | OSC 52 命令与 base64 负载 |
| `\x07` 或 `\x1b \\` | OSC 终止符 |
| `\x1b \\` | DCS 终止符（ST） |

#### 关键 tmux 配置

```bash
# ~/.tmux.conf — 必须配置的两项
set -g allow-passthrough on       # 允许 DCS 包装的序列穿透 (tmux 3.3+, 默认 off)
set -g set-clipboard on           # 处理原始 OSC 52 + 同步 tmux buffer
```

`allow-passthrough` 是服务端选项，默认关闭。关闭时 DCS 序列被静默丢弃——无错误、无剪贴板更新。

#### 嵌套 tmux

每一层嵌套需加倍所有 ESC：

```
级别 1:  \x1bPtmux;\x1b\x1b]52;c;<b64>\x07\x1b\\
级别 2:  \x1bPtmux;\x1b\x1bPtmux;\x1b\x1b\x1b\x1b]52;c;<b64>\x07\x1b\x1b\\\x1b\\
```

#### 对 slTerminal 的影响

如果用户在 slTerminal 中运行 tmux，tmux 内部的 Claude Code 会发出 DCS 包装的 OSC 52。此序列经 ConPTY → slTerminal reader → xterm.js。**xterm.js 不识别 DCS `tmux;` 前缀**——序列不会自动解开。

**slTerminal 需要额外处理 DCS 包装的 OSC 52**。可通过以下方式实现：

1. 在 OSC 52 handler 之前，注册 DCS handler 检测 `tmux;` 前缀
2. 或在 OSC handler 中同时检测原始 OSC 52 和 DCS 包装的 OSC 52

DCS handler 注册方式（xterm.js 6.0+）：

```typescript
term.parser.registerDcsHandler({ prefix: "", final: "" }, (data, params) => {
  // 检测 tmux; 包装的 OSC 52
  return false; // 返回 false 交给默认处理
});
```

但实际上，更简单的方案是在 `handlePtyOutput` 层正则匹配并预处理 DCS 包装——在 `term.write()` 之前解包。

**信息来源**：
- [tmux Clipboard Wiki](https://github.com/tmux/tmux/wiki/Clipboard)
- [tmux FAQ: passthrough escape sequence](https://github.com/tmux/tmux/wiki/FAQ#what-is-the-passthrough-escape-sequence-and-how-do-i-use-it)
- [tmux #4214 — allow-passthrough 文档遗漏](https://github.com/tmux/tmux/issues/4214)
- [Claude Code #50605 — WSL2 OSC 52](https://github.com/anthropics/claude-code/issues/50605)
- [lazygit PR #3597 — tmux OSC 52 示例](https://github.com/jesseduffield/lazygit/pull/3597)

---

### 4e. 括号粘贴

#### xterm.js 原生支持

括号粘贴模式（`ESC[200~` 开始，`ESC[201~` 结束）**已内建于 xterm.js 核心**，无需额外 addon。提交 [1dbcf70](https://github.com/xtermjs/xterm.js/commit/1dbcf70cee9ae88c69cf9724745cbdb7e5364dcc) 添加了此支持。

xterm.js 5.3.0 新增 `ignoreBracketedPasteMode` 选项：

```typescript
const term = new Terminal({
  ignoreBracketedPasteMode: true, // 即使 shell 请求，也强制关闭括号粘贴
});
```

slTerminal 当前**未设置**此选项（使用 xterm.js 默认值 false），意味着括号粘贴正常工作。

#### 工作流程

1. Shell/应用发送 `ESC[?2004h` → xterm.js 设置 `modes.bracketedPasteMode = true`
2. 用户 Ctrl+Shift+V 粘贴 → xterm.js 自动包裹 `ESC[200~...paste text...ESC[201~`
3. PTY 收到带括号的粘贴文本 → Shell/Claude Code 将粘贴与手动输入区分

#### Claude Code 使用方式

- **多行输入**：通过括号粘贴区分粘贴内容与手动输入
- **图片粘贴检测**：检测括号粘贴负载中的图片路径，渲染为 `[Image N]` 占位符
- **大粘贴折叠**：粘贴 >10,000 字符时折叠为 `[Pasted text]` 占位符，提交时发送完整文本

#### 已知问题

1. **Claude Code #50012**：Windows 终端 + PowerShell 中 >500 行粘贴时，`ESC[200~` 标记可能因 stdin 缓冲区溢出丢失，导致括号粘贴静默失败，只出现最后几行。

2. **ConPTY 输入缓冲区**：默认约 4KB 循环缓冲区。如果大粘贴超过 ConPTY 输入处理速度，数据可能丢失。缓解：使用 `term.paste(text)` 逐行写入（xterm.js 默认行为，每行经 `\r` 分隔）。

3. **slTerminal 当前 paste 实现**（`keyboard.ts`）：
   ```typescript
   handler: () => {
     readText().then((text) => {
       const term = getTerm();
       if (term) term.paste(text);  // xterm.js 自动处理括号包裹
     }).catch(() => {});
     return true;
   },
   ```
   `term.paste(text)` 逐行写入，每行 `\r` 分隔，ConPTY 输入缓冲区不会溢出。

#### 大粘贴的改进建议

- 当前 10,000 字符折叠阈值是 Claude Code 自身的限制，slTerminal 无需额外处理
- 极大量粘贴（>100,000 字符）建议：Claude Code 使用 `/add-dir` 或文件路径，而非直接粘贴
- 如需在 slTerminal 层限制粘贴大小，可在 `keyboard.ts` 的 paste handler 中添加字符数检查

**信息来源**：
- [xterm.js commit 1dbcf70 — 括号粘贴支持](https://github.com/xtermjs/xterm.js/commit/1dbcf70cee9ae88c69cf9724745cbdb7e5364dcc)
- [xterm.js v5.3.0 release](https://newreleases.io/project/github/xtermjs/xterm.js/release/5.3.0)
- [Claude Code #50012 — 大粘贴截断](https://github.com/anthropics/claude-code/issues/50012)
- [Claude Code 终端配置文档](https://code.claude.com/docs/en/terminal-config)

---

## 安全考虑

### OSC 52 攻击面

| 威胁 | 说明 | 缓解 |
|------|------|------|
| **剪贴板嗅探** | 恶意进程发送 `OSC 52;c;?\a` 读取剪贴板 | 不响应读请求（`Pd = ?`） |
| **恶意注入** | 远程输出 `rm -rf /` 到剪贴板，诱骗粘贴 | 可考虑 OSC 52 写入确认弹窗（但会降低体验） |
| **非焦点操控** | 后台 Tab 的 OSC 52 静默修改剪贴板 | 焦点检查门控（仅活跃面板生效） |
| **大负载 DoS** | 超大 base64 payload 消耗内存 | 限制 payload 大小（如 1MB） |

### 行业安全实践

| 终端 | 策略 |
|------|------|
| **Windows Terminal** | 仅写入，不允许读取。`allowOSC52=false` 全局关闭。 |
| **iTerm2** | "Applications may access clipboard" 开关，默认关闭。 |
| **Alacritty** | 四段可配：`Disabled`/`OnlyCopy`(默认)/`OnlyPaste`/`CopyPaste` |
| **xterm.js addon** | 独立 addon（非核心），需显式加载。 |
| **mintty** | `AllowSetSelection` + `AllowPasteSelection` 独立开关，默认关闭。 |

**slTerminal 推荐策略**：仅写入（`c` 选择器）、不响应读请求、可选焦点门控、payload 限制 1MB。

---

## 实施建议

### 最小可行方案（推荐）

在 `useXterm.ts` 的 `useEffect` 中，Terminal 实例创建后注册 OSC 52 handler：

```typescript
// 在 term.open(container) 之后添加
const osc52Disposable = term.parser.registerOscHandler(52, (data: string) => {
  const semicolonIdx = data.indexOf(";");
  if (semicolonIdx === -1) return true;
  const selector = data.substring(0, semicolonIdx);
  const payload = data.substring(semicolonIdx + 1);
  if (selector && selector !== "c") return true;
  if (payload === "?" || payload.length === 0) return true;
  try {
    const text = atob(payload);
    import("../../ipc/clipboard").then(({ writeText }) =>
      writeText(text).catch((e) => console.error("[OSC52] 写剪贴板失败:", e))
    );
  } catch {
    // base64 解码失败，忽略
  }
  return true;
});

// 在 cleanup 中:
// osc52Disposable.dispose();
```

### 变更文件清单

| 文件 | 变更 |
|------|------|
| `src/panels/terminal/useXterm.ts` | 注册 OSC 52 handler（约 20 行） |
| `src/panels/terminal/CLAUDE.md` | 更新 OSC 52 拦截说明 |
| `docs/claude/investigation-directions.md` | 标记 4a 为已调查 |

### 不需要的变更

| 项 | 原因 |
|----|------|
| 安装 `@xterm/addon-clipboard` | 自定义 handler 更轻量，无需额外依赖 |
| Rust 后端处理 OSC 52 | 转义序列拦截应在前端层完成（xterm.js 解析器） |
| PTY reader 线程解析 OSC 52 | reader 线程应保持透明透传，不应解析语义 |
| 括号粘贴修改 | xterm.js 原生支持，当前无问题 |
| tmux DCS 解包（第一期） | 二期考虑，需额外设计和测试 |

### 测试要点

1. OSC 52 正常写入：向 PTY 写入 `\x1b]52;c;SGVsbG8=\x07`（base64 "Hello"）→ 系统剪贴板应为 "Hello"
2. OSC 52 无效 base64：向 PTY 写入 `\x1b]52;c;!!!INVALID!!!\x07` → 不应 crash，剪贴板不变
3. OSC 52 查询被忽略：`\x1b]52;c;?\x07` → 不应修改剪贴板
4. CTRL+Shift+C 不受影响：选区复制行为不变
5. CJK 内容：含中文的 base64 payload 正确解码写入
6. 大 payload：>100KB 的 base64 内容不导致性能问题

---

## 信息来源总汇

- [xterm.js PR #4220 — ANSI OSC52 支持](https://github.com/xtermjs/xterm.js/pull/4220)
- [xterm.js PR #4863 — OSC 52 剪贴板操作](https://github.com/xtermjs/xterm.js/pull/4863)
- [xterm.js Parser Hooks 文档](https://xtermjs.org/docs/guides/hooks/)
- [xterm.js commit db5bfc7 — ClipboardAddon](https://github.com/xtermjs/xterm.js/commit/db5bfc75251c0646018123a801903e3fcdb1ddee)
- [daintree Issue #3703 — OSC 52 安全风险](https://github.com/daintreehq/daintree/issues/3703)
- [microsoft/terminal#17314 — ConPTY OSC 序列重排](https://github.com/microsoft/terminal/issues/17314)
- [microsoft/terminal#4116 — ConPTY 转义序列截断](https://github.com/microsoft/terminal/issues/4116)
- [WezTerm #3624 — ConPTY 吞噬未知序列](https://github.com/wezterm/wezterm/issues/3624)
- [Eclipse CDT Bug 573796 — ConPTY 始终 UTF-8](https://bugs.eclipse.org/bugs/show_bug.cgi?id=573796)
- [Neovim #26688 — OSC 52 EAGAIN 截断](https://github.com/neovim/neovim/issues/26688)
- [Claude Code #50605 — WSL2 OSC 52 乱码](https://github.com/anthropics/claude-code/issues/50605)
- [Claude Code #50012 — 大粘贴截断](https://github.com/anthropics/claude-code/issues/50012)
- [Claude Code #27709 — clip.exe 乱码](https://github.com/anthropics/claude-code/issues/27709)
- [LINUX DO — Windows /copy 乱码修复历程](https://linux.do/t/topic/1916261/5)
- [Tauri Clipboard Plugin 文档](https://tauri.app/plugin/clipboard/)
- [arboard — Rust 跨平台剪贴板库](https://github.com/rerun-io/arboard)
- [tmux Clipboard Wiki](https://github.com/tmux/tmux/wiki/Clipboard)
- [tmux FAQ: passthrough escape sequence](https://github.com/tmux/tmux/wiki/FAQ#what-is-the-passthrough-escape-sequence-and-how-do-i-use-it)
- [tmux #4214 — allow-passthrough 文档](https://github.com/tmux/tmux/issues/4214)
- [lazygit PR #3597 — tmux OSC 52 包装示例](https://github.com/jesseduffield/lazygit/pull/3597)
- [xterm.js commit 1dbcf70 — 括号粘贴支持](https://github.com/xtermjs/xterm.js/commit/1dbcf70cee9ae88c69cf9724745cbdb7e5364dcc)
- [Windows Terminal PR #18449 — allowOSC52](https://github.com/microsoft/terminal/pull/18449)
- [Claude Code 终端配置文档](https://code.claude.com/docs/en/terminal-config)
- [max.nardit.com — OSC 52 + tmux 实战配置](https://max.nardit.com/articles/osc52-clipboard-xterm-tmux)
