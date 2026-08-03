# 05 L1 settings+projects+error+lib 测试 Review

## 元信息
- 领域：Rust 后端顶层单文件模块（settings / projects / error / lib）
- 测试位置：`src-tauri/src/settings.rs`、`src-tauri/src/projects.rs`、`src-tauri/src/error.rs`、`src-tauri/src/lib.rs` 各自的 `#[cfg(test)]`
- 用例数：35（settings 17 + projects 12 + error 4 + lib 2）
- 覆盖率概况：settings.rs 76.2%（231/303）、projects.rs 89.0%（129/145）、error.rs 83.9%（52/62）、lib.rs 30.6%（15/49）
- 审查日期：2026-08-04

## 覆盖率缺口

### 🔴 核心逻辑零覆盖
- `settings.rs:38-106`：`save_settings` / `load_settings` 两个 `#[tauri::command]` async 函数整体未被任何测试直接调用。测试全部通过"手动复现内部逻辑"完成，导致 `spawn_blocking` 闭包、`TaskJoin` 错误映射、`app_data_dir()` 与路径拼接、`.bak` 备份与恢复、原子写入（`NamedTempFile`→`persist`）的真实集成路径全部零覆盖。
- `projects.rs:64-81`：`save_projects` / `load_projects` 两个 `#[tauri::command]` async 包装函数零覆盖；测试只调用内层 `save_to_dir` / `load_from_dir`。

### 🟡 边界分支未覆盖
- `settings.rs:12-18`：`app_data_dir()` 的 `current_exe` 失败、`exe.parent()` 失败两个错误分支未覆盖；仅测试了成功返回 exe 父目录的路径。
- `settings.rs:63-64`：`NamedTempFile::persist` 失败时映射为 `AppError::IoKind` 的分支未覆盖。
- `projects.rs:26-28`：`save_to_dir` 中 `tmp.persist` 失败映射为 `AppError::IoKind` 的分支未覆盖。
- `error.rs:49-63`：`From<serde_json::Error>`、`From<git2::Error>`、`From<tokio::task::JoinError>` 三个 `From` 实现未覆盖；仅 `From<std::io::Error>` 与 `SessionNotFound` 序列化被测。

### 🟢 低风险未覆盖
- `settings.rs` 未覆盖并发写、部分写坏、权限不足、Unicode 路径、超大 settings.json（>MB 级）等边界。当前 happy path 已覆盖，但这些场景对便携配置持久化模块属于防御性增强，非当前回归重点。

### 🟢 既定豁免
- `lib.rs:34-116`：`run()` 函数（Tauri 运行时初始化、插件加载、`generate_handler!`、启动失败退出）属于 Tauri 运行时胶水代码，L1 单测无法在不启动完整应用的情况下覆盖，由 L4 E2E 兜底。`ping` 与 `get_windows_build_number` 两条命令已被覆盖，符合既定豁免。

## 问题列表

### P-1 [🔴] [断言有效性] settings.rs 全部核心用例未调用真实被测函数，属于"模拟逻辑替代被测逻辑"
- 位置: `src-tauri/src/settings.rs:114-498`（全部 `#[test]`）
- 代码片段:
```rust
#[test]
fn test_save_settings_and_load() {
    let dir = tempfile::tempdir().unwrap();
    let settings_path = dir.path().join("settings.json");
    let settings: serde_json::Value = serde_json::json!({...});

    // --- save 逻辑（同步版）---
    let json = serde_json::to_string_pretty(&settings).unwrap();
    let mut tmp = NamedTempFile::new_in(dir.path()).unwrap();
    tmp.write_all(json.as_bytes()).unwrap();
    tmp.flush().unwrap();
    tmp.persist(&settings_path).unwrap();

    // --- load 逻辑（同步版）---
    let content = std::fs::read_to_string(&settings_path).unwrap();
    let loaded: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert_eq!(loaded, settings);
}
```
- 问题: 该用例以及 `test_load_settings_file_not_found`、`test_load_settings_corrupt_json_fallback_to_bak`、`test_load_settings_corrupt_json_no_bak`、全部 `te14_*` 用例均未调用 `save_settings()` / `load_settings()`，而是把函数内部逻辑在测试里重写一遍。结果是：
  1. `spawn_blocking` 调度、`tokio::task::JoinError` 映射（`AppError::TaskJoin`）完全未验证；
  2. `app_data_dir()` 返回 exe 目录再拼接 `settings.json` 的路径解析逻辑未与 `save_settings`/`load_settings` 集成验证；
  3. `.bak` 备份/恢复、原子写入的正式路径未在命令包装层跑通；
  4. 若被测函数内部逻辑与测试手写逻辑出现漂移（例如将来在 `save_settings` 里加入前置校验或日志），测试不会发现。
- 改法: 引入 `tokio::runtime::Runtime` 在同步测试中 `block_on` 调用真实的 `save_settings` 与 `load_settings`，并用 `std::env::set_current_exe` 不可行，但可通过把 `app_data_dir()` 改为可注入（或在测试模块内临时替换 `current_exe` 结果）来让测试指向 `tempdir`。更轻量的改法：新增针对 `save_settings`/`load_settings` async 命令的测试，手动构造 `AppHandle` 不必要，只需验证命令函数体能被正确调用（Tauri 命令在单测中可直接 `await`）。
- 变异推演: 若将 `save_settings` 中的 `.bak` 备份逻辑删除（`if settings_path.exists() { ... copy ... }` 整块移除），所有现有 `te14_*` 用例仍然全绿，因为它们不调用 `save_settings`；只有直接调用命令的测试会变红。

### P-2 [🔴] [Mock/架空] settings.rs 测试架空了真实的 invoke/Tauri 命令路径
- 位置: `src-tauri/src/settings.rs:38-106`（被测命令）
- 问题: 前端 L2 `ipc-contract.test.ts` 虽然会 mock 拦截 `save_settings` / `load_settings` 命令名，但 Rust 侧 L1 却从未验证这两条命令作为 Tauri 命令的完整行为（输入 Value、输出 `Result<(), AppError>` / `Result<Value, AppError>`）。`#[tauri::command]` 宏本身不改变语义，但若命令签名将来被误改（例如参数名、返回类型），L1 不会发现；L2 的 mock 只校验命令名与参数结构，不校验 Rust 端类型。
- 改法: 在 L1 增加对 `save_settings` 和 `load_settings` 的端到端调用（同步测试里用 `tokio::test` 或单线程 Runtime），并断言返回值类型与异常传播。
- 变异推演: 若将 `load_settings` 的返回类型误改为 `Result<String, AppError>`，当前 L1 全绿（因为没有调用它）；新增命令包装测试后会直接编译失败或运行时类型不匹配。

### P-3 [🟡] [断言有效性] projects.rs 测试只覆盖 I/O 内核，未覆盖 Tauri 命令包装层
- 位置: `src-tauri/src/projects.rs:64-81`
- 代码片段:
```rust
#[tauri::command]
pub async fn save_projects(data: String) -> Result<(), AppError> {
    let app_dir = app_data_dir()?;
    match tokio::task::spawn_blocking(move || save_to_dir(&app_dir, &data)).await {
        Ok(inner) => inner,
        Err(e) => Err(AppError::TaskJoin(e.to_string())),
    }
}
```
- 问题: 12 条测试全部调用 `save_to_dir` / `load_from_dir`，从未调用 `save_projects` / `load_projects`。`app_data_dir()` 在命令层被调用、`spawn_blocking`、`TaskJoin` 错误映射均未被验证。这导致 `save_projects` 若错误地把 `app_dir` 闭包移动到阻塞线程外、或误删 `spawn_blocking`，L1 不会发现。
- 改法: 增加 2 条测试直接 `await save_projects(...)` 与 `await load_projects()`，可用 `tokio::runtime` 在同步测试里驱动。
- 变异推演: 若将 `save_projects` 改成直接 `save_to_dir(&app_data_dir()?, &data)`（去掉 `spawn_blocking`），当前 12 条用例仍全绿；新增命令包装测试后，可通过在闭包内断言线程切换或阻塞 I/O 在 worker 线程执行来变红。

### P-4 [🟡] [覆盖度] error.rs 三个 `From` 实现未覆盖
- 位置: `src-tauri/src/error.rs:49-63`
- 代码片段:
```rust
impl From<serde_json::Error> for AppError { ... AppError::Serde(e.to_string()) }
impl From<git2::Error> for AppError { ... AppError::Git(e.to_string()) }
impl From<tokio::task::JoinError> for AppError { ... AppError::TaskJoin(e.to_string()) }
```
- 问题: 仅 `From<std::io::Error>` 与 `SessionNotFound` 序列化被测。`serde_json::Error`、`git2::Error`、`tokio::task::JoinError` 的转换逻辑虽然简单，但属于错误映射契约；若未来有人修改 `From<git2::Error>` 为 `AppError::IoKind`（例如为了统一 IO 错误），会破坏前端对 Git 错误的展示约定，当前测试无法守卫。
- 改法: 各增加一条断言：构造对应错误类型，验证转换后变体与消息内容。例如 `let e: AppError = serde_json::from_str::<Value>("").unwrap_err().into(); assert!(matches!(e, AppError::Serde(_)))`。

### P-5 [🟡] [覆盖度] settings.rs `app_data_dir()` 错误分支未覆盖
- 位置: `src-tauri/src/settings.rs:10-20`
- 代码片段:
```rust
pub(crate) fn app_data_dir() -> Result<PathBuf, AppError> {
    let exe = std::env::current_exe().map_err(|e| AppError::IoKind { ... })?;
    let exe_dir = exe.parent().ok_or_else(|| AppError::IoKind { ... })?;
    Ok(exe_dir.to_path_buf())
}
```
- 问题: `current_exe` 失败、`exe.parent()` 失败（理论上 `/` 根目录可执行文件无父目录）均返回 `AppError::IoKind`，但测试仅验证成功路径。两条错误分支对应便携分发场景下无法定位 exe 目录的降级行为，未覆盖。
- 改法: 错误分支在常规测试环境下难以直接触发，建议通过提取一个可注入的 `app_data_dir_from(exe: &Path)` 纯函数来测试；或标注为"依赖 OS 行为，由 L4 兜底"。
- 变异推演: 若将 `current_exe` 失败的错误消息从 `"无法获取可执行文件路径: {e}"` 改为固定字符串 `"unknown"`，当前测试不变红；增加错误分支测试后可捕获。

### P-6 [🟡] [覆盖度] `save_to_dir` / `save_settings` 的 `persist` 失败映射分支未覆盖
- 位置: `src-tauri/src/projects.rs:25-28`、`src-tauri/src/settings.rs:63-64`
- 代码片段:
```rust
tmp.persist(&projects_path).map_err(|e| AppError::IoKind {
    kind: format!("{:?}", e.error.kind()),
    message: format!("persist 失败: {e}"),
})?;
```
- 问题: `NamedTempFile::persist` 可能因目标目录不可写、磁盘满、权限等原因失败，当前映射把底层 `std::io::ErrorKind` 放进 `AppError::IoKind.kind`。该分支未覆盖，若映射逻辑写错（如丢掉 `e.error.kind()` 信息），L1 无法守卫。
- 改法: 可通过 mock `NamedTempFile` 或使用只读目录触发 persist 失败来补测；鉴于难以在跨平台稳定复现，至少应增加针对 `map_err` 闭包的单元测试（将闭包提取为可测试函数）。

### P-7 [🟡] [稳定性] settings.rs 测试依赖真实 `current_exe()` 目录
- 位置: `src-tauri/src/settings.rs:482-497`
- 问题: `app_data_dir_returns_exe_parent` 与 `app_data_dir_joins_settings_path` 调用 `std::env::current_exe()`，在测试运行时是测试二进制所在目录（`target/debug/deps/` 或类似），非生产 `slterminal.exe` 目录。这本身不会导致断言失败，但意味着测试并未真正验证"应用数据目录 = 生产 exe 同级"这一产品级约定，只是验证了函数实现等于 `current_exe().parent()`。
- 改法: 若要保持此约定，可接受；若希望完全隔离，应把路径提取为可注入参数。当前做法低风险，但需在报告中说明测试环境路径非生产路径。

### P-8 [🟢] [覆盖度] 未覆盖并发写、权限失败、超大 settings 等边界
- 位置: `src-tauri/src/settings.rs:38-73`、`src-tauri/src/projects.rs:15-30`
- 问题: 配置持久化模块 happy path 已覆盖，但未覆盖：
  1. 两个进程同时写 settings.json 的竞争场景；
  2. 目录/文件只读导致的写入失败；
  3. settings.json 超过常见大小（如 1MB）时的性能与正确性；
  4. 路径含 Unicode/空格/长路径（Windows `\?\` 前缀）的情况。
- 改法: 这些属于防御性测试，优先级低于命令包装层覆盖。可在 L1 增加 1-2 条高价值用例（如只读目录写入失败、大 JSON 往返）即可。

### P-9 [🟢] [既定豁免说明] lib.rs `run()` 函数未覆盖但合理
- 位置: `src-tauri/src/lib.rs:34-116`
- 问题: `run()` 包含 tracing 初始化、插件加载、`generate_handler!`、Tauri Builder 链式调用与启动失败退出逻辑，L1 无法直接测试。
- 改法: 无需改；由 L4 E2E 的"应正常启动并显示 slTerminal 标题"等用例覆盖。本问题仅作记录，不记为缺陷。

## 已做变异推演的用例清单

| 用例 | 推演篡改 | 是否会变红 | 原因 |
|------|---------|-----------|------|
| `settings.rs` 全部核心用例 | 删除 `save_settings` 中的 `.bak` 备份逻辑 | 否 | 测试不调用 `save_settings`，手动重写逻辑不含该分支 |
| `settings.rs` 全部核心用例 | 将 `load_settings` 返回类型改为 `Result<String, AppError>` | 否 | 测试不调用 `load_settings` |
| `te14_save_preserves_other_sections` / `te14_three_save_cycles_preserve_all_sections` | 将 `merge_settings` 改为深度合并 | 是 | 用例显式断言 `keybindings.terminal.paste` 应丢失（浅合并行为） |
| `te14_merge_preserves_nested_keys` | 同上 | 是 | 断言 `terminal.paste` 为 null 锁死浅合并 |
| `projects.rs` 全部 12 条用例 | 将 `save_projects` 去掉 `spawn_blocking` | 否 | 测试只调用 `save_to_dir` |
| `projects.rs` `load_corrupt_fallback_to_bak` | 删除 `load_from_dir` 的 `.bak` 回退逻辑 | 是 | 用例断言 `.bak` 恢复后主文件被修复 |
| `projects.rs` `save_creates_bak` | 删除 `save_to_dir` 的 `.bak` 复制逻辑 | 是 | 用例断言 `.bak` 存在且内容等于旧数据 |
| `error.rs` `test_from_io_error` | 将 `From<std::io::Error>` 改为 `AppError::Unknown` | 是 | 用例 match 断言变体为 `IoKind` |
| `error.rs` `test_session_not_found_serialization` | 将 `SessionNotFound` 的序列化去掉 camelCase | 是 | 断言 `sessionNotFound` 键存在 |
| `lib.rs` `test_ping_returns_pong` | 将 `ping()` 返回 `"pong2"` | 是 | 断言相等 `"pong"` |
| `lib.rs` `test_get_windows_build_number_returns_number` | 非 Windows 下返回 `Ok(1)` | 是 | 非 Windows 断言 `is_err()` |

