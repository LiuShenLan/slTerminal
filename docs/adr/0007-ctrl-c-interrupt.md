# Ctrl+C 保留为中断，Ctrl+Shift+C 复制

Status: Accepted

键盘处理中 `Ctrl+C` 永不拦截，由 xterm.js 自然发送 `\x03`（SIGINT）到 PTY。复制快捷键改为 `Ctrl+Shift+C`，粘贴为 `Ctrl+Shift+V`。

**为什么不是 Ctrl+C 复制？** 本终端为 Claude Code CLI 优化。Claude 靠 `Ctrl+C` 取消正在进行的操作——如果终端拦截了 `Ctrl+C` 用于复制，用户就无法中断 Claude。这是个硬需求，不是偏好。

**为什么不是"有选区时复制、无选区时中断"？** xterm.js 的 `_keyDown` 在 `hasSelection()` 检查前就已经 dispatch 了按键数据。选择状态和按键分发在不同的代码路径，无法在捕获阶段可靠判断。

**备选方案**：右键复制（保留 Ctrl+C 中断，但不符合终端用户习惯）；Ctrl+Insert 复制（Windows 不常用）。

**后果**：违反 Windows 用户肌肉记忆（Ctrl+C = 复制）。需要向用户明确告知快捷键方案。这是为 Claude CLI 使用场景做的刻意取舍。
