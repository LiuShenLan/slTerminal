//! 安全审计通道。
//!
//! 通道唯一形态 = tracing target [`TARGET`](`"audit"`)。生产不装 tracing
//! subscriber——tracing 的 `log-always` feature 无条件把事件桥接进既有进程
//! 日志(单日志栈,不重复初始化;审计不因任何 subscriber 装配而丢事件);
//! 测试经作用域 subscriber 捕获断言。
//!
//! 未来若装配全局 subscriber,必须移除 `log-always` 或让 subscriber
//! 过滤 audit target——否则同一事件双落点(日志 + subscriber)。
//!
//! 写设置文件的路径必须经 [`write_settings`](app 设置)与
//! [`write_user_settings`](user 层 `~/.claude/settings.json`)包装。审计记录
//! operation/path/source/result,不记内容值(防凭据进日志);包装是唯一
//! 写路径——写核心函数私有,绕过点另由源码扫描用例锁死。
//!
//! hook 文件归属对账不一致时经 [`hook_hash_mismatch`] 落期望/实测
//! SHA-256 指纹(只补缺失、不覆盖的用户配置纪律保持不变)。
//!
//! 红线:事件字段只含 operation/path/source/result 与 expected/actual
//! 指纹;不携带任意消息、底层错误、命令、文件内容或凭据值。

use std::io;
use std::path::Path;

/// 审计通道 target(唯一形态;logging 恒放行),只由本模块持有。
const TARGET: &str = "audit";

/// 审计结果是封闭词汇,调用方不得拼接动态文本。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AuditResult {
    Ok,
    Error,
    Suspended,
    Acknowledged,
    Mismatch,
}

impl AuditResult {
    fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Error => "error",
            Self::Suspended => "suspended",
            Self::Acknowledged => "acknowledged",
            Self::Mismatch => "mismatch",
        }
    }
}

/// 固定字段审计事件;私有字段与成对指纹构造阻止调用方扩展事件形态。
pub(crate) struct AuditEvent<'a> {
    operation: &'static str,
    path: Option<&'a Path>,
    source: &'static str,
    result: AuditResult,
    expected: Option<&'a str>,
    actual: Option<&'a str>,
}

impl<'a> AuditEvent<'a> {
    pub(crate) fn new(operation: &'static str, source: &'static str, result: AuditResult) -> Self {
        Self {
            operation,
            path: None,
            source,
            result,
            expected: None,
            actual: None,
        }
    }

    #[must_use]
    pub(crate) fn with_path(mut self, path: &'a Path) -> Self {
        self.path = Some(path);
        self
    }

    #[must_use]
    pub(crate) fn with_fingerprints(mut self, expected: &'a str, actual: &'a str) -> Self {
        self.expected = Some(expected);
        self.actual = Some(actual);
        self
    }
}

/// 审计唯一发点;无自由消息字段,底层错误不得作为参数传入。
pub(crate) fn record(event: AuditEvent<'_>) {
    let path = event.path.map(|path| path.display().to_string());
    match event.result {
        AuditResult::Ok | AuditResult::Acknowledged => tracing::info!(
            target: TARGET,
            operation = event.operation,
            path = path,
            source = event.source,
            result = event.result.as_str(),
            expected = event.expected,
            actual = event.actual,
        ),
        AuditResult::Error | AuditResult::Suspended | AuditResult::Mismatch => tracing::warn!(
            target: TARGET,
            operation = event.operation,
            path = path,
            source = event.source,
            result = event.result.as_str(),
            expected = event.expected,
            actual = event.actual,
        ),
    }
}

/// app 设置写通道的操作类别。
const OP_SETTINGS_WRITE: &str = "settings_write";
/// user 层 settings.json 写入的操作类别。
const OP_USER_SETTINGS_WRITE: &str = "user_settings_write";
/// 归属对账不一致的操作类别。
const OP_HOOK_HASH_MISMATCH: &str = "hook_hash_mismatch";

/// app 设置文件写入:审计 + 原子写。
///
/// 生产写通道 = `settings_store`;事件 source 标明具体写路径。
pub(crate) fn write_settings(path: &Path, content: &str, source: &'static str) -> io::Result<()> {
    write_audited(OP_SETTINGS_WRITE, path, content.as_bytes(), source)
}

/// user 层 settings.json 写入:hooks 安装器唯一写路径。
pub(crate) fn write_user_settings(
    path: &Path,
    content: &str,
    source: &'static str,
) -> io::Result<()> {
    write_audited(OP_USER_SETTINGS_WRITE, path, content.as_bytes(), source)
}

/// 私有写核心:审计包装之外的设置写调用点由源码扫描用例拒绝。
fn write_audited(
    operation: &'static str,
    path: &Path,
    bytes: &[u8],
    source: &'static str,
) -> io::Result<()> {
    let result = crate::atomic_file::write(path, bytes);
    let audit_result = if result.is_ok() {
        AuditResult::Ok
    } else {
        AuditResult::Error
    };
    record(AuditEvent::new(operation, source, audit_result).with_path(path));
    result
}

/// 磁盘 hook 文件指纹与内嵌模板不一致(被编辑/被替换)。
///
/// `expected`/`actual` 为 SHA-256 hex 指纹,不是文件内容——审计只留身份。
pub(crate) fn hook_hash_mismatch(path: &Path, expected: &str, actual: &str) {
    record(
        AuditEvent::new(
            OP_HOOK_HASH_MISMATCH,
            "ai_hook::local::hash_reconciliation",
            AuditResult::Mismatch,
        )
        .with_path(path)
        .with_fingerprints(expected, actual),
    );
}

#[cfg(test)]
pub(crate) mod test_support {
    //! 作用域 tracing 捕获:只影响当前线程,不触碰进程级 logger 槽位。

    use std::io;
    use std::sync::{Arc, Mutex};

    use tracing_subscriber::fmt::MakeWriter;

    /// 捕获句柄:Drop 恢复默认(空)subscriber。
    pub(crate) struct CapturedEvents {
        _guard: tracing::subscriber::DefaultGuard,
        buffer: Arc<Mutex<Vec<u8>>>,
    }

    impl CapturedEvents {
        /// 已格式化的全部事件文本(字段以 `key=value` 形态出现)。
        pub(crate) fn text(&self) -> String {
            String::from_utf8_lossy(&self.buffer.lock().expect("事件缓冲锁")).into_owned()
        }
    }

    #[derive(Clone, Default)]
    struct CaptureBuffer(Arc<Mutex<Vec<u8>>>);

    impl io::Write for CaptureBuffer {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.lock().expect("事件缓冲锁").extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl<'writer> MakeWriter<'writer> for CaptureBuffer {
        type Writer = Self;

        fn make_writer(&'writer self) -> Self::Writer {
            self.clone()
        }
    }

    /// 安装线程级捕获订阅器;返回句柄存活期间的事件文本。
    pub(crate) fn capture_events() -> CapturedEvents {
        let buffer = Arc::new(Mutex::new(Vec::new()));
        let subscriber = tracing_subscriber::fmt()
            .without_time()
            .with_ansi(false)
            .with_writer(CaptureBuffer(buffer.clone()))
            .finish();
        let guard = tracing::subscriber::set_default(subscriber);
        CapturedEvents {
            _guard: guard,
            buffer,
        }
    }
}

#[cfg(test)]
mod audit_tests {
    use std::path::{Path, PathBuf};

    use super::*;

    /// 字段断言容忍 tracing fmt 的引号形态(`key=value` / `key="value"`)。
    fn assert_field(text: &str, key: &str, value: &str) {
        let plain = format!("{key}={value}");
        let quoted = format!("{key}=\"{value}\"");
        assert!(
            text.contains(&plain) || text.contains(&quoted),
            "审计事件缺少字段 {key}={value}: {text}"
        );
    }

    fn collect_rust_sources(root: &Path, files: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(root).expect("读取源码目录") {
            let path = entry.expect("源码目录项").path();
            if path.is_dir() {
                collect_rust_sources(&path, files);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                files.push(path);
            }
        }
    }

    /// 取函数体文本(大括号配对);用于「唯一写路径」源码守卫。
    fn function_body<'a>(source: &'a str, signature: &str) -> &'a str {
        let start = source
            .find(signature)
            .unwrap_or_else(|| panic!("源码中缺少函数签名: {signature}"));
        let open = source[start..]
            .find('{')
            .map(|offset| start + offset)
            .expect("函数体必须存在");
        let mut depth = 0usize;
        for (index, character) in source[open..].char_indices() {
            match character {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        return &source[open..=open + index];
                    }
                }
                _ => {}
            }
        }
        panic!("函数体未闭合: {signature}");
    }

    /// 写入触发 audit 事件,字段 = operation/path/source/result。
    #[test]
    fn user_settings_write_emits_audit_event_with_path_source_and_result() {
        let captured = test_support::capture_events();
        let directory = tempfile::tempdir().expect("创建临时目录");
        let path = directory.path().join(".claude").join("settings.json");
        write_user_settings(&path, r#"{"hooks":{}}"#, "ai_hook::test").expect("审计包装写入");

        let text = captured.text();
        assert!(text.contains("audit"), "事件必须落在 audit target: {text}");
        assert_field(&text, "operation", "user_settings_write");
        assert_field(&text, "source", "ai_hook::test");
        assert_field(&text, "result", "ok");
        assert!(text.contains("settings.json"), "事件必须含目标路径: {text}");
        assert_eq!(
            std::fs::read_to_string(&path).expect("读取写入结果"),
            r#"{"hooks":{}}"#
        );
    }

    /// app 设置写通道走同一事件族,operation 区分。
    #[test]
    fn settings_write_emits_audit_event_with_its_own_operation() {
        let captured = test_support::capture_events();
        let directory = tempfile::tempdir().expect("创建临时目录");
        let path = directory.path().join("settings.json");
        write_settings(&path, "{}", "settings_store::test").expect("审计包装写入");

        let text = captured.text();
        assert_field(&text, "operation", "settings_write");
        assert_field(&text, "source", "settings_store::test");
        assert_field(&text, "result", "ok");
        assert!(text.contains("INFO"), "成功写必须降为 info: {text}");
    }

    /// 失败路径同样审计(result=error),错误照常上抛。
    #[test]
    fn settings_write_failure_is_audited_and_propagated() {
        let captured = test_support::capture_events();
        let directory = tempfile::tempdir().expect("创建临时目录");
        // 父路径是文件 → create_dir_all 失败。
        let blocker = directory.path().join("blocker");
        std::fs::write(&blocker, b"x").expect("写阻塞文件");
        let path = blocker.join("settings.json");

        assert!(write_settings(&path, "{}", "settings_store::test").is_err());
        let text = captured.text();
        assert_field(&text, "result", "error");
        assert_field(&text, "operation", "settings_write");
        assert!(text.contains("WARN"), "失败写必须保持 warn: {text}");
        assert!(!text.contains("error="), "审计不得插值底层错误文本: {text}");
    }

    /// 审计事件只含路径/字段身份,绝不携带写入内容(可能含凭据)。
    #[test]
    fn audit_events_never_embed_the_written_content() {
        let captured = test_support::capture_events();
        let directory = tempfile::tempdir().expect("创建临时目录");
        let path = directory.path().join("settings.json");
        const SECRET: &str = "sk-test-audit-content-91c2";
        let content = format!(r#"{{"env":{{"ANTHROPIC_AUTH_TOKEN":"{SECRET}"}}}}"#);

        write_settings(&path, &content, "settings_store::test").expect("写入");
        let text = captured.text();
        assert!(!text.contains(SECRET), "审计事件泄露文件内容: {text}");
        assert!(text.contains("settings.json"));
    }

    /// hook 哈希对账事件携带期望/实测指纹,不含文件内容。
    #[test]
    fn hook_hash_mismatch_event_carries_expected_and_actual_fingerprints() {
        let captured = test_support::capture_events();
        let path = Path::new("C:/Users/test/.config/opencode/plugins/slterm.js");
        hook_hash_mismatch(path, "expected-fingerprint", "actual-fingerprint");

        let text = captured.text();
        assert_field(&text, "operation", "hook_hash_mismatch");
        assert_field(&text, "result", "mismatch");
        assert_field(&text, "expected", "expected-fingerprint");
        assert_field(&text, "actual", "actual-fingerprint");
        assert!(text.contains("slterm.js"), "事件必须含路径: {text}");
    }

    /// audit target 只由 facade 持有，业务模块不得自行组装事件字段。
    #[test]
    fn audit_target_is_private_to_the_facade() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files = Vec::new();
        collect_rust_sources(&root, &mut files);
        let mut offenders = Vec::new();
        for path in files {
            if path == root.join("audit.rs") {
                continue;
            }
            let source = std::fs::read_to_string(&path).expect("读取源码");
            let compact: String = source.split_whitespace().collect();
            // 禁词拆分书写，避免未来把本测试迁出 facade 后自命中。
            if compact.contains(concat!("audit::", "TARGET"))
                || compact.contains(concat!("target:\"", "audit\""))
            {
                offenders.push(path);
            }
        }
        assert!(
            offenders.is_empty(),
            "audit target 必须只经 facade 发出: {offenders:?}"
        );
    }

    /// 唯一写路径:设置写核心只见于审计包装,生产调用点不直写。
    #[test]
    fn settings_writes_are_centralized_in_the_audit_wrapper() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");

        // 1) 写核心私有:全仓只有审计模块持有 `write_audited`。
        let mut files = Vec::new();
        collect_rust_sources(&root, &mut files);
        let mut write_cores = 0usize;
        for path in files {
            let source = std::fs::read_to_string(&path).expect("读取源码");
            // 禁词拆分书写,避免扫描器自命中。
            write_cores += source.matches(concat!("fn write_", "audited(")).count();
        }
        assert_eq!(write_cores, 1, "设置写核心必须唯一且私有于审计模块");

        // 2) 函数体级守卫:生产设置写入点确实走包装,且无旁路写。
        let settings_store =
            std::fs::read_to_string(root.join("settings_store.rs")).expect("读取 settings_store");
        let save_body = function_body(&settings_store, "fn save_merged_locked(");
        assert!(
            save_body.contains("audit::write_settings"),
            "settings_store 唯一写路径必须经审计包装"
        );
        assert!(
            !save_body.contains("atomic_file::write"),
            "settings_store 写主体不得旁路 atomic_file::write"
        );
        let recover_body = function_body(&settings_store, "fn recover_corrupted(");
        assert!(
            recover_body.contains("audit::write_settings"),
            "损坏恢复重建同样必须审计"
        );
        assert!(
            !recover_body.contains("atomic_file::write"),
            "损坏恢复重建不得旁路 atomic_file::write"
        );

        let local = std::fs::read_to_string(root.join("ai_hook/local.rs")).expect("读取 ai_hook");
        for signature in ["fn ensure_claude_hooks_in(", "fn remove_claude_hooks_at("] {
            let body = function_body(&local, signature);
            assert!(
                body.contains("audit::write_user_settings"),
                "{signature} 必须经审计包装写 user 层 settings.json"
            );
            for bypass in ["write_atomic(", "atomic_file::write", "std::fs::write"] {
                assert!(!body.contains(bypass), "{signature} 出现旁路写 {bypass}");
            }
        }
    }
}
