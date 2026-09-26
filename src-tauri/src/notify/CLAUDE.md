# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 存在理由

`src-tauri/src/notify` 递归监听项目目录，把 `notify` crate 的原始事件转换为前端可消费的 `FsEventPayload`。watcher 池 + pause/resume 机制是为 Windows 大目录注册开销专门设计的，必须文档化以避免被 stop/start 模式破坏。

## 关键约束与决策

### Watcher 池 + pause/resume（非 stop/start）

Windows 上 `notify` 递归注册目录树（如 `target/` 26K 文件）首次约 2s。`LruWatcherPool` 缓存 8 个 watcher，切换项目时 `pause_all_except(path)` 暂停非活动 watcher、恢复目标 watcher。命中缓存时延迟 <1ms，且不释放 OS 句柄。

**pinned watcher（ADR-0024）**：`notify_watch(path, pinned?)` 置条目标记——pinned 条目被 `pause_all_except` 跳过（照 touch last_used；项目切换不暂停 agent 监听，agent 监听启动也不暂停项目 watcher，两侧皆免），`evict_lru` 避让（全 pinned 退化全池 LRU）。消费方 = 「Agent 全局文件」侧栏视图（展开即 pinned 监听 agent 全局目录，折叠/卸载即 stop）；`notify_stop_watch` 语义不变（移除即清 pinned 标记）。

- **暂停 ≠ 停止**：`paused` 仅阻止事件上报，watcher 线程与 debouncer 继续运行；
- **移除语义**：项目移除/切换时须调 `notify_stop_watch` → `pool.remove(path)`，否则旧 watcher 会一直占槽到 LRU 淘汰（BE-10）；
- **池 key 用 `dunce::simplified`**，与 `fs_read_dir` 返回路径格式一致。

### Debouncer 生命周期

`notify-debouncer-full` 的 `Debouncer` 存活于 `"fs-watcher"` 线程内；线程退出时自动 drop → stop，不支持跨线程访问。

### 事件分类纯函数化

`classify_event` 编排层调用纯函数 `classify_by_kind(kind, paths)`。`EventKind` 是 notify 公开枚举，L1 可直接构造，覆盖全部 7 种事件类型。

### 事件侧排除大目录（BE-02，D8；2026-09-25 部分翻案）

`WATCH_EXCLUDE_DIRS` 为六元素常量：`node_modules`、`target`、`.venv`、`venv`、`dist`、`__pycache__`。任一路径分量命中即丢弃该事件。**仅事件侧过滤**——notify 不支持目录级排除，watcher 仍注册全树；`need_rescan` 分支不受影响。

**.git 窄放行（D8 部分翻案，2026-09-25）**：`.git` 已移出排除常量，改事件级白名单判定（`is_git_internal_path` / `is_git_whitelist_path`）——批内存在白名单路径（`.git` 后首段 `refs` 整棵子树，或恰单分量 `HEAD`/`index`/`packed-refs`，大小写不敏感）则整条放行；命中 `.git` 但无白名单路径 → 整条丢弃（objects/**、logs/**、COMMIT_EDITMSG、MERGE_* 等仍丢弃）。必须**事件级**（批内 any 判定）而非逐路径：rename(Both) 双路径批（如 `index.lock`→`index`，paths=[lock, index]）按单路径判定会把携带白名单路径的整条事件误丢。动机 = DiffPanel HEAD 侧刷新（.git 事件 → 重取 gitFileAtHead）与提交后工作区着色联动的死代码复活；消费方纪律（DiffPanel 判等加固、文件树不订阅）见前端侧登记。

### fs-poll 心跳（2026-09-25，事件丢失补漏）

`FS_POLL_INTERVAL = 10s`：watcher 事件循环在**未暂停**且距上次心跳 ≥10s 时向监听根 emit `fs-poll`（`FsPollPayload { paths }` = 监听根集合）。契约：

- **pause 停发且不推进计时** → resume 后首轮节拍立即补发；
- 心跳内嵌现有 `"fs-watcher"` 线程（`recv_timeout(100ms)` 天然节拍），无额外线程；
- 消费方仅编辑域三处（useCodeMirror / DiffPanel / LargeFileViewer）——文件树/Commit 视图不订阅；
- 定位 = **补漏兜底**（OS 事件丢失窗口的周期复核），非主通道——主同步仍是 fs-event；前端复核走 poll 模式（滚动磁盘基线判变，未变零打扰），语义见 src/panels/editor/CLAUDE.md。

### FsEventPayload detail 字段不可依赖（⑤ 登记）

notify 的 `EventAttributes` 在 Windows ReadDirectoryChangesW 通道下基本不产详情——`detail` 恒 `"Other"`（classify_by_kind 兜底值）。前端任何「按 detail 区分改动类型」的逻辑都是死分支，禁止新增 detail 依赖；语义判别只能用 kind + paths。

### fs-event 批量合并上限（BE-07）

单事件 paths 数严格超过 `FS_EVENT_PATH_BATCH_LIMIT = 100` 时，合并为单条 `Rescan` 载荷下发（携带监听根路径），防大目录整体删除/移动产生的巨型批次打爆前端。

### symlink 过滤（SEC-08）

事件路径通过 `is_symlink_path` 上溯全部祖先检查；命中 symlink 的事件不 emit，防项目内 symlink 导致外部路径经 fs-event 泄露。`need_rescan` 只发 watch root，不受影响。

### EventEmitter trait 抽离（HFN-03）

`FileWatcher` 不直接持 `AppHandle`，而是通过 `EventEmitter` trait 转发事件。生产实现 `AppHandleEmitter` 包装 `AppHandle::emit`；L1 用 `MockEmitter` 注入，驱动事件循环全链路测试。

### `notify_watch` 命令分三阶段

1. `spawn_blocking` 内做路径存在性 + 沙箱校验；
2. 持池锁 `pause_all_except` + 缓存命中检查；
3. 锁外创建 watcher（递归注册大目录耗时约 2s，不阻塞 IPC worker），再插入池中。

## 外部坑/红线

- **禁止 stop/start 轮换 watcher**：用池的 pause/resume。
- **`notify` 不支持目录级排除**：watcher 仍注册全树，排除只在事件侧生效；不要试图改 notify API。
- **项目移除必须 stopWatch**：否则池槽被无意义占用。
- **symlink 事件直接丢弃**：不要尝试解析后转发。
- **Windows symlink 测试豁免**：创建 symlink 需管理员/developer mode，相关 L1 用例在创建失败时 skip（BE-17/D5）。

## 测试模式

- **MockEmitter 注入**：通过 `FileWatcher::start_with_emitter` 注入 `MockEmitter`，记录 emit 调用，无需构造 `AppHandle`。
- **手动构造模式**：生命周期测试直接初始化 `FileWatcher` 字段 + mpsc channel，绕过 `start()`。
- **LoopHarness 线程结果信令（BE-02）**：`shutdown` = 弃事件发送端 + `join_with_timeout` 等待 + done 通道 `recv().expect` 回传——watcher 线程 panic 时发送端随线程死亡 drop，`recv` 必 Err（线程结果信令，settings.rs mpsc 先例）；`join_with_timeout` 吞 JoinError 不再掩盖 panic。
- **LruWatcherPool 测试**：用 `make_test_watcher` 创建带 mpsc 的模拟 watcher，覆盖命中、LRU 淘汰、替换、`pause_all_except`、remove、stop_all、Drop。

### 既定豁免

| 豁免项 | 原因 | 当前兜底 |
|--------|------|---------|
| Windows symlink 创建测试 | 需要管理员/developer mode | 创建失败 skip，逻辑分支由非 symlink 用例覆盖 |
| 命令壳 `tauri::State` 路径 | 依赖 tauri 运行时 | 内核逻辑已由 `notify_watch`/`notify_stop_watch` 命令层测试覆盖 |
| 真实 `notify` debouncer 内部错误分支 | 依赖 OS 句柄状态 | 核心事件路径已由 mock emitter + 真实 tempdir 集成测试覆盖 |
