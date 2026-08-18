# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 职责

文件系统监听模块——基于 `notify-debouncer-full`，递归监听项目目录的文件变更，去抖 300ms 后通过 Tauri Event 广播到前端。

## 架构决策

**Watcher 池 + pause/resume（非 stop/start）**：Windows 上 `notify` 调用 `ReadDirectoryChangesW` 递归注册目录树。大目录（如 `target/` 26K 文件）首次注册耗时约 2s。`LruWatcherPool` 缓存 `WATCHER_POOL_CAPACITY = 8` 个 watcher（BE-11：原 5→8，覆盖多项目快速切换；pause/resume 既定机制保留），切换项目时通过 `AtomicBool` pause/resume 替代销毁/重建，命中缓存时延迟 < 1ms。

**Debouncer 生命周期**：`notify-debouncer-full` 的 `Debouncer` 实例存活于 `"fs-watcher"` 线程内。线程退出时自动 drop→stop。不支持跨线程访问 debouncer API。

**暂停 ≠ 停止**：`paused` 标记仅阻止事件上报前端（经 `EventEmitter`）。watcher 线程和 debouncer 继续运行，OS 文件监听句柄保持有效。

**事件分类纯函数化**：`classify_event()` 原依赖 `DebouncedEvent`（字段不公开，不可测试）。现拆分为编排层 `classify_event(event: DebouncedEvent)` + 纯函数 `classify_by_kind(kind: &EventKind, paths: Vec<String>) -> FsEventPayload`。`EventKind` 是 notify crate 公开枚举，可直接构造测试——覆盖全部 7 种 EventKind 及子类型。

**事件侧排除大目录（BE-02，D8 定稿）**：`WATCH_EXCLUDE_DIRS = ["node_modules", "target", ".venv", "venv", "dist", ".git", "__pycache__"]`（七元素）——事件循环中过滤任一路径分量命中排除集的事件（`is_excluded_path` 纯函数）。**仅事件侧过滤**——notify 不支持目录级排除，watcher 仍注册全树；`need_rescan` 分支不受影响（只发 watch root）。`fs_read_dir` 不动（懒加载既定决策）。改排除集后跑 `is_excluded_path_matches_all_seven_dirs` + `event_loop_filters_excluded_paths_keeps_normal` + `event_loop_rescan_bypasses_exclusion_filter`。

**symlink 过滤（SEC-08，S05）**：事件路径 `symlink_metadata` 检查（`is_symlink_path`——上溯全部祖先），命中 symlink 的事件不 emit（防项目内 symlink 致外部路径经 fs-event 泄露）；`need_rescan` 只发 watch root，不受影响。L1 测试（symlink 创建失败 skip 约定同 SEC-02，`#[cfg(windows)]` 豁免登记——BE-17/D5）。

**fs-event 批量合并上限（BE-07，S13）**：已有 300ms debounce——补单批 paths 数上限 `FS_EVENT_PATH_BATCH_LIMIT = 100`：单事件 paths 数严格超限 → 合并为单条 `Rescan` 载荷下发（携带监听根路径，不再逐路径推送；防大目录整体删除/移动的巨型批次打爆前端）。agent-event 为低频控制事件不节流（评估结论 S19 登记：误伤状态机，前端消费侧不加节流）。

**EventEmitter trait 抽离（HFN-03）**：`FileWatcher` 原直接持 `AppHandle` 调 `emit`，事件循环零 L1（豁免"无 AppHandle"）。现抽 `EventEmitter` trait（`emit_fs_event(payload)`），生产实现 `AppHandleEmitter` 包装 `AppHandle::emit`（发送失败静默忽略）。`FileWatcher::start()` = `start_with_emitter(Box::new(AppHandleEmitter{..}), ..)` 的薄包装；L1 用 `MockEmitter` 注入驱动事件循环（debouncer 创建、watch 注册、pause/resume、emit 全链路可测），`notify_watch` 的沙箱校验/pool 交互分支同步补测。

## 文件

| 文件 | 职责 |
|------|------|
| `mod.rs` | `FileWatcher` 结构体 + `notify_watch`/`notify_stop_watch` Tauri 命令 + `classify_event` 事件分类（委托纯函数 `classify_by_kind`）+ 排除过滤（BE-02）/symlink 过滤（SEC-08）/批量合并（BE-07） |
| `pool.rs` | `LruWatcherPool` — LRU 淘汰的 watcher 缓存池（`WATCHER_POOL_CAPACITY=8`，BE-11）+ `remove(path)`（BE-10） |

## FileWatcher

```
FileWatcher {
    stop_tx / thread_handle — 停止信号 + 线程句柄
    watch_paths: Arc<Mutex<Vec<PathBuf>>> — 监听根路径（struct 字段仅生命周期绑定，watcher 线程持 Arc clone）
    paused: Arc<AtomicBool> — 暂停事件上报
}
```

- `start(app_handle, paths, debounce_ms)` — 薄包装：包 `AppHandleEmitter` 后委托 `start_with_emitter`
- `start_with_emitter(emitter: Box<dyn EventEmitter>, paths, debounce_ms)` — 注入 EventEmitter 的启动入口（D2 参数注入：L1 用 mock emitter 驱动，无需 AppHandle）→ 创建 debouncer + 注册 watch → spawn "fs-watcher" 线程
- `stop()` — 发送停止信号 → join 线程（幂等）
- `pause()` / `resume()` / `is_paused()` — AtomicBool 控制
- `Drop` — 发送停止信号 + join 线程，确保 OS 句柄释放

## `notify_watch` / `notify_stop_watch` 命令

```
前端 startWatch(path)
  → pool.pause_all_except(path)  // 暂停其他 watcher
  → pool.get(path) 命中？→ 返回（resume 生效）
  → 未命中 → FileWatcher::start() → pool.insert()  // 池满则 LRU 淘汰

前端 stopWatch(path)（项目移除/切换时调用，BE-10）
  → notify_stop_watch(path) → pool.remove(path)  // 移除并 stop，不再占用池槽与 OS 句柄
```

## 关键约束

- **watcher 重建开销**：不要在每次页面切换时 `stop()` + `start()` watcher。始终走池的 `pause_all_except`。
- **停止语义**：项目移除/切换时须调 `notify_stop_watch`（前端 `stopWatch`）——只 pause 不 stop 会让旧 watcher 占用池槽至 LRU 淘汰（BE-10）。
- **路径规范化**：池 key 使用 `dunce::simplified()` 处理，与 `fs_read_dir` 保持一致。
- **Drop 保证**：`LruWatcherPool::drop()` → `stop_all()` → 遍历 join 所有线程。AppState 销毁时自动触发。
- **路径 sandbox**：`notify_watch` 在创建 watcher 前校验 `validate_path_within_root()`（该函数已从 `fs/mod.rs` 迁移至 `crate::state`）。

## 测试模式

Rust 测试分布在 2 个位置：

| 位置 | 类型 | 用例数 |
|------|------|--------|
| `notify/mod.rs` `#[cfg(test)]` | 单元测试 | 45（含 BE-02 排除 3 + SEC-08 symlink 3 + BE-07 合并 2） |
| `notify/pool.rs` `#[cfg(test)]` | 单元测试 | 15（含 BE-10 remove + BE-11 容量 8） |

### EventEmitter mock 注入（HFN-03）

`FileWatcher::start()` 的生产路径需要 `AppHandle`，L1 改用 `start_with_emitter` 注入 `MockEmitter`（`#[cfg(test)]` 实现，`Arc<MockEmitter>` 记录 emit 调用）驱动事件循环，无需 AppHandle：

```rust
let emitter = Arc::new(MockEmitter::default());
let mut watcher = FileWatcher::start_with_emitter(Box::new(emitter.clone()), paths, 50).unwrap();
```

事件循环测试（mod.rs）：`event_loop_emits_classified_payload`（真实目录 + mock emitter 全链路）、`event_loop_paused_skips_emit_resume_recovers`（pause/resume）、`event_loop_errors_logged_loop_continues`（错误不终止循环）、`event_loop_stop_signal_exits` / `event_loop_sender_disconnected_exits`、`start_with_emitter_real_dir_emits_on_change`（真实临时目录集成）+ **BE-02 排除过滤（`is_excluded_path_matches_all_seven_dirs` 七元素全命中 / `event_loop_filters_excluded_paths_keeps_normal` / `event_loop_rescan_bypasses_exclusion_filter`）** + **SEC-08 symlink 过滤（`is_symlink_path_detects_symlink` / `event_loop_filters_symlink_paths_keeps_normal`；symlink 创建失败 skip 豁免 `#[cfg(windows)]`——BE-17/D5）** + **BE-07 批量合并（`event_loop_merges_oversized_batch_to_rescan` 超限合并单条 Rescan / `event_loop_batch_at_limit_stays_classified` 限制值内不合并）**。

`notify_watch` 命令层测试：沙箱三分支（`validate_watch_path_nonexistent_rejected` / `outside_root_rejected` / `inside_root_ok`）+ pool 交互（phase1 命中 resume 目标暂停其余 / phase1 未命中返回 false / phase3 插入 watcher / phase3 竞态 drop 新 watcher）。

`notify_stop_watch` 命令层（BE-10）：`p14_remove_nonexistent_returns_none`（移除不存在路径幂等）+ 存在路径移除后 pool 不再持有。

### 手动构造模式（生命周期测试）

事件循环之外的纯生命周期测试使用**手动构造模式**——直接初始化 `FileWatcher` 结构体字段，绕过 `start()`：

```rust
// 手动构造，用 mpsc channel 模拟停止信号和线程
let (stop_tx, stop_rx) = std::sync::mpsc::channel();
let handle = std::thread::spawn(move || {
    while stop_rx.recv_timeout(Duration::from_millis(100)).is_err() {}
});
let watcher = FileWatcher {
    stop_tx: Some(stop_tx),
    thread_handle: Some(handle),
    watch_paths: Arc::new(Mutex::new(vec![])),
    paused: Arc::new(AtomicBool::new(false)),
};
```

### FileWatcher 生命周期测试（mod.rs）

- `stop()` 幂等：`stop_tx.take()` 后再次 stop 不 panic
- `Drop` 线程退出：`drop(watcher)` 后**轮询等待** `thread.is_finished()`（HFN-07：2s 超时轮询替代固定 sleep(100ms)，消除慢 CI flaky）
- 替换模式：旧 watcher stop → 新 watcher 创建，验证 `is_running()` 状态切换
- `FsEventPayload` serde：验证 `camelCase` 字段命名
- 事件分类：`classify_by_kind` 纯函数覆盖全部 7 种 EventKind 及子类型

### LruWatcherPool 测试（pool.rs）

`make_test_watcher(name)` 工厂函数创建模拟 FileWatcher（纯 mpsc + AtomicBool，无真实文件系统监听），15 条测试覆盖（HFN-02 补 `insert` 内部"已存在→stop 旧 watcher"替换分支——p10 原先手动 `remove` 再 insert，分支未真正执行；**BE-10 补 `remove` 幂等分支；BE-11 补容量常量断言**）：

- **缓存命中**：insert 后 get 获取同一实例
- **LRU 淘汰**：池满（capacity=8）时淘汰最久未访问的 watcher
- **pause_all_except**：目标 path 暂停其他所有 watcher
- **replace**：相同 path 再次 insert 替换旧 watcher
- **remove / stop_all / Drop**：逐个验证资源释放；**`watcher_pool_capacity_is_8`（BE-11 容量契约）**

```rust
fn make_test_watcher(name: &str) -> FileWatcher {
    // 创建带 mpsc channel 的 FileWatcher，线程空转等待 stop 信号
}
```

### `#[cfg(test)]` helper 方法

`FileWatcher::is_running()` 仅在 `#[cfg(test)]` 下编译，避免生产代码暴露内部线程状态。
