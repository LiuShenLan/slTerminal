# Claude Code Hooks 信息检索总览

> 检索日期：2026-07-25
> 目的：为 slTerminal 针对 Claude Code hooks 功能的优化决策提供信息依据

---

## 检索范围

针对 Claude Code CLI 内置 hooks 机制（`settings.json` 中的 `hooks` 字段）的五个方向进行系统检索。

## 文件索引

### D1 — hooks 的视觉/交互反馈

终端模拟器中 hook 运行状态的可视化展示（状态指示器、进度条、通知 toast 等）。

| 文件 | 规模 | 内容 |
|------|------|------|
| [D1/D1-visual-feedback.md](D1/D1-visual-feedback.md) | 550 行 | **汇总报告**：30+ 事件完整列表、终端进度标准、第三方工具生态、社区讨论、对 slTerminal 的启示 |
| [D1/01-hooks-official-docs.md](D1/01-hooks-official-docs.md) | 901 行 | Claude Code hooks 官方文档深入分析——30+ 事件、5 种 handler 类型、输入/输出 JSON 格式、matcher 语法 |
| [D1/02-third-party-tools.md](D1/02-third-party-tools.md) | 778 行 | 18 个第三方工具按 6 类分析（Tab 颜色、HUD、TUI 仪表盘、GUI 等） |
| [D1/03-terminal-progress-standards.md](D1/03-terminal-progress-standards.md) | 475 行 | 终端进度标准——OSC 9;4、iTerm2 Shell Integration、WezTerm Lua 事件、Kitty remote control |
| [D1/04-community-discussions.md](D1/04-community-discussions.md) | 331 行 | 31 个 GitHub Issues + HN/Reddit 讨论 + feature request |

### D2 — hooks 的配置管理 UI

`settings.json` hooks 配置的图形化管理方案、Schema 定义、最佳实践。

| 文件 | 规模 | 内容 |
|------|------|------|
| [D2/D2-config-management.md](D2/D2-config-management.md) | — | **汇总报告**：完整 Schema、配置结构、5 种 Handler 类型、JSON Schema 驱动 UI 方案建议 |
| [D2/01-hooks-official-docs.md](D2/01-hooks-official-docs.md) | — | 官方 hooks 配置文档——事件类型、matcher 语法、配置字段 |
| [D2/02-settings-json-schema.md](D2/02-settings-json-schema.md) | — | SchemaStore 官方 JSON Schema（`claude-code-settings.json`）、$schema 字段支持 |
| [D2/03-vscode-config-ui-reference.md](D2/03-vscode-config-ui-reference.md) | — | VS Code tasks.json/launch.json 配置 UI 设计参考 |
| [D2/04-jetbrains-config-ui-reference.md](D2/04-jetbrains-config-ui-reference.md) | — | JetBrains External Tools/File Watchers 配置 UI 参考 |
| [D2/05-community-hooks-examples.md](D2/05-community-hooks-examples.md) | — | GitHub 上公开的 `.claude/settings.json` hooks 实际配置案例 |

### D3 — hooks 的输出/日志可视化

hooks 执行历史、stdout/stderr 日志的存储与展示方案。

| 文件 | 规模 | 内容 |
|------|------|------|
| [D3/D3-output-logging.md](D3/D3-output-logging.md) | — | **汇总报告**：exit code 三维矩阵、debug 日志格式、Warp Block 模型参考、推荐架构 |

### D4 — hooks 与终端会话(PTY)集成

SessionStart/Stop hooks 与终端 shell 会话联动方案。

| 文件 | 规模 | 内容 |
|------|------|------|
| [D4-pty-integration.md](D4-pty-integration.md) | — | **汇总报告**：hook 执行环境、SessionStart/Stop 模式、AgentPTY 集成方案、环境变量传递链、Windows 关键 Bug |

### D5 — 其他优秀项目的 hooks 优化案例

同类 AI 编程工具、终端模拟器、IDE 的扩展/hook/event 系统设计参考。

| 文件 | 规模 | 内容 |
|------|------|------|
| [D5/D5-excellent-projects.md](D5/D5-excellent-projects.md) | 365 行 | **汇总报告**：6 条行业共识、对比表格、优先级路线图 |
| [D5/D5a-ai-tools-hooks.md](D5/D5a-ai-tools-hooks.md) | 627 行 | 7 个 AI 编程工具 hook 系统横向对比（Cursor/Windsurf/Copilot/Codex/Gemini/aider/Claude Code） |
| [D5/D5b-claude-code-community.md](D5/D5b-claude-code-community.md) | 489 行 | GitHub 15+ 仓库 + Reddit/HN 20+ 讨论 + 6 个通知工具 |
| [D5/D5c-terminal-hooks-visualization.md](D5/D5c-terminal-hooks-visualization.md) | 399 行 | Windows Terminal/iTerm2/Warp/WezTerm 四终端对比 |
| [D5/D5d-ide-task-system-ui.md](D5/D5d-ide-task-system-ui.md) | 459 行 | VS Code Tasks/Extensions + JetBrains Run Configurations |

---

## 统计

- **文件总数**：19 个 markdown 文件
- **总行数**：约 7000+ 行
- **代理调用**：5 个顶层代理 + 各方向子代理（最大嵌套 3 层）
- **信息来源**：官方文档（code.claude.com）> GitHub issues/discussions > SchemaStore > 技术博客 > Reddit/Hacker News > 各工具官方文档

## 六个关键发现（跨方向共识）

1. **stdin/stdout JSON 协议已是事实标准**：Cursor/Windsurf/Copilot CLI/Codex CLI/Gemini CLI/Claude Code 全部采用同一 hook 传输契约（D5）
2. **30+ 个 hook 事件**分 9 大类，exit code 2 为唯一阻断方式，exit code 0/1/2 行为按事件类型而异（D1、D2、D3）
3. **claude-hud**（statusline API 原生集成）是最轻量的视觉反馈方案，第三方工具标准架构为 `hooks → 信号文件(JSON) → 终端适配器 → 可视化`（D1）
4. **官方 JSON Schema 已存在**（SchemaStore `claude-code-settings.json`），2025-09 起 `$schema` 字段正式支持 IDE 补全和校验（D2）
5. **Hook 无 TTY 附着**是设计决策——通过 `CLAUDE_ENV_FILE` 做环境持久化，无直接 PTY 通信通道（D4）
6. **社区最大痛点**：缺少 hook 运行时遥测（Issue #50287）、hooks.log 无轮转导致 48GB 文件（Issue #16047）（D3）
