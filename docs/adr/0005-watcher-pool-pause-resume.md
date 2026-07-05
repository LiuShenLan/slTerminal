# 文件监听器池 + pause/resume（非 stop/start）

Status: Accepted

文件系统监听器通过 LRU 池（最多 5 个）管理，切换项目时暂停其他、恢复目标——而非销毁重建。

**为什么？** Windows 上 `ReadDirectoryChangesW` 递归注册大目录树（如 `target/` 含 26,000 文件）耗时约 2 秒。每次切换项目都 stop + start 会引入不可接受的延迟。

**为什么是 pause 而非 stop？** Pause 仅阻止 `emit("fs-event")` 上报前端，watcher 线程和 OS 句柄保持活跃——恢复延迟 < 1ms。Stop 则销毁线程、关闭句柄，下次 start 需重新递归注册。

**备选方案**：全量监听所有项目（无限制 watcher 数）——内核对象耗尽风险；不缓存 watcher（每次 stop/start）——2 秒切换延迟。

**后果**：池满时 LRU 淘汰冷项目，再次访问时需冷启动（~2s）。应用退出时 `LruWatcherPool::drop()` 遍历停止所有 watcher。
