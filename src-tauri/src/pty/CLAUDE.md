# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 模块职责

PTY 管理——Windows ConPTY 终端模拟核心。负责 shell 进程的完整生命周期：发现→创建→读写→缩放→销毁→重连。

## 架构

```
shell.rs          → resolve_shell() 返回 CommandBuilder
mod.rs/spawn.rs   → pty_spawn() 握 SPAWN_LOCK（仅 openpty→spawn_command）→ reader 线程（锁外）
reader.rs         → reader_loop() 独立线程阻塞读 → Channel 推 PtyEvent
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

`shell.rs`：`pwsh.exe` → `powershell.exe` → `cmd.exe` 回退。PowerShell 通过 `-EncodedCommand`（UTF-16LE Base64）内联集成脚本（`include_str!("../../assets/shell-integration.ps1")`），消除 `%APPDATA%` 文件写入，避免 AMSI/ASR 误杀。集成脚本注入 OSC 7（cwd 跟踪）+ OSC 133 A/B/D（提示符边界+退出码）+ UTF-8 编码修复。

## 命令

```bash
# 运行后端测试（PTY 相关）
cargo test --manifest-path src-tauri/Cargo.toml pty -- --test-threads=1

# 运行单个测试
cargo test --manifest-path src-tauri/Cargo.toml <test_name> -- --test-threads=1

# 静态检查
cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings

# 开发运行（启动完整应用）
npm run tauri dev
```

## 修改注意事项

1. 新增 Tauri 命令后必须在 `lib.rs` 的 `generate_handler!` 注册，并在 `capabilities/` 显式放行
2. Rust `snake_case` ↔ JS `camelCase`，改 DTO 必须双边对应修改
3. 新增 `#[cfg(windows)]` 块需确认是否应放在本模块（架构约束 #9）
4. 修改 `shell-integration.ps1` 后要跑 `test_shell_integration_script_embedded` 测试确认嵌入内容正确
5. `reader.rs` 的 `strip_conpty_startup` 修改后务必跑全部 strip 相关测试，确认不误杀正常输出
