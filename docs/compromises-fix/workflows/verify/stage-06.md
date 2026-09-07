# Stage 06 逐项验证断言（唯一真值源）

> stage-06 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。
> Stage 特殊纪律（fix-loop constraints）：CP-007 的删/修分支由 benchmark 实测数字决定，不两个分支同时做。

## 断言清单

- **CP-006-a（契约形态）**：`src-tauri/src/fs/mod.rs` 含 `FsReadDirPage`（entries + next_cursor，ts-rs derive 导出至 `../src/types/fs.ts`）；`fs_read_dir` 命令签名为 `(path, cursor, limit, state)`（Read 确认）；`READ_DIR_PAGE_DEFAULT = 500`、`READ_DIR_PAGE_MAX = 1000` 常量存在；limit 钳制逻辑存在（Read 确认）。
- **CP-006-b（前端迁移）**：`src/ipc/fs.ts` 含 `readDirPage(path, cursor?, limit?)` wrapper；`src/features/explorer/useFileTree.ts` 首帧 + 续页拼接逻辑存在（Read 确认 gen 竞态语义保留）；`src/types/fs.ts` 含 ts-rs 生成的 `FsReadDirPage`。
- **CP-006-c（测试与文档）**：L1 六用例存在（first_page_has_cursor/last_page_null_cursor/cursor_resume_mid_list/limit_clamped/sort_stable_across_pages/git_filter_still_applied，随全量 cargo test 覆盖）；`src-tauri/src/fs/CLAUDE.md` 红线已改写为「禁止无游标全量返回」（`rg "禁止给 .fs_read_dir. 加分页" src-tauri/src/fs/CLAUDE.md` 零命中）；adr.md ADR-0009 BE-21 行已改写。
- **CP-007-a（基准用例常驻）**：`src-tauri/src/agent_history/claude/scan.rs` 含 `scan_bench_1000_sessions_median_under_50ms` 且非 `#[ignore]`；实测中位/max 数字已记录进 commit body（Read commit message 确认）。
- **CP-007-b（删除分支，达标时适用）**：`rg "SCAN_CACHE|ScanCacheKey|scan_sessions_with_force" src-tauri/src/agent_history` 零命中；`rg "scanAgentHistory\([^)]*, ?true\)" src/` 零命中；`agent_history_scan` 签名无 force 参数（Read 确认）；新增 `scan_reflects_deletion_immediately` 用例存在。
- **CP-007-c（指纹分支，不达标时适用）**：`rg "dir_mtime_ms" src-tauri/src/agent_history` 零命中；`scan_cache_key_tracks_dir_content_fingerprint` 用例存在；agent_history/CLAUDE.md BE-19 节为指纹口径。
- **CP-007 分支互斥**：b/c 两组断言按实测结论只取其一，另一组标记 N/A 并在 details 注明走了哪条分支。

## 全量测试（全部通过为门禁）

1. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
2. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
3. `npx tsc --noEmit`
4. `npx eslint src/`
5. `npm test`

## 人工验证点

- CP-007：benchmark 实测数字（中位/max）人工确认合理并落档。
