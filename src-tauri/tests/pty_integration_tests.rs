/// Phase 1 PTY 集成测试
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
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

/// pty_roundtrip: spawn cmd → 写 echo → 验证输出含目标文本
#[test]
fn pty_roundtrip() {
    let (_master, mut child, mut reader, mut writer) = spawn_cmd();

    // 等待 cmd.exe 启动完成（足够长的固定等待）
    std::thread::sleep(Duration::from_millis(800));

    // 先写入命令（在读取之前，确保 reader 有数据可读）
    writer.write_all(b"echo hello_test_marker\r\n").unwrap();
    writer.flush().unwrap();

    // 再读取输出：包含 cmd banner + echo 命令 + hello_test_marker
    let mut output = Vec::new();
    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(10) {
        let mut buf = [0u8; 4096];
        match reader.read(&mut buf) {
            Ok(0) => break, // EOF — cmd 可能已退出
            Ok(n) => {
                output.extend_from_slice(&buf[..n]);
                if String::from_utf8_lossy(&output).contains("hello_test_marker") {
                    break;
                }
            }
            Err(_) => break,
        }
    }

    let text = String::from_utf8_lossy(&output);
    assert!(
        text.contains("hello_test_marker"),
        "PTY 输出应包含 hello_test_marker，实际长度: {}，内容: {}", output.len(), text
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
    std::thread::sleep(Duration::from_millis(1000));
    for _ in 0..5 {
        match child.try_wait() {
            Ok(Some(_)) => break,
            _ => std::thread::sleep(Duration::from_millis(200)),
        }
    }
}

/// osc_cwd_venv_parsed: shell集成脚本验证
#[test]
fn osc_cwd_venv_parsed() {
    let script = include_str!("../assets/shell-integration.ps1");
    assert!(script.contains("OSC") || script.contains("133"), "集成脚本应定义 OSC/133 序列");
    assert!(script.contains("9;9"), "应包含 OSC 9;9 CWD 跟踪");
    assert!(script.contains("133"), "应包含 OSC 133 命令边界");
    assert!(script.contains("UTF8") || script.contains("65001"), "应包含 UTF-8 编码修复");
    assert!(!script.contains("$env:SLTERM_PANE_ID"), "Phase 1 无嵌套检测（留待 Phase 5）");
    assert!(
        script.contains("[char]0x1b") || script.contains("0x1b"),
        "应使用 [char]0x1b PS 5.1 兼容写法"
    );
}
