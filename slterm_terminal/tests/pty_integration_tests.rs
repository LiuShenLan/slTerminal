//! ConPTY 集成测试层（M2.4）——真机真进程，禁止 mock ConPTY。
//!
//! 覆盖：cmd 全链（spawn→写→读→resize→关闭）、默认 shell 探测链
//! （`resolve_shell`）+ pwsh 集成脚本 OSC 语义、DA1 统一身份（D02-1，
//! bundled OpenConsole 臂）、bundled 宿主句柄回归（关闭与失败 spawn 后
//! 句柄归还）、`SPAWN_LOCK` + `MAX_PTY_SESSIONS=32` 容量真验。
//!
//! **串行纪律**：真进程用例必须 `--test-threads=1` 运行；文件内再以
//! `INTEGRATION_LOCK` 兜底——容量用例会吃满会话上限，与任何并行用例
//! 互踩（并行下红灯不是语义失败，是纪律破坏）。
//!
//! **bundled 宿主用例的子进程形态**：句柄计数要求干净进程基线，且侧载
//! 状态（`conpty_status`）是进程级记录——父进程拷贝当前测试 exe 与
//! `vendor/conpty` 文件对到临时目录，以环境变量切换子臂跑同名用例
//! （上游 `windows_pty_lifecycle` 范式）。
#![cfg(windows)]

use std::sync::{Arc, Barrier};
use std::time::{Duration, Instant, SystemTime};
use std::{env, fs, process::Command};

use slterm_terminal::event::WindowSize;
use slterm_terminal::tty::{self, EventedPty, Options, Shell};

// DA1 身份用例(子臂)使用 EventLoop 全链:DA1 代答在读循环的
// StreamProcessor 内,裸 Pty 不经该层。
use slterm_terminal::event::VoidListener;
use slterm_terminal::event_loop::{EventLoop, Msg};
use slterm_terminal::sync::FairMutex;
use slterm_terminal::term::{Config, Term};

/// 标准窗口尺寸（80x24）。
const SIZE: WindowSize = WindowSize {
    num_lines: 24,
    num_cols: 80,
    cell_width: 8,
    cell_height: 16,
};

/// 集成层进程内全局锁：真机用例串行化兜底（正式纪律仍是
/// `--test-threads=1`；锁防的是漏加参数时容量/句柄用例互踩）。
static INTEGRATION_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn integration_lock() -> std::sync::MutexGuard<'static, ()> {
    INTEGRATION_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// 构造 shell 选项（白名单三 exe 之一，纯文件名直放行）。
fn options(program: &str, args: &[&str]) -> Options {
    Options {
        shell: Some(Shell::new(
            program.into(),
            args.iter().map(|a| (*a).into()).collect(),
        )),
        ..Options::default()
    }
}

/// 字节串包含判定（原始输出流形态，OSC/CSI 序列不按文本解码）。
fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

/// PATH 查找可执行文件完整路径。
fn which(name: &str) -> Option<String> {
    env::var("PATH").ok().and_then(|path| {
        env::split_paths(&path)
            .map(|dir| dir.join(name))
            .find(|full| full.exists())
            .map(|p| p.to_string_lossy().into_owned())
    })
}

/// UTF-16LE + Base64（PowerShell `-EncodedCommand` 编码，B17 链路）。
fn encode_utf16le_base64(script: &str) -> String {
    use base64::Engine as _;
    let bytes: Vec<u8> = script
        .encode_utf16()
        .flat_map(|c| c.to_le_bytes())
        .collect();
    base64::engine::general_purpose::STANDARD.encode(&bytes)
}

/// 提取 Term 网格文本（含 scrollback history，行尾 trim）。resize 的
/// 光标对账（`conpty_realign`）会把行滚进 history——断言只看可见区会
/// 间歇落空；OSC 被嗅探层消费不进 grid，普通文本在。
fn grid_text<T>(term: &Term<T>) -> String {
    use slterm_terminal::grid::Dimensions as _;
    use slterm_terminal::index::{Column, Line};
    let mut out = String::new();
    let history = term.grid().history_size() as i32;
    for row in -history..term.screen_lines() as i32 {
        let mut line = String::new();
        for col in 0..term.columns() {
            line.push(term.grid()[Line(row)][Column(col)].c);
        }
        out.push_str(line.trim_end());
        out.push('\n');
    }
    out
}

// ─── bundled 宿主子进程形态辅助 ───

/// 拷贝当前测试 exe 与 vendor/conpty 文件对到独立临时目录（子进程侧载
/// OpenConsole 的认证边界 = exe 旁完整文件对）。返回 (目录, exe 路径)。
fn stage_bundled_host(tag: &str) -> (std::path::PathBuf, std::path::PathBuf) {
    let stamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = env::temp_dir().join(format!("slterm-integ-{tag}-{}-{stamp}", std::process::id()));
    fs::create_dir(&dir).unwrap();
    let exe = dir.join(format!("{tag}.exe"));
    fs::copy(env::current_exe().unwrap(), &exe).unwrap();
    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("vendor/conpty");
    for name in ["conpty.dll", "OpenConsole.exe"] {
        fs::copy(fixtures.join(name), dir.join(name)).unwrap();
    }
    (dir, exe)
}

// ─── 用例 1:cmd 全链(spawn → 写命令 → 读输出 → resize → 关闭)───

/// 事件记录 listener:OSC 序列被 StreamProcessor 嗅探消费为事件(不进
/// grid)——壳消费的就是事件,集成断言直接验事件流;`Event::PtyWrite` 是
/// Term 的应答回写请求(DSR/模式报告等),壳职责是回环到 PTY 输入。
#[derive(Clone, Default)]
struct Recorder(Arc<std::sync::Mutex<Vec<slterm_terminal::event::Event>>>);

impl slterm_terminal::event::EventListener for Recorder {
    fn send_event(&self, event: slterm_terminal::event::Event) {
        self.0.lock().unwrap().push(event);
    }
}

impl Recorder {
    fn snapshot(&self) -> Vec<slterm_terminal::event::Event> {
        self.0.lock().unwrap().clone()
    }
}

/// conhost 窗口宽度探针（与 event_loop 光标对账同形态）：临时
/// `AttachConsole` 到子进程控制台读 `CONSOLE_SCREEN_BUFFER_INFO`——直接问
/// ConPTY 层真值，不经终端数据流（conhost 重绘编排是黑盒：子进程输出可
/// 被后续 CUP 重绘同行覆盖，grid 断言会被骗）。测试进程 stdout 是管道，
/// 短暂 detach 无碍；串行纪律下无并发 attach。
fn conhost_window_width(pid: u32) -> Option<i16> {
    use windows_sys::Win32::Foundation::{
        CloseHandle, GENERIC_READ, GENERIC_WRITE, INVALID_HANDLE_VALUE,
    };
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
    };
    use windows_sys::Win32::System::Console::{
        AttachConsole, CONSOLE_SCREEN_BUFFER_INFO, FreeConsole, GetConsoleScreenBufferInfo,
    };
    // SAFETY: 纯 Win32 控制台句柄操作,与生产探针同形态;窗口微秒级。
    unsafe {
        FreeConsole();
        if AttachConsole(pid) == 0 {
            return None;
        }
        let mut conout: Vec<u16> = "CONOUT$\0".encode_utf16().collect();
        let handle = CreateFileW(
            conout.as_mut_ptr(),
            GENERIC_READ | GENERIC_WRITE,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            std::ptr::null(),
            OPEN_EXISTING,
            0,
            std::ptr::null_mut(),
        );
        if handle == INVALID_HANDLE_VALUE {
            FreeConsole();
            return None;
        }
        let mut info: CONSOLE_SCREEN_BUFFER_INFO = std::mem::zeroed();
        let ok = GetConsoleScreenBufferInfo(handle, &mut info);
        CloseHandle(handle);
        FreeConsole();
        (ok != 0).then(|| info.srWindow.Right - info.srWindow.Left + 1)
    }
}

/// EventLoop 形态驱动辅助:产品真实形态(Pty + EventLoop + Term)——
/// in-box conhost 启动发 DSR 查询等 CPR 应答,代答在 EventLoop 读循环
/// (StreamProcessor)内;窗口外 DSR 交 Term 自答,应答经 `Event::PtyWrite`
/// 发出——本辅助轮询时把 PtyWrite 回环到 `Msg::Input`(壳语义),否则
/// conhost 等不到 CPR 会挂住行编辑。Drop 兜底关闭链:panic 路径也归还
/// 会话名额、不残留子进程。
struct DrivenTerm {
    term: Arc<FairMutex<Term<Recorder>>>,
    recorder: Recorder,
    tx: slterm_terminal::event_loop::EventLoopSender,
    join: Option<slterm_terminal::event_loop::EventLoopHandle<tty::Pty, Recorder>>,
    /// 子进程 PID(conhost 探针用;spawn 时取自 Pty)。
    child_pid: Option<u32>,
    /// 已回环的 PtyWrite 事件下标(事件列表只增不减)。
    relayed: usize,
}

impl DrivenTerm {
    fn new(opts: &Options) -> Self {
        let recorder = Recorder::default();
        let term = Arc::new(FairMutex::new(Term::new(
            Config::default(),
            &SIZE,
            recorder.clone(),
        )));
        let pty = tty::new(opts, SIZE).expect("spawn pty");
        let child_pid = pty.child_pid();
        let event_loop = EventLoop::new(term.clone(), recorder.clone(), pty, false).unwrap();
        let tx = event_loop.channel();
        let join = event_loop.spawn();
        Self {
            term,
            recorder,
            tx,
            join: Some(join),
            child_pid,
            relayed: 0,
        }
    }

    fn input(&self, bytes: &'static [u8]) {
        self.tx.send(Msg::Input(bytes.into())).expect("input 发送");
    }

    /// 壳语义回环:Term 的 PtyWrite 应答(DSR CPR 等)写回 PTY 输入。
    fn relay_pty_writes(&mut self) {
        let events = self.recorder.snapshot();
        while self.relayed < events.len() {
            if let slterm_terminal::event::Event::PtyWrite(text) = &events[self.relayed] {
                let bytes: &'static [u8] = Box::leak(text.clone().into_bytes().into_boxed_slice());
                let _ = self.tx.send(Msg::Input(bytes.into()));
            }
            self.relayed += 1;
        }
    }

    /// 轮询 grid 直到 `stop` 命中或预算尽,返回 grid 文本。
    fn await_grid(&mut self, budget: Duration, stop: impl Fn(&str) -> bool) -> String {
        let deadline = Instant::now() + budget;
        let mut seen = String::new();
        while Instant::now() < deadline {
            self.relay_pty_writes();
            {
                let t = self.term.lock();
                seen = grid_text(&t);
            }
            if stop(&seen) {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        seen
    }

    /// 关闭链:Shutdown → join(EventLoop 析构内含 Pty drop 全链)。
    fn shutdown(mut self) {
        let _ = self.tx.send(Msg::Shutdown);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

impl Drop for DrivenTerm {
    fn drop(&mut self) {
        // panic 路径兜底:归还会话名额 + 杀子进程,不留残尸。
        let _ = self.tx.send(Msg::Shutdown);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

/// cmd.exe 真 spawn 全链:写命令读回显与输出、resize 经 ConPTY 生效
/// (conhost 窗口宽度探针直读)、Shutdown 关闭链不挂起。顺带钉死回退臂
/// 可观测性:侧载尝试后未落到 bundled 时 `fallback_reason` 必须有值
/// (Win10 回退 toast 的数据源,02 篇状态观测契约)。
#[test]
fn conpty_spawn_write_read_resize_close_full_chain() {
    let _guard = integration_lock();
    let mut driven = DrivenTerm::new(&options("cmd.exe", &["/d", "/q", "/k"]));

    // 回退臂可观测性:本用例 exe 旁无文件对——默认 `conpty_sideload=true`
    // 下必为 attempted;Win11 不提取 → 回退系统 API 且原因有记录;bundled
    // 成功(exe 旁有文件对的部署形态)则无原因。两臂都合法,观测字段
    // 与后端实际状态必须自洽。
    let status = tty::conpty_status();
    assert!(
        status.attempted,
        "默认 sideload 置位下必须记录尝试: {status:?}"
    );
    assert!(
        status.bundled || status.fallback_reason.is_some(),
        "回退系统 API 时必须带原因: {status:?}"
    );

    // 写命令 → 读输出(marker 进 grid 即证写读链通)。
    // 负载裕度,非断言/语义放宽:30s 预算覆盖 Defender 扫描 + 宿主握手 +
    // 冷启动叠加(A1);真 bug 条件下该断言仍红。
    driven.input(b"echo SLTERM-M2-MARK-1\r");
    let grid = driven.await_grid(Duration::from_secs(30), |g| g.contains("SLTERM-M2-MARK-1"));
    assert!(
        grid.contains("SLTERM-M2-MARK-1"),
        "写读链未见到命令输出:\n{grid}"
    );

    // resize 80 → 47 列(奇数防巧合):经 Win32 探针直问 conhost 窗口宽度,
    // 真验 ResizePseudoConsole 生效(ConPTY 层,不是仅本地 grid 记录)。
    // ResizePseudoConsole 同步执行,轮询是等事件循环处理到该消息。
    driven
        .tx
        .send(Msg::Resize(WindowSize {
            num_lines: 24,
            num_cols: 47,
            ..SIZE
        }))
        .unwrap();
    let pid = driven.child_pid.expect("cmd 子进程 PID");
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut width = None;
    while Instant::now() < deadline {
        width = conhost_window_width(pid);
        if width == Some(47) {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(width, Some(47), "resize 后 conhost 窗口宽度应为 47 列");

    driven.shutdown();
}

/// 清理宿主目录：宿主进程可在 ClosePseudoConsole 后异步收尾，Windows
/// 短暂保留映像映射——PermissionDenied 有限重试（上游范式，5s 上界）。
fn cleanup_dir(dir: &std::path::Path) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match fs::remove_dir_all(dir) {
            Ok(()) => break,
            Err(e)
                if Instant::now() < deadline
                    && e.kind() == std::io::ErrorKind::PermissionDenied =>
            {
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => panic!("cleanup {}: {e}", dir.display()),
        }
    }
}

// ─── 用例 2:默认 shell 探测链(resolve_shell)+ pwsh 集成 OSC 真验 ───

/// `Options::default()`(shell=None → 生产 `resolve_shell` 探测链)真起
/// shell:pwsh/powershell 臂带 `-EncodedCommand` 集成脚本,bringup 即发射
/// OSC 7/9;9(→ `Event::CwdReport` cwd 报告)与 OSC 133;A(→
/// `Term::slterm_add_prompt_mark` 语义 prompt 标记,事件序保序、不发外部
/// 事件——02 篇「133;A 恢复注入闸门由语义 prompt 模型替代」);包装
/// prompt 的 OSC 133;D → `Event::CommandDone`。本机无 pwsh/powershell 时
/// 探测链回退 cmd(无集成注入),登记跳过不算失败。
#[test]
fn default_shell_resolve_chain_osc_marks() {
    let _guard = integration_lock();
    let shell = tty::resolve_shell(None).expect("resolve_shell 探测链不可全灭(cmd 兜底)");
    if tty::shell_kind_of(shell.program()) == tty::ShellKind::Cmd {
        eprintln!("本机无 pwsh/powershell,默认 shell 回退 cmd:OSC 集成语义不适用,跳过");
        return;
    }

    let mut driven = DrivenTerm::new(&Options::default());
    // 负载裕度:60s = 商店版 pwsh 冷启动 + 真实用户 profile + 宿主握手的
    // 真机上界;断言对象(事件到达 / prompt 标记置位)不变。
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut has_cwd = false;
    let mut has_prompt_mark = false;
    let mut has_command_done = false;
    while Instant::now() < deadline && !(has_cwd && has_prompt_mark && has_command_done) {
        // PSReadLine 的窗口外 DSR 查询经 Term 自答——回环防交互组件卡住。
        driven.relay_pty_writes();
        let events = driven.recorder.snapshot();
        has_cwd |= events
            .iter()
            .any(|e| matches!(e, slterm_terminal::event::Event::CwdReport(_)));
        has_command_done |= events
            .iter()
            .any(|e| matches!(e, slterm_terminal::event::Event::CommandDone { .. }));
        has_prompt_mark |= driven.term.lock().slterm_prompt_active();
        std::thread::sleep(Duration::from_millis(100));
    }
    let summary: Vec<String> = driven
        .recorder
        .snapshot()
        .iter()
        .map(|e| format!("{e:?}"))
        .collect();
    driven.shutdown();

    assert!(
        has_cwd,
        "集成脚本未产生 CwdReport(OSC 7/9;9)事件: {summary:?}"
    );
    assert!(
        has_prompt_mark,
        "集成脚本未置位语义 prompt 标记(OSC 133;A): {summary:?}"
    );
    assert!(
        has_command_done,
        "包装 prompt 未产生 CommandDone(OSC 133;D)事件: {summary:?}"
    );
}

// ─── 用例 3:pwsh 集成范式——shell-integration.ps1 经 B17 EncodedCommand 真跑 ───

/// 集成脚本在 PowerShell 语义层真跑(对照上游 `run_powershell_integration_case`
/// 范式:prelude + 脚本本体 + checks 合体,断言行经退出码与 stdout)。
/// 断言:自定义 prompt 不被覆盖(包装链调原始 prompt)、prompt 发射
/// OSC 133;D/A/B 与 OSC 7 cwd 报告。
///
/// 注:`$Host.UI.Write` 的输出 writer 在首次调用时定型——脚本末尾的
/// bringup(Update-Cwd/Send-PrePrompt)已定型为真实 stdout,checks 内
/// `[Console]::SetOut` 捕获不到后续 OSC(真机二分实证),故 OSC 断言
/// 直接落在合体脚本的 stdout 上。
#[test]
fn pwsh_integration_script_osc_semantics() {
    let _guard = integration_lock();
    let pwsh = which("pwsh.exe").or_else(|| which("powershell.exe"));
    let Some(pwsh) = pwsh else {
        eprintln!("本机无 pwsh/powershell:集成脚本用例跳过");
        return;
    };
    // prelude:自定义 prompt 先于集成脚本定义(脚本包装它,不得覆盖)。
    let prelude = "function global:prompt { 'SLTERM-USER-PROMPT> ' }\n";
    let script = include_str!("../src/tty/windows/shell-integration.ps1");
    // checks:调包装后的 prompt——自定义文本须保留(管道输出);OSC 经
    // $Host.UI.Write 进 stdout;分码退出便于定位断言行。
    let checks = r#"
$rendered = (prompt | Out-String)
if ($rendered -notlike '*SLTERM-USER-PROMPT*') { exit 31 }
[Console]::Write('PROMPT-KEPT')
exit 0
"#;
    let combined = format!("{prelude}{script}\n{checks}");
    let encoded = encode_utf16le_base64(&combined);
    // B17 链路真跑:-EncodedCommand 内联(不落盘,AMSI/ASR 红线);
    // 禁 -NoProfile 是生产 shell 参数约定,本用例测脚本语义,显式
    // -NoProfile 隔离用户 profile 变量(用例确定性优先;B17 结构守卫在 L1)。
    let out = Command::new(pwsh)
        .args(["-NoLogo", "-NoProfile", "-EncodedCommand", &encoded])
        .output()
        .expect("pwsh 进程启动");
    let stdout = out.stdout;
    let show = |bytes: &[u8]| bytes.escape_ascii().to_string();
    assert!(
        out.status.success(),
        "集成脚本语义失败 ({}):\nstdout: {}\nstderr: {}",
        out.status,
        show(&stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    // 包装 prompt 真跑且自定义文本保留(checks  exit 31 闸 + 标记)。
    assert!(
        contains(&stdout, b"PROMPT-KEPT"),
        "自定义 prompt 未保留: {}",
        show(&stdout)
    );
    // 133;D/133;B 仅包装 prompt 发射(脚本末尾 bringup 只有 133;A+OSC 7)
    // ——stdout 含它们即证包装链真跑。
    for (seq, name) in [
        (b"\x1b]133;D;".as_slice(), "OSC 133;D 退出码报告"),
        (b"\x1b]133;A", "OSC 133;A prompt 前标记"),
        (b"\x1b]133;B", "OSC 133;B prompt 后标记"),
        (b"\x1b]7;file://", "OSC 7 cwd 报告"),
    ] {
        assert!(
            contains(&stdout, seq),
            "集成脚本未发射 {name}: {}",
            show(&stdout)
        );
    }
}

// ─── 用例 4:DA1 统一身份(D02-1)——bundled OpenConsole 臂 ───

/// DA1 应答身份真机验证:子进程(exe 旁文件对 → 侧载 OpenConsole)内
/// pwsh 发 DA1 查询(`ESC[c`),经 EventLoop 读循环检测代答,子进程 stdin
/// 读回应答并 base64 回显到输出流,断言应答含统一身份 `\x1b[?64;22c`
/// 且不含其他身份前缀(in-box conhost 自答的 `?61;...` 形态)。
///
/// 为什么必须 bundled 臂:in-box conhost 收到输出流中的 DA1 查询会自答
/// (实测 31 字节 `?61;6;7;21;22;23;24;28;32;42c`),统一身份的唯一暴露
/// 面在 bundled 宿主(OpenConsole 握手与子进程查询都由 priming/代答
/// 覆盖)。子进程形态:侧载状态是进程级记录,与主测试进程隔离。
#[test]
fn da1_unified_identity_bundled_host() {
    if env::var_os("SLTERM_DA1_CHILD").is_none() {
        // 父臂:组宿主目录 → 子进程跑同名用例 → 清理(失败即非零退出)。
        let (dir, exe) = stage_bundled_host("da1");
        let status = Command::new(&exe)
            .args([
                "--exact",
                "da1_unified_identity_bundled_host",
                "--nocapture",
            ])
            .env("SLTERM_DA1_CHILD", "1")
            .status()
            .expect("DA1 子进程启动");
        cleanup_dir(&dir);
        assert!(status.success(), "DA1 子臂失败(输出见上)");
        return;
    }
    da1_child_impl();
}

/// 子臂:bundled PTY + EventLoop,pwsh 脚本发 DA1 并回读 stdin 应答。
fn da1_child_impl() {
    // 脚本:stdin 控制台切 VT 输入模式(去行缓冲/回显/处理键)→ 发 DA1 →
    // 异步读 stdin → 应答 base64 回显。8s 读超时是脚本内上界。
    let script = r#"
$sig = '[System.Runtime.InteropServices.DllImport("kernel32.dll")] public static extern System.IntPtr GetStdHandle(int n);[System.Runtime.InteropServices.DllImport("kernel32.dll")] public static extern bool GetConsoleMode(System.IntPtr h, out uint m);[System.Runtime.InteropServices.DllImport("kernel32.dll")] public static extern bool SetConsoleMode(System.IntPtr h, uint m);'
$k = Add-Type -MemberDefinition $sig -Name NativeConsole -Namespace SltermInteg -PassThru
$h = $k::GetStdHandle(-10)
$m = [uint32]0
[void]$k::GetConsoleMode($h, [ref]$m)
[void]$k::SetConsoleMode($h, ($m -band (-bnot (0x1 -bor 0x2 -bor 0x4))) -bor 0x200)
[Console]::Write([char]27 + '[c')
$in = [Console]::OpenStandardInput()
$buf = New-Object byte[] 128
$task = $in.ReadAsync($buf, 0, 128)
$n = -1
try { if ($task.Wait(8000)) { $n = $task.Result } } catch { [Console]::Write('EX:' + $_.Exception.Message + ';') }
[Console]::Write('S4:n=' + $n + ';')
if ($n -gt 0) { [Console]::Write('RESP:' + [Convert]::ToBase64String($buf[0..($n-1)]) + ':END') }
"#;
    let encoded = encode_utf16le_base64(script);
    // 桌面版 powershell.exe 优先(变量少);pwsh 探测链形态由用例 2 覆盖。
    let pwsh = which("powershell.exe")
        .or_else(|| which("pwsh.exe"))
        .expect("powershell/pwsh");
    let opts = Options {
        shell: Some(Shell::new(
            pwsh,
            vec![
                "-NoLogo".into(),
                "-NoProfile".into(),
                "-EncodedCommand".into(),
                encoded,
            ],
        )),
        ..Options::default()
    };
    // 判别力前提:本用例必须真走 bundled 臂,否则应答身份不是我们的代答。
    let pty = tty::new(&opts, SIZE).expect("bundled pty");
    let status = tty::conpty_status();
    assert!(
        status.bundled,
        "DA1 用例须走 bundled OpenConsole 臂,实际: {status:?}"
    );
    da1_child_drive(pty);
}

/// 子臂驱动:EventLoop 跑起来,轮询 grid 直到 RESP 标记,解码断言身份。
fn da1_child_drive(pty: tty::Pty) {
    use base64::Engine as _;
    let term = Arc::new(FairMutex::new(Term::new(
        Config::default(),
        &SIZE,
        VoidListener,
    )));
    let event_loop = EventLoop::new(term.clone(), VoidListener, pty, false).unwrap();
    let tx = event_loop.channel();
    let join = event_loop.spawn();

    // 负载裕度:30s = OpenConsole 握手 + pwsh 冷启动真机上界;断言对象
    // (应答字节身份)不变。
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut seen = String::new();
    while Instant::now() < deadline {
        {
            let t = term.lock();
            seen = grid_text(&t);
        }
        if seen.contains("RESP:") && seen.contains(":END") {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let _ = tx.send(Msg::Shutdown);
    let _ = join.join();

    let start = seen
        .find("RESP:")
        .unwrap_or_else(|| panic!("未见应答回显:\n{seen}"))
        + 5;
    let end = seen[start..]
        .find(":END")
        .map(|i| start + i)
        .expect("RESP 未闭合");
    let resp = base64::engine::general_purpose::STANDARD
        .decode(seen[start..end].trim())
        .expect("RESP base64 解码");
    // 统一身份(D02-1):priming/代答同值;priming 残留与本次代答可能
    // 各贡献一份,逐份都须是该身份。
    assert!(
        contains(&resp, b"\x1b[?64;22c"),
        "DA1 应答缺统一身份 ?64;22c: {}",
        resp.escape_ascii()
    );
    for alien in [
        b"\x1b[?61;".as_slice(),
        b"\x1b[?62;",
        b"\x1b[?63;",
        b"\x1b[?65;",
    ] {
        assert!(
            !contains(&resp, alien),
            "DA1 应答混入异身份 {}: {}",
            alien.escape_ascii(),
            resp.escape_ascii()
        );
    }
}

// ─── 用例 5:bundled 宿主句柄回归(上游 windows_pty_lifecycle 语义)───

/// 进程句柄计数（独占测试进程内才干净，故本用例走子进程形态）。
fn handle_count() -> u32 {
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, GetProcessHandleCount};
    let mut count = 0;
    assert_ne!(
        unsafe { GetProcessHandleCount(GetCurrentProcess(), &mut count) },
        0
    );
    count
}

/// 建一个 PTY 随即关闭：给宿主与 shell 300ms 启动（含 busy shell），**不
/// 消费输出**——close 必须能自己 drain 背压管道（语义核心，勿改成读空）。
fn close_pty(opts: &Options) {
    let pty = tty::new(opts, SIZE).expect("create ConPTY");
    std::thread::sleep(Duration::from_millis(300));
    drop(pty);
}

/// 断言句柄归还：预算 = 基线 + 2（两个进程级 lazy wait-pool 句柄，
/// 非 per-PTY 配额）。上游纪律：不得放宽预算吸收 OS 句柄缺陷。
fn assert_handles_return(baseline: u32, phase: &str) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let current = handle_count();
        if current <= baseline + 2 {
            eprintln!("{phase}: {current} handles (warm baseline {baseline})");
            return;
        }
        assert!(
            Instant::now() < deadline,
            "{phase}: 句柄泄漏: {baseline} -> {current}"
        );
        std::thread::sleep(Duration::from_millis(25));
    }
}

/// bundled conpty 资源在 close 与失败 spawn 后归还（句柄计数前后对比）。
/// 子进程形态：句柄计数须干净进程基线；侧载文件对随 exe 入临时目录。
#[test]
fn bundled_conpty_resources_return_after_close_and_failed_spawn() {
    if env::var_os("SLTERM_HANDLE_CHILD").is_none() {
        let (dir, exe) = stage_bundled_host("handles");
        let status = Command::new(&exe)
            .args([
                "--exact",
                "bundled_conpty_resources_return_after_close_and_failed_spawn",
                "--nocapture",
            ])
            .env("SLTERM_HANDLE_CHILD", "1")
            .status();
        // 宿主可在 ClosePseudoConsole 后异步收尾：清理带有限重试。
        cleanup_dir(&dir);
        assert!(status.unwrap().success(), "句柄回归子臂失败(输出见上)");
        return;
    }
    handle_child_impl();
}

/// 子臂：idle/busy/失败 spawn 三轮八次，句柄归还逐段断言。
fn handle_child_impl() {
    use std::sync::mpsc;
    // drop 死锁必须让用例失败,而不是无限挂住 cargo(上游纪律)。
    let (finished, result) = mpsc::channel();
    std::thread::spawn(move || {
        let outcome = std::panic::catch_unwind(|| {
            let idle = options("cmd.exe", &["/d", "/q", "/k"]);
            for _ in 0..3 {
                close_pty(&idle);
            }
            std::thread::sleep(Duration::from_millis(500));
            let baseline = handle_count();

            for _ in 0..8 {
                close_pty(&idle);
            }
            assert_handles_return(baseline, "idle tab close");

            // busy:输出背压下关闭(drain_detached 路径)。
            let busy = options(
                "cmd.exe",
                &[
                    "/d",
                    "/q",
                    "/c",
                    "for /L %i in (1,1,2147483647) do @echo 0123456789012345678901234567890123456789",
                ],
            );
            for _ in 0..8 {
                close_pty(&busy);
            }
            assert_handles_return(baseline, "busy tab close");

            // 失败 spawn:working_directory 不存在 → CreateProcessW 失败,
            // ConPTY 已建——测 conpty 层失败路径的句柄归还(白名单外程序
            // 在更早的白名单层被拒,测不到本路径,故用合法 shell + 假 cwd)。
            let mut failed = options("cmd.exe", &["/d", "/q", "/k"]);
            failed.working_directory =
                Some(std::path::PathBuf::from("C:\\slterm-nonexistent-dir-m24"));
            for _ in 0..8 {
                assert!(tty::new(&failed, SIZE).is_err(), "假 cwd 应使 spawn 失败");
            }
            assert_handles_return(baseline, "failed shell spawn");
        });
        finished.send(outcome.is_ok()).ok();
    });
    assert_eq!(
        result.recv_timeout(Duration::from_secs(90)),
        Ok(true),
        "句柄回归失败或 teardown 90 秒内未完成(负载裕度: bundled 宿主启动税 × 27 轮)"
    );
}

// ─── 用例 6:SPAWN_LOCK 串行化 + MAX_PTY_SESSIONS=32 容量真验 ───

/// 并发真 spawn 到上限:32 个真 PTY 占满 → 第 33 个被拒;释放 1 个名额
/// 后 4 线程并发抢,恰 1 成 3 拒(SPAWN_LOCK 内「容量判定 + 占用登记」
/// 原子);全部 drop 后名额归还,再 spawn 成功。
///
/// 走 in-box 臂(`conpty_sideload: false`):容量/串行语义与后端无关,
/// in-box 避开 OpenConsole 每会话启动税,缩短真机耗时。
#[test]
fn spawn_lock_serializes_and_capacity_32_enforced() {
    let _guard = integration_lock();
    let opts = Options {
        conpty_sideload: false,
        ..options("cmd.exe", &["/d", "/q", "/k"])
    };

    let mut held = Vec::new();
    for i in 0..tty::MAX_PTY_SESSIONS {
        held.push(tty::new(&opts, SIZE).unwrap_or_else(|e| panic!("第 {i} 个会话 spawn: {e}")));
    }
    let err = match tty::new(&opts, SIZE) {
        Ok(_) => panic!("第 33 个会话必须被拒"),
        Err(e) => e,
    };
    assert!(err.to_string().contains("32"), "拒绝消息应含上限值: {err}");

    // 并发抢名额:释放 1 个 → 4 线程 barrier 同发 → 恰 1 成 3 拒。
    //
    // A1:成功者必须**保持 Pty 存活到主线程统计完成**再释放。旧写法在线程
    // 内立即 drop,`Pty::drop` 的 session_closed() 会先于其他线程的容量
    // 检查归还名额,让第二个线程合法成功(「2 成功」),断言依赖调度而非锁。
    drop(held.pop());
    let barrier = Arc::new(Barrier::new(4));
    let release = Arc::new(Barrier::new(4 + 1));
    let (tx, rx) = std::sync::mpsc::channel();
    let mut handles = Vec::new();
    for _ in 0..4 {
        let barrier = Arc::clone(&barrier);
        let release = Arc::clone(&release);
        let tx = tx.clone();
        let opts = opts.clone();
        handles.push(std::thread::spawn(move || {
            barrier.wait();
            let pty = tty::new(&opts, SIZE).ok();
            tx.send(pty.is_some()).expect("主线程仍在收集结果");
            // 主线程断言前不得归还名额。
            release.wait();
            drop(pty);
        }));
    }
    drop(tx);
    let mut succeeded = 0;
    for _ in 0..4 {
        if rx.recv().expect("工作线程必须回报结果") {
            succeeded += 1;
        }
    }
    assert_eq!(succeeded, 1, "上限-1 并发抢名额:恰一个成功");
    release.wait();
    for h in handles {
        h.join().unwrap();
    }

    // 归还验证:全部 drop → 名额清空 → 再 spawn 成功。
    drop(held);
    let pty = tty::new(&opts, SIZE).expect("归还后 spawn 应成功");
    drop(pty);
}

/// 显式命令通道(`-e`):`explicit_command=true` 时白名单外程序放行
/// (SEC-01 豁免论证见 `Options::explicit_command`);同程序 `false` 时
/// 仍被白名单拒绝。用 System32 恒在的 `where.exe`(非白名单三家)作探针。
#[test]
fn explicit_command_bypasses_shell_allowlist() {
    let _guard = integration_lock();
    let system_root = env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
    let where_exe = format!(r"{system_root}\System32\where.exe");
    assert!(
        std::path::Path::new(&where_exe).exists(),
        "前提: where.exe 恒在"
    );

    // 默认(非显式)通道:非白名单程序必须被拒。
    let denied = tty::new(&options(&where_exe, &[]), SIZE);
    assert!(denied.is_err(), "非显式通道下 where.exe 应被白名单拒绝");

    // 显式命令通道:同程序放行,spawn 真成功。
    let mut opts = options(&where_exe, &[]);
    opts.explicit_command = true;
    let pty = tty::new(&opts, SIZE).expect("显式命令通道应放行白名单外程序");
    drop(pty);
}
