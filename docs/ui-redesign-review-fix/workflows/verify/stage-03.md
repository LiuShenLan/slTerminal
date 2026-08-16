# Stage 03 逐项验证断言（唯一真值源）

> stage-03 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **FE-04**：`src/workspace/PageDockviewHost.tsx` 的 `createGetContextMenu` 返回闭包内，`nextPanelId()` 调用仅出现在「新建终端」action 闭包中（语义式断言：菜单构建路径不再调用——须 Read 函数体确认，不接受仅移动行号）；workspace-header-actions.test.tsx 新增编号不跳用例且通过
- **FE-16**：同文件 Watermark「新建终端」按钮 `borderRadius` = 6（grep 命中）
- **FE-05**：`FileTree.tsx` 的 TreeNodeRow 不存在任何 onMouseEnter/onMouseLeave 直改 DOM style.background 的事件处理器（语义式：不限实现形态，须 Read 确认 hover 背景由 React state/props 驱动）；选中态优先语义不变（Read 确认）；explorer 相关测试增 hover state 断言且通过
- **FE-06**：`NavContextMenu.tsx` 同上（语义式，须 Read 确认）
- **FE-15**：`NavPageRow.tsx` 重命名输入框 `borderRadius` = 8（grep 命中）

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
