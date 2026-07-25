# D5 第三次验证报告

## 问题

`D5-excellent-projects.md` 第八节"官方文档"表格中，blockquote（Gemini CLI 弃用说明）插入在 AI 工具条目和终端工具条目之间，截断了 Markdown 表格。终端工具 6 行（Windows Terminal / iTerm2 / Warp / WezTerm / VS Code / JetBrains）变成无表头孤立行。

## 修改内容

将 blockquote 从表格中间移到整个表格末尾之后。

## 修改前后对比

**修改前**（第 346-353 行）：
```markdown
| aider | https://aider.chat/docs/config.html | 2025-2026 |

> **注意**：Google 于 2026年6月18日弃用了 Gemini CLI...
| Windows Terminal | https://learn.microsoft.com/en-us/windows/terminal/ | 2026 |
| iTerm2 Triggers | https://iterm2.com/documentation-triggers.html | 2026 |
| Warp DCS Hooks | https://docs.warp.dev/features/sessions/dcs-hooks | 2026 |
| WezTerm Events | https://wezterm.org/config/lua/window-events/index.html | 2026 |
| VS Code Tasks | https://code.visualstudio.com/docs/editor/tasks | 2026 |
| JetBrains Run Configurations | https://plugins.jetbrains.com/docs/intellij/run-configurations.html | 2026 |
```

**修改后**（第 346-355 行）：
```markdown
| aider | https://aider.chat/docs/config.html | 2025-2026 |
| Windows Terminal | https://learn.microsoft.com/en-us/windows/terminal/ | 2026 |
| iTerm2 Triggers | https://iterm2.com/documentation-triggers.html | 2026 |
| Warp DCS Hooks | https://docs.warp.dev/features/sessions/dcs-hooks | 2026 |
| WezTerm Events | https://wezterm.org/config/lua/window-events/index.html | 2026 |
| VS Code Tasks | https://code.visualstudio.com/docs/editor/tasks | 2026 |
| JetBrains Run Configurations | https://plugins.jetbrains.com/docs/intellij/run-configurations.html | 2026 |

> **注意**：Google 于 2026年6月18日弃用了 Gemini CLI...
```

## 效果

- AI 工具（7 行）和终端工具（6 行）合并为一张完整的 13 行表格
- blockquote 移到表后，不再截断任何表格
- 表格头（`| 项目 | 文档 URL | 日期 |`）覆盖全部 13 行
