# 03 L1 hooks 测试 Review

## 元信息

- **领域**: Rust 后端 hooks 模块（`src-tauri/src/hooks/`）
- **测试位置**:
  - `src-tauri/src/hooks/mod.rs` 内 `#[cfg(test)]`
  - `src-tauri/src/hooks/signal.rs` 内 `#[cfg(test)]`
  - `src-tauri/src/hooks/watcher.rs` 内 `#[cfg(test)]`
  - `src-tauri/src/hooks/inject.rs` 内 `#[cfg(test)]`
  - `src-tauri/src/hooks/usage.rs` 内 `#[cfg(test)]`
  - `src-tauri/src/hooks/config.rs` 内 `#[cfg(test)]`
- **用例数**: 100 条
- **覆盖率概况**: 生产代码行覆盖 **1106 / 1465 = 75.5%**
  - `mod.rs`: 61/78 = 78.2%
  - `signal.rs`: 70/88 = 79.5%
  - `watcher.rs`: 154/225 = 68.4%
  - `inject.rs`: 296/493 = 60.0%
  - `usage.rs`: 299/309 = 96.8%
  - `config.rs`: 226/272 = 83.1%
- **审查日期**: 2026-08-04
- **审查人**: Claude Code（静态审查，未运行测试）

## 覆盖率缺口

按业务风险分级。`#[cfg(test)]` 区域内的未覆盖行属于“既定豁免”，不列入生产缺口。

### 核心逻辑零覆盖（🔴）

| 位置 | 说明 | 风险 |
|------|------|------|
| `mod.rs:63–84` | `start_signal_watcher` 全局静态 watcher 启动/幂等/错误降级 | 信号监听入口，L1 未调用 |
| `signal.rs:52–79` | `process_signal_file` 读文件 -> 解析 -> emit -> 删除全流程 | watcher 实际消费信号时调用，核心流程未测 |
| `watcher.rs:46–136` | `HookSignalWatcher::start` 双通道事件循环（notify + 3s 轮询补漏） | win10 实证修复的关键兜底逻辑未在 L1 验证 |
| `watcher.rs:156–160` | `get_signal_dir` 及 home 目录失败分支 | 信号目录解析未测 |
| `inject.rs:191–274` | `hooks_inject` Tauri 命令 | 注入入口：settings.json merge、原子写、非法 JSON 中止 |
| `inject.rs:280–351` | `hooks_uninstall` Tauri 命令 | 卸载入口：handler 级剔除、目录删除 |
| `inject.rs:358–423` | `hooks_injection_status` Tauri 命令 | 状态检测入口：版本比对、三态判定 |

### 边界分支未覆盖（🟡）

| 位置 | 说明 | 风险 |
|------|------|------|
| `usage.rs:34–42` | `hooks_context_usage` async 命令包装 / spawn_blocking / TaskJoin 映射 | 命令层未测，P2-TE-05 实际只测了 `scan_transcript_usage` |
| `config.rs:66–68` | user 层 `home_dir()` 解析失败分支 | 极端环境可能失败，无回归守卫 |
| `config.rs:94`, `121`, `145–147` | IO 错误分支（read/write/persist 失败） | 只读目录/权限问题无回归守卫 |
| `config.rs:154–176`, `182–207` | `hooks_config_read` / `hooks_config_write` async 命令包装 | 参数透传、spawn_blocking 错误映射未测 |
| `inject.rs:58–62`, `74` | `disk_script_version` / `template_version` 解析失败分支 | 版本字符串异常时行为无回归守卫 |
| `inject.rs:176–179` | `inject_matchers` 遇到 `hooks` 字段非数组时的降级分支 | 畸形配置可能触发 |

### 低风险未覆盖（🟢）

- `watcher.rs:319–330` 等 `#[cfg(test)]` 测试辅助代码自身分支——不计入生产缺口。
- 部分仅做 panic/分支的 match arm（如 `serde_json::from_str` 后的 `Err`）在纯函数测试中难以触发，可在集成测试补齐。

## 问题列表

### P-01 🔴 `process_signal_file` 文件处理全流程零 L1 覆盖

- **位置**: `src-tauri/src/hooks/signal.rs:52–79`
- **代码**:
  ```rust
  pub fn process_signal_file<P: AsRef<Path>>(
      path: P,
      app_handle: &AppHandle,
  ) -> Result<(), AppError> {
      let path = path.as_ref();
      let content = fs::read_to_string(path)?;
      if let Some(payload) = parse_signal_file(&content) {
          let _ = app_handle.emit("hook-event", &payload);
      }
      fs::remove_file(path)?;
      Ok(())
  }
  ```
- **问题**: `parse_signal_file` 的 9 条单元测试只验证了纯解析函数，没有验证“读文件 -> emit -> 删除”的编排。watcher 真正调用的是 `process_signal_file`，其内部错误路径（读失败、emit 失败、删失败）全部无回归守卫。
- **修复建议**: 在 `tempdir` 中创建 `.json` 信号文件，手动构造 `AppHandle` stub 或抽取 `emit` 回调后调用 `process_signal_file`，断言文件被删除、emit 收到正确 payload。
- **变异推演**: 若把 `if let Some(payload) = parse_signal_file(&content)` 改为 `let _ = parse_signal_file(&content)`（即不 emit），当前测试仍全绿。

### P-02 🔴 三个 Tauri 注入/卸载/状态命令无 L1 单元测试

- **位置**: `src-tauri/src/hooks/inject.rs:191–274`、`280–351`、`358–423`
- **代码**（以 `hooks_inject` 为例）:
  ```rust
  #[tauri::command]
  pub async fn hooks_inject() -> Result<HookInjectionStatus, AppError> {
      spawn_blocking(inject_impl).await?
  }
  ```
- **问题**: 命令层涉及真实 `~/.claude/settings.json` 读写、非法 JSON 中止、原子写、`remove_dir_all`、版本比对等复杂行为，但 L1 仅测试了底层 helper（`remove_slterm_matchers`、`inject_matchers` 等）。这些命令的端到端行为仅由 L4 E2E 兜底，回归周期长、定位慢。
- **修复建议**: 将 `inject_impl`/`uninstall_impl`/`status_impl` 抽出为可注入路径（`home_dir`、`settings_path` 作为参数）的同步函数；L1 在 `tempdir` 中构造 home/settings 路径并直接调用，覆盖非法 JSON 中止、幂等注入、卸载保留用户 hook、版本过旧等场景。
- **变异推演**: 若把 `inject_impl` 中非法 JSON 时返回 `Err` 改为返回 `Ok(HookInjectionStatus { status: InjectionStatus::Injected, version: None })`，当前 L1 全绿。

### P-03 🔴 `HookSignalWatcher::start` 双通道事件循环无 L1 覆盖

- **位置**: `src-tauri/src/hooks/watcher.rs:46–136`
- **代码**:
  ```rust
  pub fn start(app_handle: AppHandle) -> Result<Self, AppError> {
      // ... 创建 debouncer、spawn "hook-signal-watcher" 线程 ...
      loop {
          if stop_rx.try_recv().is_ok() { break; }
          // notify 实时通道
          if let Ok(events) = event_rx.try_recv() { ... }
          // 3s 轮询补漏
          if last_poll.elapsed() >= POLL_INTERVAL { ... }
          thread::sleep(LOOP_TICK);
      }
  }
  ```
- **问题**: 双通道架构（notify 实时 + 3s 轮询补漏、目录删除后自动重建、notify 失败降级为纯轮询）是 win10 实证修复的核心，但 L1 只测试了 `collect_signal_files`、`poll_once` 纯函数和手动构造生命周期的 `stop`/`drop`。真实事件循环的通道交互、降级、重建均未验证。
- **修复建议**: 将事件循环体拆分为可单测的纯函数 `run_one_tick(events, now, last_poll, signal_dir) -> (actions, next_last_poll)`；或提供集成测试在临时目录中真实启动 watcher，写入 `.json` 文件后断言 `app_handle` 收到事件。
- **变异推演**: 若把 `POLL_INTERVAL` 改为 `Duration::from_secs(0)` 或删除 `create_dir_all(signal_dir)` 的重建逻辑，当前 L1 全绿。

### P-04 🔴 `start_signal_watcher` 全局启动入口无 L1 覆盖

- **位置**: `src-tauri/src/hooks/mod.rs:63–84`
- **代码**:
  ```rust
  pub fn start_signal_watcher(app_handle: AppHandle) {
      let mut guard = WATCHER.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
      if guard.is_some() {
          return;
      }
      match HookSignalWatcher::start(app_handle) {
          Ok(watcher) => { *guard = Some(watcher); }
          Err(e) => { tracing::warn!("..."); }
      }
  }
  ```
- **问题**: 全局静态 `WATCHER` 的幂等启动、锁中毒降级、启动失败日志路径均未在 L1 调用。若幂等检查被误改，会导致重复创建线程/句柄泄漏。
- **修复建议**: 提供测试钩子重置 `WATCHER`（或重构为可注入状态），调用 `start_signal_watcher` 两次，断言第二次不创建新 watcher；模拟 `HookSignalWatcher::start` 失败路径断言不 panic。
- **变异推演**: 若移除 `if guard.is_some() { return; }`，重复启动会产生多个 watcher，当前 L1 全绿。

### P-05 🟡 `hooks_context_usage` async 命令包装未覆盖

- **位置**: `src-tauri/src/hooks/usage.rs:34–42`
- **代码**:
  ```rust
  #[tauri::command]
  pub async fn hooks_context_usage(
      _app: AppHandle,
      transcript_path: String,
  ) -> Result<Option<ContextUsage>, AppError> {
      spawn_blocking(move || scan_transcript_usage(&transcript_path)).await?
  }
  ```
- **问题**: P2-TE-05 的 5 条用例标题看似端到端，实际只断言了 `scan_transcript_usage` 纯函数；命令包装层的参数透传、`spawn_blocking` 错误映射、`TaskJoin` 转换均未测。
- **修复建议**: 将 P2-TE-05 改为直接 `await hooks_context_usage(app_handle, path)`，或新增独立用例验证命令返回值与 `scan_transcript_usage` 一致。
- **变异推演**: 若把 `spawn_blocking(...).await?` 的 `?` 错误映射改为吞掉错误并返回 `Ok(None)`，当前 L1 全绿。

### P-06 🟡 config 读写命令包装及 IO 异常分支未覆盖

- **位置**: `src-tauri/src/hooks/config.rs:66–68`、`94`、`121`、`145–147`、`154–176`、`182–207`
- **代码**:
  ```rust
  HooksLayer::User => dirs::home_dir()
      .map(|h| h.join(".claude").join("settings.json"))
      .ok_or_else(|| AppError::IoKind(std::io::Error::other("..."))),
  ```
  ```rust
  pub async fn hooks_config_read(...) -> Result<Value, AppError> {
      let path = resolve_config_path(layer, project_path, &state).await?;
      spawn_blocking(move || read_hooks_subtree(&path)).await?
  }
  ```
- **问题**: `home_dir()` 失败、`read_hooks_subtree`/`write_hooks_subtree` 中除 `NotFound` 外的 IO 错误、`NamedTempFile::persist` 失败、命令包装层的参数透传均未测。`write_hooks_subtree` 的“损坏 JSON 拒绝覆盖”测试覆盖了主要契约，但命令层的 async 包装没有。
- **修复建议**: 增加对 `hooks_config_read`/`hooks_config_write` 命令函数的 await 调用；用只读目录模拟 `persist` 失败；通过临时 HOME 环境覆盖 `dirs::home_dir()` 失败路径。
- **变异推演**: 若把 `home_dir()` 失败时的 `AppError::IoKind` 改为 `AppError::Validation`，当前 L1 全绿。

### P-07 🟡 config user 层测试依赖真实 home 目录

- **位置**: `src-tauri/src/hooks/config.rs:233–238` 附近测试 `user_layer_resolves_to_home_claude_settings`
- **代码**:
  ```rust
  let path = resolve_config_path(HooksLayer::User, None, &state).await.unwrap();
  assert!(path.ends_with(".claude/settings.json"));
  ```
- **问题**: 测试直接调用 `resolve_config_path(HooksLayer::User, ...)`，内部使用 `dirs::home_dir()`，未隔离真实用户环境。在 CI 或特殊 HOME 配置下可能得到意外路径，且无法注入 HOME 失败场景。
- **修复建议**: 将 `resolve_config_path` 的 home 解析抽出为可注入函数，或在测试前置设置临时 `HOME` 环境变量并断言路径前缀。
- **变异推演**: 不适用（环境依赖问题）。

### P-08 🟡 `inject_adds_10_events` 弱断言未守卫 matcher 结构

- **位置**: `src-tauri/src/hooks/inject.rs:651–661`
- **代码**:
  ```rust
  for event in HOOK_EVENTS {
      let arr = settings["hooks"][event].as_array().unwrap();
      assert_eq!(arr.len(), 1);
      assert!(arr[0].get("matcher").is_some());
  }
  ```
- **问题**: 只检查键存在和数量，未断言每个事件下 hooks 数组的实际字段结构（`type="command"`、`timeout=5`、command 含脚本绝对路径）。`build_matcher_entry` 的行为缺陷无法被捕获。
- **修复建议**: 对每个 `arr[0]` 断言 `type == "command"`、`matcher == ""`、`timeout == 5`、`command` 字符串包含 `slterm-hook-reporter`。
- **变异推演**: 若把 `build_matcher_entry` 的 `timeout` 改为 `10` 或把 `matcher` 改为 `"*"`，本测试仍绿。

### P-09 🟡 serde camelCase 测试使用 `contains` 弱断言

- **位置**: `src-tauri/src/hooks/mod.rs:98–144`；`src-tauri/src/hooks/signal.rs:144–174`
- **代码**:
  ```rust
  let json = serde_json::to_string(&status).unwrap();
  assert!(json.contains("\"status\":"));
  assert!(json.contains("\"version\":"));
  ```
- **问题**: `contains` 只能验证字段名存在，无法捕获字段值或类型错误。例如 `version` 字段被序列化为字符串 `"1"` 而不是数字 `1` 时，断言仍可能通过。
- **修复建议**: 序列化后反序列化并精确断言字段值；DTO 已提供 `Deserialize` derive，可直接 `serde_json::from_str`。
- **变异推演**: 若把 `HookInjectionStatus.version` 类型改为 `String`（值为 `"1"`），`contains("\"version\":")` 仍绿。

### P-10 🟡 usage.rs P2-TE-05 用例与 `scan_transcript_usage` 集成用例重复

- **位置**: `src-tauri/src/hooks/usage.rs:371–483` 与 `256–318`
- **问题**: P2-TE-05 的 5 条用例（多条 usage 返最后、末尾无 usage 返 None、损坏行跳过、空文件、大文件尾部扫描）与前面的 `scan_transcript_usage` 集成用例覆盖相同分支，未增加命令层覆盖，造成维护负担。
- **修复建议**: 要么合并为同一组并改名为 `scan_transcript_usage_*`，要么将 P2-TE-05 改为测试 `hooks_context_usage` 命令函数。
- **变异推演**: 不适用。

### P-11 🟢 watcher 生命周期测试无结束后状态断言

- **位置**: `src-tauri/src/hooks/watcher.rs:337–348`、`350–361`
- **代码**:
  ```rust
  #[test]
  fn watcher_stop_is_idempotent() {
      let mut w = make_test_watcher();
      w.stop();
      w.stop(); // 仅验证不 panic
  }
  ```
- **问题**: 仅验证 `stop()` 调用不 panic，未验证底层线程确实已 join/结束。若实现改为“发信号但不 join”，测试仍通过。
- **修复建议**: 调用后 `assert!(handle.is_finished())`，并在 `Drop` 测试中验证线程已退出。
- **变异推演**: 若 `stop()` 只发信号不 join，当前测试仍绿。

### P-12 🟢 `handler_contains_slterm` 非字符串 command 分支未覆盖

- **位置**: `src-tauri/src/hooks/inject.rs:98–102`
- **代码**:
  ```rust
  fn handler_contains_slterm(handler: &Value) -> bool {
      handler.get("command")
          .and_then(|c| c.as_str())
          .map(|c| c.contains("slterm-hook-reporter"))
          .unwrap_or(false)
  }
  ```
- **问题**: 现有测试 `command` 均为字符串；未验证 `command` 缺失、为 `null`、为数组/对象时是否返回 `false` 且不 panic。此类防御性分支缺失回归守卫。
- **修复建议**: 增加一条用例构造 `{"hooks":[{"command":null}]}`、`{"hooks":[{"command":["node"]}]}`、`{"hooks":[{}]}`，断言 `has_slterm_matchers` 返回 `false`。
- **变异推演**: 若把 `and_then(|c| c.as_str())` 改为 `and_then(|c| Some(c.as_str().unwrap_or("")))`，遇到非字符串 command 时会 panic，当前 L1 全绿。

### P-13 🟢 `.claude/test-inventory.md` 对 hooks 模块描述含 stale 条目

- **位置**: `.claude/test-inventory.md` hooks 行描述中的“notification权限声明”
- **问题**: hooks 模块代码与测试中均不存在“notification 权限声明”相关逻辑；inventory 描述与代码不符，会误导后续审计。
- **修复建议**: 删除或修正 inventory 中 hooks 模块描述，使其与当前 `hooks_inject`/`hooks_uninstall`/`hooks_injection_status`/`hooks_context_usage`/`hooks_config_read`/`hooks_config_write` 六个命令对应。
- **变异推演**: 不适用。

## 已做变异推演的用例清单

以下用例在脑中进行了“若源码发生某种错误变异，该用例是否仍能捕获”的推演。

| 用例 | 来源文件 | 推演变异 | 是否仍能捕获 | 说明 |
|------|----------|----------|--------------|------|
| `parse_signal_file` 空 panel_id 返回 None | `signal.rs` | 把 `payload.panel_id.is_empty()` 改为恒 `false` | 是 | 空串会被错误接受 |
| `parse_signal_file` 缺 panel_id 返回 None | `signal.rs` | 给 `panel_id` 加 `#[serde(default)]` | 是 | 缺失字段会被错误接受 |
| `remove_slterm_matchers` 同组保留用户 hook | `inject.rs` | 改回组级删除（删除整个 matcher） | 是 | 用户 hook 会丢失，len 变 0 |
| `inject_idempotent` | `inject.rs` | 去掉幂等检查，总是追加 | 是 | 二次注入后长度大于首次 |
| `parse_cache_fields_missing_defaults_to_zero` | `usage.rs` | 去掉 cache 字段的 `unwrap_or(0)` | 是 | 旧格式解析返回 None |
| `scan_finds_usage_in_last_line` | `usage.rs` | 从首行正向扫描 | 是 | 返回的 tokens 变成首行 10 而非末行 100 |
| `hooks_context_usage_large_file_tail_scan` | `usage.rs` | 把 `TAIL_SCAN_BYTES` 改小至 usage 不在窗口内 | 是 | 命令返回 None |
| `write_corrupt_json_rejected_and_not_overwritten` | `config.rs` | 损坏 JSON 时仍覆盖写盘 | 是 | 原文件内容被改写，断言失败 |
| `write_preserves_other_root_fields` | `config.rs` | `write_hooks_subtree` 改为覆盖整文件 | 是 | `$schema`/`permissions` 等字段丢失 |
| `project_layer_outside_root_path_not_allowed` | `config.rs` | `resolve_config_path` 跳过沙箱校验 | 是 | 越界路径被接受 |
| `inject_adds_10_events` | `inject.rs` | `build_matcher_entry` 的 `timeout` 改为 10 | 否 | 当前断言未检查 timeout 值 |
| `watcher_stop_is_idempotent` | `watcher.rs` | `stop()` 只发信号不 join | 否 | 当前断言只检查不 panic |

## 最终摘要

本次审查发现 **13** 个问题：🔴 4 个、🟡 6 个、🟢 3 个。最关键的 3 个问题是：

1. **信号处理核心链路未测**：`process_signal_file` 与 `HookSignalWatcher::start` 双通道事件循环在 L1 完全空白，win10 轮询补漏的关键兜底仅靠 E2E 验收。
2. **Tauri 命令层大面积零覆盖**：`hooks_inject`/`hooks_uninstall`/`hooks_injection_status`/`start_signal_watcher` 均未在 L1 调用，注入/卸载的幂等、非法 JSON 中止、版本比对等契约缺乏快速回归守卫。
3. **用例断言偏弱且存在重复**：`inject_adds_10_events` 仅检查键存在、`serde` 测试用 `contains`、P2-TE-05 与 `scan_transcript_usage` 测试重复，导致缺陷（如字段类型错误、timeout 被改）无法被 L1 捕获。
