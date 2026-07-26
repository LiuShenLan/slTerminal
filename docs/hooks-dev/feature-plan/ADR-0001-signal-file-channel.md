# ADR-0001：信号文件通道 + SLTERM_PANEL_ID 页签路由

**状态**：accepted（2026-07-26， grilling 问答 Q2/Q3 确认）

## 决策

slTerminal 感知 Claude Code hook 事件的通道采用**信号文件架构**：hook 脚本（注入用户 `~/.claude/settings.json`）把事件写为 JSON 到约定路径，后端监听目录并经 IPC 推送前端；辅以 transcript JSONL 被动解析补充上下文用量。会话→页签路由采用 **`SLTERM_PANEL_ID` 环境变量**：`pty_spawn` 时注入子进程环境块，经 shell → claude → hook 脚本继承链传递，写入信号文件实现精确路由。

## 理由

官方无 hook 运行时遥测（Issue #50287），hook 子进程无 TTY（无法直接向终端写 OSC 序列）。信号文件是唯一同时满足「拿到 hook 执行事件 + 跨页签精确路由 + 与 Claude Code 解耦」的通道，且已被 tabby-claude-status 等第三方工具在 Windows 上验证。`SLTERM_PANEL_ID` 注入与现有 COLORTERM/TERM 注入同一时机，零猜测路由（cwd 匹配在同项目多页签时有歧义）。

## 否决的备选

| 备选 | 否决理由 |
|------|---------|
| 纯 transcript JSONL 解析 | 拿不到 hook 执行事件本身（官方无遥测），无法支撑四态的实时性 |
| PTY 输出被动扫描 | 模式识别精度差，「空闲」等内部状态无法从输出准确推断（D1 §5.3 明确否定） |
| HTTP localhost 服务 | 需开端口，引入端口冲突/安全面，复杂度高于文件通道 |
| debug 日志解析 | 需 `--debug` 启动 claude，不适合常态使用 |
| cwd 匹配路由 | 同项目多页签时歧义，无法精确路由 |

## 后果

- **需修改用户的 `~/.claude/settings.json`**：必须 merge 追加（不覆盖既有配置）、必须可一键卸载（配置段 + 脚本文件干净移除）、仅手动按钮触发
- **hook 脚本在用户全局配置中常驻**：用户在其他终端（非 slTerminal）启动 claude 时脚本同样会运行——脚本必须对环境容错（信号目录不可写/不存在时静默 `exit 0`），且任何情况下 exit code 恒为 0，绝不干扰 claude 自身流程（exit 2 会阻断操作）
- **脚本解释器选择受 Windows 已知 bug 约束**：`UserPromptSubmit` 空 stdin bug（#48009）的社区方案是配置 `CLAUDE_CODE_GIT_BASH_PATH` 走 Git Bash；SessionStart 冻结 bug（#23554）官方已修复但需回归关注
- 卸载后用户的 claude 配置恢复原状，slTerminal 页签状态功能降级为仅 OSC 133 C 启动检测（🟡）
