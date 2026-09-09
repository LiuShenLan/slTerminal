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

### flags 模式能力矩阵（CP-009；默认矩阵 = 旧三态，零默认漂移）

`compute_conpty_flags(build, bundled, modes)`——modes = `ConptyInputModes` 能力矩阵
（设置键 `conptyInputModes`，spawn 时读段；缺失/解析失败回退默认矩阵不阻塞 spawn）。
逐位取矩阵，默认矩阵输出与旧三态恒等：
- 0x1/0x2 直取矩阵位；
- 0x4 维持 build 门控：捆绑新 conhost（仅 Win10）或系统 conhost + Win11
  （≥`CONPTY_WIN11_MIN_BUILD`）才置位；系统 conhost + Win10 回退不置位 → `0x3`；
- 0x8 为末行矩阵位，默认矩阵不含 0x8。

默认矩阵三态等价守卫：`conpty_flags_default_matrix_matches_legacy_tristate`
（三输入 × 默认矩阵 → `0x7`/`0x7`/`0x3`）——任何默认矩阵位翻转（含 0x8 默认置位）即红。

阈值 21376 与前端 xterm 钳制（ADR-0004）同源。

### Win10 捆绑 conhost（ADR-0005）

老 Win10（build < 21376）in-box conhost 不转发鼠标 VT 序列。`vendor/conpty/` 的 conpty.dll + OpenConsole.exe 经 `include_bytes!` 嵌入，仅 Win10 在首次 spawn 前提取到 `%LOCALAPPDATA%\slterminal\conpty\` 并 `LoadLibraryW` 加载。加载/提取失败静默回退系统 ConPTY；Win11 零变化。vendor 更新后必须 Win10 实机验证。

回退状态经 `pty_conpty_status` 一次性查询暴露，启动 toast 提示降级后果（CP-010）；warn 日志与 `fallback_reason` 同源（同一 `format!("{e:#}")` 变量，零漂移）。

### PASSTHROUGH_MODE (0x8) 默认禁用 + 能力矩阵可配置化（CP-009）

0x8 会让 claude 等全屏 TUI 的鼠标滚轮完全失效（2026-07 Win11 build 26200 双向实测，
阻断条件仅真实 claude 场景复现——最小实验假阴性）。矩阵化后 0x8 仍默认关闭，经设置
页「终端输入模式」的 passthrough 开关（旁常驻警示文案）可显式启用——启用/默认翻转
都须过 ADR-0007 门禁第 3 条人工实测（真实 claude 全屏滚轮），无实测记录禁合入。

### cwd 反斜杠规范化

传给 ConPTY/`CreateProcessW` 前将 cwd 中的 `/` 替换为 `\`。

### stdin writer 必须随 PtySession 存活

绝对不能 drop stdin writer。writer 通过 `Arc<Mutex>` 共享，生命周期跟随 `PtySession`。

### Job Object 孤儿防护

每个子进程放入 Job Object，设置 `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`。父进程崩溃/退出时 OS 自动杀子进程。

### pty_kill 异步销毁

`ClosePseudoConsole` 在 pre-Win11 24H2 上可能永久阻塞（上游 Discussion #17716，Win10 永不修复）。`pty_kill` 先提取 session 释放写锁，再在 `spawn_blocking` 中执行 `kill → join reader`（`crate::thread_join::join_with_timeout` + `JOIN_TIMEOUT`=3s，BE-01 上提 crate 顶层共享件）：正常路径随闭包尾 drop；超时路径 reader detach、session 移入监督线程执行 drop（关 writer + ClosePseudoConsole），监督 3s 超时则清理线程 detach，进程退出时 OS 回收句柄（Job Object 保证子进程先死）。`PtySession::drop`（state.rs）同样只做带超时 join（CP-011）。

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
- 双侧 `canonicalize` 均失败时回退 Win32 句柄级文件身份比对（volume serial + file index）：真实文件取普通句柄身份；应用执行别名（AEL，普通 `CreateFileW` 实测 os error 1920 打不开）经 `FILE_FLAG_OPEN_REPARSE_POINT` 打开条目本身取 reparse 条目身份——两侧同一条目身份必然相等；两侧证据齐且相等才放行，任一侧证据缺失即拒绝，不降级字符串；单侧失败即拒绝。

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

- **默认矩阵不含 0x8**：任何 0x8 启用/默认矩阵位翻转须过 ADR-0007 门禁第 3 条人工实测（真实 claude 全屏 TUI 滚轮滚动），无实测记录禁合入；自动化守卫 = `conpty_flags_default_matrix_matches_legacy_tristate` 用例绿 + 默认矩阵等价断言。
- **Win10 捆绑 conhost 改动必须实机验证**：鼠标转发、键盘/IME/kitty、resize 无法靠 CI 守卫。
- **PowerShell 交互 shell 禁止 `-NoProfile`**：用户 profile（conda init 钩子等）必须原生加载——缺钩子则 `conda activate` 失效（win11 CondaError / win10 conda.bat 静默空转，B17）。
- **不要把 `#[cfg(windows)]` 放到本模块外**。
- **不要 drop stdin writer**。
- **禁止裸 join（BE-01）**：全部线程退出点统一经 `crate::thread_join::join_with_timeout`（crate 顶层共享件，超时分支 `tracing::warn` + detach，不再无界阻塞）——全仓守卫 `rg "\.join\(\)" src-tauri/src` 仅命中 thread_join.rs 白名单一处；`CleanupPlan`/`plan_cleanup_after_join_timeout` 留 reader.rs（pty 专有清理语义，不随迁）。
- **不要 stop/start 轮换 watcher**（见 @../notify/CLAUDE.md），与 pty 无关但常被误用。
- **不要解析提示符跟踪 cwd**：portable-pty 在 Windows 不返回 cwd，只能靠 OSC 7/133 序列。

## 测试模式

- **所有 PTY 测试必须 `--test-threads=1`**：ConPTY 并发 spawn 死锁。
- **集成测试**：真实 spawn `cmd.exe`，仅 Windows CI runner 上运行。

### 既定豁免（已在 `.claude/test-exemptions.md` 登记）

> 下 4 行（既有）为 PTY-12/CP-011/PTY-08/TQ-COV-03 等历史登记；**再下 8 行 = TQ-COV 收尾 CP-023 追加（2026-09-08）**——pty 残余行覆盖缺口逐条三列入表（A 分支：`#[coverage(off)]` stable 未稳定，不引入属性）。

| 豁免项 | 原因 | 当前兜底 |
|--------|------|---------|
| `reader_loop` 残余 I/O 编排 | 依赖 Channel/管道系统调用，无法在 L1 构造输入（CP-034: Channel 直写，无锁层） | 可纯函数化部分（`apply_startup_strip`/`should_inject_da1`/`eof_exit_code`/`micro_batch_tail`）已由 L1 覆盖 |
| `pty_kill` 超时→监督线程真实阻塞路径 | Win32 阻塞不可注入（ClosePseudoConsole 永久阻塞无法在 L1 构造） | 清理决策由 L1 `plan_cleanup_after_join_timeout` 2 例锁死 + pty 集成 kill 用例 + Win10 实机人工验证点（杀会话后应用无挂起） |
| 容量超限 kill 清理 | 命中上限后 kill 已 spawn 子进程依赖真实 PtySession | BE-01 判定语义由纯函数用例锁死 + Job Object 兜底 |
| `conpty_api` vendor 提取/加载回退 | 依赖真实 DLL 加载行为 | ADR-0005 Win10 实机人工验证 + `ensure_extracted` 幂等用例 + 回退状态可观测（`pty_conpty_status` + 启动 toast） |
| spawn.rs `pty_spawn`/`pty_write`/`pty_resize`/`pty_kill`/`pty_kill_all` 命令层 + PtySession 装配 + reader_loop 启动（约 :1226-1667） | Tauri State/Channel 运行时胶水（与 lib.rs run() 行同构） | L4 `terminal.e2e.ts` 真实 spawn/write/kill/杀 app 链 + `pty_integration_tests` |
| spawn.rs `add_to_job_object`/`create_and_assign_job`（约 :1692-1778） | Win32 API 组合（CreateJobObjectW/SetInformationJobObject/OpenProcess/AssignProcessToJobObject + wide 编码） | `job_name`/`job_limits` 纯构件已抽单测 |
| spawn.rs `conpty_custom` RawChild try_wait/wait/clone_killer/as_raw_handle/Debug + ConPtyMaster::resize 有效 hpc 路径（约 :320-328/412-414/430-497） | 依赖真实子进程句柄的 Win32 组合（集成测试只 kill 不 wait） | `pty_integration_tests` 真实会话兜底 |
| spawn.rs AttrList/CreateProcessW 失败 bail（约 :260/617-618） | 失败注入不可行 | L1 其余分支覆盖 + 无失败注入通道登记 |
| spawn.rs SendRawHandle pending 尾（约 :543） | 句柄发送协议尾 | 同上 |
| shell.rs 白名单拒绝分支与 canonicalize/身份比对回退（约 :59/86-88/139/184/211/290） | 依赖真实 fs/别名身份判定分支 | shell.rs 行覆盖 95.87%（残余 ~10 生产行）+ allowlist 真机用例 |
| conpty_api.rs Bundled fn 指针 create/close/resize 臂（约 :130-171） | vendor dll 函数指针错误臂（错误注入不可行） | T6/T7 真实 LoadLibraryW 用例 + 有效路径覆盖 |
| conpty_api.rs 生产 conpty_status + pty_conpty_status 命令胶水（约 :293-322） | 命令胶水（lib.rs run() 同构） | L2 ipc-pty-contract + 启动链 toast 真实执行 |
