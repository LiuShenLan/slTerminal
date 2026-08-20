# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 模块职责

PTY 管理——Windows ConPTY 终端模拟核心。负责 shell 进程的完整生命周期：发现→创建→读写→缩放→销毁→重连。

## 架构

```
shell.rs          → resolve_shell() 返回 CommandBuilder（传统路径）+ resolve_shell_info() 返回 ShellInfo（自定义 ConPTY 路径）
mod.rs/spawn.rs   → pty_spawn() 握 SPAWN_LOCK → conpty_custom::create_conpty_pair()（绕过 portable-pty，直接调 Win32）→ reader 线程（锁外）
reader.rs         → reader_loop() 独立线程阻塞读 → 微批聚合（BE-05，64KB/无可读即送）→ Channel 推 PtyEvent（READER_BUF_SIZE=16KB）
spawn.rs          → pty_spawn / pty_write / pty_resize / pty_kill / pty_kill_all（Tauri 命令）
src-tauri/src/state.rs（顶层模块，非 pty 子模块）→ PtySession 结构体 + PtyState 全局 HashMap
```

**PtyEvent 枚举**（`spawn.rs`，带 tag 的 serde 枚举，`camelCase` 序列化）：

- `Output { bytes: Vec<u8> }` — 原始终端输出字节
- `Exit { code: Option<i32> }` — 子进程退出

**PtySession**（`state.rs`）：`master`（Mutex<dyn MasterPty>）、`child`（Arc<Mutex<dyn Child>>）、`writer`（Arc<Mutex<dyn Write>>，take_writer 仅一次故共享）、`reader_handle`、`channel`（Arc<RwLock<Option<Channel>>>，可替换用于 reattach）、`output_ring`（256KB FIFO，Channel 断开时缓存）、`da1_injected`（Arc<AtomicBool>）、`job_object`（JobHandle，Windows Job Object / 非 Windows 零大小占位）、`exit_code`（Arc<Mutex<Option<i32>>>）。

### PtySession Drop

PtySession 实现 Drop，drop 时 join reader_handle 线程防止隐式 detach。

### JobHandle 跨平台设计

JobHandle 在 `#[cfg(windows)]` 下为 HANDLE RAII 包装；`#[cfg(not(windows))]` 为零大小占位类型（`new_dummy()`）。非 Windows 平台 `job_object` 始终初始化为 `Some(JobHandle::new_dummy())`，无 `#[cfg(windows)]` 条件初始化。state.rs 中 `job_object` 字段为 `Option<JobHandle>`，无需条件编译。

## 关键约束

### 架构硬约束 #9：`#[cfg(windows)]` 只允许在此处

所有 Windows 条件编译收敛于此模块。非 Windows 平台代码走 `cfg!(windows)` 运行时分支（如 `strip_conpty_startup` 原样返回）。业务逻辑不撒 cfg。

### SPAWN_LOCK 串行化

**并发 spawn 会卡死 ConPTY 输出管道**。`pty_spawn` 中 SPAWN_LOCK 仅保护 ConPTY 创建 + 子进程启动（`openpty` → `spawn_command`），reader 线程启动和 sessions 插入在锁外执行。

### 会话上限 MAX_PTY_SESSIONS=32（BE-01）

`const MAX_PTY_SESSIONS: usize = 32` 硬上限——防并发 spawn 耗尽 ConPTY/进程句柄。`pty_spawn` 在 **SPAWN_LOCK 持锁区间内**检查 `sessions.len() >= 32`（判定与 spawn 原子化，杜绝并发超发），命中上限返回 `AppError::Validation` 并显式 kill 已 spawn 的子进程（kill 后 ConPTY 输出端关闭 → reader 退出 → PtySession drop 时 join 正常返回；Job Object KILL_ON_JOB_CLOSE 兜底）。改上限后跑 spawn.rs `pty_capacity_*` 三条测试。

### CPR 注入（Windows）

`openpty()` 后立即向 stdin 写 `\x1b[1;1R`。补偿 ConPTY `VtIo::StartIfNeeded()` 的 DSR 握手，避免首次读取时 DSR 死锁。蜂鸣和首字符消失由 `reader.rs` 的 `strip_conpty_startup()` 处理——与此机制无关。

### cwd 反斜杠规范化

`CreateProcessW` 对 `/` 行为异常。传给 ConPTY 前将 cwd 中的 `/` 替换为 `\\`。

### stdin drop = 立即杀子进程（Windows）

**绝对不能 drop stdin writer**。writer 通过 `Arc<Mutex>` 共享，生命周期跟随 `PtySession`。

### Job Object 孤儿防护（Windows）

每个子进程放入 Job Object，设置 `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`。父进程崩溃/退出时 OS 自动杀所有子进程。

### pty_kill：async + spawn_blocking + kill 加固（BE-06）

`ClosePseudoConsole` 在 pre-Win11 24H2 上永久阻塞。`pty_kill` 先提取 session 释放 `RwLock` 写锁（<1ms），再在 `spawn_blocking` 中执行 `kill → join reader → drop master`，避免持锁阻塞导致后续命令级联卡死。

**kill 加固（BE-06，S06）**：
- `child.kill()` 返回值被检查——失败 `tracing::warn!` 后继续（不再 `let _ =` 丢弃）
- reader 线程 join 改「带超时轮询 `is_finished`」：`KILL_JOIN_TIMEOUT = 3s`（`join_with_timeout` 纯逻辑），超时放弃 join 记 warn——线程随 `PtySession` Drop 兜底，避免永久阻塞

### pty_kill_all：关闭兜底（BE-08）

`pty_kill_all() -> Result<u32, AppError>`（返回 kill 数）——遍历 sessions 全部 kill+join（超时语义同 BE-06）。前端关闭序列 = 先前端 `TerminalRegistry` 快速 kill，再 `pty_kill_all` 兜底——前后端不一致时后端 session 不泄漏。命令注册 + capabilities `allow-pty_kill_all`（S13 后命令数 34）。L1 测试：空返回 0 / 多 session 全灭。

### Channel 可替换 + ring buffer 回放（E1）

`reader_loop` 通过 `Arc<RwLock<Option<Channel>>>` 引用 Channel。Channel 断开时写入 `output_ring`（256KB FIFO，按 1KB 粒度整行丢弃）。重连时替换 Channel 并回放 ring buffer 内容，用于前端页面切换后恢复终端显示（E1 机制保留——对外重连命令已随 SEC-03 删除，替换逻辑仅存于 reader/state 内部）。

### ConPTY 启动序列剥离

`strip_conpty_startup()` 在首轮读取时剥离 ConPTY `VtIo::StartIfNeeded()` 注入的序列：OSC 窗口标题（含 BEL→蜂鸣）、清屏 `ESC[2J/3J`、光标归位 `ESC[H`、光标显隐 `ESC[?25h/l`、DSR `ESC[6n`。后续读取原样透传。非 Windows 平台原样返回。

### Shell 选择与 profile 注入

`shell.rs`：`pwsh.exe` → `powershell.exe` → `cmd.exe` 回退。提供两套接口——`resolve_shell()` 返回 `CommandBuilder`（旧路径/非 Windows fallback），`resolve_shell_info()` 返回 `ShellInfo`（自定义 ConPTY 路径，program 为完整路径）。Shell 可执行文件通过 `which_full_path()` 在 PATH 中解析为完整路径，确保 `CreateProcessW(lpApplicationName=...)` 正确工作。

**白名单真实路径校验（SEC-01，S02）**：用户传入 shell **含路径分隔符**时——`canonicalize` 用户路径，与 `which_full_path(文件名)` 解析结果比对，**一致才放行**（只信任 PATH 解析出的真实路径，防传 `C:\project\cmd.exe` 或篡改 PATH 绕过 → RCE）；PATH 不可解析时 `%SystemRoot%\System32` 系统目录兜底（cmd 回退自洽）。纯文件名输入维持现状。忽略大小写比较（Windows 文件系统大小写不敏感）。**alias 兼容（人工验证修复）**：canonicalize 失败（应用执行别名/特殊 ACL——Store 版 pwsh 的 `%LOCALAPPDATA%\Microsoft\WindowsApps\pwsh.exe`，CreateProcess 可运行但普通文件 API 打开失败 os error 1920）时**不拒绝**，回退归一字符串比较（`paths_match` 纯函数：`/`→`\`、去尾分隔符、忽略大小写）——alias 场景两侧字符串相同放行，伪造路径字符串必不同仍拒绝，威胁模型不变。L1 测试：伪造路径拒绝 / 合法绝对路径放行 / PATH 解析一致放行 / **paths_match 纯函数 5 条 + 统一错误文案 1 条 + 真实 alias 条件测试 1 条（shell.rs 共 31 条）**。PowerShell 通过 `-EncodedCommand`（UTF-16LE Base64）内联集成脚本（`include_str!("../../assets/shell-integration.ps1")`），消除 `%APPDATA%` 文件写入，避免 AMSI/ASR 误杀。集成脚本注入 OSC 7（cwd 跟踪）+ OSC 133 A/B/D（提示符边界+退出码）+ UTF-8 编码修复。

### 终端能力环境变量注入

`pty_spawn` 在 spawn 阶段注入四个环境变量（直接构造 `Vec<(String, String)>`，通过 `spawn_conpty_child` 的 `build_env_block` 合并到子进程环境块）：
- `COLORTERM=truecolor` — Chalk/supports-color 的核心检测信号，Ink 系 TUI 依赖此变量启用 24-bit RGB（终端平台能力，设计动机 Claude Code，对全部子进程生效）
- `TERM=xterm-256color` — 传统 terminfo 能力宣告（部分应用不看 COLORTERM，防御性设置）
- `TERM_PROGRAM=slTerminal` — 品牌标识（非功能性，行业惯例）
- `SLTERM_PANEL_ID=<panelId>` — **通用每终端路由键**（MC-110 文档记录项），值 = `request.panel_id`；子进程据此识别所属终端页签（供 hook 信号文件标记事件来源）。「无此变量 exit(0)」门控语义归各 CLI reporter 实现（见 @../hooks/CLAUDE.md），pty 层不假设消费方

注入在 spawn 阶段（非 shell rc），确保子进程一启动即可见。四个变量对 pwsh/powershell/cmd 统一注入，不加 shell 类型判断。

### 自定义 ConPTY 创建（绕过 portable-pty）

`spawn.rs` 中的 `conpty_custom` 模块（`#[cfg(windows)]`，约 300 行）绕过 `portable_pty::native_pty_system().openpty()`，直接调用 `windows` crate 的 `CreatePseudoConsole`/`CreateProcessW`，实现对 ConPTY flags 的完全控制。

**动机**：`portable-pty` 0.9.0 内部硬编码 flags=0x7（INHERIT_CURSOR | RESIZE_QUIRK | WIN32_INPUT_MODE），不暴露 `CreatePseudoConsole` 的 `dwFlags` 参数。最初为按 OS build 动态启用 `PASSTHROUGH_MODE`（0x8，Win11 22H2+ 输出吞吐优化）而绕过；后 PASSTHROUGH_MODE 已移除（见下），自定义路径保留用于 flags 完全控制。

> **PASSTHROUGH_MODE (0x8) 已移除，勿重新启用**：0x8 下 claude 等全屏 TUI（v2.1.89+ 默认 alt buffer + mouse tracking）的鼠标滚轮完全失效。2026-07 在 Win11 build 26200 真实 app 双向实测确认：诊断埋点显示 xterm 的 SGR wheel report（`\x1b[<64/65;x;yM`）完整写入 ConPTY stdin 但 claude 无反应；去掉 0x8 后滚轮恢复，claude 输出流畅度无肉眼可见退化。疑似机制为 passthrough 下 conhost 不解析子进程输出、不跟踪 DECSET mouse mode（microsoft/terminal#376；PR #9970）。**注意：无法用最小实验/自动化测试守卫此回归**——node 直接子进程的最小复现（DECSET 1002/1003/1006 + alt buffer + 60fps 帧刷写负载）在 0xF 下 stdin 的 SGR report 仍原样透传，阻断条件仅真实 claude 场景（pwsh→claude 进程树 + kitty 协议）复现，行为级集成测试会产生假阴性（0x8 下也绿）故未落地。**改 flags 必须实测真实 claude 滚轮**。

### Win10 ConPTY 宿主捆绑（ADR-0005，conpty_api.rs）

老 Win10（build < 21376）in-box conhost 的 ConPTY **不转发鼠标 VT 序列**（microsoft/terminal#376，修复 PR #4856 只在新版 conhost）——0x3/0x7 两条输入路径均实测 claude 全屏滚轮失效（0x3 分叉实验 2026-08-17 验证失败，推翻 flags 假设）。修复：`vendor/conpty/` 的 conpty.dll + OpenConsole.exe（官方 NuGet `Microsoft.Windows.Console.ConPTY` 1.24.260710001，MIT）经 `include_bytes!` 嵌入 slterminal_lib.dll，仅 Win10 在首次 pty_spawn 前提取到 `%LOCALAPPDATA%\slterminal\conpty\` 并 `LoadLibraryW` 动态加载（`OnceLock` 进程级单次解析）。

- **加载机制**：conpty.dll 定位 OpenConsole.exe 靠同目录查找（PR #12980），故两文件提取到同一目录；导出名带 Conpty 前缀（`ConptyCreatePseudoConsole`/`ConptyClosePseudoConsole`/`ConptyResizePseudoConsole`）；OpenConsole.exe 依赖全为系统 api-ms-win-*（自包含）。
- **回退**：提取/加载任一失败 → `tracing::warn!` + 静默回退系统 ConPTY（行为 = 现状）；Win11（≥21376）恒系统路径零变化。
- **提取幂等**：已存在且大小与嵌入一致 → 复用；不一致覆盖重写（vendor 升级自愈）。
- **vendor 更新**：见 `vendor/conpty/README.md`；更新后必须 Win10 实机验证。
- **实机验证红线**：自动化无法守卫真实鼠标转发（先例同 0x8/0x3）——改动必须实测真实 claude 滚轮 + 键盘/IME/kitty + resize。

### flags 三态（compute_conpty_flags）

`compute_conpty_flags(build, bundled)` 三态：捆绑新 conhost → 恒 0x7（新版完整支持 0x4）；系统 conhost + Win11（≥21376）→ 0x7；系统 conhost + Win10（<21376）→ 0x3（**回退路径**——0x3 未修复滚轮，仅因键盘/IME 已实测正常而保留，防回退场景无谓启用 0x4）。阈值 21376 与前端 xterm 钳制（ADR-0004）同源，常量单点于 `conpty_api::CONPTY_WIN11_MIN_BUILD`。

- **测试守卫**：`compute_conpty_flags` 7 条测试锁死三态表（19041/21375 系统→0x3；21376/22000/22621/26100 系统→0x7；19041 捆绑→0x7）且任何组合不含 PASSTHROUGH 0x8。

**自定义组件**：
| 类型 | 职责 |
|------|------|
| `compute_conpty_flags(build, bundled)` | 计算 flags 三态：捆绑/系统 Win11 → 0x7；系统 Win10 回退 → 0x3（见上） |
| `AttrList` | `PROC_THREAD_ATTRIBUTE_LIST` RAII wrapper（`Initialize`→`Update`→`Delete`） |
| `ConPtyMaster` | 实现 `portable_pty::MasterPty`（resize/get_size/try_clone_reader/take_writer），持有通过 `windows` crate 创建的 HPCON |
| `RawChild` | 实现 `portable_pty::Child + ChildKiller`，封装 `OwnedHandle` + `TerminateProcess` |
| `create_conpty_pair()` | 创建管道对 → `resolve_conpty_api(build)` → `api.create(flags)` → 返回 `(HPCON, ConPtyMaster)` |
| `spawn_conpty_child()` | `AttrList::set_pty(hpc)` → `CreateProcessW` → 返回 `RawChild` |

**shell 信息解耦**：`shell.rs` 新增 `ShellInfo` 结构体和 `resolve_shell_info()` 函数，返回纯数据结构（program + args），不依赖 `portable_pty::CommandBuilder`——后者内部方法为 `pub(crate)`，外部无法调用 `cmdline()`/`environment_block()`。

> `ShellInfo.program` **必须是完整路径**（非短名）。`resolve_shell_info()` 通过 `which_full_path()` 在 PATH 中解析可执行文件完整路径；用户指定 shell 时若为短名同样走 PATH 解析。原因：`spawn_conpty_child` 同时向 `CreateProcessW` 提供 `lpApplicationName` 和 `lpCommandLine`，短名 `lpApplicationName` 无法被正确搜索。

**`pty_spawn` 流程变化**：
```
旧：resolve_shell() → cmd.env() → native_pty_system().openpty() → CPR → slave.spawn_command(cmd)
新：resolve_shell_info() → extra_envs → create_conpty_pair(build) → CPR → spawn_conpty_child(hpc, &shell_info, &extra_envs, cwd)
```

**依赖**：`filedescriptor = "0.8"`（Pipe 创建）、`mopa = "0.2"`（Downcast trait blanket impl）。均仅 Windows 条件编译。

### reader 微批处理（BE-05/12，S06）

**「读到即续读」非定时器微批**——每次 read 成功（16KB 首块）后非阻塞续读（Windows 上基于 **`PeekNamedPipe` 查询管道可读字节数**（`SendRawHandle::pending_bytes`）检测管道可读），累积至 `MICRO_BATCH_MAX`（64KB）**或无可读数据**再一次 `Channel::send` + 一次 `ring_buffer_append`（BE-12：append 调用点仅批量一处，Mutex 竞争随频率自然降级；不引入无锁结构）。避免引入固定延迟——非定时器，无批处理延迟。首块经过 ConPTY 启动序列剥离，续读块在首块真实数据出现后原样透传（跨 16KB 边界残留由首块剥离状态机处理）。

**pending 检查禁用 WaitForSingleObject（信号竞态修复）**：旧实现 `WaitForSingleObject(handle, 0)` 对匿名管道读端存在**信号 reset 延迟竞态**——数据被 read 读走后信号未及时 reset → 误报「有数据」→ 微批续读的阻塞 read 空等 → **reader 线程卡死 → 该终端永久无输出**（E2E 全部终端文本为空 + win10 黑屏人工验证问题根因）。改用 `PeekNamedPipe`（同步查询可读字节数，与后续 read 同一管道状态视角，零竞态窗口）；对端关闭时 Peek 返回 0，由主循环 read Ok(0) EOF 兜底。回归测试：`pending_bytes_reflects_pipe_data_availability`（spawn.rs conpty_custom T9，真实管道锁死 空→写→读走→0 语义）。

`READER_BUF_SIZE` 常量 = 16384（16KB），首块最多 16KB、续读约 3 块满上限。效果：IPC 次数与字节数解耦（原每次 read 即 send，S06 前每次 16KB send 一次）。

**DOC-01 豁免项 1（reader_loop 残余 I/O 编排）随微批变动**：豁免表同步在 S19（本文件「reader_loop I/O 编排残余豁免」段已更新为微批后形态，明细见下）。

### reader_loop 决策逻辑纯函数化

`reader_loop()` 主循环三个 match 分支（EOF/正常读取/错误），已将可测试逻辑抽取为纯函数：

- **`apply_startup_strip(startup_drained, data) -> Option<Vec<u8>>`** — 首轮读取时剥离 ConPTY 启动序列（OSC 标题/BEL/清屏/光标归位/DSR），返回 None 表示全部剥离（跳过），Some 返回剥离后数据。`startup_drained=true` 时原样返回。6 条测试覆盖：已排空透传、全剥离跳空、部分剥离保留正文、无启动序列原样返回
- **`should_inject_da1(already_injected, data) -> bool`** — 检测输出中的 DA1 查询（`ESC[c`/`ESC[0c]`），纯布尔参数替代 AtomicBool。4 条测试覆盖：已注入跳过、含 DA1 需注入、不含 DA1 不注入、DA1 嵌入数据中
- **`eof_exit_code(wait_outcome: Result<Result<i32, ()>, ()>) -> Option<i32>`**（PTY-12）— EOF 退出码降级决策（P2-11/P2-42）：child 锁获取失败 / `child.wait()` 失败 → `None`（不硬编码 0），两级均 Ok → 真实退出码。3 条测试覆盖：成功返真实码（含 0）、wait 失败 None、锁失败 None
- **`micro_batch_tail()`**（BE-05）— 微批续读决策：pending 检查 + 续读累积（read 为系统调用，决策已抽，调用不可抽）。6 条测试覆盖：无 pending 不读 / 续读到无可读 / 至 64KB 上限停 / 读错误停 / 立即 EOF / 上限尊重首块头寸

剩余 I/O 编排残余分支因依赖 Mutex/RwLock/系统调用无法纯函数化——已逐分支在测试注释中标明依赖类型（M11 分析块，微批后形态），明细与豁免理由见下方「reader_loop I/O 编排残余豁免」段（已收编进 `.claude/test-inventory.md` 既定豁免清单，DOC-01）。

### reader_loop I/O 编排残余豁免（PTY-12，DOC-01 收编）

> **本段为豁免表登记项 1（`reader_loop` 残余 I/O 编排分支）的模块级明细**，与 `.claude/test-inventory.md`「既定豁免清单」登记一致（DOC-01 收编，Stage 02 草稿转正，**S06 微批后形态 S19 同步**）；豁免原因/当前兜底层级两列以 test-inventory 为唯一真值源。逐分支分析原文见 reader.rs 测试注释 M11 块。

`reader_loop`（reader.rs）经 PTY-12 评估：除 `apply_startup_strip` / `should_inject_da1` / `eof_exit_code` / `micro_batch_tail` 已纯函数化外，残余分支全部依赖同步原语或系统调用，判定不可抽：

| 残余分支 | 依赖 | 不可抽原因 |
|----------|------|-----------|
| channel 锁获取失败 → break（三分支） | `RwLock<Option<Channel>>` | break 语义依赖循环控制流；`tauri::ipc::Channel` 无法在 L1 构造（无 webview 运行时上下文） |
| send Output/Exit 失败 → debug 日志 | `Channel::send`（Tauri IPC） | send 依赖运行时 webview 上下文，无法注入 |
| EOF `child.wait()` | portable-pty `Child::wait()` | Windows `WaitForSingleObject` 系统调用（退出码降级决策已抽为 `eof_exit_code`，调用本身不可抽） |
| DA1 响应注入 | `writer.lock()` + 管道 I/O | Mutex + 管道系统调用（检测决策已抽为 `should_inject_da1`，注入动作不可抽） |
| 微批续读循环（`micro_batch_tail` 调用点） | 非阻塞 `WaitForSingleObject` + `read` 系统调用 | 续读决策已抽为 `micro_batch_tail` 纯函数，循环调用本身不可抽 |
| ring buffer 写入 | `ring_buffer_append`（state.rs） | 函数本体已抽取（state.rs 测试覆盖）；**BE-12 批量 append 后调用点仅微批一处**——不存在"channel 断开→写 ring"分流决策（P2-46 无条件缓存：先缓存再 send，成功路径零 clone），channel 为 None 时仅 Option 判空跳过 send，无分支逻辑可测 |
| `tracing::warn!`/`error!` 告警 | 日志宏 | I/O 副作用 |
| 读错误 → 退出码 -1 | 常量赋值 | 无分支逻辑（`Some(-1)` 字面量） |

### DA1 查询模拟响应

DA1 查询响应是终端平台能力（设计动机：Ink 系 TUI，对全部子进程生效）。Claude Code 的 Ink 渲染器启动时发送 DA1 查询（`ESC[c`）作为同步哨兵；ConPTY 拦截 DA1 查询后内部处理，不向子进程 stdout 返回响应，Ink 的 `waitFor` Promise 永不 resolve，阻塞约 60s。

`reader_loop` 在 startup_drained 后扫描输出中的 DA1 查询（`ESC[c` / `ESC[0c`），通过 `mirror_da1_query()` 检测。检测到后向子进程 stdin 注入 `ESC[?64;22c`（VT420 + ANSI 颜色），模拟 ConPTY + conhost 的一致行为。`std::sync::atomic::AtomicBool` 防重复注入（每 PTY 会话一次）。

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
| `conpty_api.rs` | ConPTY API 解析层（ADR-0005）：vendor 二进制嵌入 + Win10 提取/加载/回退 + 三函数指针封装 |
| `spawn.rs` | Tauri 命令（`pty_spawn`/`pty_write`/`pty_resize`/`pty_kill`/`pty_kill_all`）+ ConPTY 自定义实现 + PtyEvent 枚举定义 |
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

1. 新增 Tauri 命令后必须在 `lib.rs` 的 `generate_handler!` 注册 + `build.rs` 白名单 + `capabilities/default.json` 加 `allow-<cmd>`（SEC-07 契约，缺一即 invoke reject）
2. Rust `snake_case` ↔ JS `camelCase`，改 DTO 必须双边对应修改
3. 新增 `#[cfg(windows)]` 块需确认是否应放在本模块（架构约束 #9）。`win_build.rs` 更名已避开 cargo build script 歧义
4. 修改 `shell-integration.ps1` 后要跑 `test_shell_integration_script_embedded` 测试确认嵌入内容正确
5. `reader.rs` 的 `strip_conpty_startup` 修改后务必跑全部 strip 相关测试，确认不误杀正常输出
6. 修改 `READER_BUF_SIZE`/`MICRO_BATCH_MAX` 后跑 `reader_buf_size_is_16k` + `strip_startup_with_16k_boundary` + `strip_startup_with_large_payload` + `micro_batch_*` 微批 6 条测试（BE-05 契约 64KB 跨边界写死，改动须同步 `src/panels/terminal/usePtyOutput.ts` 直写阈值 256B 侧）
7. 修改 `conpty_custom` 模块（flags 计算、AttrList、ConPtyMaster、RawChild、spawn 逻辑）后跑 `conpty_custom::tests` 全部 31 条测试（另：spawn.rs 顶层 `mod tests` 28 条覆盖 validate_spawn_request/validate_session_ownership/Job Object 纯逻辑/build_cmdline 引号/BE-01 容量/BE-08 pty_kill_all）+ `pty_integration_tests` 的 `pty_spawn_custom_conpty` 端到端测试。**改 `compute_conpty_flags` 前必读其注释**——启用 PASSTHROUGH_MODE 会静默破坏 claude 全屏 TUI 滚轮（mouse input 不转发），改后须实测 claude 滚轮不回归
8. 修改 `resolve_shell_info()` / `which_full_path()` / SEC-01 白名单比对逻辑后跑 `shell::tests` 全部 24 条测试（含 4 条白名单：PATH 解析一致放行/System32 兜底/伪造路径拒绝/绝对路径非 PATH 拒绝）
9. `resolve_shell_info()` 返回的 `ShellInfo.program` **必须是完整路径**（非短名）——否则 `CreateProcessW(lpApplicationName=..., lpCommandLine=...)` 找不到可执行文件
10. 修改 `MAX_PTY_SESSIONS` 后跑 `pty_capacity_*` 三条测试 + `pty_kill_all_*` 两条（BE-01/BE-08）；修改 kill/join 超时（`KILL_JOIN_TIMEOUT`/`join_with_timeout`）后跑 `join_with_timeout_*` 三条
11. ConPTY 指针 cast：`as_raw_handle()` 已返回 `*mut c_void`，无需再 `as *mut std::ffi::c_void`——Clippy `unnecessary-cast` 会报错。同理 `std::io::Error::new(std::io::ErrorKind::Other, e)` 简化为 `std::io::Error::other(e)`（Rust 1.74+），`.map_err(|e| std::io::Error::other(e))` 进一步简化为 `.map_err(std::io::Error::other)`

## 测试模式

Rust 测试分布在 5 个位置：

| 位置 | 类型 | 用例数 | 访问级别 |
|------|------|--------|---------|
| `pty/reader.rs` `#[cfg(test)]` | 单元测试 | 42（含 BE-05 微批 6 + BE-06 join 超时 3） | `use super::*` 访问 `pub(crate)` 和私有项 |
| `pty/spawn.rs` `#[cfg(test)]` | 单元测试 | 59（`conpty_custom` 内 31 + 顶层 28） | `conpty_custom` 子模块 + 顶层 `mod tests`（validate_spawn_request/SEC-08/Job Object 纯逻辑/BE-01 容量/BE-08 pty_kill_all） |
| `pty/conpty_api.rs` `#[cfg(test)]` | 单元测试 | 5（ADR-0005 嵌入捆绑） | `use super::*` |
| `pty/shell.rs` `#[cfg(test)]` | 单元测试 | 31（SEC-01 白名单 4 + alias 兼容 7） | `use super::*` |
| `tests/pty_integration_tests.rs` | 集成测试 | 7（reattach 用例随 SEC-03 删除后） | 仅能访问 `pub` API |
| `state.rs` `#[cfg(test)]` | 单元测试 | 42 | sandbox 路径校验（含 symlink 豁免测试）+ ring buffer 纯函数测试 |

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

测试覆盖（`conpty_custom` 内 31 条）：`compute_conpty_flags`（7 条三态表——系统 Win10 19041/21375→0x3、21376/22000/22621/26100→0x7、捆绑 19041→0x7，锁死 PASSTHROUGH_MODE 0x8 永不启用）、flag 常量值验证（3 条）、`ConPtyMaster` MasterPty trait（4 条）、`AttrList` 生命周期（2 条）、`create_conpty_pair` 尺寸/尺寸修改/take_writer 单次、`build_env_block`/`build_cmdline` 引号处理、`spawn_conpty_child` 可纯化部分等。spawn.rs 顶层 `mod tests`（28 条）覆盖 `validate_spawn_request`（尺寸超限/白名单/cwd 沙箱拒绝）、`validate_session_ownership`（SEC-08 放行/拒绝）、Job Object 纯逻辑（job_name/limit flags）、**`session_capacity_check`（BE-01 三档：未达上限放行/达到上限拒绝/超上限拒绝）**、**`pty_kill_all`（BE-08：空返回 0/多 session 全灭）**与测试清理 helper。

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

测试覆盖（7 条）：echo roundtrip（写入 `echo marker` → 读取验证 marker 出现）、resize 应用（spawn → resize 30×100 → `get_size()` 验证）、kill 无孤儿（spawn → kill → `try_wait()` 验证子进程退出）、自定义 ConPTY spawn（仅 Windows CI runner）、shell 集成脚本 OSC 序列验证、会话隔离、env 注入（COLORTERM/TERM/TERM_PROGRAM）等。

### 运行约束

- **所有 PTY 测试必须 `--test-threads=1`**：`SPAWN_LOCK` + ConPTY 资源限制，并发 spawn 会死锁输出管道
- 集成测试在 Windows CI runner 上运行（依赖系统 ConPTY API）
- `shell.rs` 测试依赖 `include_str!("../../assets/shell-integration.ps1")` 编译期嵌入
