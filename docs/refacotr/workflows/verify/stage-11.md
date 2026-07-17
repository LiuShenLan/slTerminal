# Stage 11 逐项验证断言（唯一真值源）

> stage-11-docs-sync.js 与 fix-loop.js 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。
> 本 Stage 只改文档——`git status --porcelain src/ src-tauri/src/` 应无代码变更。

## 断言清单

- **DOC-01**：根 `.claude/CLAUDE.md` 约束 #10 不再要求"逐条 capabilities 放行自定义命令"（改述为"Tauri 2 自定义命令默认放行，capabilities 只管插件权限"）；`src-tauri/src/pty/CLAUDE.md` 注意事项同步
- **DOC-02**：`src/features/shortcuts/CLAUDE.md` 改述 postMessage 注入脚本机制，注明 forwardGlobalShortcuts 已删除；`e2e-tests/CLAUDE.md` 同步
- **DOC-03 + TE-17**：`.claude/test-inventory.md` 已按 grep 实测重写（L1/L2/L3/L4 数字与 `grep -cE '^\s*(it|test)\('` / `#[test]` 实测一致，含 Stage 9 新增用例）；统一计数口径声明；标注 E2E 键盘局限；声明为用例数唯一真值源
- **DOC-04**：根 CLAUDE.md 无 `claude/` 模块引用；L1/L2 用例数改为引用 test-inventory.md
- **DOC-05**：`src-tauri/src/pty/CLAUDE.md` 的 JobHandle 描述与 Stage 1 后代码一致（非 Windows 零占位始终初始化）
- **DOC-06**：`src/workspace/CLAUDE.md` 不再称 Workspace.tsx "已废弃"（改述主组件职责）
- **DOC-07**：`src/stores/CLAUDE.md` 持久化规则改述为 ipc/settings（fontSize/keybindings）与 ipc/fs（projects）
- **DOC-08**：`src/ipc/CLAUDE.md` 模块映射表含 `ping()`
- **DOC-09**：`src/panels/CLAUDE.md` 无 `useFontSizeBridge` 引用；注入脚本改述三段式（CSS 注入+键盘转发+片段拦截）
- **DOC-11**：`src/__tests__/CLAUDE.md`、`src/panels/CLAUDE.md`、`src/features/shortcuts/CLAUDE.md` 用例数均改为引用 test-inventory.md（无各自维护的数字）
- **DOC-12**：`src/panels/CLAUDE.md` sandbox 决策节补录 SEC-03 修复后的 postMessage origin 校验机制与威胁模型
- **DOC-13**：`e2e-tests/CLAUDE.md` 与 DOC-02/DOC-03 结论一致
- **sandbox 一致性**：`grep -rn "allow-same-origin" **/CLAUDE.md` 描述与现行 sandbox 配置（不含 allow-same-origin）一致
- **代码零改动**：`git status --porcelain src/ src-tauri/src/` 无变更

## 全量测试（5 条全部通过——文档 Stage 零回归）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
