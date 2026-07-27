# Phase 1 review 修复清单（phase1-fix）

> 输入：`docs/hooks-dev/phase1/review-findings.md`（真值源，3 项不符合 + 2 项根因修复方向）。
> **组织约定**：本清单仅 6 项且全部同级，不使用 P0-P4——优先级由 Stage 依赖顺序表达（本任务仅 1 个 Stage）。
> **决策依据**（2026-07-27 用户拍板）：
> 1. 问题 1（Ctrl+C 卡 ⚡）→ **文档化**（接受「下一事件自愈」现状；依据 `docs/hooks/D1/01-hooks-official-docs.md:163`：idle_prompt 在中断回提示符约 60s 后发射 → 自动转 🟡，已有内置衰减）
> 2. 修复范围 → 全量（3 不符合项 + 2 根因修复方向）
> 3. 计数漂移防复发 → 不纳入守卫脚本，仅 verify 加实查复核断言
> 4. 历史计划文档 → 不回改 `docs/hooks-dev/phase1/`；只改长期真值源（模块 CLAUDE.md）

## 基线数字（执行 agent 须按口径实查复核，禁照抄）

| 项 | 基线值 | 取数口径 |
|----|--------|---------|
| `hooks/mod.rs` `#[test]` 数 | 8 | `grep -c '#\[test\]'`（test-inventory.md:7 自述口径） |
| `hooks/signal.rs` | 9 | 同上 |
| `hooks/watcher.rs` | 6 | 同上 |
| `hooks/inject.rs` | 20 | 同上 |
| hooks 模块小计 | 43 | 上四行之和 |
| `pty/spawn.rs` | 28 | 同上 |
| L1 总计 | 274 → 约 294 | 274 − 22（旧 hooks 两行）− 29（旧 spawn）+ 43 + 28 |
| 全量总计 | 1822 → 约 1842 | L1 294 + L2 1415 + L3 116 + E2E 17（L2/L3 须实查复核） |

## 清单

### PF-DOC-01 `.claude/test-inventory.md` 用例数对账

- **位置**：`.claude/test-inventory.md:5`（全量总计行）、`:9`（L1 标题行）、`:17`（spawn.rs 行）、`:24-25`（hooks 两行）
- **问题**（review #1）：hooks L1 实 43 记 22（mod 8 记 10、inject 20 记 12、signal 9 与 watcher 6 未登记）；spawn.rs 实 28 记 29；mod.rs 覆盖描述宣称「parse_signal_file 全分支 + watcher 生命周期（start/stop/Drop）」与实际不符（实际仅 4 条 serde + 4 条 parse 冒烟）。
- **修复要点**：
  1. `hooks/mod.rs` 行：用例 10 → **8**；覆盖描述改如实（DTO serde camelCase + parse_signal_file 冒烟分支；全分支在 signal.rs、watcher 生命周期在 watcher.rs——可一句话注明分布）
  2. `hooks/inject.rs` 行：用例 12 → **20**；覆盖描述按实际核对修正
  3. 新增 `hooks/signal.rs` 行：**9**（parse_signal_file 全分支 + serde 往返）
  4. 新增 `hooks/watcher.rs` 行：**6**（is_signal_file ×4 + watcher 生命周期 ×2）
  5. `pty/spawn.rs` 行：29 → **28**
  6. L1 标题行（:9）文件数与总计按全量实查重算（基线：13 文件 / 294 用例——新增两行后文件数 15）
  7. 全量总计行（:5）按 L1/L2/L3 实查重算（基线约 1842）；同步更新行内日期
  8. **全量实查义务**：L1 全部在列文件逐一 `grep -c '#\[test\]'` 实查对账（不止修已知漂移行）；L2 按口径 `grep -cE '^\s*(it|test)\(' src/__tests__/*.test.ts src/__tests__/*.test.tsx` 求和实查 1415；L3 同法实查 116（`test/terminal/*.test.ts`）。发现新漂移一并修正并在报告中说明。E2E 17 不在本次实查范围
- **证据**：review-findings.md #1；`grep -c '#\[test\]'` 实查 8/9/6/20/28

### PF-DOC-02 `src-tauri/src/hooks/CLAUDE.md` 测试分布表修正

- **位置**：`src-tauri/src/hooks/CLAUDE.md`「测试模式」节分布表（称 mod 8 / signal 10 / watcher 4 / inject 19，总 41）
- **问题**（review #2）：实际为 8 / 9 / 6 / 20（总 43），表内数字与「共 41 用例」表述均失准。
- **修复要点**：分布表四行改 **8 / 9 / 6 / 20**，总计改 **43**；各行覆盖描述与实查一致。
- **证据**：review-findings.md #2；实查计数同上

### PF-DOC-03 P1-BE-08 对齐（并入项，无独立改动）

- **问题**（review #3）：`hooks/mod.rs` 测试位置与 phase1 计划描述不一致（parse 全分支在 signal.rs、watcher 生命周期在 watcher.rs）。
- **决策**：不移动测试、不回改 phase1 历史文档（决策 4）。
- **并入留痕**：其「inventory 覆盖列失准」并入 **PF-DOC-01** 第 1 条修正；「分布如实描述」并入 **PF-DOC-02**。本项无独立文件改动，验收由 PF-DOC-01/02 的断言覆盖。

### PF-DOC-04 信号文件瞬态特性 + dev 注入路径写入 hooks/CLAUDE.md

- **位置**：`src-tauri/src/hooks/CLAUDE.md`（架构决策节）
- **问题**（review 问题 2）：人工验证者不知信号文件「即取即删」、目录常态为空；也不知 dev 环境注入路径（前端生产代码无 `inject()` 调用方，唯一入口为 dev/E2E 构建的 E2E helper）。
- **修复要点**：在 hooks/CLAUDE.md 补一段说明，含三要素：
  1. 信号文件由 `process_signal_file` 处理后即删（`signal.rs:49-79`），debounce 50ms → 目录常态为空是设计行为，观察需用文件系统监视工具
  2. dev 环境注入路径：`npm run tauri dev` 启动后 devtools 控制台执行 `await window.__slterm_e2e_injectHooks()`（`e2e-tests/helpers.ts:296-300`）
  3. 注入状态查询/卸载：`__slterm_e2e_getHookInjectionStatus()` / `__slterm_e2e_uninstallHooks()`
- **证据**：review-findings.md 问题 2；`src-tauri/src/hooks/signal.rs:49-79`；`e2e-tests/helpers.ts:296-300`

### PF-DOC-05 Ctrl+C 中断场景已知行为文档化

- **位置**：`src/panels/CLAUDE.md`「F3 页签四态指示（hook-event + emoji）」节；`src/lib/claudeStatus.ts` 顶部注释
- **问题**（review 问题 1）：Ctrl+C 中断时 Claude Code 不发射任何 hook 事件（Stop=完成响应、StopFailure=API 错误，`docs/hooks/D1/01-hooks-official-docs.md:36-37`），状态机 `working` 无出边 → ⚡ 滞留。功能规划未规定中断语义，实现与计划一致（非 bug）。
- **修复要点**：
  1. `src/panels/CLAUDE.md` F3 节补「中断场景已知行为」段，含三要素：① Ctrl+C 中断无 hook 事件 → 页签滞留 ⚡；② 下一事件（UserPromptSubmit/Stop 等）覆盖自愈；③ 中断回提示符约 60s 无操作 → `idle_prompt` Notification → 自动转 🟡（`docs/hooks/D1/01-hooks-official-docs.md:163`）
  2. `src/lib/claudeStatus.ts` 顶部注释追加假设记录（skill 规则：无法自动化验证的假设注释留痕）——Ctrl+C 中断无 hook 事件，`working` 无出边为已知行为，依赖下一事件覆盖或 idle_prompt 衰减
- **证据**：review-findings.md 问题 1；`src/lib/claudeStatus.ts:28-62`；`src/panels/terminal/useXterm.ts:348-357`

### PF-DOC-06（计划期新发现）`config.json` 的 `claudeMdFiles` 漏登记 hooks/CLAUDE.md

- **位置**：`.claude/skills/systematic-changes-plan/config.json:42-58`
- **问题**：phase1 Stage 06 验证项 3 要求「`config.json` 的 `claudeMdFiles` 清单包含 `src-tauri/src/hooks/CLAUDE.md`（新增）」（`docs/hooks-dev/phase1/stages.md:277`），实际未加入——计划期 Glob/Read 实查发现，review 报告未列。
- **修复要点**：`claudeMdFiles` 数组追加 `"src-tauri/src/hooks/CLAUDE.md"`（按现有排序惯例插入 `src-tauri/src/notify/CLAUDE.md` 附近）。
- **证据**：`docs/hooks-dev/phase1/stages.md:277`；`config.json:42-58`（无该条目）
