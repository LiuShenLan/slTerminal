# D3-cli-native-query 事实核查报告

> 核查日期: 2026-08-01
> 核查方式: WebSearch + WebFetch 外部验证 + 内部交叉比对
> 核查范围: 全量声称逐一核实
> 方向: D3

---

## 未发现错误（已验证约 40 项声称）

逐条核实结论：

**发现 1**（cli-reference + sessions）：`--resume`/`-r` 官方原文逐词一致（含 "As of v2.1.144, background sessions appear in the picker marked with `bg`"）；五命令表（`--continue`/`--resume`/`--resume <name>`/`--from-pr <number>`/`/resume`）与 sessions 页 Resume a session 表格逐词一致。✓

**发现 2**（sessions）：session picker 快捷键表 9 项全部与官方表格逐词一致（含 Ctrl+A/Ctrl+W/Ctrl+B 描述、`Ctrl+W` "Only shown in multi-worktree repositories"）；选择器默认范围、每行显示字段、Ctrl+A 项目路径、歧义名行为（`--resume <name>` 打开选择器预填搜索词 vs `/resume <name>` 报错）、`-p`/Agent SDK 会话不出现在选择器、session ID 查找限定项目目录及 worktrees 并报 `No conversation found with session ID: <session-id>`、v2.1.211 `/loop` 首条 prompt 不显示、v2.1.169 `/cd` 会话转入新目录存储——全部与官方原文一致。✓

**发现 3**（cli-reference）：7 个标志原文逐词一致（`--continue`/`-c`、`--resume`/`-r`、`--fork-session`、`--session-id`、`--from-pr`、`--name`/`-n`、`--no-session-persistence`）；后台会话子命令 5 条（attach/logs/respawn/rm/stop，`stop` 注明 "Also accepts `claude kill`"）；`claude project purge [path]` 描述一致；"`claude --help` does not list every flag" 声明在 cli-reference 页 CLI flags 引言中逐词存在。✓

**发现 4**（issue #16901 + PR #34168）：issue 由 qsimeon 于 2026-01-08 提出、标签 `area:core`/`area:tui`/`enhancement`、优先级 Medium、两条 UX 缺口引文逐词一致、当前状态 Closed（经 PR #34168 关闭）；PR #34168 标题 "feat(plugins): Sessions plugin for listing and deleting sessions"、状态 Open 未合并、插件命令 `/sessions:list`/`/sessions:delete`、孤儿检测与三文件清理（JSONL + metadata + subagent logs 目录）、"The CLI flag approach ... would require changes to the core codebase" 引文逐词一致；文档已自注 issue Closed 与 PR 未合并并存的快照性质。✓

**发现 5**（agent-sdk/typescript + headless + sessions）：`listSessions()` 签名与描述、ListSessionsOptions 三参数（dir/limit/includeWorktrees 默认 true）、SDKSessionInfo 全部 10 字段与官方 TS 参考页逐字一致；`query()` Options 的 `resume`/`continue`/`forkSession` 条目一致；`session_id=$(claude -p "Start a review" --output-format json | jq -r '.session_id')` → `claude -p "Continue that review" --resume "$session_id"` 示例在 headless 页逐词存在；`claude -p --resume <session-id> --output-format json "summarize what we changed" | jq -r '.result'` 示例在 sessions 页 "Access conversations from scripts" 一节逐词存在（发现 5 的多来源引用覆盖该页）；transcript 存储位置 `~/.claude/projects/<project>/<session-id>.jsonl`、"The entry format is internal to Claude Code and changes between versions"、CLAUDE_CONFIG_DIR、30 天保留期（cleanupPeriodDays）、CLAUDE_CODE_SKIP_PROMPT_HISTORY 全部一致。附带核实："按 `lastModified` 降序" 虽未见于所引 TS 参考页，但官方 Python SDK 页明确 "Results are sorted by `last_modified` descending, so the first item is the newest"，声称事实正确。✓

**发现 6**（sessions）：还原范围全部与官方一致——会话历史全文（含工具调用与结果）、模型（`--model`/`ANTHROPIC_MODEL` 族覆盖时不还原）、agent、权限模式（`plan`/`bypassPermissions` 永不还原）、活跃 goal（turn 计数/计时器/token 基线重置）、未过期定时任务；不还原标志清单（`--mcp-config`/`--settings`/`--plugin-dir`/`--fallback-model`/`/add-dir` 目录）一致；settings.json/settings.local.json 启动重读一致；Pro/Max 非活动约 1 小时以上且超 100,000 tokens 的三选项对话框（Resume from summary / Resume full session as-is / Don't ask me again）一致。✓

**发现 7**（CHANGELOG @ 69da5e8）：五个版本的条目全部逐词一致（0.2.93 引入 `--continue`/`--resume`、1.0.27 `/resume` slash 命令、2.0.12 hooks 摘要、2.0.27 分支过滤+搜索、2.0.64 命名会话三连）；changelog 只含版本号不含日期属实。✓

**发现 8**（tokenbender/agent-guides）：`rg -l --no-ignore -g '*.jsonl'` 全文扫描、jq 筛选 `user` 事件按 timestamp 排序、`select(.type == "summary")` 首条提取摘要、`find ~/.claude/projects -name '*.jsonl' -mtime -7`、`~/.claude.json` 的 `projects.<path>.history` 存每项目最近 prompt 字符串（无回复）、claude-find/claude-sessions 两个 shell 辅助函数——全部与指南原文一致。✓

**来源清单**：10 个 URL 全部可访问且内容与标注的关键内容一致（含 commands 页 "`/resume` returns to an earlier conversation" 及 `/clear`、`/branch`、`/cd`、`/add-dir` 条目中对 `/resume`/`--resume` 的交叉引用；linuxcommandlibrary man 页两条引文逐词一致且确实不含 `--fork-session`/`--from-pr`）。✓

内部交叉比对：发现 1–8 与来源清单、备注声明之间无矛盾（备注明确未将社区第三方工具的细节纳入正文断言，与正文一致）。
