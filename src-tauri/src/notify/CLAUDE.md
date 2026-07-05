# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 职责

文件系统监听模块——基于 `notify-debouncer-full`，递归监听项目目录的文件变更，去抖 300ms 后通过 Tauri Event 广播到前端。

## 架构决策

**Watcher 池 + pause/resume（非 stop/start）**：Windows 上 `notify` 调用 `ReadDirectoryChangesW` 递归注册目录树。大目录（如 `target/` 26K 文件）首次注册耗时约 2s。`LruWatcherPool` 缓存最多 5 个 watcher，切换项目时通过 `AtomicBool` pause/resume 替代销毁/重建，命中缓存时延迟 < 1ms。

**Debouncer 生命周期**：`notify-debouncer-full` 的 `Debouncer` 实例存活于 `"fs-watcher"` 线程内。线程退出时自动 drop→stop。不支持跨线程访问 debouncer API。

**暂停 ≠ 停止**：`paused` 标记仅阻止 `app_handle.emit("fs-event")` 上报前端。watcher 线程和 debouncer 继续运行，OS 文件监听句柄保持有效。

## 文件

| 文件 | 职责 |
|------|------|
| `mod.rs` | `FileWatcher` 结构体 + `fs_watch` Tauri 命令 + `classify_event` 事件分类 |
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

## `fs_watch` 命令

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
- **路径 sandbox**：`fs_watch` 在创建 watcher 前校验 `validate_path_within_root()`。
