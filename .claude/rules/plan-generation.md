---
paths:
  - "plan/**/*.md"
description: "执行计划生成规则——处理 plan/ 目录时自动加载，含编排策略决策框架与 /goal 条件写法规范"
---

# 执行计划生成规则

本文件在读取 `plan/` 路径下任何文件时自动加载。记录执行计划编排方法论、`/goal` 写法规范与决策模板。

## 1. 调研与 grill 前置

grill 技术问题前，先 spawn 3–6 个 agent 并行网络检索，汇总后交叉核验矛盾信息，再向用户提出推荐方案 + 证据链。技术选型类问题必须查当前版本/社区实践，训练数据易过时。

## 2. 编排策略决策树

按依赖类型选择结构，**不问用户、直接判断**：

```
任务有什么依赖关系？
├─ 强串行（A→B→C，B 需 A 产物）
│  └─ /goal 为主干 + 主会话顺序 spawn agent
│     workflow 仅用于旁路：版本核验、验收审计
├─ 独立文件编辑（互不冲突，仅少量屏障）
│  ├─ 屏障 1–2 个 → 1 个 dynamic workflow：屏障入 Phase → 并行扇出 → 验证 → CI
│  └─ 完全无屏障 → workflow：全部并行扇出 → 验证 → CI
└─ 混合（部分串行 + 部分并行）
   └─ /goal 锁定终态 + workflow 处理可并行部分 + 主会话处理串行部分
```

**规则**：
- 只读核验/资料检索 → workflow 并行。
- 改盘任务 → 先判定文件冲突：同文件串行、异文件并行。
- `/effort ultracode` 仅当"每个实质任务都值得 workflow"时使用；`ultracode:` 关键字单次触发。

## 3. `/goal` 条件写法

### 3.1 核心机制

Evaluator 不执行命令、不读文件、不写代码，只基于主会话中已展示的文本判断。因此：
- 声称"通过"的项必须把真实命令输出贴回主会话。
- CI 状态不能放进 `/goal`（Evaluator 看不到 GitHub），单独用 `gh run view` 贴回。
- grep/源码检查格式多变，易误判，**移出 `/goal`、放入 §5 人工验收**。

### 3.2 标准格式

```
/goal <阶段> <目标>，下列每项在本会话中均有真实命令输出自证：
(1) <可测项>：<命令> <期望输出>；
(2) …
硬约束：<不可破坏的条件，含反作弊条款>
以上未全部满足则继续；或 <N> turns 后停止并逐项汇报缺口。
```

### 3.3 条件质量检查清单

- [ ] 每项有明确的**命令 + 期望输出**（如"`cargo test` 退出 0，输出含 `test_app_error` PASS"）。
- [ ] 期望输出**可在对话中找到对应文本**。
- [ ] 硬约束含**反作弊条款**（"不得弱化断言""不得放宽 lint""不得引入业务功能"）。
- [ ] 有**止盈上限**（10–30 turns）。
- [ ] 不含 CI/外部网络依赖项。

### 3.4 好条件 vs 坏条件

| ❌ 坏条件 | ✅ 好条件 |
|----------|----------|
| "所有测试通过" | `cargo test → 退出 0，输出含 "10 passed"` |
| "CI 全绿" | /goal 不含 CI，manual §5 独立验 CI |
| "代码无问题" | `npx eslint src/ → 退出 0` |
| "feature 完成" | `npm test → 退出 0，输出含 "37 tests" 且 "0 failing"` |
| 无止盈上限 | `30 turns 后停止并逐项汇报缺口` |
| 含 grep 源码检查 | grep 移到人工验收，`/goal` 只放 exit 0 + 文本匹配 |

### 3.5 精简化

`/goal` 只放自动化命令门。社区实证（1,272 轮）：条件越多 → Evaluator 误判率越高（Silent Failure 69%、Numeric Alteration 25%、Tail Ignoring 19%）。标准八门：

`cargo test` / `npm test` / `npm run test:l3` / `npm run wdio` / `npx tsc --noEmit` / `npx eslint src/` / `cargo clippy -- -D warnings` / `cargo build --debug`。

### 3.6 与 Dynamic Workflow 结合

```
/goal 启动 → 读计划 → 基线检查 → 基线全绿 → workflow 并行实现 →
workflow 回归阶段子 agent 把 cargo test/npm test 输出贴回主会话 →
Evaluator 匹配条件 → 完成 / 修复
```

**关键约束**：
1. `/goal` 是单条命令，不是每 Turn 一个；Turn 标注是概念阶段。
2. 预写 workflow 脚本是结构约束（phase 划分、parallel 编排），Agent 可调整 prompt/重试。
3. `/goal` 必须显式引用计划文件路径，否则 Agent 可能凭记忆偏离。
4. workflow 子 agent 必须把验证命令输出贴回主会话。
5. 硬约束加"W1 必须用 Workflow 工具"，零冲突场景强制并行。
6. 基线不绿时，硬约束须写明"先修基线直到全绿再进 W1"；L4 teardown 噪声可暂搁置，spec 必须 PASS。

### 3.7 终止条件三件套

成功条件（二元可测） + 失败条件（turn 上限） + 预算条件（`+500k` token 上限）。

## 4. Workflow 编排规范

### 4.1 结构

```
Phase '<名>' [barrier|parallel]
  agent <ID> — <一句话任务>（<文件>，<模型>）
```

**规则**：
- `barrier` → 该 Phase 全部完成后才进下一 Phase。
- `parallel` → Phase 内所有 agent 同时启动。
- 同文件冲突 → 合并为一个 agent 或用 pipeline 串行。
- 复杂/首次/技术选型 → `opus`；确定性小修 → `sonnet`。
- Verification Phase → 4 agent 并行：cargo test / npm test / tauri build / npm run wdio。

### 4.2 触发方式

```
ultracode: 按以下编排执行 slTerminal <Phase> <任务描述>，只做计划内修改、不做无关改动，每阶段完成后贴命令输出回主会话
```

或 `/effort ultracode`（整会话自动编排）。推荐单次触发。

### 4.3 任务卡模板

```markdown
##### Agent <ID> — <简短描述>（Phase '<阶段>'，<模型>）

| 项目 | 内容 |
|------|------|
| **任务** | <一句话> |
| **操作** | ① … ② … ③ … |
| **产出** | <落地文件/效果> |
| **验收** | <命令> → <期望输出> |
```

## 5. 执行计划标准结构

```
1. 编排总览（为什么这样编 / 三层结构图 / 版本前提与降级）
2. 如何使用 /goal（机制要点 / 完成条件 / 运行方式）
3. 如何用 dynamic workflow（边界 / W0 前置核验 / W1 验收审计 / 何时不用）
4. 各 agent 任务卡（A1..AN，表格 + 验收命令）
5. 验收闭环（自动化 / CI 门 / 人工 / DoD）
6. 执行顺序速查
7. 注意事项
8. 决策台账
（如需补救：9. 补救执行计划·第 N 轮）
```

## 6. 已知陷阱

### 6.1 证据与核验

- **条件只测易测项会被钻空子**：硬约束必须写"不弱化断言/不放宽 lint/不引业务"。
- **证据回灌是 `/goal` 正确的命门**：每步贴命令输出，不靠 agent 自述"已完成"。
- **workflow 子 agent 输出默认不可见**：回归阶段必须贴回主会话。
- **workflow 不跨会话续**：单会话内跑完，大任务先切片估费。
- **审计标"失实"前先查决策表 + 取 L0–L2 一手证据** → 详见 `audit-review.md`。
- **审查文档/核查批注本身也可能出错**：外部引用、版本号、API 行为均需独立 agent + 一手来源交叉核验。

### 6.2 版本与外部事实

- **GitHub Closed / PR merged ≠ 修复已发布**：必须查 npm/crates.io 当前版本号。例：xterm.js #5734 Closed，但 v6.0.0 不含修复，里程碑 v7.0.0。
- **依赖版本号必须实时查询**：crates.io / npm registry API，不靠记忆。例：`tauri-plugin-prevent-default` 从记忆中的 v2.0.0 实际已到 v5.0.0。
- **PR 合并状态须经 GitHub API 确证**：例：wezterm PR #5977 实为 Open，commit `7e50c4db68` 是应用层 WSL workaround，非 portable-pty crate 修复。
- **Windows API 版本阈值以 Microsoft Learn 为唯一权威来源**：例 `ClosePseudoConsole` 非阻塞阈值 build 26100（Win11 24H2），原文 "Starting Windows 11 24H2 (build 26100) ClosePseudoConsole will return immediately"。
- **阻塞操作严重度须精确到 API 级**：pre-Win11 24H2 上 `ClosePseudoConsole` 调用 `WaitForSingleObjectEx(INFINITE)` 永久阻塞；`spawn_blocking` 512 线程池上限可能耗尽。

### 6.3 代码存在性与架构

- **"修复 X 的 Y" 前须先确认 Y 存在**：不存在时应写"创建 X 的 Y"而非"修复"。例：D5 计划写"修复 Workspace.tsx 右键菜单 addPanel"，但 Workspace.tsx 无右键菜单。
- **架构文档是执行计划的权威约束**：技术选型与架构不一致时必须标注"偏离架构基线"并给出理由。grill 阶段优先查架构文档。
- **行业参考项目的实际选择权重大于理论推演**：技术选型争议时优先查同类项目的 Cargo.toml/Cargo.lock。例：terax-ai 用 portable-pty 0.9.0 + Channel IPC。
- **源码审查权威性高于文档和搜索**：关键行为存疑时直接 `Read` node_modules 源码。例：xterm.js `_keyDown` 不检查 `hasSelection()`（CoreBrowserTerminal.ts:849-851）；Dockview `doSetActivePanel()` 内部不调 `dispose()`。

### 6.4 时序与平台行为

- **能力 ≠ 时序可用**：双方有能力不代表启动时序上自动工作。例：xterm.js 能响应 ConPTY DSR `\x1b[6n`，但启动时 IPC 回路未闭合，仍需 `openpty()` 后手动写 CPR。
- **控制字符与序列化器交互是隐藏陷阱**：C0 控制字符（如 `\x03`）被 xterm.js InputHandler 消费，绝不进 buffer 单元格；`serialize()` 从 buffer 重建，无法恢复已被 parser 消费的字节。
- **Tauri `dragDropEnabled: true` 静默劫持 HTML5 DnD**：Tauri 默认在 WebView2 宿主层注册 OLE `IDropTarget`，DOM `dragover/drop` 不到达 JS。依赖 HTML5 DnD 的组件（Dockview、React DnD）必须置 `"dragDropEnabled": false`（Tauri #14373）。
- **审查文档的 issue 引用可能跨版本/跨插件误引**：核验版本（v1 vs v2）、插件（shell vs dialog vs 核心）、修复状态、根因一致性。

### 6.5 补救计划也是计划

补救执行计划必须经过与首次计划相同的审查流程。"只是小修"不是跳过审查的理由。

## 7. 决策表模板

每个执行计划末尾必须有决策台账：

```
| # | 议题 | 决策 | 落点 |
|---|------|------|------|
| EN | <简短议题名> | <一句话决策> | <落点文件/agent> |
```

编号规则：
- 首次 Phase：A1–A6（agent）+ E1–EN（策略决策）。
- 第一轮补救：D1–DN（补救项）+ E1–EN（补救策略决策）。
- 第二轮补救：R1–RN（修补项）+ F1–FN（修补策略决策）。

**决策表审计优先**：审查结果时先查决策表再下结论。
