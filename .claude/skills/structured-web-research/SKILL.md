---
name: structured-web-research
description: Use when the user has a knowledge gap ("I don't know what X can do", "how does Y work", "research Z for me") and needs structured, multi-source, cited research output. Triggers on phrases like "检索", "搜索", "研究一下", "查找资料", "what is", "how does", "research".
---

# 结构化网络检索

## 概述

将模糊知识缺口分解为多个检索方向，并行 spawn 子代理搜索+抓取，输出带来源的结构化文档。

核心原则：**AI 仅负责判断，所有确定性流程通过脚本/代码固定。**

## 何时使用

- 用户明确说"我不确定 X 是什么/能做什么"
- 需要多个信息源交叉验证的调研任务
- 输出需要可追溯来源（每条信息附 URL）

**不使用**：单事实查询（"X 的版本号是什么"）——直接 WebSearch 即可。

## 快速参考

| 复杂度 | 方向数 | 嵌套深度 | 每方向来源下限 |
|--------|--------|---------|-------------|
| 简单 | 1-2 | 1 层 | 3 |
| 中等 | 3-5 | 2-3 层 | 5 |
| 复杂 | 5+ | 4-5 层 | 8 |

## 文件索引

| 文件 | 职责 | 何时读 |
|------|------|--------|
| [pipeline.md](pipeline.md) | 5 阶段核心流程 + 决策树 | 执行检索前必读 |
| [agent-prompt-template.md](agent-prompt-template.md) | 子代理 prompt 参数化模板 | spawn 子代理前 |
| [output-spec.md](output-spec.md) | 输出目录结构和汇总格式 | 汇总阶段前 |
| [scripts/decompose.ts](scripts/decompose.ts) | 方向分解算法 | 阶段 2 |
| [scripts/depth-tier.ts](scripts/depth-tier.ts) | 深度分层算法 | 阶段 2 |
| [scripts/output-checklist.ts](scripts/output-checklist.ts) | 输出自检清单 | 子代理写入前 |
| [scripts/cross-validate.ts](scripts/cross-validate.ts) | 交叉验证算法 | 阶段 5 |
| [lessons/source-quality.md](lessons/source-quality.md) | 源头质量：常见错误类型 | 检索前 + 出错后 |
| [lessons/verification-strategy.md](lessons/verification-strategy.md) | 验证策略：工具和方法 | 阶段 5 |
| [lessons/edit-discipline.md](lessons/edit-discipline.md) | 修改规范：避免引入新错 | 修改源文件前 |

## 确定性流程 vs AI 判断

| 确定性（脚本固定） | AI 判断 |
|-------------------|--------|
| 方向分解维度矩阵 | 从问题提取核心概念 |
| 深度分层（方向数→参数） | 搜索关键词生成 |
| 子代理 prompt 结构 | 信息可信度评估 |
| 输出目录和格式 | 来源冲突裁决 |
| 输出自检清单 | |
| 交叉验证算法 | |
