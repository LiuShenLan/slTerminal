# Stage 05 逐项验证断言（唯一真值源）

> stage-05 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。
> **中间态注意**：本 Stage 后命令数 = **33**（S03 后 32 + notify_stop_watch），计数断言按 33 判定。

## 断言清单

- **BE-02**：`src-tauri/src/notify/mod.rs` 存在排除目录常量（语义式核对集合 = node_modules、target、.venv、venv、dist、.git、__pycache__ 七元素，不限常量名）
- **BE-02**：事件循环存在路径分量排除过滤（语义式，须 Read 确认：任一分量命中即不 emit）；`need_rescan` 分支不被该过滤拦截（Read 确认）；新增 L1 用例存在
- **SEC-08**：事件循环存在 `symlink_metadata` 检查，命中 symlink 的路径不 emit（语义式，须 Read 确认）；新增 L1 用例存在（含 Windows symlink 特权 skip 注释）
- **BE-10**：`notify_stop_watch` 命令存在于 notify/mod.rs；`src-tauri/src/lib.rs` `generate_handler!` 恰 33 条（逐条计数）；`src-tauri/capabilities/default.json` 含 `allow-notify_stop_watch`
- **BE-10**：`src/ipc/notify.ts` 存在 `stopWatch(path: string): Promise<void>` wrapper；`src/features/explorer/ExplorerPanel.tsx` 项目移除/切换路径调用 stopWatch（语义式，须 Read 确认调用点在移除/切换流程内，非仅定义）；新增 L2 用例存在
- **BE-11**：`src-tauri/src/notify/pool.rs` 存在容量常量且值为 8（grep 命中），附理由注释（Read 确认）

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
