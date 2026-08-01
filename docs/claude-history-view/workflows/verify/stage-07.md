# Stage 07 逐项验证断言（唯一真值源）

> stage-07 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **DOC-01**：`docs/claude-history-view/README.md` 版本头含 `v1.1`（grep 命中）；决策记录表含第 22–26 行（grep `| 22 |`～`| 26 |` 命中）；决策 10 行标注已被决策 22 推翻（Read 确认）；4.2 标题回退链含 custom-title 首优先级（Read 确认）；4.4 重命名行写 custom-title（Read 确认）；4.3.2 步骤 4 为 pty.write 注入表述（Read 确认）。
- **DOC-02**：`src-tauri/src/claude_history/CLAUDE.md` 存在且含四段：职责 / 架构决策 / 文件表 / 测试模式（Read 确认标题）；架构决策含 env 覆盖「生产不设置」标注（Read 确认）；`.claude/CLAUDE.md` 模块索引含 `claude_history` 行（grep 命中）。
- **DOC-03**：`src/features/claudeHistory/CLAUDE.md` 存在且四段齐全（Read 确认）；含操作矩阵与 ⚡ 派生局限说明（Read 确认）；`.claude/CLAUDE.md` 模块索引含 `claudeHistory` 行（grep 命中）；`src/ipc/CLAUDE.md` 模块映射表含 `claudeHistory.ts` 行且三命令名正确（grep `claude_history_scan` 命中）。
- **DOC-04**：`.claude/CLAUDE.md` 需求编号索引含 `F7` 行（grep `| F7 |` 命中），语义为 claude 历史会话查询与恢复（Read 确认）。
- **DOC-05**：`src/features/sideViews/CLAUDE.md` 的 `AgentStatusView.tsx` 行描述含三下拉框/历史会话语义（Read 确认该行已更新，非旧「渲染 Agent 会话状态列表」原文）。
- **DOC-06**：`.claude/test-inventory.md` 含 claude_history（L1）与 claude-history-*（L2）条目；计数与实跑输出一致（取数口径：Stage 07 全量测试中 `cargo test ... claude_history` 与 `npm test` 的 claude-history 文件计数，静态 grep 测试文件 `#[test]`/`it(` 计数比对）。
- **文档不撒谎（语义式）**：两份新 CLAUDE.md 与 README 修订处引用的文件路径、命令名、函数名，逐一 grep 命中真实代码（如 `claude_history_scan`、`resolve_projects_root`、`restoreHistorySession`、`HistorySessionRow`）；引用的「先例」描述（hooks/usage.rs 尾部扫描、commitContextMenu 策略模式等）Read 对照属实。
- **禁区**：`git diff` 本 Stage 不含 `src-tauri/src/pty/` 下任何文件改动；不含 `src/`、`src-tauri/src/` 下任何 `.ts/.tsx/.rs` 代码文件改动（纯文档 Stage）。

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
