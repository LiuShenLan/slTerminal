//! 非 TTY 的 pane 进程执行(平台件只消费)。
//!
//! `pane.run` 仍把命令送进现有交互 shell;这里刻意走独立 child:stdout /
//! stderr 可分开捕获,也不改变 shell history、cwd 或环境。公开参数是 argv
//! 而不是命令行字符串,因此不存在一层隐式 shell 展开。
//!
//! 环境语义 = 「继承宿主进程 + PTY 创建时冻结的 overrides」;运行中 shell
//! 里的 `export` / `$env:` 不会反向流回父进程,因此不假装能继承。

// 消费点 = M7.5 壳事件泵的 pane.exec 执行器(`RuntimeControl` 臂调用 spawn)。
#![allow(dead_code)]

use std::collections::HashMap;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::{Value, json};

use crate::platform::process::{ProcessGroup, configure_process_group};
use crate::runtime_api::{ApiError, RuntimeCommand, RuntimeDispatch};

/// 创建 pane 时冻结的、可由独立 child 安全复用的上下文(仅 Host 形态;
/// WSL 臂随不采纳点整支不迁)。
#[derive(Clone, Debug)]
pub(crate) struct PaneExecContext {
    shell_program: Option<String>,
    env: HashMap<String, String>,
    fallback_cwd: Option<PathBuf>,
}

impl PaneExecContext {
    /// Git 子进程护栏:保留宿主 SSH/credential 配置,但不弹出凭据交互。
    pub(crate) fn for_git(mut self) -> Self {
        let vars = [
            ("GIT_TERMINAL_PROMPT", "0"),
            ("GCM_INTERACTIVE", "never"),
            ("SSH_ASKPASS_REQUIRE", "never"),
        ];
        self.env
            .extend(vars.map(|(key, value)| (key.into(), value.into())));
        self
    }

    pub(crate) fn shell_program(&self) -> Option<&str> {
        self.shell_program.as_deref()
    }

    /// 创建 pane 时冻结的环境覆盖;非 TTY 子进程(补全 Git 读取)复用同一份,
    /// 保证代理/凭据等设置与 pane 内 shell 一致。
    pub(crate) fn environment(&self) -> &HashMap<String, String> {
        &self.env
    }

    pub(crate) fn process_instance(&self) -> Option<&str> {
        self.env
            .get(crate::agent_env::PROCESS_ENV)
            .map(String::as_str)
            .filter(|value| !value.is_empty())
    }

    pub(crate) fn from_pty_options(options: &slterm_terminal::tty::Options) -> Self {
        Self {
            shell_program: options
                .shell
                .as_ref()
                .map(|shell| shell.program().to_owned()),
            env: options.env.clone(),
            fallback_cwd: options
                .working_directory
                .clone()
                .or_else(|| std::env::current_dir().ok()),
        }
    }

    #[cfg(test)]
    fn host(cwd: PathBuf) -> Self {
        Self {
            shell_program: None,
            env: HashMap::new(),
            fallback_cwd: Some(cwd),
        }
    }
}

#[derive(Debug)]
struct CapturedBytes {
    bytes: Vec<u8>,
    total_bytes: u64,
}

#[derive(Debug, Serialize)]
struct CapturedText {
    text: String,
    encoding: &'static str,
    total_bytes: u64,
    captured_bytes: usize,
    truncated: bool,
}

impl CapturedBytes {
    fn into_text(self) -> CapturedText {
        let captured_bytes = self.bytes.len();
        let truncated = self.total_bytes > captured_bytes as u64;
        match String::from_utf8(self.bytes) {
            Ok(text) => CapturedText {
                text,
                encoding: "utf-8",
                total_bytes: self.total_bytes,
                captured_bytes,
                truncated,
            },
            Err(error) => CapturedText {
                text: String::from_utf8_lossy(error.as_bytes()).into_owned(),
                encoding: "utf-8-lossy",
                total_bytes: self.total_bytes,
                captured_bytes,
                truncated,
            },
        }
    }
}

/// 读满上限后继续排水但不留字节:保留 `total_bytes` 才能如实报告截断。
fn read_bounded(mut reader: impl Read, limit: usize) -> io::Result<CapturedBytes> {
    let mut kept = Vec::with_capacity(limit.min(64 * 1024));
    let mut total_bytes = 0_u64;
    let mut buffer = [0_u8; 16 * 1024];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        total_bytes = total_bytes.saturating_add(read as u64);
        let remaining = limit.saturating_sub(kept.len());
        kept.extend_from_slice(&buffer[..read.min(remaining)]);
    }
    Ok(CapturedBytes {
        bytes: kept,
        total_bytes,
    })
}

fn capture_thread(
    name: &'static str,
    reader: impl Read + Send + 'static,
    limit: usize,
) -> io::Result<std::thread::JoinHandle<io::Result<CapturedBytes>>> {
    std::thread::Builder::new()
        .name(name.to_owned())
        .spawn(move || read_bounded(reader, limit))
}

fn host_cwd(reported: &str, fallback: Option<&Path>) -> Result<PathBuf, ApiError> {
    if !reported.trim().is_empty() {
        let path = PathBuf::from(reported);
        if path.is_dir() {
            return Ok(path);
        }
        return Err(ApiError::new(
            "exec_cwd_unavailable",
            "the pane's reported working directory is not a local filesystem directory",
        )
        .details(json!({ "cwd": reported })));
    }
    fallback
        .filter(|path| path.is_dir())
        .map(Path::to_path_buf)
        .ok_or_else(|| {
            ApiError::new(
                "exec_cwd_unavailable",
                "the pane has not reported a usable local working directory",
            )
        })
}

pub(crate) fn build_command(
    context: &PaneExecContext,
    reported_cwd: &str,
    argv: &[String],
) -> Result<(Command, Value), ApiError> {
    let program = argv.first().expect("validated pane.exec argv");
    let cwd = host_cwd(reported_cwd, context.fallback_cwd.as_deref())?;
    let mut command = Command::new(program);
    command.args(&argv[1..]).current_dir(&cwd);
    command
        .envs(&context.env)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_process_group(&mut command);
    let execution = json!({ "environment": "host", "cwd": cwd });
    Ok((command, execution))
}

/// UI owner 收到 `pane.exec` 后调用:在独立线程执行并把结果 respond 回去。
pub(crate) fn spawn(dispatch: Arc<RuntimeDispatch>, context: PaneExecContext, cwd: String) {
    let worker_dispatch = dispatch.clone();
    let result = std::thread::Builder::new()
        .name("slterm-pane-exec".to_owned())
        .spawn(move || {
            let result = match &worker_dispatch.command {
                RuntimeCommand::Exec {
                    argv,
                    timeout_ms,
                    max_output_bytes,
                    ..
                } => execute(context, cwd, argv.clone(), *timeout_ms, *max_output_bytes),
                _ => Err(ApiError::new(
                    "invalid_runtime_command",
                    "command does not own a process worker",
                )),
            };
            worker_dispatch.respond(result);
        });
    if let Err(error) = result {
        dispatch.respond(Err(ApiError::new(
            "exec_spawn_failed",
            format!("failed to start the pane.exec worker: {error}"),
        )));
    }
}

/// 执行一次非 TTY 子进程:双流并行排水,超时回收整棵树并保留已捕获输出。
pub(crate) fn execute(
    context: PaneExecContext,
    cwd: String,
    argv: Vec<String>,
    timeout_ms: u64,
    max_output_bytes: usize,
) -> Result<Value, ApiError> {
    let started = Instant::now();
    let (mut command, execution) = build_command(&context, &cwd, &argv)?;
    let mut child = command.spawn().map_err(|error| {
        ApiError::new(
            "exec_spawn_failed",
            format!("failed to start {:?}: {error}", argv[0]),
        )
        .details(json!({ "argv": argv, "execution": execution }))
    })?;
    let group = ProcessGroup::attach(&child).map_err(|error| {
        let _ = child.kill();
        let _ = child.wait();
        ApiError::new(
            "exec_spawn_failed",
            format!("failed to isolate the pane.exec process: {error}"),
        )
    })?;
    let stdout = child.stdout.take().ok_or_else(|| {
        ApiError::new(
            "exec_capture_failed",
            "pane.exec child did not expose stdout",
        )
    })?;
    let stderr = child.stderr.take().ok_or_else(|| {
        ApiError::new(
            "exec_capture_failed",
            "pane.exec child did not expose stderr",
        )
    })?;
    // 两条 pipe 必须并行排水;顺序 read_to_end 会在另一条 pipe 填满时死锁。
    let stdout = capture_thread("slterm-pane-exec-stdout", stdout, max_output_bytes)
        .map_err(|error| ApiError::new("exec_capture_failed", error.to_string()))?;
    let stderr = capture_thread("slterm-pane-exec-stderr", stderr, max_output_bytes)
        .map_err(|error| ApiError::new("exec_capture_failed", error.to_string()))?;

    let timeout = Duration::from_millis(timeout_ms);
    let (status, timed_out) = loop {
        match child.try_wait() {
            Ok(Some(status)) => break (status, false),
            Ok(None) if started.elapsed() < timeout => {
                std::thread::sleep(Duration::from_millis(10));
            }
            Ok(None) => {
                group.terminate(&mut child);
                let status = child.wait().map_err(|error| {
                    ApiError::new(
                        "exec_wait_failed",
                        format!("failed to reap timed-out pane.exec child: {error}"),
                    )
                })?;
                break (status, true);
            }
            Err(error) => {
                group.terminate(&mut child);
                let _ = child.wait();
                return Err(ApiError::new(
                    "exec_wait_failed",
                    format!("failed while waiting for pane.exec child: {error}"),
                ));
            }
        }
    };
    // 直接进程退出后仍存活的后台后代也属于这次一次性 exec;先收掉它们,
    // stdout/stderr reader 才不会因继承的 pipe handle 永久等不到 EOF。
    group.finish();

    let stdout = join_capture(stdout, "stdout")?.into_text();
    let stderr = join_capture(stderr, "stderr")?.into_text();
    let capture = json!({
        "stdout": {
            "encoding": stdout.encoding,
            "total_bytes": stdout.total_bytes,
            "captured_bytes": stdout.captured_bytes,
            "truncated": stdout.truncated,
        },
        "stderr": {
            "encoding": stderr.encoding,
            "total_bytes": stderr.total_bytes,
            "captured_bytes": stderr.captured_bytes,
            "truncated": stderr.truncated,
        }
    });
    Ok(json!({
        "argv": argv,
        "execution": execution,
        "exit_code": status.code(),
        "success": status.success() && !timed_out,
        "timed_out": timed_out,
        "duration_ms": started.elapsed().as_millis() as u64,
        "stdout": stdout.text,
        "stderr": stderr.text,
        "capture": capture,
        "stdout_is_tty": false,
        "stderr_is_tty": false
    }))
}

fn join_capture(
    handle: std::thread::JoinHandle<io::Result<CapturedBytes>>,
    stream: &str,
) -> Result<CapturedBytes, ApiError> {
    handle
        .join()
        .map_err(|_| ApiError::new("exec_capture_failed", format!("{stream} reader panicked")))?
        .map_err(|error| {
            ApiError::new(
                "exec_capture_failed",
                format!("failed to read pane.exec {stream}: {error}"),
            )
        })
}

#[cfg(test)]
mod runtime_exec_tests {
    use super::*;

    #[test]
    fn bounded_reader_drains_but_keeps_only_the_limit() {
        let captured = read_bounded(std::io::Cursor::new(b"abcdefghij"), 4).expect("read");
        assert_eq!(captured.bytes, b"abcd");
        assert_eq!(captured.total_bytes, 10);
        let text = captured.into_text();
        assert!(text.truncated);
        assert_eq!(text.captured_bytes, 4);
    }

    #[test]
    fn independent_child_keeps_stdout_and_stderr_separate() {
        let cwd = std::env::current_dir().expect("cwd");
        #[cfg(windows)]
        let argv = vec![
            "cmd.exe".to_owned(),
            "/D".to_owned(),
            "/S".to_owned(),
            "/C".to_owned(),
            "(echo out)&(echo err 1>&2)&exit /b 7".to_owned(),
        ];
        #[cfg(not(windows))]
        let argv = vec![
            "sh".to_owned(),
            "-c".to_owned(),
            "printf out; printf err >&2; exit 7".to_owned(),
        ];

        let result = execute(PaneExecContext::host(cwd), String::new(), argv, 5_000, 1024)
            .expect("child executes");
        assert_eq!(result["exit_code"], 7);
        assert!(result["stdout"].as_str().expect("stdout").contains("out"));
        assert!(result["stderr"].as_str().expect("stderr").contains("err"));
        assert_eq!(result["capture"]["stdout"]["encoding"], "utf-8");
        assert_eq!(result["stdout_is_tty"], false);
        assert_eq!(result["timed_out"], false);
    }

    #[test]
    fn timeout_kills_the_process_and_preserves_captured_output() {
        let cwd = std::env::current_dir().expect("cwd");
        #[cfg(windows)]
        let argv = vec![
            "cmd.exe".to_owned(),
            "/D".to_owned(),
            "/S".to_owned(),
            "/C".to_owned(),
            "(echo before-timeout)&ping -n 10 127.0.0.1 >NUL".to_owned(),
        ];
        #[cfg(not(windows))]
        let argv = vec![
            "sh".to_owned(),
            "-c".to_owned(),
            "printf before-timeout; sleep 10".to_owned(),
        ];
        let result = execute(PaneExecContext::host(cwd), String::new(), argv, 300, 1024)
            .expect("timed out child");
        assert_eq!(result["timed_out"], true);
        assert_eq!(result["success"], false);
        assert!(
            result["stdout"]
                .as_str()
                .expect("stdout")
                .contains("before-timeout")
        );
    }
}
