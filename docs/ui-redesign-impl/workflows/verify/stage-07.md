# Stage 07 逐项验证断言（唯一真值源）

> stage-07 脚本与 fix-loop 的 verify agent 均以本文件为准。

## 断言清单

- **OV-01**：`src/lib/ConfirmDialog.tsx` 与 `src/lib/toast.tsx` 存在且经 barrel 导出；Read 确认——ConfirmDialog 遮罩/描边/圆角 8/阴影/主次按钮规格均 token 引用、Promise<boolean> API；toast 三型语义色 12% 底+描边、自动消失；两者挂载于 `src/App.tsx` 根部
- **OV-02**：`grep "await ask(\|void ask(\|{ ask }\| ask," src/` 零命中；`src/ipc/dialog.ts` 无 ask 导出（open/save 保留）；原 9 处调用点全部改 confirmDialog/toast（逐文件 grep confirmDialog 命中：commitContextMenu.ts、HistorySessionList.tsx、HooksConfigPanel.tsx、useHooksConfig.ts、ExplorerPanel.tsx、FileTree.tsx）
- **OV-03**：Read `TerminalRenameDialog.tsx` 与 `SessionActionDialog.tsx` 确认浮层规范（l3 底/0.09 描边/圆角 8/阴影/按钮规格 token）
- **OV-04**：Read 4 处菜单渲染（PageDockviewHost 页签菜单、FileTree、CommitFileList、agentHistory 菜单）确认项 28px、圆角 5px、hover token、危险项 ERROR_FG（不限实现写法，须 Read 逐处确认规格值）
- **OV-05**：Read `ExplorerPanel.tsx` 错误横幅确认 token 引用 + 关闭钮 IconClose（无 × 字符残留）
- **OV-TEST**：`npm test` 通过（含 confirm-dialog/toast 新测试，依测试 agent 结果）

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
