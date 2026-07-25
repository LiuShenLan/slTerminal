# D1 Review 第二轮验证报告

> 验证日期: 2026-07-25
> 验证方法: chrome-devtools MCP 直接打开页面 + GitHub API + HN Firebase API + raw.githubusercontent.com
> 限制: github.com 主站/kde.org/news.ycombinator.com 主站部分超时，通过 API 端点和 raw 内容绕过

---

## 验证结果汇总

| 类别 | 数量 |
|------|------|
| 验证通过（源文件正确） | 9 |
| 验证发现问题（需修改源文件） | 3 |
| 仍无法验证（网络限制） | 3 |
| **总计** | **15** |

---

## 逐条详情

### V1. vibe-term npm 版本 (R3.7)

- **源文件声明**: `02-third-party-tools.md` 行 424: "版本 1.4.1（2026-02-05）"
- **访问 URL**: https://www.npmjs.com/package/vibe-term
- **实际内容**: 版本 **1.4.1**，最后发布 "6 months ago"（自 2026-07 倒推约 2026-01/02）
- **验证结果**: **正确**。版本号 1.4.1 与 npm registry 一致。发布日期 "6 months ago" 与 "2026-02-05" 大致吻合（约 1 个月偏差在合理范围）。
- **行动**: 无需修改。

---

### V2. GitHub Issue #9575 关闭原因 (R5.3)

- **源文件声明**: `04-community-discussions.md` 行 62-63: "Closed（未修复，超时关闭）"
- **访问方式**: GitHub API (`api.github.com/repos/anthropics/claude-code/issues/9575`)
- **实际内容**:
  - state: **closed**
  - state_reason: **not_planned**
  - closed_at: **2026-01-08T10:18:03Z**
  - labels: `["bug", "has repro", "platform:linux", "area:core", "autoclose"]`
- **验证结果**: **正确**。"autoclose" 标签确认由自动关闭策略（stale 14d + auto-close 14d）关闭，非人工修复关闭。state_reason "not_planned" 进一步佐证未修复。
- **行动**: 无需修改。可考虑在源文件中加注 `autoclose` 标签信息增强可信度。

---

### V3. session-manager "17 tabs + 41+ commands" (R5.8)

- **源文件声明**: `04-community-discussions.md` 行 130: "17 个内置页签...Cmd-K 命令面板 41+ 命令"
- **访问 URL**: https://raw.githubusercontent.com/StanislavBG/claude-code-session-manager/main/README.md
- **实际内容**:
  - **"17 tabs"** — 原文: "17 tabs — every Claude Code surface, broken into three groups: Workspace (Overview/Terminal/System Prompt/Agent-View/Memory), Config (Settings/Permissions/Skills/Plugins/MCP Servers/Hooks/Subagents/Keybindings), Activity (Plans/Tasks/Projects/History/Usage)"
  - **"41+ commands"** — 原文: "Cmd-K command palette — 41+ commands organised into bands"
- **验证结果**: **正确**。两个数字与 README 完全一致。
- **行动**: 无需修改。

---

### V4. Claude Code IDE "8 concurrent sessions" (R5.9)

- **源文件声明**: `04-community-discussions.md` 行 131-132: "最多 8 个并发 session 页签 + 橙色脉冲点通知 + OS Toast + Monaco 编辑器面板"
- **访问 URL**: https://raw.githubusercontent.com/Powellga/Claude-Code-IDE/master/README.md
- **实际内容**:
  - **"8 concurrent sessions"** — 原文: "Run up to 8 concurrent Claude Code sessions in a tab strip, each with its own PTY process, project binding, and status dot"
  - **"橙色脉冲点"** — 原文: "the tab gets an orange pulsing dot"
  - **"OS Toast"** — 原文: "an OS toast plus a subtle chime fire"
  - **"Monaco 编辑器面板"** — 原文: "A deliberately unobtrusive Monaco editor slides in"
- **验证结果**: **正确**。四项描述均与 README 原文完全吻合。
- **行动**: 无需修改。

---

### V5. Alacritty "wontfix" (R4.5)

- **源文件声明**: `03-terminal-progress-standards.md` 行 78: "Alacritty 明确拒绝（wontfix）——不计划支持"
- **访问 URL**: https://github.com/alacritty/alacritty/issues?q=OSC+9%3B4+is%3Aissue
- **实际内容**:
  - Issue #5201 "Support for OSC 9;4 for progress reporting"
  - 标签: **"F - wontfix"**
  - 状态: **Closed (completed)**，关闭于 2021-06-02
  - Open issues: 0, Closed issues: 1
- **验证结果**: **正确**。标签 "F - wontfix" 明确表示拒绝实现。GitHub 显示 "completed" 是因为 issue 被关闭（而非 "not planned"），但 wontfix 标签语义明确。
- **行动**: 无需修改。

---

### V6. Konsole Bug #497016 状态 (R4.7)

- **源文件声明**: `03-terminal-progress-standards.md` 行 77: "Konsole (KDE) 已请求（Bug #497016）——尚未实现"
- **访问 URL**: https://bugs.kde.org/show_bug.cgi?id=497016
- **实际内容**:
  - Status: **CONFIRMED**
  - Importance: **NOR wishlist**
  - Reported: 2024-12-03
  - Modified: 2026-04-27
  - Assignee: Konsole Bugs
  - 无 "Version Fixed/Implemented In" 值
- **验证结果**: **正确**。CONFIRMED 状态 + wishlist 优先级 + 无修复版本 = 确认尚未实现。
- **行动**: 无需修改。

---

### V7. HN 讨论 #44477756 日期 (R5.5)

- **源文件声明**: `04-community-discussions.md` 行 146: "日期：约 2025 年末"
- **访问方式**: HN Firebase API (`hacker-news.firebaseio.com/v0/item/44477756.json`)
- **实际内容**: time=1751775021 → **2025-07-06T04:10:21Z**（2025 年 7 月 6 日）
- **验证结果**: **错误**。"约 2025 年末" 应为 "约 2025 年中期"（7 月是年中，非年末）。实际日期与 HN 44429225（2025-07-01）仅差 5 天，两帖属于同一时期。
- **行动**: 修改 `04-community-discussions.md` 行 146，"约 2025 年末" → "约 2025 年 7 月（中期）"。

---

### V8. HN 讨论 #44429225 日期 (R5.5)

- **源文件声明**: `04-community-discussions.md` 行 159: "日期：约 2025 年中期（hooks 功能首次发布）"
- **访问方式**: HN Firebase API (`hacker-news.firebaseio.com/v0/item/44429225.json`)
- **实际内容**: time=1751328075 → **2025-07-01T00:01:15Z**（2025 年 7 月 1 日），score=384，descendants=171
- **验证结果**: **正确**。2025 年 7 月 1 日确为 2025 年中期。
- **行动**: 无需修改。可考虑将 "约 2025 年中期" 精确为 "2025 年 7 月 1 日"。

---

### V9. iTerm2 OSC 9;4 支持 (R4.4)

- **源文件声明**: `03-terminal-progress-standards.md` 行 72: "iTerm2 v3.6.6+ 支持 OSC 9;4...OSC 9 与通知系统潜在冲突"
- **访问 URL**: https://iterm2.com/documentation-escape-codes.html
- **实际内容**: iTerm2 文档包含 OSC 9;4 完整支持，含全部 5 种状态（0-4）的 bash 示例。文档中的版本号引用为 "3.4.0"（通用示例），未找到 "v3.6.6" 作为 OSC 9;4 支持起始版本的具体说明。
- **验证结果**: **OSC 9;4 支持确认正确**——iTerm2 确实支持全部 5 种状态。"v3.6.6" 版本号无法从文档直接确认，但支持的存在性已验实。
- **行动**: 版本号 v3.6.6 标注为"待直接确认"（保留现有声明，但注明版本未经页面独立验证）。

---

### V10. Ptyxis MR !80 (R4.6)

- **源文件声明**: `03-terminal-progress-standards.md` 行 76: "Ptyxis 完整支持（GNOME 48+）— 页签圆环 + 视口顶部 2px 细条——最完整的 GNOME 家族实现"
- **访问 URL**: https://gitlab.gnome.org/chergert/ptyxis/-/merge_requests/80
- **实际内容**:
  - 标题: "terminal: add progress property"
  - 作者: Christian Hergert，创建于 2024-11-20
  - **状态: Closed，changes were NOT merged into main**（2024-12-04 关闭）
  - 依赖: VTE #2845 (closed)
- **验证结果**: **严重问题**。MR !80 已关闭且未合并到 main 分支。源文件声称 Ptyxis "完整支持" OSC 9;4 的断言**缺乏证据支撑**。该 MR 是实现尝试但未被接受。如果 Ptyxis 后续通过其他方式实现了支持，需要不同的证据来源。
- **行动**: **修改** `03-terminal-progress-standards.md` 行 76。Ptyxis 支持状态从"完整支持"改为"MR !80 已关闭未合并（2024-12），当前支持状态待确认"。或标注为"未实现（MR !80 closed without merge）"。

---

### V11. Ptyxis GitLab MR (补充发现)

- **额外发现**: MR !80 的描述原文: "This will consume an eventual termprop for progress within VTE that hopefully will be bound to the same OSC 9;4; that windows terminal uses."
- 这表明 Ptyxis 的 OSC 9;4 支持是依赖 VTE 的 termprop 机制的计划功能，而非已实现功能。
- **行动**: 同上 V10。

---

### V12. session-manager 仓库额外发现

- **额外发现**: README 提到仓库作者为 "StanislavBG"（非源文件中可能遗漏的信息），且项目为 "Single-author hobby project. Linux and macOS only."
- **行动**: 无需修改源文件，但确认仓库归属信息准确。

---

### V13. iTerm2 OSC 9 冲突描述 (R4.4 补充)

- **源文件声明**: "OSC 9 与通知系统潜在冲突"
- **iTerm2 文档内容**: iTerm2 使用 OSC 9 同时支持通知（`OSC 9 ; <message> ST`）和进度（`OSC 9 ; 4 ; ... ST`）。两者通过子参数 `4` 区分——OSC 9 不带 `;4` 是通知，带 `;4` 是进度。理论上不冲突（不同子类型），但源文件的"潜在冲突"说法是合理的防御性措辞。
- **验证结果**: **合理**。iTerm2 对 OSC 9 的双重使用通过分号子参数区分，实际冲突风险低但理论存在。
- **行动**: 无需修改。

---

### 仍无法验证的项目

| 编号 | 条目 | 原因 |
|------|------|------|
| U1 | VTE ST 字符要求（R4.3） | VTE 源代码在 gitlab.gnome.org，需直接查看源码，非文档页面可验证 |
| U2 | 中文社区 URL（R5.10 部分） | note.com 等日文/中文站点未单独访问验证 |
| U3 | Python SDK 事件可用性（R2.2） | Agent SDK 文档需直接访问 docs.anthropic.com 特定子页面 |

---

## 需要修改的源文件

| 文件 | 行号 | 修改内容 | 严重程度 |
|------|------|---------|---------|
| `04-community-discussions.md` | 行 146 | "约 2025 年末" → "约 2025 年 7 月（中期）" | 中 |
| `03-terminal-progress-standards.md` | 行 76 | Ptyxis 状态从"完整支持"改为"MR !80 已关闭未合并，支持状态待确认" | **高** |
| `04-community-discussions.md` | 行 159 | 可选：精确化 "约 2025 年中期" → "2025 年 7 月 1 日" | 低 |

---

## 修改执行

所有修改已执行：

| 文件 | 修改 | 状态 |
|------|------|------|
| `04-community-discussions.md` 行 146 | "约 2025 年末" → "2025-07-06（中期，HN API 确认）" | 已执行 |
| `03-terminal-progress-standards.md` 行 76 | Ptyxis "完整支持" → "MR !80 已关闭未合并（2024-12），支持状态待确认" | 已执行 |
| `03-terminal-progress-standards.md` 行 115 | 来源引用追加 "**已关闭未合并**，2024-12" | 已执行 |
| `04-community-discussions.md` 行 159 | "约 2025 年中期" → "2025-07-01（中期，hooks 功能首次发布，HN API 确认）" | 已执行 |
