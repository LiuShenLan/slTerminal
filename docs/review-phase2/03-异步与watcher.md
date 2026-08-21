# 03-异步与watcher（S04/S05）

## 1. 闭环判定表

| 条目 | 判定 | 证据 | 备注 |
|---|---|---|---|
| BE-04 | fixed | `src-tauri/src/state.rs:248-272` set_project_root 改 async，canonicalize 包在 spawn_blocking；`src-tauri/src/notify/mod.rs:348-391` notify_watch 改 async，FileWatcher::start_with_emitter 包在 spawn_blocking；L1 `state::project_root_tests` 4 条全绿 | notify_watch 前置沙箱校验仍有同步 canonicalize（见新发现 3） |
| BE-02 | fixed | `src-tauri/src/notify/mod.rs:30-38` WATCH_EXCLUDE_DIRS 七元素；`:196-203` is_excluded_path 按整分量比较；`:251-254` event_loop 排除过滤；L1 3 条过滤测试通过 | Windows 大小写敏感（见新发现 6） |
| BE-10 | partial | `src-tauri/src/notify/mod.rs:404-421` notify_stop_watch 命令；`src-tauri/src/notify/pool.rs:92-98` remove 调用 stop；`src-tauri/src/lib.rs:106` 注册；`src-tauri/capabilities/default.json:42` allow；`src/ipc/notify.ts:17-20` wrapper；`src/workspace/Workspace.tsx:247` 切换时 stopWatch(prev)；a40ee09 将 watcher 管理上提到 Workspace | 项目/页面移除至空页面路径未 stopWatch（见新发现 2） |
| BE-11 | fixed | `src-tauri/src/notify/pool.rs:21` WATCHER_POOL_CAPACITY = 8；`:430-433` 常量断言；L1 15 条 pool 测试通过 | — |
| SEC-08 | fixed | `src-tauri/src/notify/mod.rs:217-226` is_symlink_path 上溯祖先检查；`:255-258` event_loop 过滤 symlink；L1 3 条 symlink 测试通过 | 每条事件路径都 stat 全部祖先，事件风暴时 I/O 开销偏高（可选优化） |
| SEC-14 | fixed | `src-tauri/src/state.rs:277-297` apply_project_root 失败分支清空旧 root；`:865-898` L1 测试 `set_project_root_failure_clears_old_root` 通过 | lock 中毒分支未清空（见新发现 5） |
| FE-04 | fixed | `src/App.tsx:99-101` toast；`src/stores/projects.ts:165-169` toast；`src/workspace/Workspace.tsx:250-252` toast；`src/__tests__/workspace-switch-order.test.tsx` 17 条通过 | `src/workspace/pageApis.ts:switchToPageShared` 同路径未 toast（见新发现 4） |

## 2. 新发现问题

### 新发现 1：set_project_root 后端未做并发序列化，快速切换项目时旧请求可能覆盖新 root（P1）

**位置**：`src-tauri/src/state.rs:254-272`、`src/stores/projects.ts:159-186`

**说明**：`set_project_root_impl` 内 canonicalize 在 spawn_blocking 中异步执行，执行完才调用 `apply_project_root` 写锁。两个请求可并发：用户快速 A→B 切换时，若 A 的 canonicalize 慢于 B 完成，A 的 `apply_project_root` 会把 `project_root` 写回 A，导致 activePage 已是 B 但沙箱 root 为 A。`Workspace.tsx` SEC-01 effect 也 fire-and-forget 调用，进一步放大竞态。

**修改建议**：在 `set_project_root` 层引入互斥或队列（如 `tokio::sync::Mutex` 包裹整个 `set_project_root_impl` 调用），保证后端按调用顺序串行生效；或前端切换逻辑统一 await 并取消/丢弃过期请求。

---

### 新发现 2：项目/页面移除至空页面时未调用 stopWatch，watcher 残留至 LRU 淘汰（P1）

**位置**：`src/workspace/Workspace.tsx:116-152`、`src/workspace/Workspace.tsx:238-258`、`src/stores/projects.ts:92-103`

**说明**：`Workspace.tsx` 的 SEC-01 effect 只在 `activePageId` 切到另一个带 rootPath 的页面时执行 `stopWatch(prev)`。当用户删除当前唯一页面/项目，或移除整个项目导致 `activePageId` 被置为 null 时，effect 直接 `return`，不会停止当前项目的 watcher。`onDeletePage` 与 `removeProject` 也未显式释放。旧 watcher 继续占用 OS 句柄和池槽，直至 LRU 淘汰。

**修改建议**：在 `activePageId` 变为 null 的 effect 清理路径、或 `onDeletePage`/`removeProject` 调用链中，对 `prevRootRef.current` 调用 `stopWatch` 并清空 ref。

---

### 新发现 3：notify_watch 前置沙箱校验仍在 IPC worker 同步执行 canonicalize（P2）

**位置**：`src-tauri/src/notify/mod.rs:310-318`、`src-tauri/src/state.rs:199-241`

**说明**：`notify_watch` 虽然把 `FileWatcher::start_with_emitter` 移入 spawn_blocking，但在进入 spawn_blocking 之前，`validate_watch_path` 会同步调用 `path.exists()` 和 `validate_path_within_root`，后者对 root 与 target 都执行 `dunce::canonicalize`/`canonicalize_or_ancestor`。网络驱动器或大量符号链接场景下仍会阻塞 IPC worker。

**修改建议**：将 `validate_watch_path` 一并移入 spawn_blocking 闭包，或把 canonicalize 也包进 `spawn_blocking`；保持命令外层的错误映射不变。

---

### 新发现 4：switchToPageShared 失败路径未 toast，与 FE-04 三处调用点不一致（P2）

**位置**：`src/workspace/pageApis.ts:49-73`

**说明**：`switchToPageShared` 调用 `setProjectRoot` 时仅 `console.error`，未像 `App.tsx`、`stores/projects.ts`、`Workspace.tsx` 一样 toast 告警。虽然 `Workspace.tsx` SEC-01 effect 在 activePageId 变化后会再次触发并 toast，但同一失败会被处理两次（一次静默、一次告警），且从导航树直接切换时用户可能先看到静默失败。

**修改建议**：在 `switchToPageShared` 的 catch 块中统一调用 `toast.show("warning", "项目根路径设置失败，文件操作可能被拒绝")`，保持与 checklist FE-04 的一致性。

---

### 新发现 5：set_project_root 失败清空旧 root 在写锁中毒分支未执行（P2）

**位置**：`src-tauri/src/state.rs:283-288`

**说明**：`apply_project_root` 错误分支使用 `if let Ok(mut root) = project_root.write()` 清空旧 root。若锁已中毒（watcher 线程或 reader 线程 panic），`write()` 返回 Err，旧 root 不会被清空，沙箱可能继续放行旧路径。

**修改建议**：将清空逻辑放在获取写锁的错误处理统一路径中，或对 `project_root.write().map_err(...)` 失败时同样返回 Err 并显式 tracing；若保持现状，应在 `src-tauri/src/CLAUDE.md` 的 Mutex 中毒决策段登记此残余风险。

---

### 新发现 6：watcher 排除目录匹配大小写敏感，Windows 可能漏排非小写目录名（P2）

**位置**：`src-tauri/src/notify/mod.rs:196-203`

**说明**：`is_excluded_path` 用 `WATCH_EXCLUDE_DIRS.contains(&seg)` 做严格字符串匹配。Windows 文件系统大小写不敏感，若目录实际名为 `Node_Modules`、`Target` 或 `NODE_MODULES`，事件不会被排除。

**修改建议**：在 Windows 下将事件路径分量与排除集做不区分大小写比较（如 `.eq_ignore_ascii_case` 或转小写后比较），保持跨平台语义一致。

---

### 新发现 7：Workspace SEC-01 effect 未等待 setProjectRoot 完成即启动 startWatch（P2）

**位置**：`src/workspace/Workspace.tsx:244-253`

**说明**：该 effect 内 `stopWatch(prev)`、`setProjectRoot(rootPath)`、`startWatch(rootPath)` 均为 fire-and-forget。`startWatch` 到达后端时，`project_root` 可能尚未更新（尤其 E2E helper 直接修改 activePageId 而不走 `switchToPageShared`），导致沙箱校验失败、watcher 未启动。

**修改建议**：将 `setProjectRoot` 与 `startWatch` 串行化：`await setProjectRoot(...)`，成功后再 `await startWatch(...)`；失败时跳过 startWatch。

## 3. 依赖人工验证

| 验证点 | 静态可查证据 |
|---|---|
| S05 大仓库事件量实测 | `src-tauri/src/notify/mod.rs:30-38` 排除集 + `:251-258` 过滤逻辑已落地；`:222-226` symlink 过滤逻辑已落地；L1 过滤/合并测试覆盖。真实大仓库下 CPU/事件量降幅需实测。 |
