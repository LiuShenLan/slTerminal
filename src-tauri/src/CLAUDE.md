# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 模块职责

`src-tauri/src/` 顶层单文件模块——不属于任何功能子模块（pty/fs/git/notify/hooks/agent_history）的全局支撑件：

- **`lib.rs`** — Tauri 入口：`generate_handler!` 注册全部命令 + `AppState` 注入 + `.setup()` 启动 HookSignalWatcher + 窗口事件
- **`settings.rs`** — 设置持久化（`load_settings`/`save_settings`，`~/.slterminal/settings.json`）
- **`projects.rs`** — 项目数据持久化（`load_projects`/`save_projects`，exe 同级 `slterminal-projects.json`）
- **`state.rs`** — `AppState`（project_root + pty state + watcher 池）+ `PtySession`/`PtyState` + 路径沙箱 `validate_path_within_root`
- **`error.rs`** — 统一错误类型 `AppError`

## 架构决策

### settings.rs — 浅合并持久化

`save_settings` 只写前端传入的 slice（如 `{ keybindings }` 或 `{ sideBar }`），后端**浅合并** top-level 键——各 store 各写各的互不覆盖（fontSize/keybindings/sideBar 三段独立写入）。应用数据目录 `~/.slterminal` 由 `resolve_app_data_dir` 解析，测试用 `AppDataDirGuard` RAII 注入覆盖（SPE-04）。

### projects.rs — exe 同级 JSON 绕过沙箱

项目数据存 exe 同级 `slterminal-projects.json`（应用级元数据，非用户项目文件），**绕过路径沙箱**（照 settings.rs 先例）。原子写（NamedTempFile + persist）+ `.bak` 备份兜底；加载时 JSON 损坏/缺失 → 尝试 `.bak` → 仍失败 → `"{}"`。

### state.rs — AppState 与路径沙箱

- `AppState`：`project_root: RwLock<Option<PathBuf>>` + `pty: PtyState` + `watcher_pool: LruWatcherPool`（notify 模块）
- `PtySession`/`PtyState`：PTY 会话生命周期（master/child/writer/reader_handle/channel/output_ring/exit_code/da1_injected/job_object/panel_id）——细节见 @pty/CLAUDE.md
- `validate_path_within_root`：路径沙箱核心，覆盖全部 10 个命令（fs 6 + notify_watch + git_status/git_diff + pty_spawn cwd）；`canonicalize_or_ancestor` 支持已删除文件路径校验（上溯最近存在的祖先）。对 `project_root=None` 一律拒绝（非 cfg!(test)）——前端加载时序保障见 @fs/CLAUDE.md

### error.rs — 统一错误类型

`AppError`（camelCase 序列化）：IoKind/Pty/Git/Serde/Unknown/SessionNotFound/TaskJoin/Notify/Validation/PathNotAllowed。全部 Tauri 命令返回 `Result<_, AppError>`（硬约束 #3）。

### 模块边界

子模块（pty/fs/git/notify/hooks/agent_history）经 `use crate::state::...` / `use crate::error::AppError` 引用顶层件；顶层件不反向依赖子模块实现（仅 `lib.rs` 聚合注册）。新增顶层单文件模块（非功能子模块）时在此登记。

## 文件

| 文件 | 职责 |
|------|------|
| `lib.rs` | Tauri 入口：命令注册（33 条 generate_handler!——hooks 泛化 6 条 `agent_hooks_inject`/`agent_hooks_uninstall`/`agent_hooks_injection_status`/`agent_hooks_restore_statusline`/`agent_hooks_config_read`/`agent_hooks_config_write` + 历史泛化 3 条 `agent_history_scan`/`agent_history_delete`/`agent_history_read_title`，旧命令名 `hooks_*`/`claude_history_*`/`agent_context_usage` 零残留）+ State 注入 + setup（watcher 启动 + statusline 桥接启动重注入）+ 窗口关闭清理 |
| `settings.rs` | 设置持久化：`load_settings`/`save_settings` 浅合并 + `resolve_app_data_dir` |
| `projects.rs` | 项目数据持久化：`load_projects`/`save_projects`（exe 同级，绕过沙箱）+ `.bak` 兜底 |
| `state.rs` | `AppState` + `PtySession`/`PtyState`（细节 @pty/CLAUDE.md）+ `validate_path_within_root` 路径沙箱（15 条 sandbox 测试 + 32 条含 ring buffer） |
| `error.rs` | `AppError` 统一错误类型（10 变体，camelCase 序列化） |

## 测试模式

- `state.rs` `#[cfg(test)]`：32 条——sandbox 路径校验 + ring buffer 纯函数（pty 模块文档登记）
- `settings.rs`/`projects.rs`：命令层测试经 `AppDataDirGuard` 注入 tempdir（SPE-04）；projects 复用 settings 的守卫
- `error.rs`：AppError serde camelCase 序列化断言
- 命令注册完整性由 `src/__tests__/ipc-*-contract.test.ts` 契约测试间接守护（前端 wrapper 命令名与 lib.rs 注册一致）

## 修改注意事项

1. 新增 Tauri 命令后在 `lib.rs` 的 `generate_handler!` 注册（33 条现行清单见 `src/ipc/CLAUDE.md` 模块映射）
2. 修改 `validate_path_within_root`/`canonicalize_or_ancestor` 后跑 `state.rs` 全部测试 + `fs`/`git`/`notify`/`pty` 相关命令层测试（沙箱是 10 命令的共同前置）
3. 修改 `AppError` 变体后同步检查各模块错误映射（`From` impl 与 `map_err` 站点）
4. settings/projects 浅合并语义改动时核对 `src/stores/CLAUDE.md`（各 store 独立写入依赖浅合并）
