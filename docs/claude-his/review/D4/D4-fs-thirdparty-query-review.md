# D4-fs-thirdparty-query 事实核查报告

> 核查日期: 2026-08-01
> 核查方式: WebSearch + WebFetch 外部验证 + 内部交叉比对
> 核查范围: 全量声称逐一核实
> 方向: D4

---

核查结论：11 项发现 + 19 条来源条目全量核实（WebFetch 逐条核对 GitHub 仓库/README/原始文章/HN 线程/PyPI），发现 3 处错误，其余声称均与来源一致。

## 错误 1: cclens 安装的三个 skill 名称遗漏了 `cclens-` 前缀

- **文件+行号**: `D:\data\learn\code\slTerminal\docs\claude-his\D4-fs-thirdparty-query.md` (行 67)
- **原声称**: "cclens `install` 子命令向 `~/.claude/skills/` 复制三个 skill：searching-history、exporting-history、resuming-from-history。"
- **错误类型**: 事实错误
- **正确信息**: 三个 skill 的实际名称为 `cclens-searching-history`、`cclens-exporting-history`、`cclens-resuming-from-history`（均带 `cclens-` 前缀）。文档给出的名称在来源页面中不存在。
- **反证来源**: https://github.com/negipo/cclens — README 原文："`install` — 'Installs three skills to `~/.claude/skills/`'"；"The three installed skills: `cclens-searching-history` — 'search past sessions by keyword, branch, or date'; `cclens-exporting-history` — 'export session conversations as Markdown'; `cclens-resuming-from-history` — 'load past session context into the current session'"

## 错误 2: 引用的动机描述 "technically readable, but practically useless for humans" 在全部引用来源中均不存在

- **文件+行号**: `D:\data\learn\code\slTerminal\docs\claude-his\D4-fs-thirdparty-query.md` (行 51)
- **原声称**: "其动机描述：JSONL "technically readable, but practically useless for humans"。"
- **错误类型**: 来源不支撑
- **正确信息**: 该句在发现 6 引用的全部来源中均查无此文：claude-code-transcripts 的 PyPI 页（描述为 "Convert Claude Code session files (JSON or JSONL) to clean, mobile-friendly HTML pages with pagination"）、GitHub 仓库 README（同前，无此表述）、Simon Willison 原始文章（https://simonwillison.net/2025/Dec/25/claude-code-transcripts/，最接近的表述是 "still are not quite as human-friendly as I'd like"）、alldevblogs 转载页均无此句。检索显示该短语实际出自 claude-code-transcripts 的 fork 项目 ai-code-sessions（PyPI），并非 Simon Willison 原项目动机描述。
- **反证来源**: https://pypi.org/project/claude-code-transcripts/（完整描述仅 "Convert Claude Code session files (JSON or JSONL) to clean, mobile-friendly HTML pages with pagination."）；https://raw.githubusercontent.com/simonw/claude-code-transcripts/main/README.md（开篇 tagline 同上，无此句）；https://simonwillison.net/2025/Dec/25/claude-code-transcripts/（原文最接近表述："the earlier timeline tools ... still are not quite as human-friendly as I'd like"，无 "technically readable" 句）；短语出处为 https://pypi.org/project/ai-code-sessions/ （fork 项目）

## 错误 3: session-index 描述"依赖免费 dashboard"与来源"dependency-free"（零依赖）矛盾

- **文件+行号**: `D:\data\learn\code\slTerminal\docs\claude-his\D4-fs-thirdparty-query.md` (行 106)
- **原声称**: "| https://github.com/MrPickering/session-index | 源码仓库 | 依赖免费 dashboard；Windows 路径 ... |"
- **错误类型**: 事实错误
- **正确信息**: session-index 是零依赖（dependency-free）的免费本地 dashboard——README 自述 "A small, dependency-free dashboard ..."，且 "There are no runtime packages to install."，无任何外部 dashboard 依赖、无付费/免费套餐之分。文档"依赖免费 dashboard"表述与来源直接矛盾（疑为 dependency-free 的误译）。
- **反证来源**: https://raw.githubusercontent.com/MrPickering/session-index/main/README.md — "A small, dependency-free dashboard for finding, resuming, and following up on local Codex and Claude Code sessions."；"There are no runtime packages to install."；"runs on your machine, listens only on localhost ... no telemetry, account, or remote assets"
