---
name: code-refactor
description: 当需要对整个代码仓做系统性审查与重构（多维度 review → 问题清单 → 分阶段修复），或已有 review 报告需转化为分阶段执行计划时使用。
---

# 系统性代码重构 Skill

## 概述

此 skill 编码了 slTerminal 项目的系统性重构流程：**探索→计划→分阶段 Workflow 执行→验证循环→提交**。

核心思路：将重构问题按模块和依赖关系分组为多个 Stage，每个 Stage 内使用 Workflow 工具并行编排 subagent，通过"重构→全量测试→逐项验证→修复循环"确保每阶段零回归。

## 使用方式

```
/code-refactor "<重构目标描述>"
```

也可分步调用：已完成 Step 1/2 后，要求"执行 Step 3/4 制定计划但不执行"。

## 执行流程

### Step 1: 探索

**维度先经用户确认**：`explore-dimensions.json` 的默认 5 维只是起点，实际维度（数量、划分）用 AskUserQuestion 与用户敲定后再 spawn。有效的补充维度：架构硬约束合规、安全审查、文档与代码一致性。

**Review subagent prompt 必备四要素**（缺一会出事故）：
1. **只读 review，禁止修改任何代码**
2. **不要向用户提问，按给定范围直接执行到底**（subagent 会继承"缺信息就问"的全局指令而中止等待，主 agent 只能 SendMessage 恢复，浪费一轮）
3. **报告只写存在的问题，不写优点**；每条带严重级别 + `file:line` + 修复建议
4. **用内置 Write 工具写中文 markdown**（filesystem MCP 写中文有乱码风险）

**产出命名**：`docs/review-NN-<dimension>.md`（NN 从 01 开始），不用 00-summary 式命名。

**进度跟踪（强制）**：
- spawn 时记录总数，每个 subagent 返回时汇报"已完成 N/总数"进度表
- **spawn 数 = 返回数后才进 Step 2**
- subagent 失败（如 API 配额 403）需重新 spawn 时：**先查 `docs/` 目标文件是否已产出**——"失败"的 agent 可能只是通知丢失，实际仍在跑并最终完成。已产出则不再 spawn；若已重复 spawn 立即 `TaskStop` 去重

### Step 2: 生成汇总

全部返回后读齐各分报告，生成 `docs/review-00-summary.md`：
- 按严重程度和模块汇总
- **跨维度共性问题单独列表**——同一问题被多个维度独立发现是最高可信度信号，汇总中优先呈现
- 只写问题，不写优点

### Step 3: 制定清单

**先去重再列清单**：86 条原始发现去重合并后约 77 项是常态（同一问题被多维度报告）。合并关系在清单中留痕（哪些条并入了哪项）。

**ID 编号**：按模块前缀 + 序号（本项目用 `SEC/BE/FE/TE/DOC`），Stage 划分按 ID 引用，禁止用"第几个问题"。

**必须 grill 用户确认的决策类型**（不问清不能列清单）：
- 修复范围：全量 / 高+中危 / 仅高危
- "改代码 vs 改文档"矛盾项（如约束与实际行为不符，往哪边对齐）
- 引入新功能或行为变更的项（如启用从未生效的沙箱）
- 大范围测试补写是否纳入

**产出**：`docs/refactoring-checklist.md`——按优先级（P0-P4）+ 模块组织的完整清单，每项含 ID、位置、修复要点。50-80 项均属正常规模。

### Step 4: 划分阶段

按"**Stage 内文件不重叠**"原则划分。**每 Stage 3-15 项**——强耦合大项（跨前后端的单一任务）可 2-3 项独立成 Stage。

划分规则：
- **并行组**：改动不同文件的项 → 同一 Stage 内并行 agent（≤5 个）
- **串行组**：改动共享文件的项 → 并入同一 agent 或 pipeline 顺序执行
- **Stage 间允许重复碰同一文件**：Stage 串行执行 + 每 Stage commit，先后 Stage 改同一文件无冲突（如 spawn.rs 可先后出现在安全 Stage 与补测 Stage）
- **文档同步固定为最后一个 Stage**——文档必须反映所有代码 Stage 完成后的最终状态
- 高风险项（核心模块重构）独立 Stage 并在计划标注人工验证点

**产出**：`docs/refactoring-stages.md`，每个 Stage 必须包含：
- 改动项 ID 列表 + agent 文件分工表（label / 负责项 / 文件，证明无文件重叠）
- 实现要点（含项目特定坑的引用）
- **验证项（`__VERIFY_ITEMS__`）**：每条是 grep/测试可机械检验的断言，不写"检查是否合理"
- commit message

计划文档同时写入 Step 5 执行规则与 Step 6 收尾步骤，使后续阶段可直接消费。**Step 3/4 只输出计划文档，不改任何代码。**

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

**执行 agent 的 prompt 必须显式声明项目禁区**（从根 CLAUDE.md 提取）。本项目禁区示例：`compute_conpty_flags` 固定 0x7 不可动（PASSTHROUGH_MODE 吞鼠标输入）。

### Step 6: 收尾

所有 Stage 完成后：
- 最终全量构建验证（含 L3/L4，不只每 Stage 的 L1/L2 门禁）
- 抽查根 CLAUDE.md 与最终代码一致性
- 归档 commit：review 报告 + 计划文档一并提交
- 输出收尾报告：Stage commit 列表、最终测试用例数、未修复项（若有，含原因）

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
- 单阶段内并行 agent ≤ 5 个；**两个并行 agent 禁止改同一文件**（硬性）
- 核心重构（如模块拆分）放在独立 Stage，分配更多 agent
- 修复循环最多 3 轮，超过需人工介入
- 每个执行 agent 只改分配给自己的项，不顺手改无关代码（surgical changes）
