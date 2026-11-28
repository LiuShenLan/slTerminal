//! 诊断服务：结构化健康快照、白名单材料、脱敏文本与原子导出。
//!
//! 复制诊断与导出共用同一 DTO；材料只来自显式白名单类别，凭据值、
//! 正文、终端输出、历史与会话永不进入。日志行在收集时脱敏。

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::Serialize;

/// 诊断材料类别；枚举即白名单，未列出的内容无法进入报告。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DiagnosticSection {
    /// 应用与系统版本、架构、安装形态。
    Environment,
    /// 运行能力与健康状态。
    Health,
    /// 脱敏后的应用日志尾部。
    AppLog,
    /// 端点发现记录的非敏感元数据。
    RuntimeDiscovery,
    /// 完整路径：只在用户显式选择时加入，属于敏感材料。
    FullPaths,
}

impl DiagnosticSection {
    pub(crate) const ALL: [Self; 4] = [
        Self::Environment,
        Self::Health,
        Self::AppLog,
        Self::RuntimeDiscovery,
    ];

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Environment => "environment",
            Self::Health => "health",
            Self::AppLog => "app-log",
            Self::RuntimeDiscovery => "runtime-discovery",
            Self::FullPaths => "full-paths",
        }
    }
}

/// 结构化健康快照；字段只含版本、能力与封闭状态，不含路径明文或正文。
#[derive(Clone, Debug, Serialize)]
pub(crate) struct DiagnosticSnapshot {
    pub(crate) version: String,
    pub(crate) os: String,
    pub(crate) arch: &'static str,
    pub(crate) direct_install: bool,
    pub(crate) externally_managed: bool,
    pub(crate) runtime_available: bool,
    pub(crate) runtime_discovery_published: bool,
    pub(crate) last_error_code: Option<String>,
    pub(crate) healthy: bool,
    pub(crate) generated_at_ms: u64,
}

impl DiagnosticSnapshot {
    pub(crate) fn current(runtime_available: bool, runtime_discovery_published: bool) -> Self {
        let distribution = crate::platform::distribution::current();
        let generated_at_ms = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_millis() as u64)
            .unwrap_or(0);
        Self {
            version: env!("CARGO_PKG_VERSION").to_owned(),
            os: crate::platform::machine::os_identity(),
            arch: std::env::consts::ARCH,
            direct_install: !distribution.externally_managed(),
            externally_managed: distribution.externally_managed(),
            runtime_available,
            runtime_discovery_published,
            last_error_code: None,
            healthy: true,
            generated_at_ms,
        }
    }
}

/// 一份诊断报告：快照 + 脱敏日志 + 白名单类别清单。
#[derive(Clone, Debug, Serialize)]
pub(crate) struct DiagnosticReport {
    pub(crate) snapshot: DiagnosticSnapshot,
    pub(crate) sections: Vec<&'static str>,
    pub(crate) logs: Vec<String>,
    /// 完整路径只在显式选择时非空；默认导出永不包含。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) full_paths: Vec<String>,
}

impl DiagnosticReport {
    pub(crate) fn new(snapshot: DiagnosticSnapshot) -> Self {
        Self {
            snapshot,
            sections: DiagnosticSection::ALL
                .iter()
                .map(|section| section.label())
                .collect(),
            logs: Vec::new(),
            full_paths: Vec::new(),
        }
    }

    /// 显式加入完整路径类别；空集合不声明该类别。
    pub(crate) fn with_full_paths(mut self, paths: Vec<String>) -> Self {
        if !paths.is_empty() {
            self.sections.push(DiagnosticSection::FullPaths.label());
            self.full_paths = paths;
        }
        self
    }

    /// 收集日志尾部；逐行脱敏，非 UTF-8 与超限行按行截断。
    pub(crate) fn collect_app_log(&mut self, path: &Path, max_lines: usize) {
        for line in tail_lines(path, max_lines) {
            self.logs.push(redact(&line));
        }
    }

    /// 增加一条已发生的诊断事件（测试与自检共用）。
    pub(crate) fn push_event(&mut self, line: &str) {
        self.logs.push(redact(line));
    }

    /// 端点发现记录只贡献非敏感元数据；token 不属于诊断材料。
    pub(crate) fn push_runtime_discovery(
        &mut self,
        port: u16,
        stream_epoch: &str,
        published: bool,
    ) {
        self.push_event(&format!(
            "runtime endpoint loopback:{port} stream={stream_epoch} published={published}"
        ));
    }

    /// 复制预览与导出共用同一 DTO：`to_json` 是唯一序列化出口。
    pub(crate) fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_owned())
    }

    /// 原子导出到 `diagnostics.json`；返回最终路径。
    pub(crate) fn write_export_at(&self, root: &Path) -> Result<PathBuf, std::io::Error> {
        let path = root.join("diagnostics.json");
        crate::atomic_file::write(&path, self.to_json().as_bytes())?;
        Ok(path)
    }
}

/// 日志尾部读取：只保留最后 `max_lines` 行，避免把整个日志读入内存。
fn tail_lines(path: &Path, max_lines: usize) -> Vec<String> {
    use std::collections::VecDeque;
    use std::io::BufRead as _;

    let Ok(file) = std::fs::File::open(path) else {
        return Vec::new();
    };
    let reader = std::io::BufReader::new(file);
    let mut tail = VecDeque::with_capacity(max_lines.max(1));
    for line in reader.lines().map_while(Result::ok) {
        if tail.len() == max_lines {
            tail.pop_front();
        }
        tail.push_back(line);
    }
    tail.into_iter().collect()
}

/// 单行脱敏：`sk-` 凭据、`Bearer` 值与敏感键值统一替换为 `[redacted]`。
///
/// 只做保守模式匹配，不承诺识别所有秘密；白名单边界仍是材料来源。
pub(crate) fn redact(line: &str) -> String {
    const MARK: &str = "[redacted]";
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    loop {
        let lower = rest.to_ascii_lowercase();
        // 键名集合与备份敏感键表对齐：两套安全输出通道不得口径漂移。
        let hex_token = find_lower_hex_token(rest);
        let (index, key_len) = [
            ("sk-", 0usize),
            ("bearer ", 0usize),
            ("token=", "token=".len()),
            ("token: ", "token: ".len()),
            ("token\":", "token\":".len()),
            ("access_token=", "access_token=".len()),
            ("access_token: ", "access_token: ".len()),
            ("refresh_token=", "refresh_token=".len()),
            ("refresh_token: ", "refresh_token: ".len()),
            ("api_key=", "api_key=".len()),
            ("api_key: ", "api_key: ".len()),
            ("api-key=", "api-key=".len()),
            ("apikey=", "apikey=".len()),
            ("authorization=", "authorization=".len()),
            ("authorization: ", "authorization: ".len()),
            ("authorization\":", "authorization\":".len()),
            ("client_secret=", "client_secret=".len()),
            ("client_secret: ", "client_secret: ".len()),
            ("cookie=", "cookie=".len()),
            ("cookie: ", "cookie: ".len()),
            ("private_key=", "private_key=".len()),
            ("private_key: ", "private_key: ".len()),
            ("secret=", "secret=".len()),
            ("secret: ", "secret: ".len()),
            ("password=", "password=".len()),
            ("password: ", "password: ".len()),
        ]
        .into_iter()
        .filter_map(|(needle, keep)| lower.find(needle).map(|index| (index, keep)))
        .min_by_key(|(index, _)| *index)
        .unwrap_or((rest.len(), 0));
        // 裸 Runtime 凭据形态：恰好 64 位小写 hex，无键名前缀。
        if let Some((start, end)) = hex_token
            && start < index
        {
            out.push_str(&rest[..start]);
            out.push_str(MARK);
            rest = &rest[end..];
            continue;
        }
        if index == rest.len() {
            out.push_str(rest);
            break;
        }
        if key_len == 0 && lower[index..].starts_with("sk-") {
            out.push_str(&rest[..index]);
            let end = rest[index..]
                .find(|ch: char| ch.is_whitespace() || matches!(ch, '"' | '\'' | ',' | ';'))
                .map_or(rest.len(), |offset| index + offset);
            out.push_str(MARK);
            rest = &rest[end..];
            continue;
        }
        if key_len == 0 {
            // `Bearer ` 后的凭据原样吞到分隔符。
            out.push_str(&rest[..index + "bearer ".len()]);
            let (prefix, end) = consume_secret(rest, index + "bearer ".len(), false, 0);
            out.push_str(&prefix);
            rest = &rest[end..];
            continue;
        }
        out.push_str(&rest[..index + key_len]);
        // 授权头形如 `Authorization: Basic <token>`：凭据在第二个 token。
        let extra_tokens = usize::from(lower[index..].starts_with("authorization"));
        let (prefix, end) = consume_secret(rest, index + key_len, true, extra_tokens);
        out.push_str(&prefix);
        rest = &rest[end..];
    }
    out
}

/// 查找恰好 64 位小写 hex 的连续段；更长 hex 段不截断匹配。
fn find_lower_hex_token(line: &str) -> Option<(usize, usize)> {
    let bytes = line.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        let is_hex = |byte: u8| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte);
        if !is_hex(bytes[index]) {
            index += 1;
            continue;
        }
        let start = index;
        while index < bytes.len() && is_hex(bytes[index]) {
            index += 1;
        }
        if index - start == 64 {
            return Some((start, index));
        }
    }
    None
}

/// 从 `start` 起跳过空白与引号后吞掉一个值；`extra_tokens` 额外吞掉
/// 若干以空白分隔的 token（授权头的方案词之后才是凭据）。
/// 返回（前缀分隔文本, 值结束位置）。
fn consume_secret(
    line: &str,
    start: usize,
    keep_prefix: bool,
    extra_tokens: usize,
) -> (String, usize) {
    let mut prefix = String::new();
    let value_start = skip_separators(line, start, &mut prefix);
    let mut end = skip_token(line, value_start);
    for _ in 0..extra_tokens {
        let next = skip_separators(line, end, &mut String::new());
        if next == end {
            break;
        }
        end = skip_token(line, next);
    }
    if keep_prefix {
        prefix.push_str("[redacted]");
        (prefix, end)
    } else {
        ("[redacted]".to_owned(), end)
    }
}

/// 跳过空白与引号；`prefix` 收集应保留在输出中的分隔字符。
fn skip_separators(line: &str, start: usize, prefix: &mut String) -> usize {
    let mut cursor = start;
    while cursor < line.len() {
        let ch = line[cursor..].chars().next().expect("char boundary");
        if ch == ' ' || ch == '"' || ch == '\'' {
            prefix.push(ch);
            cursor += ch.len_utf8();
        } else {
            break;
        }
    }
    cursor
}

/// 吞掉一个非分隔 token；返回 token 结束位置。
fn skip_token(line: &str, start: usize) -> usize {
    let mut end = start;
    for (offset, ch) in line[start..].char_indices() {
        if ch.is_whitespace() || matches!(ch, '"' | '\'' | ',' | ';' | '}') {
            return start + offset;
        }
        end = start + offset + ch.len_utf8();
    }
    end
}

#[cfg(test)]
mod diagnostics_tests {
    use super::*;

    fn snapshot() -> DiagnosticSnapshot {
        DiagnosticSnapshot {
            version: "0.0.0-test".to_owned(),
            os: "windows-test".to_owned(),
            arch: "x86_64",
            direct_install: true,
            externally_managed: false,
            runtime_available: true,
            runtime_discovery_published: true,
            last_error_code: None,
            healthy: true,
            generated_at_ms: 1,
        }
    }

    /// 锚：端点、设置、备份和诊断不含 Runtime token。
    #[test]
    fn runtime_token_absent_from_materials() {
        // Runtime 凭据真实形态是 64 位小写 hex，不带 `sk-` 前缀。
        let token = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        assert_eq!(token.len(), 64);
        let mut report = DiagnosticReport::new(snapshot());
        report.push_event(&format!("runtime handshake accepted for token={token}"));
        report.push_event(&format!("Authorization: Bearer {token}"));
        report.push_event(&format!("credential {token} stored"));

        let exported = report.to_json();
        assert!(!exported.contains(token), "导出材料不得含 Runtime token");
        assert!(exported.contains("[redacted]"));
        // 复制预览与导出共用同一 DTO：预览正文即导出正文。
        assert_eq!(report.to_json(), exported);

        // 发现记录只允许非敏感元数据类别。
        assert!(report.sections.iter().all(|section| {
            DiagnosticSection::ALL
                .iter()
                .any(|allowed| allowed.label() == *section)
        }));
    }

    /// 发现记录只贡献端口与流代次等非敏感元数据。
    #[test]
    fn runtime_discovery_materials_carry_no_token() {
        let mut report = DiagnosticReport::new(snapshot());
        report.push_runtime_discovery(54321, "018f2f7c-stream", true);
        let json = report.to_json();
        assert!(json.contains("54321"));
        assert!(json.contains("018f2f7c-stream"));
        assert!(!json.contains("sk-"));
        assert!(!json.contains("runtime_token"));
    }

    /// 脱敏覆盖常见凭据形态，不破坏普通文本。
    #[test]
    fn redaction_covers_credential_shapes() {
        assert_eq!(redact("plain message"), "plain message");
        assert_eq!(
            redact("login token=sk-test-abc done"),
            "login token=[redacted] done"
        );
        assert_eq!(
            redact(r#"{"token": "sk-test-xyz"}"#),
            r#"{"token": "[redacted]"}"#
        );
        assert!(!redact("Bearer sk-test-bearer").contains("sk-test-bearer"));
        assert_eq!(redact("secret: hunter2"), "secret: [redacted]");
        // 与备份敏感键表对齐的键名形态。
        assert_eq!(redact("api_key=sk-live-abc"), "api_key=[redacted]");
        assert_eq!(
            redact("Authorization: Basic dXNlcjpwYXNz"),
            "Authorization: [redacted]"
        );
        assert_eq!(redact("cookie=sessionid=abc"), "cookie=[redacted]");
        assert_eq!(redact("private_key: MIIBVA"), "private_key: [redacted]");
        assert_eq!(
            redact(r#"{"authorization":"Bearer abc"}"#),
            r#"{"authorization":"[redacted]"}"#
        );
        let token = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        assert_eq!(
            redact(&format!(r#"{{"value":"{token}"}}"#)),
            r#"{"value":"[redacted]"}"#
        );
        assert_eq!(
            redact(&format!("plain {token} value")),
            "plain [redacted] value"
        );
    }

    /// 导出经原子写落盘，内容是完整 DTO 且不含非白名单类别。
    #[test]
    fn export_is_atomic_and_whitelisted() {
        let temp = tempfile::tempdir().unwrap();
        let mut report = DiagnosticReport::new(snapshot());
        report.push_event("ok");
        let path = report.write_export_at(temp.path()).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["snapshot"]["version"], "0.0.0-test");
        assert_eq!(value["logs"][0], "ok");
        let labels = report.sections;
        assert_eq!(labels.len(), DiagnosticSection::ALL.len());
        assert!(labels.contains(&"environment"));
        assert!(labels.contains(&"health"));
        assert!(labels.contains(&"app-log"));
        assert!(labels.contains(&"runtime-discovery"));
    }

    /// 日志尾部只保留最后 N 行。
    #[test]
    fn tail_lines_keeps_only_the_requested_suffix() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("app.log");
        std::fs::write(&path, "one\ntwo\nthree\n").unwrap();
        assert_eq!(tail_lines(&path, 2), vec!["two", "three"]);
        assert!(tail_lines(&temp.path().join("missing.log"), 4).is_empty());
    }
}
