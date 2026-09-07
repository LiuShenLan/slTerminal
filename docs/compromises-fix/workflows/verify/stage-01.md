# Stage 01 逐项验证断言（唯一真值源）

> stage-01 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。
> 断言对齐 checklist.md S01 各条目「验证」段；行号为起草期定位辅助，以锚点文本为准。

## 断言清单

- **CP-025**：`test ! -e src/features/sidebar` 成立（目录物理删除）；`grep -rn "features/sidebar" src/ .claude/skills/` 零命中（docs/compromises* 与 knip.out 历史产物不计）；`src/__tests__/agent-history-restore.test.ts` 头部注释指 `features/navTree`（Read 确认）。
- **CP-026**：`test -f src/features/navTree/useAgentStatus.ts` 且 `test ! -e src/features/agentStatus`；`grep -rn "features/agentStatus" src/ knip.json` 零命中；`grep -n "currentProjectName\|AgentStatusState\|AgentStatusResult" src/features/navTree/useAgentStatus.ts` 零命中；`grep -n "60_000" src/features/navTree/NavTree.tsx` ≥1 命中；`npx knip` 无新增 issue。
- **CP-021**：`grep -n "Date.now()" src/features/navTree/NavHistoryRow.tsx` 零命中；`NavHistoryRowProps` 含必填 `now: number`（Read 确认）；`src/features/agentHistory/CLAUDE.md` MC-318 已改写为「navTree 宿主 60s ticker 驱动」口径（Read 确认，语义式：旧「时间冻结」已知行为口径不复存在）。
- **CP-038**：`grep -n '"@types/markdown-it": "\^14.2.0"' package.json` 命中；`npm ls @types/markdown-it` 退出码 0 且版本 ≥14.2.x；若走了异常分支（pin 成因浮现），则 `.claude/adr.md` ADR-0006 例外登记存在（Read 确认二选一形态）。
- **CP-027**：`npm run sync:startup-colors` 退出码 0 且输出含「无改动」（幂等）；`grep -n "#0a0a0b" index.html src-tauri/tauri.conf.json src/main.tsx` 仅前两文件各 1 命中、main.tsx 0 命中；`grep -rn "既定例外" src/main.tsx src/theme/schemes/linear.ts` 零命中；`src/theme/startupColors.ts` 存在且头注含「生成物」「勿手改」；`package.json` scripts 含 `sync:startup-colors`/`predev`/`prebuild` 三行。
- **CP-032**：`grep -n "对齐契约" e2e-tests/CLAUDE.md` 命中且七 override 成因表 7 行全列（Read 确认：serialize-javascript/deepmerge-ts/@puppeteer/browsers/glob/@wdio/globals/expect-webdriverio/webdriverio，各带成因 commit a027b17 或 1233336）；`npm ls webdriverio @wdio/globals` 单实例（退出码 0）；未走升级分支时 `git diff --stat package.json package-lock.json` 为空（HEAD 对比工作区口径，以 commit 后复跑为准则豁免此条）。
- **CP-001**：`node scripts/check-ts7-trigger.mjs` 退出码 ∈ {0,1,2} 且输出含三态之一语义文案（当前预期 1=未达成；若实跑时上游已变以退出码语义为准）；`grep -n "check-ts7-trigger" .claude/adr.md docs/compromises.md` 均命中；`src/__tests__/deps-ts7-trigger.test.ts` 存在且五用例（由全量 npm test 覆盖）。

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `npm test`
4. `npx knip --production`
5. `npx tauri build --debug --no-bundle`（CP-027 prebuild 接线端到端验证）
