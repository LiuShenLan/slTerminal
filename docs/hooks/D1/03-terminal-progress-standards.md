# 终端进度/状态指示器标准调研

> 日期：2026-07-25
> 目的：调研主流终端模拟器中进度指示器、状态栏通知、标记/注释系统的标准化程度与实现差异，为 slTerminal 的 shell integration 与进度指示功能设计提供参考。

---

## 1. OSC 9;4 — 进度指示器协议（核心标准）

### 1.1 概述

OSC 9;4 是目前最广泛采用的终端进度指示器标准，最初由 **ConEmu** 引入，后经 **Windows Terminal**（2020-11，PR #8055）推广至主流生态。

序列格式：

```
ESC ] 9 ; 4 ; <state> ; <progress> ST
```

- `ESC` = 0x1B（`\x1b` / `\033` / `\e`）
- `ST` = 0x07 BEL（`\a`）或 `ESC \`（0x1B 0x5C）
- **ConEmu 原始格式使用 BEL 终止**，Windows Terminal 文档也使用 BEL；VTE 要求 `ESC \` 终止

### 1.2 状态码

| state | 含义 | progress 参数 | 典型视觉效果 |
|-------|------|-------------|------------|
| **0** | 清除/隐藏进度 | 忽略 | 关闭进度条 |
| **1** | 正常进度 | 0-100 | 蓝色/绿色进度条 |
| **2** | 错误状态 | 0-100（可选，缺省=0） | 红色进度条 |
| **3** | 不确定态（脉冲/spinner） | 忽略 | 来回扫描动画 |
| **4** | 警告/暂停 | 0-100（可选，缺省=0） | 黄色进度条 |

> **状态 4 的歧义**：ConEmu/Windows Terminal 将 state 4 解释为"Warning"，WezTerm 解析 state 4 序列但不暴露为 `pane:get_progress()` 的有效状态（仅返回 0/1/2/3）。Gradle 等工具发出 state 4 作为 warning 语义。实际使用中建议避开 state 4 以保证跨终端兼容。

### 1.3 示例代码

**Bash：**
```bash
# 正常进度 50%
printf '\033]9;4;1;50\a'
# 不确定态
printf '\033]9;4;3;\a'
# 清除
printf '\033]9;4;0;\a'
```

**PowerShell：**
```powershell
Write-Host -NoNewline ("`e]9;4;1;50`a")
```

**Rust：**
```rust
// 使用 osc94 crate
use osc94::Progress;
let mut progress = Progress::default();
progress.start();              // state 1, indeterminate
progress.set_progress(50);     // state 1, 50%
progress.error();              // state 2
progress.finish();             // state 0
```

### 1.4 终端支持矩阵

| 终端 | 支持 | 进度显示位置 | 备注 |
|------|------|------------|------|
| **Windows Terminal** | v1.6+ | 页签圆环 + 任务栏 | 后续采纳者（协议经 PR #8055 推广至主流生态） |
| **ConEmu** | 原生支持 | 任务栏 | 协议创始人 |
| **WezTerm** | 支持 | 页签 + 标题栏 + Lua API (`pane:get_progress()`) | state 4 不支持（视为 Paused） |
| **Ghostty** | 1.2+ 全面支持 | 窗口 chrome | `TERM_PROGRAM=ghostty` |
| **iTerm2** | v3.6.6+ | 页签/标题栏 | OSC 9 与通知系统潜在冲突 |
| **Kitty** | 支持 | 页签 | OSC 9 历史冲突（issue #8768：dotnet 输出被误识别为通知） |
| **VS Code 集成终端** | 支持（via xterm.js addon） | 回调给 IDE | `@xterm/addon-progress` |
| **GNOME Terminal (VTE)** | vte 0.80.0+ | 页签圆环 | 要求 `ESC \` 终止，不接受 BEL |
| **Ptyxis** | MR !80 已关闭未合并（2024-12） | — | 实现尝试（依赖 VTE #2845），变更未合入 main。当前支持状态待确认 |
| **Konsole (KDE)** | 已请求（Bug #497016） | — | 尚未实现 |
| **Alacritty** | 明确拒绝（wontfix） | — | 不计划支持 |
| **xterm.js** | 通过 addon 支持 | 回调给宿主 | `@xterm/addon-progress` (~1.4KB) |

### 1.5 终端检测方法

应用可以通过以下环境变量判断是否支持 OSC 9;4：

| 环境变量 | 终端 |
|---------|------|
| `WT_SESSION` | Windows Terminal |
| `TERM_PROGRAM=ghostty` | Ghostty |
| `TERM_PROGRAM=wezterm` | WezTerm |
| `TERM_PROGRAM=ptyxis` | Ptyxis |
| `VTE_VERSION` | VTE 系（GNOME Terminal 等） |
| `ConEmuPID` | ConEmu |
| `OSC_PROGRESS` | 强制启用（LLDB 方案） |

保守策略：未知终端默认**不发送** OSC 9;4 序列，避免在 Alacritty 等不支持终端产生无意义输出。

### 1.6 tmux 穿透

在 tmux 内部发送 OSC 9;4 需要 DCS 穿透：

```bash
# 需要 tmux.conf: set -g allow-passthrough on
printf '\033Ptmux;\033\033]9;4;1;50\a\033\\'
```

### 1.7 来源

- Windows Terminal 进度栏文档：https://learn.microsoft.com/en-us/windows/terminal/tutorials/progress-bar-sequences
- Windows Terminal PR #8055（ConEmu OSC 9;4 实现）：https://github.com/microsoft/terminal/commit/16e8a84c
- ConEmu ANSI 逃逸码文档：https://conemu.github.io/en/AnsiEscapeCodes.html#ConEmu_specific_OSC
- WezTerm issue #6581：https://github.com/wezterm/wezterm/issues/6581
- WezTerm `pane:get_progress()` API：https://wezterm.org/config/lua/pane/get_progress.html
- Ghostty shell integration：https://github.com/HazAT/pi-ghostty
- VTE issue #2885（错误状态永久残留 bug）：https://gitlab.gnome.org/GNOME/vte/-/work_items/2885
- Ptyxis MR !80（视口顶部进度条，**已关闭未合并**，2024-12）：https://gitlab.gnome.org/chergert/ptyxis/-/merge_requests/80
- xterm.js issue #5250：https://github.com/xtermjs/xterm.js/issues/5250
- xterm.js addon PR #5251：https://github.com/xtermjs/xterm.js/pull/5251
- Konsole feature request #497016：https://bugs.kde.org/show_bug.cgi?id=497016
- LLDB OSC 9;4 PR #162162：https://github.com/llvm/llvm-project/pull/162162
- Cargo OSC 9;4 支持（1.87）：https://github.com/rust-lang/cargo/pull/14615
- `osc94` Rust crate：https://docs.rs/osc94
- `termpulse` Rust crate：https://docs.rs/termpulse

---

## 2. OSC 133 — 语义提示符协议（FinalTerm/FTCS）

### 2.1 概述

OSC 133 源自 **FinalTerm**，通过四个序列标记 shell 提示符/命令/输出的边界。所有主流终端均支持，是现代 shell integration 的基础层。

### 2.2 序列定义

| 序列 | 名称 | 时机 | 副作用 |
|------|------|------|--------|
| `OSC 133 ; A ST` | `FTCS_PROMPT` | 提示符开始前 | 创建 mark |
| `OSC 133 ; B ST` | `FTCS_COMMAND_START` | 提示符结束后，命令输入前 | 标记命令起点 |
| `OSC 133 ; C ST` | `FTCS_COMMAND_EXECUTED` | 命令输出开始前 | 标记输出区域起点 |
| `OSC 133 ; D ; <exitCode> ST` | `FTCS_COMMAND_FINISHED` | 命令执行完成后 | exitCode=0 → 绿色 √；非0 → 红色 × |

### 2.3 终端利用方式

| 终端 | 利用 OSC 133 实现的功能 |
|------|----------------------|
| **Windows Terminal** | scrollbar marks（滚动条标记）、`scrollToMark` 快捷键（Ctrl+Up/Down）、`selectCommand`/`selectOutput` 文本选择 |
| **iTerm2** | 左侧边距蓝色三角 mark、Cmd+Shift+Up/Down 跳转、退出码着色、`Alert on next mark` 模态通知 |
| **Kitty** | 滚动条提示符标记、`kitty @ scroll-window` 跳转 |
| **WezTerm** | 语义化提示符/命令区域识别、Lua event hook |
| **Ghostty** | 提示符标记、命令导航 |
| **VS Code 终端** | shell integration marks |

### 2.4 Windows Terminal 配置示例

```json
"profiles": {
    "defaults": {
        "showMarksOnScrollbar": true,
        "autoMarkPrompts": true
    }
},
"actions": [
    { "keys": "ctrl+up",   "command": { "action": "scrollToMark", "direction": "previous" } },
    { "keys": "ctrl+down", "command": { "action": "scrollToMark", "direction": "next" } },
    { "command": { "action": "selectOutput", "direction": "prev" } },
    { "command": { "action": "selectCommand", "direction": "prev" } }
]
```

> 这些设置在 Terminal 1.21 之前带 `experimental.` 前缀，1.21 起去除。

### 2.5 iTerm2 额外特性

- **`Alert on next mark`**（Cmd+Opt+A）：长命令执行完成时触发模态提醒
- **退出码着色**：成功 mark 绿色，失败 mark 红色
- **多行提示符**：通过 `$(iterm2_prompt_mark)` 自定义 mark 位置
- **Triggers 系统**：无需安装 shell integration 脚本，通过正则匹配实现提示符检测

### 2.6 实现要点

Windows Terminal 内部的数据结构（来自 `Marks.hpp`）：

```cpp
struct ScrollbarData {
    enum Category { Default, Error, Warning, Success, Prompt };
    Category category;
    optional<uint32_t> exitCode;
    optional<Color> color;
};
struct MarkExtents {
    // start, end, commandEnd, outputEnd
};
```

### 2.7 来源

- Windows Terminal shell integration 文档：https://learn.microsoft.com/en-us/windows/terminal/tutorials/shell-integration
- FTCS_PROMPT PR #13163：https://github.com/microsoft/terminal/pull/13163
- Mark 渲染修复 PR #19185：https://github.com/microsoft/terminal/commit/e818dafa
- iTerm2 shell integration 文档：https://iterm2.com/shell_integration.html
- iTerm2 专有逃逸码文档：https://iterm2.com/documentation-escape-codes.html
- Kitty shell integration 文档：https://sw.kovidgoyal.net/kitty/shell-integration/
- WezTerm shell integration 文档：https://wezterm.org/shell-integration.html

---

## 3. 终端通知系统

### 3.1 协议分派

不同终端使用不同的 OSC 码触发桌面通知：

| 终端 | 协议 | 序列格式 |
|------|------|---------|
| iTerm2 | **OSC 9** | `ESC ] 9 ; <message> BEL` |
| rxvt-unicode | **OSC 777** | `ESC ] 777 ; notify ; <title> ; <body> BEL` |
| Ghostty | **OSC 777**（也兼容 OSC 9） | 同上 |
| WezTerm | **OSC 777**（也兼容 OSC 9，2021年起） | 同上 |
| Kitty | **OSC 99** | `ESC ] 99 ; <metadata> ; <payload> ST` |
| foot (Wayland) | **OSC 777** | 同上 |

### 3.2 Kitty OSC 99 详细格式

Kitty 的通知协议最为完善，支持分块、交互和生命周期管理：

```
ESC ] 99 ; key1=val1:key2=val2 ; payload ST
```

关键元数据参数：

| 键 | 值 | 默认 | 说明 |
|----|-----|------|------|
| `p` | `title`/`body`/`close`/`icon`/`?`/`alive`/`buttons` | `title` | 负荷类型 |
| `i` | 字母数字标识符 | 无 | 通知 ID，`i=0` 为向后兼容保留 |
| `d` | `0` 或 `1` | `1` | 0=尚未完成（累积分块）；1=完成，立即显示 |
| `a` | `focus`/`report`（可加 `-` 前缀） | `focus` | 点击动作：聚焦窗口/发送报告回终端 |
| `e` | `0` 或 `1` | `0` | 1=负荷为 Base64 编码 UTF-8 |
| `c` | `0` 或 `1` | `0` | 1=关闭时发回关闭事件 |
| `o` | `always`/`unfocused`/`invisible` | `always` | 何时显示通知 |
| `u` | `0`/`1`/`2` | 无 | 紧急度：0=低，1=普通，2=严重 |
| `w` | 整数 >= -1 | -1 | 自动关闭毫秒数；-1=OS 默认，0=永不过期 |
| `s` | Base64 UTF-8 | `system` | 提示音 |

分块示例：
```bash
# 第一块（标题，未完成）
printf '\x1b]99;i=1:d=0;Downloading\x1b\\'
# 第二块（正文，完成）
printf '\x1b]99;i=1:p=body;File saved to /tmp\x1b\\'
```

支持的操作：
- **更新**：发送同 `i` 的新通知
- **关闭**：`ESC ] 99 ; i=<id> : p=close ; ST`
- **轮询存活**：`ESC ] 99 ; i=<id> : p=alive ; ST`
- **能力查询**：`ESC ] 99 ; i=<id> : p=? ; ST`（返回支持的键/值列表）

### 3.3 兼容性与检测

```bash
# 探测终端通知支持
if [ -n "$KITTY_WINDOW_ID" ]; then
    PROTO="osc99"  # Kitty
elif [ -n "$ITERM_SESSION_ID" ]; then
    PROTO="osc9"   # iTerm2
elif [ -n "$WEZTERM_PANE" ] || [ "$TERM_PROGRAM" = "ghostty" ] || [ -n "$TERMINAL_ID" ]; then
    PROTO="osc777" # WezTerm / Ghostty / foot
fi
```

### 3.4 来源

- Kitty 桌面通知文档：https://sw.kovidgoyal.net/kitty/desktop-notifications/
- Kitty notification issue #8768：https://github.com/kovidgoyal/kitty/issues/8768
- Kitty OSC 99 support issue #9777：https://github.com/kovidgoyal/kitty/issues/9777
- WezTerm toast notifications issue #489：https://github.com/wezterm/wezterm/issues/489
- iTerm2 通知设置：https://iterm2.com/documentation-escape-codes.html
- pi-notify (npm)：https://www.npmjs.com/package/pi-notify
- Nextflow OSC 777/9/99 PR #6839：https://github.com/nextflow-io/nextflow/pull/6839

---

## 4. 终端特定高级功能

### 4.1 iTerm2 — 专有逃逸码体系（OSC 1337）

iTerm2 在 OSC 133 基础上扩展了 OSC 1337 指令系列：

| 序列 | 用途 |
|------|------|
| `OSC 1337 ; SetUserVar=<key>=<base64value> ST` | 设置用户自定义变量 |
| `OSC 1337 ; ShellIntegrationVersion=<n> ; shell=<name> ST` | 声明 shell integration 版本 |
| `OSC 1337 ; RemoteHost=<user@host> ST` | 报告远程主机信息 |
| `OSC 1337 ; CurrentDir=<path> ST` | 报告当前工作目录 |

**`iterm2_set_user_var` 机制**：

iTerm2 周期性地调用 shell 中定义的 `iterm2_print_user_vars` 函数，收集用户变量后在 Badge、状态栏中引用：

```bash
# bash/zsh
function iterm2_print_user_vars() {
  iterm2_set_user_var gitBranch $((git branch 2>/dev/null) | grep \* | cut -c3-)
}
```

Badge 模板引用：`\(user.gitBranch)`

### 4.2 WezTerm — Lua 事件系统

WezTerm 通过 Lua 提供强大的事件驱动扩展能力：

**关键事件**：

| 事件 | 触发时机 | 用途 |
|------|---------|------|
| `format-tab-title` | 页签标题需要重算 | 显示进度/用户变量/git 分支 |
| `update-status` | 状态栏更新 | 自定义状态栏内容 |
| `user-var-changed` | OSC 1337 用户变量变更 | 响应 shell 侧变量变化 |
| `window-config-reloaded` | `set_config_overrides()` 调用后 | 运行时切换配置 |

**用户变量传递路径（Passing Data from Pane to Lua）**：

```
Shell 端: printf "\033]1337;SetUserVar=%s=%s\007" git_branch $(echo -n "main" | base64)
         ↓ (触发 user-var-changed + update-status + format-tab-title)
Lua 端:  tab.active_pane.user_vars.git_branch
         pane:get_user_vars()
```

**进度访问**：
```lua
-- pane:get_progress() 返回值：
-- "None"                  — 无进度报告
-- {Percentage = 50}       — 确定进度 (state 1)
-- {Error = 75}            — 错误状态 (state 2)
-- "Indeterminate"         — 不确定态 (state 3)
-- (state 4 不支持)
```

**format-tab-title 中的进度可视化**：
```lua
wezterm.on('format-tab-title', function(tab)
  local progress = tab.active_pane.progress -- nightly build
  local branch = tab.active_pane.user_vars.git_branch
  local title = tab.active_pane.title
  -- 渲染带进度图标/Nerd Font glyph 的标题
end)
```

该事件是**同步**的，不能调用 `wezterm.run_child_process`；返回字符串（纯文本）或 `FormatItem` 表格（带样式）。

### 4.3 Kitty — Remote Control 协议

Kitty 的 Remote Control 使用 **DCS** 逃逸码（非 OSC 99），允许外部程序控制终端窗口/标签页：

```
DCS ... JSON payload ... ST
```

常用命令：
```bash
kitty @ new-window                    # 新建窗口
kitty @ set-tab-title "my title"      # 设置标签页标题
kitty @ scroll-window                 # 滚动窗口
kitty @ send-text "hello\n"           # 向窗口发送文本
kitty @ get-text                      # 获取窗口文本
kitty @ ls                            # 列出所有窗口
```

Remote Control 与通知系统（OSC 99）是两套独立协议。

### 4.4 来源

- WezTerm `format-tab-title` 文档：https://wezterm.org/config/lua/window-events/format-tab-title.html
- WezTerm "Passing Data from a pane to Lua" 食谱：https://wezterm.org/recipes/passing-data.html
- WezTerm `pane:get_progress()` 文档：https://wezterm.org/config/lua/pane/get_progress.html
- Kitty remote control 文档：https://sw.kovidgoyal.net/kitty/remote-control/
- iTerm2 专有逃逸码文档：https://iterm2.com/documentation-escape-codes.html
- iTerm2 badges 文档：https://iterm2.com/documentation-badges.html

---

## 5. Hyper 终端 shell integration

Hyper 是基于 Electron 的终端，其插件系统提供：

- **生命周期钩子**：Electron `app` / `BrowserWindow` 事件
- **Redux 中间件**：拦截和响应 session actions
- **UI 组件装饰**：Terminal、Header、Tab 元素

Hyper **不实现** OSC 9;4、OSC 133 或其他标准 shell integration 协议。其插件开发聚焦于 Electron 层面的 UI 修饰和 Redux 状态管理，而非终端逃逸码层。对 OSC 9;4 的支持需要通过 xterm.js（Hyper 内部使用 xterm.js 渲染），可借助 `@xterm/addon-progress` addon 集成。

> 来源：Hyper 插件开发文档（搜索结果）

---

## 6. 实际采纳案例

### 6.1 工具/库采用 OSC 9;4

| 工具 | 语言 | 使用方式 | 检测策略 |
|------|------|---------|---------|
| **Cargo** (v1.87) | Rust | 构建进度 → 任务栏 | `term.progress.term-integration` 配置项 |
| **LLDB** | C++ | 调试进度 → 任务栏 | `show-progress` 设置，`OSC_PROGRESS` 环境变量强制 |
| **mise** | Rust | 工具安装进度 | `MISE_TERMINAL_PROGRESS` 环境变量，加权子步跟踪 |
| **Nextflow** | Java | 工作流执行进度 | `NXF_OSC_PROGRESS`，含 tmux 穿透支持 |
| **systemd** | C | 长时间任务进度 | 直接 emit 序列 |
| **Gradle** | Java | 构建进度 | 直接 emit 序列（state 4=warning） |
| **pytest** | Python | 测试进度（PR #13072） | 环境变量检测 |
| **Starship** | Rust | 提示符装饰 | 终端检测后 emit |
| **oh-my-posh** | Go | 提示符装饰 | 终端检测后 emit |
| **spinners** (Rust crate) | Rust | spinner 生命周期 | 提议中（issue #40） |

### 6.2 npm 生态

- **`osc-progress`**：OSC 9;4 辅助库（TypeScript），自动终端检测
  ```typescript
  import { startOscProgress, createOscProgressController } from "osc-progress";
  const stop = startOscProgress({ label: "Working" });
  // ... do work ...
  stop();
  ```
- **`@xterm/addon-progress`**：xterm.js 的 OSC 9;4 addon (~1.4KB)
  ```javascript
  const progressAddon = new ProgressAddon();
  terminal.loadAddon(progressAddon);
  progressAddon.onChange(({state, value}) => {
    // state: 0-4, value: 0-100
    // 在此调用宿主 UI 更新
  });
  ```
- **`pi-notify`**：OSC 777/9/99 通知库，自动终端检测
- **`pi-ghostty`**：Ghostty 专用，进度指示 + 动态标题

### 6.3 Rust 生态

- **`osc94`** crate：OSC 9;4 序列构建/解析库 + CLI 二进制
- **`termpulse`** crate：三级回退（OSC → ASCII → Silent）+ 智能终端检测
- **`kutil`** crate：含 `con_emu::progress_state` 模块

---

## 7. 设计建议

### 7.1 优先级排序（对于 slTerminal）

1. **OSC 9;4 进度指示器** — 生态最广、实用价值最高、实现最简单
2. **OSC 133 (FTCS) 提示符标记** — shell integration 基础层、WebView2 可用 xterm.js addon
3. **通知系统** — 多种协议并存（OSC 9/99/777），需要多协议支持或走系统级通知 API

### 7.2 实现路径

**方案 A（xterm.js addon 路径）**：
- 利用 `@xterm/addon-progress` 解析 OSC 9;4 序列
- 在回调中更新 Tauri WebView2 窗口的任务栏进度（通过 Tauri window API）
- 页签进度通过 `format-tab-title` 等效机制渲染
- 优势：最小化自定义逃逸码解析

**方案 B（Rust 后端解析）**：
- 在 PTY reader 层解析 OSC 9;4 / OSC 133 序列
- 通过 Tauri Event 广播到前端
- 优势：不依赖 xterm.js addon 版本、可支持非 xterm 面板（编辑器等）
- 注意：不得破坏 reader_loop 吞吐（OSC 解析须轻量）

### 7.3 避坑清单

| 陷阱 | 说明 |
|------|------|
| **ConPTY flags = 0x7 不可改为 0xF** | PASSTHROUGH_MODE (0x8) 吞鼠标滚轮事件，见 `pty/CLAUDE.md` |
| **VTE 要求 ST = `ESC \`** | BEL 终止在 VTE 系终端无效 |
| **iTerm2 OSC 9 冲突** | iTerm2 用 OSC 9 做通知——若同时 emit OSC 9;4 进度和 OSC 9 通知，需正确分派 |
| **tmux 穿透** | 在 tmux 中无效序需要 DCS `ESC P tmux; ...` 包裹 |
| **state 4 语义不统一** | Warning vs Paused——建议只使用 state 0/1/2/3，避免 state 4 |
| **保守检测** | 未知终端不发送 OSC 序列 |
