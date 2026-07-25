---
name: verify-research-output
description: Use when reviewing structured web research output for factual correctness — the user has markdown files containing claims with cited sources and wants each claim verified against live sources. Triggers on phrases like "验证检索结果", "review 是否正确", "核查信息", "fact-check research".
---

# 验证检索输出

## 概述

对结构化网络检索的输出进行系统化事实核查——每条有来源引用的声称逐一外部验证，输出仅含错误的 review 文件。

核心原则：**AI 仅负责语义判断；固定流程通过脚本/伪代码固化；不确定时默认不修改。**

## 何时使用

- 用户有结构化检索产物（含声称 + 来源引用）需要验证
- 多方向并行检索后，需要对各方向交叉核实
- 输出需要区分"确认正确"和"发现错误"

**不使用**：单事实核查（"X 的 API 还在吗"）——直接 WebSearch。不用于验证代码逻辑。

## 快速参考

| 阶段 | 做什么 | 谁做 | 确定性 |
|------|--------|------|--------|
| 1. 并行验证 | spawn N 子代理，每个验证一个检索方向 | N 子代理 | [pipeline.md](pipeline.md) 定义流程 |
| 2. 交叉裁决 | 识别跨方向矛盾，查权威来源裁决 | 主代理 | [scripts/cross-check.ts](scripts/cross-check.ts) |
| 3. 逐条核实 | 对每个 review 声称判定接受/拒绝/部分接受 | 主代理 | [lessons/error-taxonomy.md](lessons/error-taxonomy.md) |
| 4. 修改源文件 | 仅对确认正确的声称修改源文件 | 主代理 | [lessons/reviewer-pitfalls.md](lessons/reviewer-pitfalls.md) |

## 文件索引

| 文件 | 职责 | 何时读 |
|------|------|--------|
| [pipeline.md](pipeline.md) | 4 阶段核心流程 + 子代理 prompt 规范 | 执行前必读 |
| [output-spec.md](output-spec.md) | Review 输出目录结构 + 错误条目格式 + 汇总模板 | 阶段 1 子代理写入前 |
| [lessons/error-taxonomy.md](lessons/error-taxonomy.md) | 4 种错误类型的症状、案例、根因、预防规则 | 阶段 3 分类判定前 |
| [lessons/reviewer-pitfalls.md](lessons/reviewer-pitfalls.md) | 审阅者常见失误模式——虚构内容、概念混淆等 | 阶段 3 判定前 + 出错后 |
| [lessons/verification-methods.md](lessons/verification-methods.md) | 验证工具×场景矩阵 + 证据层级 + 来源优先级 | 阶段 1 子代理 + 阶段 2 裁决 |
| [scripts/classify-error.ts](scripts/classify-error.ts) | 错误分类算法：声称+证据→错误类型 | 阶段 3 |
| [scripts/review-template.ts](scripts/review-template.ts) | Review 文件模板生成：路径→Markdown 骨架 | 阶段 1 子代理 |
| [scripts/cross-check.ts](scripts/cross-check.ts) | 交叉验证冲突检测：多 review→矛盾矩阵 | 阶段 2 |

## 确定性流程 vs AI 判断

| 确定性（脚本固定） | AI 判断 |
|-------------------|--------|
| 错误分类算法（4 类型判定规则） | 声称是否被来源支撑（语义理解） |
| Review 输出格式和模板 | 矛盾裁决（两个方向谁对） |
| 交叉验证冲突检测 | 搜索关键词生成 |
| 证据层级优先级 | 修改措辞的具体方式 |
| 子代理 prompt 结构 | |
| 源文件修改的安全约束 | |
