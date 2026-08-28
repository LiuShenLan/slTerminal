# F10 编码套餐余量展示 — Stage 划分（stages）

> 配套清单：`docs/f10-plan-balance/checklist.md`（16 项六段式 + 决策记录）。
> Stage 串行执行，每 Stage 全量门禁 + verify 断言通过后 commit。

> **跨边界契约（写死，各 agent 不各自推断）**：
> - 命令：`get_plan_balance()` / `refresh_plan_balance()` 均无参；返回 `Vec<PlanBalanceInfo>`；事件 `plan-balance-updated`，payload = `PlanBalanceInfo[]`
> - DTO 键集合（camelCase）：PlanBalanceInfo = `amount/frozen/planId/sourceId/updatedAt/windows`；AmountInfo = `currency/value`；WindowsInfo = `fiveHour/sevenDay`；WindowInfo = `remainingPercent/resetsAt`
> - logo 路径：`/plan-icons/<planId>.png`；planId ∈ `deepseek`/`kimi`
> - 三处注册：lib.rs `generate_handler!` + build.rs `AppManifest::commands` + `capabilities/default.json allow-<cmd>`（D11）

## Stage 01 — 后端 plan_balance 模块（PB-BE-01~08）

**agent 分工**（单 agent 顺序执行——偏离「并行优先」的理由：模块全新建，DTO/trait 被全部文件引用，并行拆分无法各自编译验证，交接风险大于提速收益）：

| label | 负责项 | 触碰文件 |
|-------|--------|----------|
| backend | PB-BE-01~08 顺序 | src-tauri/Cargo.toml、src-tauri/src/plan_balance/{mod,source,query,deepseek,kimi}.rs（新建）、src-tauri/src/plan_balance/CLAUDE.md（新建）、src-tauri/src/lib.rs、src-tauri/build.rs、src-tauri/capabilities/default.json、src-tauri/src/settings.rs、.claude/CLAUDE.md（模块索引一行） |

**实现要点**：
- ureq 3 API 以 D10 核实为准（`Agent::config_builder().timeout_global` / `.header()` / `.call()` / `into_body().read_json()`，需 `json` feature）；`tokio` 加 `time` feature
- 命令核心参数化（poll_once_with/merge_slot 纯函数）L1 不触网不触盘；setup 启动轮询照 lib.rs:93-99 现状加一行
- token 红线：DTO 无 token 字段（serde 键集合测试锁死）；模块内 tracing/错误消息禁止插值 token 与 Authorization 头
- 每文件代码骨架在 checklist PB-BE-02~06 条目内，照抄适配，不另行设计

**验证项**：见 `docs/f10-plan-balance/workflows/verify/stage-01.md`（逐 ID 断言 + token 红线语义式断言 + serde 键集合断言 + 三处注册计数断言；中间态说明：本 Stage 后前端未实现，npm test/tsc 不受影响但门禁仍跑全量防意外）。

**人工验证点**：无（本 Stage 全自动化）。

**commit message**：`feat(plan-balance): F10 后端套餐余量模块（来源/查询注册表 + 轮询推送 + 双命令）`

## Stage 02 — 前端展示层（PB-FE-01~06）

**agent 分工**（pipeline 串行两 agent——fe-ui 依赖 fe-data 的 ipc 模块存在才能编译；文件零重叠）：

| 序 | label | 负责项 | 触碰文件 |
|----|-------|--------|----------|
| 1 | fe-data | PB-FE-01/02/03/04 | src/types/planBalance.ts（新建）、src/ipc/planBalance.ts（新建）、src/ipc/index.ts、src/__tests__/setup.ts、src/__tests__/CLAUDE.md、src/__tests__/ipc-plan-balance-contract.test.ts（新建） |
| 2 | fe-ui | PB-FE-05/06 | src/features/navTree/planBalanceModel.ts（新建）、src/features/navTree/usePlanBalance.ts（新建）、src/features/navTree/PlanBalanceFooter.tsx（新建）、src/features/navTree/NavTree.tsx、src/__tests__/plan-balance-model.test.ts（新建）、src/__tests__/plan-balance-footer.test.tsx（新建） |

**实现要点**：
- 跨边界契约以本文档头部为准；fe-ui 按契约 import（`../../ipc/planBalance` 三函数签名写死）
- footer 挂 NavTree.tsx 树区 div 与「添加项目」钮注释之间（U1）；颜色仅 DIM_FG/SEPARATOR_BG token（硬约束 #6，无新例外）；data-e2e 两值 `plan-balance-footer`/`plan-balance-row` 写死
- 代码骨架在 checklist PB-FE-02/05/06 条目内，照抄适配

**验证项**：见 `docs/f10-plan-balance/workflows/verify/stage-02.md`（逐 ID 断言 + 「footer 位于树区与添加项目钮之间」位置断言 + 「不存在硬编码颜色」语义式断言 + setup.ts mock 双登记断言 + 既有 nav-tree 用例零改动断言——`git diff --stat` 不含 nav-tree 两测试文件）。

**人工验证点**：① footer 视觉（位置/行高 28/发丝线/字号 12/logo 14px）——无法自动化，收尾实测确认；② 点击节流真实交互。

**commit message**：`feat(plan-balance): F10 前端余量 footer（DTO/ipc/纯函数/hook/组件 + L2 测试）`

## Stage 03 — 文档与登记（PB-DOC-01~04）

**agent 分工**（单 agent）：

| label | 负责项 | 触碰文件 |
|-------|--------|----------|
| docs | PB-DOC-01~04 | src/features/navTree/CLAUDE.md、src/ipc/CLAUDE.md、src-tauri/src/CLAUDE.md、.claude/test-inventory.md、（条件性）.claude/CLAUDE.md F10 行 / CONTEXT.md |

**实现要点**：
- test-inventory 计数**实跑取数**（`cargo test` 总数 + `grep -c '#\[test]'` 双核对、`npm test` Vitest 报告数；登记纪律 TQ-CI-01 三处一致：表头/段小计/行级和）
- 文档口径对照最终代码核实（防文档撒谎）；PB-DOC-04 为验证项（CONTEXT.md 四术语与 F10 索引行对照实现，漂移才修订——修订 CONTEXT.md 时需临时 `git add CONTEXT.md`，见 execution-plan）

**验证项**：见 `docs/f10-plan-balance/workflows/verify/stage-03.md`（计数三处一致断言 + 豁免行四列齐全 + 文档 grep 断言 + CONTEXT/F10 索引语义核对）。

**人工验证点**：无。

**commit message**：`docs(plan-balance): F10 模块文档与用例清单登记`

## 收尾人工实测（全部 Stage 完成后，交付前）

1. **真实账号一轮**：`~/.claude/settings.json` 配 deepseek → 构建 debug 包（`npx tauri build --debug --no-bundle`）→ 启动确认 footer 显示金额与 tooltip「上次更新」；改配 kimi → 确认双窗/重置时间；`planBalance.intervalSec` 改 10 重启确认轮询变密。
2. **视觉确认**：footer 位置（U1 树与添加项目钮之间）、发丝线、行高 28、logo 14px、hover、点击节流。
3. **断网/错误 token**：确认保留旧值静默（不炸不闪不丢行）。
4. **kimi 响应字段实证**：规格字段名（`window.duration`/`timeUnit`/`detail.resetTime`/`usage`/`totalQuota`）与真实 API 一致性——若漂移，修订 kimi.rs 解析与罐装测试。
5. **ureq HTTPS（rustls 证书）**在本机与 win10 目标机实测可达。
