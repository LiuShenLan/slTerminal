# Stage 01 逐项验证断言（唯一真值源）

> stage-01 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **BE-01**：`src-tauri/src/app_dir.rs` 含 `SLTERM_DATA_DIR` 常量定义，且 `app_data_dir()` 内存在 `std::env::var_os` 分支返回该目录（空串被 `.filter` 忽略）；优先级顺序须 Read 确认：测试 guard（`#[cfg(test)]` 块）在前、env 分支居中、`resolve_app_data_dir(std::env::current_exe())` 兜底在最后。
- **BE-01**：`grep -rn "app_data_dir()" src-tauri/src --include="*.rs"` 消费方仅 settings.rs / projects.rs / plan_balance/mod.rs（无新增未知消费方）。
- **BE-02**：`grep -c "tracing::" src-tauri/src/projects.rs` ≥ 3；且打点为纯日志（Read 确认不改变 `load_from_dir` 四分支返回语义——.bak 双保险仍在）。
- **FE-01**：`grep -c "loadSucceeded" src/stores/projects.ts` ≥ 5；`export function markLoadSucceeded` 存在；`_resetPersistence` 内重置 `loadSucceeded = false`。
- **FE-01**：`loadFromDisk` 不含 try/catch 吞异常（Read 确认异常上抛）；`JSON.parse` 后存在 projects 字段结构校验 throw（语义：projects 存在但非对象即抛错）。
- **FE-01**：`saveToDisk` 首行后存在空写守卫（语义：`!loadSucceeded && projects 为空` 时 console.warn + return 不写盘，须 Read 确认守卫位置在写盘调用之前）。
- **FE-01**：`loadAllProjects` 不含 catch（Read 确认直传 `loadFromDisk()`）。
- **SEC-18**：`git grep -nE "sk-[A-Za-z0-9_-]{16,}"` 零命中。

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
5. `npm test`
6. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
