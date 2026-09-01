# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 存在理由

`src-tauri/src/pty` 封装 Windows ConPTY 的完整生命周期，使上层只需通过 panelId 与 sessionId 操作终端。这里集中处理 spawn 串行化、自定义 ConPTY flags、Win10 conhost 捆绑、启动序列剥离、DA1 模拟、Job Object 孤儿防护等机制——这些外部行为无法通过代码本身推断原因与红线，必须文档化。

## 关键约束与决策

### `#[cfg(windows)]` 只准在此处

业务条件编译全部收敛到本模块（spawn.rs / shell.rs / conpty_api.rs / win_build.rs）。非 Windows 平台用 `cfg!(windows)` 运行时分支，业务逻辑不撒 `#[cfg]`。

### SPAWN_LOCK 串行化 spawn

并发 spawn 会卡死 ConPTY 输出管道。`SPAWN_LOCK` 只保护 `create_conpty_pair` + `spawn_conpty_child`，reader 线程启动与 sessions 插入在锁外执行。

### 自定义 ConPTY 路径

绕过 `portable-pty` 以控制 `CreatePseudoConsole` 的 `dwFlags`。动机：portable-pty 0.9.0 硬编码 flags=0x7，不暴露参数；保留自定义路径用于 flags 完全控制与 Win10 conhost 捆绑。

### flags 三态

`compute_conpty_flags(build, bundled)`：
- 捆绑新 conhost（仅 Win10）→ `0x7`
- 系统 conhost + Win11（≥`CONPTY_WIN11_MIN_BUILD`）→ `0x7`
- 系统 conhost + Win10 回退 → `0x3`

阈值 21376 与前端 xterm 钳制（ADR-0004）同源。

### Win10 捆绑 conhost（ADR-0005）

老 Win10（build < 21376）in-box conhost 不转发鼠标 VT 序列。`vendor/conpty/` 的 conpty.dll + OpenConsole.exe 经 `include_bytes!` 嵌入，仅 Win10 在首次 spawn 前提取到 `%LOCALAPPDATA%\slterminal\conpty\` 并 `LoadLibraryW` 加载。加载/提取失败静默回退系统 ConPTY；Win11 零变化。vendor 更新后必须 Win10 实机验证。

### PASSTHROUGH_MODE (0x8) 永久禁用

0x8 会让 claude 等全屏 TUI 的鼠标滚轮完全失效。该问题无法被最小实验或自动化测试守卫（假阴性），改 flags 必须实测真实 claude 滚轮。

### cwd 反斜杠规范化

传给 ConPTY/`CreateProcessW` 前将 cwd 中的 `/` 替换为 `\`。

### stdin writer 必须随 PtySession 存活

绝对不能 drop stdin writer。writer 通过 `Arc<Mutex>` 共享，生命周期跟随 `PtySession`。

### Job Object 孤儿防护

每个子进程放入 Job Object，设置 `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`。父进程崩溃/退出时 OS 自动杀子进程。

### pty_kill 异步销毁

`ClosePseudoConsole` 在 pre-Win11 24H2 上可能永久阻塞。`pty_kill` 先提取 session 释放写锁，再在 `spawn_blocking` 中执行 `kill → join reader → drop master`。`KILL_JOIN_TIMEOUT = 3s` 轮询 `is_finished`，超时放弃 join 并记 warn。

### Channel 可替换 + ring buffer 回放（E1）

`reader_loop` 通过 `Arc<RwLock<Option<Channel>>>` 引用 Channel。Channel 断开时写入 256KB ring buffer；重连时替换 Channel 并回放。该机制保留于内部，对外重连命令已随 SEC-03 删除。

### 终端能力环境变量

spawn 阶段统一注入：
- `COLORTERM=truecolor`
- `TERM=xterm-256color`
- `TERM_PROGRAM=slTerminal`
- `SLTERM_PANEL_ID=<panelId>`

`SLTERM_PANEL_ID` 供 hooks 信号文件标记事件来源，变量语义见 @../hooks/CLAUDE.md。

### Shell 白名单（SEC-01 / SEC-15）

仅允许 `pwsh.exe` / `powershell.exe` / `cmd.exe`。用户传入含路径分隔符的 shell 时：
- `which_full_path` 解析真实路径，与用户路径比对，一致才放行；
- PATH 不可解析时 `%SystemRoot%\System32` 兜底；
- 双侧 `canonicalize` 均失败时回退归一字符串比对（alias/Store 版 pwsh 兼容），单侧失败即拒绝。

PowerShell 通过 `-EncodedCommand` 内联 `shell-integration.ps1`，避免 `%APPDATA%` 文件写入触发 AMSI/ASR。启动参数固定 `-NoLogo -NoExit -EncodedCommand`，**禁止 `-NoProfile`**——用户 profile 必须先于集成脚本原生加载（B17，守卫用例 `pwsh_args_no_noprofile_b17`）。

### 启动序列剥离

`strip_conpty_startup()` 在首轮读取时剥离 ConPTY `VtIo::StartIfNeeded()` 注入的序列：OSC 窗口标题（含 BEL）、清屏、光标归位、光标显隐、DSR。后续读取原样透传。

### DA1 查询模拟响应

ConPTY 拦截 DA1 查询（`ESC[c`/`ESC[0c]`）后不返回响应，导致 Claude Code Ink 渲染器阻塞约 60s。`reader_loop` 在 startup_drained 后扫描输出，检测到后向 stdin 注入 `ESC[?64;22c`，同一会话仅注入一次。

### CPR 注入

spawn 后立即向 stdin 写 `\x1b[1;1R`，补偿 ConPTY `VtIo::StartIfNeeded()` 的 DSR 握手，避免首次读取时 DSR 死锁。

### 会话上限 MAX_PTY_SESSIONS=32（BE-01）

`pty_spawn` 在 `SPAWN_LOCK` 区间内检查容量；sessions 写锁内还有一次原子复查兜底。命中上限时显式 kill 已 spawn 子进程。

## 外部坑/红线

- **改 flags 必须实测真实 claude 滚轮**：自动化无法守卫 PASSTHROUGH 0x8 回归。
- **Win10 捆绑 conhost 改动必须实机验证**：鼠标转发、键盘/IME/kitty、resize 无法靠 CI 守卫。
- **永不启用 0x8**。
- **PowerShell 交互 shell 禁止 `-NoProfile`**：用户 profile（conda init 钩子等）必须原生加载——缺钩子则 `conda activate` 失效（win11 CondaError / win10 conda.bat 静默空转，B17）。
- **不要把 `#[cfg(windows)]` 放到本模块外**。
- **不要 drop stdin writer**。
- **不要 stop/start 轮换 watcher**（见 @../notify/CLAUDE.md），与 pty 无关但常被误用。
- **不要解析提示符跟踪 cwd**：portable-pty 在 Windows 不返回 cwd，只能靠 OSC 7/133 序列。

## 测试模式

- **所有 PTY 测试必须 `--test-threads=1`**：ConPTY 并发 spawn 死锁。
- **L1 测试分布**：`reader.rs`、`spawn.rs`、`shell.rs`、`conpty_api.rs` 各自的 `#[cfg(test)]`，以及 `tests/pty_integration_tests.rs`。
- **集成测试**：真实 spawn `cmd.exe`，仅 Windows CI runner 上运行。

### 既定豁免（已在 `.claude/test-exemptions.md` 登记）

| 豁免项 | 原因 | 当前兜底 |
|--------|------|---------|
| `reader_loop` 残余 I/O 编排 | 依赖 `RwLock<Option<Channel>>`/Mutex/管道系统调用，无法在 L1 构造输入 | 可纯函数化部分（`apply_startup_strip`/`should_inject_da1`/`eof_exit_code`/`micro_batch_tail`）已由 L1 覆盖 |
| 容量超限 kill 清理 | 命中上限后 kill 已 spawn 子进程依赖真实 PtySession | BE-01 判定语义由纯函数用例锁死 + Job Object 兜底 |
| `conpty_api` vendor 提取/加载回退 | 依赖真实 DLL 加载行为 | ADR-0005 Win10 实机人工验证 + `ensure_extracted` 幂等用例 |
| Mutex 中毒分支 | 临界区无 panic，中毒不可达 | 未来引入锁内 panic 代码时须补测试或换原语 |
