# D5b：Claude Code 社区 Hooks 实际使用案例

> 研究日期：2026-07-25
> 覆盖渠道：GitHub、Reddit (r/ClaudeAI)、Hacker News、Anthropic 官方文档/博客、技术博客 (dev.to/腾讯云/CSDN 等)

---

## 1. GitHub 上的实际项目

### 1.1 disler/claude-code-hooks-mastery -- 全事件演示仓库

**来源**：https://github.com/disler/claude-code-hooks-mastery
**日期**：2026

覆盖 13 个 hook 事件（社区计数，官方文档核心事件约 8 种，D5a 子报告列出 12+；差异源于部分事件如 `Setup`、`PermissionRequest` 在官方文档中可能被分类为 hook 类型而非事件）：`UserPromptSubmit`、`PreToolUse`、`PostToolUse`、`PostToolUseFailure`、`Notification`、`Stop`、`SubagentStart`、`SubagentStop`、`PreCompact`、`SessionStart`、`SessionEnd`、`PermissionRequest`、`Setup`。使用 Python + UV 单文件脚本，包含 TTS 通知、代码质量校验（Ruff、类型检查）、状态行、自定义 slash 命令和子代理。

设计模式：**UV 单文件脚本**——每个 hook 一个 Python 文件，零 venv 依赖，`uv run script.py` 直接执行。

### 1.2 karanb192/claude-code-hooks -- Hook 市场 + 可安装插件

**来源**：https://github.com/karanb192/claude-code-hooks
**日期**：2026

即用型 hooks 集合 + **插件市场**。覆盖安全（阻止危险命令 `rm -rf`、`curl | sh`、fork bomb）、secrets 保护（`.env` / 凭证）、git 安全（阻止 `push --force`、`reset --hard`）、自动 staging、代码格式化。可通过 `/plugin install` 安装。

设计模式：**插件化分发**——hook 配置打包为安装式插件，用户零配置启用。

### 1.3 joaoariedi/ai-assisted-development-framework -- SDD 规约驱动开发框架

**来源**：https://github.com/joaoariedi/ai-assisted-development-framework
**日期**：2026 (v4.3)

企业级 7 hook 体系，用于规约驱动开发（SDD）：

| Hook 事件 | 脚本 | 用途 | 超时 |
|-----------|------|------|------|
| PreToolUse (Bash) | `quality-before-commit.sh` | gitleaks secrets 检测 + lint | 120s |
| PreToolUse (Edit\|Write) | `block-sensitive-files.sh` | 拦截 .env / *.key / *.pem / .git/ | - |
| PostToolUse (Edit\|Write) | `format-after-edit.sh` | ruff/biome/gofmt/rustfmt 自动格式化 | 15s |
| PostToolUse (Edit\|Write) | `run-tests-after-edit.sh` | 测试套件（节流 15s，非阻塞） | 30s |
| Notification | `notify-on-block.sh` | 桌面通知（Linux/macOS） | - |
| Stop | `stop-quality-check.sh` | 最终质量验证 | 10s |

额外集成 5 个自定义 agent（test-specialist、quality-guardian、code-reviewer、review-coordinator、forensic-specialist）+ rtk CLI 输出压缩（60-90% token 节省）。

设计模式：**规约流水线**——hooks 只做确定性执法（格式化/拦截/通知），不确定性的设计审查交给子 agent。

### 1.4 lbartoszcze/autohook-coding-agent-superpowers -- 自我进化的 Hook 系统

**来源**：https://github.com/lbartoszcze/autohook-coding-agent-superpowers
**日期**：2026

核心理念：**当你对 Claude 的行为感到沮丧时，系统自动建议并创建新 hook**，确保同样的问题永不再次发生。覆盖的痛点：
- 中途停下来反复确认
- 创建大量重复现有功能的无用文件
- 编写数千行长的文件
- 不读文档直接幻觉式报错
- 不实际测试就说"可能是由于..."

一键安装：`git clone && ./install.sh`

设计模式：**自愈式 harness**——hook 不是静态配置，而是随着使用体验不断增长的规则库。

### 1.5 lasso-security/claude-hooks -- Prompt Injection 防御

**来源**：https://github.com/lasso-security/claude-hooks
**日期**：2026

安全专用 hook：扫描工具输出中的 prompt injection 尝试，检测到则拦截。PreToolUse + PostToolUse 双层防御。

设计模式：**安全防火墙**——hooks 作为不可绕过的安全层，拦截 prompt injection 攻击。

### 1.6 Payshak/claude-hook-kit -- TypeScript SDK

**来源**：https://github.com/Payshak/claude-hook-kit
**日期**：2026

`npm install claude-hook-kit` -- 为 TypeScript 开发者提供类型安全、可测试、可组合的 hook 开发工具。包含完整的测试工具和组合模式。

设计模式：**SDK/库模式**——将 hooks 从 shell 脚本提升为类型安全的 TS 工程。

### 1.7 gabriel-dehan/claude_hooks -- Ruby DSL

**来源**：https://github.com/gabriel-dehan/claude_hooks
**日期**：2026

`gem install claude_hooks` -- Ruby DSL 用于创建 hooks。entrypoint/handler 架构、配置合并、工具监控示例。适合 Ruby 技术栈团队。

设计模式：**DSL 模式**——领域特定语言降低 hook 编写门槛。

### 1.8 tonghuikang/claude-code-template -- 个人模板仓库

**来源**：https://github.com/tonghuikang/claude-code-template
**日期**：2026

跨 agent 支持（Claude Code & Codex），包含 TTS（Kokoro）、技能目录、hook 模板。在相关生命周期节点注入指令。

设计模式：**模板化**——homesick/dotfiles 风格的 Claude Code 配置模板。

### 1.9 其他值得关注的仓库

| 仓库 | 焦点 |
|------|------|
| [ronaldeddings/Basic-Claude-Code-Hook-For-Context](https://github.com/ronaldeddings/Basic-Claude-Code-Hook-For-Context) | TypeScript hook 阻止 `DROP DATABASE/TABLE`、`DELETE FROM`（PostgreSQL/MySQL/MongoDB/Redis） |
| [adhenawer/claude-setups](https://github.com/adhenawer/claude-setups) | 社区 hook 配置分享仓库——发现和共享 hook 及指令 |
| [Imran-ml/claude-skills](https://github.com/Imran-ml/claude-skills) | 完整配置仓库：hooks + skills + subagents + MCP + keybindings |
| [dwmkerr/claude-toolkit](https://github.com/dwmkerr/claude-toolkit) | Hook 开发指南 + 质量检查示例（Ruff/ESLint/Prettier/gitleaks） |
| [shakacode/claude-code-commands-skills-agents](https://github.com/shakacode/claude-code-commands-skills-agents) | Hook 指南文档 + 完整 hook 事件参考 |
| [MuhammadUsmanGM/claude-code-best-practices](https://github.com/MuhammadUsmanGM/claude-code-best-practices) | Hook 最佳实践 + 工具配置 README |
| [FlorianBruniaux/claude-code-ultimate-guide](https://github.com/FlorianBruniaux/claude-code-ultimate-guide) | 终极指南 + `auto-format.sh` bash hook 示例 |
| [luongnv89/claude-howto](https://github.com/luongnv89/claude-howto) | 06-hooks/README.md 中文 hook 教程 |

---

## 2. Reddit (r/ClaudeAI) 讨论

### 2.1 社区共识：从 prompt 规则迁移到 hooks 执法层

**来源**：
- Reddit 帖子 `1t9ak8o` (2026-05-10, ~184 赞) -- vibe-coder 分享：prompt 层指令不可靠，只有 hooks 能真正执法
- Reddit 帖子 `1tb047p` (2026-05-13, ~17 赞) -- "Writ" 插件：30 个 bash hooks + Neo4j 知识图谱

四个独立开发者在 96 小时内收敛到同一结论：
> CLAUDE.md 规则没有执法机制——模型可以偏离它们（社区普遍观测 CLAUDE.md 规则不遵守率在 22-40% 范围；最接近的定量来源：Tygart Media 报告约 30% 偏离、GitHub Issue #32163 提及 30-40% 偏离）。Shell hooks（`PreToolUse`、`PostToolUse`）在进程级可靠执法，因为模型无法选择不触发它们。

### 2.2 推荐的最小可行 Hook 链

社区生产环境中推荐的"最小可行"配置：
1. **PreToolUse hook** -- 拦截 `rm -rf`、`git reset --hard` 等破坏性命令
2. **PostToolUse hook** -- 只追加审计日志，记录所有工具调用
3. **SessionEnd hook** -- compact 前快照 session 状态到磁盘
4. **.env 路径守卫** -- 跨 subagent 边界阻止读取 .env 文件

### 2.3 三层自动化策略

Reddit power users 推荐组合：
- **Hooks**：确定性自动化（格式化、linting、拦截）——100% 执行
- **MCP Servers**：外部连接（GitHub、Slack、Reddit、数据库）——结构化数据
- **`-p` flag**：非交互/CI-CD 执行——可脚本化

---

## 3. Hacker News 讨论

### 3.1 Recall -- 持久化记忆系统 (4 个生命周期 Hooks)

**来源**：https://news.ycombinator.com/item?id=47189906
**日期**：2026

通过 4 个 hook 实现跨 session 持久化记忆：
- **SessionStart**：获取记忆
- **观察 hook**：捕获 git commits/文件变更事件
- **PreCompact**：context compaction 前保存状态
- **SessionEnd**：记录 session 摘要

存储：Redis + embedding 语义搜索。MIT 开源。安装：`/plugin install recall@claude-plugins-official`

设计模式：**记忆管道**——hooks 串联实现持久化上下文，解决 session 间失忆。

### 3.2 Pickle Rick -- Stop Hook 实现持续 Agent 循环

**来源**：https://news.ycombinator.com/item?id=47091363
**日期**：2026

通过 **Stop hook** 拦截 session 退出，注入新 prompt 驱动 Claude 持续工作：
```
PRD → ticket 分解 → research/plan/implement/refactor
```
结构化 session 摘要（phase、iteration count、ticket checklist）让 Claude 在 context 压缩后仍保有上下文。

设计模式：**无限循环 harness**——Stop hook exit 2 阻止 Agent 停止，实现全自动开发流水线。

### 3.3 Draft -- Session-Init Hook 注入持久化产品上下文

**来源**：https://news.ycombinator.com/item?id=48080538
**日期**：2026

**SessionStart hook** 在每次 session 开始时注入结构化上下文摘要（~5K tokens），包含公司、产品、优先级、团队、决策。学习是推理驱动的——agent 自行决定何时通过 `/draft:learn` 持久化信息。

设计模式：**上下文注入**——session 启动时一次性注入全部项目背景。

### 3.4 MCR (Model Context Retrieval) -- 双 Hook 自动加载笔记

**来源**：Hacker News 讨论（关联帖子 47670002）
**日期**：2026

- **Hook 1**：匹配用户 prompt 与 Obsidian vault 索引，注入相关笔记
- **Hook 2**：拦截工具调用（Read、Grep、WebSearch）作为"知识缺口"信号，注入相关笔记到工具结果旁

纯 Python stdlib，~20ms/hook，无向量数据库。

设计模式：**知识注入**——hooks 作为实时 RAG 层，在 LLM 看到内容前注入知识。

### 3.5 TDD Guard -- Hooks 强制 TDD 纪律

**来源**：Hacker News 讨论
**日期**：2026

通过 hooks **阻止编辑直到测试通过**，强制执行真正的 TDD 工作流（先测试，再实现）。跨 Claude Code、Codex、GitHub Copilot 通用。

设计模式：**质量门禁**——hooks 作为不可绕过的流程执法器。

### 3.6 Han -- 129 插件 Stop Hook 验证市场

**来源**：https://news.ycombinator.com/item?id=46150605
**日期**：2026

在 **Stop 事件**运行验证 hooks——类型检查、测试、linting 任一失败则无法继续。129 个插件覆盖 19+ 语言、30+ 领域。退出码 2 强制 Agent 修复后再停止。

设计模式：**验证市场**——社区贡献的语言/框架验证规则。

### 3.7 Claude Remote Approver -- PermissionRequest Hook

**来源**：https://news.ycombinator.com/item?id=47111171
**日期**：2026

通过 **PermissionRequest hook** 将每个权限请求以 push 通知发送到手机（ntfy.sh），在手机上审批/拒绝。120 秒无应答自动拒绝。

设计模式：**远程审批**——hooks 桥接移动端与桌面端。

### 3.8 Claude-Nonstop -- 自动账户切换 + Slack 通知

**来源**：https://news.ycombinator.com/item?id=47082232
**日期**：2026

Hooks 用于：发送 session 进度到 Slack + 检测 rate limit 后自动迁移 session 到另一账户。

设计模式：**运维自动化**——hooks 处理基础设施层问题（rate limit/通知/账户切换）。

### 3.9 Claude Code Kit -- 自动激活 Skills

**来源**：https://news.ycombinator.com/item?id=45789960
**日期**：2026

安装 hooks 后在检测到关键词或文件编辑时自动激活对应框架 skill（Next.js、React、Tailwind、Prisma 等）。

设计模式：**上下文感知激活**——hooks 根据代码特征自动加载对应 skill。

---

## 4. Anthropic 官方

### 4.1 官方 Hooks 文档

**来源**：https://code.claude.com/docs/en/hooks

支持 8 种核心事件（官方文档列出的核心集，不含 Setup、PostToolUseFailure、Notification、PermissionDenied、FileChanged 等；D5a 子报告列出 12+ 含以上扩展事件）：`PreToolUse`、`PostToolUse`、`PermissionRequest`、`PreCompact`、`SessionStart`、`Stop`、`SubagentStop`、`UserPromptSubmit`。

5 种 hook 类型：
| 类型 | 说明 |
|------|------|
| `command` | Shell 脚本/二进制（确定性执行） |
| `http` | HTTP POST 外部服务（2026.02 引入） |
| `prompt` | LLM 判断（用 Claude 做决策） |
| `agent` | 子 agent 做复杂推理 |
| `mcp_tool` | MCP 工具调用 |

配置文件优先级：Enterprise > Project > User；Local 覆盖 project。

### 4.2 官方博客：How to Configure Hooks

**来源**：https://claude.com/blog/how-to-configure-hooks
**日期**：2026

官方推荐的配置方法：
- 交互式 CLI：`/hooks` 引导式配置
- 手动 JSON：编辑 settings 文件
- AI 辅助：让 Claude 从 CLAUDE.md 自动生成 hooks

强调的 4 个层次：
| 文件 | 范围 | 版本控制 |
|------|------|----------|
| `.claude/settings.local.json` | 本地项目（个人） | 不提交 |
| `.claude/settings.json` | 项目（团队共享） | 提交 |
| `~/.claude/settings.json` | 全局（所有项目） | 不跟踪 |
| 插件 `hooks/hooks.json` | 插件用户 | 随插件分发 |

### 4.3 官方博客：Steering Claude Code

**来源**：https://claude.com/blog/steering-claude-code-skills-hooks-rules-subagents-and-more
**日期**：2026

官方决策树——何时用 Hook 而非其他机制：

| 场景 | 推荐方案 | 原因 |
|------|---------|------|
| "每次 X 总是做 Y" | **Hook** settings.json | 确定性执行，不依赖模型 |
| "永远不做这个" | **PreToolUse hook** exit 2 | 真正不可绕过的守卫 |
| 30 行 CLAUDE.md 程序 | **Skills** `.claude/skills/` | 减少上下文膨胀 |
| API 特定规则不需路径范围 | **Rules** + `paths:` scoping | 精确范围 |
| 个人偏好在项目级 | **User/local** settings | 不污染团队配置 |

### 4.4 官方 10 大 Hook 模式

**来源**：https://github.com/anthropics/claude-plugins-official/blob/main/plugins/plugin-dev/skills/hook-development/references/patterns.md
**日期**：2026

| # | 模式 | 用途 | 事件 |
|---|------|------|------|
| 1 | Security Validation | 拦截敏感路径写入 | PreToolUse |
| 2 | Test Enforcement | 修改代码但未跑测试时阻止停止 | Stop |
| 3 | Context Loading | SessionStart 检测项目类型并配置环境 | SessionStart |
| 4 | Notification Logging | 审计日志记录所有通知 | Notification |
| 5 | MCP Tool Monitoring | 用 regex matcher 拦截破坏性 MCP 操作 | PreToolUse |
| 6 | Build Verification | 要求构建成功才允许 Stop | Stop |
| 7 | Permission Confirmation | 危险 Bash 命令要求用户确认 | PreToolUse |
| 8 | Code Quality Checks | Write/Edit 后自动运行 linter/formatter | PostToolUse |
| 9 | Temporarily Active Hooks | 用 flag 文件控制 hook 开关，无需改配置 | PreToolUse |
| 10 | Configuration-Driven Hooks | JSON 配置文件驱动可配置行为 | SessionStart |

### 4.5 官方 Exit Code 协议（关键）

| 退出码 | 含义 | 行为 |
|--------|------|------|
| **0** | 成功/允许 | stdout 作为反馈或上下文注入 |
| **2** | **阻塞错误** | stderr 作为错误消息返回 Claude，操作被阻止 |
| **其他非零** | 非阻塞警告 | stderr 仅显示给用户，执行继续 |

> **社区共识**：`exit 1` 不阻塞是 #1 踩坑点——Unix 惯例认为 exit 1 是失败，但 Claude Code 将其视为非阻塞警告。

### 4.6 官方环境变量

Hook 脚本可使用的环境变量：
- `CLAUDE_PROJECT_DIR` -- 项目根路径
- `CLAUDE_CODE_REMOTE` -- Web 环境下为 true
- `CLAUDE_ENV_FILE` -- SessionStart 持久化变量
- `CLAUDE_TOOL_INPUT_FILE_PATH` -- PostToolUse 中可用
- `CLAUDE_PLUGIN_ROOT` -- 插件根目录

### 4.7 官方安全最佳实践

1. 校验和消毒所有 stdin 输入
2. Shell 变量必须引号包裹：`"$VAR"` 防注入
3. 使用绝对路径
4. 避免处理 .env、凭证等敏感文件
5. 永不记录 secrets——输出消毒：API keys、tokens、passwords
6. 最小权限原则
7. PreToolUse hooks 作为防火墙——危险命令黑名单
8. 审查 MCP servers——仅使用可信来源
9. exit 2 阻塞时确保错误信息输出到 **stderr**
10. 企业用 managed settings——管理员部署，不可覆盖

---

## 5. 技术博客

### 5.1 "Claude Code used 2.5M tokens on my project. I got it down to 425K with 6 hook scripts"

**来源**：https://dev.to/cytostack/claude-code-used-25m-tokens-on-my-project-i-got-it-down-to-425k-with-6-hook-scripts-d40
**日期**：2026

6 个 hook 脚本（工具名 **OpenWolf**，npm 包 `openwolf`）实现了显著 token 节省。**注意**：标题声称 "2.5M → 425K"（约 83%）仅为**单个大项目的最佳案例**。作者报告在 20 个项目中**平均减少约 65.8%**，非 83%。Token 跟踪为估算制（~15% 精度范围）。`cerebrum.md` 合规率约 85-90%：

| Hook | 脚本 | 用途 |
|------|------|------|
| PostToolUse (Read) | `post-read.js` | 记录每个文件的 token 估算 |
| PostToolUse (Write) | `post-write.js` | 自动更新项目 anatomy 索引 + 追加学习记忆 |
| PreToolUse (Read) | `pre-read.js` | 注入 anatomy.md 索引——Claude 判断描述是否足够，避免打开完整文件 |

核心创新：自动生成 `anatomy.md`（每个文件一行描述 + token 估算），Read hook 注入此索引，让 Claude 决定是否需要读完整文件。

### 5.2 "The Claude Code hooks system changed how I work"

**来源**：https://dev.to/idapixl/the-claude-code-hooks-system-changed-how-i-work-heres-what-i-built-173i
**日期**：2026

多 agent 持久化记忆系统的完整实现：
- **SessionStart hook**：调用 `vault-pulse.sh --fast` 重建上下文状态并注入
- **PreToolUse hook**：safety guardrail——拦截向 vault 外的写入 + `rm -rf` 匹配
- **PostToolUse hook**：mid-session changelog 记录所有变更

设计要点：critical cron sessions 中无人在场——SessionStart 必须自动注入完整上下文。

### 5.3 "Claude Code Hooks explained: config structure, matchers, and a copy-paste PreToolUse guard"

**来源**：https://dev.to/rulestack/claude-code-hooks-explained-config-structure-matchers-and-a-copy-paste-pretooluse-guard-58jj
**日期**：2026

面向初学者的完整教程，涵盖：
- 配置结构（三层嵌套：事件 → matcher → hooks 数组）
- Matcher 语法详解（`*`、`Edit|Write`、`Bash(npm test*)`、`mcp__.*`）
- 可直接复制的 PreToolUse guard 脚本（阻止 `rm -rf /`、`chmod 777`、`>/etc/`）

### 5.4 "Claude Code Hooks: The Feature You're Ignoring While Babysitting Your AI"

**来源**：https://lakshminp.com/2026/01/claude-code-hooks/
**日期**：2026-01

观点文章：对比 hooks（100% 执行）vs skills（约 50-80% 触发率，概率性）的可靠性差异。强调 PostToolUse 自动格式化是最受欢迎且零风险的入门 hook。Skills 的触发是概率性的（模型决定是否加载），hooks 是确定性的（100% 执行）。

### 5.5 中文社区资源

| 来源 | 内容 |
|------|------|
| [腾讯云：Claude Code Hooks 2026 完整实战指南](https://cloud.tencent.com.cn/developer/article/2689241) | 6 个生产级 hook 场景，附完整脚本和配置 |
| [腾讯云：Claude Code Hooks 零上下文成本的自动化](https://cloud.tencent.com.cn/developer/article/2689501) | hooks 不消耗 context token 的特性详解 |
| [CSDN：Claude Code 工程化实战 第 17 讲](https://blog.csdn.net/qq_36858702/article/details/162794259) | hooks 事件驱动自动化 |
| [w3cschool：Claude Code 钩子指南](https://www.w3cschool.cn/aicodingguide/claude-code-hooks.html) | 中文 hook 配置入门 |
| [GUVI：Claude Code Hooks 配置指南](https://www.guvi.in/blog/claude-code-hooks-how-to-configure-them/) | 面向初学者的配置教程 |
| [Skywork：Claude Code CLI Hooks 最佳实践](https://skywork.ai/blog/slide-template/expert-guide-claude-code-cli-hooks-best-practices-6/) | 最佳实践精简指南 |

---

## 6. 通知类 Hooks 专项

社区中通知类 hook 是第二大热门应用场景（仅次于安全拦截），形成了完整的工具生态：

| 工具 | 安装方式 | 支持渠道 |
|------|---------|---------|
| [ai-agent-notifier](https://github.com/DevinoSolutions/ai-agent-notifier) | `npx ai-agent-notifier setup` | Desktop + ntfy + Slack/Discord/Telegram |
| [claude-notify](https://github.com/ddaikodaiko/claude-notify) | `npm install -g @daik0z/claude-notify` | Desktop + ntfy + Slack/Discord webhooks |
| [agent-notify](https://github.com/hellolib/agent-notify) | `npx agent-notify` | Desktop + ntfy + Slack/Discord/Telegram/飞书/钉钉/企微/Bark |
| [claude-notifier](https://github.com/felipeelias/claude-notifier) | 静态二进制 | ntfy + terminal-notifier |
| [claude-notifications-go](https://github.com/jaeinkim/claude-notifications-go) | 6 种通知类型 | Slack/Discord/Telegram/ntfy/Teams/PagerDuty |
| [ccnotify](https://www.npmjs.com/package/ccnotify) | `npx ccnotify` | Discord/ntfy/macOS |

典型配置模式（手动版）：
```json
{
  "hooks": {
    "Notification": [{
      "hooks": [{
        "type": "command",
        "command": "curl -d 'Claude needs input' ntfy.sh/your-topic"
      }]
    }],
    "Stop": [{
      "hooks": [{
        "type": "command",
        "command": "curl -d 'Task complete' ntfy.sh/your-topic"
      }]
    }]
  }
}
```

---

## 7. 社区用例汇总表

| 用例类型 | 社区出现频次 | 典型配置 | 代表性来源 |
|---------|------------|---------|-----------|
| **危险命令拦截** (rm -rf / git push --force / DROP TABLE) | 极高（几乎所有仓库） | PreToolUse + exit 2 | karanb192, joaoariedi, ronaldeddings |
| **自动格式化** (Prettier/Ruff/gofmt) | 极高 | PostToolUse + Edit\|Write | disler, joaoariedi, Anthropic 官方 |
| **桌面/手机通知** (ntfy/Slack/Discord) | 高（6+ 独立工具） | Notification + Stop | DevinoSolutions, ddaikodaiko, hellolib |
| **SessionStart 上下文注入** (git status/项目结构/TODO) | 高 | SessionStart + stdout | Anthropic 官方, disler, idapixl |
| **敏感文件保护** (.env/*.key/*.pem/.git/) | 高 | PreToolUse + exit 2 | joaoariedi, karanb192, Anthropic 官方 |
| **Stop Hook 质量门禁** (lint/test 不过则不能停止) | 中高 | Stop + exit 2 | Han 市场, Pickle Rick, joaoariedi |
| **持久化记忆** (跨 session 上下文) | 中（3 个独立项目） | SessionStart+SessionEnd+PreCompact | Recall, Draft, MCR |
| **Token 优化** (anatomy 索引 + 选择性读取) | 中 | PreToolUse(Read) + PostToolUse(Write/Read) | cytostack (dev.to) |
| **Prompt Injection 防御** | 中 | PreToolUse + PostToolUse | lasso-security |
| **TDD 强制执行** (测试不过不能编辑) | 低中 | PreToolUse + exit 2 | TDD Guard |
| **自我进化 Hook** (自动建议新 hook) | 低 | SessionStart + 反馈环 | lbartoszcze |
| **远程审批** (手机审批权限) | 低 | PermissionRequest + ntfy | Claude Remote Approver |
| **自动账户切换** (rate limit 迁移) | 低 | Notification + 脚本 | Claude-Nonstop |
| **Hook 开发框架** (SDK/DSL) | 中（TypeScript/Ruby 各一） | - | Payshak, gabriel-dehan |

---

## 8. 关键教训与反模式

### 8.1 最高频踩坑点

1. **`exit 1` 不阻塞**——必须用 `exit 2` 才能真正阻止操作。这是社区最高频的踩坑。
2. **PostToolUse 无法撤销**——已经执行的操作无法通过 exit 2 撤销，只能给出 stderr 反馈。
3. **Hook 不能放松权限**——返回 `"allow"` 不覆盖 settings 中的 deny 规则。
4. **Hook 变更需重启**——修改 hooks 配置后必须重启 Claude Code 或执行 `/hooks` 才能生效。
5. **并发执行**——同一事件的多个 hook 并行运行；相同命令自动去重。

### 8.2 社区推荐起点

三步入门：
1. 部署一个触发你实际痛点的 hook
2. 测试后逐步扩展
3. **不要第一周就加 12 个 hook**（循环依赖真实存在）

推荐的"第一天"配置：
- **PreToolUse + Bash**：拦截 `rm -rf`、`--force`、`DROP TABLE`
- **PreToolUse + Edit\|Write**：保护 `.env`、lock files、credentials
- **PostToolUse + Edit\|Write**（非阻塞）：自动 lint + 审计日志

### 8.3 已知 Bug

- **Stop hook + plugin 组合 bug**（[Issue #10412](https://github.com/anthropics/claude-code/issues/10412)）：Stop hook exit 2 在 `.claude/hooks/` 下正常工作，但通过插件系统安装时显示"⏺ Stop hook prevented continuation"并卡住
- **Windows PreToolUse/PostToolUse 误报**（[Issue #45065](https://github.com/anthropics/claude-code/issues/45065)）：Windows 上即使 exit 0 也显示 "hook error"

---

*研究完成。数据来源截至 2026-07-25，基于 GitHub、Reddit、Hacker News、Anthropic 官方、dev.to 等渠道的公开内容。*
