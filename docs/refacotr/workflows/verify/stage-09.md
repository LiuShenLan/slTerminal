# Stage 9 逐项验证断言（唯一真值源）

> stage-09-core-tests.js 与 fix-loop.js 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。
> 本 Stage 只新增/修改测试——`git diff` 中生产代码（`#[cfg(test)]` 块之外）应无改动。

## 断言清单

- **TE-01**：`spawn.rs` 的 `#[cfg(test)]` 模块含 env 注入（COLORTERM/TERM/TERM_PROGRAM）、cwd 反斜杠规范化、ring buffer 回放、session 移除不级联的断言；`pty_integration_tests.rs` 含 reattach 用例
- **TE-02**：`src/__tests__/use-xterm-integration.test.ts` 新文件存在，仅 mock `ipc/pty`（真实 Terminal/FitAddon）；用例覆盖 80×24 回退、`term.onData`→`pty.write`、visible 切换 WebGL 释放/重建
- **TE-03**：`title-manager.test.ts` 含 onDeletePage 用例（删除后终端编号重置、SaveAs 同名冲突）
- **TE-05**：`grep "覆盖率 0%" src/__tests__/layout-serde.test.ts` 无命中
- **TE-06**：`layout-serde.test.ts` 含嵌套 branch 修补与 activeGroup 保留断言用例
- **TE-07**：`shortcuts.test.ts` 含 addEventListener/removeEventListener spy 断言、`setOverrides(undefined)`、exportContextBindings 覆盖影响、resolve handler 返回 false 用例
- **生产代码零改动**：`git status --porcelain src/ src-tauri/src/` 中非测试文件（非 `#[cfg(test)]` 块）无变更

## 全量测试（5 条全部通过；L1/L2 用例数较基线增加）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
