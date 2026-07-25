# D5c 终端 Hook/Event 可视化 -- 事实核查

## 错误 1: Warp DCS Hook "13 个 shell 生命周期事件"实际有 17 个

- **文件+行号**: `D5c-terminal-hooks-visualization.md` (行 242-256)
- **原声称**: 列出 12 个 DCS Hook（Precmd/Preexec/CommandStarted/CommandFinished/Bootstrapped/InitShell/InitSubshell/InputBuffer/Clear/SSH/InitSsh/ExitShell/SourcedRcFileForWarp = 13）
- **错误类型**: 事实错误
- **正确信息**: Warp 源码 `dcs_hooks.rs` 的 `DProtoHook` 枚举定义了 **17 个**生命周期 hook 类型。D5c 遗漏了: `PreInteractiveSSHSession`、`FinishUpdate`、`RemoteWarpificationIsUnavailable`、`SshTmuxInstaller`、`TmuxInstallFailed`。D5c 的表格列了 12 个 hook 名 + 1 个隐含的 "CommandStarted"（实际源码中没有独立 CommandStarted 事件——它是从 Preexec 派生的）
- **反证来源**: 
  - `github.com/warpdotdev/Warp/blob/9c5c4253/app/src/terminal/model/ansi/dcs_hooks.rs` — DProtoHook 枚举 17 variants
  - CVE-2026-54686 披露 — 确认 hook 类型数量

## 错误 2: Warp OSC 777 Agent States 表格不完整

- **文件+行号**: `D5c-terminal-hooks-visualization.md` (行 263-272)
- **原声称**: 列出 5 个 agent session 状态
- **错误类型**: 事实错误（不完整）
- **正确信息**: Warp 的 `CLIAgentSessionsModel` 状态机有 3 个核心内部状态（InProgress/Success/Blocked），映射到 OSC 777 v1 schema 的 **7 个事件**：session_start、prompt_submit、tool_complete、stop、idle_prompt、question_asked、permission_request。D5c 仅描述了 5 个状态的文字概要，遗漏了具体的协议事件名
- **反证来源**: 
  - `warpdotdev/claude-code-warp` — 6 个 hook 配置，对应 OSC 777 事件
  - Warp source `dcs_hooks.rs` + `handler.rs` — per-tab state machine

## 错误 3: iTerm2 Triggers "26 种动作"计数近似正确但表格有描述不准确

- **文件+行号**: `D5c-terminal-hooks-visualization.md` (行 106-133)
- **原声称**: 列出 26 种 Trigger Action
- **错误类型**: 事实错误（轻微）
- **正确信息**: 
  - 26 这个数字基本正确，但表格中部分 action 名称与实际文档不符:
    - D5c 写的 "Change Style" 实际文档中可能指组合操作（多个 UI 元素同时变化）
    - D5c 写的 "Fold to Named Mark" 实际 iTerm2 3.5.12+ 中名为 **"Fold Section"** 而非 "Fold to Named Mark"
    - D5c 写的 "Inject Data" 在文档中简称为 **"Inject"**
  - iTerm2 3.5.12 (2025年4月) 新增了 Set Named Mark 和 Fold Section——D5c 未注明版本变化
- **反证来源**: 
  - `iterm2.com/documentation-triggers.html` — 官方 26 种 action 清单
  - iTerm2 3.5.12 changelog — 新增 Fold Section、Set Named Mark

## 错误 4: Warp OSC 9 描述中缺少对格式限制的说明

- **文件+行号**: `D5c-terminal-hooks-visualization.md` (行 222-233)
- **原声称**: "OSC 9: 仅正文通知"、"OSC 777: 标题+正文通知"
- **错误类型**: 事实错误（不完整）
- **正确信息**: 
  - OSC 9 格式为 `ESC ] 9 ; <body> BEL` — 仅正文，描述正确
  - OSC 777 的 `warp://cli-agent` 协议更为复杂——它使用结构化 JSON payload（`{"v":1, ...}`），不仅仅用于"标题+正文通知"，而是 Warp 的 **per-tab agent 状态机的主要通信通道**
  - D5c 将 OSC 777 简化为通知机制，忽略了其在 Warp 架构中更核心的 agent 状态跟踪角色
  - Payload 中应避免换行和分号（或需正确转义）
- **反证来源**: 
  - `docs.warp.dev/terminal/more-features/notifications/` — OSC 9/777 文档
  - `warpdotdev/claude-code-warp` — OSC 777 实际用于 agent 状态跟踪，非仅通知

## 错误 5: WezTerm "12 个内置事件"遗漏 GUI 事件

- **文件+行号**: `D5c-terminal-hooks-visualization.md` (行 303-318)
- **原声称**: 列出 12 个内置事件
- **错误类型**: 事实错误（不完整）
- **正确信息**: D5c 列出了 12 个 window events（format-tab-title、format-window-title、update-status、update-right-status、augment-command-palette、bell、new-tab-button-click、open-uri、user-var-changed、window-config-reloaded、window-focus-changed、window-resized），但遗漏了 **gui-startup** 和 **gui-attached** 两个 GUI 事件（自 20220624 版本起可用）。这两者不是 window events 而是 GUI lifecycle events，但在"内置事件"总览中应该包括
- **反证来源**: 
  - `wezterm.org/config/lua/window-events/` — 12 window events
  - `github.com/wezterm/wezterm/blob/main/docs/config/lua/gui-events/gui-startup.md` — 额外 GUI 事件

## 错误 6: Windows Terminal "Proto Extensions（规划中）"表述不准确

- **文件+行号**: `D5c-terminal-hooks-visualization.md` (行 75)
- **原声称**: "Proto Extensions（规划中）：允许外部程序生成 JSON 片段注入 Terminal 配置"
- **错误类型**: 过时信息
- **正确信息**: "Proto extensions" 是最初的 spec 名称（2020年左右）。该功能已在 PR #7632 中实现并发布，正式名称为 **JSON Fragment Extensions**。v1.24 (2024-2025) 已加入 Extensions 管理 UI。D5c 将已发布的功能标注为"规划中"不准确。原文的 "Proto extensions-spec.md" 是历史 spec 文档（2020年），不应引为当前状态
- **反证来源**: 
  - `learn.microsoft.com/en-us/windows/terminal/json-fragment-extensions` — 生产文档，非"规划中"
  - `app.semanticdiff.com/gh/microsoft/terminal/commit/654c0cc...` — PR #7632 实现
  - v1.24 changelog — Extensions UI 发布

## 错误 7: Warp 对比表中"DCS Hooks 无手动配置"的表述可能过时

- **文件+行号**: `D5c-terminal-hooks-visualization.md` (行 286)
- **原声称**: "DCS Hooks 无手动配置——全部由 shell 集成脚本自动注入，对用户透明"
- **错误类型**: 过时信息
- **正确信息**: Warp 于 2026 年 5 月开源（AGPL-3.0），第三方开发者现在可以直接修改源码配置 DCS hooks。此外，Warp 的 `warp://` deeplinks 和 `WARP_CLI_AGENT_PROTOCOL_VERSION` 环境变量允许扩展进行协议协商和自定义配置。"对用户透明"对基础功能成立，但对开发者而言，2026 年 5 月后配置面已显著扩大
- **反证来源**: 
  - Warp 开源公告 (2026年5月)
  - `warpdotdev/claude-code-warp` — 第三方插件显示 DCS 配置并非完全透明
