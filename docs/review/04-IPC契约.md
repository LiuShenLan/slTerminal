# IPC/API 契约 审查报告

## 1. 发现列表

| # | 位置(file:line) | 级别 | 描述 | 建议 |
|---|-----------------|------|------|------|
| 1 | `D:\data\learn\code\slTerminal\src\types\fs.ts:12-14` | P1 | `DirEntry.size`/`modified` 在 Rust 端为 `Option<u64>`，serde 对目录条目会序列化为 `null`；TS 端声明为可选 `?: number`，语义上是“可能缺失”而非“可能为 null”，与运行时实际 JSON 形状不一致。 | 改为 `size: number \| null; modified: number \| null;`，与 `Option<u64>` 精确对应。 |
| 2 | `D:\data\learn\code\slTerminal\src\types\notify.ts:5` | P2 | `FsEventPayload.detail` 在 Rust 端为必填 `String`（notify/mod.rs:380 永远赋值），TS 端却声明为可选 `?: string`。 | 改为 `detail: string`，与后端输出保持一致。 |
| 3 | `D:\data\learn\code\slTerminal\src\types\hooksConfig.ts:9-99` | P2 | hooks 配置相关 DTO（`HooksLayer`、`HooksConfigJson`、各 handler 模型）在前端有强类型，但 Rust 端 `agent_hooks_config_read/write` 直接使用 `serde_json::Value`，无对应 struct；非法 layer 或 handler 形状只能在运行时被后端 `parse_layer`/`write_hooks_subtree` 拒绝。 | 在 Rust 端补充 hooks 子树 DTO（至少 layer 枚举与 handler 结构体），或在前端把 `HooksLayer` 收窄为 `"user" \| "project" \| "local"` 并在写入前做客户端校验。 |
| 4 | `D:\data\learn\code\slTerminal\src\types\fs.ts:12`、`D:\data\learn\code\slTerminal\src\types\agentHistory.ts:21`、`D:\data\learn\code\slTerminal\src\types\agent.ts:21` | P2 | Rust `u64`（`DirEntry.size/modified`、`AgentHistorySession.mtimeMs`、`AgentEventPayload.timestamp`）映射到 TS `number`，超出 `Number.MAX_SAFE_INTEGER`（2^53）后会丢失精度；虽然当前时间戳/普通文件大小不会触发，但文件大小理论上可超过安全整数范围。 | 对可能极大的 `u64` 字段（如文件大小）在 DTO 层使用 `string` 或 `bigint`；若保持 `number`，应在文档中明确可接受的范围。 |
| 5 | `D:\data\learn\code\slTerminal\src\ipc\*`（多处 `.catch`） | P2 | 所有命令统一返回 `Result<_, AppError>` 且序列化为 camelCase，但前端没有集中解析 `AppError` 变体的逻辑，调用方多以字符串形式 catch 后 `console.warn` 或显示占位文案，无法根据错误类型做差异化处理。 | 增加 `parseAppError(error): { variant, message }` 辅助函数，统一从 Tauri reject 中解析 `ioKind`/`sessionNotFound`/`pathNotAllowed` 等结构化错误。 |
| 6 | `D:\data\learn\code\slTerminal\src\types\pty.ts:9-16` | P2 | `SpawnRequest.cols/rows` 为 `number`，而 Rust 端为 `u16` 并在 `validate_spawn_request` 中做上限校验；TS 类型不限制范围，超大值会在 invoke 时被后端拒绝。 | 在 wrapper 或类型层面增加范围校验（1..32767），提前给出明确错误。 |
| 7 | `D:\data\learn\code\slTerminal\src\types\hooksConfig.ts:9` | P2 | `HooksLayer` 为任意 `string`，但后端 `parse_layer` 只接受 `user`/`project`/`local`；传入非法 layer 会触发 `AppError::Validation`。 | 收窄为 `"user" \| "project" \| "local"`，与后端合法值集一致。 |

## 2. 命令对照表

| # | Rust 注册命令（`src-tauri/src/lib.rs`） | 行号 | 前端封装位置 | 前端调用命令名 | 状态 |
|---|------------------------------------------|------|--------------|----------------|------|
| 1 | `ping` | 79 | `src/ipc/index.ts:20` | `ping` | OK |
| 2 | `get_windows_build_number` | 80 | `src/ipc/pty.ts:94` | `get_windows_build_number` | OK |
| 3 | `state::set_project_root` | 81 | `src/ipc/fs.ts:41` | `set_project_root` | OK |
| 4 | `pty::spawn::pty_spawn` | 82 | `src/ipc/pty.ts:19` | `pty_spawn` | OK |
| 5 | `pty::spawn::pty_write` | 83 | `src/ipc/pty.ts:37` | `pty_write` | OK |
| 6 | `pty::spawn::pty_resize` | 84 | `src/ipc/pty.ts:57` | `pty_resize` | OK |
| 7 | `pty::spawn::pty_kill` | 85 | `src/ipc/pty.ts:66` | `pty_kill` | OK |
| 8 | `pty::spawn::pty_reattach` | 86 | `src/ipc/pty.ts:80` | `pty_reattach` | OK |
| 9 | `fs::fs_read_file` | 87 | `src/ipc/fs.ts:9` | `fs_read_file` | OK |
| 10 | `fs::fs_write_file` | 88 | `src/ipc/fs.ts:17` | `fs_write_file` | OK |
| 11 | `fs::fs_read_dir` | 89 | `src/ipc/fs.ts:22` | `fs_read_dir` | OK |
| 12 | `fs::fs_create_dir` | 90 | `src/ipc/fs.ts:27` | `fs_create_dir` | OK |
| 13 | `fs::fs_delete` | 91 | `src/ipc/fs.ts:32` | `fs_delete` | OK |
| 14 | `fs::fs_rename` | 92 | `src/ipc/fs.ts:37` | `fs_rename` | OK |
| 15 | `settings::save_settings` | 93 | `src/ipc/settings.ts:15` | `save_settings` | OK |
| 16 | `settings::load_settings` | 94 | `src/ipc/settings.ts:8` | `load_settings` | OK |
| 17 | `projects::save_projects` | 95 | `src/ipc/projects.ts:13` | `save_projects` | OK |
| 18 | `projects::load_projects` | 96 | `src/ipc/projects.ts:8` | `load_projects` | OK |
| 19 | `git::git_status` | 97 | `src/ipc/git.ts:9` | `git_status` | OK |
| 20 | `git::git_diff` | 98 | `src/ipc/git.ts:17` | `git_diff` | OK |
| 21 | `git::git_file_at_head` | 99 | `src/ipc/git.ts:28` | `git_file_at_head` | OK |
| 22 | `git::git_rollback` | 100 | `src/ipc/git.ts:39` | `git_rollback` | OK |
| 23 | `git::git_unstage` | 101 | `src/ipc/git.ts:50` | `git_unstage` | OK |
| 24 | `notify::notify_watch` | 102 | `src/ipc/notify.ts:14` | `notify_watch` | OK |
| 25 | `hooks::agent_hooks_inject` | 103 | `src/ipc/agentHooks.ts:20` | `agent_hooks_inject` | OK |
| 26 | `hooks::agent_hooks_uninstall` | 104 | `src/ipc/agentHooks.ts:25` | `agent_hooks_uninstall` | OK |
| 27 | `hooks::agent_hooks_injection_status` | 105 | `src/ipc/agentHooks.ts:32` | `agent_hooks_injection_status` | OK |
| 28 | `hooks::agent_hooks_restore_statusline` | 106 | `src/ipc/agentHooks.ts:42` | `agent_hooks_restore_statusline` | OK |
| 29 | `hooks::agent_hooks_config_read` | 107 | `src/ipc/hooksConfig.ts:21` | `agent_hooks_config_read` | OK |
| 30 | `hooks::agent_hooks_config_write` | 108 | `src/ipc/hooksConfig.ts:40` | `agent_hooks_config_write` | OK |
| 31 | `agent_history::agent_history_scan` | 109 | `src/ipc/agentHistory.ts:15` | `agent_history_scan` | OK |
| 32 | `agent_history::agent_history_delete` | 110 | `src/ipc/agentHistory.ts:28` | `agent_history_delete` | OK |
| 33 | `agent_history::agent_history_read_title` | 111 | `src/ipc/agentHistory.ts:42` | `agent_history_read_title` | OK |

**结论**：`generate_handler!` 注册 33 条自定义命令，`src/ipc/` 中 33 条前端 invoke 封装一一对应，无“已注册但前端无封装”的孤儿命令，也无“前端 invoke 但未注册”的命令。

## 3. DTO 对照抽查明细

### 3.1 文件系统：`DirEntry`

| Rust (`src-tauri/src/fs/mod.rs:17-28`) | TS (`src/types/fs.ts:4-15`) | 状态 |
|----------------------------------------|------------------------------|------|
| `name: String` | `name: string` | OK |
| `path: String` | `path: string` | OK |
| `is_dir: bool` | `isDir: boolean` | OK |
| `size: Option<u64>` | `size?: number` | **P1 漂移**：Rust 序列化为 `null`，TS 应声明为 `number \| null` |
| `modified: Option<u64>` | `modified?: number` | **P1 漂移**：同上 |

### 3.2 Git：`GitStatusEntry` / `DiffHunk`

| Rust (`src-tauri/src/git/mod.rs:19-26`, `31-40`) | TS (`src/types/git.ts:4-26`) | 状态 |
|----------------------------------------------------|-------------------------------|------|
| `path: String` / `status: String` / `old_path: Option<String>` | `path` / `status` / `oldPath: string \| null` | OK |
| `old_start/u32` / `old_lines/u32` / `new_start/u32` / `new_lines/u32` | `oldStart/number` / `oldLines/number` / `newStart/number` / `newLines/number` | OK（u32 在 number 安全范围） |

### 3.3 PTY：`SpawnRequest` / `PtyEvent`

| Rust (`src-tauri/src/pty/spawn.rs:907-918`, `856-861`) | TS (`src/types/pty.ts:4-16`) | 状态 |
|---------------------------------------------------------|-------------------------------|------|
| `panel_id: String`, `cols: u16`, `rows: u16`, `cwd: Option<String>`, `shell: Option<String>` | `panelId: string`, `cols: number`, `rows: number`, `cwd?: string`, `shell?: string` | OK（`#[serde(rename_all = "camelCase")]` 已声明） |
| `Output { bytes: Vec<u8> }` / `Exit { code: Option<i32> }`，`#[serde(tag="type", content="data", rename_all="camelCase")]` | `\| { type: "output"; data: { bytes: number[] } } \| { type: "exit"; data: { code: number \| null } }` | OK |

### 3.4 监听：`FsEventPayload`

| Rust (`src-tauri/src/notify/mod.rs:28-35`) | TS (`src/types/notify.ts:2-6`) | 状态 |
|--------------------------------------------|--------------------------------|------|
| `paths: Vec<String>` | `paths: string[]` | OK |
| `kind: String` | `kind: string` | OK |
| `detail: String`（永远有值） | `detail?: string` | **P2 漂移**：应改为必填 |

### 3.5 Agent Hooks：`AgentEventPayload` / `AgentHookInjectionStatus`

| Rust (`src-tauri/src/hooks/signal.rs:31-57`, `src-tauri/src/hooks/mod.rs:31-52`) | TS (`src/types/agent.ts:12-42`) | 状态 |
|----------------------------------------------------------------------------------|----------------------------------|------|
| `panel_id/event/timestamp/session_id/cwd` + `usage_source_path: Option<String>` + `tool_name/notification_type/cli_id/used_percentage` | 对应 camelCase 字段，`usageSourcePath?/usedPercentage?/cliId?` 可选 | OK（可选字段均配 `#[serde(default)]`） |
| `AgentInjectionStatus`（`Injected/NotInjected/Outdated`，camelCase） | `"injected" \| "notInjected" \| "outdated"` | OK |
| `AgentHookInjectionStatus { status, version: Option<u32> }` | `{ status, version: number \| null }` | OK |

### 3.6 Agent History：`AgentHistorySession` / `AgentHistoryTitle`

| Rust (`src-tauri/src/agent_history/mod.rs:31-48`, `57-62`) | TS (`src/types/agentHistory.ts:9-37`) | 状态 |
|--------------------------------------------------------------|----------------------------------------|------|
| `session_id/cwd/title/title_source/first_prompt/mtime_ms/cwd_exists/cli_id` | 对应 camelCase 字段，`cwd/title/firstPrompt` 为 `string \| null` | OK |
| `AgentHistoryTitle { title: Option<String>, title_source: String }` | `{ title: string \| null, titleSource: string }` | OK |

### 3.7 Hooks 配置

| Rust | TS (`src/types/hooksConfig.ts`) | 状态 |
|------|---------------------------------|------|
| `serde_json::Value`（`agent_hooks_config_read/write`） | `HooksLayer`、`HooksConfigJson`、GUI/JSON handler 模型 | **P2 弱类型**：后端无强类型 DTO，仅靠运行时校验 |

## 4. 扫描范围

- 后端命令与 DTO：`D:\data\learn\code\slTerminal\src-tauri\src\lib.rs`（`generate_handler!`）
- 后端模块：`src-tauri\src\fs\mod.rs`、`git\mod.rs`、`pty\spawn.rs`、`notify\mod.rs`、`hooks\mod.rs`、`hooks\signal.rs`、`agent_history\mod.rs`、`settings.rs`、`projects.rs`、`error.rs`
- 前端封装层：`D:\data\learn\code\slTerminal\src\ipc\*`（含 `index.ts`、`pty.ts`、`fs.ts`、`git.ts`、`settings.ts`、`projects.ts`、`notify.ts`、`agentHooks.ts`、`hooksConfig.ts`、`agentHistory.ts`）
- 前端 DTO：`D:\data\learn\code\slTerminal\src\types\*`（`agent.ts`、`agentHistory.ts`、`fs.ts`、`git.ts`、`hooksConfig.ts`、`notify.ts`、`pty.ts`）
- 已排除：`src\__tests__\*`、`e2e-tests\*`、Rust 测试目录、插件 re-export 文件（`clipboard.ts`/`dialog.ts`/`shell.ts`/`notification.ts`/`window.ts`）中的官方插件命令。

## 5. 未覆盖区域

- `src/ipc/clipboard.ts`、`dialog.ts`、`shell.ts`、`notification.ts`、`window.ts` 为 Tauri 官方插件或 Window API 的薄封装，不属于 `generate_handler!` 注册的自定义命令，本次未逐条核对其插件命令名/权限。
- `src-tauri/src/pty/spawn.rs` 中大量 ConPTY 内部实现细节（Job Object、ring buffer、reader 线程）不在 IPC 契约维度审查范围内。
- 运行时真实序列化（camelCase ↔ snake_case、Channel 序列化、Uint8Array → number[]）属于 L4 E2E 验证范围，本次静态审查未执行。

## 6. 建议修复（按优先级排序）

1. **P1：修正 `DirEntry` 可选性**（`src/types/fs.ts:12-14`）——把 `size?: number` / `modified?: number` 改为 `size: number \| null` / `modified: number \| null`，与 Rust `Option<u64>` 的 `null` 输出精确对应。
2. **P2：修正 `FsEventPayload.detail` 必填性**（`src/types/notify.ts:5`）——改为 `detail: string`。
3. **P2：收紧 `HooksLayer` 类型**（`src/types/hooksConfig.ts:9`）——改为 `"user" \| "project" \| "local"`，与后端 `parse_layer` 合法值集一致。
4. **P2：补充 hooks 配置 Rust 端 DTO**（`src-tauri/src/hooks/claude/config.rs`）——把 `agent_hooks_config_read/write` 的 `serde_json::Value` 升级为 layer 枚举 + hooks 子树结构体，提升双边类型安全。
5. **P2：增加 AppError 结构化解析器**（可在 `src/ipc/error.ts` 新建）——统一把 Tauri reject 的 JSON 解析为 `{ variant, message }`，供调用方按错误类型处理（如 `PathNotAllowed`、`SessionNotFound`）。
6. **P2：对 `SpawnRequest.cols/rows` 加范围校验**（`src/ipc/pty.ts` 或类型层）——提前拦截 0 或超过 `i16::MAX` 的值，避免无意义的后端往返。
7. **P2：文档化/处理 `u64 → number` 精度风险**——对 `DirEntry.size`、`AgentHistorySession.mtimeMs`、`AgentEventPayload.timestamp` 等字段，明确业务可接受范围；若未来需支持超大文件，改用字符串传输。
