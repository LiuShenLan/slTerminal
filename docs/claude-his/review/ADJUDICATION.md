# 跨方向矛盾裁决

> 裁决日期: 2026-08-01
> 依据: verify-research-output lessons/verification-methods.md 的证据层级（官方文档 L1 > 源码/原始文件 L2 > 交叉 L3 > 间接 L4）

## 冲突 1: 目录编码规则 — D1 全量替换 vs D2 部分替换

| 方向 | 声称 |
|------|------|
| D1 (D1-storage-location.md:16) | 目录名 = 工作目录绝对路径中**非字母数字字符全部替换为 `-`**（官方文档原文） |
| D2 (D2-transcript-jsonl-format.md:13) | `[project-path]` 编码——**`/` 与 `_` 替换为 `-`**（ccrider schema 转述） |

**裁决**: D1 正确，D2 表述不完整。
**正确信息**: 规则为「非字母数字字符全部替换为 `-`」——`/`、`_`、`:`（Windows 盘符冒号）、空格等均替换（Windows 例 `C:\dev\foo_bar` → `C--dev-foo-bar`，双破折号来自 `:` 与 `\` 连续替换）。
**依据**: 官方 sessions 文档原文「`<project>` is your working directory path with non-alphanumeric characters replaced by `-`」(https://code.claude.com/docs/en/sessions)；D1 验证子代理的 Issue #54066 一手证据 `C:\dev\foo_bar` → `C--dev-foo-bar`。
**严重程度**: P2（D2 的例子正确，但规则描述遗漏冒号等字符——对 Windows 路径编码有实际误导风险）

## 冲突 2: 存储路径 — D1/D2/D5 记号差异

| 方向 | 声称 |
|------|------|
| D1 (D1-storage-location.md:8) | `~/.claude/projects/<project>/<session-id>.jsonl` |
| D2 (D2-transcript-jsonl-format.md:13) | `~/.claude/projects/[project-path]/[sessionId].jsonl` |
| D5 (D5-resume-usage.md:111) | `~/.claude/projects/` JSONL 文件 |

**裁决**: 伪冲突——`<project>` / `[project-path]` 为同一概念的两种记号（编码后工作目录路径），三方事实一致。不修改。

## 冲突 3: 选择器默认范围 — D3 vs D5

| 方向 | 声称 |
|------|------|
| D3 (D3-cli-native-query.md:52) | 「默认当前项目，Ctrl+A 扩全项目」（主代理 claims 提取错误，行 52 实为标志表） |
| D5 (D5-resume-usage.md:47) | 默认当前 worktree + `/add-dir` 会话，Ctrl+A 扩全项目 |

**裁决**: 伪冲突——D3 发现 2（D3-cli-native-query.md:43）实际表述为「选择器默认范围：当前 worktree 的会话（后台会话标 `bg`）+ 经 `/add-dir` 添加过当前目录的会话」，与 D5 逐字一致，均出自官方 sessions 文档。主代理 claims 提取时行号与内容错误，不构成文档问题。不修改。

## 冲突汇总

| # | 冲突 | 裁决 | 需修改文件 | 严重程度 |
|---|------|------|-----------|---------|
| 1 | 目录编码规则 | D1 正确，D2 不完整 | D2-transcript-jsonl-format.md | P2 |
| 2 | 存储路径 | 伪冲突（记号差异） | — | — |
| 3 | 选择器默认范围 | 伪冲突（提取错误） | — | — |
