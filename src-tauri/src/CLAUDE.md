# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 存在理由

`src-tauri/src/` 顶层单文件模块承载各功能子模块共享的全局支撑件：应用数据目录、用户 home 目录、settings/projects 持久化、全局 `AppState`、路径沙箱、统一错误类型。这些模块的跨模块契约（数据目录、home 解析、持久化格式、沙箱语义、错误消息约定）需要在顶层文档化，避免各子模块重复解释或相互穿透。

## 关键约束与决策

**模块间不互相穿透（硬约束 #2）**：功能子模块之间禁止互相引用，共享只经 `state.rs` 的 `AppState`；本层顶层单文件模块承担全局支撑件。

### app_dir.rs — 应用数据目录单点（BE-16）

`app_data_dir`/`resolve_app_data_dir` 上提至本模块，settings 与 projects 均从这里导入，避免跨模块直接引用。数据目录为 exe 同级（便携分发语义）。**数据目录三级来源**：测试 guard（`cfg(test)`，生产零编译）> `SLTERM_DATA_DIR` env（E2E 隔离，空串视为未设置）> exe 同级推导。同模块承载：
- `LoadResult<T>`：`{ data, corrupted }`，无文件时 `data:null, corrupted:false`；损坏回退默认值或 `.bak` 命中时 `corrupted:true`（BE-14/D11）；
- `MAX_PERSIST_BYTES = 1MB`：save 侧大小上限，settings/projects 共用（SEC-11）；
- `AppDataDirGuard`：测试用 RAII 注入覆盖应用目录（SPE-04）。

### home.rs — 用户 home 目录解析单点（ADR-0016，E2E 假 home 隔离键）

- **home 解析统一经 `crate::home::home_dir()`**：优先级 = cfg(test) `HomeDirGuard` > env `USERPROFILE`（非空，空串视为未设置）> `dirs::home_dir()`。
- **Windows 事实（勿改，ADR-0016）**：dirs 6.0.0 / dirs-sys 0.5.0 的 `home_dir()` 走 `SHGetKnownFolderPath`，**完全不读 USERPROFILE/HOME env**——生产代码禁止裸 `dirs::home_dir()`（grep 收敛纪律，仅 cfg(test) 上下文允许；曾有两份照抄守卫复制 + watcher 一处裸调用，已全部收敛于此）。
- 消费点：hooks/claude（路径辅助/注入/statusline 备份）、hooks/watcher（信号目录——跨进程一致性承重墙）、plan_balance（余量来源）、agent_history/claude/scan.rs（fallback；`SLTERM_CLAUDE_PROJECTS_DIR` env 覆盖留 provider 内部，优先级高于 home）。
- 与 `app_dir.rs`（应用数据目录 = exe 同级/SLTERM_DATA_DIR）是两个不同概念目录：app_dir 管应用自身持久化，home 管用户配置（`~/.claude`、`~/.slterminal`）。

### settings.rs — 浅合并 + 保存互斥 + 白名单

- **浅合并**：`save_settings` 只写前端传入的顶层 slice，后端浅合并 top-level 键，各 store 各写各的互不覆盖；
- **`SETTINGS_SAVE_LOCK`**：前端三 store 启动时几乎同时触发 debounced 保存，`spawn_blocking` 闭包持锁串行化读-合并-写，避免 Windows persist rename 时句柄占用导致 PermissionDenied（SPE-06）；
- **SEC-11**：顶层键白名单（数组共 7 项）`["fontSize", "keybindings", "sideBar", "colorScheme", background_tasks::SETTINGS_KEY, "cliAliases", pty::spawn::SETTINGS_KEY]` + 序列化后大小上限 1MB。**键名聚合决策（F11）**：前端消费型五键（fontSize/keybindings/sideBar/colorScheme/cliAliases）无后端模块可归，键名集中于此字面量；后端消费型域键名归域模块——`backgroundTasks` 段经 `crate::background_tasks::SETTINGS_KEY` 引用（契约断链先例：fontSize store 曾发平铺键被拒，已改段形态双侧锁死），`conptyInputModes` 段（CP-009，ConPTY 输入模式能力矩阵，spawn 时后端读段，前端设置页「终端输入模式」写）经 `crate::pty::spawn::SETTINGS_KEY` 引用。`backgroundTasks` 段 = F12 后台定时任务配置（子键 per taskId：enabled/intervalSec）；写入侧除手改文件外专用命令通道 `background_tasks_set_config`（校验 → 读-改-写子键合并 → 本模块写通道落盘——禁止自建第二写通道）。`cliAliases` 段 = CLI 启动别名配置（子键 per cliId：别名数组，如 `{"claude":["cc"]}`）——**纯透传段**：语法与全命名空间唯一性校验全在前端 cliProfiles 域（`aliasValidation.ts`），本层不设专用命令/DTO（前端 profile 注册表是内置命令名唯一知识源，Rust 复刻即双源漂移，ADR-0014）。
- **`save_settings_blocking` 同步写通道（F12 抽取）**：校验（白名单 + 大小上限）→ 浅合并 → 原子写 + .bak，供 async `save_settings` 命令与 `background_tasks::set_config_core`（spawn_blocking 内，跨 await 持 MutexGuard 不可行）共用——全仓唯一 settings.json 写通道，禁止另建。**读侧语义（R2b）**：读现有文件经 `read_existing_settings` 共用读法——文件不存在（NotFound）→ 按空数据合并（首次启动合法）；读失败/解析失败 → Err 传播且不落盘（曾 `.ok()` 吞错走 Null 覆盖致顶层键全丢仍写成功，已修复）——与 load 侧损坏 `.bak` 回退语义刻意不同。

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

### preview.rs — 预览渲染独立 webview 窗口域（S10-②，ADR-0019）

docViewer 预览内容渲染于独立 WebviewWindow（label = `preview-<panelId>`，前端 `makePreviewLabel` 对应）的窗口/内容管理单点：

- **预览 CSP 域 = 自定义协议 `slterm-preview`**（Windows 映射 `http://slterm-preview.localhost`，tauri 文档实证）：`register_uri_scheme_protocol` 注册于 lib.rs run()；host_protocol 只服务宿主页（固定桥接页内嵌 const，建 sandbox iframe + 上下行中继），响应不带全局 CSP——资产协议页恒被注入全局 CSP，收紧后（CP-012）运行时内联脚本在资产域不可行（决策详见 ADR-0019 决策二）。
- **内容存储**：模块级 `CONTENT_STORE`（parking_lot Mutex + HashMap，label → {seq, html, bg}）——渲染产物经 `preview_render` 存储并 emit 定向 ping（`preview:render-ping`），宿主页经 `preview_pull` 拉取（seq 防乱序；宿主加载即拉一次兜底事件丢失窗口）。
- **label 校验**：须 `preview-` 前缀 + ASCII 字母数字/下划线/连字符、≤96 字符——全部命令入口统一校验（防任意窗口操纵）。
- **几何换算**：`preview_sync` 收 CSS 视口坐标（前端锚点矩形），经主窗 inner 原点 + scale_factor 换算物理屏幕坐标驱动位置/尺寸/显隐；隐藏态不建窗（内容先行存储，宿主加载后拉取）；窗口 = owned 无边框、focusable(false)、skip_taskbar。
- **命令一律 async + run_on_main 编排**：窗口创建/几何/显隐等窗口域操作要求主线程（tao/wry 事件泵）——sync 命令在主线程 IPC 回调内建窗会等消息泵死锁（2026-09-08 实测 30s 超时）；窗口域操作一律经 `run_on_main`（mpsc + run_on_main_thread）回主线程执行并取回结果。
- **命令清单**：preview_sync / preview_close / preview_render / preview_pull（lib.rs + build.rs + capabilities 三处注册）。

### parking_lot 换装（CP-005）

`state.rs` 等全部持锁站点用 `parking_lot::Mutex/RwLock`，中毒攻击面消除（锁内 panic 不再连锁 panic 等待方）；新建持锁临界区一律 parking_lot，禁止再引入 `std::sync::Mutex/RwLock`（grep 守卫）。

## 外部坑/红线

- **新增命令必须三处注册**：`lib.rs` 的 `generate_handler!`、`build.rs` 的 `AppManifest::new().commands(...)`、`capabilities/default.json` 的 `allow-<cmd>`（SEC-07），缺一即 invoke reject。
- **阻塞 I/O 一律 `spawn_blocking`**：不得在 async 命令体直接跑阻塞 I/O（硬约束 #3）。
- **capabilities/ 按窗口域声明权限**：Tauri 2 自定义命令经 build.rs `AppManifest::commands` 生成 allow-<cmd>（SEC-07），capabilities 文件按窗口 label 白名单逐条 allow——主窗口 = default.json；**预览窗口域 = preview.json**（windows glob `preview-*`，最小权限：core:event:default + allow-preview-pull——宿主页桥仅事件收发与内容拉取）；新窗口域须配对应 capability 文件，不追加通配 `*`（硬约束 #10）。
- **DTO 改 Rust 单源生成（CP-024）**：DTO 面由 Rust `#[derive(TS)]` 生成 `src/types/`（硬约束 #4）；改 DTO = 改 Rust 字段/serde/`#[ts]` 属性 → 跑 `cargo test --test lib_tests export_bindings -- --test-threads=1` → `git diff --exit-code -- src/types` 守卫（CI 门禁 step 已落地——ci.yml Guard — src/types），JS 侧生成物禁手改。
- **`project_root_lock` 必须覆盖 canonicalize+apply 全程**：不要拆锁，否则有慢路径覆盖风险（SEC-16）。
- **不要在持锁临界区引入 panic**：parking_lot 无中毒（守卫 Drop 自动释放），但临界区仍保持短小无 panic 纪律（CP-005）。
- **settings 顶层键白名单勿擅自扩充**：前端各 store 独立写入依赖此白名单。

## 测试模式

- 测试 `#[cfg(windows)]` 原则上改运行时 `cfg!(windows)` 分支；依赖 Windows 编译期 API（symlink 等）无法运行时区分的例外保留 cfg，须在所属模块 CLAUDE.md 登记豁免（硬约束 #9）。
- 命令注册完整性由 L2 IPC 契约测试间接守护。

### 既定豁免

| 豁免项 | 原因 | 当前兜底 |
|--------|------|---------|
| Windows symlink 特权测试 | 创建 symlink 需管理员/developer mode | `#[cfg(windows)]` 保留；失败时 skip |
