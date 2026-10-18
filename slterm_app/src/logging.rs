//! 日志:stdout + 按需文件双写(照上游 logging.rs 裁剪,legacy 壳臂删除:
//! message bar 事件汇、winit 代理随旧壳消亡)。
//!
//! 主程序在启动链中恰好调用一次 [`initialize`]。console 子系统(D04-1)下
//! 从 shell 启动时 stdout 有效;双击启动无控制台,写失败静默(诊断落日志文件)。

use std::fs::{File, OpenOptions};
use std::io::{self, LineWriter, Stdout, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;
use std::{env, process};

use log::{Level, LevelFilter};

/// 环境变量:日志文件路径(启动失败后用户可从此键读回日志位置)。
const LOG_PATH_ENV: &str = "SLTERM_LOG";

/// 环境变量:额外放行的日志 target,分号分隔。
const EXTRA_LOG_TARGETS_ENV: &str = "SLTERM_EXTRA_LOG_TARGETS";

/// 环境变量:debug_log 的落盘开关(诊断辅助,非生产日志通道)。
#[allow(dead_code)] // 随 debug_log 的调用点(04 篇 runtime 诊断链)接线。
const DEBUG_LOG_ENV: &str = "SLTERM_DEBUG_LOG";

/// 常驻放行 target:自家两 crate + 安全审计通道。gpui/依赖树的 Error/Warn 在
/// max_level=Trace 时放行(见 is_allowed_target),常态不刷屏。
///
/// `audit` = 10 篇 M10.12 SEC-12/13/17 唯一通道(tracing target 经 log 桥接
/// 落到本 logger);取证面不随常态档位静默。
const ALLOWED_TARGETS: &[&str] = &["slterm_terminal", "slterm", "audit"];

/// 诊断辅助:写 data_dir 下的 debug 日志,`SLTERM_DEBUG_LOG` 显式开启才落盘。
/// 给启动期/发布形态下没法走 log crate 的环节用(开关读取有 OnceLock 缓存)。
#[allow(dead_code)] // 调用点(启动期文件/进程诊断)归 03/04 篇接线。
pub(crate) fn debug_log(message: impl AsRef<str>) {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    if !*ENABLED.get_or_init(|| {
        env::var(DEBUG_LOG_ENV).is_ok_and(|value| {
            let value = value.trim();
            !value.is_empty() && value != "0" && !value.eq_ignore_ascii_case("false")
        })
    }) {
        return;
    }

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| format!("{}.{:03}", duration.as_secs(), duration.subsec_millis()))
        .unwrap_or_else(|_| "0.000".to_owned());
    let path = crate::platform::dirs::data_dir().join("slterm_debug.log");
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(
            file,
            "[{timestamp} pid={}] {}",
            process::id(),
            message.as_ref()
        );
    }
}

/// 用户配置的额外放行 target。
fn extra_log_targets() -> &'static [String] {
    static EXTRA_LOG_TARGETS: OnceLock<Vec<String>> = OnceLock::new();

    EXTRA_LOG_TARGETS.get_or_init(|| {
        env::var(EXTRA_LOG_TARGETS_ENV).map_or(Vec::new(), |targets| {
            targets.split(';').map(ToString::to_string).collect()
        })
    })
}

/// 初始化 logger。verbosity 旗标(-v/-q 抬降档位)归 04 篇 CLI 重建后接线;
/// M3 恒定 Warn——无旗标时可调面为空,静默优先于刷屏。
pub fn initialize() -> Result<Option<PathBuf>, log::SetLoggerError> {
    log::set_max_level(LevelFilter::Warn);

    let logger = Logger::new();
    let path = logger.file_path();
    log::set_boxed_logger(Box::new(logger))?;

    Ok(path)
}

pub struct Logger {
    logfile: Mutex<OnDemandLogFile>,
    stdout: Mutex<LineWriter<Stdout>>,
    start: Instant,
}

impl Logger {
    fn new() -> Self {
        Logger {
            logfile: Mutex::new(OnDemandLogFile::new()),
            stdout: Mutex::new(LineWriter::new(io::stdout())),
            start: Instant::now(),
        }
    }

    fn file_path(&self) -> Option<PathBuf> {
        let logfile_lock = self.logfile.lock().ok()?;
        Some(logfile_lock.path().clone())
    }
}

impl log::Log for Logger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.level() <= log::max_level()
    }

    fn log(&self, record: &log::Record<'_>) {
        // 取 target 的 crate 段(首个 ':' 之前)。
        let index = record
            .target()
            .find(':')
            .unwrap_or_else(|| record.target().len());
        let target = &record.target()[..index];

        // 只放自家 crate;Trace 档下外来 crate 的 Error/Warn 也放行。
        if !self.enabled(record.metadata()) || !is_allowed_target(record.level(), target) {
            return;
        }

        let message = create_log_message(record, target, self.start);

        if let Ok(mut logfile) = self.logfile.lock() {
            let _ = logfile.write_all(message.as_ref());
        }

        if let Ok(mut stdout) = self.stdout.lock() {
            let _ = stdout.write_all(message.as_ref());
        }
    }

    fn flush(&self) {}
}

fn create_log_message(record: &log::Record<'_>, target: &str, start: Instant) -> String {
    let runtime = start.elapsed();
    let secs = runtime.as_secs();
    let nanos = runtime.subsec_nanos();
    let mut message = format!(
        "[{}.{:0>9}s] [{:<5}] [{}] ",
        secs,
        nanos,
        record.level(),
        target
    );

    // 多行 payload 的续行对齐;不处理全角字符,len 足够。
    let alignment = message.len();

    let lines = record.args().to_string();
    for line in lines.split('\n') {
        let line = format!("{}\n{:width$}", line, "", width = alignment);
        message.push_str(&line);
    }

    // 去掉尾行多余的填充。
    message.truncate(message.len() - alignment);
    message
}

/// target 是否放行。
fn is_allowed_target(level: Level, target: &str) -> bool {
    match (level, log::max_level()) {
        (Level::Error, LevelFilter::Trace) | (Level::Warn, LevelFilter::Trace) => true,
        _ => ALLOWED_TARGETS.contains(&target) || extra_log_targets().iter().any(|t| t == target),
    }
}

/// 按需创建:进程没写日志就不留空文件;运行中被删则下次写时重建。
struct OnDemandLogFile {
    file: Option<LineWriter<File>>,
    created: Arc<AtomicBool>,
    path: PathBuf,
}

impl OnDemandLogFile {
    fn new() -> Self {
        let mut path = env::temp_dir();
        path.push(format!("slterm-{}.log", process::id()));

        // 路径经环境变量发布,启动失败诊断可读回。
        unsafe {
            env::set_var(LOG_PATH_ENV, path.as_os_str());
        }

        OnDemandLogFile {
            path,
            file: None,
            created: Arc::new(AtomicBool::new(false)),
        }
    }

    fn file(&mut self) -> Result<&mut LineWriter<File>, io::Error> {
        // 运行中被删的文件下次写时重建。
        if self.file.is_some() && !self.path.as_path().exists() {
            self.file = None;
        }

        if self.file.is_none() {
            let file = OpenOptions::new()
                .append(true)
                .create_new(true)
                .open(&self.path);

            match file {
                Ok(file) => {
                    self.file = Some(io::LineWriter::new(file));
                    self.created.store(true, Ordering::Relaxed);
                    let _ = writeln!(
                        io::stdout(),
                        "Created log file at \"{}\"",
                        self.path.display()
                    );
                }
                Err(e) => {
                    let _ = writeln!(io::stdout(), "Unable to create log file: {e}");
                    return Err(e);
                }
            }
        }

        Ok(self.file.as_mut().unwrap())
    }

    fn path(&self) -> &PathBuf {
        &self.path
    }
}

impl Write for OnDemandLogFile {
    fn write(&mut self, buf: &[u8]) -> Result<usize, io::Error> {
        self.file()?.write(buf)
    }

    fn flush(&mut self) -> Result<(), io::Error> {
        self.file()?.flush()
    }
}

#[cfg(test)]
mod logging_tests {
    use super::*;

    #[test]
    fn allowed_targets_cover_own_crates_and_trace_escape() {
        assert!(is_allowed_target(Level::Info, "slterm"));
        assert!(is_allowed_target(Level::Info, "slterm_terminal"));
        // 未知 target 在常态档位下不放行,Trace 档下 Error/Warn 放行。
        log::set_max_level(LevelFilter::Warn);
        assert!(!is_allowed_target(Level::Info, "gpui"));
        assert!(!is_allowed_target(Level::Warn, "gpui"));
        log::set_max_level(LevelFilter::Trace);
        assert!(is_allowed_target(Level::Warn, "gpui"));
        assert!(!is_allowed_target(Level::Info, "gpui"));
        log::set_max_level(LevelFilter::Warn);
    }

    #[test]
    fn log_message_aligns_continuation_lines() {
        let record = log::Record::builder()
            .args(format_args!("first\nsecond"))
            .level(Level::Info)
            .target("slterm")
            .build();
        let message = create_log_message(&record, "slterm", Instant::now());
        let lines: Vec<&str> = message.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].ends_with("first"));
        // 续行缩进到 payload 起点。
        let indent = lines[1].len() - lines[1].trim_start().len();
        assert_eq!(indent, lines[0].find("first").unwrap());
        assert!(lines[1].ends_with("second"));
    }
}
