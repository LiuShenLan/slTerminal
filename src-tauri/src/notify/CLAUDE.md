# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 职责

文件系统监听模块——基于 `notify-debouncer-full`，递归监听项目目录的文件变更，去抖 300ms 后通过 Tauri Event 广播到前端。

## 架构决策

**Watcher 池 + pause/resume（非 stop/start）**：Windows 上 `notify` 调用 `ReadDirectoryChangesW` 递归注册目录树。大目录（如 `target/` 26K 文件）首次注册耗时约 2s。`LruWatcherPool` 缓存最多 5 个 watcher，切换项目时通过 `AtomicBool` pause/resume 替代销毁/重建，命中缓存时延迟 < 1ms。

**Debouncer 生命周期**：`notify-debouncer-full` 的 `Debouncer` 实例存活于 `"fs-watcher"` 线程内。线程退出时自动 drop→stop。不支持跨线程访问 debouncer API。

**暂停 ≠ 停止**：`paused` 标记仅阻止 `app_handle.emit("fs-event")` 上报前端。watcher 线程和 debouncer 继续运行，OS 文件监听句柄保持有效。

**事件分类纯函数化**：`classify_event()` 原依赖 `DebouncedEvent`（字段不公开，不可测试）。现拆分为编排层 `classify_event(event: DebouncedEvent)` + 纯函数 `classify_by_kind(kind: &EventKind, paths: Vec<String>) -> FsEventPayload`。`EventKind` 是 notify crate 公开枚举，可直接构造测试——覆盖全部 7 种 EventKind 及子类型（19 条测试）。

## 文件

| 文件 | 职责 |
|------|------|
| `mod.rs` | `FileWatcher` 结构体 + `notify_watch` Tauri 命令 + `classify_event` 事件分类（委托纯函数 `classify_by_kind`） |
| `pool.rs` | `LruWatcherPool` — LRU 淘汰的 watcher 缓存池 |

## FileWatcher

```
FileWatcher {
    stop_tx / thread_handle — 停止信号 + 线程句柄
    watch_paths: Arc<Mutex<Vec<PathBuf>>> — 监听根路径
    paused: Arc<AtomicBool> — 暂停事件上报
}
```

- `start(app_handle, paths, debounce_ms)` — 创建 debouncer + 注册 watch → spawn "fs-watcher" 线程
- `stop()` — 发送停止信号 → join 线程（幂等）
- `pause()` / `resume()` / `is_paused()` — AtomicBool 控制
- `Drop` — 发送停止信号 + join 线程，确保 OS 句柄释放

## `notify_watch` 命令

```
前端 startWatch(path)
  → pool.pause_all_except(path)  // 暂停其他 watcher
  → pool.get(path) 命中？→ 返回（resume 生效）
  → 未命中 → FileWatcher::start() → pool.insert()  // 池满则 LRU 淘汰
```

## 关键约束

- **watcher 重建开销**：不要在每次页面切换时 `stop()` + `start()` watcher。始终走池的 `pause_all_except`。
- **路径规范化**：池 key 使用 `dunce::simplified()` 处理，与 `fs_read_dir` 保持一致。
- **Drop 保证**：`LruWatcherPool::drop()` → `stop_all()` → 遍历 join 所有线程。AppState 销毁时自动触发。
- **路径 sandbox**：`notify_watch` 在创建 watcher 前校验 `validate_path_within_root()`（该函数已从 `fs/mod.rs` 迁移至 `crate::state`）。

## 测试模式

Rust 测试分布在 2 个位置：

| 位置 | 类型 | 用例数 |
|------|------|--------|
| `notify/mod.rs` `#[cfg(test)]` | 单元测试 | 24 |
| `notify/pool.rs` `#[cfg(test)]` | 单元测试 | 12 |

### 无 AppHandle 测试

`FileWatcher::start()` 需要 `AppHandle`（Tauri 运行时资源），单元测试无法创建。因此测试使用**手动构造模式**——直接初始化 `FileWatcher` 结构体字段，绕过 `start()`：

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
- `Drop` 线程退出：`drop(watcher)` 后验证 `thread.is_finished()` 返回 true
- 替换模式：旧 watcher stop → 新 watcher 创建，验证 `is_running()` 状态切换
- `FsEventPayload` serde：验证 `camelCase` 字段命名

### LruWatcherPool 测试（pool.rs）

`make_test_watcher(name)` 工厂函数创建模拟 FileWatcher（纯 mpsc + AtomicBool，无真实文件系统监听），12 条测试覆盖：

- **缓存命中**：insert 后 get 获取同一实例
- **LRU 淘汰**：池满（capacity=5）时淘汰最久未访问的 watcher
- **pause_all_except**：目标 path 暂停其他所有 watcher
- **replace**：相同 path 再次 insert 替换旧 watcher
- **remove / stop_all / Drop**：逐个验证资源释放

```rust
fn make_test_watcher(name: &str) -> FileWatcher {
    // 创建带 mpsc channel 的 FileWatcher，线程空转等待 stop 信号
}
```

### `#[cfg(test)]` helper 方法

`FileWatcher::is_running()` 仅在 `#[cfg(test)]` 下编译，避免生产代码暴露内部线程状态。
