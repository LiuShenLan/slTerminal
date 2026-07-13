---
name: code-refactor
description: 系统性代码重构——从代码仓探索到分阶段执行的全链路自动化重构流程。基于审查报告自动划分阶段，使用 Workflow 工具编排 subagent 并行重构，每阶段自动验证循环直至全部修复。
---

# 系统性代码重构 Skill

## 概述

此 skill 编码了 slTerminal 项目的系统性重构流程：**探索→计划→分阶段 Workflow 执行→验证循环→提交**。

核心思路：将重构问题按模块和依赖关系分组为多个 Stage，每个 Stage 内使用 Workflow 工具并行编排 subagent，通过"重构→全量测试→逐项验证→修复循环"确保每阶段零回归。

## 使用方式

```
/refactor "<重构目标描述>"
```

例如：`/refactor "审查前端性能和后端错误处理，优化代码质量"`

## 执行流程

### Step 1: 探索

读取 `explore-dimensions.json` 获取预定义的探索维度。启动并行 Explore Agent，每个维度对应一个 Agent。

- 默认 5 维度：前端架构、后端架构、代码质量、测试体系、性能可维护性
- 每个 Agent 深度阅读代码文件，输出发现的问题到 `docs/` 目录
- Agent 可继续 spawn 子 Agent 深入分析子领域

### Step 2: 生成汇总

所有探索 Agent 完成后，生成 `docs/00-summary.md`——按严重程度和模块汇总所有发现，包含跨维度高频问题 TOP N。

### Step 3: 制定清单

基于汇总报告，主 agent 与用户交互确定重构范围，生成 `docs/refactoring-checklist.md`——按模块和优先级组织的完整问题清单（~50-70 项）。

### Step 4: 划分阶段

按"文件不重叠、改动无冲突"原则划分 Stage。每个 Stage 包含 5-15 个改动项，分属不同文件。

划分规则：
- **并行组**：改动不同文件的项 → 同一 Stage 内并行 agent
- **串行组**：改动共享文件的项 → 同一 Stage 内 pipeline 顺序执行
- **高风险项**：核心模块拆分（如 useXterm 拆分）→ 独立 Stage

### Step 5: 执行阶段

主 agent 逐 Stage 执行。每个 Stage 使用 Workflow 工具：

**阶段 Workflow 结构**（见 `templates/stage-workflow.js`）：
```
Phase 1: 并行重构 → 多个 agent 同时修改互不影响的文件
Phase 2: 串行重构 → pipeline 处理有依赖的改动（可选）
Phase 3: 全量测试 → agent 执行所有测试命令
Phase 4: 逐项验证 → agent 逐条检查，返回结构化结果
```

Workflow 返回后，主 agent 检查 `verifyResult.allFixed`：
- `true` → 所有项通过 → `git commit` → 下一阶段
- `false` → 启动修复 Workflow

**修复 Workflow 结构**（见 `templates/fix-workflow.js`）：
```
Phase 1: 修复问题 → agent 修复 failedItems
Phase 2: 全量测试 → agent 执行所有测试
Phase 3: 逐项验证 → agent 重新检查
```

主 agent 循环直到 `allFixed === true`。

### Step 6: 收尾

所有 Stage 完成后：
- 最终全量构建验证
- 可选：更新 CLAUDE.md 文档（使用此 skill 的文档同步子流程）

## 配置

项目特定配置在 `config.json`（与 SKILL.md 同目录）：
- **commands**: 测试/构建/检查命令
- **modules**: 前端/后端/测试模块路径
- **architecture**: 架构约束定义
- **claudeMdFiles**: CLAUDE.md 文件清单（重构后需同步更新）

修改 `config.json` 即可适配不同的测试框架或目录结构。

## 探索维度

`explore-dimensions.json` 定义代码仓探索的维度和 Agent 提示词骨架。修改此文件可调整后续重构的探索方向和深度。

## 模板

所有 Workflow 模板在 `templates/` 目录：

| 文件 | 用途 |
|------|------|
| `stage-workflow.js` | 单阶段重构 Workflow 模板 |
| `fix-workflow.js` | 修复循环 Workflow 模板 |
| `verify-schema.json` | 验证结果 JSON Schema |

主 agent 使用时：读取模板 → 替换 `__PLACEHOLDER__` → 传给 `Workflow({ script: ... })`。

### 占位符说明

| 占位符 | 替换为 |
|--------|--------|
| `__STAGE_NUMBER__` | 阶段编号 (1, 2, ...) |
| `__STAGE_DESCRIPTION__` | 阶段简短描述 |
| `__STAGE_NAME__` | Workflow 名称 (stage1-xxx) |
| `__PARALLEL_AGENTS__` | 并行 agent 数组，每个含 `label` 和 `prompt` |
| `__SEQUENTIAL_AGENTS__` | pipeline 阶段数组（无可留空） |
| `__FAILED_ITEMS__` | 修复 Workflow 中需要修复的项 ID 列表 (JSON 数组) |
| `__PROJECT_ROOT__` | 项目根路径（从 config.json 读取） |
| `__TEST_COMMANDS__` | 测试命令列表（从 config.json 的 commands 读取） |

## 注意事项

- 每阶段一个 commit：`refactor: StageN — 描述`
- 直接 main 分支操作，不使用 git worktree
- 单阶段内 agent 数量控制在 5 个以内
- 核心重构（如模块拆分）放在独立 Stage，分配更多 agent
- 修复循环最多 3 轮，超过需人工介入
