# Stage 1 逐项验证断言

> stage-1 脚本与 fix-loop 的 verify agent 均以本文件为准。

## 断言清单

- **P2-BE-01**：`src-tauri/Cargo.toml` 的 `[dependencies]` 段存在 `tauri-plugin-notification = "2"`（grep 命中行）。
- **P2-BE-02**：`src-tauri/src/lib.rs` 中 `tauri_plugin_notification::init()` 被调用，且 `generate_handler!` 宏参数列表含 `hooks_context_usage`（grep 命中）。
- **P2-BE-03**：`src-tauri/capabilities/default.json` 的 `permissions` 数组含 `"notification:default"`（grep 命中）。
- **P2-BE-04**：`src-tauri/src/hooks/` 中存在 `hooks_context_usage` 函数/命令实现；函数体在 `spawn_blocking` 内执行文件读取；逆行扫描 JSONL 行；任何失败路径返回 `Ok(None)` 或等价的 `Ok(None)` 而不是 `unwrap`/`expect`（须 Read 代码确认）。
- **P2-BE-05**：存在 `ContextUsage` 结构体，字段为 `input_tokens` 与 `output_tokens`，且带 `#[serde(rename_all = "camelCase")]`；JS 侧 `src/types/hooks.ts` 存在 `ContextUsage` 接口，字段为 `inputTokens`/`outputTokens`。
- **P2-BE-06**：`src-tauri/src/hooks/signal.rs`（或阶段 1 解析文件）解析后的事件 DTO 含 `transcript_path` 字段；若字段缺失，解析逻辑不会 panic（须 Read 代码确认缺省处理）。

## 全量测试（全部通过为门禁）

1. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
2. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`

## 语义式断言

- `hooks_context_usage` 的尾部读取逻辑必须避免一次性加载整个文件到内存（须 Read 代码确认使用 seek/tail buffer，不接受 `std::fs::read_to_string` 全读）。
- 任何 JSON 解析失败不得 panic 或返回 `Err` 导致前端崩溃；必须降级为 `Ok(None)`（须 Read 代码确认）。
