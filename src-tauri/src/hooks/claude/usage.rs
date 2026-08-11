//! claude hooks provider 用量查询 —— 从 transcript 文件尾部 64KB 逆行扫描 token 用量（MC-213 下沉）
//!
//! 职责：
//! - ContextUsage DTO 定义在 hooks/mod.rs（跨边界契约共享 DTO，MC-214 四字段保留）
//! - parse_usage_line 纯函数：单行 JSON → Option<ContextUsage>
//! - scan_transcript_usage 纯 I/O 逻辑：provider.context_usage 经命令层 spawn_blocking 调用

use crate::hooks::ContextUsage;
use std::io::{Read, Seek, SeekFrom};

/// 扫描 transcript 文件的 token 用量（纯 I/O 逻辑，命令层在 spawn_blocking 内调用）
pub(crate) fn scan_transcript_usage(path: &str) -> Option<ContextUsage> {
    let mut file = std::fs::File::open(path).ok()?;
    let file_size = file.metadata().ok()?.len();

    // 从文件尾部读取最多 64KB（TRANSCRIPT_TAIL_BYTES）
    let read_start = file_size.saturating_sub(TRANSCRIPT_TAIL_BYTES);
    if read_start > 0 {
        file.seek(SeekFrom::Start(read_start)).ok()?;
    }

    let mut buf = Vec::with_capacity(TRANSCRIPT_TAIL_BYTES as usize);
    file.read_to_end(&mut buf).ok()?;

    let text = String::from_utf8_lossy(&buf);
    // 收集所有行（借用 text 存活到循环结束）
    let lines: Vec<&str> = text.lines().collect();
    if lines.is_empty() {
        return None;
    }

    // 若从中途读取，首行为不完整的截断行——跳过
    let start_idx = usize::from(read_start > 0 && !lines.is_empty());

    // 从末行逆行扫描
    for line in lines.iter().skip(start_idx).rev() {
        if let Some(usage) = parse_usage_line(line) {
            return Some(usage);
        }
    }

    None
}

/// 从 transcript 尾部读取的字节数（64KB）
const TRANSCRIPT_TAIL_BYTES: u64 = 64 * 1024;

/// 解析单行 JSON 中的 usage 信息
///
/// 预期结构：`{"message": {"usage": {"input_tokens": N, "output_tokens": M}}}`
/// 解析失败或字段缺失返回 None。
fn parse_usage_line(line: &str) -> Option<ContextUsage> {
    let v: serde_json::Value = serde_json::from_str(line).ok()?;
    let usage = v.get("message")?.get("usage")?;
    let input_tokens = usage.get("input_tokens")?.as_u64()?;
    let output_tokens = usage.get("output_tokens")?.as_u64()?;
    let cache_read_input_tokens = usage
        .get("cache_read_input_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let cache_creation_input_tokens = usage
        .get("cache_creation_input_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    Some(ContextUsage {
        input_tokens,
        output_tokens,
        cache_read_input_tokens,
        cache_creation_input_tokens,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    // ── parse_usage_line 单元测试 ──

    #[test]
    fn parse_valid_usage_line() {
        let json = r#"{"message":{"usage":{"input_tokens":100,"output_tokens":50}}}"#;
        let r = parse_usage_line(json);
        assert!(r.is_some());
        let u = r.unwrap();
        assert_eq!(u.input_tokens, 100);
        assert_eq!(u.output_tokens, 50);
    }

    #[test]
    fn parse_missing_usage_field() {
        let json = r#"{"message":{"other":"data"}}"#;
        assert!(parse_usage_line(json).is_none());
    }

    #[test]
    fn parse_missing_message_field() {
        let json = r#"{"usage":{"input_tokens":100}}"#;
        assert!(parse_usage_line(json).is_none());
    }

    #[test]
    fn parse_invalid_json() {
        assert!(parse_usage_line("not json").is_none());
        assert!(parse_usage_line("{broken").is_none());
    }

    #[test]
    fn parse_empty_string() {
        assert!(parse_usage_line("").is_none());
    }

    #[test]
    fn parse_large_token_values() {
        let json = r#"{"message":{"usage":{"input_tokens":18446744073709551615,"output_tokens":999999999}}}"#;
        let r = parse_usage_line(json);
        assert!(r.is_some());
        let u = r.unwrap();
        assert_eq!(u.input_tokens, u64::MAX);
        assert_eq!(u.output_tokens, 999999999);
    }

    #[test]
    fn parse_extra_fields_ignored() {
        // usage 中含额外字段不影响解析
        let json = r#"{"message":{"usage":{"input_tokens":10,"output_tokens":20,"cache_read":5,"cache_write":3}}}"#;
        let r = parse_usage_line(json);
        assert!(r.is_some());
        let u = r.unwrap();
        assert_eq!(u.input_tokens, 10);
        assert_eq!(u.output_tokens, 20);
    }

    #[test]
    fn parse_nested_extra_fields_ignored() {
        // message 中除了 usage 还有其他字段
        let json = r#"{"message":{"id":"msg_001","usage":{"input_tokens":30,"output_tokens":40},"model":"claude"}}"#;
        let r = parse_usage_line(json);
        assert!(r.is_some());
        let u = r.unwrap();
        assert_eq!(u.input_tokens, 30);
        assert_eq!(u.output_tokens, 40);
    }

    #[test]
    fn parse_input_tokens_not_number() {
        let json = r#"{"message":{"usage":{"input_tokens":"abc","output_tokens":50}}}"#;
        assert!(parse_usage_line(json).is_none());
    }

    // ── PF2-TE-06: cache 字段解析 ──
    //
    // 注意：mockIPC（L2）只守 JS 侧形状——cache 字段的真实解析（后端反序列化 +
    // 缺失 → 默认 0）必须由 L1 覆盖。以下用例使用真实 serde+parse_usage_line，
    // 不依赖前端 mock。

    #[test]
    fn parse_cache_fields_extracted() {
        // transcript 含 cache_read_input_tokens / cache_creation_input_tokens 时正确提取
        let json = r#"{"message":{"usage":{"input_tokens":100,"output_tokens":50,"cache_read_input_tokens":200,"cache_creation_input_tokens":300}}}"#;
        let r = parse_usage_line(json);
        assert!(r.is_some());
        let u = r.unwrap();
        assert_eq!(u.input_tokens, 100);
        assert_eq!(u.output_tokens, 50);
        assert_eq!(u.cache_read_input_tokens, 200);
        assert_eq!(u.cache_creation_input_tokens, 300);
    }

    #[test]
    fn parse_cache_fields_missing_defaults_to_zero() {
        // 旧 transcript 不含 cache 字段 → 默认 0（兼容旧格式）
        let json = r#"{"message":{"usage":{"input_tokens":100,"output_tokens":50}}}"#;
        let r = parse_usage_line(json);
        assert!(r.is_some());
        let u = r.unwrap();
        assert_eq!(u.input_tokens, 100);
        assert_eq!(u.output_tokens, 50);
        assert_eq!(u.cache_read_input_tokens, 0);
        assert_eq!(u.cache_creation_input_tokens, 0);
    }

    #[test]
    fn parse_cache_fields_explicit_zero() {
        // cache 字段显式为 0
        let json = r#"{"message":{"usage":{"input_tokens":100,"output_tokens":50,"cache_read_input_tokens":0,"cache_creation_input_tokens":0}}}"#;
        let r = parse_usage_line(json);
        assert!(r.is_some());
        let u = r.unwrap();
        assert_eq!(u.input_tokens, 100);
        assert_eq!(u.output_tokens, 50);
        assert_eq!(u.cache_read_input_tokens, 0);
        assert_eq!(u.cache_creation_input_tokens, 0);
    }

    #[test]
    fn parse_only_one_cache_field_present() {
        // 仅 cache_read_input_tokens 存在，cache_creation_input_tokens 缺失 → 后者默认 0
        let json = r#"{"message":{"usage":{"input_tokens":100,"output_tokens":50,"cache_read_input_tokens":200}}}"#;
        let r = parse_usage_line(json);
        assert!(r.is_some());
        let u = r.unwrap();
        assert_eq!(u.input_tokens, 100);
        assert_eq!(u.output_tokens, 50);
        assert_eq!(u.cache_read_input_tokens, 200);
        assert_eq!(u.cache_creation_input_tokens, 0);
    }

    // ── scan_transcript_usage 集成测试（spawn_blocking 不参与测试） ──

    fn make_temp_transcript(lines: &[&str]) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("transcript.jsonl");
        let mut file = std::fs::File::create(&file_path).unwrap();
        for line in lines {
            writeln!(file, "{line}").unwrap();
        }
        (dir, file_path)
    }

    #[test]
    fn scan_finds_usage_in_last_line() {
        let (_dir, path) = make_temp_transcript(&[
            r#"{"message":{"usage":{"input_tokens":10,"output_tokens":5}}}"#,
            r#"{"message":{"usage":{"input_tokens":100,"output_tokens":50}}}"#,
        ]);
        let r = scan_transcript_usage(path.to_str().unwrap());
        assert!(r.is_some());
        let u = r.unwrap();
        // 应返回最后一行（最新的）usage
        assert_eq!(u.input_tokens, 100);
        assert_eq!(u.output_tokens, 50);
    }

    #[test]
    fn scan_skips_useage_in_early_lines() {
        // usage 在中间行，但最后一行无 usage——应返回中间行的 usage
        let (_dir, path) = make_temp_transcript(&[
            r#"{"message":{"usage":{"input_tokens":10,"output_tokens":5}}}"#,
            r#"{"message":{"other":"no usage here"}}"#,
        ]);
        let r = scan_transcript_usage(path.to_str().unwrap());
        assert!(r.is_some());
        let u = r.unwrap();
        assert_eq!(u.input_tokens, 10);
        assert_eq!(u.output_tokens, 5);
    }

    #[test]
    fn scan_no_usage_returns_none() {
        let (_dir, path) = make_temp_transcript(&[
            r#"{"type":"system","message":"hello"}"#,
            r#"{"type":"assistant","message":"hi"}"#,
        ]);
        let r = scan_transcript_usage(path.to_str().unwrap());
        assert!(r.is_none());
    }

    #[test]
    fn scan_file_not_found_returns_none() {
        let r = scan_transcript_usage("/nonexistent/path/transcript.jsonl");
        assert!(r.is_none());
    }

    #[test]
    fn scan_empty_file_returns_none() {
        let (_dir, path) = make_temp_transcript(&[]);
        let r = scan_transcript_usage(path.to_str().unwrap());
        assert!(r.is_none());
    }

    #[test]
    fn scan_json_parse_error_in_line_skipped() {
        let (_dir, path) = make_temp_transcript(&[
            r#"{broken json line}"#,
            r#"{"message":{"usage":{"input_tokens":42,"output_tokens":7}}}"#,
        ]);
        let r = scan_transcript_usage(path.to_str().unwrap());
        assert!(r.is_some());
        let u = r.unwrap();
        assert_eq!(u.input_tokens, 42);
    }

    // ── TRANSCRIPT_TAIL_BYTES 常量 ──

    #[test]
    fn transcript_tail_bytes_is_64k() {
        assert_eq!(TRANSCRIPT_TAIL_BYTES, 64 * 1024);
    }

    // ── ContextUsage serde camelCase ──

    #[test]
    fn context_usage_serialize_camelcase() {
        let u = ContextUsage {
            input_tokens: 100,
            output_tokens: 50,
            cache_read_input_tokens: 200,
            cache_creation_input_tokens: 300,
        };
        let json = serde_json::to_string(&u).unwrap();
        assert!(json.contains("\"inputTokens\""));
        assert!(json.contains("\"outputTokens\""));
        assert!(json.contains("\"cacheReadInputTokens\""));
        assert!(json.contains("\"cacheCreationInputTokens\""));
        assert!(!json.contains("\"input_tokens\""));
        assert!(!json.contains("\"output_tokens\""));
        assert!(!json.contains("\"cache_read_input_tokens\""));
        assert!(!json.contains("\"cache_creation_input_tokens\""));
    }

    #[test]
    fn context_usage_deserialize_camelcase() {
        let json = r#"{"inputTokens":200,"outputTokens":300,"cacheReadInputTokens":400,"cacheCreationInputTokens":500}"#;
        let u: ContextUsage = serde_json::from_str(json).unwrap();
        assert_eq!(u.input_tokens, 200);
        assert_eq!(u.output_tokens, 300);
        assert_eq!(u.cache_read_input_tokens, 400);
        assert_eq!(u.cache_creation_input_tokens, 500);
    }

    #[test]
    fn context_usage_deserialize_missing_cache_fields() {
        // 旧 transcript 不含 cache 字段 → #[serde(default)] 兜底为 0
        let json = r#"{"inputTokens":200,"outputTokens":300}"#;
        let u: ContextUsage = serde_json::from_str(json).unwrap();
        assert_eq!(u.input_tokens, 200);
        assert_eq!(u.output_tokens, 300);
        assert_eq!(u.cache_read_input_tokens, 0);
        assert_eq!(u.cache_creation_input_tokens, 0);
    }

    // ── provider 层 context_usage（HUK-05：参数透传 + None/Some 返回映射，经 trait impl） ──
    //
    // 纯函数扫描路径由上方 scan_transcript_usage 组直接覆盖（HUK-10 去重，仅存一组）；
    // 本组经 ClaudeHooksProvider::context_usage（CliHooksProvider trait impl）调用，
    // 覆盖 trait 层参数透传与返回值映射，同时保留大文件尾部扫描用例（原 P2-TE-05）。
    // 命令层 cliId 透传/未知 cliId 由 hooks::mod 命令层测试覆盖（block_on 直测）。

    use crate::hooks::claude::ClaudeHooksProvider;
    use crate::hooks::provider::CliHooksProvider;

    /// 参数透传 usageSourcePath：不同文件 → 各自扫描结果（Some 返回映射）
    #[test]
    fn context_usage_passes_usage_source_path() {
        let (_dir_a, path_a) = make_temp_transcript(&[
            r#"{"message":{"usage":{"input_tokens":10,"output_tokens":5}}}"#,
        ]);
        let (_dir_b, path_b) = make_temp_transcript(&[
            r#"{"message":{"usage":{"input_tokens":20,"output_tokens":6}}}"#,
        ]);

        let a = ClaudeHooksProvider
            .context_usage(path_a.to_str().unwrap())
            .unwrap()
            .unwrap();
        let b = ClaudeHooksProvider
            .context_usage(path_b.to_str().unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(a.input_tokens, 10, "usage_source_path 应透传到扫描逻辑");
        assert_eq!(b.input_tokens, 20);
    }

    /// None 返回映射：文件无 usage → Ok(None)
    #[test]
    fn context_usage_none_mapping() {
        let (_dir, path) = make_temp_transcript(&[r#"{"type":"system","message":"no usage"}"#]);

        let r = ClaudeHooksProvider
            .context_usage(path.to_str().unwrap())
            .unwrap();
        assert!(r.is_none(), "无 usage 应映射为 Ok(None)");
    }

    /// 大文件（>128KB）仅读尾部 64KB（原 P2-TE-05 用例 5，HUK-10 迁移至 provider 层）
    ///
    /// 构造约 200KB 的 JSONL 文件，前 140KB 为无 usage 的填充行
    /// （超出 TRANSCRIPT_TAIL_BYTES 窗口），usage 行在末尾 1KB 内。
    /// 经 provider 层调用，验证全链路仍只加载尾部窗口。
    #[test]
    fn context_usage_large_file_tail_scan() {
        let mut file = NamedTempFile::new().unwrap();

        // 填充约 140KB 的 padding 行（有效 JSON 但无 usage 字段）
        let padding_line = "{\"type\":\"pad\",\"n\":0}\n";
        let target_padding: usize = 140 * 1024;
        let mut written: usize = 0;
        while written < target_padding {
            let n = file.write(padding_line.as_bytes()).unwrap();
            written += n;
        }

        // 在尾部窗口内写入中间行 + 有效 usage 行
        writeln!(file, r#"{{"type":"assistant","msg":"nearly done"}}"#).unwrap();
        writeln!(
            file,
            r#"{{"message":{{"usage":{{"input_tokens":99999,"output_tokens":88888}}}}}}"#
        )
        .unwrap();
        writeln!(file, r#"{{"type":"system","msg":"fin"}}"#).unwrap();
        file.flush().unwrap();

        // 验证文件大小确实 > 128KB
        let meta = std::fs::metadata(file.path()).unwrap();
        assert!(
            meta.len() > 128 * 1024,
            "文件应大于128KB，实际 {} 字节",
            meta.len()
        );

        let r = ClaudeHooksProvider
            .context_usage(file.path().to_str().unwrap())
            .unwrap();
        assert!(r.is_some(), "应从尾部 64KB 找到 usage");
        let u = r.unwrap();
        assert_eq!(u.input_tokens, 99999);
        assert_eq!(u.output_tokens, 88888);
    }
}
