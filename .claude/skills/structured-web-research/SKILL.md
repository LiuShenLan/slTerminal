---
name: structured-web-research
description: Use when the user has a knowledge gap ("I don't know what X can do", "how does Y work", "research Z for me") and needs structured, multi-source, cited research output. Triggers on phrases like "检索", "搜索", "研究一下", "查找资料", "what is", "how does", "research".
---

# 结构化网络检索

## 概述

将模糊知识缺口分解为多个检索方向，并行 spawn 子代理搜索+抓取，输出带来源的结构化文档。

核心原则：**AI 仅负责判断，所有确定性流程通过脚本固定。**

`<skill目录>` = 本 SKILL.md 所在目录。脚本调用方式：`node <skill目录>/scripts/xxx.mjs`。

## 何时使用

- 用户明确说"我不确定 X 是什么/能做什么"
- 需要多个信息源交叉验证的调研任务
- 输出需要可追溯来源（每条信息附 URL）

**不使用**：单事实查询（"X 的版本号是什么"）——直接 WebSearch 即可。

## 快速参考

| 方向数 | 每方向来源下限 | fetch TopN |
|--------|-------------|------------|
| 1-2 | 3 | 3 |
| 3-5 | 5 | 5 |
| 6+ | 8 | 8 |

精确参数由 `scripts/depth-tier.mjs` 计算，不凭此表估算。

## 文件索引

| 文件 | 职责 | 何时读 |
|------|------|--------|
| [pipeline.md](pipeline.md) | 5 阶段核心流程 | 执行检索前必读 |
| [agent-prompt-template.md](agent-prompt-template.md) | 子代理 prompt 参数化模板 + 写入前自检清单 | spawn 子代理前 |
| [output-spec.md](output-spec.md) | 输出目录结构和汇总格式 | 汇总阶段前 |
| [scripts/decompose.mjs](scripts/decompose.mjs) | 方向分解（概念×维度矩阵） | 阶段 2 |
| [scripts/depth-tier.mjs](scripts/depth-tier.mjs) | 深度分层（方向数→参数） | 阶段 2 |

## 确定性流程 vs AI 判断

| 确定性（脚本固定） | AI 判断 |
|-------------------|--------|
| 方向分解维度矩阵（decompose.mjs） | 从问题提取核心概念 |
| 深度分层参数（depth-tier.mjs） | 搜索关键词生成 |
| 子代理 prompt 结构（模板文件） | 信息可信度评估 |
| 输出目录和格式（output-spec.md） | 来源冲突时的取舍 |
| 写入前自检清单（模板文件） | |

## 验证修正

检索产出后的事实核查 → 调用 `verify-research-output` skill（错误体系/证据层级/修改规范的唯一真值都在那边）。
