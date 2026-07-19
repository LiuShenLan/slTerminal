# Stage 01 逐项验证断言（唯一真值源）

> stage-01 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **CV-BE-01**：`src-tauri/src/git/mod.rs` 的 `git_status()` 中 `StatusOptions` 链含 `recurse_untracked_dirs(true)`（grep `recurse_untracked_dirs` 命中）；且 mod.rs tests 中存在新测试：构造含多文件的未跟踪目录，断言 statuses 逐文件出现而非目录单条目（Read 确认测试存在且语义如此）
- **CV-BE-02**：前提——grep `renames_head_to_index` 与 `renames_index_to_workdir` 均命中 `git_status` 的 `StatusOptions` 链（缺失则 renamed 状态位永不置位、oldPath 恒 null，直接判 not_fixed）；`GitStatusEntry` 结构体含 `old_path: Option<String>` 字段；status 收集循环中 renamed 条目（`INDEX_RENAMED`/`WT_RENAMED`）从 delta `old_file()` 取旧路径并按 path 同格式拼绝对路径（Read 确认）；`src/types/git.ts` 的 `GitStatusEntry` 含 `oldPath: string | null`；L1 存在 oldPath 相关测试——renamed 用例须为 `git mv` 真实场景（Read 确认非直接构造 git2::Status 状态位——直接构造测试绿但行为不存在，判 partial）+ 非 renamed 为 null + serde `"oldPath"` camelCase
- **CV-BE-03**：`src-tauri/src/git/mod.rs` 存在 `pub async fn git_file_at_head`，签名为 `(repo_path: String, file_path: String, state: State<'_, AppState>) -> Result<String, AppError>`；实现含 `validate_path_within_root` 与 `spawn_blocking`；UnbornBranch 与路径不存在统一返回消息含 `"HEAD 中不存在"` 的 `AppError::Git`（Read 确认语义，不限写法）；`src-tauri/src/lib.rs` 的 `generate_handler!` 含 `git::git_file_at_head`；L1 存在 4 类测试（读取成功 / UnbornBranch / 不在 HEAD / 子目录路径）
- **CV-BE-04**：`src/ipc/git.ts` 存在 `gitFileAtHead(repoPath, filePath)`，`invoke` 命令名为 `"git_file_at_head"`、参数 `{ repoPath, filePath }`（Read 确认）；`src/__tests__/ipc-contract.test.ts` 存在对应四维测试（命令名/参数/返回/异常）
- **语义断言**：全部新 IPC 调用只出现在 `src/ipc/git.ts`（grep `git_file_at_head` 在 `src/` 下仅命中 ipc 层与测试，组件零直接 invoke——硬约束 #1）

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
