# Stage 06 逐项验证断言（唯一真值源）

> stage-06 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；本 Stage 为纯历史档注记，零代码变更。

## 断言清单

- **SEC-02**：`rg "落地复核注记（review2-fix SEC-02" docs/compromises-fix-review-fix/checklist.md` 1 命中；Read 确认注记位于 L4 双 spec 行下、口径 = 实际仅执行 html.e2e.ts / markdown.e2e.ts 由 S06 全量间接覆盖、原文行保留。
- **SEC-03**：`rg "落地复核更正（review2-fix SEC-03" docs/compromises-fix-review-fix/checklist.md` 1 命中；Read 确认注记位于 catch 零命中断言行下、口径 = :182 空参 catch 存量保留实际命中 1 / 断言意图已达、原文行保留。
- **BE-04**：`rg "落地复核批注（review2-fix BE-04" docs/compromises-fix-review-fix/checklist.md` 1 命中；Read 确认注记位于 601 写死步骤行下、口径 = 实际 1201 三页链 + 理由、原文行保留。
- **FE-05**：`rg "落地复核更正（review2-fix FE-05" docs/compromises-fix-review-fix/stages.md docs/compromises-fix-review-fix/workflows/verify/stage-04.md` 各 1 命中；Read 确认两处口径 = 实际路径 src/panelRegistry.ts、原文行保留。
- **DOC-03**：`rg "落地复核更正（review2-fix DOC-03" docs/compromises-fix-review-fix/execution-plan.md` 1 命中；Read 确认口径 = 计数 8/10/11 不一 + 漏改 5 处由 review2-fix DOC-01 补齐、原文行保留。
- **DOC-04**：`rg "落地复核更正（review2-fix DOC-04" docs/compromises-fix-review-fix/workflows/verify/stage-07.md` 1 命中；Read 确认口径 = 按需预置常态不存在、断言恒假、正确口径 = 预置逻辑代码仍在、原文行保留。
- **注记形态总则（语义式）**：六条注记均为「追加」形态——逐条 Read 注记上下文确认被更正原文行逐字仍在（历史断言零改写）。

## 全量测试（全部通过为门禁；逐条串行执行，禁并行）

1. `git status --porcelain` 甄别：本 Stage 变更仅 `docs/compromises-fix-review-fix/` 下 5 文件（checklist.md / stages.md / execution-plan.md / workflows/verify/stage-04.md / workflows/verify/stage-07.md）——零代码文件变更确认
2. `npx tsc --noEmit`
3. `npx eslint src/`
