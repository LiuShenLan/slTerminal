# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 模块职责

hooks 模块——Claude Code hook 宿主侧增强：信号文件通道（接收 + 解析 + 广播）、hook 脚本注入/卸载/状态查询、transcript token 用量查询。

核心数据流：

```
claude 子进程 hook 触发
  → Node 脚本读 stdin → 组装 JSON → 原子写 .tmp → rename .json
  → HookSignalWatcher（notify NonRecursive, 50ms debounce）
  → process_signal_file（读 → parse → emit("hook-event") → 删文件）
  → 前端 onHookEvent 回调 → eventToStatus → 页签 emoji 更新
```

## 架构决策

### 单事件单文件 + 原子 rename（备选 A）

信号文件通道采用单事件单文件方案：hook 脚本先写 `.tmp` 再 `fs.renameSync` 成 `.json`。后端 watcher 通过 `notify` 的 Create 事件检测新文件——rename 完成即代表文件完整落盘，天然避免半写文件。处理完成后删除文件，防止目录无限膨胀。

对比备选 B（JSONL 追加）：JSONL 在 Windows 上多进程并发 `appendFile` 无法保证行级原子性，需额外加锁或合并逻辑，Phase 1 复杂度无显著收益。

详见 `docs/hooks-dev/phase1/checklist.md`「开放项决策」章节。

### SLTERM_PANEL_ID 环境变量路由

PTY spawn 时注入 `SLTERM_PANEL_ID` 环境变量（与 `COLORTERM`/`TERM`/`TERM_PROGRAM` 同一时机），值等于 `request.panel_id`。hook 脚本通过 `process.env.SLTERM_PANEL_ID` 读取，写入信号文件 `panelId` 字段。无此环境变量（非 slTerminal 启动的 claude）→ 脚本直接 `process.exit(0)`。

### 脚本任何路径 exit 0（C10 契约）

`slterm-hook-reporter.js` 零依赖（仅 Node.js >= 18 内置 API），任何代码路径恒 `process.exit(0)`：stdin 为空、JSON 解析失败、目录不可写、无 `SLTERM_PANEL_ID`、顶层异常——全部静默退出。不向 stderr 输出，确保 Claude Code hook 机制不受干扰。

### settings.json 注入/卸载规则

注入时操作 `~/.claude/settings.json`：

- **merge 策略**：读现有 settings → 移除旧 slterm matcher（幂等升级）→ 10 事件每事件键下追加 `{matcher: "", hooks: [{type: "command", command: "node \"<脚本绝对路径>\"", timeout: 5}]}` → 原子写回（NamedTempFile + persist）。用户已有自定义 matcher 保留。
- **非法 JSON 中止**：settings.json 格式错误时返回 AppError，不改动文件。用户需先手动修复。
- **matcher 识别**：通过 command 字符串含 `"slterm-hook-reporter"` 判定。不含此子串的 matcher 视为用户条目，保留不动。
- **10 事件**：`SessionStart`、`SessionEnd`、`UserPromptSubmit`、`Stop`、`StopFailure`、`PreToolUse`、`PostToolUse`、`PostToolUseFailure`、`Notification`、`PermissionRequest`（C9 规定）。

卸载时：

- **安全策略**：settings.json 非法 JSON → 静默跳过配置清理（不损坏用户文件），但仍删除 `~/.slterminal/hooks/` 和 `~/.slterminal/hooks-events/` 目录。
- **清理范围**：移除全部含 slterm 子串的 matcher → 删除空事件键 → 若 hooks 段全空则移除整个 `"hooks"` 键 → 原子写回。
- **目录删除**：`remove_dir_all` 删 `~/.slterminal/hooks/`（脚本）+ `~/.slterminal/hooks-events/`（信号文件），忽略不存在的目录。

### 版本检测

脚本模板通过 `include_str!("../../assets/slterm-hook-reporter.js")` 编译期嵌入。注入状态检测三态：

| 状态 | 判定条件 |
|------|---------|
| `Injected` | 脚本存在 + settings.json 含 slterm matcher + 磁盘 `SCRIPT_VERSION` === 编译期模板版本 |
| `Outdated` | 脚本存在 + settings.json 含 slterm matcher + 版本不匹配 |
| `NotInjected` | 脚本不存在、或 matcher 缺失、或 settings.json 解析失败 |

版本从脚本首行 `const SCRIPT_VERSION = N;` 提取（纯文本解析，无需执行 JS），模板版本和磁盘版本各自提取后比较。

### HookSignalWatcher 全局静态实例

watcher 使用 `static WATCHER: Mutex<Option<HookSignalWatcher>>`（模块级），避免在 `state.rs` 的 `AppState` 中新增字段导致循环依赖。`start_signal_watcher` 幂等：已启动则跳过不报错。watcher 线程名 `hook-signal-watcher`，在 `lib.rs` 的 `.setup()` 中启动。

### 信号文件瞬态特性 + dev 环境注入路径

**目录常态为空是设计行为**：`process_signal_file`（`signal.rs:49-79`）处理后无论 emit 成败均立即 `fs::remove_file` 删除文件，watcher debounce 仅 50ms（`watcher.rs:37`）。信号文件从产生到删除存活亚秒级，任何时刻 `ls` 几乎都看不到文件——目录为空恰是管道正常工作的表现。如需观察信号文件，应使用文件系统监视工具（如 `watchexec`）或临时停 watcher。

**dev 环境注入/卸载/状态查询路径**：前端生产代码无 `inject()` 调用方（F2 入口并入阶段 3），唯一注入入口是 dev/E2E 构建下的 E2E helper（`E2E_ENABLED` 门控，`e2e-tests/helpers.ts:296-300`）：
- `npm run tauri dev` 启动后，devtools 控制台执行 `await window.__slterm_e2e_injectHooks()`（= `hooks.inject()`）
- 状态查询：`await window.__slterm_e2e_getHookInjectionStatus()`（= `hooks.getInjectionStatus()`）
- 卸载：`await window.__slterm_e2e_uninstallHooks()`（= `hooks.uninstall()`）

生产构建（`npx tauri build`）这些 helper 被 tree-shake 排除，注入功能需阶段 3 的 GUI 入口。

## 文件

| 文件 | 职责 |
|------|------|
| `mod.rs` | 模块入口：`InjectionStatus` 枚举 + `HookInjectionStatus` DTO + `start_signal_watcher()` + 静态 `WATCHER` |
| `signal.rs` | 信号文件解析与处理：`HookEventPayload` DTO（8 字段 camelCase）、`parse_signal_file()` 纯函数、`process_signal_file()` 文件处理流程 |
| `watcher.rs` | 信号目录监听器 `HookSignalWatcher`：基于 `notify` + `notify-debouncer-full`，NonRecursive，50ms debounce，线程名 `hook-signal-watcher`，`stop()` 幂等 + `Drop` 清理 |
| `inject.rs` | 注入/卸载/状态三命令：`hooks_inject`、`hooks_uninstall`、`hooks_injection_status`。阻塞 I/O 经 `spawn_blocking` 串行化（硬约束 #3） |
| `usage.rs` | transcript token 用量查询：`hooks_context_usage` 命令 + `ContextUsage` DTO + `parse_usage_line` / `scan_transcript_usage` 纯 I/O 逻辑 |
| `../../assets/slterm-hook-reporter.js` | Node 单文件 hook 上报脚本（`include_str!` 嵌入），零依赖，C10 契约 |

## 命令

三个 Tauri 命令均在 `lib.rs` 的 `generate_handler!` 注册，读写 `~/.claude/settings.json`（绕过 project_root 路径沙箱，照 `settings.rs` 先例）。

### hooks_inject

签名：`async fn hooks_inject() -> Result<HookInjectionStatus, AppError>`

流程：确保 `~/.slterminal/hooks/` 目录存在 → 原子写脚本（NamedTempFile + persist）→ 读 `~/.claude/settings.json`（不存在或空则视为空 JSON 对象）→ 非法 JSON 返回 AppError（不改动文件）→ `remove_slterm_matchers` 清理旧段 → `inject_matchers` 追加 10 事件 matcher → 原子写回 → 返回 `{ status: "injected", version: 1 }`。

返回值 `HookInjectionStatus`：`{ status: "injected" | "notInjected" | "outdated", version: number | null }`（C6 契约，camelCase）。

### hooks_uninstall

签名：`async fn hooks_uninstall() -> Result<(), AppError>`

流程：读 `~/.claude/settings.json` → 移除全部 slterm matcher → 清理空事件键 → 若 hooks 段全空则移除整个键 → 原子写回 → `remove_dir_all` 删 `~/.slterminal/hooks/` + `~/.slterminal/hooks-events/`。JSON 非法时静默跳过配置清理，仍删目录。

### hooks_injection_status

签名：`async fn hooks_injection_status() -> Result<HookInjectionStatus, AppError>`

流程：检查脚本文件是否存在且为普通文件 → 检查 settings.json 中是否有 slterm matcher → 版本比对（磁盘 `SCRIPT_VERSION` vs 模板 `SCRIPT_VERSION`）→ 返回三态之一。

### hooks_context_usage

签名：`async fn hooks_context_usage(_app: AppHandle, transcript_path: String) -> Result<Option<ContextUsage>, AppError>`

参数：`transcript_path` — transcript JSONL 文件路径（来自 `HookEventPayload.transcript_path`）。

返回：`Option<ContextUsage>`——最后一条含 `message.usage` 的行的 token 数据；无 usage 记录或文件异常返回 `null`（非 error）。

流程：`spawn_blocking` 内打开文件 → 读取尾部最多 64KB（`TRANSCRIPT_TAIL_BYTES`）→ UTF-8 解码 → 按行分割 → 从中途起始则跳过首行（截断行）→ 逆行扫描 → 对每行调用 `parse_usage_line` → 返回首个匹配的 usage。

## ContextUsage DTO

`usage.rs` 定义的 token 用量 DTO（C5 契约，camelCase 序列化）：

```rust
pub struct ContextUsage {
    pub input_tokens: u64,   // 输入 token 数，序列化为 "inputTokens"
    pub output_tokens: u64,  // 输出 token 数，序列化为 "outputTokens"
}
```

前端对应 `src/types/hooks.ts` 的 `ContextUsage` 接口（`inputTokens` / `outputTokens`：`number`），IPC 封装见 `src/ipc/hooks.ts` 的 `getContextUsage(transcriptPath)` / `contextUsage(transcriptPath)`。

## 实现要点

### 尾部读取 + 逆行扫描

`scan_transcript_usage` 从文件尾部读取最多 64KB（`TRANSCRIPT_TAIL_BYTES = 64 * 1024`），不足则全读。从中途起始时跳过首行（可能为截断行，JSON 不完整），其余行自末行逆向扫描，遇含 `message.usage` 的完整 JSON 行即返回。此策略确保仅读最小必要数据量，不加载全文件。

### parse_usage_line 纯函数

`parse_usage_line(line: &str) -> Option<ContextUsage>` 解析单行 JSON：提取 `message.usage.input_tokens` 与 `message.usage.output_tokens`（均为 `u64`）。JSON 非法、字段缺失、类型不匹配均返回 `None`，不 panic。

### 解析失败返回 None

任何异常路径均返回 `Ok(None)`（非 `Err`）：文件不存在、权限不足、UTF-8 无效字节、JSONL 全无 usage 行——调用方统一按"无 token 数据"处理，不区分具体错误类型。

## 关键约束

- **阻塞 I/O 必须 `spawn_blocking`**：四命令（三注入/卸载命令 + `hooks_context_usage`）均经 `tokio::task::spawn_blocking` 串行化（硬约束 #3）。
- **原子写**：settings.json 和脚本文件均使用 `tempfile::NamedTempFile` + `persist()`，确保写盘原子性。
- **路径规范化**：脚本绝对路径经 `dunce::simplified()` 处理（剥 Windows `\\?\` 前缀），matcher command 中反斜杠统一替换为 `/`。
- **模板内嵌**：`slterm-hook-reporter.js` 通过 `include_str!` 编译期嵌入，无需运行时读 assets 目录。
- **DTO 双边对应**：`HookInjectionStatus` / `InjectionStatus` / `HookEventPayload` / `ContextUsage` 均 `snake_case` ↔ JS `camelCase`（硬约束 #4）。
- **三命令走绝对 home 路径**：不依赖 `project_root`（类似 `settings.rs`/`projects.rs`），故不经过路径沙箱 `validate_path_within_root`。

## 测试模式

Rust 测试分布 5 个位置（均为 `#[cfg(test)] mod tests` 嵌入源文件），共 66 用例。

| 位置 | 用例数 | 覆盖范围 |
|------|--------|---------|
| `mod.rs` `#[cfg(test)]` | 8 | InjectionStatus/HookInjectionStatus serde（camelCase）、parse_signal_file 快速冒烟（合法/缺 panelId/非法 JSON/空串） |
| `signal.rs` `#[cfg(test)]` | 9 | parse_signal_file 全分支（合法完整/optionals null/缺 panelId/空 panelId/非法 JSON/空串/仅空白）、camelCase 序列化+反序列化往返 |
| `watcher.rs` `#[cfg(test)]` | 6 | is_signal_file（.json/.JSON/.tmp/无扩展名）、watcher 生命周期（stop 幂等、Drop join 线程） |
| `inject.rs` `#[cfg(test)]` | 20 | template_version 正值、HOOK_EVENTS 计数+唯一+关键事件、has_slterm_matchers（空/无 hooks 键/命中/用户 hook 不误检/null hooks 值）、disk_script_version（解析/无版本/缺失/空格分号）、remove_slterm_matchers（清理 slterm 条目+保留用户 hook/清理空事件键/无 slterm 条目）、inject_matchers（10 事件齐全/保留用户 matcher/二次注入幂等）、build_matcher_entry（timeout=5/matcher 空/type=command）、模板内嵌校验（非空/含 SLTERM_PANEL_ID/含 SCRIPT_VERSION） |
| `usage.rs` `#[cfg(test)]` | 23 | parse_usage_line 全分支（合法/缺字段/缺 message/非法 JSON/空串/大值 u64/额外字段忽略/类型不匹配）、scan_transcript_usage 集成（末行命中/中间行回溯/无 usage/文件不存在/空文件/损坏行跳过）、ContextUsage serde camelCase（序列化+反序列化）、TRANSCRIPT_TAIL_BYTES 常量（64KB）、hooks_context_usage 端到端（多条 usage 返最后/无 usage 返 None/损坏行跳过/空文件/大文件 >128KB 仅读尾部 64KB）——P2-TE-05 五用例 |

### 单元测试组织

所有单元测试遵循标准 Rust 模式——`#[cfg(test)] mod tests` 嵌入源文件底部，`use super::*` 导入父模块全部项。

### 信号解析测试模式

`parse_signal_file` 是纯函数（`&str → Option<HookEventPayload>`），无依赖，直接构造 JSON 字符串断言：

```rust
// 合法完整 JSON
let json = r#"{"panelId":"p1","event":"PreToolUse",...}"#;
let p = parse_signal_file(json).unwrap();
assert_eq!(p.panel_id, "p1");
assert_eq!(p.event, "PreToolUse");

// panelId 为空串 → None
assert!(parse_signal_file(r#"{"panelId":"","event":"SessionStart",...}"#).is_none());

// 非法 JSON → None
assert!(parse_signal_file("not json").is_none());
```

### 注入/卸载测试模式

使用 `tempfile::tempdir()` 隔离文件系统，不依赖真实 `~/.claude/settings.json`：

- **has_slterm_matchers**：直接传入 `serde_json::json!({...})` 构造 settings，验证检测逻辑。
- **disk_script_version**：`tempdir` → `std::fs::write` 写含 `SCRIPT_VERSION` 的 .js 文件 → 调用 `disk_script_version` 验证解析。
- **remove_slterm_matchers / inject_matchers**：直接操作 `serde_json::Map`，验证清理/追加/幂等行为。
- **模板内嵌校验**：`HOOK_SCRIPT_TEMPLATE` 是编译期常量，直接断言其长度和关键字符串。

三命令本身（`hooks_inject`/`hooks_uninstall`/`hooks_injection_status`）为 async fn 且依赖 `spawn_blocking`，单元测试未直接覆盖——由 L4 E2E 关键路径（P1-TE-03）验收真实 settings.json 读写。

### watcher 生命周期测试模式

`HookSignalWatcher::start()` 需要 `AppHandle`，单元测试无法构造。测试使用**手动构造模式**（同 `FileWatcher` 测试，见 `notify/CLAUDE.md`）：

```rust
let (stop_tx, stop_rx) = mpsc::channel();
let handle = std::thread::spawn(move || { let _ = stop_rx.recv(); });
let mut w = HookSignalWatcher {
    stop_tx: Some(stop_tx),
    thread_handle: Some(handle),
};
w.stop();  // 第一次停止
w.stop();  // 第二次应不 panic（幂等）
```

### usage token 用量测试模式

`usage.rs` 测试分三层：

**parse_usage_line 纯函数测试**（9 条）：无依赖，直接构造 JSON 字符串断言：
```rust
assert_eq!(parse_usage_line(r#"{"message":{"usage":{"input_tokens":100,"output_tokens":50}}}"#).unwrap(),
           ContextUsage { input_tokens: 100, output_tokens: 50 });
assert!(parse_usage_line("not json").is_none());
assert!(parse_usage_line("").is_none());
```

**scan_transcript_usage 集成测试**（6 条）：使用 `NamedTempFile` 构造 JSONL 文件，验证尾部扫描 + 逆行逻辑：
```rust
let (_dir, path) = make_temp_transcript(&[
    r#"{"message":{"usage":{"input_tokens":10,"output_tokens":5}}}"#,
    r#"{"message":{"usage":{"input_tokens":100,"output_tokens":50}}}"#,
]);
let r = scan_transcript_usage(path.to_str().unwrap()).unwrap();
assert_eq!(r.input_tokens, 100);  // 末行优先
```

**hooks_context_usage L1 端到端**（5 条，P2-TE-05）：验证多条 usage（逆行返最后）、末尾无 usage（返 None）、损坏行跳过、空文件、大文件（>128KB padding + usage 在最后 1KB → 仅加载尾部 64KB 仍命中）。

`scan_transcript_usage` 是纯 I/O 逻辑函数（非 async），测试直接调用不依赖 `spawn_blocking`。`hooks_context_usage` 命令本身为 async fn 由 L4 E2E 验收。

### 运行

```bash
# 运行全部 hooks 测试
cargo test --manifest-path src-tauri/Cargo.toml hooks -- --test-threads=1

# 运行单个文件测试
cargo test --manifest-path src-tauri/Cargo.toml hooks::inject -- --test-threads=1
cargo test --manifest-path src-tauri/Cargo.toml hooks::signal -- --test-threads=1
cargo test --manifest-path src-tauri/Cargo.toml hooks::watcher -- --test-threads=1
cargo test --manifest-path src-tauri/Cargo.toml hooks::usage -- --test-threads=1
```

## 修改注意事项

1. 新增 Tauri 命令后在 `lib.rs` 的 `generate_handler!` 注册。
2. 修改 `HOOK_EVENTS` 常量后跑 `hook_events_count_and_unique` + `hook_events_contains_key_events` 测试。
3. 修改 `slterm-hook-reporter.js` 后：确保 `SCRIPT_VERSION` 递增 + 跑 `template_version_positive` / `template_is_non_empty` 测试确认内嵌内容正确。
4. 修改注入逻辑（`remove_slterm_matchers`/`inject_matchers`/`build_matcher_entry`）后跑 `inject.rs` 全部 20 条测试，尤其幂等测试（`inject_idempotent`）。
5. 修改 `parse_signal_file` / `process_signal_file` 后跑 `signal.rs` 全部 9 条测试 + `mod.rs` 的 4 条快速冒烟测试。
6. **绝对不要修改 C10 契约**——`slterm-hook-reporter.js` 任何代码路径必须 `process.exit(0)`。新增 catch 分支时确认静默退出、不写 stderr。
7. 修改注入命令的 settings.json 读写逻辑后，务必跑 L4 E2E（`npm run e2e`）验证真实 settings.json merge/卸载/非法 JSON 中止行为（P1-TE-03）。
8. 修改 `parse_usage_line` / `scan_transcript_usage` / `TRANSCRIPT_TAIL_BYTES` 后跑 `usage.rs` 全部 23 条测试，尤其 P2-TE-05 五用例（大文件尾部扫描、损坏行跳过）。
9. 修改 `ContextUsage` DTO 字段后同步更新 `src/types/hooks.ts`（前端 `ContextUsage` 接口）和 `src/ipc/hooks.ts`（IPC wrapper），跑 `ipc-hooks-contract.test.ts` + `context_usage_serialize_camelcase` / `context_usage_deserialize_camelcase` 测试。
