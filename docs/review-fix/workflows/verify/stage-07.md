# Stage 07 逐项验证断言（唯一真值源）

> stage-07 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **BE-03**：`src-tauri/src/fs/mod.rs` 的 `fs_read_file` 签名含 `Channel<FsReadChunk>` 参数（grep 命中）；`FsReadChunk` 结构含 `data: String`、`done: bool` 两字段（Read 确认）
- **BE-03**：存在 256KB 块大小常量（grep 命中 256 相关常量定义）；10MB 上限校验保留（Read 确认 metadata 大小校验在分块发送之前）
- **BE-03**：UTF-8 边界处理存在（语义式，须 Read 确认：多字节字符跨块不被切散——回退到 char boundary 或等效机制；发现直接 from_utf8_lossy 整块切分判 partial 并说明风险）
- **BE-03**：发送序列终态为 `done: true` 消息（Read 确认）；新增 L1 用例存在（多块拼接还原、多字节跨界、超限拒绝、空文件）
- **BE-03**：`src/ipc/fs.ts` 的 `readFile(path: string): Promise<string>` 签名不变（Read 确认）；内部 Channel 监听拼接逻辑存在（语义式）
- **BE-03**：`git diff --name-only` 确认 `src/panels/diff/DiffPanel.tsx`、`src/panels/html/HtmlPanel.tsx`、`src/panels/editor/useCodeMirror.ts` 三消费方本 Stage 零改动
- **BE-03**：`src/__tests__/ipc-contract.test.ts` 已更新为新 payload 形态（grep Channel/done 相关断言）

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
