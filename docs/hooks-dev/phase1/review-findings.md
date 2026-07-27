# Phase 1 开发验证 — 问题与根因分析

> 范围：对照 `docs/hooks-dev/phase1/`（checklist 28 项 + stages 验证项）的静态符合性 review + 两个人工验证问题的根因。只记录问题，符合项从略。
> 日期：2026-07-27

## 一、不符合开发计划的项

### 1. [P1-TE-05 / P1-DOC-05] `.claude/test-inventory.md` 用例数与代码实际不一致（严重度：中）

- **计划要求**：Stage 06 同步 `.claude/test-inventory.md`，作为「项目用例数唯一真值源」登记 Phase 1 新增 L1/L2/L4 用例数并更新总计。
- **实际**：按文件自述口径（`grep -c '#\[test\]'`）实查，多处处数字与覆盖描述失准：

| 位置 | 清单记录 | 实际 | 差异 |
|------|---------|------|------|
| `hooks/mod.rs` | 10（.claude/test-inventory.md:24） | 8 | −2 |
| `hooks/inject.rs` | 12（.claude/test-inventory.md:25） | 20 | +8 |
| `hooks/signal.rs` | 未登记 | 9 | 缺行 |
| `hooks/watcher.rs` | 未登记 | 6 | 缺行 |
| hooks 模块 L1 小计 | 22 | 43 | −21 |
| `pty/spawn.rs` | 29（.claude/test-inventory.md:17） | 28 | −1 |

- 覆盖描述同样失准：test-inventory.md:24 称 `mod.rs` 含「parse_signal_file 全分支 + watcher 生命周期（start/stop/Drop）」，实际 `mod.rs` 只有 4 条 serde + 4 条 parse 冒烟；parse 全分支在 `signal.rs`，watcher 生命周期在 `watcher.rs`。
- 连带：L1 总计（274）与全量总计（1822）需按上表重算。
- **证据**：`.claude/test-inventory.md:17,24-25`；`grep -c '#\[test\]'` 实查 `src-tauri/src/hooks/{mod,signal,watcher,inject}.rs` = 8/9/6/20，`src-tauri/src/pty/spawn.rs` = 28。
- **修复方向**：按实际计数修正两行 + 补 signal/watcher 两行 + 修正 mod.rs 覆盖描述 + 重算 L1/全量总计。

### 2. [P1-DOC-05 连带] `src-tauri/src/hooks/CLAUDE.md` 测试分布表失准（严重度：中）

- **计划要求**：Stage 06 新建 hooks 模块 CLAUDE.md，测试模式表反映真实分布。
- **实际**：表中称 mod 8 / signal 10 / watcher 4 / inject 19（总 41），实际为 8 / 9 / 6 / 20（总 43）。
- **证据**：`src-tauri/src/hooks/CLAUDE.md`「测试模式」分布表；实查计数同上。
- **修复方向**：与 #1 一并按实际计数修正。

### 3. [P1-BE-08] `hooks/mod.rs` 测试位置与计划不一致（严重度：低）

- **计划要求**：`mod.rs` 的 `#[cfg(test)]` 覆盖 DTO serde、`parse_signal_file` 全分支（合法/缺 panelId/非法 JSON/空串）及 watcher 生命周期（start/stop/Drop）。
- **实际**：`mod.rs` 仅 4 条 serde + 4 条 parse 冒烟（缺空 panelId、仅空白等分支）；完整 parse 分支在 `signal.rs`，watcher 生命周期在 `watcher.rs`（手动构造模式）。
- **证据**：`src-tauri/src/hooks/mod.rs:80-166`；`src-tauri/src/hooks/signal.rs:81-175`；`src-tauri/src/hooks/watcher.rs:120-182`。
- **说明**：功能覆盖无缺失（各分支在兄弟文件补齐），仅位置与计划描述不符，并导致 #1 的统计口径混乱。
- **修复方向**：以现状为准修正计划描述与 inventory 覆盖列即可，无需移动测试。

## 二、人工验证问题根因

### 问题 1：Ctrl+C 中断后页签卡在 ⚡ 而非 ✅

**现象**：claude 处理 prompt 中按 Ctrl+C，处理停止并返回输入框（符合预期），但页签仍显示 ⚡；继续使用后图标自恢复正常流转（用户确认）。

**根因**：三层叠加，核心是第一层——

1. **Claude Code 在用户主动中断时不发射任何 hook 事件**。已核对的事件语义：`Stop` = 主代理**完成响应输出**；`StopFailure` = 轮次因 **API 错误**结束（`docs/hooks/D1/01-hooks-official-docs.md:36-37`）。Ctrl+C 中断既不是正常完成也不是 API 错误，无任何事件产生——用户实测「卡住不变」即为佐证（若 Stop 发射，状态会转 ✅）。
2. **四态状态机 `working` 无中断出边**。`eventToStatus` 无任何中断类事件映射（`src/lib/claudeStatus.ts:28-62`）；`useXterm` 的 hook-event 处理仅在 `SessionEnd` 时清图标，无超时/兜底机制（`src/panels/terminal/useXterm.ts:348-357`）。无事件 → 状态滞留 ⚡，直到下一事件（UserPromptSubmit/Stop 等）覆盖——与「后续自恢复」现象一致。
3. **功能规划缺口**。`docs/hooks-dev/feature-plan/` 全文无 Ctrl+C/中断/abort 语义规定（grep 无命中），阶段 1 计划亦未覆盖该场景。

**性质**：规划缺口 + 实现无兜底，**非实现 bug**（实现与阶段 1 计划一致）。

**修复方向**（候选，拍板留 fix 阶段）：
- a) 前端为 `working` 加超时衰减（最后事件后 N 秒无新事件 → 降级/清除）——注意长工具调用期间同样无事件，存在误判风险；
- b) 接受「下一事件自愈」现状，在文档中注明中断场景的已知行为；
- c) 跟踪 Claude Code 官方是否补中断类 hook 事件，再做事件驱动修复。

### 问题 2：`~/.slterminal/hooks-events/` 测试期间无任何信号文件

**现象**：人工验证全程观察该目录，始终为空。

**根因**：非缺陷，系设计行为 + 验证指引缺口——

1. **信号文件「即取即删」是契约设计**（C2 备选 A，防目录无限膨胀）：`process_signal_file` 处理后无论 emit 成败均 `fs::remove_file`（`src-tauri/src/hooks/signal.rs:49-79`）；watcher debounce 仅 50ms（`src-tauri/src/hooks/watcher.rs:37`）。文件从产生到删除存活亚秒级，任何时刻 `ls` 几乎都看不到文件。
2. **验证期间信号通道实际通畅**：⚡ 只能由 hook 事件驱动（`eventToStatus`：UserPromptSubmit/PreToolUse/PostToolUse → working，`src/lib/claudeStatus.ts:35-38`；OSC 133 C 仅设 🟡）。⚡ 出现即证明文件已产生并被 watcher 消费删除——目录为空恰是管道正常工作的表现。（用户已确认：验证时注入过 hooks，验证后还原了 settings.json。）
3. **验证指引缺口**：阶段 1 人工验证点（`docs/hooks-dev/phase1/stages.md:70-72,194-196,240-241`）未说明信号文件的瞬态特性，也未写明注入的操作路径——前端生产代码无 `inject()` 调用方（计划内如此，F2 入口并入阶段 3），唯一注入入口是 dev/E2E 构建下的 `window.__slterm_e2e_injectHooks()`（`e2e-tests/helpers.ts:296-300`），文档未指明。

**修复方向**：
- 在阶段 1 人工验证点或 `src-tauri/src/hooks/CLAUDE.md` 注明信号文件「即取即删、目录常态为空」的瞬态特性；
- 写明 dev 环境注入路径：devtools 控制台执行 `await window.__slterm_e2e_injectHooks()`；如需观察信号文件，可用文件系统监视工具（而非 `ls`）或临时停 watcher。
