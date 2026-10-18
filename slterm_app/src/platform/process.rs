//! 非交互子进程适配:一次性探针与宿主控制台隔离。
//!
//! slterm 是 console 子系统(D04-1),子进程默认继承宿主控制台——codex
//! 能力探针这类一次性进程不该继承:继承会共享控制台的 Ctrl 事件路由与
//! 输出归属(探针的 stderr 直写宿主屏幕)。`CREATE_NO_WINDOW` 让探针无
//! 控制台运行;stdio 全部由调用方重定向,无交互需求。
//!
//! 用法:构造完参数、`spawn()` 之前过一道。
//!
//! **不要**给需要与用户交互或必须附着控制台会话的子进程加这个。

use std::process::Command;

/// `CREATE_NO_WINDOW` 的唯一定义处——别再写 `0x0800_0000`。
pub(crate) const CREATE_NO_WINDOW: u32 = windows_sys::Win32::System::Threading::CREATE_NO_WINDOW;

/// 让子进程不带控制台运行。
pub(crate) fn hidden_command(command: &mut Command) -> &mut Command {
    use std::os::windows::process::CommandExt as _;
    command.creation_flags(CREATE_NO_WINDOW)
}

#[cfg(test)]
mod process_tests {
    use super::*;
    use std::io::Write as _;
    use std::process::Stdio;

    /// 真机锚点:探针形态(无控制台 + stdio 全管道)真实 spawn 一轮,
    /// 子进程确认自己拿不到控制台窗口。
    #[test]
    fn hidden_command_child_runs_without_a_console() {
        let script = r#"
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public class ConsoleProbe { [DllImport("kernel32.dll")] public static extern IntPtr GetConsoleWindow(); }
'@
if ([ConsoleProbe]::GetConsoleWindow() -ne [IntPtr]::Zero) { exit 91 }
[Console]::Out.Write([Console]::In.ReadLine())
exit 7
"#;
        let system = std::env::var_os("SystemRoot").expect("Windows system directory");
        let powershell =
            std::path::Path::new(&system).join("System32/WindowsPowerShell/v1.0/powershell.exe");
        let mut command = Command::new(&powershell);
        command
            .args([
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                script,
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        hidden_command(&mut command);
        let mut child = command.spawn().expect("start console probe");
        child
            .stdin
            .take()
            .unwrap()
            .write_all(b"probe-stdin\n")
            .unwrap();
        let output = child.wait_with_output().expect("wait for console probe");
        assert_eq!(output.status.code(), Some(7), "{output:?}");
        assert_eq!(output.stdout, b"probe-stdin");
    }
}
