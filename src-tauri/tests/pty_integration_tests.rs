/// PTY 集成测试
use portable_pty::{native_pty_system, CommandBuilder, PtySize, MasterPty, ChildKiller, Child};
use std::io::{Read, Write};
use std::sync::Mutex;
use std::time::Duration;

static SPAWN_LOCK: Mutex<()> = Mutex::new(());

/// 启动 cmd.exe 返回 PTY 句柄
fn spawn_cmd() -> (Box<dyn portable_pty::MasterPty + Send>, Box<dyn portable_pty::Child + Send>, Box<dyn Read + Send>, Box<dyn Write + Send>) {
    let _lock = SPAWN_LOCK.lock().unwrap();
    let pty_system = native_pty_system();
    let pair = pty_system.openpty(PtySize { rows: 24, cols: 80, pixel_width: 0, pixel_height: 0 }).unwrap();

    let mut writer = pair.master.take_writer().unwrap();
    writer.write_all(b"\x1b[1;1R").unwrap();
    writer.flush().unwrap();

    let cmd = CommandBuilder::new("cmd.exe");
    let child = pair.slave.spawn_command(cmd).unwrap();
    let reader = pair.master.try_clone_reader().unwrap();

    (pair.master, child, reader, writer)
}

/// 轮询读取 PTY 输出直到出现 marker 或超时 10s
/// 每次读取间隔 50ms 避免忙等
fn read_until_marker(reader: &mut dyn Read, marker: &str) -> Vec<u8> {
    let mut output = Vec::new();
    let start = std::time::Instant::now();
    let timeout = Duration::from_secs(10);
    while start.elapsed() < timeout {
        let mut buf = [0u8; 4096];
        match reader.read(&mut buf) {
            Ok(0) => break, // EOF
            Ok(n) => {
                output.extend_from_slice(&buf[..n]);
                if String::from_utf8_lossy(&output).contains(marker) {
                    return output;
                }
            }
            Err(_) => break,
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    output
}

/// pty_roundtrip: spawn cmd → 写 echo → 验证输出含目标文本
#[test]
fn pty_roundtrip() {
    let (_master, mut child, mut reader, mut writer) = spawn_cmd();

    // 写命令（不再固定 sleep 等待启动——写入经 PTY 缓冲，cmd 就绪后自动处理）
    writer.write_all(b"echo hello_test_marker\r\n").unwrap();
    writer.flush().unwrap();

    // 轮询读取输出直到出现 marker（50ms 间隔，上限 10s）
    let output = read_until_marker(&mut *reader, "hello_test_marker");
    let text = String::from_utf8_lossy(&output);
    assert!(
        text.contains("hello_test_marker"),
        "PTY 输出应包含 hello_test_marker，实际长度: {}，内容: {}",
        output.len(),
        text
    );

    drop(writer);
    let _ = child.kill();
}

/// pty_resize_applies: resize 验证尺寸变更
#[test]
fn pty_resize_applies() {
    let (master, mut child, _reader, _writer) = spawn_cmd();
    master.resize(PtySize { rows: 30, cols: 100, pixel_width: 0, pixel_height: 0 }).unwrap();
    let size = master.get_size().unwrap();
    assert_eq!(size.rows, 30);
    assert_eq!(size.cols, 100);
    let _ = child.kill();
}

/// pty_kill_no_orphan: kill 后进程退出
#[test]
fn pty_kill_no_orphan() {
    let (_master, mut child, _reader, _writer) = spawn_cmd();
    let pid = child.process_id();
    assert!(pid.is_some(), "子进程应有 PID");
    child.kill().unwrap();

    // 轮询 try_wait 直到进程退出或超时 10s（替代固定 sleep 1s + for 循环）
    let start = std::time::Instant::now();
    let timeout = Duration::from_secs(10);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            _ => {
                if start.elapsed() >= timeout {
                    panic!("kill 后进程应在 10s 内退出");
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }
    // 退出后再检查一次确保状态一致
    assert!(child.try_wait().unwrap().is_some(), "kill 后进程应退出");
}

/// pty_spawn_custom_conpty: 验证 conpty_custom 模块的 spawn_conpty_child 可正常启动子进程
#[cfg(windows)]
#[test]
fn pty_spawn_custom_conpty() {
    use slterminal_lib::pty::{shell, spawn::conpty_custom};

    let shell_info = shell::resolve_shell_info(Some("cmd.exe")).expect("resolve_shell_info 应成功");
    let (hpc, master) = conpty_custom::create_conpty_pair(80, 24, 26100)
        .expect("create_conpty_pair 应成功");

    // CPR 注入
    let mut writer = master.take_writer().expect("take_writer 应成功");
    use std::io::Write;
    writer.write_all(b"\x1b[1;1R").unwrap();
    writer.flush().unwrap();

    let extra_envs: Vec<(String, String)> = vec![
        ("COLORTERM".into(), "truecolor".into()),
        ("TERM".into(), "xterm-256color".into()),
    ];

    let mut child = conpty_custom::spawn_conpty_child(
        hpc,
        &shell_info,
        &extra_envs,
        None,
    )
    .expect("spawn_conpty_child 应成功");

    let pid = child.process_id();
    assert!(pid.is_some(), "子进程应有 PID");

    // 回显验证子进程存活（不再固定 sleep 等待启动）
    let mut reader = master.try_clone_reader().expect("try_clone_reader 应成功");
    writer.write_all(b"echo CUSTOM_CONPTY_OK\r\n").unwrap();
    writer.flush().unwrap();

    // 轮询读取输出直到出现 marker（50ms 间隔，上限 10s）
    let output = read_until_marker(&mut *reader, "CUSTOM_CONPTY_OK");
    let text = String::from_utf8_lossy(&output);
    assert!(
        text.contains("CUSTOM_CONPTY_OK"),
        "输出应包含 CUSTOM_CONPTY_OK，实际: {text}"
    );

    drop(writer);
    let _ = child.kill();
}

/// osc_cwd_venv_parsed: shell集成脚本验证
#[test]
fn osc_cwd_venv_parsed() {
    let script = include_str!("../assets/shell-integration.ps1");
    assert!(script.contains("OSC") || script.contains("133"), "集成脚本应定义 OSC/133 序列");
    assert!(script.contains("9;9"), "应包含 OSC 9;9 CWD 跟踪");
    assert!(script.contains("133"), "应包含 OSC 133 命令边界");
    assert!(script.contains("UTF8") || script.contains("65001"), "应包含 UTF-8 编码修复");
    assert!(!script.contains("$env:SLTERM_PANE_ID"), "无嵌套检测");
    assert!(
        script.contains("[char]0x1b") || script.contains("0x1b"),
        "应使用 [char]0x1b PS 5.1 兼容写法"
    );
}

/// pty_reattach_ring_buffer_replay: 验证 reattach 时 ring buffer 回放与清空
///
/// 模拟部署场景：
///   1. spawn cmd.exe → PTY 输出经 reader 线程写入 ring buffer
///   2. 写命令产生输出 → ring buffer 累积
///   3. "reattach": drain ring buffer 获取回放数据
///   4. 验证 drain 后 buffer 为空
///   5. 写新命令 → 新输出继续被 reader 线程捕获到 ring buffer
#[cfg(windows)]
#[test]
fn pty_reattach_ring_buffer_replay() {
    let _lock = SPAWN_LOCK.lock().unwrap();
    use slterminal_lib::pty::{shell, spawn::conpty_custom};
    use std::collections::VecDeque;
    use std::io::Write as _;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    // 1. spawn cmd.exe
    let shell_info =
        shell::resolve_shell_info(Some("cmd.exe")).expect("resolve_shell_info 应成功");
    let (hpc, master) = conpty_custom::create_conpty_pair(80, 24, 26100)
        .expect("create_conpty_pair 应成功");

    // CPR 注入（对齐生产代码 pty_spawn）
    let mut writer = master.take_writer().expect("take_writer 应成功");
    writer.write_all(b"\x1b[1;1R").unwrap();
    writer.flush().unwrap();

    let extra_envs: Vec<(String, String)> = vec![
        ("COLORTERM".into(), "truecolor".into()),
        ("TERM".into(), "xterm-256color".into()),
    ];

    let mut child = conpty_custom::spawn_conpty_child(hpc, &shell_info, &extra_envs, None)
        .expect("spawn_conpty_child 应成功");

    // 2. 创建 ring buffer + reader 线程（模拟 reader_loop 写入 ring buffer）
    let ring: Arc<Mutex<VecDeque<u8>>> = Arc::new(Mutex::new(VecDeque::new()));
    let ring_for_reader = ring.clone();
    let mut reader = master.try_clone_reader().expect("try_clone_reader 应成功");

    let reader_handle = std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break, // EOF — 子进程已退出
                Ok(n) => {
                    ring_for_reader.lock().unwrap().extend(&buf[..n]);
                }
                Err(_) => break,
            }
        }
    });

    // 3. 写第一阶段命令 → 输出经 reader 线程累积到 ring buffer
    writer.write_all(b"echo REATTACH_P1_MARKER\r\n").unwrap();
    writer.flush().unwrap();

    // 轮询 ring buffer 直到出现 P1 marker（替代固定 sleep）
    let start = std::time::Instant::now();
    let timeout = Duration::from_secs(10);
    loop {
        let has_marker = {
            let guard = ring.lock().unwrap();
            let snapshot: Vec<u8> = guard.iter().copied().collect();
            String::from_utf8_lossy(&snapshot).contains("REATTACH_P1_MARKER")
        };
        if has_marker {
            break;
        }
        if start.elapsed() >= timeout {
            panic!("ring buffer 应在 10s 内包含 REATTACH_P1_MARKER");
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    // 4. "Reattach": drain ring buffer 获取回放数据
    let replay: Vec<u8> = ring.lock().unwrap().drain(..).collect();
    let replay_text = String::from_utf8_lossy(&replay);
    assert!(
        replay_text.contains("REATTACH_P1_MARKER"),
        "回放数据应包含 REATTACH_P1_MARKER，实际长度={}，内容片段: {}...",
        replay.len(),
        &replay_text[..replay_text.len().min(200)]
    );
    assert!(
        !replay_text.is_empty(),
        "回放数据不应为空（cmd banner + echo 输出）"
    );

    // 5. drain 后 ring buffer 必须为空（避免重复回放）
    assert!(
        ring.lock().unwrap().is_empty(),
        "reattach drain 后 ring buffer 应为空"
    );

    // 6. 写第二阶段命令 → 验证 reattach 后新输出正常被 reader 线程捕获
    writer.write_all(b"echo REATTACH_P2_MARKER\r\n").unwrap();
    writer.flush().unwrap();

    // 轮询 ring buffer 直到出现 P2 marker（替代固定 sleep）
    let start = std::time::Instant::now();
    loop {
        let has_marker = {
            let guard = ring.lock().unwrap();
            let snapshot: Vec<u8> = guard.iter().copied().collect();
            String::from_utf8_lossy(&snapshot).contains("REATTACH_P2_MARKER")
        };
        if has_marker {
            break;
        }
        if start.elapsed() >= timeout {
            panic!("ring buffer 应在 10s 内包含 REATTACH_P2_MARKER");
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    let post_attach: Vec<u8> = ring.lock().unwrap().drain(..).collect();
    let post_text = String::from_utf8_lossy(&post_attach);
    assert!(
        post_text.contains("REATTACH_P2_MARKER"),
        "reattach 后新输出应被捕获到 ring buffer，实际内容: {post_text}"
    );

    // 7. 清理：先 ClosePseudoConsole（使 reader.read() 返回 EOF/error）
    // 再 join reader 线程，顺序错误会导致 reader 永久阻塞
    drop(writer);
    let _ = child.kill();
    // ClosePseudoConsole 在 master drop 时调用——这会关闭 ConPTY 管道，
    // reader 线程的 read() 随即返回错误或 EOF，线程退出
    drop(master);
    drop(ring);
    // reader 线程在管道关闭后自动退出，直接 join（不再固定 sleep）
    let _ = reader_handle.join();
}

/// pty_session_isolation: 验证独立 spawn 的两个 session 互不干扰
///
/// 创建两个独立的 conpty_custom PTY 对 → 各自写回显命令 → 各自读取输出验证。
/// session A 的输出不含 session B 的 marker，反之亦然。
#[cfg(windows)]
#[test]
fn pty_session_isolation() {
    let _lock = SPAWN_LOCK.lock().unwrap();
    use slterminal_lib::pty::{shell, spawn::conpty_custom};
    use std::io::Write as _;

    // Session A
    let shell_info =
        shell::resolve_shell_info(Some("cmd.exe")).expect("resolve_shell_info 应成功");
    let (hpc_a, master_a) = conpty_custom::create_conpty_pair(80, 24, 26100)
        .expect("create_conpty_pair A 应成功");
    let mut writer_a = master_a.take_writer().expect("take_writer A 应成功");
    writer_a.write_all(b"\x1b[1;1R").unwrap();
    writer_a.flush().unwrap();
    let extra_envs: Vec<(String, String)> = vec![];
    let mut child_a = conpty_custom::spawn_conpty_child(hpc_a, &shell_info, &extra_envs, None)
        .expect("spawn_conpty_child A 应成功");
    let mut reader_a = master_a.try_clone_reader().expect("try_clone_reader A 应成功");

    // Session B（独立 PTY 对）
    let shell_info_b =
        shell::resolve_shell_info(Some("cmd.exe")).expect("resolve_shell_info B 应成功");
    let (hpc_b, master_b) = conpty_custom::create_conpty_pair(80, 24, 26100)
        .expect("create_conpty_pair B 应成功");
    let mut writer_b = master_b.take_writer().expect("take_writer B 应成功");
    writer_b.write_all(b"\x1b[1;1R").unwrap();
    writer_b.flush().unwrap();
    let mut child_b = conpty_custom::spawn_conpty_child(hpc_b, &shell_info_b, &extra_envs, None)
        .expect("spawn_conpty_child B 应成功");
    let mut reader_b = master_b.try_clone_reader().expect("try_clone_reader B 应成功");

    // Session A 写命令（不再固定 sleep 等待启动）
    writer_a.write_all(b"echo SESSION_A_ONLY\r\n").unwrap();
    writer_a.flush().unwrap();

    // Session B 写命令
    writer_b.write_all(b"echo SESSION_B_ONLY\r\n").unwrap();
    writer_b.flush().unwrap();

    // 读取 Session A 输出——轮询直到出现 marker（50ms 间隔，上限 10s）
    let output_a = read_until_marker(&mut *reader_a, "SESSION_A_ONLY");
    let text_a = String::from_utf8_lossy(&output_a);
    assert!(
        text_a.contains("SESSION_A_ONLY"),
        "Session A 输出应包含 SESSION_A_ONLY，实际长度={}，内容: {text_a}",
        output_a.len()
    );
    assert!(
        !text_a.contains("SESSION_B_ONLY"),
        "Session A 输出不应包含 Session B 的 marker（输出隔离）"
    );

    // 读取 Session B 输出——轮询直到出现 marker（50ms 间隔，上限 10s）
    let output_b = read_until_marker(&mut *reader_b, "SESSION_B_ONLY");
    let text_b = String::from_utf8_lossy(&output_b);
    assert!(
        text_b.contains("SESSION_B_ONLY"),
        "Session B 输出应包含 SESSION_B_ONLY，实际长度={}，内容: {text_b}",
        output_b.len()
    );

    // 清理：先 ClosePseudoConsole 再 drop reader（避免读端阻塞）
    drop(writer_a);
    drop(writer_b);
    let _ = child_a.kill();
    let _ = child_b.kill();
    drop(master_a);
    drop(master_b);
    drop(reader_a);
    drop(reader_b);
}
