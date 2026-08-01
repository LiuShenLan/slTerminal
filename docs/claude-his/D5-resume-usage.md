# claude --resume 用法与语义 — 参数与交互行为

> 检索日期: 2026-08-01
> 来源优先级: 官方文档 > 源码仓库 > 技术博客 > 社区讨论
> 官方文档域名截至 2026-08-01 为 `code.claude.com/docs`（旧 `docs.claude.com` 已迁移）；Claude Code 当前版本线为 v2.1.x。

## 关键发现

### 发现 1: 官方 `--resume`/`-r` 三种参数形式 — 无参=交互式选择器，带 ID=指定会话，带名字=按名恢复

- 来源: https://code.claude.com/docs/en/cli-reference (2026-08-01 抓取)
- 详情: 官方 CLI 参考对 `--resume, -r` 的逐字描述为：

  > "Resume a specific session by ID or name, or show an interactive picker to choose a session. The picker and name search include sessions that added this directory with `/add-dir`; passing a session ID searches only the current project directory and its git worktrees. As of v2.1.144, [background sessions] appear in the picker marked with `bg`"

  官方示例：`claude --resume auth-refactor`；命令表示例：`claude -r "<session>" "query"` — "Resume session by ID or name"（如 `claude -r "auth-refactor" "Finish this PR"`，即带会话标识 + 一条 prompt 直接发起）。

- 来源: https://code.claude.com/docs/en/sessions (2026-08-01 抓取)
- 详情: 会话管理页面给出四个入口的对照表：

  | 命令 | 作用 |
  |------|------|
  | `claude --continue` | Resumes the most recent session in the current directory |
  | `claude --resume` | Opens the session picker（交互式选择器） |
  | `claude --resume <name>` | Resumes the named session directly（按名直恢复） |
  | `claude --from-pr <number>` | Opens the session picker filtered to sessions linked to that pull request |
  | `/resume` | Switches to a different conversation from inside an active session（会话内切换） |

  名称解析两态：`claude --resume <name>` 精确匹配则直接恢复；名字模糊（多会话命中）则**打开选择器并把名字预填为搜索词**；`/resume <name>` 模糊时**报错**，需无参运行 `/resume` 打开选择器。名称解析跨当前仓库及其全部 worktree（名字不在当前目录也能直接恢复）。

### 发现 2: `--continue`/`-c` 与 `--resume` 的语义差异 — 最近会话、无交互

- 来源: https://code.claude.com/docs/en/cli-reference (2026-08-01 抓取)
- 详情: 官方逐字描述：

  > `--continue, -c` — "Load the most recent conversation in the current directory. Includes sessions that added this directory with `/add-dir`"

  与 `--resume` 无参的根本区别：`--continue` 直接加载当前目录最近一次会话，**不打开选择器、无交互**；`--resume` 无参时总是打开交互式选择器。二者都纳入通过 `/add-dir` 添加过当前目录的会话。`--continue` 不适合"从历史中挑一个"的场景（社区 issue #46865 明确指出现有用户"`-c` only resumes the most recent session, not a filtered picker"）。

### 发现 3: fork 相关标志是 `--fork-session`（不存在 `--fork` 标志）

- 来源: https://code.claude.com/docs/en/cli-reference (2026-08-01 抓取)
- 详情: 官方 CLI 参考中分支相关标志为 **`--fork-session`**，逐字描述：

  > "When resuming, create a new session ID instead of reusing the original (use with `--resume` or `--continue`)"

  示例：`claude --resume abc123 --fork-session`、`claude --continue --fork-session`。语义：复制当前会话历史到新会话 ID，原会话保持不变。会话内对应 `/branch` 斜杠命令（`/branch [name]`，省略名字时以会话首个 prompt 命名新分支）。sessions 页面注明：fork 出的会话获得独立 session ID 并在选择器中作为独立行出现；同一会话在两个终端不 fork 地同时恢复时，两条消息流会交错进同一 transcript。

### 发现 4: 相关标志 `--session-id`（指定 UUID 新建/附着）与 `--from-pr`（按 PR 过滤选择器）

- 来源: https://code.claude.com/docs/en/cli-reference (2026-08-01 抓取)
- 详情: 官方逐字描述：

  - `--session-id` — "Use a specific session ID for the conversation (must be a valid UUID)"。示例：`claude --session-id "550e8400-e29b-41d4-a716-446655440000"`。
  - `--from-pr` — "Open the session picker filtered to sessions linked to a specific pull request."。示例：`claude --from-pr 123`。
  - `claude rm <id>` 的说明注明删除的只是会话入口，"The conversation transcript stays on your local machine, available through `claude --resume`"——即 `rm` 后仍可经 `--resume` 恢复 transcript。
  - `--name`/`-n` 用于命名会话，"You can resume a named session with `claude --resume <name>`"。

### 发现 5: 交互式选择器（session picker）行为 — 默认当前项目过滤、Ctrl+A/W/B 扩大范围、字符即搜索

- 来源: https://code.claude.com/docs/en/sessions (2026-08-01 抓取)
- 详情: 官方文档"Use the session picker"一节（`/resume` 或 `claude --resume` 无参打开）：

  - **默认显示范围**："Sessions from the current worktree, including background sessions, which are marked `bg` in the list" + "Sessions started elsewhere that added the current directory with `/add-dir`"。
  - **范围快捷键**：`Ctrl+W` 扩大到当前仓库全部 worktree（仅多 worktree 仓库显示，再按返回当前 worktree）；`Ctrl+A` 扩大到本机所有项目（再按返回当前仓库）；`Ctrl+B` 过滤到当前 git 分支的会话。
  - **搜索**：按 `/` 或任何非空格的打印字符进入搜索模式，按标题/分支/tag/PR URL 过滤；粘贴 GitHub/GitLab/Bitbucket PR 或 MR URL 可直接定位创建该 PR 的会话。**搜索不匹配项目路径**（见发现 9）。
  - **导航/操作**：`↑/↓` 导航、`→/←` 展开/折叠分组、`Enter` 恢复高亮会话、`Space` 预览内容（终端无法捕获粘贴时 `Ctrl+V` 亦可）、`Ctrl+R` 重命名高亮会话、`Esc` 退出。
  - **行内容**：会话名（设置了则显示，否则 AI 生成的标题/摘要/首 prompt）+ 距上次活动时间 + git 分支 + 文件大小；`Ctrl+A` 扩至全项目后额外显示项目路径。
  - **跨范围选择行为**：选同仓库其他 worktree 的会话 → 就地恢复；选无关项目的会话 → **复制一条 `cd` + resume 命令到剪贴板**（不直接跨项目恢复）。
  - **失败行为**：`claude --resume` 选择器加载失败时打印 `Failed to resume the conversation` 及重试命令后以退出码 1 退出；会话内 `/resume` 失败则仅报告、当前会话继续。

### 发现 6: 恢复后加载什么 — 完整对话历史 + 模型/agent/权限模式等状态，部分标志需重传

- 来源: https://code.claude.com/docs/en/sessions (2026-08-01 抓取)
- 详情: 官方"What a resumed session restores"一节：

  - **对话历史**：完整历史，含工具调用与结果。
  - **模型**：沿用会话所用模型；但模型已退役、被 `availableModels` 禁止、启动时 `--model` 标志或 `ANTHROPIC_MODEL` 系列环境变量指定、或使用 Bedrock/Vertex/Foundry 等按部署 ID 的提供商时不恢复。
  - **Agent**：`--agent` 启动的会话恢复后继续以该 agent 运行（含 system prompt、工具限制、模型），可重传 `--agent` 改选；找不到 agent 时以默认工具/提示词恢复并警告（min-version 2.1.216）。
  - **权限模式**：`plan` 与 `bypassPermissions` **永不恢复**，需启动时重新启用；`auto` 仅当账户仍满足条件时恢复；可传 `--permission-mode` 覆盖。
  - **其他**：恢复时仍活跃的 goal 延续（轮数/计时/token 基线重置）；未过期的定时任务恢复；后台 Bash 与 monitor 任务不恢复。
  - **不自动恢复的配置**：`--mcp-config`、`--settings`、`--plugin-dir`、`--fallback-model`、`--add-dir` 添加的目录需重传；`settings.json`/`settings.local.json` 启动时重新读取，无需重传。
  - **长会话恢复对话框**（Pro/Max 计划、不活跃超约 1 小时且 >100,000 tokens 时，首条消息前弹出）：① Resume from summary（立即执行 `/compact`，后续请求携带摘要而非全文）；② Resume full session as-is（全文重处理并重新缓存）；③ Don't ask me again（全文恢复并永久不再询问）。

### 发现 7: 程序化（非交互）恢复 — `claude -p --resume <session-id>` 官方支持

- 来源: https://code.claude.com/docs/en/headless (2026-08-01 抓取)
- 详情: 官方 headless 文档逐字："Use `--continue` to continue the most recent conversation, or `--resume` with a session ID to continue a specific conversation." 官方示例（多会话管理）：

  ```bash
  # First request
  claude -p "Review this codebase for performance issues"
  # Continue the most recent conversation
  claude -p "Now focus on the database queries" --continue
  # 捕获 session_id 后按 ID 恢复
  session_id=$(claude -p "Start a review" --output-format json | jq -r '.session_id')
  claude -p "Continue that review" --resume "$session_id"
  ```

  两条命令须在同一目录运行：session ID 查找限定当前项目目录及其 git worktrees。

- 来源: https://code.claude.com/docs/en/sessions (2026-08-01 抓取)
- 详情: 脚本接口示例：`claude -p --resume <session-id> --output-format json "summarize what we changed" | jq -r '.result'`。**关键边界**：`claude -p` 或 Agent SDK 创建的会话**不出现在交互选择器中**，但可以按 session ID 经 `claude --resume <session-id>` 恢复；在非会话起始目录运行则报 `No conversation found with session ID: <session-id>`。`--output-format json` 的返回体含 `session_id` 字段（可捕获供后续恢复）。

### 发现 8: session ID 查找范围与 transcript 存储位置

- 来源: https://code.claude.com/docs/en/sessions (2026-08-01 抓取)
- 详情: 官方逐字：session ID 查找 "is scoped to the current project directory and its git worktrees, so a session created elsewhere reports `No conversation found with session ID: <session-id>`"。transcript 存储于 `~/.claude/projects/<project>/<session-id>.jsonl`（`<project>` 为工作目录绝对路径、非字母数字字符替换为 `-`），JSONL 行格式为内部实现、版本间可变（官方警告脚本直接解析可能随时失效）。保留期默认 30 天（`cleanupPeriodDays` 可调）；`CLAUDE_CONFIG_DIR` 可整体迁移存储位置。

- 来源: https://github.com/anshul-garg27/claude-picker (2026-08-01 抓取)
- 详情: 第三方工具佐证同一存储事实："Claude Code stores sessions in `~/.claude/projects/` as JSONL files"，项目目录经 lossy 编码（`/Users/you/my_project` → `-Users-you-my-project`）；另存 `~/.claude/sessions/` 会话元数据，含 `forkedFrom` 字段可检测 fork/链。

- 来源: https://code.claude.com/docs/en/agent-sdk/sessions (2026-08-01 抓取)
- 详情: SDK 文档同名提示：resume 返回全新会话的最常见原因是 `cwd` 不匹配——会话按 `~/.claude/projects/<encoded-cwd>/*.jsonl` 存储，从不同目录 resume 时 SDK 在错误位置查找。

### 发现 9: 选择器"按项目过滤"是社区争议点 — 默认行为在版本间反复，搜索不支持项目路径

- 来源: https://github.com/anthropics/claude-code/issues/46865 (2026-04-12 打开，**已关闭**)
- 详情: "Add setting to filter /resume picker by current project directory"（Claude Code v2.1.104，2026-04 时点）。报告称 **v2.1.101 起选择器默认显示所有项目的会话**（官方 changelog 视为修复"narrow default view hiding sessions from other projects"），对多项目用户造成列表杂乱；请求 `settings.json` 增加 `resume.filterByCurrentProject: true` 恢复按目录过滤默认值。无维护者评论，已关闭。

- 来源: https://github.com/anthropics/claude-code/issues/47581 (2026-04-13 打开，**closed as not planned**)
- 详情: "[FEATURE] Session picker search should also filter by project path"（v2.1.104）。逐字确认当前搜索行为："the session picker's search bar filters sessions by title, git branch, tag, and PR number — but **not** by the project path (working directory)"。请求把 `projectPath` 加入搜索谓词，未采纳。

- 来源: https://github.com/anthropics/claude-code/issues/59941 (2026-05-17 打开，**已关闭**)
- 详情: "/resume should search sessions globally across all project directories"。报告者称 `/resume` 只显示当前工作目录的会话（与官方当前文档的默认范围一致），请求全局搜索；其 workaround 是自定义 `/ses` 斜杠命令 + shell wrapper 直接读 `~/.claude/projects/*/*.jsonl` 用 fzf 导航（依赖未公开的内部文件格式）。

- 来源: https://code.claude.com/docs/en/sessions (2026-08-01 抓取)
- 详情: 截至 2026-08-01，官方文档（含 min-version 2.1.211/2.1.216 更新）描述的默认范围是**当前 worktree + `/add-dir` 会话**，即默认按当前项目过滤、`Ctrl+A` 才扩大。与 issue #46865 所述 v2.1.101 起的"默认全项目"不一致——两处均为各版本时点的真实观察，**默认范围在 v2.1.x 期间发生过变化**，实现终端功能时不应假设默认范围，应依赖 `Ctrl+A`/`Ctrl+W` 这类显式切换或直接解析 `~/.claude/projects/` 目录。

### 发现 10: Agent SDK 侧的 continue/resume/fork 语义与 CLI 对齐（供程序化集成参考）

- 来源: https://code.claude.com/docs/en/agent-sdk/sessions (2026-08-01 抓取)
- 详情: SDK 官方逐字：

  > "**Continue** finds the most recent session in the current directory. You don't track anything. ... **Resume** takes a specific session ID. You track the ID. Required when you have multiple sessions ... or want to return to one that isn't the most recent. **Fork** is different: it creates a new session that starts with a copy of the original's history. The original stays unchanged."

  session ID 从结果消息的 `session_id` 字段捕获（每次结果都有，无论成败）。Python/TypeScript 提供 `list_sessions()` / `listSessions()` 与 `get_session_messages()` / `getSessionMessages()`，官方明言"Use them to build custom session pickers"——即**官方认可基于 SDK 自建会话选择器**；另有 `get_session_info()`/`rename_session()`/`tag_session()` 等会话管理函数。

### 发现 11: `/resume` 曾有文档缺口，截至 2026-08-01 已补齐

- 来源: https://github.com/anthropics/claude-code/issues/8584 (2025-10-01 打开，**autoclosed**)
- 详情: "[DOCS] Missing Documentation for Various Claude Code Features"。报告时 `/resume` 斜杠命令（"Interactively resume a past conversation (distinct from the CLI flag)"）不在官方 cli-reference/slash-commands 页面，与 `--debug`、`--fallback-model` 等标志同列缺失项。issue 被 bot 自动关闭（autoclose 标签），无维护者回复。截至 2026-08-01 抓取时，官方 `cli-reference` 与 `sessions` 页面已含 `--resume`/`-r`、`--continue`/`-c`、`--fork-session`、`--session-id`、`--from-pr`、`--name` 的完整描述及选择器行为，文档缺口已实际补齐。

## 来源清单

| 来源 URL | 类型 | 关键内容 |
|-----------|------|---------|
| https://code.claude.com/docs/en/cli-reference | 官方文档 | `--resume`/`-r`、`--continue`/`-c`、`--fork-session`、`--session-id`、`--from-pr` 逐字描述与示例 |
| https://code.claude.com/docs/en/sessions | 官方文档 | 四入口对照表、选择器默认范围/快捷键/搜索、恢复内容清单、长会话恢复对话框、transcript 存储路径与 30 天保留 |
| https://code.claude.com/docs/en/headless | 官方文档 | `claude -p --resume <session-id>` / `-p --continue` 程序化恢复示例、`--output-format json` 捕获 `session_id` |
| https://code.claude.com/docs/en/agent-sdk/sessions | 官方文档 | SDK continue/resume/fork 语义、session_id 捕获、`list_sessions`/`get_session_messages` 自建 picker、cwd 不匹配陷阱 |
| https://github.com/anthropics/claude-code/issues/46865 | 社区 issue（closed） | v2.1.101 起选择器默认全项目、请求 `resume.filterByCurrentProject` 设置 |
| https://github.com/anthropics/claude-code/issues/47581 | 社区 issue（closed, not planned） | 确认选择器搜索仅匹配 title/branch/tag/PR number、不含项目路径 |
| https://github.com/anthropics/claude-code/issues/59941 | 社区 issue（closed） | /resume 仅当前目录会话的全局搜索请求、fzf workaround |
| https://github.com/anthropics/claude-code/issues/8584 | 社区 issue（autoclosed） | `/resume` 曾缺官方文档的原始报告（2025-10），现文档已补齐 |
| https://github.com/anshul-garg27/claude-picker | 第三方工具（源码仓库） | `~/.claude/projects/` JSONL 存储佐证、lossy 编码、`forkedFrom` fork 检测、两栏 picker |
| https://skillsplayground.com/guides/claude-code-cheat-sheet/ | 技术博客（仅搜索摘要） | `claude -r "id"` "Resume a specific conversation by ID" 第三方佐证 |
| https://skywork.ai/blog/slide-template/everything-about-claude-code-cli-flags-quick-start-guide/ | 技术博客（仅搜索摘要） | `-c`/`--continue` 恢复当前目录最近会话的第三方表述 |

## 对 slTerminal 实现"查询历史 session 并恢复"功能的要点提炼

1. **参数三态已官方确认**：`claude --resume`（无参=交互选择器）、`claude --resume <session-id>`（指定会话，UUID）、`claude --resume <name>`（按名恢复，模糊时回退选择器并预填搜索词）。短形式 `-r` 等价。
2. **恢复语义**：`--resume` 无参总是交互式（选择器），`--continue` 无交互直取最近会话；非交互程序化恢复用 `claude -p --resume <session-id>` 官方支持。
3. **选择器默认按当前项目过滤**（当前 worktree + `/add-dir` 会话），但该默认曾在 v2.1.101 被改为全项目、社区持续争议——不应假设默认范围稳定。替代实现可直接扫描 `~/.claude/projects/<encoded-cwd>/` 目录（JSONL 格式为内部实现，版本间可变，官方警告）。
4. **session ID 查找有目录范围限制**：跨目录恢复报 "No conversation found"；恢复会话必须从原会话起始目录（或其 git worktree）运行——终端 UI 若要跨目录恢复，需先 `cd` 到对应目录（官方选择器跨项目时正是复制 `cd`+resume 命令到剪贴板）。
5. **分支标志名为 `--fork-session`**（配合 `--resume`/`--continue` 使用），不是 `--fork`；会话内对应 `/branch`。
