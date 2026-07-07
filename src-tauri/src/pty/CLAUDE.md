# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 模块职责

PTY 管理——Windows ConPTY 终端模拟核心。负责 shell 进程的完整生命周期：发现→创建→读写→缩放→销毁→重连。

## 架构

```
shell.rs          → resolve_shell() 返回 CommandBuilder（传统路径）+ resolve_shell_info() 返回 ShellInfo（自定义 ConPTY 路径）
mod.rs/spawn.rs   → pty_spawn() 握 SPAWN_LOCK → conpty_custom::create_conpty_pair()（绕过 portable-pty，直接调 Win32）→ reader 线程（锁外）
reader.rs         → reader_loop() 独立线程阻塞读 → Channel 推 PtyEvent（READER_BUF_SIZE=16KB）
mod.rs            → pty_write / pty_resize / pty_kill / pty_reattach
state.rs          → PtySession 结构体 + PtyState 全局 HashMap
```

**PtyEvent 枚举**（`spawn.rs`，带 tag 的 serde 枚举，`camelCase` 序列化）：

- `Output { bytes: Vec<u8> }` — 原始终端输出字节
- `Exit { code: Option<i32> }` — 子进程退出

**PtySession**（`state.rs`）：`master`（Mutex<dyn MasterPty>）、`child`（Arc<Mutex<dyn Child>>）、`writer`（Arc<Mutex<dyn Write>>，take_writer 仅一次故共享）、`reader_handle`、`channel`（Arc<RwLock<Option<Channel>>>，可替换用于 reattach）、`output_ring`（256KB FIFO，Channel 断开时缓存）。

## 关键约束

### 架构硬约束 #9：`#[cfg(windows)]` 只允许在此处

所有 Windows 条件编译收敛于此模块。非 Windows 平台代码走 `cfg!(windows)` 运行时分支（如 `strip_conpty_startup` 原样返回）。业务逻辑不撒 cfg。

### SPAWN_LOCK 串行化

**并发 spawn 会卡死 ConPTY 输出管道**。`pty_spawn` 中 SPAWN_LOCK 仅保护 ConPTY 创建 + 子进程启动（`openpty` → `spawn_command`），reader 线程启动和 sessions 插入在锁外执行。

### CPR 注入（Windows）

`openpty()` 后立即向 stdin 写 `\x1b[1;1R`。补偿 ConPTY `VtIo::StartIfNeeded()` 的 DSR 握手，避免首次读取时 DSR 死锁。蜂鸣和首字符消失由 `reader.rs` 的 `strip_conpty_startup()` 处理——与此机制无关。

### cwd 反斜杠规范化

`CreateProcessW` 对 `/` 行为异常。传给 ConPTY 前将 cwd 中的 `/` 替换为 `\\`。

### stdin drop = 立即杀子进程（Windows）

**绝对不能 drop stdin writer**。writer 通过 `Arc<Mutex>` 共享，生命周期跟随 `PtySession`。

### Job Object 孤儿防护（Windows）

每个子进程放入 Job Object，设置 `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`。父进程崩溃/退出时 OS 自动杀所有子进程。

### pty_kill：async + spawn_blocking

`ClosePseudoConsole` 在 pre-Win11 24H2 上永久阻塞。`pty_kill` 先提取 session 释放 `RwLock` 写锁（<1ms），再在 `spawn_blocking` 中执行 `kill → join reader → drop master`，避免持锁阻塞导致后续命令级联卡死。

### Channel 可替换 + ring buffer 回放（E1）

`reader_loop` 通过 `Arc<RwLock<Option<Channel>>>` 引用 Channel。Channel 断开时写入 `output_ring`（256KB FIFO，按 1KB 粒度整行丢弃）。`pty_reattach` 替换 Channel 并回放 ring buffer 内容，用于前端页面切换后恢复终端显示。

### ConPTY 启动序列剥离

`strip_conpty_startup()` 在首轮读取时剥离 ConPTY `VtIo::StartIfNeeded()` 注入的序列：OSC 窗口标题（含 BEL→蜂鸣）、清屏 `ESC[2J/3J`、光标归位 `ESC[H`、光标显隐 `ESC[?25h/l`、DSR `ESC[6n`。后续读取原样透传。非 Windows 平台原样返回。

### Shell 选择与 profile 注入

`shell.rs`：`pwsh.exe` → `powershell.exe` → `cmd.exe` 回退。提供两套接口——`resolve_shell()` 返回 `CommandBuilder`（旧路径/非 Windows fallback），`resolve_shell_info()` 返回 `ShellInfo`（自定义 ConPTY 路径，program 为完整路径）。Shell 可执行文件通过 `which_full_path()` 在 PATH 中解析为完整路径，确保 `CreateProcessW(lpApplicationName=...)` 正确工作。PowerShell 通过 `-EncodedCommand`（UTF-16LE Base64）内联集成脚本（`include_str!("../../assets/shell-integration.ps1")`），消除 `%APPDATA%` 文件写入，避免 AMSI/ASR 误杀。集成脚本注入 OSC 7（cwd 跟踪）+ OSC 133 A/B/D（提示符边界+退出码）+ UTF-8 编码修复。

### 终端能力环境变量注入

`pty_spawn` 在 spawn 阶段注入三个环境变量（直接构造 `Vec<(String, String)>`，通过 `spawn_conpty_child` 的 `build_env_block` 合并到子进程环境块）：
- `COLORTERM=truecolor` — Chalk/supports-color 的核心检测信号，Claude Code 依赖此变量启用 24-bit RGB
- `TERM=xterm-256color` — 传统 terminfo 能力宣告（部分应用不看 COLORTERM，防御性设置）
- `TERM_PROGRAM=slTerminal` — 品牌标识（非功能性，行业惯例）

注入在 spawn 阶段（非 shell rc），确保子进程一启动即可见。三个变量对 pwsh/powershell/cmd 统一注入，不加 shell 类型判断。

### 自定义 ConPTY 创建（绕过 portable-pty）

`spawn.rs` 中的 `conpty_custom` 模块（`#[cfg(windows)]`，约 300 行）绕过 `portable_pty::native_pty_system().openpty()`，直接调用 `windows` crate 的 `CreatePseudoConsole`/`CreateProcessW`，实现对 ConPTY flags 的完全控制。

**动机**：`portable-pty` 0.9.0 内部硬编码 flags=0x7（INHERIT_CURSOR | RESIZE_QUIRK | WIN32_INPUT_MODE），不暴露 `CreatePseudoConsole` 的 `dwFlags` 参数，无法按 OS build 动态启用 `PASSTHROUGH_MODE`（0x8，Win11 22H2+ 约 10-20x 吞吐提升）。

**自定义组件**：
| 类型 | 职责 |
|------|------|
| `compute_conpty_flags(build)` | 计算 flags：基础 0x7，build ≥ 22621 追加 PASSTHROUGH_MODE |
| `AttrList` | `PROC_THREAD_ATTRIBUTE_LIST` RAII wrapper（`Initialize`→`Update`→`Delete`） |
| `ConPtyMaster` | 实现 `portable_pty::MasterPty`（resize/get_size/try_clone_reader/take_writer），持有通过 `windows` crate 创建的 HPCON |
| `RawChild` | 实现 `portable_pty::Child + ChildKiller`，封装 `OwnedHandle` + `TerminateProcess` |
| `create_conpty_pair()` | 创建管道对 → `CreatePseudoConsole(flags)` → 返回 `(HPCON, ConPtyMaster)` |
| `spawn_conpty_child()` | `AttrList::set_pty(hpc)` → `CreateProcessW` → 返回 `RawChild` |

**shell 信息解耦**：`shell.rs` 新增 `ShellInfo` 结构体和 `resolve_shell_info()` 函数，返回纯数据结构（program + args），不依赖 `portable_pty::CommandBuilder`——后者内部方法为 `pub(crate)`，外部无法调用 `cmdline()`/`environment_block()`。

> `ShellInfo.program` **必须是完整路径**（非短名）。`resolve_shell_info()` 通过 `which_full_path()` 在 PATH 中解析可执行文件完整路径；用户指定 shell 时若为短名同样走 PATH 解析。原因：`spawn_conpty_child` 同时向 `CreateProcessW` 提供 `lpApplicationName` 和 `lpCommandLine`，短名 `lpApplicationName` 无法被正确搜索。

**`pty_spawn` 流程变化**：
```
旧：resolve_shell() → cmd.env() → native_pty_system().openpty() → CPR → slave.spawn_command(cmd)
新：resolve_shell_info() → extra_envs → create_conpty_pair(build) → CPR → spawn_conpty_child(hpc, &shell_info, &extra_envs, cwd)
```

**依赖**：`filedescriptor = "0.8"`（Pipe 创建）、`mopa = "0.2"`（Downcast trait blanket impl）。均仅 Windows 条件编译。

### reader 缓冲区大小

`reader.rs` 的 `READER_BUF_SIZE` 常量设为 16384（16KB）。189KB/s 输出场景下约 12 次/秒 read() 调用（4KB 为 47 次/秒），减少约 75% 系统调用。

## 命令

```bash
# 运行后端测试（PTY 相关）
cargo test --manifest-path src-tauri/Cargo.toml pty -- --test-threads=1

# 运行单个测试
cargo test --manifest-path src-tauri/Cargo.toml <test_name> -- --test-threads=1
```

## 修改注意事项

1. 新增 Tauri 命令后必须在 `lib.rs` 的 `generate_handler!` 注册，并在 `capabilities/` 显式放行
2. Rust `snake_case` ↔ JS `camelCase`，改 DTO 必须双边对应修改
3. 新增 `#[cfg(windows)]` 块需确认是否应放在本模块（架构约束 #9）
4. 修改 `shell-integration.ps1` 后要跑 `test_shell_integration_script_embedded` 测试确认嵌入内容正确
5. `reader.rs` 的 `strip_conpty_startup` 修改后务必跑全部 strip 相关测试，确认不误杀正常输出
6. 修改 `READER_BUF_SIZE` 后跑 `reader_buf_size_is_16k` + `strip_startup_with_16k_boundary` + `strip_startup_with_large_payload` 测试
7. 修改 `conpty_custom` 模块（flags 计算、AttrList、ConPtyMaster、RawChild、spawn 逻辑）后跑 `conpty_custom::tests` 全部 14 条测试 + `pty_integration_tests` 的 `pty_spawn_custom_conpty` 端到端测试
8. 修改 `resolve_shell_info()` / `which_full_path()` 后跑 `shell::tests` 全部 9 条测试
9. `resolve_shell_info()` 返回的 `ShellInfo.program` **必须是完整路径**（非短名）——否则 `CreateProcessW(lpApplicationName=..., lpCommandLine=...)` 找不到可执行文件
