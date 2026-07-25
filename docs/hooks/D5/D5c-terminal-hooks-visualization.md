# D5c：终端模拟器的 Hook/Event 可视化

调研日期：2026-07-25

## 1. Windows Terminal

### 1.1 Profiles & Actions

**Actions 系统**是 Windows Terminal 的核心可编程机制。每个 Action 是一个可绑定到快捷键的操作命令。

**内置 Action 列表**（部分）：
| Action | 说明 |
|--------|------|
| `newTab` | 新建标签页 |
| `closeTab` / `closePane` / `closeWindow` | 关闭标签页/窗格/窗口 |
| `splitPane` (horizontal/vertical/auto) | 分屏 |
| `duplicateTab` / `duplicatePane` | 复制标签页/窗格 |
| `nextTab` / `prevTab` | 切换标签页 |
| `copy` / `paste` | 复制/粘贴 |
| `find` | 查找 |
| `sendInput` | 向终端发送文本 |
| `adjustFontSize` / `resetFontSize` | 调整字号 |
| `scrollDown` / `scrollUp` / `scrollDownPage` / `scrollUpPage` | 滚动 |
| `toggleFullscreen` | 全屏切换 |
| `openSettings` | 打开设置 |
| `moveFocus` | 焦点在窗格间移动 |
| `resizePane` | 调整窗格大小 |

**配置方式**：纯 JSON，在 `settings.json` 或 `defaults.json` 中定义。Action 可绑定到 `keybindings` 数组中的 `keys` 字段。支持 `iterateOn: "profiles"` 动态生成按 profile 迭代的嵌套命令。

```json
{
  "name": "Duplicate Tab",
  "command": "duplicateTab",
  "keys": "ctrl+alt+a"
}
```

Actions 按 `id` 标识。`defaults.json` 和 fragments 中的 actions 必须含 `id`，用户自定义 actions 未指定 id 时自动生成。

**来源**：
- https://github.com/microsoft/terminal/blob/main/doc/specs/ (2026-07)
- https://learn.microsoft.com/zh-cn/windows/terminal/json-fragment-extensions (2026-07)

### 1.2 Fragment Extensions

JSON Fragment Extensions 允许第三方应用向 Windows Terminal 注入 profiles 和 color schemes，无需直接编辑 `settings.json`。

**JSON 结构**：
```json
{
  "profiles": [
    { "updates": "{GUID}", "fontSize": 16 },
    { "name": "MyProfile", "commandline": "pwsh.exe" }
  ],
  "schemes": [
    { "name": "MyScheme", "black": "#0C0C0C", ... }
  ]
}
```

- `updates` 字段 + profile GUID → 修改已有 profile
- 省略 `updates` + 提供 `name` → 新建 profile
- GUID 通过 Version 5 UUID（name-based, UTF-16LE）生成，使用两个命名空间
- 支持分发的媒体资源（图标、背景图、pixel shader），自 Terminal >= 1.24

**文件放置路径**：

| 安装类型 | 路径 |
|---------|------|
| Store 应用 | appxmanifest 声明扩展 + `Public\Fragments\` |
| 全局安装 | `C:\ProgramData\Microsoft\Windows Terminal\Fragments\{app}\{file}.json` |
| 用户安装 | `C:\Users\<user>\AppData\Local\Microsoft\Windows Terminal\Fragments\{app}\{file}.json` |

**JSON Fragment Extensions**（已实现，原名为 Proto Extensions——PR #7632，v1.24 起含 Extensions 管理 UI）：允许第三方应用向 Windows Terminal 注入 profiles 和 color schemes，无需直接编辑 `settings.json`。

**来源**：
- https://github.com/MicrosoftDocs/terminal/blob/main/TerminalDocs/json-fragment-extensions.md (2026-07)
- https://github.com/microsoft/terminal/blob/main/doc/specs/Proto extensions-spec.md (2026-07)

### 1.3 UI 可视化方式

- **settings.json 手动编辑**：主要配置入口，纯文本 JSON，无 GUI builder
- **Settings UI（内置）**：提供 profiles 和外观的 GUI 设置页面，但不覆盖所有 actions
- **Command Palette**（`Ctrl+Shift+P`）：显示所有已注册命令，支持搜索过滤，箭头键导航，Enter 执行。是 actions 的主要发现/使用界面
- **Actions 无独立可视化管理界面**：所有 actions/keybindings 通过 JSON 配置，无触发器式条件匹配

---

## 2. iTerm2

### 2.1 Triggers 系统

Triggers 是 iTerm2 的核心 Hook 机制——基于正则表达式的自动响应系统。

**配置路径**：Settings > Profiles > Advanced > Triggers > Edit

**每个 Trigger 包含**：
- **Regular Expression**：匹配终端输出的正则表达式
- **Action**：匹配后执行的动作（26 种）
- **Parameter**：动作参数（支持 `\0`-`\9` 反向引用、`\e`/`\n`/`\t` 等转义、`\xNN` 十六进制）
- **Instant**：勾选后每行匹配立即触发，不等换行

**全部 26 种 Trigger Action**：

| Action | 说明 |
|--------|------|
| **Annotate** | 为匹配文本关联注释 |
| **Bounce Dock Icon** | Dock 图标跳动直到 iTerm2 成为前台窗口 |
| **Capture Output** | 保存匹配行到 Captured Output 工具面板 |
| **Change Style** | 修改匹配文本的样式 |
| **Fold Section**（3.5.12+） | 折叠匹配行到上一个命名标记之间的行 |
| **Highlight Line** | 高亮匹配文本所在的整行（参数设颜色） |
| **Highlight Text** | 高亮匹配文本（参数设颜色） |
| **Inject** | 向输入流注入字节（文本或控制序列） |
| **Invoke Script Function** | 调用 Python API 定义的函数 |
| **Make Hyperlink** | 将匹配文本变为可点击超链接 |
| **Open Password Manager** | 打开密码管理器 |
| **Post Notification** | 发送 Notification Center 通知 |
| **Prompt Detected** | 告知 iTerm2 shell 提示符位置（Shell Integration） |
| **Report Directory** | 告知 iTerm2 当前目录（Shell Integration） |
| **Report User & Host** | 告知 iTerm2 用户/主机名（Shell Integration） |
| **Ring Bell** | 播放系统提示音 |
| **Run Command** | 执行用户定义的命令 |
| **Run Coprocess** | 运行协进程 |
| **Run Silent Coprocess** | 运行协进程但不显示输出 |
| **Send Text** | 向终端发送文本（模拟用户输入） |
| **Set Mark** | 设置标记 |
| **Set Named Mark** | 设置命名标记 |
| **Set Title** | 设置会话标题 |
| **Set User Variable** | 设置用户自定义变量 |
| **Show Alert** | 显示弹窗 |
| **Stop Processing Triggers** | 阻止后续 trigger 对当前文本执行 |

**来源**：
- https://iterm2.com/documentation-one-page.html (2026-07)
- https://iterm2.com/documentation-triggers.html (2026-07)

### 2.2 Smart Selection

**配置路径**：Settings > Profiles > Advanced > Smart Selection

基于正则表达式的智能文本选择。通过 **quad-click（四击）** 触发，自动按规则选中匹配文本。

**每条规则包含**：
- **Regular Expression**：匹配规则
- **Precision**：置信度（Very Low / Low / Normal / High / Very High）

高 Precision 规则优先于低 Precision 规则。例如 URL 匹配（高 Precision）优先于通用单词匹配（低 Precision），除非单词匹配显著更长。

匹配文本的右键菜单可添加自定义操作。

**来源**：
- https://iterm2.com/documentation-smart-selection.html (2026-07)
- https://iterm2.com/documentation-preferences-profiles-advanced.html (2026-07)

### 2.3 Python API

iTerm2 Python API 提供完整的脚本化扩展能力，是一个独立运行的 Python 进程，通过 AppleScript 桥接与 iTerm2 通信。

**核心机制**：

| 机制 | 说明 |
|------|------|
| **Hooks (RPC)** | 通过装饰器注册回调，修改应用默认行为 |
| **Variables** | 会话/标签页/窗口的 JSON 可编码状态变量，通过 `iterm2.Reference()` 绑定 |
| **Function Calls** | 命名函数，供 Triggers 的 "Invoke Script Function" 调用 |
| **Daemons** | AutoLaunch 目录中的长驻脚本 (`iterm2.run_forever()`) |
| **Custom Control Sequences** | 监听自定义转义序列，实现程序→脚本通信 |

**可用的 Hook 装饰器**：

| Hook | 用途 |
|------|------|
| `@iterm2.TitleProviderRPC` | 自定义会话标题。接收 session 上下文变量，返回标题字符串 |
| `@iterm2.StatusBarRPC` | 自定义状态栏组件。接收 knobs 配置 + Reference inputs，返回字符串或字符串数组 |
| `CustomControlSequenceMonitor` | 监听自定义控制序列。需要 shared secret identity + regex，支持全局或按 session_id 监听 |

**Variables 系统**：iTerm2 内部状态（会话名、当前目录、主机名等）以变量形式暴露，Python 脚本通过 `iterm2.Reference("variableName?")` 订阅。变量变化时自动触发关联的 hook 重新执行。

**脚本放置路径**：`~/Library/Application Support/iTerm2/Scripts/AutoLaunch/`（自动启动的长驻脚本）

**来源**：
- https://iterm2.com/python-api/tutorial/hooks.html (2026-07)
- https://iterm2.com/python-api/registration.html (2026-07)
- https://iterm2.com/python-api/customcontrol.html (2026-07)
- https://iterm2.com/3.3/documentation-scripting-fundamentals.html (2026-07)

### 2.4 UI 可视化方式

- **Triggers**：GUI 表格编辑器（Settings > Profiles > Advanced > Triggers > Edit），逐条配置 regex/action/parameter/instant
- **Smart Selection**：GUI 规则列表编辑器（Settings > Profiles > Advanced > Smart Selection），逐条配置 regex/precision
- **Python API**：无内置 GUI——通过外部 Python 脚本 + 文件系统注册。脚本放置到指定目录后，iTerm2 在启动时自动加载
- **Script Console**：iTerm2 菜单栏提供 Scripts 菜单，可手动运行/管理脚本
- **Captured Output 工具面板**：Trigger 的 Capture Output 结果在此面板中可视化展示

---

## 3. Warp

### 3.1 Workflows

Warp 的 Workflows 系统允许用户将一系列命令保存为可参数化的模板，通过 Ctrl+Shift+R 命令面板搜索和执行。Workflows 支持 `{{param}}` 占位符、条件分支和多步骤执行。

**配置方式**：GUI 界面（Warp Drive / Team Warp Drive）创建、编辑和共享 Workflows。不是基于配置文件。

### 3.2 Notifications

Warp 提供两套通知机制：

**A. 内置通知**（系统级，GUI 控制）

| 触发条件 | 行为 |
|---------|------|
| 长时间命令完成 | 超过可配置秒数后，切到其他应用时弹桌面通知 |
| 需要密码输入 | 检测到 `sudo` 等密码提示时通知 |

配置路径：**Settings > Features > Notifications**。默认启用。

**B. 自定义通知 Hook**（转义序列，脚本级控制）

| 序列 | 格式 | 说明 |
|------|------|------|
| **OSC 9** | `ESC ] 9 ; <body> BEL` | 仅正文通知 |
| **OSC 777** | `ESC ] 777 ; notify ; <title> ; <body> BEL` | 标题+正文通知 |

```bash
# 示例
printf '\033]9;Build complete\007'
printf '\033]777;notify;Deploy;Success on prod\007'
```

跨平台（macOS/Windows/Linux），零外部依赖。payload 中避免换行和分号（或正确转义）。

**来源**：
- https://docs.warp.dev/terminal/more-features/notifications/ (2026-07)

### 3.3 Blocks & DCS Hooks

Warp 的核心架构基于 **Blocks** 概念——每个命令及其输出为一个独立的块（Block），块之间由提示符边界分隔。

**DCS Hook 协议**：Warp 的 Rust 后端通过 `DProtoHook` 枚举解析 shell 发出的 DCS 转义序列，跟踪 shell 生命周期：

> **信息来源说明**：Warp 的 GitHub 仓库 (`warpdotdev/Warp`) 为 issues-only 仓库，客户端源代码（含 `dcs_hooks.rs`）截至 2026-07 尚未开源。以下 DCS Hook 列表来自 Warp 官方文档、生态项目 (`warpdotdev/claude-code-warp`) 和社区逆向分析，未经过源码直接验证。具体 hook 数量和名称可能因 Warp 版本而异。

| Hook | 触发时机 |
|------|---------|
| `Precmd` | 提示符渲染前 |
| `Preexec` | 命令执行前 |
| `CommandStarted` | 命令开始执行 |
| `CommandFinished` | 命令执行完成 |
| `Bootstrapped` | Shell 初始化完成 |
| `InitShell` / `InitSubshell` | Shell / 子 Shell 初始化 |
| `InputBuffer` | 捕获当前输入缓冲状态 |
| `Clear` | 终端清屏 |
| `SSH` / `InitSsh` | SSH 会话生命周期 |
| `ExitShell` | Shell 退出 |
| `SourcedRcFileForWarp` | RC 文件加载完成（Warpify 信号） |

**Shell 集成注入**：通过在 `.zshrc`/`.bashrc` 末尾注入 DCS 序列实现自动 Warpify：
```
printf '\eP$f{"hook": "SourcedRcFileForWarp", "value": { "shell": "zsh"}}\x9c'
```

**Agent Session States**：对于 AI 编码代理（Claude Code、Gemini CLI 等），Warp 定义了额外的会话状态 hook：

| 状态 | 含义 |
|------|------|
| Prompt submitted | 用户发送提示，Agent 工作中 |
| Tool executing | Agent 运行工具中 |
| Idle / waiting for input | Agent 等待用户输入 |
| Permission requested | Agent 需要工具执行授权 |
| Task complete | Agent 完成当前轮次 |

这些状态通过 OSC 777 → `warp://cli-agent` 的 JSON payload 通信，驱动内联状态指示器和系统通知。

> **信息来源说明**：OSC 777 的具体协议定义（7 个事件类型、JSON schema）来自 Warp 生态项目 (`warpdotdev/claude-code-warp`) 和终端通知生态文档。Warp 核心源码未开源，此协议未经源码级验证。

**来源**：
- https://docs.warp.dev/terminal/warpify/subshells (2026-07)
- https://www.warp.dev/blog/universal-agent-support-level-up-coding-agent-warp (2026-07)

### 3.4 UI 可视化方式

- **Notifications 设置**：Settings > Features > Notifications — 开关 + 时间阈值滑块
- **Warp Drive**：Workflows 的 GUI 管理器，支持创建、编辑、搜索、参数化
- **Command Palette**（Ctrl+Shift+R）：Workflows 发现和执行入口
- **Blocks 可视化**：每个命令天然以 Block 形式展示，命令输出隔离、可折叠、可单独复制
- **Agent 状态指示器**：内联显示 AI Agent 当前状态（运行中/等待输入/已完成）
- **DCS Hooks 基础功能为自动注入**：由 shell 集成脚本自动注入，对用户透明（客户端源码截至 2026-07 尚未开源）；`warp://` deeplinks 和 `WARP_CLI_AGENT_PROTOCOL_VERSION` 环境变量允许第三方扩展进行协议协商和自定义配置
- **Unified Notification Center**：跨所有 Agent 和终端会话的统一通知面板

---

## 4. WezTerm

### 4.1 Event System

WezTerm 的事件系统完全通过 Lua 配置实现，核心 API 为 `wezterm.on(event_name, callback)`。

**特性**：
- **多回调注册**：同一事件可注册多个回调，按注册顺序执行
- **返回值控制**：回调返回 `false` 阻止后续回调 + 阻止默认行为
- **不可注销**：回调仅在配置重载时清空（Lua 状态重建）
- **自 20201031-154415-9614e117** 版本起可用

**所有内置事件**（12 个 window 事件 + 2 个 GUI 生命周期事件 `gui-startup`、`gui-attached`）：

| 事件名 | 参数 | 用途 |
|--------|------|------|
| `format-tab-title` | tab, tabs, panes, config, hover, max_width | 自定义标签页标题，返回 string 或 FormatItems 表 |
| `format-window-title` | tab, pane, tabs, panes, config | 自定义窗口标题，第一个注册的生效 |
| `update-status` | window, pane | 周期性更新状态栏（间隔由 `status_update_interval` 控制） |
| `update-right-status` | window, pane | 周期性更新右侧状态栏 |
| `augment-command-palette` | window, pane | 向命令面板添加自定义条目（brief/doc/action/icon） |
| `bell` | window, pane | 终端响铃时触发 |
| `new-tab-button-click` | window, pane, button, default_action | 点击新建标签按钮时触发，可阻止默认行为 |
| `open-uri` | window, pane, uri | URI 打开时触发 |
| `user-var-changed` | window, pane, name, value | 用户变量变化时触发 |
| `window-config-reloaded` | window, pane | 配置重载后触发 |
| `window-focus-changed` | window, pane | 窗口焦点状态变化 |
| `window-resized` | window, pane | 窗口尺寸变化 |

**来源**：
- https://wezterm.org/config/lua/window-events/index.html (2026-07)
- https://wezterm.org/config/lua/wezterm/on.html (2026-07)

### 4.2 Lua Configuration Hooks

**`wezterm.action_callback(callback)`**：自 20211204 版本起，组合事件注册和 Action 创建，用于内联键位绑定的回调。

```lua
action = wezterm.action_callback(function(win, pane)
  wezterm.log_info('WindowID:', win:window_id())
end)
```

**Custom Events**：支持任意用户定义事件名。两种触发方式：
- `wezterm.emit(event_name)` — Lua 代码程序化触发
- `EmitEvent` key assignment — 键位绑定触发

```lua
wezterm.on('my-custom-event', function(window, pane)
  -- 自定义逻辑
end)

return {
  keys = {
    { key = 'E', mods = 'CTRL', action = act.EmitEvent 'my-custom-event' },
  },
}
```

**动态配置覆盖**：`window:set_config_overrides()` + `window:get_config_overrides()` — 运行时修改窗口配置（如切换连字、字体等）。每次调用触发 `window-config-reloaded` 事件（小心无限循环）。

**User Variables**：终端内可通过 OSC 1337 设置用户变量，Lua 端通过 `user-var-changed` 事件监听，`format-tab-title` 等事件中通过 `tab.active_pane.user_vars` 读取。

**来源**：
- https://wezterm.org/config/lua/wezterm/on.html#custom-events (2026-07)
- https://wezterm.org/config/lua/wezterm/action_callback.md (2026-07)
- https://wezterm.org/recipes/passing-data.html (2026-07)

### 4.3 UI 可视化方式

- **纯 Lua 配置**：所有事件/hook 通过 `wezterm.lua` 文件以代码形式配置，无 GUI
- **Command Palette**：`augment-command-palette` 事件可向命令面板注入自定义命令，命令面板是 actions 的主要发现界面
- **Status Bar**：`update-status` / `update-right-status` 周期性渲染左右状态栏，可直接展示动态信息
- **Tab/Window Title**：`format-tab-title` / `format-window-title` 实时渲染标签页和窗口标题
- **无触发条件 UI**：不像 iTerm2 的 Triggers 表格编辑器——WezTerm 没有条件式匹配的触发机制，事件回调是纯代码
- **`wezterm.log_info()`**：调试输出到 WezTerm 日志，是主要的调试手段

---

## 对比总结表格

| 维度 | Windows Terminal | iTerm2 | Warp | WezTerm |
|------|-----------------|--------|------|---------|
| **Event/Hook 类型** | Actions（命令）+ Keybindings（绑键） | Triggers（26 种动作）+ Smart Selection + Python Hooks | DCS Hooks（shell 生命周期）+ OSC 9/777（通知）+ Blocks（命令隔离） | 14 个内置 Events（12 window + 2 GUI）+ 自定义 Events |
| **配置方式** | 纯 JSON（settings.json + fragments） | GUI 表格编辑器 + Python 脚本文件 | GUI Settings + Shell 转义序列注入 | 纯 Lua 代码（wezterm.lua） |
| **UI 可视化** | Settings UI + Command Palette | Triggers 表格编辑器 + Smart Selection 列表 + Script Console | Settings UI + Warp Drive + Blocks 可视化 + Agent 状态指示器 | Command Palette + 状态栏 + 标题栏渲染 |
| **可编程性** | 低（JSON 静态配置） | 高（Python API + Custom Control Sequences + Variables 绑定） | 中（Shell 转义序列 + Agent SDK） | 高（Lua 脚本 + wezterm.emit + action_callback + 动态配置覆盖） |
| **条件触发** | 无（仅按键驱动） | 强（Triggers regex 匹配终端输出 + Instant 模式） | 强（DCS Hooks 按 shell 生命周期自动触发） | 弱（仅事件驱动，无 terminal-output 模式匹配） |
| **第三方扩展** | Fragment Extensions（JSON 文件放置） | Python API（脚本文件 + AutoLaunch 目录） | Agent SDK（npm 包 + warp:// 协议） | Lua 配置模块（require） |
| **通知能力** | 无 | Post Notification + Bounce Dock Icon + Show Alert | 内置桌面通知 + OSC 9/777 自定义通知 | 无内置通知，可通过 `wezterm.run_child_process` 调用外部工具 |
| **Shell 集成** | 无（纯终端模拟） | Shell Integration（Prompt Detected + Report Directory + Report User & Host triggers） | DCS Hooks（自动注入 RC 文件，全生命周期跟踪） | User Variables（OSC 1337）+ Pane 信息查询 |
| **学习曲线** | 低 | 中高 | 中 | 高 |
| **生态系统** | 小（Microsoft 自身体系） | 大（成熟社区 + 大量 Python 脚本） | 增长中（Agent 生态 + 商业产品） | 中（Lua 社区 + WezTerm 插件） |

---

## 对 slTerminal 的启示

1. **iTerm2 Triggers 是最成熟的"条件→动作"模型**：regex 匹配终端输出 → 26 种动作，完全通过 GUI 表格管理。slTerminal 可参考其表格编辑器设计，提供类似的可视化 hook 配置面板。

2. **Warp 的 DCS Hook 协议是 shell 生命周期跟踪的现代化方案**：通过转义序列自动注入 RC 文件，实现 Precmd/Preexec/CommandFinished 等事件。slTerminal 已有 OSC 133（提示符边界检测），可扩展为更完整的 hook 体系。

3. **WezTerm 的 Lua Event System 是代码配置的最高自由度方案**：`wezterm.on` + `wezterm.emit` + `action_callback` 三件套提供完整的事件编程模型。但其可视化程度为零——所有配置都在代码中，用户需要编程能力。

4. **Windows Terminal 的 Actions + Keybindings 是最简单的绑定模型**：command + keys 的 JSON 映射，配合 Command Palette 发现。但其 hook 能力极弱——没有终端输出匹配的条件触发。

5. **所有四个终端都提供了某种形式的 Notification 机制**：iTerm2 的 Post Notification/Show Alert，Warp 的 OSC 9/777 桌面通知，WezTerm 依赖外部工具，Windows Terminal 无此能力。通知是 hook 的重要输出通道。

6. **GUI vs 代码配置的分野**：iTerm2 提供最完善的 GUI 触发器管理，WezTerm 提供最强大的代码配置，Warp 介于两者之间（GUI + 转义序列），Windows Terminal 最受限（纯 JSON）。slTerminal 定位需在易用性和灵活性间权衡。
