# D5-resume-usage 事实核查报告

> 核查日期: 2026-08-01
> 核查方式: WebSearch + WebFetch 外部验证 + 内部交叉比对
> 核查范围: 全量声称逐一核实
> 方向: D5

---

<!-- 逐条追加错误条目，格式见 output-spec.md「错误条目模板」。
     全部正确则写: 未发现错误（已验证 N 项声称）。 -->

## 错误 1: issue #46865 标题被虚构添加了 `[FEATURE]` 前缀

- **文件+行号**: `D:\data\learn\code\slTerminal\docs\claude-his\D5-resume-usage.md` (第 119 行)
- **原声称**: `- 详情: "[FEATURE] Add setting to filter /resume picker by current project directory"（Claude Code v2.1.104，2026-04 时点）。报告称 **v2.1.101 起选择器默认显示所有项目的会话**（官方 changelog 视为修复"narrow default view hiding sessions from other projects"）…无维护者评论，已关闭。`
- **错误类型**: 事实错误
- **正确信息**: issue #46865 的实际标题为 **"Add setting to filter /resume picker by current project directory"**，不含 `[FEATURE]` 前缀。该前缀为文档作者添加（文档中同为引号的 #47581 标题 `[FEATURE] Session picker search should also filter by project path` 是逐字正确的，可作对照）。标题以外的声称（2026-04-12 打开、环境版本 v2.1.104、v2.1.101 起默认全项目、`resume.filterByCurrentProject: true` 设置请求、无维护者评论、已关闭）均与来源一致。
- **反证来源**: https://github.com/anthropics/claude-code/issues/46865（两次独立抓取确认）
  - 关键引用: "**Exact title** (no prefix — no `[FEATURE]`, `[BUG]`, or `[DOCS]` tag): > Add setting to filter /resume picker by current project directory"；"**Status:** Closed"

---

**核查说明**：除上述 1 处错误外，其余声称全部与来源一致。已核实 12 个来源 URL（cli-reference、sessions、headless、agent-sdk/sessions 四个官方页面 + issues #46865/#47581/#59941/#8584 + claude-picker 仓库 + skillsplayground.com + skywork.ai 两个博客）的 60 余项声称：

- 全部 CLI 标志名逐字一致：`--resume`/`-r`、`--continue`/`-c`、`--fork-session`（无 `--fork`）、`--session-id`、`--from-pr`、`--name`/`-n`，官方原文与引用逐字吻合（含 "As of v2.1.144…marked with `bg`" 版本注记）
- 发现 5/6/8 的选择器行为、恢复内容清单、长会话对话框、transcript 存储路径/30 天保留/`cleanupPeriodDays`/`CLAUDE_CONFIG_DIR` 均逐字吻合；发现 7/10 的 headless 与 SDK 引用逐字吻合
- 发现 9 的三个 issue（#46865 除标题前缀外、#47581、#59941）的版本号、打开日期、关闭方式、逐字引用均一致；#8584 标题在文档中被截断为前缀（`[DOCS] Missing Documentation for Various Claude Code Features`，实际全称含 `(CLI Flags, Slash Commands, & Tools)`），因所引部分逐字正确、未产生误导，不计为错误
- 无死链、无过时信息（所有页面抓取于 2026-08-01，内容与文档引用一致）
