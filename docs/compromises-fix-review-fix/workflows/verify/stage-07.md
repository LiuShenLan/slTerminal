# Stage 07 逐项验证断言（唯一真值源）

> stage-07 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **DOC-01**：`rg "jsonSchemaCm.ts 五导出" docs/compromises.md` 零命中；`rg "七导出" docs/compromises.md` 命中 CP-002 行（Read 确认「七导出——二处生产接线，余五导出仅测试直驱」口径）。
- **DOC-02**：`.temp/node22.bak` 不存在（Glob/PowerShell Test-Path 确认）；`rg "待全量 e2e 确认后删" docs/compromises.md` 零命中；CP-003 注记含「已于全量 e2e 确认后删除（2026-09-09）」（Read 确认）；`.temp/node22` 预置通道仍在（Test-Path 确认未误删存活功能）。
- **DOC-03**：exec-doc-replay agent 报告含红测演练四步记录（命令 + exit code：改一字符 → diff 非 0 → checkout 还原 → gen+diff 归 0），且四步 exit code 与预期一致（第 2 步非 0、第 4 步 0）——Read 重构 agent 报告确认；终态 `git status` 确认 `src/panels/markdown/generated/katexInlineCss.ts` 零持久变更（测试 agent 门禁第 3 条结果承载）。
- **DOC-04**：`rg "落地复核 2026-09-09" docs/compromises-fix/workflows/verify/stage-10.md docs/compromises-fix/checklist.md` 各命中一处；Read 确认注记口径 = 宿主页 CSP meta 形态（default-src 'none'; script-src/style-src 'unsafe-inline'; img-src data:; font-src data:）且历史断言原文未改写（注记追加形态）。
- **DOC-05**：`rg "页内分屏" docs/compromises.md` 命中 CP-004 行（Read 确认消亡登记 + 2026-09-09 裁决接受不恢复口径）。
- **DOC-06**：`rg "thread_join" docs/compromises.md docs/compromises-fix/checklist.md` ≥ 2 命中；Read 确认 compromises.md CP-011 注记 = 全仓零裸 join + 守卫命令口径；compromises-fix/checklist.md:674（CP-005 验证节）与 :851（CP-011 验证节）各含落地复核注记（守卫命令与 src-tauri/src/CLAUDE.md 现行守卫命令逐字一致——交叉 Read 确认）。
- **DOC-07**：`rg "keyset" docs/compromises.md` 命中 CP-006 行（Read 确认游标根治句口径）。
- **DOC-08**：`rg "TE-05" docs/compromises.md` 命中 CP-030 行（Read 确认探针副作用 + 官方形态收窄登记）。
- **DOC-09**：`rg "721" .claude/CLAUDE.md` 零命中；`rg "串行" .claude/CLAUDE.md` 命中新增纪律条（Read 确认 L1/L2 禁并行 + CP-007 负载敏感 + 2026-09-08 实测数字口径）。
- **DOC-10**：`rg "存在性 \+ sha256 快照比对" .claude/test-exemptions.md` 零命中；Read :70 附近确认哨兵键级口径（hooks/statusLine/env 存在性+值快照比对；statusline-backup 维持 sha256、hooks/ 整树）。
- **DOC-11**：`rg "2026-09-09 复核加固" docs/compromises.md` ≥ 3 命中（CP-012/CP-022/CP-007 三行各一——Read 抽查确认 SEC-02/FE-03~05/BE-06 联动口径）。

## 主 agent 自查项（verify 后 commit 时执行，不经 verify agent）

- Stage 07 commit body 含红测演练四行输出记录（自 exec-doc-replay agent 报告抄写）。

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `node scripts/gen-katex-inline.mjs && git diff --exit-code src/panels/markdown/generated/katexInlineCss.ts`
