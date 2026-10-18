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

/// 一次性子进程的进程组/作业对象配置:超时回收整棵树时连后代一起收。
///
/// `pane.exec` 的 stdin 是 null、stdout/stderr 走管道,从头到尾没有交互,
/// 因此同样挂 `CREATE_NO_WINDOW`。
#[cfg(windows)]
#[allow(dead_code)] // 消费点 = runtime_exec::build_command(pane.exec,M7.5)。
pub(crate) fn configure_process_group(command: &mut Command) {
    hidden_command(command);
}

#[cfg(not(windows))]
#[allow(dead_code)] // 消费点 = runtime_exec::build_command(pane.exec,M7.5)。
pub(crate) fn configure_process_group(_: &mut Command) {}

/// 一次性子进程的作业对象句柄(Drop 关闭句柄 = 内核回收整棵树)。
#[allow(dead_code)] // 消费点 = runtime_exec::execute 的超时整树回收(M7.5)。
pub(crate) struct ProcessGroup {
    #[cfg(windows)]
    job: windows_sys::Win32::Foundation::HANDLE,
}

#[allow(dead_code)] // 消费点 = runtime_exec::execute(M7.5 接线)。
impl ProcessGroup {
    pub(crate) fn attach(child: &std::process::Child) -> std::io::Result<Self> {
        #[cfg(windows)]
        {
            use std::mem::{size_of, zeroed};
            use std::os::windows::io::AsRawHandle as _;
            use std::ptr;
            use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
            use windows_sys::Win32::System::JobObjects::{
                AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
                JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
                SetInformationJobObject,
            };

            // SAFETY: 指针均指向调用期内有效的 POD 值;返回的 job 句柄由
            // ProcessGroup 独占并在 Drop 时关闭一次。
            unsafe {
                let job = CreateJobObjectW(ptr::null(), ptr::null());
                if job.is_null() {
                    return Err(std::io::Error::last_os_error());
                }
                let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = zeroed();
                limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                if SetInformationJobObject(
                    job,
                    JobObjectExtendedLimitInformation,
                    (&raw const limits).cast(),
                    size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                ) == 0
                {
                    let error = std::io::Error::last_os_error();
                    CloseHandle(job);
                    return Err(error);
                }
                if AssignProcessToJobObject(job, child.as_raw_handle() as HANDLE) == 0 {
                    let error = std::io::Error::last_os_error();
                    CloseHandle(job);
                    return Err(error);
                }
                Ok(Self { job })
            }
        }
        #[cfg(not(windows))]
        {
            let _ = child;
            Ok(Self {})
        }
    }

    /// 终止作业对象内的全部进程(先整树,再兜底单进程)。
    pub(crate) fn terminate(&self, child: &mut std::process::Child) {
        #[cfg(windows)]
        unsafe {
            use windows_sys::Win32::System::JobObjects::TerminateJobObject;
            let _ = TerminateJobObject(self.job, 1);
        }
        let _ = child.kill();
    }

    /// 直接子进程已退出后,收掉仍持有 pipe 句柄的后台后代并等 EOF。
    /// 句柄关闭由 `Drop` 统一负责,这里只终止作业对象。
    pub(crate) fn finish(self) {
        #[cfg(windows)]
        unsafe {
            use windows_sys::Win32::System::JobObjects::TerminateJobObject;
            let _ = TerminateJobObject(self.job, 1);
        }
    }
}

impl Drop for ProcessGroup {
    fn drop(&mut self) {
        #[cfg(windows)]
        unsafe {
            // SAFETY: `job` 由本守卫独占持有,仅在 Drop 时关闭一次。
            windows_sys::Win32::Foundation::CloseHandle(self.job);
        }
    }
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
