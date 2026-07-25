---
name: verify-research-output
description: Use when reviewing research/investigation markdown documents for factual correctness — the user has markdown files containing claims with cited sources and wants each claim verified against live sources. Triggers on phrases like "验证检索结果", "review 是否正确", "核查信息", "fact-check research".
---

# 验证检索输出

## 概述

对带来源引用的 markdown 文档做系统化事实核查——每条有来源引用的声称逐一外部验证，输出仅含错误的 review 文件。输入可以是 `structured-web-research` 的产出，也可以是任何手写/其他工具生成的带引用 markdown。

核心原则：**AI 仅负责语义判断；固定流程通过脚本固化；不确定时默认不修改。**

`<skill目录>` = 本 SKILL.md 所在目录。脚本调用方式：`node <skill目录>/scripts/xxx.mjs`。

## 何时使用

- 用户有带"声称 + 来源引用"的 markdown 文档需要逐条核查
- 多份文档之间需要交叉核实矛盾
- 输出需要区分"确认正确"和"发现错误"

**不使用**：单事实核查（"X 的 API 还在吗"）——直接 WebSearch。不用于验证代码逻辑。

## 快速参考

| 阶段 | 做什么 | 谁做 | 固化物 |
|------|--------|------|--------|
| 1. 并行验证 | spawn N 子代理，每个验证一组文件 | N 子代理 | [pipeline.md](pipeline.md) 阶段 1 |
| 2. 交叉裁决 | AI 提取 claims → 脚本检测冲突 → AI 查权威来源裁决 | 主代理 | [scripts/detect-conflicts.mjs](scripts/detect-conflicts.mjs) |
| 3. 逐条核实 | 对每条 review 声称**全量**独立复核，判定接受/拒绝/部分接受 | 主代理 | [scripts/classify-error.mjs](scripts/classify-error.mjs) |
| 4. 修改源文件 | 仅对确认正确的声称修改源文件 | 主代理 | [lessons/edit-discipline.md](lessons/edit-discipline.md) |

## 文件索引

| 文件 | 职责 | 何时读 |
|------|------|--------|
| [pipeline.md](pipeline.md) | 4 阶段核心流程 + 子代理 prompt 规范 | 执行前必读 |
| [output-spec.md](output-spec.md) | Review 输出目录结构 + 错误条目格式 + 汇总模板 | 阶段 1 子代理写入前 |
| [lessons/error-taxonomy.md](lessons/error-taxonomy.md) | 4 种错误类型的症状、案例、根因、预防规则（错误体系唯一真值） | 阶段 3 分类判定前 |
| [lessons/verification-methods.md](lessons/verification-methods.md) | 工具×场景矩阵 + 5 级证据层级 + 来源优先级（证据体系唯一真值） | 阶段 1 子代理 + 阶段 2 裁决 |
| [lessons/reviewer-pitfalls.md](lessons/reviewer-pitfalls.md) | 审阅者常见失误模式——虚构内容、概念混淆等 | 阶段 3 判定前 + 出错后 |
| [lessons/edit-discipline.md](lessons/edit-discipline.md) | 修改规范：Edit 流程 + 防引入新错（修改规范唯一真值） | 阶段 4 修改前 |
| [scripts/detect-conflicts.mjs](scripts/detect-conflicts.mjs) | 冲突检测：claims JSON→矛盾矩阵 | 阶段 2 |
| [scripts/classify-error.mjs](scripts/classify-error.mjs) | 错误分类：反证类型→错误类型+严重程度 | 阶段 3 |
| [scripts/review-template.mjs](scripts/review-template.mjs) | Review 文件骨架生成 | 阶段 1 子代理 |

## 确定性流程 vs AI 判断

| 确定性（脚本固定） | AI 判断 |
|-------------------|--------|
| 冲突检测算法（detect-conflicts.mjs） | 从文件提取 claims（语义理解） |
| 错误分类规则映射（classify-error.mjs） | 声称是否被来源支撑（语义理解） |
| Review 输出格式和模板（review-template.mjs + output-spec.md） | 反证属于哪种 kind |
| 证据层级优先级（verification-methods.md） | 矛盾裁决（两个方向谁对） |
| 子代理 prompt 结构（pipeline.md） | 搜索关键词生成 |
| 源文件修改的安全约束（edit-discipline.md） | 修改措辞的具体方式 |
