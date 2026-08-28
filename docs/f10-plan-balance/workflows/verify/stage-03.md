# Stage 03 逐项验证断言（唯一真值源）

> stage-03-docs.js 与 fix-loop.js 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；文档类断言须对照真实代码核实，防文档撒谎。
> 中间态：本 Stage 为终态——代码已在 Stage 01/02 就绪，本 Stage 只落文档与登记。

## 断言清单

### PB-DOC-01 navTree/ipc CLAUDE.md

- **PB-DOC-01-a**：grep `plan-balance-footer` 命中 src/features/navTree/CLAUDE.md（行结构契约 footer 条目 + data-e2e 契约两处）；grep `plan-balance-row` 命中
- **PB-DOC-01-b**：grep `plan-balance-model.test.ts` / `plan-balance-footer.test.tsx` / `ipc-plan-balance-contract.test.ts` 命中 src/features/navTree/CLAUDE.md 测试模式节
- **PB-DOC-01-c**：grep `onPlanBalanceUpdated` 命中 src/ipc/CLAUDE.md；Read 确认含「planBalance 命令（F10）」段（三函数语义：挂载拉快照 / refresh 恒 Ok（D6）/ 订阅有变化才推送（D5））
- **PB-DOC-01-d**：**文档不撒谎（语义式）**——Read navTree/CLAUDE.md footer 条目与 ipc/CLAUDE.md planBalance 段，对照真实代码（src/features/navTree/PlanBalanceFooter.tsx、src/ipc/planBalance.ts）核实：位置描述（树区与添加项目钮之间）、节流 5s、行高 28、事件名、函数签名全部与代码一致；不一致判 partial

### PB-DOC-02 src-tauri CLAUDE.md

- **PB-DOC-02-a**：grep `planBalance` 命中 src-tauri/src/CLAUDE.md「settings.rs」节；Read 确认白名单口径为五键且补注 F10 轮询间隔（读取侧 resolve_poll_interval，越界回退 60s）
- **PB-DOC-02-b**：**文档不撒谎**——Read 该节对照 src-tauri/src/settings.rs:17 实际白名单与 src-tauri/src/plan_balance/mod.rs resolve_poll_interval 常量（60/10/3600），不一致判 partial

### PB-DOC-03 test-inventory 登记

- **PB-DOC-03-a**：L1 表含 plan_balance 五文件行（grep `plan_balance` 命中 .claude/test-inventory.md ≥5 处）；settings.rs 行用例数为 26；新增行覆盖要点含 F10
- **PB-DOC-03-b**：L2「IPC 层」表含 `ipc-plan-balance-contract.test.ts` 行；「导航树」表含 `plan-balance-model.test.ts` 与 `plan-balance-footer.test.tsx` 两行
- **PB-DOC-03-c**：豁免清单表新增行四列齐全——grep `plan_balance 真实 HTTP 查询` 命中；Read 确认该行含「ureq fetch」「tokio 轮询」「人工实测」「F10」要素
- **PB-DOC-03-d**：**计数三处一致（TQ-CI-01）**——表头总数行 = 各层段小计之和 = 行级和；且与实跑取数一致：L1 总数 = 测试 agent 报告的 cargo test N passed（或写明差异口径）；L2 总数 = Vitest 报告 Tests 总数；plan_balance 五文件行级用例数之和 = grep -c '#\[test]' 实计数之和（据测试 agent 统计判定，不一致判 partial）
- **PB-DOC-03-e**：计数口径行文件数更新——L1 34→39、L2 156→159（以实际新增为准；Read 该行确认）

### PB-DOC-04 CONTEXT/F10 索引核实

- **PB-DOC-04-a**：Read CONTEXT.md:240-253 四术语（编码套餐/套餐余量/用量窗口/余量来源）仍在且描述与最终实现一致（「余量来源」行为对照 src-tauri/src/plan_balance/source.rs resolve 语义）；漂移则须已修订（git diff 含 CONTEXT.md 视为已修订，核对修订内容）
- **PB-DOC-04-b**：Read .claude/CLAUDE.md F10 索引行，描述与最终实现一致（导航树底部/user 层 settings.json env 判定/后端定时查询推送）；漂移则须已修订

### 纪律断言

- **DOC-ONLY**：`git diff --stat HEAD`（对 Stage 03 未 commit 工作区）中不含任何 .rs/.ts/.tsx 文件（本 Stage 只改文档与登记；例外：无）

## 全量测试（全部通过为门禁）

1. `npm test`
2. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`

（文档 Stage 不改代码，tsc/eslint/clippy 已由 Stage 01/02 门禁锁定；本 Stage 双测试命令兼作 test-inventory 计数取数源）
