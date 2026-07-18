---
name: systematic-changes
description: 当需要对整个代码仓做系统性审查与重构（多维度 review → 问题清单 → 分阶段修复），或已有 review 报告需转化为分阶段执行计划，或已有故障排查/debug 报告需转化为分阶段修复计划时使用。
---

# 系统性变更 Skill（重构 / 修复）

## 概述

此 skill 编码了 slTerminal 项目的系统性变更流程（重构或修复）：**探索→计划→分阶段 Workflow 执行→验证循环→提交**。

核心思路：将变更问题按模块和依赖关系分组为多个 Stage，每个 Stage 内使用 Workflow 工具并行编排 subagent，通过"变更→全量测试→逐项验证→修复循环"确保每阶段零回归。

## 使用方式

```
/systematic-changes "<变更目标描述>"
```

也可分步调用：已完成 Step 1/2 后，要求"执行 Step 3/4 制定计划但不执行"。

## 执行流程

### Step 1: 探索

**维度先经用户确认**：`explore-dimensions.json` 的默认 6 维只是起点，实际维度（数量、划分）用 AskUserQuestion 与用户敲定后再 spawn。有效的补充维度：架构硬约束合规、安全审查、文档与代码一致性、跨边界契约一致性（前后端命令签名/必填字段/DTO 双边对照——缺此维度曾漏检"后端加必填参数、前端未同步"故障）。

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
- 防复发/回归测试是否纳入（守护本次修复的契约守卫、时序断言等）

**产出**：`docs/refactoring-checklist.md`（修复类任务用 `docs/debug-checklist.md`，按任务性质命名）——按优先级（P0-P4）+ 模块组织的完整清单，每项含 ID、位置、修复要点。50-80 项均属正常规模。

#### 修复类清单（debug 报告输入）的必备项

输入为故障分析报告（而非全量 review）时，除逐故障修复项外，清单必须含以下五类项（本次 DBG-1~11 实证，缺一则同类故障必复发）：

1. **波及面全量调用点**：从分析报告精确引用全部调用点（含边缘调用点，如 `App.tsx` 关窗清理 kill），逐项成清单条目，verify 逐点机械断言（DBG-2 = 8 处调用点逐一列出）。不接受"大致位置"。
2. **契约守卫测试**：跨边界契约（IPC 命令/参数、DTO 字段）变更必配 payload **键集合精确匹配**测试（DBG-4）——存在性断言防不住字段增删漂移。
3. **mock 边界盲区认知**：mockIPC 只守 JS 侧形状——后端必填参数缺失时 invoke 必 reject 且被 `.catch` 吞掉 = 运行时静默失败、测试全绿（故障B 正是如此漏过四级测试）。契约类清单项的 verify 不得仅以 mock 层测试通过为据。
4. **静默 catch 可观测性**：修复路径上的 `catch { return [] }`/`.catch(() => {})` 至少补 `console.error`（DBG-7）——静默 catch 既是故障放大器也是定位障碍。
5. **前置条件前置化 + 辅助代码清扫**：后端有全局状态门控（sandbox root、归属校验）时，前端状态切换必须把门控设置作为**前置 await**，不能只靠 effect 兜底（DBG-5/6）；且清扫范围必须含测试辅助代码——`e2e-tests/helpers.ts` 曾复制生产代码同一时序倒置 bug（DBG-8）。

### Step 4: 划分阶段

按"**Stage 内文件不重叠**"原则划分。**每 Stage 3-15 项**——强耦合大项（跨前后端的单一任务）可 2-3 项独立成 Stage。

划分规则：
- **并行组**：改动不同文件的项 → 同一 Stage 内并行 agent（≤5 个）
- **串行组**：改动共享文件的项 → 并入同一 agent 或 pipeline 顺序执行
- **Stage 间允许重复碰同一文件**：Stage 串行执行 + 每 Stage commit，先后 Stage 改同一文件无冲突（如 spawn.rs 可先后出现在安全 Stage 与补测 Stage）
- **文档同步固定为最后一个 Stage**——文档必须反映所有代码 Stage 完成后的最终状态
- 高风险项（核心模块重构）独立 Stage 并在计划标注人工验证点
- **跨边界契约写死**：Stage 拆分跨前后端/多 agent 共享接口时（IPC 命令名+签名+参数、token 命名、数据结构字段），契约原文写死在 stages 文档与各脚本头部——两边 agent 不各自推断
- **共享常量/token 改动**：prompt 要求 agent 先查全部消费方再动笔
- **无法自动化验证的假设**（规范推断、平台行为）：代码注释记录假设来源 + stages 文档标注人工验证点，由收尾实测兜底
- **门禁命令按 Stage 触碰文件选择**：被 touch 文件不在静态检查覆盖内（如 `e2e-tests/helpers.ts` 不在根 tsconfig include）时，该 Stage 门禁须补构建级命令（`npx vite build` 打包图验证兜底）——tsc include 外的文件没有构建级门禁等于没门禁

**产出**：`docs/refactoring-stages.md`，每个 Stage 必须包含：
- 改动项 ID 列表 + agent 文件分工表（label / 负责项 / 文件，证明无文件重叠）
- 实现要点（含项目特定坑的引用）
- **验证项**：每条是 grep/Read/测试可机械检验的断言，不写"检查是否合理"；"禁止存在 X"类写语义式（"不限变量名，须 Read 代码确认"），防改名迎合。执行期断言落盘为 `verify/stage-NN.md` 独立文件（见 Step 5.4）
- commit message（前缀按变更性质：重构 `refactor:`、修复 `fix:`、文档 `docs:`）

计划文档同时写入 Step 5 执行规则与 Step 6 收尾步骤，使后续阶段可直接消费。**Step 3/4 只输出计划文档，不改任何代码。**

### Step 5: 执行阶段

主 agent 逐 Stage 串行执行，严格按顺序不跳阶段。每个 Stage 一个 Workflow 脚本：

**阶段 Workflow 结构**（见 `templates/stage-workflow.js`）：
```
Phase 1: 并行重构 → 多个 agent 同时修改互不影响的文件
Phase 2: 串行重构 → pipeline 处理有依赖的改动（可选）
Phase 3: 全量测试 → agent 执行所有测试命令
Phase 4: 逐项验证 → agent 逐条检查，返回结构化结果
```

Workflow 返回后，主 agent 检查 `verifyResult.allFixed`：
- `true` → 所有项通过 → `git commit` → 下一阶段
- `false` → 启动修复 Workflow（`templates/fix-workflow.js`），循环直到 `allFixed === true`，最多 3 轮

**执行 agent 的 prompt 必须显式声明项目禁区**——从 config.json `forbiddenZones` 读取，写入脚本 PREAMBLE。

#### 5.1 脚本生成纪律

- **逐 ID 对照 checklist 原文写脚本**，禁止凭记忆或凭 stages 摘要——凭记忆曾致 4 个 Stage 脚本与 checklist 编号/内容大面积错位
- 写完逐 ID 自检：脚本内每个 ID 的表述 ↔ checklist 条目一致
- 脚本落盘为文件（`workflows/stage-NN-*.js`），用 `Workflow({ scriptPath })` 调用，不内联——便于预检与 resume
- 首次调用前**语法预检**：Workflow 运行时允许顶层 `return`，标准 `node --check` 误判非法——用 `new Function` 包 async 函数体仅编译不执行：
  ```bash
  for f in workflows/*.js; do
    node -e 'const fs=require("fs");const src=fs.readFileSync(process.argv[1],"utf8").replace(/^export\s+const\s+meta/m,"const meta");try{new Function("agent","parallel","pipeline","phase","log","args","budget","workflow","meta",`"use strict"; return (async()=>{ ${src} \n})`);console.log("OK: "+process.argv[1])}catch(e){console.log("FAIL: "+process.argv[1]+" -- "+e.message)}' "$f"
  done
  ```

#### 5.2 Workflow 运行时语义（踩坑实录）

- `agent()` 未返回（被跳过/API 错误）时返回 `null` → 脚本必须写兜底默认值（`rawVerify ?? {...}`），否则主 agent 拿到 undefined
- `resumeFromRunId` 只对**未完成** run 有效：已正常结束的 run 全命中缓存、原样返回——verify 已结束的 no-return 靠 resume 救是死循环
- **no-return 按阶段分流**：verify 阶段 → 主 agent inline spawn 单个 verify agent（复用脚本内 prompt+schema+verify 文件+testResult）；重构/测试阶段 → `TaskStop` + `resumeFromRunId`
- `meta` 必须纯字面量（无变量/函数调用/模板插值）

#### 5.3 并行 agent 测试纪律

- 同一 Stage 并行 agent **不跑有共享资源冲突的测试**（PTY/端口/全局锁类）——重构阶段只做编译级检查（如 `cargo test --no-run`），真实执行统一由全量测试 agent 单点跑（跨进程并发同样死锁）
- cargo 系命令共享 target 目录锁，并行时排队属正常——测试 prompt 注明"勿中止"

#### 5.4 verify 体系

- 断言抽为独立 `verify/stage-NN.md`（模板 `templates/verify-file.md`）——stage 脚本与 fix-loop 共用同一真值源，修复循环与初验同一标尺
- **语义式断言**："禁止存在 X"类写语义式（"不限变量名，须 Read 代码确认"）；**正向意图断言同样写语义式**——如"panelId 必须来自 TerminalRegistry 的 Map 键，不接受字面量""不存在未前置 `await setProjectRoot` 的 `setActivePage` 调用点""文档描述须对照当前代码核实不撒谎"，防字面通过/改名迎合
- verify prompt 三件套：意图核对总则（"不仅核对字面断言，还须 Read 代码判断实现是否达成断言意图——字面通过但意图未达判 partial 并说明理由"）+ 测试结果拼入 prompt（agent 不重跑）+ no-return 兜底
- schema `required: ['allFixed', 'failedItems', 'details']`

#### 5.5 测试 agent 报告格式

每命令一行 exit code + 通过/失败；失败附**前 50 行**错误摘要，勿贴完整输出——testResult 会拼入 verify prompt，超长会撑爆上下文。

#### 5.6 主 agent 编排

- `git add` 路径限定枚举所有 Stage 可触路径（从 config.json `gitAddPaths` 读取，含 `.claude/` 精确文件——文档 Stage 会改根 CLAUDE.md），不用 `-A`；commit 前 `git status` 甄别预期外改动
- commit message 以 stages 文档各 Stage 原文为准（文档 Stage 是 `docs:` 前缀，非一律 `refactor:`）
- 时间盒：后台运行期间每 15-20 分钟查 /workflows；单 Stage 超 60 分钟无进展 → `TaskStop` + 报告用户
- 执行计划含进度跟踪表，支持全局暂停/恢复（恢复时从首个未完成 Stage 继续，其前 commit 已落盘无需重做）
- 文档同步类 agent 必须先读 `git log`/diff 再动笔，禁凭记忆写文档

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
- **workflow.gitAddPaths**: Stage commit 的 `git add` 路径限定枚举（含 `.claude/` 精确文件）
- **workflow.forbiddenZones**: 项目禁区清单（写入各脚本 PREAMBLE）

修改 `config.json` 即可适配不同的测试框架或目录结构。

## 探索维度

`explore-dimensions.json` 定义代码仓探索的维度和 Agent 提示词骨架。修改此文件可调整后续重构的探索方向和深度。

## 模板

所有 Workflow 模板在 `templates/` 目录：

| 文件 | 用途 |
|------|------|
| `stage-workflow.js` | 单阶段重构 Workflow 模板 |
| `fix-workflow.js` | 修复循环 Workflow 模板（args 参数化，强制校验） |
| `verify-schema.json` | 验证结果 JSON Schema |
| `verify-file.md` | 逐 Stage 验证断言文件模板（stage 脚本与 fix-loop 共用的唯一真值源） |

主 agent 使用时：读取模板 → 替换 `__PLACEHOLDER__` → 落盘 `workflows/stage-NN-*.js` → 语法预检（见 5.1）→ `Workflow({ scriptPath })` 调用。

### 占位符说明

| 占位符 | 替换为 |
|--------|--------|
| `__STAGE_NUMBER__` | 阶段编号 (1, 2, ...) |
| `__STAGE_DESCRIPTION__` | 阶段简短描述 |
| `__STAGE_NAME__` | Workflow 名称 (stage1-xxx) |
| `__PARALLEL_AGENTS__` | 并行 agent 数组，每个含 `label` 和 `prompt` |
| `__SEQUENTIAL_AGENTS__` | pipeline 阶段数组（无可留空） |
| `__PROJECT_ROOT__` | 项目根路径（从 config.json 读取） |
| `__TEST_COMMANDS__` | 测试命令列表（从 config.json 的 commands 读取） |
| `__VERIFY_FILE__` | 断言文件路径（如 `workflows/verify/stage-01.md`） |
| `__FORBIDDEN_ZONES__` | 项目禁区文本（从 config.json `forbiddenZones` 读取） |
| `__PREAMBLE_EXTRA__` | Stage 特殊纪律（如"只改测试"，无可留空） |
| `__CHECKLIST_FILE__` | checklist 文件路径（fix-workflow.js 用） |
| `__STAGES_FILE__` | stages 文件路径（fix-workflow.js 用） |

> 串行块转换占位符 `__SEQUENTIAL_PHASE__` / `__SEQUENTIAL_PHASE_BLOCK__` / `__SEQUENTIAL_RESULT__`：`__SEQUENTIAL_AGENTS__` 非空时按 stage-workflow.js 文末「替换说明」展开，为空时删除对应行——三者不进入上表。

## 注意事项

- 每阶段一个 commit，message 以 stages 文档原文为准（前缀按变更性质：`refactor:`/`fix:`/`docs:`）
- 直接 main 分支操作，不使用 git worktree
- 单阶段内并行 agent ≤ 5 个；**两个并行 agent 禁止改同一文件**（硬性）
- 核心重构（如模块拆分）放在独立 Stage，分配更多 agent
- 修复循环最多 3 轮，超过需人工介入
- 每个执行 agent 只改分配给自己的项，不顺手改无关代码（surgical changes）

**Red flags（踩坑实录，出现即停）：**
- 凭记忆写 workflow 脚本——必须逐 ID 对照 checklist 原文
- 用 `node --check` 预检含顶层 return 的 Workflow 脚本——误报，用 5.1 的 `new Function` 预检
- 对已正常结束的 run 用 `resumeFromRunId` 救 no-return——全命中缓存死循环；verify 阶段改 inline spawn
- `git add -A`——必须路径限定，且枚举含 `.claude/` 精确文件
- 并行 agent 各自跑资源共享型测试（如含 PTY spawn 的 `cargo test`）——跨进程并发死锁
- 跨边界契约只改一侧——后端加必填参数，前端 wrapper/调用点未同步（故障B：终端输入全静默失败）
- 以为 mock 边界的契约测试能守住真实反序列化——mockIPC 过不了后端必填校验，`.catch` 一吞测试全绿
- 修时序/契约 bug 漏扫 E2E helpers 等辅助代码——它们常与生产代码复制同一模式、同一 bug
- 门禁一刀切——tsc include 外的文件（如 `e2e-tests/helpers.ts`）没有构建级门禁等于没门禁
