//! 短期原生探测的输出与等待上限;仅在后台线程使用。
//!
//! 补全的 Git 发现是 3s/1MB 双预算的协作式子进程(08 篇边界 2):本模块给
//! 出可取消的读取原语——stdout 落临时文件避免后代继承管道导致读线程永不
//! 退出,超时/取消/超量任一命中即终止整个作业对象。

use std::io::{self, Read as _, Seek as _};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// 运行命令并读取至多 `limit` 字节;超时、取消、超量、非零退出均为错误。
///
/// 调用方负责整段请求的总预算:多次读取共用同一 `timeout` 由上层换算。
pub(crate) fn read_cancellable(
    mut command: Command,
    timeout: Duration,
    limit: usize,
    cancelled: &dyn Fn() -> bool,
) -> io::Result<Vec<u8>> {
    if cancelled() {
        return Err(io::ErrorKind::Interrupted.into());
    }
    let mut output = tempfile::tempfile()?;
    command
        .stdin(Stdio::null())
        .stdout(output.try_clone()?)
        .stderr(Stdio::null());
    super::process::configure_process_group(&mut command);
    let mut child = command.spawn()?;
    let group = match super::process::ProcessGroup::attach(&child) {
        Ok(group) => group,
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
    };
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None)
                if !cancelled()
                    && Instant::now() < deadline
                    && output
                        .metadata()
                        .is_ok_and(|meta| meta.len() <= limit as u64) =>
            {
                std::thread::sleep(Duration::from_millis(10));
            }
            result => {
                group.terminate(&mut child);
                let _ = child.wait();
                if cancelled() {
                    return Err(io::ErrorKind::Interrupted.into());
                }
                return Err(result.err().unwrap_or_else(|| {
                    io::Error::other("Process probe exceeded its time or output limit")
                }));
            }
        }
    };
    group.finish();
    if cancelled() {
        return Err(io::ErrorKind::Interrupted.into());
    }
    if !status.success() {
        return Err(io::Error::other(format!(
            "Process probe exited with {status}"
        )));
    }
    output.rewind()?;
    let mut bytes = Vec::new();
    output.take(limit as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err(io::ErrorKind::InvalidData.into());
    }
    Ok(bytes)
}

#[cfg(test)]
mod process_output_tests {
    use super::*;

    #[test]
    fn probes_bound_output_and_reap_timed_out_children() {
        let command = |script: &str| {
            let mut command = Command::new("cmd.exe");
            command.args(["/d", "/c", script]);
            command
        };
        assert_eq!(
            read_cancellable(command("echo ok"), Duration::from_secs(2), 8, &|| false).unwrap(),
            b"ok\r\n"
        );
        assert!(
            read_cancellable(
                command("echo too-long-output"),
                Duration::from_secs(2),
                2,
                &|| false
            )
            .is_err()
        );
        assert!(
            read_cancellable(
                command("ping -n 20 127.0.0.1 >nul"),
                Duration::from_millis(50),
                1024,
                &|| false
            )
            .is_err()
        );
    }

    #[test]
    fn cancelled_probe_reaps_its_child_and_reports_interruption() {
        let mut command = Command::new("cmd.exe");
        command.args(["/d", "/c", "ping -n 20 127.0.0.1 >nul"]);
        let checks = std::cell::Cell::new(0);
        let error = read_cancellable(command, Duration::from_secs(5), 1024, &|| {
            checks.set(checks.get() + 1);
            checks.get() > 1
        })
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::Interrupted);
        assert!(
            checks.get() > 1,
            "cancellation occurs after process creation"
        );
    }
}
