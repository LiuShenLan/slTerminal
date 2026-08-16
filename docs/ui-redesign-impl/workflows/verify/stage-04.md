# Stage 04 逐项验证断言（唯一真值源）

> stage-04 脚本与 fix-loop 的 verify agent 均以本文件为准。

## 断言清单

- **TB-01**：grep `"decorations": false` 命中 `src-tauri/tauri.conf.json`
- **TB-02**：`src/features/titleBar/TitleBar.tsx` 存在；Read 确认——高度 34px、背景取 TITLEBAR_BG token（非硬编码 #141416）、底部发丝线 SEPARATOR_BG token、三段结构（标识/项目·页面/三钮）、三钮 38x26、关闭 hover 底 #c04747
- **TB-03**：grep `minimizeWindow\|toggleMaximizeWindow\|closeWindow` 命中 `src/ipc/window.ts`（三导出）；Read 确认 closeWindow 实现为 getCurrentWindow().close() 而非 destroy/exit
- **TB-04**：grep `data-tauri-drag-region` 命中 `TitleBar.tsx`；Read 确认三钮不在 drag region 内（或经样式排除）；双击 handler 调 toggleMaximizeWindow
- **TB-05**：grep `TitleBar` 命中 `src/App.tsx`；Read 确认 ready 后 TitleBar 位于 Workspace 之上
- **TB-06**：`src/__tests__/title-bar.test.tsx` 存在且 `npm test` 通过（依测试 agent 结果）
- **TB-07（人工验证点，verify agent 标注 skipped-manual）**：实机验证拖拽移动/双击最大化还原/三钮功能/关窗后 PTY 清理（P1-19 链路）/失去原生标题栏与阴影已接受（Win+方向键 Aero Snap 仍可用——OS 窗口管理，与 decorations:false 无关，2026-08 实机验证修订）——不纳入 allFixed 判定，但 details 中须注明此项为人工验证

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
6. `npx vite build`
