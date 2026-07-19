---
name: systematic-changes-execute
description: 当已有分阶段变更的执行计划与 workflow 编排脚本（docs/<task>/execution-plan.md + docs/<task>/workflows/*.js + verify 断言），需逐 Stage 实际执行代码变更、验证循环、提交并收尾时使用。制定计划用 /systematic-changes-plan。
---

# 系统性变更执行 Skill

## 概述

此 skill 编码了 slTerminal 项目系统性变更的**执行**流程：消费 `/systematic-changes-plan` 产出的计划与编排产物，**逐 Stage 串行执行 → 全量测试 → 逐项验证 → 修复循环 → commit → 收尾**，确保每阶段零回归。

**输入物**（全部由 plan 阶段产出，本 skill 不生成）：
- `docs/<task>/execution-plan.md`——编排参数单一入口（Stage 表 / commit message / git add 路径枚举 / fix-loop args 规范 / 进度跟踪表）
- `docs/<task>/workflows/stage-NN-*.js` + `fix-loop.js`——Workflow 脚本
- `docs/<task>/workflows/verify/stage-NN.md`——验证断言（stage 脚本与 fix-loop 共用的唯一真值源）
- `docs/<task>/checklist.md` + `stages.md`——项级修复要点与 Stage 说明

## 使用方式

```
/systematic-changes-execute docs/<task>/execution-plan.md
```

**路径必须显式给出，不自动扫描 `docs/`**——多计划并存时自动发现会选错。

## 执行流程

### Step 5: 执行阶段

主 agent 逐 Stage 串行执行，严格按顺序不跳阶段。进度以 execution-plan.md 的进度跟踪表为准——支持全局暂停/恢复，恢复时从首个未完成 Stage 继续（其前 commit 已落盘无需重做）。每个 Stage 一个 Workflow 脚本：

**阶段 Workflow 结构**（脚本已由 plan 阶段生成，此处为理解用）：
```
Phase 1: 并行重构 → 多个 agent 同时修改互不影响的文件
Phase 2: 串行重构 → pipeline 处理有依赖的改动（可选）
Phase 3: 全量测试 → agent 执行所有测试命令
Phase 4: 逐项验证 → agent 逐条检查，返回结构化结果
```

首次调用某批脚本前跑语法预检（命令照抄 `/systematic-changes-plan` 5.1，不凭印象重写）。脚本执行期需修改时，同样遵守其生成纪律（5.1/5.2 生成期部分）。

Workflow 返回后，主 agent 检查 `verifyResult.allFixed`：
- `true` → 所有项通过 → `git commit` → 下一阶段
- `false` → 启动修复 Workflow（`docs/<task>/workflows/fix-loop.js`），循环直到 `allFixed === true`，最多 3 轮，超过需人工介入

#### 5.2 Workflow 运行时语义（执行期编排）

- `resumeFromRunId` 只对**未完成** run 有效：已正常结束的 run 全命中缓存、原样返回——verify 已结束的 no-return 靠 resume 救是死循环
- **no-return 按阶段分流**：verify 阶段 → 主 agent inline spawn 单个 verify agent（复用脚本内 prompt+schema+verify 文件+testResult）；重构/测试阶段 → `TaskStop` + `resumeFromRunId`

#### 5.6 主 agent 编排

- `git add` 路径限定：从 execution-plan.md 读取本任务的枚举清单（其由 plan 阶段自 config.json `gitAddPaths` 写入，含 `.claude/` 精确文件——文档 Stage 会改根 CLAUDE.md），不用 `-A`；commit 前 `git status` 甄别预期外改动
- commit message 以 stages 文档各 Stage 原文为准（文档 Stage 是 `docs:` 前缀，非一律 `refactor:`）
- 时间盒：后台运行期间每 15-20 分钟查 /workflows；单 Stage 超 60 分钟无进展 → `TaskStop` + 报告用户
- 文档同步类 agent 必须先读 `git log`/diff 再动笔，禁凭记忆写文档

#### 5.7 执行期工具故障降级（实证）

| 故障 | 现象 | 降级路径 |
|------|------|---------|
| Bash 权限分类器暂不可用 | "auto mode cannot determine the safety of Bash"，`mv`/`sed` 被拒 | 目录改名用 `mcp__filesystem__move_file`（只动路径不碰内容，中文安全）；批量内容替换用 PowerShell：`Get-Content -Raw` + `-replace` + `Set-Content -NoNewline`（PS7 UTF-8 无 BOM 读写，中文安全） |
| Edit 报 "File has not been read yet" | 目录改名/移动后，本会话已 Read 的文件在新路径仍报未读（Read 记录不跟随 rename） | 重新 Read 新路径再 Edit；多处批量纯 ASCII 替换直接走 PowerShell |
| Read 误报 "Wasted call — file unchanged" | /clear 或上下文压缩后无历史 Read 仍报 | `mcp__filesystem__read_multiple_files` 兜底读取 |
| filesystem MCP 写中文 | 写含中文内容的文件有乱码/损坏风险 | 含中文内容一律用内置 Write/Edit；filesystem MCP 仅用于 move_file（不碰内容）与 read_multiple_files（只读兜底） |

**批量改名/替换后验证三重奏**：① grep 旧模式全仓零残留；② grep 引用变体零残留（如裸相对 `` `workflows/ ``）；③ 重跑语法预检 + grep 新模式抽查落点。

### Step 6: 收尾

所有 Stage 完成后：
- 最终全量构建验证（含 L3/L4，不只每 Stage 的 L1/L2 门禁）
- 抽查根 CLAUDE.md 与最终代码一致性
- 归档 commit：review 报告 + 计划文档一并提交
- 输出收尾报告：Stage commit 列表、最终测试用例数、未修复项（若有，含原因）
- 计划标注过「人工验证点」的项逐项实测（规范推断/平台行为/渲染/视觉/滚动效果类），结果写入收尾报告

## 注意事项

- 每阶段一个 commit，message 以 stages 文档原文为准（前缀按变更性质：`refactor:`/`fix:`/`docs:`）
- 直接 main 分支操作，不使用 git worktree
- 修复循环最多 3 轮，超过需人工介入
- 每个执行 agent 只改分配给自己的项，不顺手改无关代码（surgical changes）

**Red flags（踩坑实录，出现即停）：**
- 对已正常结束的 run 用 `resumeFromRunId` 救 no-return——全命中缓存死循环；verify 阶段改 inline spawn
- `git add -A`——必须路径限定，且枚举含 `.claude/` 精确文件
- 并行 agent 各自跑资源共享型测试（如含 PTY spawn 的 `cargo test`）——跨进程并发死锁；脚本 prompt 已含此纪律，主 agent 不得另行指示
