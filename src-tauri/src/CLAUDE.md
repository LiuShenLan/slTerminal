# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 存在理由

`src-tauri/src/` 顶层单文件模块承载各功能子模块共享的全局支撑件：应用数据目录、settings/projects 持久化、全局 `AppState`、路径沙箱、统一错误类型。这些模块的跨模块契约（数据目录、持久化格式、沙箱语义、错误消息约定）需要在顶层文档化，避免各子模块重复解释或相互穿透。

## 关键约束与决策

### app_dir.rs — 应用数据目录单点（BE-16）

`app_data_dir`/`resolve_app_data_dir` 上提至本模块，settings 与 projects 均从这里导入，避免跨模块直接引用。数据目录为 exe 同级（便携分发语义）。同模块承载：
- `LoadResult<T>`：`{ data, corrupted }`，无文件时 `data:null, corrupted:false`；损坏回退默认值或 `.bak` 命中时 `corrupted:true`（BE-14/D11）；
- `MAX_PERSIST_BYTES = 1MB`：save 侧大小上限，settings/projects 共用（SEC-11）；
- `AppDataDirGuard`：测试用 RAII 注入覆盖应用目录（SPE-04）。

### settings.rs — 浅合并 + 保存互斥 + 白名单

- **浅合并**：`save_settings` 只写前端传入的顶层 slice，后端浅合并 top-level 键，各 store 各写各的互不覆盖；
- **`SETTINGS_SAVE_LOCK`**：前端三 store 启动时几乎同时触发 debounced 保存，`spawn_blocking` 闭包持锁串行化读-合并-写，避免 Windows persist rename 时句柄占用导致 PermissionDenied（SPE-06）；
- **SEC-11**：顶层键白名单（数组仍 5 项）`["fontSize", "keybindings", "sideBar", "colorScheme", plan_balance::SETTINGS_KEY]` + 序列化后大小上限 1MB。**键名聚合决策（F11）**：前端消费型四键（fontSize/keybindings/sideBar/colorScheme）无后端模块可归，键名集中于此字面量；后端消费型域键名归域模块——`planBalance` 经 `crate::plan_balance::SETTINGS_KEY` 引用（契约断链先例：fontSize store 曾发平铺键被拒，已改段形态双侧锁死）。`planBalance` 段 = F10 轮询间隔（默认 60，越界/缺失/损坏回退 60s）；F11 起写入侧除手改文件外新增专用命令通道 `plan_balance_set_interval`（复用本模块 save_settings 写通道：白名单/浅合并/原子写/.bak/SETTINGS_SAVE_LOCK——禁止自建第二写通道）。

### projects.rs — exe 同级 JSON 绕过沙箱

项目数据存 exe 同级的 `slterminal-projects.json`，原子写（NamedTempFile + persist）+ `.bak` 备份；损坏/缺失 → `.bak` 回退 → 仍失败返回 `data:null, corrupted:true`。SEC-11 校验大小 1MB + 必须为 JSON 对象。

### state.rs — `AppState` 与路径沙箱

- `AppState`：pty 状态、watcher 池、git 缓存、project_root 共享；
- `project_root_lock: tokio::sync::Mutex<()>`（SEC-16）：`set_project_root` canonicalize+apply 全程互斥，防止 A→B 快速切换时慢 canonicalize 的 A 后写回覆盖 B；
- `validate_path_within_root`：覆盖 fs/git/notify/pty 全部受沙箱保护的命令；相对路径先 join root 再 canonicalize，目标不存在时上溯最近存在的祖先；`project_root=None` 时拒绝（`cfg!(test)` 豁免）。

### error.rs — 统一错误类型与消息语义（BE-13/BE-15）

`AppError` 共 11 变体（camelCase 序列化），全部 Tauri 命令返回 `Result<_, AppError>`。约定：
- `message` 为用户可见业务语义；
- 技术细节进 `tracing`，不暴露给前端；
- 带路径的 IO 错误用 `io_error(...)` 辅助函数，避免直接 `?` 走 `From<std::io::Error>` 丢失上下文。

### std Mutex 中毒保持现状（DOC-10）

`state.rs` 等处的 `Arc<Mutex>` 保持标准库 `std::sync::Mutex`。持锁临界区均为短小无 panic 路径，中毒实际不可达，换 `parking_lot` 是零收益依赖变更。新建持锁临界区时保持「锁内不做可能 panic 的工作」纪律。

## 外部坑/红线

- **新增命令必须三处注册**：`lib.rs` 的 `generate_handler!`、`build.rs` 的 `AppManifest::new().commands(...)`、`capabilities/default.json` 的 `allow-<cmd>`（SEC-07），缺一即 invoke reject。
- **改 DTO 必须双边同步**：Rust `snake_case` ↔ JS `camelCase`，改一边必须改另一边（硬约束 #4）。
- **`project_root_lock` 必须覆盖 canonicalize+apply 全程**：不要拆锁，否则有慢路径覆盖风险（SEC-16）。
- **不要在持锁临界区引入 panic**：保持 Mutex 中毒不可达纪律。
- **settings 顶层键白名单勿擅自扩充**：前端各 store 独立写入依赖此白名单。

## 测试模式

- `state.rs` `#[cfg(test)]`：路径沙箱校验（含 symlink 特权豁免）+ `GitRepoCache` LRU 纯逻辑。
- `app_dir.rs` `#[cfg(test)]`：`resolve_app_data_dir` 三分支 + `LoadResult` serde + `AppDataDirGuard` 注入/恢复。
- settings/projects 命令层测试经 `AppDataDirGuard` 注入 tempdir（SPE-04）。
- `error.rs`：AppError serde camelCase 序列化断言。
- 命令注册完整性由 `src/__tests__/ipc-*-contract.test.ts` 间接守护。

### 既定豁免

| 豁免项 | 原因 | 当前兜底 |
|--------|------|---------|
| Windows symlink 特权测试 | 创建 symlink 需管理员/developer mode | `#[cfg(windows)]` 保留；失败时 skip |
| Mutex 中毒分支 | 临界区无 panic | 未来锁内引入 panic 代码时须换原语或补测试 |
