# Stage 04 逐项验证断言（唯一真值源）

> stage-04 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **FE-12**：`src/lib/ConfirmDialog.tsx` 主/次按钮 `borderRadius: 6`（grep 命中）；次按钮无 `border` 声明（须 Read 确认描边已删，主按钮 border: "none" 保留）
- **FE-28**：ConfirmDialog 挂载后确认按钮获得焦点（autoFocus 或 effect 内 focus()——须 Read 确认存在）；Tab/Shift+Tab 在取消/确认两钮间循环（须 Read 确认焦点陷阱逻辑存在）；`data-e2e="confirm-ok"/"confirm-cancel"/"confirm-dialog-mask"` 三选择器原样保留（grep 命中）；confirm-dialog.test.tsx 增聚焦/Enter/Tab 循环用例且通过
- **FE-13**：`TerminalRenameDialog.tsx` 输入框 `borderRadius: 8`、两按钮 `borderRadius: 6`（grep 命中）；terminal-rename-dialog 测试通过
- **FE-14**：`SessionActionDialog.tsx` 两按钮 `borderRadius: 6`（grep 命中）
- **FE-29**：`toast.tsx` 通知容器含 `role="status"` 与 `aria-live="polite"`（grep 命中）；toast.test.tsx 增断言且通过

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
