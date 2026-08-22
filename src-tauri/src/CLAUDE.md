# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 模块职责

`src-tauri/src/` 顶层单文件模块——不属于任何功能子模块（pty/fs/git/notify/hooks/agent_history）的全局支撑件：

- **`lib.rs`** — Tauri 入口：`generate_handler!` 注册全部命令 + `AppState` 注入 + `.setup()` 启动 HookSignalWatcher + 窗口事件
- **`app_dir.rs`** — 应用数据目录解析/测试守卫/共享 DTO（BE-16 上提）
- **`settings.rs`** — 设置持久化（`load_settings`/`save_settings`，`~/.slterminal/settings.json`）
- **`projects.rs`** — 项目数据持久化（`load_projects`/`save_projects`，exe 同级 `slterminal-projects.json`）
- **`state.rs`** — `AppState`（project_root + pty state + watcher 池 + git 缓存）+ `PtySession`/`PtyState` + 路径沙箱 `validate_path_within_root`
- **`error.rs`** — 统一错误类型 `AppError`

## 架构决策

### app_dir.rs — 应用数据目录单点（BE-16）

`app_data_dir`/`resolve_app_data_dir` 上提至 `app_dir.rs`（BE-16：原 `projects.rs` 直接导入 `settings::app_data_dir` 违反约束 #2），settings/projects 均从该模块导入。同模块承载：

- `LoadResult<T>` — load 命令统一返回结构 `{ data, corrupted }`（BE-14/D11：无文件 = `data:null, corrupted:false`；损坏回退默认值 = `corrupted:true`；`.bak` 命中也算 corrupted=true——数据来自备份）
- `MAX_PERSIST_BYTES = 1MB` — save 侧大小上限（SEC-11，settings/projects 共用）
- `AppDataDirGuard` — 测试守卫（RAII 注入覆盖应用目录，SPE-04）

### settings.rs — 浅合并持久化 + SEC-11 校验 + 保存互斥

`save_settings` 只写前端传入的 slice（如 `{ keybindings }` 或 `{ sideBar }`），后端**浅合并** top-level 键——各 store 各写各的互不覆盖（fontSize/keybindings/sideBar 三段独立写入，前端 payload 顶层键必须是白名单段名——fontSize 平铺键断链先例已被双侧测试锁死）。应用数据目录 = exe 同级（`app_dir::app_data_dir`），测试用 `AppDataDirGuard` RAII 注入覆盖（SPE-04）。

**`SETTINGS_SAVE_LOCK` 互斥（SPE-06 场景转正）**：前端三 store 启动时几乎同时各触发一次 debounced 保存，并发对同一 settings.json 读-合并-写（persist rename + .bak copy）时 Windows 上偶发 PermissionDenied——`save_settings` 的 spawn_blocking 闭包持锁串行化全程（锁内无 panic 路径；`load_settings` 不加锁，读旧值无害）。

**SEC-11（S09）**：save 侧校验——顶层键白名单（`SETTINGS_ALLOWED_KEYS = [fontSize, keybindings, sideBar, colorScheme]`，spawn_blocking 前快速失败）+ 序列化后大小上限 `MAX_PERSIST_BYTES`（1MB）。**load 返回 `LoadResult`**（BE-14/D11）：损坏回退默认值 + `.bak` 命中均 `corrupted:true`。

### projects.rs — exe 同级 JSON 绕过沙箱

项目数据存 exe 同级 `slterminal-projects.json`（应用级元数据，非用户项目文件），**绕过路径沙箱**（照 settings.rs 先例）。原子写（NamedTempFile + persist）+ `.bak` 备份兜底；加载时 JSON 损坏/缺失 → 尝试 `.bak` → 仍失败 → `data:null, corrupted:true`（BE-14）。SEC-11：save 侧大小上限 1MB + 结构校验（须为 JSON 对象）。

### state.rs — AppState 与路径沙箱

- `AppState`：`project_root: RwLock<Option<PathBuf>>` + `project_root_lock: tokio::sync::Mutex<()>`（SEC-16：set_project_root 串行化锁——canonicalize+apply 全程互斥，A→B 快速切换时慢 canonicalize 的 A 不得后写回覆盖 B）+ `pty: PtyState` + `watcher_pool: LruWatcherPool`（notify 模块）+ `git_repo_cache: LruCache`（BE-09 容量 8，见 @git/CLAUDE.md）
- `PtySession`/`PtyState`：PTY 会话生命周期（master/child/writer/reader_handle/channel/output_ring/exit_code/da1_injected/job_object/panel_id）——细节见 @pty/CLAUDE.md
- `validate_path_within_root`：路径沙箱核心，覆盖全部 10 个命令（fs 6 + notify_watch + git_status/git_diff + pty_spawn cwd）；`canonicalize_or_ancestor` 支持已删除文件路径校验（上溯最近存在的祖先）。对 `project_root=None` 一律拒绝（非 cfg!(test)）——前端加载时序保障见 @fs/CLAUDE.md

### std Mutex 中毒保持现状（09#14，DOC-10 登记）

state.rs 多处 `Arc<Mutex<...>>`（child/writer/output_ring 等）用标准库 `std::sync::Mutex`。review 建议换 `parking_lot`（无中毒）或 `catch_unwind` 包裹——**决策：保持现状，不引入**。理由：中毒仅在持锁 panic 时发生（毒后 `.lock()` 返回 Err），本仓持锁临界区均为短小无 panic 路径（锁内不做可能 panic 的分配/IO），中毒实际不可达；换 parking_lot 是零收益依赖变更。**`parking_lot`/`catch_unwind` 仅作未来引入高风险外部代码（可能在锁内 panic）时的预案**——届时再评估。新建持锁临界区时保持「锁内不做可能 panic 的工作」纪律即可。

**BE-24 例外（SEC-14 锁中毒分支可观测化）**：`apply_project_root` 失败时清空旧 root（SEC-14）在写锁中毒时不生效（旧 root 残留至进程退出）——已 `tracing::warn!` 可观测化，接受偏差（中毒本身不可达，见上节）。

### error.rs — 统一错误类型

`AppError`（camelCase 序列化）：IoKind/Pty/Git/Serde/**ConfigParse**（BE-15，配置 JSON 损坏场景）/Unknown/SessionNotFound/TaskJoin/Notify/Validation/PathNotAllowed——共 11 变体。全部 Tauri 命令返回 `Result<_, AppError>`（硬约束 #3）。**BE-13（S08）**：`From<std::io::Error>` 本身不动，fs/settings/projects 命令内 `map_err` 调用点注入路径上下文，错误消息含路径；用户可见消息改业务语义（如「保存设置失败」），技术细节进 tracing。

### 模块边界

子模块（pty/fs/git/notify/hooks/agent_history）经 `use crate::state::...` / `use crate::error::AppError` 引用顶层件；顶层件不反向依赖子模块实现（仅 `lib.rs` 聚合注册）。顶层单文件模块间共享经 `app_dir.rs`/`state.rs`（如 settings/projects 均从 `app_dir` 导入 `app_data_dir`/`LoadResult`，BE-16）。新增顶层单文件模块（非功能子模块）时在此登记。

## 文件

| 文件 | 职责 |
|------|------|
| `lib.rs` | Tauri 入口：命令注册（**34 条** generate_handler!——hooks 泛化 6 条 `agent_hooks_inject`/`agent_hooks_uninstall`/`agent_hooks_injection_status`/`agent_hooks_restore_statusline`/`agent_hooks_config_read`/`agent_hooks_config_write` + 历史泛化 3 条 `agent_history_scan`/`agent_history_delete`/`agent_history_read_title` + `pty_kill_all`（BE-08）+ `notify_stop_watch`（BE-10），旧命令名 `hooks_*`/`claude_history_*`/`agent_context_usage`/`pty_reattach`（SEC-03）零残留）+ State 注入 + setup（watcher 启动 + statusline 桥接启动重注入）+ 窗口关闭清理。命令白名单经 `build.rs` `AppManifest::new().commands(&[...])` + `capabilities/default.json` 逐条 `allow-<cmd>`（SEC-07） |
| `app_dir.rs` | 应用数据目录单点（BE-16）：`app_data_dir`/`resolve_app_data_dir` + `LoadResult` 共享 DTO + `MAX_PERSIST_BYTES` + `AppDataDirGuard` 测试守卫 |
| `settings.rs` | 设置持久化：`load_settings`/`save_settings` 浅合并 + `resolve_app_data_dir`（自 app_dir 导入）+ LoadResult corrupted 契约（BE-14）+ SEC-11 白名单/大小校验 |
| `projects.rs` | 项目数据持久化：`load_projects`/`save_projects`（exe 同级，绕过沙箱）+ `.bak` 兜底 + corrupted 契约 + SEC-11 校验 |
| `state.rs` | `AppState` + `PtySession`/`PtyState`（细节 @pty/CLAUDE.md）+ `validate_path_within_root` 路径沙箱 + `git_repo_cache` LRU（BE-09） |
| `error.rs` | `AppError` 统一错误类型（11 变体含 ConfigParse，camelCase 序列化） |

## 测试模式

- `state.rs` `#[cfg(test)]`：42 条——sandbox 路径校验（含 symlink 特权测试，**豁免 #cfg(windows) 保留**——BE-17/D5，Windows symlink 需管理员/developer mode）+ ring buffer 纯函数（pty 模块文档登记）+ `git_repo_cache` LRU 纯逻辑（BE-09）
- `app_dir.rs` `#[cfg(test)]`：7 条——`resolve_app_data_dir` 三分支 + `LoadResult` serde 形态 + `AppDataDirGuard` 注入/恢复
- `settings.rs`/`projects.rs`：命令层测试经 `AppDataDirGuard` 注入 tempdir（SPE-04）；projects 复用 settings 的守卫；含 corrupted 契约（BE-14）与 SEC-11 校验用例
- `error.rs`：AppError serde camelCase 序列化断言（含 ConfigParse 变体）
- 命令注册完整性由 `src/__tests__/ipc-*-contract.test.ts` 契约测试间接守护（前端 wrapper 命令名与 lib.rs 注册一致，34 条）

## 修改注意事项

1. 新增 Tauri 命令后在 `lib.rs` 的 `generate_handler!` 注册（**34 条现行清单**），且须同步：`build.rs` 白名单 `AppManifest::new().commands(...)` + `capabilities/default.json` 加 `allow-<cmd>`（SEC-07 契约，缺一即 invoke reject）
2. 修改 `validate_path_within_root`/`canonicalize_or_ancestor` 后跑 `state.rs` 全部测试 + `fs`/`git`/`notify`/`pty` 相关命令层测试（沙箱是 10 命令的共同前置）
3. 修改 `AppError` 变体后同步检查各模块错误映射（`From` impl 与 `map_err` 站点）+ `src/ipc/appError.ts` 前端解析器（FE-02 全 11 变体测试）
4. settings/projects 浅合并语义改动时核对 `src/stores/CLAUDE.md`（各 store 独立写入依赖浅合并）
5. `LoadResult`/`MAX_PERSIST_BYTES`/`AppDataDirGuard` 改动后跑 `app_dir.rs` 全部测试 + settings/projects 命令层测试；app_data_dir 解析路径变更须核对 `src/ipc/settings.ts`/`projects.ts` wrapper 契约
6. 新增持锁临界区保持「锁内不做可能 panic 的工作」纪律（Mutex 中毒保持现状决策，见上）
