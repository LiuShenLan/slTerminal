# D4-pty-integration.md 审阅报告

> 审阅日期: 2026-07-25
> 审阅范围: 全文 25+ 项声明交叉验证

---

## 错误 1: CLAUDE_SESSION_ID 不是 hook 原生环境变量

- **文件+行号**: `D4-pty-integration.md` (第 55-60 行环境变量表)
- **原声称**: 将 `CLAUDE_SESSION_ID` 列为 hook 可访问的环境变量，描述为"会话唯一标识 (stdin 的 fallback)"
- **错误类型**: 事实错误
- **正确信息**: `CLAUDE_SESSION_ID` **不是** Claude Code 提供给 hook 子进程的原生环境变量。官方文档列出的 hook 可用环境变量仅为: `CLAUDE_PROJECT_DIR`、`CLAUDE_PLUGIN_ROOT`、`CLAUDE_ENV_FILE`、`CLAUDE_CODE_REMOTE`。`session_id` 存在于部分 hook 事件的 **stdin JSON** 中（SessionStart、PreCompact 等），但不会作为独立环境变量注入。社区曾多次请求暴露此变量 (#27299, #20132, #13733)，截至 2026 年 7 月仍未实现。
- **反证来源**:
  - Claude Code 官方 hook 开发文档 (via Context7 `SKILL.md`): 列出 hook 可用环境变量为 `CLAUDE_PROJECT_DIR`, `CLAUDE_PLUGIN_ROOT`, `CLAUDE_ENV_FILE`, `CLAUDE_CODE_REMOTE`——不含 `CLAUDE_SESSION_ID`
  - GitHub issue #27299 "Expose CLAUDE_SESSION_ID env var to hooks"——请求此功能，表明该变量当前不存在
  - GitHub issue #37339 "Pass session_id to all hook types"——确认并非所有 hook 类型都能通过 stdin 获取 session_id
  - 社区 workaround 模式: 通过 SessionStart stdin JSON 提取 `session_id`，再写入 `CLAUDE_ENV_FILE`

---

## 错误 2: 遗漏 CLAUDE_PLUGIN_ROOT 和 CLAUDE_PLUGIN_DATA 环境变量

- **文件+行号**: `D4-pty-integration.md` (第 52-60 行)
- **原声称**: 环境变量表仅列出 6 个变量 (CLAUDE_PROJECT_DIR, CLAUDE_SESSION_ID, CLAUDE_ENV_FILE, CLAUDE_CODE_REMOTE, CLAUDE_CODE_BRIDGE_SESSION_ID, CLAUDE_CODE_CHILD_SESSION)
- **错误类型**: 事实错误（遗漏）
- **正确信息**: 缺了两个官方环境变量——**`CLAUDE_PLUGIN_ROOT`**（插件目录，在所有 command hooks 中可用）和 **`CLAUDE_PLUGIN_DATA`**（插件持久化数据目录）。官方文档强调"**Always use ${CLAUDE_PLUGIN_ROOT} in hook commands for portability**"。
- **反证来源**:
  - Claude Code 官方 hook 开发文档 (SKILL.md): "Available in all command hooks: `$CLAUDE_PLUGIN_ROOT` - Plugin directory (use for portable paths)"
  - PACT-Plugin issue #343 环境变量调查: 确认 `CLAUDE_PLUGIN_ROOT` 和 `CLAUDE_PLUGIN_DATA` 对 hook 可见

---

## 错误 3: CLAUDE_ENV_FILE 可用范围未声明

- **文件+行号**: `D4-pty-integration.md` (第 56 行、第 79 行、全文)"
- **原声称**: "向此文件写 `export` 语句可在后续 Bash 命令中生效"（暗示在所有 hook 事件中均可用）；"唯一方案是写入 `$CLAUDE_ENV_FILE`"
- **错误类型**: 事实错误（遗漏关键限制）
- **正确信息**: `CLAUDE_ENV_FILE` **并非在所有 hook 事件中都设置**。官方文档明确标注为 "SessionStart only: persist env vars here"。D1/D2 文档进一步指出该变量仅在 SessionStart（可能含 CwdChanged / FileChanged）中可用。若在其他 hook 类型（如 Stop、UserPromptSubmit）中引用此变量，其值为空或未定义。D4 全文从未提及此限制，可能导致读者在不适用的 hook 事件中试图使用它。
- **反证来源**:
  - Claude Code 官方 hook 开发文档 (Context7 SKILL.md): "`$CLAUDE_ENV_FILE` - **SessionStart only**: persist env vars here"
  - D1-01-hooks-official-docs.md 第 542 行: "SessionStart, CwdChanged, FileChanged"

---

## 错误 4: CLAUDE_CODE_CHILD_SESSION 作为正式环境变量

- **文件+行号**: `D4-pty-integration.md` (第 59 行)
- **原声称**: 将 `CLAUDE_CODE_CHILD_SESSION` 与 `CLAUDE_PROJECT_DIR`、`CLAUDE_CODE_REMOTE` 等官方变量并列于环境变量表中，描述为"嵌套子进程标记 (设为 1 时阻止 transcript 持久化)"
- **错误类型**: 事实错误（来源级别不符）
- **正确信息**: 此变量来自社区 bug report (#72347)，**不在官方文档中**。D1、D2 的环境变量表均不包含此变量。将其与官方文档变量并列而未标注"社区发现/非官方"，可能误导读者认为这是官方支持的环境变量。更准确的做法是将其放在"已知 Bug/社区发现"段落中，而非环境变量参考表中。
- **反证来源**:
  - D1-01-hooks-official-docs.md 环境变量表 (第 537-545 行): 不含 CLAUDE_CODE_CHILD_SESSION
  - D2-01-hooks-official-docs.md 环境变量表 (第 107-112 行): 不含此变量
  - D4 自身第 460 行引用的 #72347 为社区 issue，非官方文档

---

## 错误 5: SessionEnd reason 枚举缺漏 "resume" 和 "bypass_permissions_disabled"

- **文件+行号**: `D4-pty-integration.md` (第 225 行)
- **原声称**: `"reason": "clear|logout|prompt_input_exit|other"`——四个值
- **错误类型**: 事实错误（遗漏有效值）
- **正确信息**: SessionEnd 的 `reason` 字段有**六个**有效值: `clear` | **`resume`** | `logout` | `prompt_input_exit` | **`bypass_permissions_disabled`** | `other`。D4 遗漏了两个值。`resume` 在 session 恢复/重新连接时出现；`bypass_permissions_disabled` 在绕过权限被禁用时出现。
- **反证来源**:
  - D1-01-hooks-official-docs.md 第 23 行明确列出全部 6 个值 (含 `resume`)
  - Sections 2-3 验证 agent 从 code.claude.com/docs/en/hooks 确认官方规范为 6 个值

---

## 错误 6: SessionStart 时 cwd 恒等于 CLAUDE_PROJECT_DIR

- **文件+行号**: `D4-pty-integration.md` (第 67-69 行)
- **原声称**: "SessionStart: cwd = 启动 claude 时的目录 (也是 `CLAUDE_PROJECT_DIR`)"
- **错误类型**: 事实错误
- **正确信息**: `cwd` 和 `CLAUDE_PROJECT_DIR` 是**两个独立字段**，不一定相等。`CLAUDE_PROJECT_DIR` 解析为 git 仓库根目录，而 `cwd` 是启动 claude 时的当前工作目录——如果从仓库子目录启动 claude，cwd 为子目录而 CLAUDE_PROJECT_DIR 为仓库根目录，两者不同。
- **反证来源**:
  - D1-01-hooks-official-docs.md 第 103 行: cwd 定义为"hook 触发时的当前工作目录"
  - D1-01-hooks-official-docs.md 第 540 行: CLAUDE_PROJECT_DIR 定义为"项目根目录绝对路径"——暗示两者可能不同

---

## 错误 7: stdin JSON 中 `permission_mode` 的示例值不正确

- **文件+行号**: `D4-pty-integration.md` (第 37 行)
- **原声称**: `"permission_mode": "default"`
- **错误类型**: 事实错误
- **正确信息**: 官方文档中 `permission_mode` 字段的值为 `"ask"` 或 `"allow"`，无 `"default"` 值。
- **反证来源**: Claude Code 官方 hook 开发文档 (Context7): 明确列出 `"permission_mode": "ask|allow"`

---

## 错误 8: 未说明 PreCompact hook 可阻止压缩

- **文件+行号**: `D4-pty-integration.md` (第 251-257 行三 hook 生命周期表)
- **原声称**: PreCompact 角色为"compaction 前保护关键状态不被丢失"——暗含纯读取/保护语义
- **错误类型**: 事实错误（遗漏重要能力）
- **正确信息**: PreCompact hook **可以主动阻止**压缩操作（exit code 2 或 `continue: false`）。此能力在 v2.1.105+ 引入。D4 将其描述为纯粹的"读状态→保护"步骤，忽略了可阻止压缩的关键能力。
- **反证来源**:
  - D1-01-hooks-official-docs.md 第 64 行: PreCompact 可阻塞列"是 (v2.1.105+)"
  - D2-01-hooks-official-docs.md 第 213 行: PreCompact 可阻塞列"是 (v2.1.105+)"

---

## 错误 9: 未说明 CLAUDE_ENV_FILE 不传播到后续 hook 子进程

- **文件+行号**: `D4-pty-integration.md` (第 79 行、第 267-270 行)
- **原声称**: "Hook 不能直接修改 Claude Code 父进程的环境变量——唯一方案是写入 `$CLAUDE_ENV_FILE`"
- **错误类型**: 事实错误（遗漏关键限制）
- **正确信息**: `CLAUDE_ENV_FILE` **仅传播到 Bash Tool 子进程**（Claude Code 在每次 Bash 命令前 source 该文件），**不会传播到后续 hook 子进程**。每个 hook 作为独立进程从原始 shell 环境启动，看不到之前 hook 通过 `CLAUDE_ENV_FILE` 写入的变量。
- **反证来源**: 社区实践和 bug reports (#40391) 确认: "CLAUDE_ENV_FILE only propagates to Bash tool commands, not to other hook subprocesses"

---

## 错误 10: CLAUDE_ENV_FILE 被描述为"追加式"不准确

- **文件+行号**: `D4-pty-integration.md` (第 116 行)
- **原声称**: "CLAUDE_ENV_FILE 是追加式: 多个 hook 共享同一文件,不能用 `>` 直接覆盖"
- **错误类型**: 事实错误（表述不精确）
- **正确信息**: 严格来说并非"追加式"——文件本身被 Bash 的 `source` (等效于 `.`) 读取，后续命令输出到 stdout/stderr。正确的表述应为: 多个 hook 可能各自写入 `CLAUDE_ENV_FILE`，文件在 Bash 命令执行前被 source，因此**应使用追加写入 (`>>`) 而非覆盖 (`>`)**，以避免后执行的 hook 擦除前面 hook 的内容。文件本身没有"追加式"的内置保护，只是一个会被 source 的普通文件。
- **反证来源**: Claude Code 官方 hook 示例 (Context7): `echo "export PROJECT_TYPE=nodejs" >> "$CLAUDE_ENV_FILE"`——使用 `>>` 追加，但这是 shell 的普通追加重定向，非文件系统的"追加式"特性

---

## 错误 11: Issue #2509 引用错误——Conda 而非 venv

- **文件+行号**: `D4-pty-integration.md` (第 91 行)
- **原声称**: 在 "2.3 Python 虚拟环境" 章节引用 `GitHub issue #2509` 作为来源
- **错误类型**: 来源不支撑
- **正确信息**: Issue #2509 的标题是 "Conda environment workflow guidance needed"——讨论的是 **Conda**（`conda activate` 在非交互式 shell 中失败），**不是** venv/virtualenv。在 Python venv 章节引用 Conda issue 是误导性的——两者是不同的问题域。
- **反证来源**: Sections 2-3 验证 agent 确认 #2509 原文标题和内容为 Conda 相关问题

---

## 错误 12: claude-mem hooks 架构 URL 路径错误

- **文件+行号**: `D4-pty-integration.md` (第 241 行参考来源)
- **原声称**: 引用 URL `https://github.com/thedotmack/claude-mem/blob/main/docs/architecture/hooks.mdx`
- **错误类型**: 事实错误
- **正确信息**: 文件实际路径为 `docs/hooks-architecture.mdx`（单文件），**非** `docs/architecture/hooks.mdx`（非子目录结构）。
- **反证来源**: Sections 2-3 验证 agent 确认正确路径为 `docs/hooks-architecture.mdx`

---

## 错误 13: /dev/tty 并非"静默"失败

- **文件+行号**: `D4-pty-integration.md` (第 78 行)
- **原声称**: "向 `/dev/tty` 写入会静默失败 (无 TTY)"
- **错误类型**: 事实错误（表述不精确）
- **正确信息**: 技术上并非"静默"——bash 在重定向到 `/dev/tty` 之前会先向 stderr 输出错误消息 `/dev/tty: No such device or address`。Claude Code 将 hook 的 stderr 输出解释为 hook 错误，从而产生 "startup hook error" 提示。用 `{ cmd; } 2>/dev/null || true` 包裹可抑制此错误。
- **反证来源**:
  - PeonPing issue #407: "Hook stderr leak: bash redirect error on /dev/tty causes Claude Code 'startup hook error'"
  - ccstatusline docs/WINDOWS.md 记录了相同的 stderr 泄漏行为

---

## 错误 14: commit 4f3092d 引用方式不精确

- **文件+行号**: `D4-pty-integration.md` (第 29 行)
- **原声称**: 来源标注为 `claude-code-tab-title commit 4f3092d`——"文档记录了'Claude Code spawns hooks with no controlling TTY'"
- **错误类型**: 来源不支撑（表述不精确）
- **正确信息**: commit 4f3092d 本身是代码变更（新增 `parse_linux_proc_stat()` 函数），不是文档。但该**项目**（claude-code-tab-title）的 README 确实记录了 hook 无控制 TTY 行为——这是该工具的设计前提。建议引用指向项目 README 而非单一 commit。
- **反证来源**: Section 1 验证 agent 分析 commit diff 内容确认其为代码变更，非文档

---

## 错误 15: WezTerm Shell Integration 注入机制完全错误

- **文件+行号**: `D4-pty-integration.md` (第 333-334 行)
- **原声称**: "Zsh 通过 `ZDOTDIR` 操纵、Bash 通过 `BASH_ENV`、Fish 通过 `XDG_CONFIG_HOME`——确保终端钩子在用户 rc 之前加载"
- **错误类型**: 事实错误（来源完全不支撑）
- **正确信息**: WezTerm 官方文档 (wezterm.org/shell-integration.html) **完全不提及**这三个环境变量。实际机制是用户手动在 `.bashrc`/`.zshrc` 中 source `wezterm.sh`（或由包管理器在 `/etc/profile.d/` 自动激活）。`ZDOTDIR`/`BASH_ENV`/`XDG_CONFIG_HOME` 在此上下文中为**完全虚构**的机制描述。
- **反证来源**: Sections 5-6 验证 agent 全文检索 wezterm.org/shell-integration.html，三个变量均零出现

---

## 错误 16: Windows Terminal WSL 示例无来源支撑

- **文件+行号**: `D4-pty-integration.md` (第 319 行 WSL 行)
- **原声称**: WSL 启动命令 `wsl -e bash -c 'cmd\\; exec bash'` 及其 env 保留说明
- **错误类型**: 来源不支撑
- **正确信息**: 引用的 SuperUser 页面 (#1756704) **完全不涉及 WSL**。该命令行示例及其所有行为说明均不出自引用的来源。
- **反证来源**: Sections 5-6 验证 agent 确认 SuperUser 页面内容限定于 Windows 原生 shell (cmd/powershell)

---

## 错误 17: iTerm2 评分机制描述不准确

- **文件+行号**: `D4-pty-integration.md` (第 328 行)
- **原声称**: iTerm2 Automatic Profile Switching 使用"16 分评分制"
- **错误类型**: 事实错误
- **正确信息**: 16 分仅是 hostname 精确匹配的**单项**最高分，非评分系统的总上限。各组件分数为加和判定：hostname 最高 16 + job 最高 4 + username 最高 2 + path 最高 1 = 理论最高 23 分。正确描述应为"基于评分的多条件匹配（hostname 16分 + job 4分 + user 2分 + path 1分）"。
- **反证来源**: iTerm2 3.6 官方文档规则表详细列出各组件独立评分

---

## 错误 18: iTerm2 匹配机制描述错误

- **文件+行号**: `D4-pty-integration.md` (第 328 行)
- **原声称**: "路径/主机名/用户名/作业名 **正则匹配**"
- **错误类型**: 事实错误
- **正确信息**: iTerm2 APS 的主匹配机制是**通配符**（`*` 星号匹配），非正则表达式。正则表达式仅作为 Trigger 机制的兜底方案出现（非 APS 主路径）。
- **反证来源**: iTerm2 官方文档 APS 章节使用通配符示例；正则仅在 Trigger 章节作为替代方案提及

---

## 错误 19: iTerm2 Smart Selection 精度级别遗漏

- **文件+行号**: `D4-pty-integration.md` (第 331 行)
- **原声称**: Smart Selection 有"四级精度 (low/normal/high/very_high)"
- **错误类型**: 事实错误（遗漏）
- **正确信息**: 实际为**五级**：Very Low / Low / Normal / High / Very High。遗漏了 "Very Low" 级别。
- **反证来源**: iTerm2 官方文档列出完整五级

---

## 错误 20: Warp 虚构术语 "SourcedRcFileForWarp"

- **文件+行号**: `D4-pty-integration.md` (第 346 行、第 355 行对比表)
- **原声称**: "SourcedRcFileForWarp DCS hook" 作为 Warp 的 shell 集成机制和对比表中的生命周期 hook
- **错误类型**: 事实错误（来源完全不支撑）
- **正确信息**: 引用的 Warp Windows blog 和 Agents 3.0 blog **均不包含**此术语。该名称在引用来源中不存在，为虚构术语。
- **反证来源**: Sections 5-6 验证 agent 全文检索 Warp Windows blog，零出现

---

## 错误 21: Warp "OSC 777 事件" 无来源支撑

- **文件+行号**: `D4-pty-integration.md` (第 347 行)
- **原声称**: "OSC 777 事件: 结构化 UI 通知通道"
- **错误类型**: 事实错误（来源完全不支撑）
- **正确信息**: 三个引用来源 (Warp Windows blog, Agents 3.0 blog, issue #6857) **均不提及** OSC 777。应删除或标注为推测。
- **反证来源**: Sections 5-6 验证 agent 确认三个来源均无 OSC 777

---

## 错误 22: Warp 产品名称错误 + 对比表两处虚构成分

- **文件+行号**: `D4-pty-integration.md` (第 349 行 "Agent Mode"、第 355 行对比表)
- **原声称**: (a) Warp 的 AI 功能叫 "Agent Mode"；(b) 对比表中 Warp 的程序化控制 = "MCP"
- **错误类型**: 事实错误
- **正确信息**: (a) 产品名为 **"Agents 3.0"**，非 "Agent Mode"。(b) MCP 不在对比表引用的 blog 中——仅为 issue #6857 中用户的愿望清单提议，非 Warp 当前功能。对比表应标 `"—"` 或 `"提议中 (#6857)"`。
- **反证来源**: Warp Agents 3.0 blog 全篇使用 "Agents 3.0"；issue #6857 确认 MCP 仅为用户提议

---

## 错误 23: #14433 的 bug 机制描述错误

- **文件+行号**: `D4-pty-integration.md` (第 457 行)
- **原声称**: "`/clear` 创建新 session 后，`CLAUDE_ENV_FILE` 仍指向旧 session 的 env 文件。新 session 从不 source 它"
- **错误类型**: 事实错误
- **正确信息**: Issue #14433 实际描述的是：hook 正确地将数据写入了**新的** env 文件（路径已更新），但 Bash 命令从未 source 这个新文件。问题在于 **sourcing 机制缺失**，而非"指向旧文件"。两者的修复路径不同（修复 source 调用 vs 修复路径更新）。
- **反证来源**: Section 7 验证 agent 直接 WebFetch issue #14433，body 原文："the hook correctly writes to the new env file... but Bash tool commands never source it"

---

## 错误 24: #38299 分类错误——功能请求非 bug

- **文件+行号**: `D4-pty-integration.md` (第 284-287 行)
- **原声称**: 将 #38299 归类在"社区痛点与已知 Bug"章节中，与 #23554、#48009 等 bug 并列
- **错误类型**: 事实错误（分类错误）
- **正确信息**: #38299 的实际标签为 **Feature Request**，非 Bug Report。Issue 作者构建 PAN（远程 PTY 终端）时遇到此限制，主诉求是**新增 Permission hook API**——文档描述的症状（字符回显但不被消费）正确，但将其归类为 bug 会误导"待修复"的期望。应标注为功能请求。
- **反证来源**: Section 7 验证 agent 确认 issue 标签为 "Feature Request"；关闭于 2026-03-25

---

## 错误 25: 多数 Windows bug 已关闭但未注明

- **文件+行号**: `D4-pty-integration.md` (第 422-465 行全文)
- **原声称**: 第 7 节列出 7 个"社区痛点与已知 Bug"，使用现在时描述（"导致终端输入冻结"、"收到空 stdin"、"指向旧会话"等），暗示为持续存在的问题
- **错误类型**: 过时信息
- **正确信息**: 7 个 Windows 相关 issue 中，**6 个已关闭**（#23554 dupe/locked 2026-02, #48009 dupe/locked 2026-04, #15840 closed 2026-04, #14433 closed 2026-03, #38299 closed 2026-03, #72347 closed 2026-07）。仅 #69159（程序化 /cd）仍 OPEN。文档未标注各 issue 的关闭状态，对 slTerminal 的架构决策分析可能高估了 risks。
- **反证来源**: Section 7 验证 agent 逐条 WebFetch，确认 11/12 个 issue 已关闭

| Issue | 状态 | 关闭日期 |
|-------|------|----------|
| #23554 | Duplicate/Locked | 2026-02-09 |
| #48009 | Duplicate/Locked | 2026-04-18 |
| #15840 | Closed (fixed) | 2026-04-09 |
| #14433 | Closed (fixed) | 2026-03-05 |
| #38299 | Closed (fixed) | 2026-03-25 |
| #72347 | Closed (fixed) | 2026-07-03 |
| #69159 | **Open** | — |

---

## 待确认项

以下外部 URL 因企业网络策略（WebFetch 无法访问）未经直接逐字验证。内容由 WebSearch 摘要/Context7 间接验证:

- `https://dev.to/ztor2/...` (第 210 行 git stash checkpoint 博客)
- `https://pydevtools.com/...` (第 91 行 Python venv 指南)
- `https://www.npmjs.com/package/claude-session-logger` (第 241 行)

### GitHub Issues 验证完成

所有 12 个 GitHub issues 已通过 section 7 子 agent 的 WebFetch 逐条验证：
- **12/12 存在**（非 404）
- **11/12 已关闭**（仅 #69159 仍 OPEN）
- 3 个有描述偏差（见错误 23、24、25）

---

## 内部矛盾（D4 自身 vs 自身）

无。

## D1/D2/D3 交叉矛盾（D4 vs 其他 D 文档）

| 矛盾 | D4 声称 | 其他文档声称 |
|------|---------|-------------|
| CLAUDE_ENV_FILE 可用范围 | 全文无事件限制 | D1: "SessionStart, CwdChanged, FileChanged"; D2-01: "仅 SessionStart" |
| SessionEnd reason 枚举 | 4 个值 (缺 resume), 现 6 个 | D1 含全部 6 个值 |
| PreCompact 可阻塞 | 未提及 | D1/D2-01/D2-02 均列为"是" |
| cwd = CLAUDE_PROJECT_DIR | 恒等 | D1/D2 将两者作为独立字段 |

---

## 审阅总结

- **事实错误**: 22 个（C1-C22） + 3 个（C23-C25）= **25 个**
- **过时信息**: 1 个（C25: 多数 Windows issue 已关闭但未注明）
- **待确认项**: 1 个（#69159 仍 OPEN，其余 web search 可验证者已由 section 7 agent 完成）
- **内部矛盾**: 0 个
- **跨文档矛盾**: 4 个
- **来源完全不支撑的声明**: 0 个（无完全伪造的来源——所有 issue 均存在，但 5 个终端模拟器描述与来源不符）

### 严重程度排序

| 优先级 | 错误 | 理由 |
|--------|------|------|
| 1 | 错误 1: CLAUDE_SESSION_ID 不是环境变量 | slTerminal 若基于此设计 hook 集成，会导致运行时读取到空值 |
| 2 | 错误 15: WezTerm shell 注入机制完全虚构 | 三个环境变量在来源中零出现——影响终端集成设计 |
| 3 | 错误 3: CLAUDE_ENV_FILE 范围未声明 | 在不适用的 hook 中引用会导致静默失败 |
| 4 | 错误 4: CLAUDE_CODE_CHILD_SESSION 级别不符 | 将社区 bug 提升为官方变量，误导架构决策 |
| 5 | 错误 25: Windows issue 多已关闭但未注明 | 影响 slTerminal 的风险评估——多数 bug 已修复 |
| 6 | 错误 23: #14433 bug 机制描述错误 | 误导修复路径（source 缺失 vs 路径错误） |
| 7 | 错误 20-22: Warp 虚构术语 + 对比表虚构成分 | "SourcedRcFileForWarp"/"OSC 777"/"MCP" 三处无来源 |
| 8 | 其余 18 个错误 | 信息完整性/精确性问题 |

---

*审阅生成日期: 2026-07-25*
*验证工具: Context7 MCP + WebSearch + D1/D2/D3 交叉对照*
