# L1 Rust 测试 Review：claude_history / fs / notify

## 元信息

| 项 | 内容 |
|----|------|
| 审查范围 | `src-tauri/src/claude_history/*`、`src-tauri/src/fs/mod.rs`、`src-tauri/src/notify/*` |
| 用例总数 | **121**（claude_history 56、fs 28、notify 24 + pool 13） |
| 审查方式 | 静态代码 + 覆盖率脚本 `docs/test-review-problem/coverage/rust-extract-uncovered.cjs` + 人工逐行核对 |
| 审查日期 | 2026-08-04 |

> 本次审查**未运行测试/构建**，所有结论基于源码与覆盖率工具的静态输出。行号以 `Read` 读取到的源码为准。

---

## 覆盖率缺口

| 等级 | 文件:行号 | 说明 | 类别 |
|------|-----------|------|------|
| 🟡 | `src-tauri/src/notify/mod.rs:62-157` | `FileWatcher::start()` 整体（debouncer 创建、watch 注册、事件循环、pause/resume、`need_rescan`、错误日志） | 既定豁免（`CLAUDE.md` 说明 L1 无 AppHandle，采用手动构造）/ 但核心逻辑零覆盖 |
| 🟡 | `src-tauri/src/notify/mod.rs:214-270` | `notify_watch` 命令（pool pause/resume、缓存命中、沙箱校验、新建 watcher、race 检查、insert） | 既定豁免（同上）/ 命令包装零覆盖 |
| 🔴 | `src-tauri/src/notify/pool.rs:66` | `LruWatcherPool::insert()` 中“同一 path 替换旧 watcher 时调用 `old_entry.watcher.stop()`”分支 | **真缺口** |
| 🟡 | `src-tauri/src/fs/mod.rs:48,105-106,175,199,233,276` | 各命令 `.await` 后 `TaskJoin(e)` 映射分支（spawn_blocking panic） | 真缺口，概率极低 |
| 🟡 | `src-tauri/src/fs/mod.rs:221` | `fs_delete` 路径不存在时返回 `AppError::IoKind` | 真缺口 |
| 🟡 | `src-tauri/src/fs/mod.rs` 多处属性行 | 如 `#[tauri::command]`（line 31、57、114、180、207、240 等） | 非代码，忽略 |
| 🟡 | `src-tauri/src/fs/mod.rs:519,547,572-575,581,612` | 位于 `write_file_tests` 测试模块内部 | 测试代码，豁免 |
| 🟡 | `src-tauri/src/claude_history/scan.rs:33-37` | `claude_history_scan` 命令包装（spawn_blocking、await、map_err） | 既定豁免（`CLAUDE.md` 说明命令包装不直测） |
| 🟡 | `src-tauri/src/claude_history/scan.rs:42,49,54,58` | `scan_sessions` 中 `resolve_projects_root` 无 home、`read_dir` 失败、条目非目录、项目目录不可读等降级路径 | 真缺口，低概率 |
| 🟡 | `src-tauri/src/claude_history/ops.rs:62-67` | `claude_history_delete` 命令包装 | 既定豁免 |
| 🟡 | `src-tauri/src/claude_history/ops.rs:43,48` | `locate_session_jsonl` 中 `read_dir(root)` 失败、`entry` 非目录分支 | 真缺口，低概率 |
| 🟡 | `src-tauri/src/claude_history/ops.rs:73` | `delete_session` 中 `resolve_projects_root()` 返回 `None` | 真缺口，低概率 |
| 🟢 | `src-tauri/src/claude_history/jsonl.rs:62,76,185,196` | 工具报告未覆盖，但阅读代码后对应 head 超限 break、缺 type continue、tail custom-title 空值过滤、tail 末尾。现有用例已触及对应分支，疑似覆盖率行号偏移或部分命中 | 观察项，不建议立即补用例 |

---

## 问题列表

### P-1 🟡 `fs/mod.rs` 写文件测试与实现同构，无法有效捕获实现偏差

- **位置**：`src-tauri/src/fs/mod.rs:write_file_tests`（约 492-632 行）
- **问题**：`write_file_tests` 模块自己重写了 `use_crlf` 检测与 `final_content` 转换逻辑，等于把被测函数内部的算法复制了一遍。

```rust
// 测试中的“期望生成逻辑”
let use_crlf = std::fs::File::open(&file_path).map_or_else(
    |_| cfg!(windows),
    |mut file| { ... contains("\r\n") }
);
let final_content = if use_crlf {
    content.replace("\r\n", "\n").replace('\n', "\r\n")
} else { content };
```

- **影响**：如果 `fs_write_file` 内部把 `map_or_else` 改为永远返回 `true` 或永远返回 `false`，测试里的检测也会同步“认为”原文件是 CRLF/LF，导致期望跟着变，单条用例改错可能仍绿。虽然 `lf_preserved_when_original_is_lf` 与 `new_file_defaults_to_crlf_on_windows` 两条组合仍能抓住“总是 CRLF/LF”的极端变异，但 `crlf_preserved_when_original_is_crlf` 本身对“总是 CRLF”不敏感。
- **建议**：将 `write_file_tests` 改为直接调用 `fs_write_file` 命令（类似 `command_wrapper_tests`），或使用固定输入/输出断言（例如写死原文件内容 CRLF、写死期望输出），不再复用被测算法。
- **维度**：断言有效性、结构与可维护性。

---

### P-2 🟡 `notify/pool.rs` `p10_insert_same_path_replaces_old_watcher` 未覆盖真正的替换分支

- **位置**：`src-tauri/src/notify/pool.rs:66`、`src-tauri/src/notify/pool.rs:p10_insert_same_path_replaces_old_watcher`
- **问题**：被测代码在 `insert` 中处理“同一 path 二次插入”时会自动 `stop()` 旧 watcher：

```rust
if let Some(mut old_entry) = self.entries.remove(&path) {
    old_entry.watcher.stop();   // line ~66
}
```

但测试先手动 `pool.remove(&path)`，再 `pool.insert(path, new_watcher)`，导致 `insert` 内部的 `remove -> stop` 分支永远不会被执行。

- **影响**：若有人误删或破坏 `old_entry.watcher.stop()`，现有 L1 仍全绿，真实场景下旧 watcher 线程/句柄可能泄漏。
- **建议**：去掉测试里手动的 `remove`，直接连续 `pool.insert(path, old)`、`pool.insert(path, new)`，并断言 `old.is_running() == false`（或验证 stop 信号已发送）。
- **维度**：断言有效性、测试覆盖度。

---

### P-3 🟡 `notify/mod.rs` `FileWatcher::start()` 与 `notify_watch` 命令在 L1 零覆盖

- **位置**：`src-tauri/src/notify/mod.rs:62-157`（`FileWatcher::start`）、`src-tauri/src/notify/mod.rs:214-270`（`notify_watch`）
- **问题**：核心生产逻辑——debouncer 创建、目录 watch 注册、事件循环、pause/resume 过滤、`need_rescan` 重扫、Tauri event 发射、路径沙箱校验——完全没有 L1 覆盖。`CLAUDE.md` 明确说明单元测试“无 AppHandle，采用手动构造 FileWatcher 字段”，因此这是**既定测试模式**，不是误测。
- **影响**：回归成本高。任何对 start/事件循环/沙箱逻辑的修改，只能依赖 L2（mock 了 `onFsEvent`）和 L4（真实文件监听）间接验证，定位问题慢。
- **建议**：
  1. 在文档中显式标记为“L1 已知豁免，必须由 L4 守卫”；
  2. 若可行，抽出一个 `EventEmitter` trait 或传入 `app_handle.emit` 闭包，使 L1 能在无真实 Tauri runtime 的情况下验证事件循环逻辑。
- **维度**：测试覆盖度、Mock 使用合理性（豁免但需文档化）。

---

### P-4 🟡 `fs/mod.rs` 多个正常异常路径未覆盖

- **位置**：`src-tauri/src/fs/mod.rs:221`（`fs_delete` 路径不存在）、`fs_create_dir` / `fs_delete` 的 root 外拒绝分支
- **问题**：
  - `fs_delete` 对“目标不存在”返回 `AppError::IoKind { kind: "path", ... }` 没有测试；
  - `fs_create_dir`、`fs_delete` 的“路径超出 project_root”沙箱拒绝未覆盖（`command_wrapper_tests` 只测了 `fs_read_file`、`fs_write_file`、`fs_rename`、`fs_read_dir` 的越界）；
  - 各命令 `spawn_blocking` 任务 panic 后的 `TaskJoin` 映射（line 48、105-106、175、199、233、276）未覆盖。
- **影响**：错误消息、沙箱行为、panic 传播路径回归风险。`TaskJoin` 分支属于极低概率，但“路径不存在”和“越界”是常见用户错误。
- **建议**：
  - 增加 `test_fs_delete_not_found_returns_path_error`；
  - 增加 `test_fs_create_dir_outside_root_rejected`、`test_fs_delete_outside_root_rejected`；
  - `TaskJoin` 分支可通过注入 panic 用例或文档豁免。
- **维度**：测试覆盖度、断言有效性。

---

### P-5 🟡 `claude_history` 命令包装与部分 IO 降级路径未覆盖

- **位置**：
  - `src-tauri/src/claude_history/scan.rs:33-37`（`claude_history_scan` 命令包装）
  - `src-tauri/src/claude_history/ops.rs:62-67`（`claude_history_delete` 命令包装）
  - `src-tauri/src/claude_history/scan.rs:42,49,54,58`（扫描根无 home、根不可读、条目非目录、项目目录不可读）
  - `src-tauri/src/claude_history/ops.rs:43,48,73`（`locate_session_jsonl` 失败路径、`resolve_projects_root` 无 home）
- **问题**：命令包装层（`spawn_blocking` + `await` + `map_err`）和若干 IO 失败降级路径未测试。`CLAUDE.md` 说明“命令包装不直测，由 L4 验收”，因此命令包装属于**既定豁免**；但 IO 降级路径（例如扫描根被移除、目录权限异常）是真实异常分支。
- **影响**：命令包装异常映射的回归无法被 L1 快速发现；IO 降级路径若被改错，可能导致扫描/删除在异常环境下崩溃而非优雅降级。
- **建议**：
  - 命令包装保持文档豁免；
  - 对 IO 降级路径增加构造性测试：设置只读/不存在的扫描根、损坏 env 等。
- **维度**：测试覆盖度、用例设计质量。

---

### P-6 🟡 `claude_history/scan.rs` 环境变量无 RAII 清理，panic 可能污染后续用例

- **位置**：`src-tauri/src/claude_history/scan.rs:163-169`、`src-tauri/src/claude_history/ops.rs:109-115`
- **问题**：测试通过 `set_scan_root(...)` / `unset_scan_root()` 手动成对设置 `SLTERM_CLAUDE_PROJECTS_DIR`。如果测试在 `set` 之后、`unset` 之前 panic，环境变量会残留，影响后续用例。虽然 L1 强制 `--test-threads=1` 且当前用例都会走到 `unset`，但这不是防呆设计。
- **影响**：在 CI 或未来新增测试时，一旦某个用例 panic，后续所有依赖默认 home 路径的用例可能产生误导性失败。
- **建议**：实现一个 RAII guard，例如：

```rust
struct ScanRootGuard {
    prev: Option<String>,
}
impl Drop for ScanRootGuard { ... }
```

- **维度**：稳定性风险、用例设计质量。

---

### P-7 🟡 `notify/mod.rs` `FileWatcher` Drop 测试依赖固定 `sleep(100ms)`，存在抖动风险

- **位置**：`src-tauri/src/notify/mod.rs:567`
- **问题**：`file_watcher_drop_stops_thread` 测试在 `drop(watcher)` 后 `sleep(Duration::from_millis(100))`，然后断言线程已结束。
- **影响**：在慢 CI runner 或高负载机器上，100ms 可能不足以让线程退出，导致偶发 flaky。
- **建议**：改为轮询等待 + 超时（最多 2-5s），例如：

```rust
let start = Instant::now();
while start.elapsed() < Duration::from_secs(2) && !handle.is_finished() {
    std::thread::sleep(Duration::from_millis(10));
}
assert!(handle.is_finished());
```

- **维度**：稳定性风险。

---

### P-8 🟡 `fs/mod.rs` 使用 `std::mem::transmute` 伪造 `tauri::State`，依赖内部布局

- **位置**：`src-tauri/src/fs/mod.rs:285-288`
- **问题**：测试 helper `as_tauri_state` 通过 unsafe transmute 把 `&AppState` 转成 `tauri::State<'_, AppState>`。代码已有 `// SAFETY:` 注释，但仍依赖 Tauri 内部表示不变。
- **影响**：Tauri 升级后如果 `State` 内部布局变化，测试可能 UB 或错误通过，且问题难定位。
- **建议**：把业务逻辑抽到不依赖 `State` 的纯函数（如 `fn do_read_file(root: Option<&Path>, path: &Path)`），命令层只做薄转发；`command_wrapper_tests` 保留对命令入口的覆盖即可。
- **维度**：Mock 使用合理性、结构与可维护性。

---

### P-9 🟢 `notify/pool.rs` `p9_drop_stops_all_watchers` 没有任何断言

- **位置**：`src-tauri/src/notify/pool.rs:303-307`
- **问题**：测试只创建 pool 然后 drop，没有验证 `Drop` 是否调用了 `stop_all()` 或 watcher 是否停止。
- **影响**：如果 `LruWatcherPool` 的 `Drop` 实现被误删，该测试仍然全绿。
- **建议**：在测试 watcher 中记录 `stop()` 调用次数，或 drop 后断言 `pool.is_empty()` / 所有 watcher 已停止。
- **维度**：断言有效性。

---

### P-10 🟢 `claude_history/scan.rs` `scan_multiple_sessions_sorted_input_order` 命名与断言易误导

- **位置**：`src-tauri/src/claude_history/scan.rs:240-260`
- **问题**：测试对返回结果排序后再比较，实际上不验证扫描顺序；但用例名暗示“测试顺序”。`CLAUDE.md` 已明确扫描顺序无契约，排序/分组是前端职责。
- **影响**：新开发者可能误以为后端保证顺序，导致误改或误报。
- **建议**：重命名为 `scan_collects_all_sessions_across_dirs`，断言使用集合或只比较内容与数量。
- **维度**：结构与可维护性。

---

### P-11 🟢 `claude_history/ops.rs` 空串 UUID 验证的消息断言恒真

- **位置**：`src-tauri/src/claude_history/ops.rs:139-148`
- **问题**：测试循环里 `assert!(msg.contains(bad), ...)`，当 `bad = ""` 时恒为 `true`，空串输入并未真正验证错误消息内容。不过 `is_uuid_filename("")` 本身已由长度分支拒绝，功能正确。
- **影响**：消息内容的回归（例如错误提示漏了“非法”）对空串输入不会被捕获。
- **建议**：对空串单独断言 `msg.contains("非法")`，或从非法输入列表中排除空串的消息检查。
- **维度**：断言有效性。

---

## 核心用例变异推演（抽样）

| 用例/区域 | 变异假设 | 是否会变红 | 原因 |
|-----------|----------|------------|------|
| `jsonl.rs` `head_prompt_truncated_to_200_chars` | `PROMPT_MAX_CHARS` 改为 100 | 🔴 | 断言 `chars().count() == 200` 失败 |
| `jsonl.rs` `head_cwd_takes_first_non_empty_line` | `parse_head` 取最后一条 cwd | 🔴 | cwd 变成第二条路径，断言失败 |
| `jsonl.rs` `head_titles_last_wins` | 同类型标题取第一条 | 🔴 | `ai_title` 与 `summary` 断言失败 |
| `jsonl.rs` `head_limit_stops_before_mid_file_title` | `HEAD_SCAN_LIMIT_BYTES` 扩大到 1MB | 🔴 | 中部 custom-title 被读到，标题断言失败 |
| `jsonl.rs` `tail_scan_finds_title_in_last_64kb_of_large_file` | `TAIL_SCAN_BYTES` 缩小到 1KB | 🔴 | 尾部 custom-title 不在窗口内 |
| `jsonl.rs` `tail_title_custom_prefers_ai_even_if_ai_later` | 取消 custom 优先级，改为纯 last-wins | 🔴 | 结果变成后面的 ai-title |
| `scan.rs` `scan_excludes_agent_non_uuid_subagents` | 不检查 `agent-*` 前缀 | 🔴 | 返回 agent 文件，断言长度/IDs 失败 |
| `scan.rs` `scan_corrupt_jsonl_produces_degraded_entry` | 解析失败改为 panic 或返回 Err | 🔴 | 测试捕获不到降级条目 |
| `ops.rs` `delete_stays_within_scan_root` | 直接用 `root.join(session_id)` 定位 | 🔴 | 越界哨兵文件不会被触碰，但目标文件也找不到，delete 失败 |
| `ops.rs` `validate_session_id_rejects_non_uuid_inputs` | 校验改为恒通过 | 🔴 | `unwrap_err()` panic |
| `fs/mod.rs` `test_fs_rename_rejects_existing_directory` | 允许覆盖目录 | 🔴 | 断言 `dst_path.exists()` 会失败 |
| `fs/mod.rs` `test_fs_read_file_outside_root_rejected` | 移除沙箱 | 🔴 | 返回内容而非错误 |
| `fs/mod.rs` `crlf_preserved_when_original_is_crlf`（单独）| `fs_write_file` 永远转 CRLF | 🟢 | 测试自身检测原文件为 CRLF，期望 CRLF，仍绿；需与 `lf_preserved_when_original_is_lf` 组合才能抓 |
| `fs/mod.rs` `new_file_defaults_to_crlf_on_windows` | 新文件默认改为 LF | 🔴 | Windows 下断言 CRLF 失败 |
| `notify/pool.rs` `p4_insert_at_capacity_evicts_lru` | 淘汰策略改为 MRU | 🔴 | `a`（最久未用）不会被移除，断言 `!contains(a)` 失败 |
| `notify/pool.rs` `p3_get_updates_last_used_reordering_lru` | `get()` 不更新 last_used | 🔴 | `a` 变成最旧，被 evict，断言 `contains(a)` 失败 |
| `notify/pool.rs` `p10_insert_same_path_replaces_old_watcher` | 删除 `old_entry.watcher.stop()` | 🟢 | 测试先手动 `remove`，不会触发 insert 内替换分支；按 P-2 建议修改后会变红 |
| `notify/mod.rs` `classify_create_file` | `Create` 被错误映射为 `Remove` | 🔴 | `payload.kind` 断言失败 |

---

## 结论

本次审查共识别 **11 项问题**，其中 **3 项建议优先修复**：

1. **`fs/mod.rs` 写文件测试与实现同构（P-1）**：直接影响 CRLF 保持契约的回归防护，应改为直接调用命令或固定输入/输出断言。
2. **`notify/pool.rs` 替换旧 watcher 分支未覆盖（P-2）**：可能导致 watcher 线程泄漏，修复成本低（去掉测试中的手动 `remove` 即可）。
3. **`notify/mod.rs` 核心事件循环零覆盖（P-3）**：虽为既定豁免，但应在文档/CI 中明确由 L4 守卫，并考虑抽 trait 让 L1 可测。

其余问题多为异常路径未覆盖、测试断言偏弱或稳定性风险，可按优先级逐步补齐。
