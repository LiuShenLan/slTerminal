# Windows ConPTY SPAWN_LOCK 串行化

Status: Accepted

所有 ConPTY spawn 操作必须通过全局 `Mutex` 串行执行。并发调用 `CreatePseudoConsole` + `CreateProcessW` 会导致 ConPTY 输出管道永久卡死，无法恢复。

**为什么不是重试机制？** ConPTY 的死锁是底层内核对象级别的，不是瞬时竞争——重试不会自愈，只会累积更多死锁的管道。

**备选方案**：并发 spawn + 进程级隔离（每个 spawn 用独立进程，但 WebView2 宿主架构不支持多进程 Terminal）；异步 spawn 回调队列（仍需串行，只是语法糖）。

**后果**：终端创建吞吐量限制为一次一个。锁仅保护 ConPTY 创建 + 子进程启动；读取线程启动和会话映射插入在锁外执行，最小化持有时间。
