# slTerminal Rust 后端架构审查报告

> 审查日期：2026-07-12 | 审查范围：src-tauri/src/ 全部 Rust 源代码

---

## 1. 总体评价

**代码质量：高**。项目展现了扎实的 Rust 工程实践：

- **Windows ConPTY 深度集成**：绕过 portable-pty 限制，自行实现 ConPTY 创建/进程管理/DA1 注入/启动序列剥离，细节处理到位
- **并发安全**：SPAWN_LOCK 串行化、RwLock 合理使用、锁嵌套文档化
- **资源管理**：Job Object 孤儿防护、RAII 全链路覆盖、ring buffer FIFO 淘汰
- **错误处理**：统一 AppError 枚举 + From trait 转换链，错误信息可序列化到前端
- **测试覆盖**：纯函数抽取（strip_conpty_startup、classify_by_kind、mirror_da1_query）、Mutex 锁粒度测试、LRU 淘汰测试——总计约 193 条 L1 用例

主要待改进点：生产代码中存在 Mutex `.unwrap()` 可能 panic 的路径、`clone_killer()` 使用 `unimplemented!()` 风险、部分错误被静默吞掉。

---

## 2. 各模块详细分析

### 2.1 PTY 模块（src-tauri/src/pty/）

#### 2.1.1 spawn.rs（~900 行）

**spawn 流程分析：**

```
pty_spawn() 流程：
1. 获取 SPAWN_LOCK（Mutex<()>）
2. 解析 shell → ShellInfo（完整路径）
3. 注入环境变量（COLORTERM/TERM/TERM_PROGRAM）
4. 创建 ConPTY 管道对（CreatePseudoConsole with PASSTHROUGH_MODE flag）
5. take_writer（仅一次）→ Arc<Mutex<>> 共享
6. 写 CPR \x1b[1;1R 到 stdin（补偿 ConPTY DSR 握手）
7. CreateProcessW 启动子进程
8. 子进程放入 Job Object（KILL_ON_JOB_CLOSE）
9. SPAWN_LOCK 释放
10. 创建 reader 线程 + 注册到 PtyState
```

SPAWN_LOCK 仅保护步骤 2-8（ConPTY 创建 + 子进程启动），reader 线程和 sessions 插入在锁外执行——临界区最小化，设计正确。

**问题列表：**

| # | 严重度 | 位置 | 描述 |
|---|--------|------|------|
| P1 | 🔴 | `spawn.rs:256` | `RawChild::clone_killer()` 使用 `unimplemented!()`。虽然当前项目代码从不调用此方法（child 通过 `Arc<Mutex<>>` 管理），但若将来引入的代码通过 `Box<dyn ChildKiller + Send + Sync>` 的 Clone 实现间接调用，将触发 panic 导致进程崩溃。建议返回一个存根实现（如返回自身引用计数克隆），或至少在文档中明确标注调用条件和风险 |
| P2 | 🟡 | `spawn.rs:193,209,213,220` | `ConPtyMaster` 的 `resize`/`get_size`/`try_clone_reader`/`take_writer` 中使用 `self.inner.lock().unwrap()`。若 Mutex 被毒化（其他线程 panic 持锁），这些调用将级联 panic。建议改为 `.expect("ConPtyInner lock poisoned")` 以提供明确的崩溃信息 |
| P3 | 🟡 | `spawn.rs:245,264,285,300` | `RawChild` 的 `kill`/`try_wait`/`wait`/`as_raw_handle` 中使用 `self.proc_handle.lock().unwrap()`——同上 |
| P4 | 🟡 | `spawn.rs:426` (pty_spawn) | `get_windows_build_number().unwrap_or(0)`——若 `get_windows_build_number()` 调用失败，build=0 会导致 flags 计算为 `0x7`（无 PASSTHROUGH_MODE），丢失约 10-20x 吞吐提升。虽然这是安全降级（Win10 兼容），但错误被静默吞掉。建议至少 `tracing::warn!` 记录降级 |
| P5 | 🔵 | `spawn.rs:255` | `RawChild::clone_killer` 文档注释说"此方法仅用于 trait 兼容，不应被调用"——但 trait 方法不应当有"不应被调用"的假设。见 P1 |

**ConPTY 自定义实现评价：**

- `compute_conpty_flags()` — 正确按 OS build 动态启用 PASSTHROUGH_MODE（22621+），4 条测试覆盖
- `AttrList` — RAII wrapper 完整：`Initialize` → `Update` → `Drop` 中 `DeleteProcThreadAttributeList`
- `ConPtyMaster` — 正确实现 `MasterPty` trait，`take_writer` 仅允许一次（第二次返回错误）
- `ConPtyInner::drop()` — 在 `ClosePseudoConsole` 前先 drop writer（确保子进程 stdin EOF 传递）
- `RawChild::try_wait()` — 非阻塞检查 `WaitForSingleObject(0)` → `WAIT_TIMEOUT` 返回 None
- `build_cmdline()` — 正确处理含空格路径的引号包裹
- `build_env_block()` — 继承当前进程环境 + 追加/覆盖 extra_envs

**Job Object 孤儿防护：**

- `add_to_job_object()` + `create_and_assign_job()` — 正确设置 `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`
- `JobHandle` RAII — 持有 job handle 防止过早触发，Drop 时 `CloseHandle`
- unsafe 块有详细的 SAFETY 文档注释

#### 2.1.2 reader.rs（~280 行）

**reader_loop 分析：**

三个分支正确性：Ok(0) EOF → child.wait() 获取真实退出码 → 发送 Exit 事件；Ok(n) → 首轮剥离启动序列 → DA1 注入检测 → ring buffer 缓存 → Channel 发送；Err → 记录错误退出码 → 发送 Exit。

**问题列表：**

| # | 严重度 | 位置 | 描述 |
|---|--------|------|------|
| P6 | 🟡 | `reader.rs:77,120,137` | `c.send(PtyEvent::Exit/Output)` 的返回值被 `let _ =` 吞掉。Channel 发送失败（前端已断开）是正常场景（页面切换），但应区分"前端断开"与"IPC 序列化错误"——当前实现无法区分。建议至少对 Channel::send 的错误做 `tracing::debug!` 记录 |
| P7 | 🟡 | `reader.rs:110-111` | DA1 响应注入的 `write_all` + `flush` 返回值被忽略。若写入失败（子进程 stdin 已关闭），DA1 响应无法送达，Claude Code 启动可能阻塞约 60s。建议记录 warn |

**纯函数设计评价：**

- `strip_conpty_startup()` — 纯函数，正确剥离 OSC 标题/BEL、清屏、光标归位、DSR。测试覆盖完整（21 条）
- `apply_startup_strip()` — 首轮 vs 非首轮分支纯函数化
- `mirror_da1_query()` — 滑动窗口扫描，排除 DA2 (`ESC[>c`) 和 XTVERSION
- `should_inject_da1()` — 组合 `AtomicBool` flag + `mirror_da1_query`，纯函数可测试

reader_loop 中无法纯函数化的部分（I/O 编排：channel read/write、child.wait()、ring buffer 写入）已在 M11 注释中完整分析。

#### 2.1.3 shell.rs（~240 行）

**Shell 选择链：** pwsh.exe → powershell.exe → cmd.exe，正确。

**关键设计：**
- `resolve_shell_info()` 返回 `ShellInfo.program` 为完整路径（通过 `which_full_path()` 在 PATH 中解析），确保 `CreateProcessW(lpApplicationName=...)` 正确定位
- PowerShell 通过 `-EncodedCommand` + Base64 UTF-16LE 内联集成脚本，避免 `%APPDATA%` 文件写入被 AMSI/ASR 误杀
- `include_str!("../../assets/shell-integration.ps1")` 编译期嵌入，分发 exe 不依赖外部脚本路径

**问题：无严重问题。** `resolve_shell_info` 的用户指定 shell 路径解析逻辑正确（已有路径分隔符→原样使用，短名→PATH解析）。

#### 2.1.4 build.rs（PTY 子模块）

单纯获取 Windows build 号（`nt_version::get()` 提取低 28 位）。非 Windows 返回 `AppError::Unknown`——前端降级用 build=21376。无问题。

---

### 2.2 文件系统模块（src-tauri/src/fs/mod.rs）

#### 2.2.1 路径沙箱（validate_path_within_root）

```rust
canonicalize(root) → canonicalize(target) → starts_with 检查
```

**覆盖的攻击向量：**
- 根外路径 → canonicalize 后不匹配 → 拒绝
- 路径穿越 `../` → canonicalize 解析真实路径 → 拒绝
- 符号链接指向根外 → canonicalize 跟踪 → 拒绝（测试覆盖）
- 目标不存在 → 沿父级上溯找到已存在祖先 → 拼接剩余路径再检查
- project_root 未设置 → 跳过校验（开发兼容）

**问题列表：**

| # | 严重度 | 位置 | 描述 |
|---|--------|------|------|
| P8 | 🟡 | `mod.rs:266` (fs_rename) | 若目标路径已存在且为目录，先 `remove_dir_all` 再 `rename`——这可能意外删除数据（用户误操作覆盖目录）。建议与前端确认是否需要二次确认对话框 |
| P9 | 🟡 | `mod.rs:226` (fs_write_file) | CRLF 检测取前 64KB 样本——若文件前 64KB 不含 CRLF 但后面有（极端场景），会导致行尾切换。影响极小但值得记录 |
| P10 | 🔵 | `mod.rs:333` | `as_tauri_state()` 使用 `transmute` 将 `&T` 转为 `tauri::State<T>`。仅测试使用，依赖 `State(&T)` 内部布局不变。若 Tauri 未来改变 State 布局，此假设失效。但仅影响测试编译/运行，不影响生产 |

**CRLF 行尾保持逻辑评价：**

`fs_write_file` 中行尾检测和转换逻辑正确：
- 原文件 CRLF → 将 LF 转为 CRLF
- 原文件 LF → 保持 LF  
- 新文件 → Windows 默认 CRLF
- 混合行尾 → 统一为 CRLF（如原文件为 CRLF）

**fs_read_dir 过滤策略：** 仅过滤 `.git/`，`node_modules/`、`target/` 等大目录不再硬编码过滤——依赖前端懒加载控制性能。合理。

---

### 2.3 Git 模块（src-tauri/src/git/mod.rs）

#### 2.3.1 核心设计

- **仓库缓存**：`get_or_open_repo()` 以 workdir 为 key 缓存 `git2::Repository`，从磁盘重新打开独立实例返回（绕过 git2 生命周期限制）
- **缓存命中检测**：子目录在已缓存 workdir 子树内 → 命中
- **git_status**：`status_to_str()` 纯函数映射 12 种 git2 Status flags
- **git_diff**：行级 line callback 实现 modified/added/deleted hunk 合并（'-'→'+' 配对为 ModifiedMarker）

**问题列表：**

| # | 严重度 | 位置 | 描述 |
|---|--------|------|------|
| P11 | 🟡 | `mod.rs:~80` (get_or_open_repo) | 缓存命中时通过 `search.starts_with(workdir) || workdir.starts_with(&search)` 检测——前者正常（子目录在 workdir 内），后者（workdir 是 search 子路径）可能匹配到不相关的仓库。例如：search="/a/b" 而 workdir="/a"（"/a" 是 "/a/b" 的前缀但不是祖先）。好在 `git2::Repository::open` 会验证 workdir 确实有 `.git` 目录，错误匹配会被后续操作过滤。建议去除此反向检测 |
| P12 | 🔵 | `mod.rs:~950` (test) | `compute_diff_hunks()` 在测试中完整复制了生产代码的 diff 行回调逻辑——DRY 违反。若生产代码逻辑变更，测试可能不同步。建议抽取为公用函数 |

**git_diff 行回调逻辑评价：**

- '-'→'+' 配对检测通过 `prev_was_del` flag 实现——正确
- context 行触发 flush → 隔离不同变更组——正确
- 末尾残留组通过函数返回前的 `flush_pending` 处理——正确
- 非标准 diff（'+'→'-' 顺序）会被拆分为独立的 added + deleted hunk（降级而非错误）——合理

#### 2.3.2 性能

- `git_diff` 使用 `diff_tree_to_workdir_with_index` 单文件 pathspec，非全仓库 diff
- `git_status` 使用 `StatusOptions::include_untracked(true)` 扫描全仓库（无 pathspec 限制）
- 两者均通过 `spawn_blocking` 包装，不阻塞 tokio runtime

---

### 2.4 Notify 模块（src-tauri/src/notify/）

#### 2.4.1 架构

```
前端 fs_watch(path)
  → pool.pause_all_except(path)  // 暂停其他 watcher
  → pool.get(path) 命中？→ 返回（resume 生效）
  → 未命中 → FileWatcher::start() → pool.insert()  // 池满则 LRU 淘汰
```

**问题列表：**

| # | 严重度 | 位置 | 描述 |
|---|--------|------|------|
| P13 | 🔵 | `mod.rs:162,165,197,200` | `FileWatcher::stop()` 和 `Drop` 中 `let _ = tx.send(())` 和 `let _ = handle.join()`——发送失败（接收端已断开）和 join 失败的错误被忽略。虽然线程崩溃时 join 返回 Err 意味着线程 panic，但当前实现无法感知 |
| P14 | 🔵 | `Cargo.toml` | `notify = "9.0.0-rc.4"` 使用 RC 版本。文档注释（P1-25）已标注关注正式版后升级。当前 RC 在功能上稳定但 API 可能有 breaking changes |

**LruWatcherPool 评价：**

- LRU 淘汰：`min_by_key(|(_, e)| e.last_used)` → 淘汰最久未访问 watcher——正确
- `pause_all_except()`：目标 path resume，其余 pause——正确。空池无 panic
- `insert` 替换：同 path 再次插入时先 stop 旧 watcher 再插入新——正确
- `stop_all` + `Drop`：双重保障线程资源释放——正确
- 池容量 5：够覆盖典型多项目切换场景

**FileWatcher 生命周期：**

- `start()` → spawn "fs-watcher" 线程 → debouncer 存活于线程内
- `stop()` → 发送停止信号 → join 线程（幂等——stop_tx.take() 后再次 stop 不 panic）
- `pause()`/`resume()` → `AtomicBool` 控制事件上报，watcher 线程和 OS 句柄保留
- `Drop` → 发送停止信号 + join 线程（确保 OS 句柄释放）

---

### 2.5 状态管理（src-tauri/src/state.rs）

**AppState 结构：**

```rust
AppState {
    pty: PtyState,                          // PTY 会话管理
    file_watchers: Mutex<LruWatcherPool>,   // 文件监听器池
    project_root: RwLock<Option<PathBuf>>,   // 当前项目根路径
    git_repo_cache: Mutex<HashMap<PathBuf, Repository>>, // git 仓库缓存
}
```

**锁粒度分析：**

| 操作 | 锁路径 | 风险评估 |
|------|--------|---------|
| pty_spawn | spawn_lock.lock() → sessions.write() | 安全：spawn_lock 是独立 Mutex |
| pty_write | sessions.read() → writer.lock() | 安全：外层读锁 + 内层独立 Mutex |
| pty_resize | sessions.read() → master.lock() | 安全：无反向路径。文档已标注未来重构风险 |
| pty_kill | sessions.write() → 释放 → spawn_blocking | G1b：提取 session 后释放锁再 join 线程 |
| pty_reattach | sessions.read() → channel.write() + ring.lock() | 安全：外层读锁 |
| fs_watch | file_watchers.lock() → pool.pause_all_except | 安全：单层锁 |
| git_status | git_repo_cache.lock() → spawn_blocking | 安全：锁在 spawn_blocking 前释放 |

**无已知死锁风险。** `pty_kill` 的 G1b 模式（async + spawn_blocking + 先释放 RwLock 再 join）是正确实践。

**问题列表：**

| # | 严重度 | 位置 | 描述 |
|---|--------|------|------|
| P15 | 🔵 | `state.rs:36` | `PtySession` 中 `#[cfg(windows)] pub job_object` 注释说"符合架构约束 #9 精神"。严格来说架构约束 #9 要求 `#[cfg(windows)]` 只在 pty/ 模块出现，此处出现在 state.rs。功能上正确（job_object 只在 PTY 创建时使用），建议将 JobHandle 封装在 PtySession 内部（或移到 pty/ 模块的类型定义中） |

**ring_buffer_append 评价：**

- 容量 256KB，超过时按约 1KB 粒度整行淘汰
- `rposition(|&b| b == b'\n')` 在 drain_target 范围内找最后一个换行，对齐行边界
- 无换行时（超长行，罕见）按 drain_target 淘汰

---

### 2.6 命令注册（src-tauri/src/lib.rs）

**generate_handler! 注册的命令（20 个）：**

```
ping, get_windows_build_number,
pty_spawn, pty_write, pty_resize, pty_kill, pty_reattach,
fs_read_file, fs_write_file, fs_read_dir, fs_create_dir, fs_delete, fs_rename,
set_project_root, clear_git_cache,
git_status, git_diff,
fs_watch,
save_settings, load_settings
```

**问题列表：**

| # | 严重度 | 位置 | 描述 |
|---|--------|------|------|
| P16 | 🔵 | `lib.rs:27-33` | 注释说明"Tauri 2 默认行为：invoke_handler 注册的自定义命令自动对所有窗口放行"。这意味着当前权限模型依赖 Tauri 2 的默认宽松模式。若未来 Tauri 版本收紧默认行为，所有命令可能需要显式白名单。建议在 `capabilities/default.json` 中预先声明所有自定义命令权限（即使当前非必需） |
| P17 | 🔵 | `lib.rs:40-48` | `#[cfg(debug_assertions)]` 控制 `wdio-webdriver` 插件初始化——这是编译期条件而非运行时 cargo feature。生产构建中插件代码存在（Cargo.toml 始终依赖）但从不初始化。如前述，这无法在 Cargo.toml 层面按 profile 排除。但可考虑使用自定义 feature flag 替代 `#[cfg(debug_assertions)]` |

**参数验证：** 各命令在函数体内做基本校验（如 `pty_write` 检查 session_id、`fs_read_file` 做路径沙箱校验），无独立验证层。当前规模下合理。

---

### 2.7 错误处理（src-tauri/src/error.rs）

**AppError 枚举覆盖度：**

| 变体 | 覆盖场景 | From 转换 |
|------|---------|----------|
| IoKind | 文件 I/O（路径校验/读写/元数据） | `std::io::Error` |
| Pty | PTY 创建/通信/锁 | 无（手动构造） |
| Git | git 操作 | `git2::Error` |
| Serde | JSON 序列化 | `serde_json::Error` |
| Unknown | 未分类错误 | 无（手动） |
| SessionNotFound | PTY session 不存在 | 无（手动） |
| TaskJoin | tokio spawn_blocking join 失败 | `JoinError` |
| Notify | 文件监听错误 | 无（手动） |

**问题列表：**

| # | 严重度 | 位置 | 描述 |
|---|--------|------|------|
| P18 | 🟡 | 多处 | 多个关键路径中使用 `let _ =` 静默吞掉错误（见 2.1.2 P6/P7、2.4 P13）。虽然这些错误在相应上下文中通常无害（Channel 断开 = 前端切换页面），但缺乏可观测性。建议统一使用 `tracing::debug!` 或 `tracing::trace!` 记录 |
| P19 | 🔵 | `error.rs` | `AppError::IoKind` 的 `kind` 字段使用 `format!("{:?}", e.kind())` 存储 `Debug` 输出——前端收到的是 Rust Debug 字符串（如 "NotFound"），而非稳定的错误码。前端如需精确匹配，建议改用 u8 错误码或独立枚举变体 |

---

### 2.8 其他

#### 2.8.1 Cargo.toml

| # | 严重度 | 位置 | 描述 |
|---|--------|------|------|
| P20 | 🔵 | `Cargo.toml` | `tauri-plugin-wdio-webdriver = "1"` 始终编译，但仅 `#[cfg(debug_assertions)]` 时初始化。生产 exe 中包含未使用的 WebDriver 代码（约 1-2MB）。注释已说明原因（debug_assertions 不是 Cargo target cfg predicate） |
| P21 | 🔵 | `Cargo.toml` | `notify = "9.0.0-rc.4"` 和 `notify-debouncer-full = "0.8.0-rc.2"` 使用 RC 版本。已有 P1-25 注释追踪 |

#### 2.8.2 tauri.conf.json

| # | 严重度 | 位置 | 描述 |
|---|--------|------|------|
| P22 | 🟡 | `tauri.conf.json` | `"csp": null` — CSP 关闭。对于本地桌面应用（非公网服务），此设置可接受（避免 WebView2 CSP 阻止必要资源）。但若未来引入远程内容加载，需重新启用 CSP |
| P23 | 🔵 | `tauri.conf.json` | `dragDropEnabled: false` — 拖放被禁用，window 配置干净 |

#### 2.8.3 main.rs

- `#![windows_subsystem = "windows"]` — 隐藏控制台窗口，正确
- `panic::set_hook` — 将 panic 信息写入 `crash.log`，诊断友好
- `unwrap_or_else(|_| ".".into())` — `current_dir()` 失败时使用当前目录，合理降级

#### 2.8.4 settings.rs

- **原子写入**：`NamedTempFile` → `flush()` → `persist()` → 写入前 `.bak` 备份——正确
- **浅合并**：`merge_settings` 逐键覆盖——支持 `fontSize` 和 `keybindings` 独立写入
- **损坏恢复**：`load_settings` JSON 解析失败 → 尝试 `.bak` → 仍失败返回 Null

| # | 严重度 | 位置 | 描述 |
|---|--------|------|------|
| P24 | 🟡 | `settings.rs:53` | `save_settings` 中 `.bak` 备份的 `std::fs::copy` 失败被 `let _ =` 忽略。若磁盘满或权限不足，旧 `.bak` 不会被更新——下次恢复可能回滚到过时数据。建议至少 `tracing::warn!` |
| P25 | 🔵 | `settings.rs:44` | `std::fs::create_dir_all(&app_dir)` 使用 `?` 传播错误——若 `~/.slterminal` 创建失败（如磁盘满），整个 save 失败。正确行为 |

---

### 2.9 capabilities 权限审查

`capabilities/default.json` 仅声明了以下 plugin 权限：

```json
"core:default", "core:window:allow-destroy",
"opener:default", "dialog:default",
"clipboard-manager:allow-write-text", "clipboard-manager:allow-read-text",
"wdio-webdriver:default"
```

自定义命令（20 个 Tauri command）依赖 Tauri 2 默认行为——`invoke_handler` 注册的命令自动对所有窗口放行。**权限模型中无违规**。如项目要迁移到显式白名单模式，需追加 20 条命令权限声明。

---

## 3. 架构约束合规性

| 约束 | 状态 | 说明 |
|------|------|------|
| #1 前端不碰 OS | ✅ | invoke 仅出现在 `src/ipc/` |
| #2 后端按功能分模块 | ✅ | pty/fs/git/notify 各自独立 |
| #3 命令统一注册 | ✅ | 20 条命令在 `generate_handler!` |
| #4 DTO 双边对应 | ✅ | `#[serde(rename_all = "camelCase")]` 全局使用 |
| #9 `#[cfg(windows)]` 仅在 pty/ | ⚠️ | `state.rs:36` 有 `#[cfg(windows)] pub job_object`——已注释说明"符合精神"，功能正确但位置不完全合规 |
| #10 权限最小化 | ✅ | capabilities 无通配 `*`，仅声明必要 plugin 权限 |

---

## 4. 重构建议汇总（按优先级排列）

### 🔴 严重（应尽快修复）

| # | 建议 | 涉及文件 |
|---|------|---------|
| P1 | `RawChild::clone_killer()` 替换 `unimplemented!()` 为实际实现（如返回自身克隆或 panic-free 存根） | `pty/spawn.rs:256` |

### 🟡 中等（建议下个迭代处理）

| # | 建议 | 涉及文件 |
|---|------|---------|
| P2/P3 | ConPtyMaster/RawChild 中 `Mutex::lock().unwrap()` 改为 `.expect("上下文")` | `pty/spawn.rs:193,209,213,220,245,264,285,300` |
| P4 | `get_windows_build_number().unwrap_or(0)` 添加 `tracing::warn!` 降级日志 | `pty/spawn.rs:426` |
| P6 | reader_loop 中 `c.send()` 失败添加 `tracing::debug!` 记录（区分正常断开 vs 序列化错误） | `pty/reader.rs:77,120,137` |
| P7 | DA1 响应注入失败添加 `tracing::warn!` | `pty/reader.rs:110-111` |
| P8 | `fs_rename` 覆盖目录行为评估——是否需要前端二次确认 | `fs/mod.rs:266` |
| P9 | `fs_write_file` CRLF 检测——考虑检测整个文件而非仅前 64KB | `fs/mod.rs:226` |
| P11 | `get_or_open_repo` 中 `workdir.starts_with(&search)` 反向检测评估 | `git/mod.rs:~80` |
| P18 | 统一用 `tracing::debug!` 记录被 `let _ =` 忽略的子系统错误 | 多处 |
| P22 | 评估是否需重新启用 CSP（当前 null 可接受但需文档化） | `tauri.conf.json` |
| P24 | `.bak` 备份 copy 失败添加 `tracing::warn!` | `settings.rs:53` |

### 🔵 建议（可延后）

| # | 建议 | 涉及文件 |
|---|------|---------|
| P5 | `clone_killer` 文档补充——移除"不应被调用"的假设性描述 | `pty/spawn.rs:255` |
| P10 | `as_tauri_state()` 添加 tauri::State 布局假设的测试断言 | `fs/mod.rs:333` |
| P12 | 抽取 `compute_diff_hunks` 为公用函数消除测试重复 | `git/mod.rs` |
| P13 | FileWatcher stop/Drop 中的 `let _ =` 加 tracing | `notify/mod.rs:162,165,197,200` |
| P14 | 跟踪 notify 正式版发布 → 升级依赖 | `Cargo.toml` |
| P15 | `#[cfg(windows)]` job_object 移动到 pty/ 模块类型定义 | `state.rs:36` |
| P16 | capabilities 预先声明自定义命令权限（可选，当前 Tauri 2 默认放行） | `capabilities/default.json` |
| P17 | 考虑用 feature flag 替代 `#[cfg(debug_assertions)]` 控制 wdio-webdriver | `lib.rs` / `Cargo.toml` |
| P19 | IoKind.kind 考虑改用稳定错误码而非 Debug 字符串 | `error.rs` |
| P20/P21 | 跟踪上游依赖发布（notify 正式版、wdio-webdriver feature flag 支持） | `Cargo.toml` |
| P25 | settings 目录创建失败已有 `?` 传播——无需改动 | `settings.rs:44` |

---

## 5. 亮点总结

1. **ConPTY 自定义实现质量高** —— 绕过 portable-pty 限制直接调 Win32 API，PASSTHROUGH_MODE 动态启用、RAII 全链路覆盖
2. **关键路径错误不吞** —— PTY spawn/kill/resize 中所有关键错误均通过 `?` 传播到 AppError
3. **纯函数抽取得当** —— `strip_conpty_startup`、`classify_by_kind`、`mirror_da1_query`、`status_to_str` 等纯函数极大方便测试
4. **并发模型清晰** —— SPAWN_LOCK 串行化 + RwLock 读写分离 + G1b 先释放锁再 join 线程
5. **Job Object 孤儿防护** —— 进程崩溃/退出时 OS 自动杀所有子进程，无僵尸进程
6. **shell 集成脚本编译期嵌入** —— `include_str!` 消除外部文件依赖，避免 AMSI/ASR 误杀
7. **设置原子写入 + bak 恢复** —— `NamedTempFile` + `.bak` 备份防止写中断导致配置丢失
8. **ring buffer 整行淘汰** —— FIFO 按 `\n` 边界淘汰，避免 UTF-8 序列截断
9. **CRLF 行尾保持** —— 写入文件时检测原文件行尾风格，避免 CodeMirror LF 归一化引起 git diff 误报
