# Stage 05 逐项验证断言（唯一真值源）

> stage-05 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **BE-05（三处注册）**：`rg "fs_stat" src-tauri/src/lib.rs` 命中（generate_handler!）；`rg "fs_stat" src-tauri/build.rs` 命中（commands 43）；`rg "allow-fs-stat" src-tauri/capabilities/default.json` 命中。
- **BE-05（DTO+wrapper）**：`rg "FsMetadata" src/types/fs.ts` 命中（生成物含 `sizeBytes: number`、`mtimeMs: number | null` camelCase 形态——Read 确认）；`rg "statFile" src/ipc/fs.ts` 命中（wrapper 签名 `(path: string) => Promise<FsMetadata>`——Read 确认）；`git diff --exit-code -- src/types` 绿（导出后无漂移——测试 agent 结果中 L1 全量含 export_bindings 即承载，另 Read git status 确认 src/types 无未提交漂移）。
- **BE-05（测试）**：L1 三用例（按 checklist BE-05 测试同步节点名——rg 命中 fs/mod.rs 或对应 tests/ 文件）+ L2 ipc 契约用例存在；cargo test / npm test 绿（测试 agent 结果承载）。
- **FE-03**：`rg "LINE_TEXT_COLOR" src/panels/editor/largeFileViewer/LargeFileViewer.tsx` 零命中；Read 确认组件内 `useState(() => schemeRegistry.getActive().editor.overrides.plainText)` + useEffect onDidChange 订阅存在；`LINE_FONT_FAMILY` 保留（rg 命中）。
- **FE-04（信号+props）**：LargeFileSignal 定义仅 `{ filePath }`（rg 定位定义处 Read 确认无 sizeBytes）；LargeFileViewerProps 无 fileSizeBytes（Read 确认）；`rg "fileSizeBytes" src/` 零命中；EditorPanel/GitShowPanel/DiffPanel 三处 LargeFileViewer 使用点不再传 fileSizeBytes（Read 确认）。
- **FE-04（信息条）**：Read LargeFileViewer——fileMeta state 经 `fs.statFile` 挂载 effect 填充；信息条渲染 `fileMeta !== null ? formatSize(fileMeta.sizeBytes) : "…"` 形态。
- **FE-04（收窄登记）**：GitShowPanel/DiffPanel 的 10MB 判定仍用 text.length（Read 确认保留，checklist 登记的 blob 无 stat 通道收窄未被破坏）。
- **FE-05**：`rg "fileGen" src/panels/editor/largeFileViewer/blockCache.ts` 命中（代际 Map）；`rg "invalidateFile" src/panels/editor/largeFileViewer/blockCache.ts` 命中（代际递增 + 前缀清 cache——Read 确认 inflight 保留靠代际比对，无强制 abort）；`_resetBlockCache` 清 fileGen（Read 确认）。
- **FE-05（消费链）**：`rg "fileRev" src/panels/editor/largeFileViewer/useLineIndex.ts` 命中（签名 `(filePath, fileRev)`，fileKey 复合 `${filePath}#${fileRev}`——Read 确认）；Read LargeFileViewer——fileRev state + baseMetaRef + onFsEvent 订阅（Modify 过滤 + 路径归一化命中 → 重 stat 比对 → invalidateFile + setFileRev+1）存在，静默重载无 dirty 交互。
- **FE-08**：Read `src/panels/editor/useCodeMirror.ts` 读盘路径——stat 预检在 readFile 之前（顺序意图断言：>MAX 时 setLargeFile({filePath}) 零读盘返回；>WARN 时 confirmDialog 文案含真实字节 formatSize(meta.sizeBytes)；`filePathRef.current === undefined` 判取消）；读后 doc.length > MAX 复核保留（TOCTOU 防线）。

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
5. `npm test`
6. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
