# D5 汇总报告 -- 事实核查

## 错误 1: "stdin/stdout JSON 协议成为事实标准"过度概括

- **文件+行号**: `D5-excellent-projects.md` (行 23-35)
- **原声称**: "Cursor、Windsurf、Copilot CLI、Codex CLI、Gemini CLI、Claude Code 6 个主流 AI 编程工具均采用相同的 hook 传输契约：stdin JSON 输入、stdout JSON 输出、exit 0/2 语义"
- **错误类型**: 事实错误（过度概括）
- **正确信息**: 
  - Cursor、Windsurf、Gemini CLI、Claude Code 确实使用原始 stdin/stdout JSON + exit code 语义
  - Copilot CLI 使用 **JSON-RPC over stdio**（结构化协议，含 request/response/notification，非原始 stdin→stdout 管道），其 SDK 扩展模式通过 `joinSession()` 建立持久化 JSON-RPC 连接
  - Codex CLI 使用 **JSON-RPC over stdio（JSONL 格式）**，其 App Server 协议是双向 JSON-RPC，非简单的 stdin→stdout 单次调用
  - 相同点仅限于"都经 stdio 传 JSON"，但协议层不兼容——Copilot CLI 的 SDK 扩展不能直接替换 Claude Code hook 脚本
- **反证来源**: 
  - Copilot CLI SDK 文档: `github.com/github/copilot-sdk/blob/main/nodejs/docs/extensions.md` — 明确使用 JSON-RPC over stdio 的 `joinSession()` API，非原始 stdin/stdout JSON
  - Codex App Server: `openai.com/index/unlocking-the-codex-harness/` — "双向 JSON-RPC 协议"、"JSONL 格式"，非单次 stdin→stdout
  - `github.com/weykon/agent-hooks` — 跨工具统一接口的存在本身反证各工具协议不兼容

## 错误 2: Claude Code "12+" hook 事件数低估

- **文件+行号**: `D5-excellent-projects.md` (行 265 对比表格)
- **原声称**: Claude Code 列"12+" hook 事件数
- **错误类型**: 事实错误（计数不完整）
- **正确信息**: D5a 子报告详细列出 Claude Code 有 **12 个核心事件**（SessionStart、Setup、UserPromptSubmit、PreToolUse、PostToolUse、PostToolUseFailure、Stop、SubagentStop、PreCompact、Notification、PermissionDenied、FileChanged），另加实验性事件（ElicitationResult、WorktreeCreate 等），总计 **14+**
- **反证来源**: 
  - D5a 子报告 (行 331-345) 明确列出 12+2 个事件
  - 官方文档: `code.claude.com/docs/en/hooks`

## 错误 3: Gemini CLI 事件数表述不准确（11 vs 实际完整列表）

- **文件+行号**: `D5-excellent-projects.md` (行 43、264)
- **原声称**: "Gemini CLI: 11 个事件"
- **错误类型**: 来源不支撑
- **正确信息**: D5a 子报告 (行 230-238) 仅列出 9 个带描述的事件（SessionStart/SessionEnd/BeforeAgent/AfterAgent/BeforeModel/AfterModel/BeforeToolSelection/BeforeTool/AfterTool/PreCompress/Notification = 11），但实际 geminicli.com/docs/hooks 列出了更完整的事件集。此外，Gemini CLI 的 BeforeModel/AfterModel 是**独有的细粒度事件**（其他工具没有），这使得事件计数直接对比意义不大
- **反证来源**: geminicli.com/docs/hooks (已验证存在 BeforeModel/AfterModel)

## 错误 4: Windsurf 收购金额未核实

- **文件+行号**: `D5-excellent-projects.md` (传递给 D5a)
- **原声称**: "约 2.5 亿美元收购"
- **错误类型**: 来源不支撑
- **正确信息**: 收购确认发生（2025年 Cognition 收购 Codeium/Windsurf），2026年6月2日更名为 Devin Desktop。但 **收购金额未在公开来源中明确披露**——搜索 "$250M" 时未找到独立验证来源，多家报道仅提"收购"未提金额
- **反证来源**: 
  - `apidog.com/blog/whats-new-in-devin-2026/` — 确认收购和更名，但未提金额
  - `dev.to/jovan_chan_.../windsurf-is-now-devin-desktop` — 确认 June 2, 2026 更名

## 错误 5: Copilot CLI "13 JSON hooks + 6 SDK hooks"计数存疑

- **文件+行号**: `D5-excellent-projects.md` (行 265)
- **原声称**: Copilot CLI "13+6 SDK"
- **错误类型**: 来源不支撑
- **正确信息**: SDK 扩展的 6 个核心 hook（onSessionStart/onUserPromptSubmitted/onPreToolUse/onPostToolUse/onErrorOccurred/onSessionEnd）已确认。但 JSON 配置文件 hooks 的 **13 个事件**与 SDK 事件高度重叠（sessionStart/sessionEnd/preToolUse/postToolUse/postToolUseFailure/permissionRequest/preCompact/agentStop/subagentStart/subagentStop/errorOccurred/notification/userPromptSubmitted = 13），两者不应简单相加——它们是同一事件集的不同配置方式。D5a 将其计为 "13+6" 暗示 19 个不同事件，有误导
- **反证来源**: D5a 子报告 (行 134-138) 列出 JSON 配置的 13 个事件名，与 SDK 6 个事件名明显重叠

## 错误 6: "P0 建议"中的 PreToolUse/PostToolUse 定位不适用于终端层

- **文件+行号**: `D5-excellent-projects.md` (行 316)
- **原声称**: "P0: PreToolUse/PostToolUse 事件（基于 OSC 133 C/D）"
- **错误类型**: 内部矛盾
- **正确信息**: 报告自身多次强调 slTerminal 是**终端级 hook**（非应用层），但 P0 建议直接引用 Claude Code 应用层的 PreToolUse/PostToolUse 事件名。终端层 PTY 事件应使用 PTY 生命周期术语（如 Precmd/Preexec/CommandFinished 或 PTY Write/Read）。与 D5c (行 105) 自身建议一致：应使用 "OSC 133 C/D → shell 生命周期 hook"
- **反证来源**: 同文件行 45 自身定义："slTerminal 定位为终端级 hook（非应用层），应聚焦中粒度：PTY 输入/输出生命周期、命令执行前后、OSC 序列检测"

## 错误 7: WezTerm "12 个内置事件"略微不准确

- **文件+行号**: `D5-excellent-projects.md` (行 111)
- **原声称**: "12 个内置事件"
- **错误类型**: 事实错误（轻微）
- **正确信息**: WezTerm 文档 `wezterm.org/config/lua/window-events/` 列出 **12 个 window 事件**，另加 **gui-startup** 和 **gui-attached** 两个 GUI 事件（非 window 事件），以及**自定义事件**（`wezterm.emit`）。严格说 window events 确实是 12 个，但"内置事件"应该包括 GUI 事件，总计 14 个
- **反证来源**: 
  - `wezterm.org/config/lua/window-events/` — 12 window events
  - `github.com/wezterm/wezterm/blob/main/docs/config/lua/gui-events/gui-startup.md` — 额外 GUI 事件

## 错误 8: 来源列表中缺少对 Gemini CLI 即将被替代的注明

- **文件+行号**: `D5-excellent-projects.md` (行 340)
- **原声称**: "Gemini CLI | Google 官方文档 | 2025-2026"
- **错误类型**: 过时信息
- **正确信息**: Google 于 **2026年6月18日** 弃用了 Gemini CLI 的免费/个人层级，转向新的 **Antigravity CLI** (`agy`)。付费 API key 和开源 `gemini` 二进制不受影响。作为 2026-07-25 的研究报告，应注明此重要变化
- **反证来源**: geminicli.com 公告 (2026-06-18)
