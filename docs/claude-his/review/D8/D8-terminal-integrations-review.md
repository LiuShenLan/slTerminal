# D8-terminal-integrations 事实核查报告

> 核查日期: 2026-08-01
> 核查方式: WebSearch + WebFetch 外部验证 + 内部交叉比对
> 核查范围: 全量声称逐一核实
> 方向: D8

---

未发现错误（已验证 16 个来源 URL、约 160 项细节声称）。

核验明细：
- 官方文档（code.claude.com/docs/en/sessions、vs-code）：`--continue`/`--resume`/`--from-pr`/`/resume` 行为表、session ID 作用域与错误消息原文、picker 键位（Ctrl+W/A/B/R/Space）、跨项目剪贴板行为、JSONL 存储路径与编码、内部格式警告原文、4 个配置项、恢复还原语义与三选项对话框阈值（1 小时/100,000 tokens）、VS Code 扩展 Session history/双 tab/URI handler——全部与官方页面逐字相符
- 社区工具 README（claude-recall、claude-code-tools 0.2.5、clauhist 1.0.1、ccboard 0.16.3、cc-history-viewer、resume-resume、tmux-claude-session-manager、tmux-claude-code、claude-launcher、claude+）：工具名/命令名/端口号（3080/3333）/依赖版本（tmux≥3.2、Python 3.6+/3.11+、Node≥18、CC≥2.1.139）/greedy path 解码示例/性能数字（p50 6ms@166MB、94ms@3GB、~5.5s）/引用原文（"Nothing here scans processes…"、"the source of truth…"、"decodes the project path (cross-referencing the JSONL cwd field)"、"No dependencies beyond fzf and python3" 等）全部吻合
- GitHub issues：49095（72 会话/14 项目、All Projects tab 设计、Closed as not planned、关联 4 issue、agsoft 扩展）、60610（10 会话仅 4 个索引、history.jsonl 为 UI 数据源）、56104（v2.1.126 回归、CLI 正常）、50170（realpath→UNC、`--server-share-myproject` vs `X--myproject`、跳过 realpath 修复建议）、47746（worktree 参与经 issue 评论确认）、45814（单会话分裂多条目）、9258（symlink 历史丢失经 30 条评论确认）——均与 issue 正文/评论相符
- 文中标注"未直接抓取"的条目（60610/56104/50170、claude-launcher、zellij/cch/rvu）经独立抓取验证其内容声称均属实；检索缺口部分自我声明为未验证，不构成声称
