# Stage 09 逐项验证断言（唯一真值源）

> stage-09 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。
> **S02 未拆红线（embed-manifest 通道失实转登记，2026-09-07）**：L1 定向命令维持旧形态 = `cargo test --test lib_tests <filter> -- --test-threads=1`；文中裸 `cargo test <filter>`（如 fs_read_file_range）断言按此形态执行或以全量（无 filter）覆盖判定。

## 断言清单

- **CP-022-a（后端命令）**：`grep -n "fs_read_file_range" src-tauri/src/lib.rs src-tauri/build.rs src-tauri/capabilities/default.json` 各 ≥1（三处注册）；`src-tauri/src/fs/` 内实现含 UTF-8 边界裁剪（Read 确认裁到完整字符边界）；`cargo test fs_read_file_range -- --test-threads=1` 绿（随全量覆盖：空区间/越界 clamp/多字节边界/不存在路径 AppError）。
- **CP-022-b（前端模块）**：`src/panels/editor/largeFileViewer/` 三文件存在（LargeFileViewer.tsx/useLineIndex.ts/blockCache.ts）；常量 `READ_BLOCK_BYTES = 256*1024`、`BLOCK_CACHE_LIMIT = 32`、`LARGE_FILE_LINE_HEIGHT = 20` 存在（grep 各 ≥1）；`src/ipc/fs.ts` 含 `readFileRange` wrapper；`useCodeMirror.ts` 返回值含 `largeFile` 信号且 `filePathRef.current = undefined` 保留（Read 确认）。
- **CP-022-c（引导改造）**：`useCodeMirror.ts` 超限分支不再置拒绝文案 doc（Read 确认改为 largeFile 信号）；`GitShowPanel.tsx`/`DiffPanel.tsx` 超限分支渲染 LargeFileViewer（Read 确认，diff 单侧超限降级单文档浏览）；**10MB/1MB 常量原值不动**（`grep -n "10_000_000\|1_000_000" src/panels/editor/useCodeMirror.ts` 各 ≥1）。
- **CP-022-d（测试与文档）**：`npx vitest run large-file-viewer use-code-mirror gitshow-panel diff-panel` 绿（随全量覆盖）；`src/panels/editor/CLAUDE.md` FE-31 节改写为四层防线口径（Read 确认）。

## 全量测试（全部通过为门禁）

1. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
2. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
3. `npx tsc --noEmit`
4. `npx eslint src/`
5. `npm test`

## 人工/L4 验证点

- >10MB 文本经 editor 打开 → 只读浏览可滚动至 EOF 且行内容正确（抽样断言）。
