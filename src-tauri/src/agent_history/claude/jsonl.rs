//! 历史会话 JSONL 轻量解析 —— 纯函数（BE-03/BE-04）
//!
//! 性能约束（规格 3.4）：禁止整文件读取。每文件只读：
//! - 头部 ≤512KB：顺序扫描收集 cwd / 标题候选 / 首条可见 user prompt
//! - 尾部 ≤64KB：逆行扫描最后一条 custom-title / ai-title（覆写式 last wins）
//!
//! 容错规则（规格 3.3）：未知 type 忽略、字段可缺失、单行 JSON 解析失败
//! （EOF 截断/损坏）即停止或跳过，永不 panic、永不返回 Err。

use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::Path;

use super::TitleSource;

/// 头部顺序扫描上限（512KB，BE-03）
pub const HEAD_SCAN_LIMIT_BYTES: u64 = 512 * 1024;

/// 尾部标题扫描字节数（64KB，照 hooks/usage.rs TRANSCRIPT_TAIL_BYTES 先例，BE-04）
pub const TAIL_SCAN_BYTES: u64 = 64 * 1024;

/// 首条 prompt 截断长度（字符上限，BE-03）
const PROMPT_MAX_CHARS: usize = 200;

/// 头部解析结果（cwd + 标题候选 + 首条可见 prompt）
#[derive(Debug, Default, Clone)]
pub struct HeadInfo {
    /// 首个含非空 cwd 字段的行
    pub cwd: Option<String>,
    /// 首条可见 user prompt（已截断 ≤200 字符）
    pub first_prompt: Option<String>,
    /// 头部窗口内最后一条 custom-title（last-wins 覆盖）
    pub custom_title: Option<String>,
    /// 头部窗口内最后一条 ai-title（last-wins 覆盖）
    pub ai_title: Option<String>,
    /// 头部窗口内最后一条 summary（last-wins 覆盖）
    pub summary: Option<String>,
}

/// 顺序扫描文件头部收集会话元数据（BE-03）
///
/// 规则：
/// - 命中首条可见 user prompt 或累计读取超 HEAD_SCAN_LIMIT_BYTES 即结束
/// - 可见 prompt 判定：`type=="user"` 且 `message.content` 为字符串；
///   跳过 `isMeta:true` 行、content 为数组的行（tool_result 载体）、
///   trim 后以 `<` 开头的行（`<command-name>`/`<local-command-caveat>` 等占位符）、
///   trim 后为空的行
/// - 未知 type 行忽略；单行 JSON 解析失败（EOF 截断行/损坏行）即停止，不报错
/// - cwd 取首个含非空 cwd 字段的行（目录名只是 cwd 的有损编码，禁止反解码）
/// - 任何 I/O 失败返回已收集结果（容错，不 panic）
pub fn parse_head(path: &Path) -> HeadInfo {
    let mut info = HeadInfo::default();
    let Ok(file) = std::fs::File::open(path) else {
        return info;
    };
    let mut reader = BufReader::new(file);
    let mut read_bytes: u64 = 0;
    let mut line = String::new();
    loop {
        line.clear();
        // 读取失败或 EOF → 结束
        let Ok(n) = reader.read_line(&mut line) else {
            break;
        };
        if n == 0 {
            break;
        }
        read_bytes += n as u64;
        if read_bytes > HEAD_SCAN_LIMIT_BYTES {
            break;
        }
        // JSON 解析失败（EOF 截断行/损坏行）→ 停止扫描，不报错（容错规则 3）
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) else {
            break;
        };
        let Some(type_name) = v.get("type").and_then(|t| t.as_str()) else {
            continue;
        };
        // cwd：首个含非空 cwd 字段的行
        if info.cwd.is_none() {
            if let Some(cwd) = v
                .get("cwd")
                .and_then(|c| c.as_str())
                .filter(|s| !s.is_empty())
            {
                info.cwd = Some(cwd.to_string());
            }
        }
        match type_name {
            "custom-title" => {
                if let Some(t) = v
                    .get("customTitle")
                    .and_then(|x| x.as_str())
                    .filter(|s| !s.is_empty())
                {
                    info.custom_title = Some(t.to_string()); // 同类型 last-wins
                }
            }
            "ai-title" => {
                if let Some(t) = v
                    .get("aiTitle")
                    .and_then(|x| x.as_str())
                    .filter(|s| !s.is_empty())
                {
                    info.ai_title = Some(t.to_string());
                }
            }
            "summary" => {
                if let Some(t) = v
                    .get("summary")
                    .and_then(|x| x.as_str())
                    .filter(|s| !s.is_empty())
                {
                    info.summary = Some(t.to_string());
                }
            }
            "user" => {
                if let Some(prompt) = visible_prompt(&v) {
                    info.first_prompt = Some(truncate_prompt(prompt));
                    break; // 命中首条可见 prompt，提前结束
                }
            }
            _ => {
                // 未知 type 忽略（容错规则 1）
            }
        }
    }
    info
}

/// 从 user 行提取可见 prompt 文本（不可见返回 None）
///
/// 跳过：isMeta:true 元消息、content 非字符串（数组 = tool_result 载体）、
/// trim 后以 `<` 开头（本地命令占位符）、trim 后为空。
fn visible_prompt(v: &serde_json::Value) -> Option<&str> {
    // isMeta:true 元消息跳过
    if v.get("isMeta").and_then(|m| m.as_bool()) == Some(true) {
        return None;
    }
    let content = v.get("message")?.get("content")?;
    // content 为数组（tool_result 载体）→ 跳过
    let text = content.as_str()?;
    let trimmed = text.trim();
    // `<command-name>`/`<local-command-caveat>`/`<local-command-stdout>` 等占位符跳过
    if trimmed.starts_with('<') || trimmed.is_empty() {
        return None;
    }
    Some(trimmed)
}

/// 截断 prompt 至 ≤200 字符（按字符截取，UTF-8 安全，BE-03）
fn truncate_prompt(text: &str) -> String {
    text.chars().take(PROMPT_MAX_CHARS).collect()
}

/// 尾部 64KB 逆行扫描最后一条标题（BE-04，覆写式 last wins）
///
/// 从文件尾部读 ≤64KB；从中途起始则跳过首行（可能为截断行）。
/// 逆序扫描：`custom-title` 类型恒优先（遇到即返回）；全程无 custom-title
/// 则返回最后一条 `ai-title`。文件不存在/解析失败等任何异常返回 None。
pub fn parse_tail_title(path: &Path) -> Option<(String, TitleSource)> {
    let mut file = std::fs::File::open(path).ok()?;
    let file_size = file.metadata().ok()?.len();
    let read_start = file_size.saturating_sub(TAIL_SCAN_BYTES);
    if read_start > 0 {
        file.seek(SeekFrom::Start(read_start)).ok()?;
    }
    let mut buf = Vec::with_capacity(TAIL_SCAN_BYTES as usize);
    file.read_to_end(&mut buf).ok()?;

    let text = String::from_utf8_lossy(&buf);
    let lines: Vec<&str> = text.lines().collect();
    if lines.is_empty() {
        return None;
    }
    // 从中途读取时首行为不完整的截断行——跳过（照 usage.rs 策略）
    let start_idx = usize::from(read_start > 0 && !lines.is_empty());

    // 逆行扫描：记录最后一条 ai-title 兜底；遇到 custom-title 立即返回
    let mut last_ai_title: Option<(String, TitleSource)> = None;
    for line in lines.iter().skip(start_idx).rev() {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            continue; // 截断/损坏行跳过
        };
        let Some(type_name) = v.get("type").and_then(|t| t.as_str()) else {
            continue;
        };
        match type_name {
            "custom-title" => {
                if let Some(t) = v
                    .get("customTitle")
                    .and_then(|x| x.as_str())
                    .filter(|s| !s.is_empty())
                {
                    // custom-title 类型恒优先于 ai-title（决策 22）
                    return Some((t.to_string(), TitleSource::CustomTitle));
                }
            }
            "ai-title" if last_ai_title.is_none() => {
                if let Some(t) = v
                    .get("aiTitle")
                    .and_then(|x| x.as_str())
                    .filter(|s| !s.is_empty())
                {
                    // 逆行中第一条 ai-title = 文件最后一条（last wins）
                    last_ai_title = Some((t.to_string(), TitleSource::AiTitle));
                }
            }
            _ => {}
        }
    }
    last_ai_title
}

/// 标题回退链合成（决策 22）：custom-title > ai-title > summary > 首条 prompt
///
/// 尾部扫描结果（last wins 物理最新）优先于头部候选；四路皆空 → (None, None)。
pub fn resolve_title(
    head: &HeadInfo,
    tail: Option<(String, TitleSource)>,
) -> (Option<String>, TitleSource) {
    if let Some((t, src)) = tail {
        return (Some(t), src);
    }
    if let Some(t) = &head.custom_title {
        return (Some(t.clone()), TitleSource::CustomTitle);
    }
    if let Some(t) = &head.ai_title {
        return (Some(t.clone()), TitleSource::AiTitle);
    }
    if let Some(t) = &head.summary {
        return (Some(t.clone()), TitleSource::Summary);
    }
    if let Some(p) = &head.first_prompt {
        return (Some(p.clone()), TitleSource::FirstPrompt);
    }
    (None, TitleSource::None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    // ── 测试辅助 ──

    /// 写 jsonl 文件（文件名与解析无关，用固定名）
    fn write_jsonl(lines: &[&str]) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.jsonl");
        let mut f = std::fs::File::create(&path).unwrap();
        for l in lines {
            writeln!(f, "{l}").unwrap();
        }
        (dir, path)
    }

    /// 构造文件并返回 HeadInfo（简化调用）
    fn head_of(lines: &[&str]) -> HeadInfo {
        let (_dir, path) = write_jsonl(lines);
        parse_head(&path)
    }

    // ── parse_head：cwd 收集 ──

    #[test]
    fn head_cwd_takes_first_non_empty_line() {
        // 多个含 cwd 字段的行 → 取首个
        let h = head_of(&[
            r#"{"type":"system","cwd":"C:\\first","version":1}"#,
            r#"{"type":"user","cwd":"C:\\second","message":{"content":"你好"}}"#,
        ]);
        assert_eq!(h.cwd.as_deref(), Some("C:\\first"));
        assert_eq!(h.first_prompt.as_deref(), Some("你好"));
    }

    #[test]
    fn head_cwd_missing_is_none() {
        // 全部行无 cwd 字段 → None（无 cwd 会话）
        let h = head_of(&[
            r#"{"type":"summary","summary":"摘要","leafUuid":"x"}"#,
            r#"{"type":"user","message":{"content":"你好"}}"#,
        ]);
        assert!(h.cwd.is_none());
    }

    #[test]
    fn head_cwd_null_or_empty_skipped() {
        // cwd 为 null / 空串的行不作为 cwd 来源
        let h = head_of(&[
            r#"{"type":"system","cwd":null}"#,
            r#"{"type":"system","cwd":""}"#,
            r#"{"type":"user","cwd":"C:\\real","message":{"content":"你好"}}"#,
        ]);
        assert_eq!(h.cwd.as_deref(), Some("C:\\real"));
    }

    // ── parse_head：首条可见 prompt（跳过 4 类 + 正常命中） ──

    #[test]
    fn head_prompt_hits_first_visible_user_line() {
        let h = head_of(&[
            r#"{"type":"assistant","message":{"content":"前导回复"}}"#,
            r#"{"type":"user","message":{"content":"第一条真正的问题"}}"#,
            r#"{"type":"user","message":{"content":"第二条（不应被取）"}}"#,
        ]);
        assert_eq!(h.first_prompt.as_deref(), Some("第一条真正的问题"));
    }

    #[test]
    fn head_prompt_skips_is_meta() {
        // isMeta:true 行跳过，继续找下一条可见 prompt
        let h = head_of(&[
            r#"{"type":"user","isMeta":true,"message":{"content":"<command-name>ls</command-name>"}}"#,
            r#"{"type":"user","message":{"content":"真实提问"}}"#,
        ]);
        assert_eq!(h.first_prompt.as_deref(), Some("真实提问"));
    }

    #[test]
    fn head_prompt_skips_array_content() {
        // content 为数组（tool_result 载体）→ 跳过
        let h = head_of(&[
            r#"{"type":"user","message":{"content":[{"type":"tool_result","content":"输出"}]}}"#,
            r#"{"type":"user","message":{"content":"真实提问"}}"#,
        ]);
        assert_eq!(h.first_prompt.as_deref(), Some("真实提问"));
    }

    #[test]
    fn head_prompt_skips_angle_bracket_prefix() {
        // 以 `<` 开头（本地命令占位符）→ 跳过
        let h = head_of(&[
            r#"{"type":"user","message":{"content":"<command-name>claude</command-name>"}}"#,
            r#"{"type":"user","message":{"content":"<local-command-caveat>注意事项</local-command-caveat>"}}"#,
            r#"{"type":"user","message":{"content":"真实提问"}}"#,
        ]);
        assert_eq!(h.first_prompt.as_deref(), Some("真实提问"));
    }

    #[test]
    fn head_prompt_skips_blank_after_trim() {
        // trim 后为空 → 跳过
        let h = head_of(&[
            r#"{"type":"user","message":{"content":"   "}}"#,
            r#"{"type":"user","message":{"content":"真实提问"}}"#,
        ]);
        assert_eq!(h.first_prompt.as_deref(), Some("真实提问"));
    }

    #[test]
    fn head_prompt_none_when_all_invisible() {
        // 全部行不可见 → first_prompt None，不 panic
        let h = head_of(&[
            r#"{"type":"user","isMeta":true,"message":{"content":"x"}}"#,
            r#"{"type":"user","message":{"content":[{"type":"tool_result"}]}}"#,
        ]);
        assert!(h.first_prompt.is_none());
    }

    // ── parse_head：容错 ──

    #[test]
    fn head_tolerates_truncated_eof_line() {
        // 末尾截断行（运行中会话）→ 停止扫描不报错，已收集候选保留
        let h = head_of(&[
            r#"{"type":"summary","summary":"摘要","leafUuid":"x"}"#,
            r#"{"type":"user","message":{"content":"你好"}"#, // 截断行（缺闭合）
        ]);
        assert_eq!(h.summary.as_deref(), Some("摘要"));
        // 截断行前无可见 prompt（该行本身解析失败即停止）
        assert!(h.first_prompt.is_none());
    }

    #[test]
    fn head_ignores_unknown_type_lines() {
        // 未知 type 行忽略，不中止扫描
        let h = head_of(&[
            r#"{"type":"queue-operation","op":"xxx"}"#,
            r#"{"type":"progress","n":1}"#,
            r#"{"type":"summary","summary":"摘要","leafUuid":"x"}"#,
            r#"{"type":"future-type","custom":"数据"}"#,
            r#"{"type":"user","message":{"content":"真实提问"}}"#,
        ]);
        assert_eq!(h.summary.as_deref(), Some("摘要"));
        assert_eq!(h.first_prompt.as_deref(), Some("真实提问"));
    }

    #[test]
    fn head_missing_file_returns_default() {
        // 文件不存在 → 空 HeadInfo，不 panic
        let h = parse_head(std::path::Path::new("C:\\不存在\\x.jsonl"));
        assert!(h.cwd.is_none());
        assert!(h.first_prompt.is_none());
    }

    #[test]
    fn head_prompt_truncated_to_200_chars() {
        // 超过 200 字符的 prompt → 截断至 200 字符（中文按字符计）
        let long_prompt = "问".repeat(300);
        let json = format!(r#"{{"type":"user","message":{{"content":"{long_prompt}"}}}}"#);
        let h = head_of(&[&json]);
        let p = h.first_prompt.unwrap();
        assert_eq!(p.chars().count(), 200);
    }

    // ── parse_head：头部标题候选 last-wins ──

    #[test]
    fn head_titles_last_wins() {
        // 同类型标题多次出现 → 取最后一条
        let h = head_of(&[
            r#"{"type":"ai-title","aiTitle":"旧标题","sessionId":"x"}"#,
            r#"{"type":"ai-title","aiTitle":"新标题","sessionId":"x"}"#,
            r#"{"type":"summary","summary":"旧摘要","leafUuid":"x"}"#,
            r#"{"type":"summary","summary":"新摘要","leafUuid":"x"}"#,
            r#"{"type":"custom-title","customTitle":"旧自定义","sessionId":"x"}"#,
            r#"{"type":"custom-title","customTitle":"新自定义","sessionId":"x"}"#,
        ]);
        assert_eq!(h.ai_title.as_deref(), Some("新标题"));
        assert_eq!(h.summary.as_deref(), Some("新摘要"));
        assert_eq!(h.custom_title.as_deref(), Some("新自定义"));
    }

    #[test]
    fn head_skips_empty_title_candidates() {
        // 标题字段为空串/null → 不作为候选
        let h = head_of(&[
            r#"{"type":"ai-title","aiTitle":"","sessionId":"x"}"#,
            r#"{"type":"summary","summary":null,"leafUuid":"x"}"#,
            r#"{"type":"custom-title","customTitle":"有效标题","sessionId":"x"}"#,
        ]);
        assert!(h.ai_title.is_none());
        assert!(h.summary.is_none());
        assert_eq!(h.custom_title.as_deref(), Some("有效标题"));
    }

    // ── 大文件：头部 512KB 上限 + 尾部 64KB 协同 ──

    /// 写入 count 行 padding（未知 type，合法 JSON，不影响任何收集）
    fn write_padding(f: &mut std::fs::File, count: usize) -> usize {
        let mut written = 0usize;
        for _ in 0..count {
            let line =
                b"{\"type\":\"pad\",\"n\":0,\"payload\":\"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\"}\n";
            f.write_all(line).unwrap();
            written += line.len();
        }
        written
    }

    #[test]
    fn head_limit_stops_before_mid_file_title() {
        // 文件 >512KB：custom-title 在 520KB 处（头部窗口外、尾部窗口外）→ 收不到
        let (_dir, path) = write_jsonl(&[]);
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"type":"summary","summary":"头部摘要","leafUuid":"x"}}"#
        )
        .unwrap();
        // 填充至 ~520KB（超过 512KB 头部窗口）
        while f.metadata().unwrap().len() < 520 * 1024 {
            write_padding(&mut f, 50);
        }
        writeln!(
            f,
            r#"{{"type":"custom-title","customTitle":"中部标题","sessionId":"x"}}"#
        )
        .unwrap();
        // 再填充至 ~600KB，确保中部标题也不在尾部 64KB 窗口内
        while f.metadata().unwrap().len() < 600 * 1024 {
            write_padding(&mut f, 50);
        }
        f.flush().unwrap();

        let head = parse_head(&path);
        // 头部窗口收到 summary，但未越过 512KB 读到中部 custom-title
        assert_eq!(head.summary.as_deref(), Some("头部摘要"));
        assert!(head.custom_title.is_none(), "头部扫描不应越过 512KB 上限");
        assert!(head.first_prompt.is_none());

        let tail = parse_tail_title(&path);
        assert!(tail.is_none(), "尾部窗口不应包含中部标题");
        // 回退链落位 summary
        let (title, src) = resolve_title(&head, tail);
        assert_eq!(title.as_deref(), Some("头部摘要"));
        assert_eq!(src, TitleSource::Summary);
    }

    #[test]
    fn tail_scan_finds_title_in_last_64kb_of_large_file() {
        // 文件 >512KB：custom-title 在文件末尾（尾部 64KB 窗口内）→ 尾部命中
        let (_dir, path) = write_jsonl(&[]);
        let mut f = std::fs::File::create(&path).unwrap();
        while f.metadata().unwrap().len() < 600 * 1024 {
            write_padding(&mut f, 50);
        }
        writeln!(
            f,
            r#"{{"type":"custom-title","customTitle":"尾部标题","sessionId":"x"}}"#
        )
        .unwrap();
        f.flush().unwrap();

        // 头部窗口内无任何候选（padding 全为未知 type）
        let head = parse_head(&path);
        assert!(head.custom_title.is_none());
        assert!(head.summary.is_none());

        let tail = parse_tail_title(&path);
        assert_eq!(
            tail.as_ref().map(|(t, _)| t.as_str()),
            Some("尾部标题"),
            "尾部 64KB 应命中末尾 custom-title"
        );
        let (title, src) = resolve_title(&head, tail);
        assert_eq!(title.as_deref(), Some("尾部标题"));
        assert_eq!(src, TitleSource::CustomTitle);
    }

    // ── parse_tail_title：last-wins 与类型优先级 ──

    #[test]
    fn tail_title_last_custom_title_wins() {
        // 尾部窗口内多条 custom-title → 取文件最后一条
        let (_dir, path) = write_jsonl(&[
            r#"{"type":"custom-title","customTitle":"旧自定义","sessionId":"x"}"#,
            r#"{"type":"ai-title","aiTitle":"自动标题","sessionId":"x"}"#,
            r#"{"type":"custom-title","customTitle":"新自定义","sessionId":"x"}"#,
        ]);
        let tail = parse_tail_title(&path).unwrap();
        assert_eq!(tail.0, "新自定义");
        assert_eq!(tail.1, TitleSource::CustomTitle);
    }

    #[test]
    fn tail_title_custom_prefers_ai_even_if_ai_later() {
        // ai-title 写入晚于 custom-title → 类型优先级仍 custom-title 赢（决策 22）
        let (_dir, path) = write_jsonl(&[
            r#"{"type":"custom-title","customTitle":"用户重命名","sessionId":"x"}"#,
            r#"{"type":"ai-title","aiTitle":"后写入的自动标题","sessionId":"x"}"#,
        ]);
        let tail = parse_tail_title(&path).unwrap();
        assert_eq!(tail.0, "用户重命名");
        assert_eq!(tail.1, TitleSource::CustomTitle);
    }

    #[test]
    fn tail_title_ai_fallback() {
        // 仅 ai-title → 取最后一条 ai-title
        let (_dir, path) = write_jsonl(&[
            r#"{"type":"ai-title","aiTitle":"旧自动","sessionId":"x"}"#,
            r#"{"type":"ai-title","aiTitle":"新自动","sessionId":"x"}"#,
        ]);
        let tail = parse_tail_title(&path).unwrap();
        assert_eq!(tail.0, "新自动");
        assert_eq!(tail.1, TitleSource::AiTitle);
    }

    #[test]
    fn tail_title_tolerates_truncated_last_line() {
        // 末尾截断行 → 跳过，仍能找到前面完整标题行
        let (_dir, path) = write_jsonl(&[
            r#"{"type":"custom-title","customTitle":"完整标题","sessionId":"x"}"#,
            r#"{"type":"custom-title","customTitle":"截断行","sessionId":"x""#, // 缺闭合
        ]);
        let tail = parse_tail_title(&path).unwrap();
        assert_eq!(tail.0, "完整标题");
        assert_eq!(tail.1, TitleSource::CustomTitle);
    }

    #[test]
    fn tail_title_empty_or_missing_file() {
        let (_dir, empty) = write_jsonl(&[]);
        assert!(parse_tail_title(&empty).is_none());
        assert!(parse_tail_title(std::path::Path::new("C:\\不存在\\x.jsonl")).is_none());
    }

    // ── resolve_title：标题回退链 5 态 ──

    #[test]
    fn resolve_chain_custom_title_wins_over_ai_title() {
        let head = HeadInfo {
            custom_title: Some("自定义".to_string()),
            ai_title: Some("自动".to_string()),
            summary: Some("摘要".to_string()),
            first_prompt: Some("提问".to_string()),
            cwd: None,
        };
        let (t, src) = resolve_title(&head, None);
        assert_eq!(t.as_deref(), Some("自定义"));
        assert_eq!(src, TitleSource::CustomTitle);
    }

    #[test]
    fn resolve_chain_ai_title_wins_over_summary() {
        let head = HeadInfo {
            custom_title: None,
            ai_title: Some("自动".to_string()),
            summary: Some("摘要".to_string()),
            first_prompt: Some("提问".to_string()),
            cwd: None,
        };
        let (t, src) = resolve_title(&head, None);
        assert_eq!(t.as_deref(), Some("自动"));
        assert_eq!(src, TitleSource::AiTitle);
    }

    #[test]
    fn resolve_chain_summary_wins_over_prompt() {
        let head = HeadInfo {
            custom_title: None,
            ai_title: None,
            summary: Some("摘要".to_string()),
            first_prompt: Some("提问".to_string()),
            cwd: None,
        };
        let (t, src) = resolve_title(&head, None);
        assert_eq!(t.as_deref(), Some("摘要"));
        assert_eq!(src, TitleSource::Summary);
    }

    #[test]
    fn resolve_chain_first_prompt_only() {
        let head = HeadInfo {
            custom_title: None,
            ai_title: None,
            summary: None,
            first_prompt: Some("提问".to_string()),
            cwd: None,
        };
        let (t, src) = resolve_title(&head, None);
        assert_eq!(t.as_deref(), Some("提问"));
        assert_eq!(src, TitleSource::FirstPrompt);
    }

    #[test]
    fn resolve_chain_all_none() {
        let head = HeadInfo::default();
        let (t, src) = resolve_title(&head, None);
        assert!(t.is_none());
        assert_eq!(src, TitleSource::None);
    }

    #[test]
    fn resolve_tail_beats_head_candidates() {
        // 尾部（last wins 物理最新）优先于头部任何候选
        let head = HeadInfo {
            custom_title: Some("头部自定义".to_string()),
            ai_title: None,
            summary: Some("头部摘要".to_string()),
            first_prompt: Some("提问".to_string()),
            cwd: None,
        };
        let tail = Some(("尾部自动标题".to_string(), TitleSource::AiTitle));
        let (t, src) = resolve_title(&head, tail);
        assert_eq!(t.as_deref(), Some("尾部自动标题"));
        assert_eq!(src, TitleSource::AiTitle);
    }
}
