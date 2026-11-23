//! 短期原生探测的输出与等待上限;仅在后台线程使用。
//!
//! 补全的 Git 发现是 3s/1MB 双预算的协作式子进程(边界 2):本模块给
//! 出可取消的读取原语——stdout 落临时文件避免后代继承管道导致读线程永不
//! 退出,超时/取消/超量任一命中即终止整个作业对象。

use std::io::{self, Read as _, Seek as _};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

/// 有界子进程的结构化结局；调用方不得靠错误字符串猜测失败原因。
#[derive(Debug)]
pub(crate) enum ProcessOutputError {
    Spawn(io::Error),
    Io(io::Error),
    Cancelled,
    Timeout,
    OutputLimit { limit: usize },
    Exit(ExitStatus),
}

impl From<io::Error> for ProcessOutputError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// 有界子进程的双通道真实输出；非零退出由 `status` 表达。
#[derive(Debug)]
pub(crate) struct ProcessOutput {
    pub(crate) status: ExitStatus,
    pub(crate) stdout: Vec<u8>,
    pub(crate) stderr: Vec<u8>,
}

/// 运行命令并分别保留有界 stdout/stderr；调用方决定非零退出的领域语义。
pub(crate) fn run_cancellable(
    mut command: Command,
    timeout: Duration,
    limit: usize,
    cancelled: &dyn Fn() -> bool,
) -> Result<ProcessOutput, ProcessOutputError> {
    if cancelled() {
        return Err(ProcessOutputError::Cancelled);
    }
    let mut stdout = tempfile::tempfile()?;
    let mut stderr = tempfile::tempfile()?;
    command
        .stdin(Stdio::null())
        .stdout(stdout.try_clone()?)
        .stderr(stderr.try_clone()?);
    super::process::configure_process_group(&mut command);
    let mut child = command.spawn().map_err(ProcessOutputError::Spawn)?;
    let group = match super::process::ProcessGroup::attach(&child) {
        Ok(group) => group,
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error.into());
        }
    };
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {}
            Err(error) => {
                terminate(&group, &mut child);
                return Err(error.into());
            }
        }
        if cancelled() {
            terminate(&group, &mut child);
            return Err(ProcessOutputError::Cancelled);
        }
        if Instant::now() >= deadline {
            terminate(&group, &mut child);
            return Err(ProcessOutputError::Timeout);
        }
        let bytes = stdout
            .metadata()
            .and_then(|stdout| stderr.metadata().map(|stderr| stdout.len() + stderr.len()));
        match bytes {
            Ok(bytes) if bytes <= limit as u64 => {}
            Ok(_) => {
                terminate(&group, &mut child);
                return Err(ProcessOutputError::OutputLimit { limit });
            }
            Err(error) => {
                terminate(&group, &mut child);
                return Err(error.into());
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    group.finish();
    if cancelled() {
        return Err(ProcessOutputError::Cancelled);
    }
    stdout.rewind()?;
    stderr.rewind()?;
    let stdout_bytes = read_bounded(stdout, limit)?;
    let stderr_bytes = read_bounded(stderr, limit)?;
    if stdout_bytes.len() + stderr_bytes.len() > limit {
        return Err(ProcessOutputError::OutputLimit { limit });
    }
    Ok(ProcessOutput {
        status,
        stdout: stdout_bytes,
        stderr: stderr_bytes,
    })
}

fn read_bounded(output: std::fs::File, limit: usize) -> Result<Vec<u8>, ProcessOutputError> {
    let mut bytes = Vec::new();
    output.take(limit as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err(ProcessOutputError::OutputLimit { limit });
    }
    Ok(bytes)
}

/// 运行命令并读取至多 `limit` 字节;非零退出仍按旧合同映射为 `Exit`。
pub(crate) fn read_cancellable(
    command: Command,
    timeout: Duration,
    limit: usize,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, ProcessOutputError> {
    let output = run_cancellable(command, timeout, limit, cancelled)?;
    if !output.status.success() {
        return Err(ProcessOutputError::Exit(output.status));
    }
    Ok(output.stdout)
}

fn terminate(group: &super::process::ProcessGroup, child: &mut Child) {
    group.terminate(child);
    let _ = child.wait();
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
        assert!(matches!(
            read_cancellable(
                command("echo too-long-output"),
                Duration::from_secs(2),
                2,
                &|| false
            ),
            Err(ProcessOutputError::OutputLimit { limit: 2 })
        ));
        assert!(matches!(
            read_cancellable(
                command("ping -n 20 127.0.0.1 >nul"),
                Duration::from_millis(50),
                1024,
                &|| false
            ),
            Err(ProcessOutputError::Timeout)
        ));
    }

    #[test]
    fn bounded_process_output_preserves_exit_status_and_stderr() {
        let mut command = Command::new("cmd.exe");
        command.args(["/d", "/c", "echo stdout&echo stderr 1>&2&exit 7"]);

        let output = run_cancellable(command, Duration::from_secs(2), 1024, &|| false).unwrap();

        assert_eq!(output.status.code(), Some(7));
        assert_eq!(output.stdout, b"stdout\r\n");
        assert_eq!(output.stderr, b"stderr \r\n");
    }

    #[test]
    fn bounded_process_output_counts_stderr_toward_the_limit() {
        let mut command = Command::new("cmd.exe");
        command.args(["/d", "/c", "echo too-much-stderr 1>&2"]);

        assert!(matches!(
            run_cancellable(command, Duration::from_secs(2), 4, &|| false),
            Err(ProcessOutputError::OutputLimit { limit: 4 })
        ));
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
        assert!(matches!(error, ProcessOutputError::Cancelled));
        assert!(
            checks.get() > 1,
            "cancellation occurs after process creation"
        );
    }
}
