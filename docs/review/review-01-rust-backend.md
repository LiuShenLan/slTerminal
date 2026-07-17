# Review: Rust 后端代码质量

## [严重级别:高] 多数 PTY 命令仍是同步命令并直接执行阻塞 I/O

- **位置**
  - `src-tauri/src/pty/spawn.rs:600` `pty_spawn`
  - `src-tauri/src/pty/spawn.rs:764` `pty_write`
  - `src-tauri/src/pty/spawn.rs:786` `pty_resize`
  - `src-tauri/src/pty/spawn.rs:847` `pty_reattach`

- **问题**
  上述命令均为同步 `#[tauri::command] pub fn`，直接在调用线程里执行阻塞操作：创建 ConPTY / `CreateProcessW`、向 PTY 管道 `write_all`/`flush`、`ResizePseudoConsole`、从 ring buffer drain 并 `Channel::send`。硬约束 #3 要求"阻塞 I/O 用 `spawn_blocking`"，且 `pty_kill` 已按此改为 `async` + `spawn_blocking`，但其余 PTY 命令未改。Tauri 同步命令会占用 webview/tokio 调度线程，导致 IPC 调用期间 UI 冻结。

- **建议**
  将四个命令统一改为 `async fn`，把阻塞部分包入 `tokio::task::spawn_blocking`：
  - `pty_spawn`：`create_conpty_pair`、`CreateProcessW`、`take_writer`、CPR 注入；
  - `pty_write`/`pty_resize`：管道写/刷新、`ResizePseudoConsole`；
  - `pty_reattach`：ring buffer 拷贝与 `Channel::send`。

## [严重级别:高] `validate_path_within_root` 对不存在的相对路径存在沙箱绕过

- **位置** `src-tauri/src/state.rs:99-135`

- **问题**
  当目标路径不存在且为相对路径（如 `..\secret.txt`）时，函数会沿父级上溯到第一个存在的祖先，再用 `canonical_root.join(remainder)` 拼接，最后用 `Path::starts_with` 做词法前缀比较。`starts_with` 不会解析 `..`，因此 `C:\project\..\secret.txt` 会被判定为在根内，而后续真实的 `std::fs` 操作会解析到 `C:\secret.txt`，造成项目根外读写。

- **建议**
  对相对路径先以 `project_root` 为基准解析为绝对路径，再尝试 `dunce::canonicalize`；若目标不存在，应拒绝或至少对拼接结果再做一次规范化/前缀校验，不要依赖 `join` + `starts_with` 的词法比较。

## [严重级别:高] Job Object 分配失败时子进程可能成孤儿

- **位置** `src-tauri/src/pty/spawn.rs:686-705`

- **问题**
  `spawn_conpty_child` 成功后才调用 `add_to_job_object`。若 `OpenProcess`/`AssignProcessToJobObject` 失败，`?` 会提前返回，`child` 局部变量被 drop。`RawChild` 只关闭进程句柄，不终止进程，且此时还没有 `JobHandle`，子进程不再受 `KILL_ON_JOB_CLOSE` 保护，可能遗留为孤儿进程。

- **建议**
  在 `add_to_job_object` 失败路径显式调用 `child.kill()`；或把 Job Object 创建逻辑移入 `spawn_conpty_child` 内部，使其成为 spawn 不可分割的一部分，失败即销毁已创建的子进程。

## [严重级别:中] `fs_write_file` 为检测行尾读取整个文件

- **位置** `src-tauri/src/fs/mod.rs:79-87`

- **问题**
  代码用 `std::fs::read(&path)` 把整个文件读入内存，只是为了取前 64KB 判断是否有 `\r\n`。保存大文件时会造成与文件大小相等的瞬时内存占用，极端情况下可能 OOM。

- **建议**
  改用 `std::fs::File::open` 并只读取前 `CRLF_SAMPLE_MAX_BYTES` 字节作为样本。

## [严重级别:中] `fs_rename` 对 Windows 上已存在的文件不会覆盖

- **位置** `src-tauri/src/fs/mod.rs:244-250`

- **问题**
  注释声明"目标路径存在时覆盖"，但代码只在目标为目录时 `remove_dir_all`，对已有文件直接调用 `std::fs::rename`。Windows 上 `rename` 遇到目标文件已存在会返回错误，导致行为与注释不符且跨平台不一致。

- **建议**
  在 `std::fs::rename` 前统一处理已存在目标：目录则 `remove_dir_all`，文件则 `remove_file`。

## [严重级别:中] `save_settings` 在 `spawn_blocking` 外执行阻塞 I/O

- **位置** `src-tauri/src/settings.rs:34-37`

- **问题**
  `save_settings` 是 async 命令，但在进入 `spawn_blocking` 之前就调用了 `std::fs::create_dir_all(&app_dir)?`，会阻塞 tokio worker 线程。

- **建议**
  把 `create_dir_all` 移入 `spawn_blocking` 闭包内部。

## [严重级别:中] Git 仓库缓存可能返回错误的子仓库

- **位置** `src-tauri/src/git/mod.rs:78-90`

- **问题**
  `get_or_open_repo` 的缓存命中条件包含 `workdir.starts_with(&search)`。如果缓存里已有一个子目录仓库（如子模块），后续用父目录调用时会命中并返回该子仓库。例如缓存 `/project/submodule` 后调用 `git_status("/project")` 会错误使用 `/project/submodule` 的 repo 对象。

- **建议**
  删除 `workdir.starts_with(&search)` 分支，仅保留 `search.starts_with(workdir)`；子目录访问应通过 `Repository::discover` 得到正确的仓库，而不是返回缓存中的子仓库。

## [严重级别:中] 自定义命令未在 capabilities 中显式放行

- **位置**
  - `src-tauri/src/lib.rs:24-28`、`src-tauri/src/lib.rs:65-84`
  - `src-tauri/capabilities/default.json`

- **问题**
  `default.json` 只声明了插件权限，未列出 `ping`、`pty_*`、`fs_*`、`git_*`、`notify_*`、`get_windows_build_number` 等自定义命令；`lib.rs` 注释也明确说自定义命令"无需在 `capabilities/default.json` 中逐条声明"。这与架构硬约束 #10"新增命令必须在 `capabilities/` 显式放行，不用通配 `*`"直接冲突。

- **建议**
  确认 Tauri 2 当前版本是否支持在 capabilities 中限制自定义命令；若支持则补全白名单，否则应更新根 CLAUDE.md 中的硬约束 #10 以避免误导。

## [严重级别:低] `RawChild::try_wait`/`wait` 未检查 `WaitForSingleObject` 错误返回值

- **位置** `src-tauri/src/pty/spawn.rs:278-289`、`293-304`

- **问题**
  `try_wait` 只把 `WAIT_TIMEOUT`(0x102) 当未退出，其余值（包括 `WAIT_FAILED` 0xFFFFFFFF）都直接进入 `GetExitCodeProcess`，可能读到尚未终止进程的过时退出码。`wait` 则完全忽略 `WaitForSingleObject` 返回值。

- **建议**
  明确检查返回值是否为 `WAIT_OBJECT_0`，非预期值时返回 `std::io::Error`。

## [严重级别:低] `RawChild::clone_killer` 在句柄复制失败时 panic

- **位置** `src-tauri/src/pty/spawn.rs:259-269`

- **问题**
  `clone_killer` 对 `try_clone()` 使用 `.expect("RawChild clone_killer: 复制进程句柄失败")`。若句柄表耗尽等罕见情况发生，会直接 panic。

- **建议**
  返回 `std::io::Result<Box<dyn ChildKiller + Send + Sync>>`，把失败交给调用方处理。

## [严重级别:低] PTY 模块多处 `expect` 在锁中毒时 panic

- **位置** `src-tauri/src/pty/spawn.rs:199`、`215`、`219`、`226`、`251`、`262`、`276`、`297`、`312` 等

- **问题**
  `ConPtyInner`、`RawChild` 的多个方法在获取 `Mutex` 锁时使用 `.expect("... lock poisoned")`。锁中毒通常意味着线程 panic，但把这些错误转成 panic 会让 Tauri 命令直接崩溃，而不是返回可处理的 `AppError`。

- **建议**
  将锁中毒处理为 `std::io::Error` 或 `anyhow::Error`，并通过 trait 方法返回 `Err`。

## [严重级别:低] `build_env_block` 环境块顺序不确定

- **位置** `src-tauri/src/pty/spawn.rs:101-120`

- **问题**
  用 `HashMap` 收集环境变量后再迭代写入环境块，`HashMap` 迭代顺序随机。虽然绝大多数应用不依赖环境变量顺序，但少数 legacy 脚本或工具可能受影响，且不利于问题复现。

- **建议**
  用 `Vec<(String, String)>` 或保持 `std::env::vars()` 原始顺序，仅对 `extra_envs` 做覆盖。

## [严重级别:低] `which_full_path` 未按平台分隔 PATH

- **位置** `src-tauri/src/pty/shell.rs:129-139`

- **问题**
  代码固定按 `;` 分割 `PATH`。在非 Windows 平台 `PATH` 分隔符是 `:`，且路径中可能包含 `;`，会导致解析错误。虽然 `resolve_shell_info` 的结果在 `#[cfg(not(windows))]` 分支被忽略，但函数本身已被调用，行为不可移植。

- **建议**
  使用 `std::env::split_paths` 按平台分隔符处理。

## [严重级别:低] `SPAWN_LOCK` 保护范围大于注释说明

- **位置** `src-tauri/src/pty/spawn.rs:606-704`

- **问题**
  注释声明锁只保护"ConPTY 创建 + 子进程启动"，但代码实际把 `master.take_writer`、CPR 注入、`spawn_conpty_child`、`add_to_job_object` 都包在锁内，锁粒度比设计粗，延长了串行化时间。

- **建议**
  仅围绕 `create_conpty_pair` 与 `spawn_conpty_child` 加锁；`take_writer`、CPR 注入、Job Object 绑定可在锁外执行。

## [严重级别:低] 启动序列剥离不在多轮 read 中持续生效

- **位置** `src-tauri/src/pty/reader.rs:85-94`

- **问题**
  `apply_startup_strip` 只要处理过一次数据就设置 `startup_drained = true`。如果 ConPTY 启动序列跨越 16KB 缓冲区边界（例如本轮只剩部分 OSC 标题，下轮才是清屏/归位序列），后续序列不会被剥离。

- **建议**
  在确认已经遇到真实非启动输出之前，保持状态机持续剥离；或至少当本轮数据全部被识别为启动序列时才继续下一轮剥离。

## [严重级别:低] `COORD` 尺寸转换可能产生负值

- **位置** `src-tauri/src/pty/spawn.rs:205-208`

- **问题**
  `size.cols as i16` / `size.rows as i16` 在传入值超过 32767 时会回绕为负数，进而传入 `ResizePseudoConsole`。

- **建议**
  在 `SpawnRequest` 反序列化或转换前校验 `cols`/`rows` 不超过 `i16::MAX`。

## [严重级别:低] `notify_watch` 在持有池锁期间创建 watcher

- **位置** `src-tauri/src/notify/mod.rs:233-250`

- **问题**
  `notify_watch` 先锁住 `state.file_watchers`，然后在锁内调用 `FileWatcher::start`（含 `debouncer.watch` 等可能阻塞的操作）。大目录下注册 `ReadDirectoryChangesW` 可能耗时数秒，期间其他需要访问 watcher 池的操作都被阻塞。

- **建议**
  在锁外完成 `FileWatcher::start`，再短暂持锁插入池内。

## [严重级别:低] Win32 FFI 多处 unsafe 块缺少 SAFETY 注释

- **位置** `src-tauri/src/pty/spawn.rs:130-433` 内多个 `unsafe` 块

- **问题**
  `CreatePseudoConsole`、`CreateProcessW`、`ClosePseudoConsole`、`WaitForSingleObject`、`InitializeProcThreadAttributeList`、`UpdateProcThreadAttribute`、`TerminateProcess` 等 unsafe 调用周围缺少 SAFETY 注释，未说明调用者如何保证指针/句柄有效性。

- **建议**
  为每个 unsafe 块补充简短 SAFETY 注释，说明为何当前上下文满足 Win32 API 的前置条件。

---

## 实际 review 过的文件清单

- `src-tauri/src/main.rs`
- `src-tauri/src/lib.rs`
- `src-tauri/src/error.rs`
- `src-tauri/src/state.rs`
- `src-tauri/src/settings.rs`
- `src-tauri/src/fs/mod.rs`
- `src-tauri/src/git/mod.rs`
- `src-tauri/src/notify/mod.rs`
- `src-tauri/src/notify/pool.rs`
- `src-tauri/src/pty/mod.rs`
- `src-tauri/src/pty/spawn.rs`
- `src-tauri/src/pty/reader.rs`
- `src-tauri/src/pty/shell.rs`
- `src-tauri/src/pty/win_build.rs`
- `src-tauri/tests/pty_integration_tests.rs`
