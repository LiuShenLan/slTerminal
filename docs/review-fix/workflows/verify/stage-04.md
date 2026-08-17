# Stage 04 逐项验证断言（唯一真值源）

> stage-04 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **BE-04**：`src-tauri/src/state.rs` 的 `set_project_root` 与 `src-tauri/src/notify/mod.rs` 的 `notify_watch` 均为 `async fn`（grep 命中）；阻塞段（dunce::canonicalize / FileWatcher::start）位于 `spawn_blocking` 内（语义式，须 Read 确认）
- **SEC-14**：`set_project_root` 失败分支（canonicalize 失败/目录不可读）返回 Err 且将 `project_root` 置 None（语义式，须 Read 确认清空旧 root 的写锁赋值存在）
- **SEC-14**：新增 L1 用例存在——构造失败路径断言 Err 且 project_root 为 None（grep 测试函数）
- **FE-04**：`src/App.tsx`、`src/stores/projects.ts`、`src/workspace/Workspace.tsx` 三处 setProjectRoot 失败路径均含 `toast.show(`（grep 命中逐处列出）
- **FE-04**：三处失败路径**仍完成切换**——toast 调用后无 return/throw 阻断切换流程（语义式，须 Read 确认 DBG-9 契约未被改为阻止切换；发现阻断判 not_fixed）
- **FE-04**：`src/__tests__/workspace-switch-order.test.tsx` 含 toast 断言（grep `toast` 命中）且原 14 用例保留（全量测试绿佐证）

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
