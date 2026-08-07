# Stage 06 逐项验证断言（唯一真值源）

> 中间态口径：本 Stage 仅改文档；任何代码文件（src/、src-tauri/、e2e-tests/、test/）零改动。
> 取数口径：本 Stage 无编译门禁，断言全部由 Grep/Read 静态核实 + markdown 语法抽查承担。
> 修正记录（执行期，Stage 06 首验）：断言 9 允许清单扩展 3 项——`src/lib/CLAUDE.md`（checklist 修正记录 4 连带：E2E_ENABLED 六站点→五站点 + main.tsx 内联门控说明，execution-plan.md Stage 06 清单已登记含此文件）、`docs/color-plan/execution-plan.md`（进度簿记）、`docs/color-plan/workflows/stage-06-docs.js`（A2 任务追加，修正记录 4 正当扩展，同 CONTEXT.md/adr.md 先例）。

## 断言清单

| # | 对应项 | 断言 | 检验方法 |
|---|--------|------|---------|
| 1 | DOC-01 | `CONTEXT.md` 含 4 新术语条目：配色 token、配色方案、方案注册表、启动链 fail-safe 色 | Grep 四词 |
| 2 | DOC-02 | `.claude/adr.md` 含 `ADR-0002` 条目，内容涉及配色方案系统（schemes/SchemeRegistry/facade） | Grep + Read |
| 3 | DOC-03 | 根 `.claude/CLAUDE.md` 无旧 #6 措辞「所有颜色只在 `theme/colors.ts` 定义为 token」；新 #6 措辞含 `theme/schemes` 与 facade 语义 | Grep 旧措辞零命中 + Read 新措辞 |
| 4 | DOC-03 | 根 `.claude/CLAUDE.md` 模块索引 src/theme 行职责已更新（含「方案」语义） | Read |
| 5 | DOC-04 | `src/theme/CLAUDE.md` 无「既定例外」「无独立测试文件」两过时句；含 schemes/Registry/facade/overrides 四件描述与新增方案步骤 | Grep 两旧句零命中 + Read |
| 6 | DOC-05 | `src/panels/CLAUDE.md` 硬约束 #6 例外句改为 adapter 表述（含「adapter」或「映射 active 方案」语义）；其余内容无 diff | Grep + git diff 行数核对 |
| 7 | DOC-06 | `docs/color-implementation.md` 与 `docs/color-inventory.md` 无「临时摸底」注记；含新架构现状描述（schemes/facade） | Grep + Read |
| 8 | 诚实性 | 文档描述与当前代码一致——语义式：抽查 theme/CLAUDE.md 文件表所列文件（schemes/3 文件 + schemeRegistry.ts + overrides.ts）全部真实存在；抽查根 CLAUDE.md 新 #6 措辞与 colors.ts 现状（facade）不矛盾 | Glob + Read 抽查 |
| 9 | 中间态 | 本 Stage diff 仅含七文档 + 修正记录 4 扩展三项：CONTEXT.md、.claude/adr.md、.claude/CLAUDE.md、src/theme/CLAUDE.md、src/panels/CLAUDE.md、src/lib/CLAUDE.md、docs/color-implementation.md、docs/color-inventory.md、docs/color-plan/execution-plan.md、docs/color-plan/workflows/stage-06-docs.js；任何代码文件零改动 | git diff --name-only HEAD |
| 10 | 语法 | 改动文档 markdown 语法抽查：标题层级连续、表格行列闭合、无未闭合代码围栏 | Read 抽查 |

## 全量测试（全部通过为门禁）

- 无编译门禁（纯文档 Stage）——断言 1-10 全部通过即门禁
