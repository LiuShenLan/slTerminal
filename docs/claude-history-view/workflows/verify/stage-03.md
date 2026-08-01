# Stage 03 逐项验证断言（唯一真值源）

> stage-03 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **FE-01**：`src/types/claudeHistory.ts` 存在；`HistorySession` 接口字段恰为 `sessionId/cwd/title/titleSource/firstPrompt/mtimeMs/cwdExists` 七键（Read 逐字对照）；`TitleSource` = `"customTitle" | "aiTitle" | "summary" | "firstPrompt" | "none"`（Read 逐字对照）；`src/types/index.ts` grep `claudeHistory` 命中（export 登记）。
- **DTO 双边一致（意图断言）**：`src/types/claudeHistory.ts` 的七键与后端 `src-tauri/src/claude_history/mod.rs` 的 serde 输出键**逐字一致**（Read 双侧对照，防漂移）。
- **FE-02**：`src/ipc/claudeHistory.ts` 存在；导出 `scanHistory`/`deleteHistorySession`/`renameHistorySession` 三函数（Read 确认）；invoke 命令名为 `claude_history_scan`/`claude_history_delete`/`claude_history_rename`（grep 逐字命中）；invoke 参数为 camelCase `{ sessionId }` / `{ sessionId, newTitle }`（Read 确认）；`src/ipc/index.ts` grep `claudeHistory` 命中（barrel 登记）。
- **invoke 单点（语义式）**：`claudeHistory.ts` 之外的新增文件中不存在 `invoke(` 调用（grep `src/` 新增文件确认——硬约束 #1）。
- **FE-03**：`src/__tests__/ipc-claude-history-contract.test.ts` 存在；三命令 × 四维（命令名/参数结构/正常返回/异常传播）每命令各 4 条断言共 12 条用例（Read 测试文件计数 it/test 块）；异常传播断言为 rejects（Read 确认不吞异常）。
- **禁区**：`git diff` 本 Stage 不含 `src-tauri/src/pty/` 下任何文件改动。

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
