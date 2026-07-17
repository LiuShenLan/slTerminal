# Stage 4 逐项验证断言（唯一真值源）

> stage-04-backend-modules.js 与 fix-loop.js 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **SEC-04**：`fs_rename` 目标为已存在目录返回 Err（无 `remove_dir_all` 静默删除路径）；目标为已存在文件先 `remove_file` 再 rename；两条路径各有单元测试
- **BE-03**：`fs_write_file` 不再 `std::fs::read` 全量读，仅 `File::open` + 读前 `CRLF_SAMPLE_MAX_BYTES` 字节样本
- **SEC-05**：`git_status`/`git_diff` 入口有 `validate_path_within_root`；`Repository::discover` 结果 workdir 同样校验
- **BE-06**：`get_or_open_repo` 无 `workdir.starts_with(&search)` 分支；有"父目录调用不命中子仓库缓存"测试
- **BE-15**：`notify_watch` 中 `FileWatcher::start` 在池锁外完成，短暂持锁仅插入
- **BE-05**：`save_settings` 的 `create_dir_all` 在 `spawn_blocking` 闭包内
- **TE-13**：`grep "compute_diff_hunks" src-tauri/src/git/mod.rs` 仅剩 1 处（生产纯函数），测试直调（副本已删）
- **TE-14**：fs/settings 命令包装层有新增单测（参数透传、错误映射、sandbox 校验分支）

## 全量测试（5 条全部通过，含 git 61+ 条）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
