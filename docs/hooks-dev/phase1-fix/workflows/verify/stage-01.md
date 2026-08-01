# Stage 01 逐项验证断言（唯一真值源）

> stage-01 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **PF-DOC-01a**：`.claude/test-inventory.md` 中 hooks 模块四行用例数为：`src-tauri/src/hooks/mod.rs` = 8、`src-tauri/src/hooks/signal.rs` = 9、`src-tauri/src/hooks/watcher.rs` = 6、`src-tauri/src/hooks/inject.rs` = 20（grep 各行命中）；`src-tauri/src/pty/spawn.rs` 行 = 28。
- **PF-DOC-01b**：test-inventory.md 的 L1 总计与全量总计为实查重算值——verify agent 须自行按口径实查复核：L1 全部在列文件 `grep -c '#\[test\]'` 求和与 L1 标题行一致；L2 按 `grep -cE '^\s*(it|test)\(' src/__tests__/*.test.ts src/__tests__/*.test.tsx` 求和与 L2 标题行一致；L3 同法（`test/terminal/*.test.ts`）；全量总计 = L1+L2+L3+E2E 之和。（基线参考：L1 = 294、L2 = 1415、L3 = 116、全量 = 1842；如实查与基线不符，以实查为准，文档须等于实查值。）
- **PF-DOC-01c**：test-inventory.md 中 `hooks/mod.rs` 行的覆盖描述不再宣称其含「parse_signal_file 全分支」或「watcher 生命周期（start/stop/Drop）」（语义式：Read 该行确认描述与 mod.rs 实际内容——4 条 DTO serde + 4 条 parse 冒烟——一致）。
- **PF-DOC-02**：`src-tauri/src/hooks/CLAUDE.md` 测试分布表为 mod 8 / signal 9 / watcher 6 / inject 20、总计 43（grep 命中 8/9/6/20/43，且不残留「共 41 用例」类旧数字）。
- **PF-DOC-03**：无独立断言——本项为并入留痕项，验收由 PF-DOC-01c 与 PF-DOC-02 覆盖。
- **PF-DOC-04**：`src-tauri/src/hooks/CLAUDE.md` 含信号文件瞬态特性说明（语义式三要素，Read 确认：① 处理后即删、目录常态为空是设计行为；② dev 注入路径 `await window.__slterm_e2e_injectHooks()`；③ 状态查询/卸载 helper）+ grep `__slterm_e2e_injectHooks` 命中。
- **PF-DOC-05a**：`src/panels/CLAUDE.md`「F3 页签四态指示」节含中断场景行为段（语义式三要素，Read 确认：① Ctrl+C 中断无 hook 事件 → 页签滞留 ⚡；② 下一事件覆盖自愈；③ idle_prompt 约 60s 自动转 🟡）。
- **PF-DOC-05b**：`src/lib/claudeStatus.ts` 仅注释变更（`git diff src/lib/claudeStatus.ts` 不含任何代码行改动）；顶部注释含中断假设记录（语义式：Ctrl+C 中断无 hook 事件、working 无出边为已知行为）。
- **PF-DOC-06**：`.claude/skills/systematic-changes-plan/config.json` 的 `claudeMdFiles` 含 `"src-tauri/src/hooks/CLAUDE.md"`（grep 命中），且文件为合法 JSON（`node -e "JSON.parse(require('fs').readFileSync(...))"` 或等价验证）。
- **PF-CONST-01**：`docs/hooks-dev/phase1/` 目录零改动——`git diff --name-only HEAD -- docs/hooks-dev/phase1/` 无输出（决策 4：历史文档不回改；执行时若尚未 commit 用 `git status --short` 辅助判定）。

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/lib/claudeStatus.ts`

> 豁免说明：本 Stage 无运行时行为变更（文档 + 单行注释），不跑 `npm test` / `cargo test` / clippy（豁免理由见 `docs/hooks-dev/phase1-fix/stages.md` Stage 01 门禁段）。
