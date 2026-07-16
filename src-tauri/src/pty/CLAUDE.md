# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 模块职责

PTY 管理——Windows ConPTY 终端模拟核心。负责 shell 进程的完整生命周期：发现→创建→读写→缩放→销毁→重连。

## 架构

```
shell.rs          → resolve_shell() 返回 CommandBuilder（传统路径）+ resolve_shell_info() 返回 ShellInfo（自定义 ConPTY 路径）
mod.rs/spawn.rs   → pty_spawn() 握 SPAWN_LOCK → conpty_custom::create_conpty_pair()（绕过 portable-pty，直接调 Win32）→ reader 线程（锁外）
reader.rs         → reader_loop() 独立线程阻塞读 → Channel 推 PtyEvent（READER_BUF_SIZE=16KB）
spawn.rs          → pty_spawn / pty_write / pty_resize / pty_kill / pty_reattach（Tauri 命令）
src-tauri/src/state.rs（顶层模块，非 pty 子模块）→ PtySession 结构体 + PtyState 全局 HashMap
```

**PtyEvent 枚举**（`spawn.rs`，带 tag 的 serde 枚举，`camelCase` 序列化）：

- `Output { bytes: Vec<u8> }` — 原始终端输出字节
- `Exit { code: Option<i32> }` — 子进程退出

**PtySession**（`state.rs`）：`master`（Mutex<dyn MasterPty>）、`child`（Arc<Mutex<dyn Child>>）、`writer`（Arc<Mutex<dyn Write>>，take_writer 仅一次故共享）、`reader_handle`、`channel`（Arc<RwLock<Option<Channel>>>，可替换用于 reattach）、`output_ring`（256KB FIFO，Channel 断开时缓存）、`da1_injected`（Arc<AtomicBool>）、`job_object`（JobHandle，Windows Job Object / 非 Windows 零大小占位）、`exit_code`（Arc<Mutex<Option<i32>>>）。

### PtySession Drop

PtySession 实现 Drop，drop 时 join reader_handle 线程防止隐式 detach。

### JobHandle 跨平台设计

JobHandle 在 `#[cfg(windows)]` 下为 HANDLE RAII 包装；`#[cfg(not(windows))]` 为零大小占位类型（`new_dummy()`），state.rs 无需条件编译。

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

**动机**：`portable-pty` 0.9.0 内部硬编码 flags=0x7（INHERIT_CURSOR | RESIZE_QUIRK | WIN32_INPUT_MODE），不暴露 `CreatePseudoConsole` 的 `dwFlags` 参数。最初为按 OS build 动态启用 `PASSTHROUGH_MODE`（0x8，Win11 22H2+ 输出吞吐优化）而绕过；后 PASSTHROUGH_MODE 已移除（见下），自定义路径保留用于 flags 完全控制。

> **PASSTHROUGH_MODE (0x8) 已移除，勿重新启用**：passthrough 下 conhost 对子进程输出直通不解析，无法跟踪子进程的 DECSET 1000/1002/1006 mouse mode 请求，导致 terminal→child 方向的 SGR mouse report 不被转发（microsoft/terminal#376；PR #9970 的转发机制依赖 conhost 解析子进程输出）——claude 等全屏 TUI（v2.1.89+ 默认 alt buffer + mouse tracking）的鼠标滚轮完全失效。2026-07 在 Win11 build 26200 实测确认：诊断埋点显示 xterm 的 SGR wheel report（`\x1b[<64/65;x;yM`）完整写入 ConPTY stdin 但 claude 无反应；去掉 0x8 后滚轮恢复，claude 输出流畅度无肉眼可见退化。`compute_conpty_flags` 的 4 条测试锁死「任何 build 都返回 0x7」。

**自定义组件**：
| 类型 | 职责 |
|------|------|
| `compute_conpty_flags(build)` | 计算 flags：固定 0x7（PASSTHROUGH_MODE 已移除——吞 mouse input，见上）；保留 build 参数供未来按 build 恢复 |
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

### reader_loop 决策逻辑纯函数化

`reader_loop()` 约 90 行含 7 个关键分支（EOF/正常读取/错误/DA1 注入/ring buffer 写入/Channel 断连/Windows 首轮剥离），已将可测试逻辑抽取为纯函数：

- **`apply_startup_strip(startup_drained, data) -> Option<Vec<u8>>`** — 首轮读取时剥离 ConPTY 启动序列（OSC 标题/BEL/清屏/光标归位/DSR），返回 None 表示全部剥离（跳过），Some 返回剥离后数据。`startup_drained=true` 时原样返回。6 条测试覆盖：已排空透传、全剥离跳空、部分剥离保留正文、无启动序列原样返回
- **`should_inject_da1(already_injected, data) -> bool`** — 检测输出中的 DA1 查询（`ESC[c`/`ESC[0c]`），纯布尔参数替代 AtomicBool。4 条测试覆盖：已注入跳过、含 DA1 需注入、不含 DA1 不注入、DA1 嵌入数据中

剩余 I/O 编排（channel send vs ring buffer 回退、EOF `child.wait()`、`tracing::warn!`）因依赖 Mutex/RwLock/系统调用无法纯函数化——已逐分支在测试注释中标明依赖类型，标记为"已尽力"。

### DA1 查询模拟响应

Claude Code 的 Ink 渲染器启动时发送 DA1 查询（`ESC[c`）作为同步哨兵。ConPTY 拦截 DA1 查询后内部处理，不向子进程 stdout 返回响应。Ink 的 `waitFor` Promise 永不 resolve，阻塞约 60s。

`reader_loop` 在 startup_drained 后扫描输出中的 DA1 查询（`ESC[c` / `ESC[0c`），通过 `mirror_da1_query()` 检测。检测到后向子进程 stdin 注入 `ESC[?64;22c`（VT420 + ANSI 颜色），模拟 ConPTY + conhost 的一致行为。`std::sync::atomic::AtomicBool` 防重复注入（每会话一次）。

涉及的变更：
- `reader.rs`：`mirror_da1_query()` 扫描函数 + reader_loop 中注入逻辑；`reader_loop` 签名新增 `writer: Arc<Mutex<Box<dyn Write + Send>>>` + `da1_injected: Arc<AtomicBool>` 参数
- `state.rs`：`PtySession` 新增 `da1_injected: Arc<AtomicBool>` 字段
- `spawn.rs`：spawn 时创建 `da1_injected` 并传递 writer + flag 到 reader 线程

### ESC[s/ESC[u 误剥离修复

`strip_conpty_startup()` 的 `match_csi_startup` 中原有 `b's' | b'u' => Some(3)` 分支，将标准 VT100 光标保存/恢复（`ESC[s`/`ESC[u`）误标记为 ConPTY 启动序列。该分支实际为死代码——`ESC[s`/`ESC[u` 仅 2 字节（不含 `[`），永远不进入 CSI 序列匹配器。已删除。

## 文件

| 文件 | 职责 |
|------|------|
| `mod.rs` | PTY 模块入口：模块声明 + re-export |
| `spawn.rs` | Tauri 命令（`pty_spawn`/`pty_write`/`pty_resize`/`pty_kill`/`pty_reattach`）+ ConPTY 自定义实现 + PtyEvent 枚举定义 |
| `reader.rs` | 独立 reader 线程：`reader_loop()` 阻塞读取 PTY 输出 → Channel 推送 PtyEvent；`strip_conpty_startup()` 启动序列剥离；`apply_startup_strip()` 纯函数；`mirror_da1_query()` DA1 查询检测 |
| `shell.rs` | Shell 发现与选择：`resolve_shell()` / `resolve_shell_info()` → pwsh → powershell → cmd 回退；`which_full_path()` PATH 解析 |
| `state.rs` | 位于 `src-tauri/src/state.rs`（顶层模块）：`PtySession` 结构体 + `PtyState` 全局 HashMap + `validate_path_within_root` 路径沙箱 |
| `win_build.rs` | Windows build 号获取：通过 `nt_version` crate 的 RtlGetNtVersionNumbers 获取真实 build 号（低 28 位），非 Windows 平台返回 Unknown 错误 |

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
3. 新增 `#[cfg(windows)]` 块需确认是否应放在本模块（架构约束 #9）。`win_build.rs` 更名已避开 cargo build script 歧义
4. 修改 `shell-integration.ps1` 后要跑 `test_shell_integration_script_embedded` 测试确认嵌入内容正确
5. `reader.rs` 的 `strip_conpty_startup` 修改后务必跑全部 strip 相关测试，确认不误杀正常输出
6. 修改 `READER_BUF_SIZE` 后跑 `reader_buf_size_is_16k` + `strip_startup_with_16k_boundary` + `strip_startup_with_large_payload` 测试
7. 修改 `conpty_custom` 模块（flags 计算、AttrList、ConPtyMaster、RawChild、spawn 逻辑）后跑 `conpty_custom::tests` 全部 13 条测试 + `pty_integration_tests` 的 `pty_spawn_custom_conpty` 端到端测试。**改 `compute_conpty_flags` 前必读其注释**——启用 PASSTHROUGH_MODE 会静默破坏 claude 全屏 TUI 滚轮（mouse input 不转发），改后须实测 claude 滚轮不回归
8. 修改 `resolve_shell_info()` / `which_full_path()` 后跑 `shell::tests` 全部 7 条测试
9. `resolve_shell_info()` 返回的 `ShellInfo.program` **必须是完整路径**（非短名）——否则 `CreateProcessW(lpApplicationName=..., lpCommandLine=...)` 找不到可执行文件
10. ConPTY 指针 cast：`as_raw_handle()` 已返回 `*mut c_void`，无需再 `as *mut std::ffi::c_void`——Clippy `unnecessary-cast` 会报错。同理 `std::io::Error::new(std::io::ErrorKind::Other, e)` 简化为 `std::io::Error::other(e)`（Rust 1.74+），`.map_err(|e| std::io::Error::other(e))` 进一步简化为 `.map_err(std::io::Error::other)`

## 测试模式

Rust 测试分布在 5 个位置：

| 位置 | 类型 | 用例数 | 访问级别 |
|------|------|--------|---------|
| `pty/reader.rs` `#[cfg(test)]` | 单元测试 | 28 | `use super::*` 访问 `pub(crate)` 和私有项 |
| `pty/spawn.rs` `#[cfg(test)]` | 单元测试 | 13 | 在 `conpty_custom` 子模块内 |
| `pty/shell.rs` `#[cfg(test)]` | 单元测试 | 7 | `use super::*` |
| `tests/pty_integration_tests.rs` | 集成测试 | 5 | 仅能访问 `pub` API |
| `state.rs` `#[cfg(test)]` | 单元测试 | 15 | sandbox 路径校验纯函数测试 |

### 单元测试组织

所有单元测试遵循标准 Rust 模式——`#[cfg(test)] mod tests` 嵌入源文件底部，`use super::*` 导入父模块全部项。无独立 test 子目录或共享 helper 模块。

### ConPTY 隔离测试（spawn.rs）

`conpty_custom` 模块的 `create_conpty_pair()` 可独立调用，不依赖完整 spawn 流程：

```rust
// 直接创建 PTY 对，不 spawn 子进程
let (hpc, master) = create_conpty_pair(80, 24, 22621).unwrap();
// 验证初始尺寸
assert_eq!(master.get_size().unwrap(), PtySize { rows: 24, cols: 80, .. });
// 验证 writer 仅可 take 一次
assert!(master.take_writer().is_ok());
assert!(master.take_writer().is_err()); // 第二次失败
```

测试覆盖：`compute_conpty_flags`（4 条——全部 build 均返回 0x7，锁死 PASSTHROUGH_MODE 不启用）、flag 常量值验证（3 条）、`ConPtyMaster` MasterPty trait（4 条）、`AttrList` 生命周期（2 条）。

### 启动序列剥离测试（reader.rs）

`strip_conpty_startup()` 是纯函数——输入字节数组，输出剥离后的字节数组：

```rust
// 验证剥离 ConPTY 启动序列
assert_eq!(
    strip_conpty_startup(b"\x1b]0;pwsh\x07\x1b[2J\x1b[HPS C:\\> "),
    b"PS C:\\> "
);
```

测试覆盖：剥离 OSC 标题/BEL、清屏（CSI 2J/3J）、光标归位（CSI H）、DSR（CSI 6n）、光标显隐（CSI ?25h/l）；保留 OSC 7 cwd、普通文本、ESC[s/ESC[u；16KB 边界 + >4KB 大数据块完整保留。

### DA1 查询模拟测试（reader.rs）

`mirror_da1_query()` 纯函数测试：
- 标准 DA1 `ESC[c]` 检测 ✓
- 前导零 `ESC[0c]` 检测 ✓
- DA2 `ESC[>c]` 不触发
- 普通文本含 `[c` 不误触发
- XTVERSION `ESC[>0q]` 不触发

### 集成测试（tests/pty_integration_tests.rs）

真实 spawn `cmd.exe`，端到端验证 PTY 通信：

```rust
static SPAWN_LOCK: Mutex<()> = Mutex::new(()); // 串行化

fn spawn_cmd() -> (Box<dyn MasterPty>, Box<dyn Child>, Box<dyn Read>, Box<dyn Write>) {
    let _lock = SPAWN_LOCK.lock().unwrap();
    // 创建 PTY → spawn cmd.exe → 返回四种 handle
}
```

测试覆盖：echo roundtrip（写入 `echo marker` → 读取验证 marker 出现）、resize 应用（spawn → resize 30×100 → `get_size()` 验证）、kill 无孤儿（spawn → kill → `try_wait()` 验证子进程退出）、自定义 ConPTY spawn（仅 Windows CI runner）、shell 集成脚本 OSC 序列验证。

### 运行约束

- **所有 PTY 测试必须 `--test-threads=1`**：`SPAWN_LOCK` + ConPTY 资源限制，并发 spawn 会死锁输出管道
- 集成测试在 Windows CI runner 上运行（依赖系统 ConPTY API）
- `shell.rs` 测试依赖 `include_str!("../../assets/shell-integration.ps1")` 编译期嵌入
