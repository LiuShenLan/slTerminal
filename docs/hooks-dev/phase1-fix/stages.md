# Phase 1 review 修复 — Stage 划分（phase1-fix）

> 清单：`docs/hooks-dev/phase1-fix/checklist.md`（6 项，ID 前缀 PF-DOC）。
> 本任务为纯文档修正 + 单行代码注释，零运行时行为变更，仅 1 个 Stage（即最后 Stage，满足「文档同步固定最后 Stage」）。

## Stage 01：文档对账与行为文档化

**内容**：test-inventory 全量对账 + 两个模块 CLAUDE.md 修正 + 中断行为/信号瞬态文档化 + config.json 补登记。

**项数**：6（PF-DOC-01 ~ PF-DOC-06）

**改动文件**：
- `.claude/test-inventory.md`
- `src-tauri/src/hooks/CLAUDE.md`
- `src/panels/CLAUDE.md`
- `src/lib/claudeStatus.ts`（**仅顶部注释**，禁动任何代码）
- `.claude/skills/systematic-changes-plan/config.json`（仅 `claudeMdFiles` 追加一行）

**Agent 文件分工表**（文件零重叠）：

| label | 负责项 | 文件 |
|-------|--------|------|
| doc-inventory | PF-DOC-01 | `.claude/test-inventory.md` |
| doc-module | PF-DOC-02/03/04/05/06 | `src-tauri/src/hooks/CLAUDE.md`、`src/panels/CLAUDE.md`、`src/lib/claudeStatus.ts`（仅注释）、`.claude/skills/systematic-changes-plan/config.json` |

**实现要点**：

- **doc-inventory（PF-DOC-01）**：
  - 先读 `docs/hooks-dev/phase1-fix/checklist.md` 的 PF-DOC-01 条目与「基线数字」表；基线仅供核对，**必须按口径实查**：L1 全部在列文件逐一 `grep -c '#\[test\]'`；L2 用 `grep -cE '^\s*(it|test)\(' src/__tests__/*.test.ts src/__tests__/*.test.tsx` 求和；L3 同法（`test/terminal/*.test.ts`）。发现基线外漂移一并修正并在报告中说明。
  - hooks 两行改 8/20 + 新增 signal 9、watcher 6 两行（插入位置与现有 hooks 行相邻，L1 文件数 13→15）；spawn.rs 29→28；mod.rs 覆盖描述改如实（4 条 DTO serde + 4 条 parse 冒烟；注明全分支在 signal.rs、watcher 生命周期在 watcher.rs）。
  - L1 标题行（:9）与全量总计行（:5）按实查重算，日期更新为 2026-07-27。
- **doc-module（PF-DOC-02 ~ 06）**：
  - PF-DOC-02：hooks/CLAUDE.md 测试分布表改 8/9/6/20、总 43，覆盖描述与实查一致。
  - PF-DOC-03：无独立改动（并入留痕，验收由 PF-DOC-01/02 覆盖）——不触碰任何文件。
  - PF-DOC-04：hooks/CLAUDE.md 架构决策节补「信号文件瞬态特性 + dev 注入路径」段（三要素照 checklist PF-DOC-04）。
  - PF-DOC-05：`src/panels/CLAUDE.md`「F3 页签四态指示」节补「中断场景已知行为」段（三要素照 checklist PF-DOC-05）；`src/lib/claudeStatus.ts` 顶部注释追加假设记录（**只允许注释，禁动代码**）。
  - PF-DOC-06：`config.json` 的 `claudeMdFiles` 追加 `"src-tauri/src/hooks/CLAUDE.md"`（插入 `src-tauri/src/notify/CLAUDE.md` 附近，保持 JSON 合法）。
- **禁区**：`compute_conpty_flags` 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入。
- **历史文档不回改**（决策 4）：禁止改动 `docs/hooks-dev/phase1/` 下任何文件。

**验证项**（详细断言见 `docs/hooks-dev/phase1-fix/workflows/verify/stage-01.md`）：
1. test-inventory.md：hooks 四行 = 8/9/6/20、spawn.rs = 28、L1/全量总计 = 实查重算值；mod.rs 覆盖描述不再宣称「parse 全分支 + watcher 生命周期」。
2. hooks/CLAUDE.md：分布表 = 8/9/6/20 总 43；含瞬态特性段 + `__slterm_e2e_injectHooks`。
3. panels/CLAUDE.md F3 节：含中断行为三要素（Ctrl+C 无事件 / 下一事件自愈 / idle_prompt ~60s 转 🟡）。
4. claudeStatus.ts：仅注释变更（`git diff` 不含代码行）；注释含中断假设记录。
5. config.json：`claudeMdFiles` 含 `src-tauri/src/hooks/CLAUDE.md`，JSON 合法。
6. `docs/hooks-dev/phase1/` 零改动（`git diff --name-only` 不命中）。
7. 门禁：`npx tsc --noEmit` + `npx eslint src/lib/claudeStatus.ts` exit 0。

**门禁命令（豁免全量测试，理由）**：本 Stage 无运行时行为变更（文档 + 单行注释），不跑 `npm test` / `cargo test` / clippy——偏离 skill 默认全量测试规则的豁免理由即此。门禁 = `npx tsc --noEmit` + `npx eslint src/lib/claudeStatus.ts`。

**人工验证点**：无（全部静态可验证）。

**commit message**：`docs: Phase 1 review 修复——用例计数对账 + 中断行为/信号瞬态文档化`
