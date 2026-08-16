# Stage 09 逐项验证断言（唯一真值源）

> stage-09 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。
> Stage 特殊纪律：只改文档与注释——`git diff` 确认逻辑代码零变更（App.css:9 注释与 linear.ts 文件头注释允许）。
> 文档类断言：须对照当前代码/文件核实，防文档撒谎。

## 断言清单

- **SPEC-01**：`docs/ui-redesign/requirements.md` UI-204 行与 `docs/ui-redesign/design.md` 字号阶梯表含「内容区默认 14px」例外登记（grep 命中）；`src/App.css:9` 注释含内容区例外注记；代码侧 `fontSize.ts:17` / `terminal/theme.ts:12` / `useCodeMirror.ts:139,296` / `main.tsx:30` 保持 14 未动（git diff 零命中）
- **SPEC-02**：requirements.md UI-305 行与 design.md 含「文件树（explorer）行 24px」分档登记（grep 命中）；`explorer/CLAUDE.md` 文件表 FileTree 行含行高 24 档登记；`FileTree.tsx:202` 保持 24 未动
- **DOC-01**：`.claude/test-inventory.md` 无 history.e2e.ts 的 dialogAsk ⚠️ 警告段（grep `dialogAsk` 零命中）；覆盖描述与 `e2e-tests/history.e2e.ts:587-619` 现状（confirm-ok 点击）一致
- **DOC-02**：`e2e-tests/CLAUDE.md` 写 history.e2e.ts 为「7 条 active」（对照 history.e2e.ts 实际 it 块数 = 7 核实）；无「孤儿行 ✗」条目；`waitForPanelTabIcon` 字样零命中
- **DOC-03**：`src/stores/CLAUDE.md` 侧栏默认态描述 = nav/explorer/commit 三槽（对照 `sideBarState.ts` DEFAULT_ZONES 核实）；无 projects/agent-status 四槽描述
- **DOC-04**：`src/features/shortcuts/CLAUDE.md` hooks 入口描述 = 活动栏底部配置钮 → openHooksConfigFromActivityBar；无「侧栏右键菜单」入口描述
- **DOC-05**：`src/ipc/CLAUDE.md` window.ts 描述为七个 wrapper（对照 src/ipc/window.ts 实际导出数 = 7 核实）
- **DOC-06**：`src/panels/CLAUDE.md` index.ts 文件表含 HooksConfigPanel（对照 src/panels/index.ts 导出核实）
- **DOC-07**：`src/theme/CLAUDE.md` 与 `src/theme/schemes/linear.ts` 文件头的 tauri.conf.json 行号引用 = :21（对照 tauri.conf.json 实际行号核实）
- **DOC-08**：`src/workspace/CLAUDE.md` index.ts 文件表导出项与 `src/workspace/index.ts` 实际导出一致；pageApis 行含 findPanelForSession/findPageIdForPanelId；F8 段含 view.contentComponent 公开类型成员注记（VER-01）
- **DOC-09**：根 `.claude/CLAUDE.md` 硬约束 #6 例外指向不再单独指向 src/panels/CLAUDE.md（grep 确认），改汇总指向 theme/explorer/navTree 三处登记；模块索引 agentHistory 行无「HistorySessionList/Row 供导航树历史区」现役表述
- **DOC-10**：`agentHistory/CLAUDE.md` 无 HistorySessionList/Row 现役式描述（历史注记除外）；`navTree/CLAUDE.md` 删除项目描述 = confirmDialog（无 window.confirm 表述）、反查登记指向 workspace/pageApis；`src/lib/CLAUDE.md` 无 HistorySessionList 提及
- **DOC-11**：`.claude/test-inventory.md` 含本修复新增/迁移/删除用例登记（抽查 Stage 01 pageapis 新用例、Stage 04 confirm-dialog 焦点用例、Stage 07 迁移用例、Stage 08 TE-03 更名在册）
- **Stage 09 纪律**：`git diff` 中逻辑代码零变更（仅 docs/、CLAUDE.md、test-inventory、App.css/linear.ts 注释）

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
