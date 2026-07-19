---
name: systematic-changes-plan
description: 当需要对代码仓做系统性审查并制定分阶段变更计划（多维度 review → 问题清单 → Stage 划分 → workflow 编排脚本），或已有 review/debug 报告需转化为分阶段执行计划，或已有需求文档需制定分阶段开发计划与 workflow 编排时使用。实际执行已有计划用 /systematic-changes-execute。
---

# 系统性变更计划 Skill（重构 / 修复 / 需求开发）

## 概述

此 skill 编码了 slTerminal 项目的系统性变更**计划**流程：**探索→汇总→清单→划分阶段→执行产物（计划文档 + workflow 脚本 + verify 断言）**。

核心思路：将变更问题按模块和依赖关系分组为多个 Stage，每个 Stage 内使用 Workflow 工具并行编排 subagent，通过"变更→全量测试→逐项验证→修复循环"确保每阶段零回归。本 skill 只产出计划与编排产物，**不改任何代码**；实际执行见 `/systematic-changes-execute`。

**产出物**：
- `docs/<task>/checklist.md`（清单）+ `stages.md`（Stage 划分）+ `execution-plan.md`（编排参数）
- `docs/workflows/stage-NN-*.js`（Stage 脚本）+ `fix-loop.js`（修复循环）+ `verify/stage-NN.md`（断言）

## 使用方式

```
/systematic-changes-plan "<变更目标描述>"
/systematic-changes-plan "<需求文档路径>"
```

**起点路由**（按输入判断模式）：
- **review 驱动**：目标是对代码仓做系统性审查 → 从 Step 1 起步；已有 review/debug 报告 → 从 Step 3 起步
- **需求驱动**：输入是需求文档 → **跳过 Step 1/2，从 Step 3 起步**（需求文档即真值源，无需探索与汇总）

也可分步调用：已完成 Step 1/2 后，要求"执行 Step 3/4 制定计划但不执行"。

## 执行流程

### Step 1: 探索（仅 review 驱动）

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

### Step 2: 生成汇总（仅 review 驱动）

全部返回后读齐各分报告，生成 `docs/review-00-summary.md`：
- 按严重程度和模块汇总
- **跨维度共性问题单独列表**——同一问题被多个维度独立发现是最高可信度信号，汇总中优先呈现
- 只写问题，不写优点

### Step 3: 制定清单

**先去重再列清单**：86 条原始发现去重合并后约 77 项是常态（同一问题被多维度报告）。合并关系在清单中留痕（哪些条并入了哪项）。

**ID 编号**：按模块前缀 + 序号（本项目用 `SEC/BE/FE/TE/DOC`），Stage 划分按 ID 引用，禁止用"第几个问题"。

**事实核验**（清单中的事实性断言必须核实，禁凭印象）：
- **既有文件路径/文件名**：Glob 实查，禁凭记忆（实证：`panel-registry.test.ts` 被 8 处误写为驼峰 `panelRegistry.test.ts`）
- **库/框架行为前提**：条目依赖库特定行为（默认配置、状态位、选项开关）时，必须 Read 源码/官方文档核实一手证据，并把前提写进条目（实证：git2 默认不做 rename 检测，`StatusOptions` 未开 `renames_*` 则 renamed 状态位永不置位，依赖它的计划全链落空）
- **行为变更的跨模块波及面**：改默认行为/选项时 grep 受影响的其他功能面（含其测试断言）写入条目（实证：`recurse_untracked_dirs` 波及 explorer 着色）
- **组织方式偏离规范时写明替代约定**（如不用 P0-P4，在清单头部说明"优先级由 Stage 依赖顺序表达"）

**必须 grill 用户确认的决策类型**（不问清不能列清单）：
- 修复范围：全量 / 高+中危 / 仅高危
- "改代码 vs 改文档"矛盾项（如约束与实际行为不符，往哪边对齐）
- 引入新功能或行为变更的项（如启用从未生效的沙箱）
- 大范围测试补写是否纳入
- 防复发/回归测试是否纳入（守护本次修复的契约守卫、时序断言等）

**产出**：`docs/<task>/checklist.md`（如 `refactoring-checklist.md`、`debug-checklist.md`，按任务性质命名）——按优先级（P0-P4）+ 模块组织的完整清单，每项含 ID、位置、修复要点。50-80 项均属正常规模。

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
- **共享集合/常量/token 改动**：消费方排查在**计划阶段**完成并列入清单——grep 断言字面量（如 `.size).toBe(`），**含测试文件**，不只留给执行 agent（实证：FILE_PANEL_TYPES 扩成员漏 `workspace-file-panel-types.test.ts`，Stage 全量测试直接红）
- **无法自动化验证的假设**（规范推断、平台行为、渲染/视觉/滚动效果）：代码注释记录假设来源 + stages 文档标注人工验证点，由收尾实测兜底。**逐项检查**——每个 Stage 过一遍"是否含此类假设"，stages 须含「人工验证点」段 + 执行收尾列人工实测项（实证：规则存在但产出漏标，diff 占位对齐/滚动同步险些漏到交付后）
- **门禁命令按 Stage 触碰文件选择**：被 touch 文件不在静态检查覆盖内（如 `e2e-tests/helpers.ts` 不在根 tsconfig include）时，该 Stage 门禁须补构建级命令（`npx vite build` 打包图验证兜底）——tsc include 外的文件没有构建级门禁等于没门禁
- **分工表文件清单 = 脚本 prompt 触碰文件全集**——含"仅加 export"类微改文件；漏列使"无文件重叠"证明失真（实证：CV-FE-03 需改 `useCodeMirror.ts` 加 export，分工表与清单均未列）
- **"现状自动正确"的隐式决策显式化**：新成员落入现有分派/映射逻辑（如 `isAlwaysRenderPanel` 自动 true）时，accept 还是改白名单必须写明决策 + 理由，禁写"确认即可"（实证：gitshow/diff 自动 always 与 editor 排除理由矛盾）
- **偏离本节规则须写明豁免理由**（实证：Stage 仅 2 项——在 stages 补一句划分理由即可，不重新划分）

**产出**：`docs/<task>/stages.md`，每个 Stage 必须包含：
- 改动项 ID 列表 + agent 文件分工表（label / 负责项 / 文件，证明无文件重叠）
- 实现要点（含项目特定坑的引用）
- **验证项**：每条是 grep/Read/测试可机械检验的断言，不写"检查是否合理"；"禁止存在 X"类写语义式（"不限变量名，须 Read 代码确认"），防改名迎合。执行期断言落盘为 `verify/stage-NN.md` 独立文件（见 5.4）
- commit message（前缀按变更性质：重构 `refactor:`、修复 `fix:`、文档 `docs:`）

**execution-plan.md 只写任务特定编排参数**（Stage 表 / commit message / git add 路径枚举 / fix-loop args 规范 / 进度跟踪表）；**通用执行规则不复制进计划文档**——单一真值源在 `/systematic-changes-execute`。**Step 3/4 只输出计划文档，不改任何代码。**

#### 计划落盘前自检清单（强制，逐条机械核对）

实证 12 条 review 缺陷的共性 = 计划缺一道事实核查关。stages 落盘后、交付执行前逐条核对：

1. 清单/stages/脚本引用的每个既有文件路径 Glob 命中（禁凭记忆）
2. 依赖库行为的条目有一手证据（源码 `file:line` 或官方文档）
3. 共享集合改动的消费方清单含测试文件（grep 断言字面量）
4. 各 Stage 分工表文件清单 = 对应脚本 prompt 触碰文件全集
5. verify 每条断言与"该 Stage 完成后的真实中间态"一致（计数/枚举类按中间态推导，不照抄终态）
6. verify 每条断言的核实数据在本 Stage 门禁命令产出内，或写明静态取数口径
7. 同一参数取值跨文件只在一处定义——execution-plan 等引用脚本头注释，不复制值（实证：fix-loop constraints 三处矛盾）
8. 无法自动化验证的假设已标人工验证点 + 收尾实测项
9. 偏离规则处已写明豁免/替代理由

#### Step 4.5: 计划评审（review）处理

收到对计划文档的 review（用户或他人产出）后：

1. **逐条行号级取证**（Grep/Glob/Read 一手证据）先判定正确/不正确——不凭 review 措辞直接改，也不凭记忆反驳
2. **正确 → 全链同步修订**：checklist / stages / 脚本 / verify 四处一并改（缺陷通常全链分布，只改一处留矛盾）；review 漏列的同模式缺陷 grep 全仓一并处理；修订后**重跑 5.1 语法预检** + grep 旧措辞零残留
3. **不正确 → 在 review 文件补充错误原因**（附证据）

### 执行产物生成纪律

Stage 脚本与 verify 文件是本 skill 的最终产物，编写时遵守以下纪律（编号沿用全流程约定，便于与存量计划文档对照）。执行期的编排规则（resume 语义 / no-return 分流 / 时间盒 / git 操作）见 `/systematic-changes-execute`。

#### 5.1 脚本生成纪律

- **执行 agent 的 prompt 必须显式声明项目禁区**——从 config.json `forbiddenZones` 读取，写入脚本 PREAMBLE
- **逐 ID 对照 checklist 原文写脚本**，禁止凭记忆或凭 stages 摘要——凭记忆曾致 4 个 Stage 脚本与 checklist 编号/内容大面积错位
- 写完逐 ID 自检：脚本内每个 ID 的表述 ↔ checklist 条目一致
- **路径约定一次写对**：脚本与 verify 文件统一落盘 `docs/workflows/`（复数），所有文档/脚本内引用一律写仓根相对全路径 `docs/workflows/...`——禁止单数 `docs/workflow/`、禁止裸相对 `workflows/`（实证：按单数落盘后统一改名，牵连 26 处 JS 引用 + 9 处计划文档引用全量替换）
- 脚本落盘为文件（`docs/workflows/stage-NN-*.js`），用 `Workflow({ scriptPath })` 调用，不内联——便于预检与 resume
- **prompt 模板字符串内的 Markdown 行内代码反引号必须转义**（`\``）——未转义反引号会终止 JS 模板字符串（实证：stage-07 脚本内未转义的 `` `#[test]` `` 报 `SyntaxError: Invalid or unexpected token`，预检首轮 7/8 抓出）
- 首次调用前**语法预检**：Workflow 运行时允许顶层 `return`，标准 `node --check` 误判非法——用 `new Function` 包 async 函数体仅编译不执行。**照抄以下命令，不凭印象重写**（转写漏步实证两次 SyntaxError：漏"剥 export"→`Unexpected token 'export'`（`new Function` 不认 ESM）；漏"包 async 函数体"→`await is only valid in async functions`）：
  ```bash
  for f in docs/workflows/*.js; do
    node -e 'const fs=require("fs");const src=fs.readFileSync(process.argv[1],"utf8").replace(/^export\s+const\s+meta/m,"const meta");try{new Function("agent","parallel","pipeline","phase","log","args","budget","workflow","meta",`"use strict"; return (async()=>{ ${src} \n})`);console.log("OK: "+process.argv[1])}catch(e){console.log("FAIL: "+process.argv[1]+" -- "+e.message)}' "$f"
  done
  ```
  PowerShell 版（本项目主 shell，实证可用）：
  ```powershell
  Get-ChildItem docs/workflows/*.js | ForEach-Object {
    node -e "const fs=require('fs');const src=fs.readFileSync(process.argv[1],'utf8').replace(/^export /gm,'');new Function('return (async()=>{'+src+'\n})');console.log('OK',process.argv[1])" $_.FullName
  }
  ```

#### 5.2 Workflow 脚本编写语义（生成期）

- `agent()` 未返回（被跳过/API 错误）时返回 `null` → 脚本必须写兜底默认值（`rawVerify ?? {...}`），否则主 agent 拿到 undefined
- `meta` 必须纯字面量（无变量/函数调用/模板插值）

#### 5.3 并行 agent 测试纪律

- 同一 Stage 并行 agent **不跑有共享资源冲突的测试**（PTY/端口/全局锁类）——重构阶段只做编译级检查（如 `cargo test --no-run`），真实执行统一由全量测试 agent 单点跑（跨进程并发同样死锁）
- cargo 系命令共享 target 目录锁，并行时排队属正常——测试 prompt 注明"勿中止"

#### 5.4 verify 体系

- 断言抽为独立 `verify/stage-NN.md`（模板 `templates/verify-file.md`）——stage 脚本与 fix-loop 共用同一真值源，修复循环与初验同一标尺
- **断言与该 Stage 完成后的真实中间态一致**——计数/枚举类断言按中间态推导，不照抄终态（实证：Stage 03 后 PANEL_TYPES 为四类型，verify 误写"五类型"，agent 无法判定）
- **断言证据可得性**：每条断言的核实数据必须在本 Stage 门禁命令产出内；不可得则收窄取数口径（如静态 grep 计数）或补门禁命令（实证：verify 要 cargo test 统计行但 Stage 07 门禁不跑 cargo）
- **语义式断言**："禁止存在 X"类写语义式（"不限变量名，须 Read 代码确认"）；**正向意图断言同样写语义式**——如"panelId 必须来自 TerminalRegistry 的 Map 键，不接受字面量""不存在未前置 `await setProjectRoot` 的 `setActivePage` 调用点""文档描述须对照当前代码核实不撒谎"，防字面通过/改名迎合
- verify prompt 三件套：意图核对总则（"不仅核对字面断言，还须 Read 代码判断实现是否达成断言意图——字面通过但意图未达判 partial 并说明理由"）+ 测试结果拼入 prompt（agent 不重跑）+ no-return 兜底
- schema `required: ['allFixed', 'failedItems', 'details']`

#### 5.5 测试 agent 报告格式

每命令一行 exit code + 通过/失败；失败附**前 50 行**错误摘要，勿贴完整输出——testResult 会拼入 verify prompt，超长会撑爆上下文。

## 配置

项目特定配置在 `config.json`（与 SKILL.md 同目录）：
- **commands**: 测试/构建/检查命令
- **modules**: 前端/后端/测试模块路径
- **architecture**: 架构约束定义
- **claudeMdFiles**: CLAUDE.md 文件清单（重构后需同步更新）
- **workflow.gitAddPaths**: Stage commit 的 `git add` 路径限定枚举（含 `.claude/` 精确文件）——计划期写入 execution-plan.md，执行期从 execution-plan 读取
- **workflow.forbiddenZones**: 项目禁区清单（写入各脚本 PREAMBLE）

修改 `config.json` 即可适配不同的测试框架或目录结构。

## 探索维度

`explore-dimensions.json` 定义代码仓探索的维度和 Agent 提示词骨架。修改此文件可调整后续重构的探索方向和深度。（仅 review 驱动模式的 Step 1 使用）

## 模板

所有 Workflow 模板在 `templates/` 目录：

| 文件 | 用途 |
|------|------|
| `stage-workflow.js` | 单阶段重构 Workflow 模板 |
| `fix-workflow.js` | 修复循环 Workflow 模板（args 参数化，强制校验） |
| `verify-schema.json` | 验证结果 JSON Schema |
| `verify-file.md` | 逐 Stage 验证断言文件模板（stage 脚本与 fix-loop 共用的唯一真值源） |

主 agent 使用时：读取模板 → 替换 `__PLACEHOLDER__` → 落盘 `docs/workflows/stage-NN-*.js` → 语法预检（见 5.1）。

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
| `__VERIFY_FILE__` | 断言文件路径（如 `docs/workflows/verify/stage-01.md`） |
| `__FORBIDDEN_ZONES__` | 项目禁区文本（从 config.json `forbiddenZones` 读取） |
| `__PREAMBLE_EXTRA__` | Stage 特殊纪律（如"只改测试"，无可留空） |
| `__CHECKLIST_FILE__` | checklist 文件路径（fix-workflow.js 用） |
| `__STAGES_FILE__` | stages 文件路径（fix-workflow.js 用） |

> 串行块转换占位符 `__SEQUENTIAL_PHASE__` / `__SEQUENTIAL_PHASE_BLOCK__` / `__SEQUENTIAL_RESULT__`：`__SEQUENTIAL_AGENTS__` 非空时按 stage-workflow.js 文末「替换说明」展开，为空时删除对应行——三者不进入上表。

## 注意事项

- 单阶段内并行 agent ≤ 5 个；**两个并行 agent 禁止改同一文件**（硬性）
- 核心重构（如模块拆分）放在独立 Stage，分配更多 agent
- **Step 3/4 只输出计划文档，不改任何代码**——代码变更一律由 `/systematic-changes-execute` 完成

**Red flags（踩坑实录，出现即停）：**
- 凭记忆写 workflow 脚本——必须逐 ID 对照 checklist 原文
- 用 `node --check` 预检含顶层 return 的 Workflow 脚本——误报，用 5.1 的 `new Function` 预检
- 凭印象重写 5.1 预检命令——bash→PowerShell 转写漏"剥 export"或"包 async"各报一种 SyntaxError（实证两次失败），照抄原文
- 脚本/verify 路径单复数或相对/全路径混用——统一 `docs/workflows/` 全路径一次写对；改名牵连全量引用替换
- 凭记忆写既有文件路径或库行为前提——路径 Glob 实查、库默认行为 Read 源码核实（实证：测试文件名 8 处误写；git2 rename 默认关闭致计划全链落空）
- 共享集合改动只查生产代码消费方——测试文件的断言字面量（`.size).toBe(`）也是消费方，漏了 Stage 全量测试直接红
- 分工表漏列"微改"文件（仅加 export 等）——文件清单 ≠ prompt 触碰全集时"无文件重叠"证明失真
- verify 断言照抄终态——计数/枚举类必须与该 Stage 完成后的中间态一致（"五类型"实证）
- verify 断言要门禁不产出的数据——收窄取数口径或补门禁命令
- "确认即可/现状自动正确"——隐式决策必须显式化并记录理由
- JS 模板字符串内写未转义反引号——终止字符串 SyntaxError（stage-07 实证，预检能稳定抓出）
- 跨边界契约只改一侧——后端加必填参数，前端 wrapper/调用点未同步（故障B：终端输入全静默失败）
- 以为 mock 边界的契约测试能守住真实反序列化——mockIPC 过不了后端必填校验，`.catch` 一吞测试全绿
- 修时序/契约 bug 漏扫 E2E helpers 等辅助代码——它们常与生产代码复制同一模式、同一 bug
- 门禁一刀切——tsc include 外的文件（如 `e2e-tests/helpers.ts`）没有构建级门禁等于没门禁
