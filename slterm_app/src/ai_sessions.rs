//! 发现本机的 AI CLI 历史会话(claude / codex),给命令面板的「恢复 AI
//! 会话」列表供数据。
//!
//! # 为什么只读文件头部
//!
//! 会话 jsonl 动辄几十 MB,整读会把面板打开卡成秒级。而标题几乎总在最前:
//! claude 压缩过的会话第一行就是 `{"type":"summary",...}`,新会话则是首条
//! user 消息;codex 的 rollout 第一行是 session_meta(带 cwd)。所以这里每个
//! 文件最多读 [`HEAD_BYTES`],而且只为真正要显示的前 N 条读。
//!
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use crate::ai_agents::AgentKind;

/// 每个会话文件最多读多少字节找标题。
pub const HEAD_BYTES: usize = 64 * 1024;
/// 会话标题的数据层上限；这不是 UI 的视觉截断阈值。
/// 列表会按标题真实可用宽度做 ellipsis,正常会话名不会在数据层提前丢失。
pub const TITLE_STORAGE_CHARS: usize = 512;

#[derive(Debug, Clone)]
pub struct AiSession {
    pub source: AgentKind,
    /// resume 用的会话 id(claude:jsonl 文件名主干;codex:rollout 文件名末尾
    /// 的 uuid)。
    pub id: String,
    /// 列表主行。扫描阶段为空,[`scan`] 只为最终显示的那批读文件补齐。
    pub title: String,
    /// 副信息:claude 是项目目录(编码过);codex 是会话的 cwd。
    pub project: String,
    pub modified: SystemTime,
    path: PathBuf,
}

impl AiSession {
    /// 在终端里敲下去就能恢复这个会话的命令行。
    pub fn resume_command(&self) -> Option<String> {
        self.source.resume_command(&self.id)
    }

    /// 从这条历史会话创建独立分叉,不改变原会话。
    pub fn fork_command(&self) -> Option<String> {
        self.source.fork_command(&self.id)
    }

    /// hint 里的位置词。codex 有真实 cwd,取末段目录名;claude 只有编码过的
    /// 项目目录名(`D--temp-build-slTerminal`,分隔符和 `_` 都压成了 `-`,
    /// 不可逆),取最后一个 `-` 后的尾段——不完整但足够认出「是哪个项目」。
    pub fn place_label(&self) -> String {
        match self.source {
            AgentKind::Codex => Path::new(&self.project)
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default(),
            AgentKind::Claude => self.project.rsplit('-').next().unwrap_or("").to_owned(),
            _ => Path::new(&self.project)
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| self.project.clone()),
        }
    }

    fn fill_details(&mut self) {
        // claude 的项目目录就是 jsonl 的父目录名。
        if self.source == AgentKind::Claude && self.project.is_empty() {
            self.project = self
                .path
                .parent()
                .and_then(Path::file_name)
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
        }
        let Ok(head) = read_head(&self.path, HEAD_BYTES) else {
            self.ensure_title();
            return;
        };
        match self.source {
            AgentKind::Claude => {
                if let Some(title) = claude_title(&head) {
                    self.title = title;
                }
            }
            AgentKind::Codex => {
                let (title, cwd) = codex_details(&head);
                if let Some(title) = title {
                    self.title = title;
                }
                if let Some(cwd) = cwd {
                    self.project = cwd;
                }
            }
            _ => {}
        }
        self.ensure_title();
    }

    fn ensure_title(&mut self) {
        if self.title.is_empty() {
            let short = self.id.chars().take(8).collect::<String>();
            self.title = format!("{} 会话 {short}", self.source.display_name());
        }
    }
}

/// 扫描 CLI 原生档案。
pub fn scan(limit: usize) -> Vec<AiSession> {
    let Some(home) = crate::platform::dirs::home_dir() else {
        return Vec::new();
    };
    scan_at(&home, limit)
}

fn scan_at(home: &Path, limit: usize) -> Vec<AiSession> {
    let mut all = Vec::new();
    all.extend(scan_claude(home));
    all.extend(scan_codex(home));
    all.sort_by_key(|session| std::cmp::Reverse(session.modified));
    all.truncate(limit);
    for session in &mut all {
        session.fill_details();
    }
    all
}

/// 「3 分钟前 / 2 小时前 / 5 天前」式的相对时间,面板右侧的 hint 用。
pub fn relative_label(modified: SystemTime) -> String {
    relative_label_at(modified, SystemTime::now())
}

fn relative_label_at(modified: SystemTime, now: SystemTime) -> String {
    let elapsed = now.duration_since(modified).unwrap_or(Duration::ZERO);
    let minutes = elapsed.as_secs() / 60;
    match minutes {
        0 => "刚刚".to_owned(),
        1..=59 => format!("{minutes} 分钟前"),
        60..=1439 => format!("{} 小时前", minutes / 60),
        _ => format!("{} 天前", minutes / 1440),
    }
}

fn modified_of(path: &Path) -> SystemTime {
    path.metadata()
        .and_then(|metadata| metadata.modified())
        .unwrap_or(SystemTime::UNIX_EPOCH)
}

/// `~/.claude/projects/<编码目录>/<uuid>.jsonl`,一个文件一个会话。
fn scan_claude(home: &Path) -> Vec<AiSession> {
    let root = home.join(".claude").join("projects");
    let Ok(projects) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for project in projects.flatten() {
        let project_label = project.file_name().to_string_lossy().into_owned();
        let Ok(files) = std::fs::read_dir(project.path()) else {
            continue;
        };
        for file in files.flatten() {
            let path = file.path();
            if path
                .extension()
                .is_none_or(|extension| extension != "jsonl")
            {
                continue;
            }
            let Some(id) = path
                .file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
            else {
                continue;
            };
            out.push(AiSession {
                source: AgentKind::Claude,
                id,
                title: String::new(),
                project: project_label.clone(),
                modified: modified_of(&path),
                path,
            });
        }
    }
    out
}

/// `~/.codex/sessions/YYYY/MM/DD/rollout-<时间戳>-<uuid>.jsonl`。
fn scan_codex(home: &Path) -> Vec<AiSession> {
    let root = home.join(".codex").join("sessions");
    let mut out = Vec::new();
    // 目录按年/月/日三层嵌套;有限深度的手写递归比引一个 walkdir 轻。
    let mut stack = vec![(root, 0u8)];
    while let Some((dir, depth)) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if depth < 4 {
                    stack.push((path, depth + 1));
                }
                continue;
            }
            if path
                .extension()
                .is_none_or(|extension| extension != "jsonl")
            {
                continue;
            }
            let Some(id) = path
                .to_str()
                .and_then(crate::session::codex_rollout_id)
                .map(str::to_owned)
            else {
                continue;
            };
            out.push(AiSession {
                source: AgentKind::Codex,
                id,
                title: String::new(),
                project: String::new(),
                modified: modified_of(&path),
                path,
            });
        }
    }
    out
}

fn read_head(path: &Path, limit: usize) -> std::io::Result<String> {
    use std::io::Read;
    let mut file = std::fs::File::open(path)?;
    let mut buffer = vec![0u8; limit];
    let mut filled = 0;
    // read 不保证填满:读到 EOF 或缓冲满为止。
    loop {
        let read = file.read(&mut buffer[filled..])?;
        if read == 0 {
            break;
        }
        filled += read;
        if filled == buffer.len() {
            break;
        }
    }
    buffer.truncate(filled);
    Ok(String::from_utf8_lossy(&buffer).into_owned())
}

fn truncate_title(text: &str) -> String {
    let text = text.trim().replace(['\r', '\n'], " ");
    let mut out: String = text.chars().take(TITLE_STORAGE_CHARS).collect();
    if text.chars().count() > TITLE_STORAGE_CHARS {
        out.push('…');
    }
    out
}

/// CLI 自动注入的指令/上下文块——JSONL 里挂在 user 名下,看着像「第一条
/// 用户消息」,实际没有一个字是用户敲的。拿它们当标题,列表里就是一排
/// 「# AGENTS.md instructions <INSTRUCTIONS>…」(用户 08-02 截图的现场)。
fn looks_injected(text: &str) -> bool {
    text.starts_with('<')
        || text.starts_with("Caveat:")
        || text.starts_with("# AGENTS.md")
        || text.starts_with("# CLAUDE.md")
        || text.starts_with("[Request interrupted")
}

/// claude 会话的标题:优先 `summary` 行(压缩过的会话第一行就是),否则
/// 首条 user 消息的文本。头部截断出的半行 JSON 解析失败即跳过,天然安全。
fn claude_title(head: &str) -> Option<String> {
    for line in head.lines() {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        match value.get("type").and_then(|kind| kind.as_str()) {
            Some("summary") => {
                if let Some(summary) = value.get("summary").and_then(|summary| summary.as_str())
                    && !summary.trim().is_empty()
                {
                    return Some(truncate_title(summary));
                }
            }
            Some("user") => {
                let content = value.pointer("/message/content");
                let text = match content {
                    Some(serde_json::Value::String(text)) => Some(text.clone()),
                    Some(serde_json::Value::Array(parts)) => parts.iter().find_map(|part| {
                        (part.get("type").and_then(|kind| kind.as_str()) == Some("text"))
                            .then(|| part.get("text").and_then(|text| text.as_str()))
                            .flatten()
                            .map(str::to_owned)
                    }),
                    _ => None,
                };
                if let Some(text) = text {
                    let text = text.trim();
                    // isMeta 是 CLI 打在注入行上的标记(/clear 回显、上下文
                    // 快照等)——内容再像人话也不是用户说的;没打标记的再过
                    // 一遍 `looks_injected` 的形态筛。
                    let meta = value
                        .get("isMeta")
                        .and_then(|meta| meta.as_bool())
                        .unwrap_or(false);
                    if !meta && !text.is_empty() && !looks_injected(text) {
                        return Some(truncate_title(text));
                    }
                }
            }
            _ => {}
        }
    }
    None
}

/// codex rollout 的 (标题, cwd):session_meta 行给 cwd,首条 user 输入给标题。
fn codex_details(head: &str) -> (Option<String>, Option<String>) {
    let mut title = None;
    let mut cwd = None;
    for line in head.lines() {
        if title.is_some() && cwd.is_some() {
            break;
        }
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let Some(payload) = value.get("payload") else {
            continue;
        };
        if cwd.is_none()
            && let Some(dir) = payload.get("cwd").and_then(|cwd| cwd.as_str())
        {
            cwd = Some(dir.to_owned());
        }
        if title.is_none()
            && payload.get("role").and_then(|role| role.as_str()) == Some("user")
            && let Some(parts) = payload
                .get("content")
                .and_then(|content| content.as_array())
        {
            let text = parts.iter().find_map(|part| {
                (part.get("type").and_then(|kind| kind.as_str()) == Some("input_text"))
                    .then(|| part.get("text").and_then(|text| text.as_str()))
                    .flatten()
                    .map(str::to_owned)
            });
            if let Some(text) = text {
                let text = text.trim();
                if !text.is_empty() && !looks_injected(text) {
                    title = Some(truncate_title(text));
                }
            }
        }
    }
    (title, cwd)
}

#[cfg(test)]
mod session_scan_tests {
    use super::*;
    use std::time::UNIX_EPOCH;

    struct HomeEnvGuard {
        saved: [(&'static str, Option<std::ffi::OsString>); 2],
    }

    impl HomeEnvGuard {
        fn set(home: &Path) -> Self {
            let saved = ["USERPROFILE", "HOME"].map(|name| (name, std::env::var_os(name)));
            // 环境变量是进程级;测试固定串行,守卫在断言失败时也还原。
            for (name, _) in &saved {
                unsafe { std::env::set_var(name, home) };
            }
            Self { saved }
        }
    }

    impl Drop for HomeEnvGuard {
        fn drop(&mut self) {
            for (name, value) in &self.saved {
                match value {
                    Some(value) => unsafe { std::env::set_var(name, value) },
                    None => unsafe { std::env::remove_var(name) },
                }
            }
        }
    }

    struct FileRestoreGuard {
        path: PathBuf,
        original: Option<Vec<u8>>,
    }

    impl FileRestoreGuard {
        fn replace(path: PathBuf, contents: &str) -> Self {
            let original = std::fs::read(&path).ok();
            std::fs::write(&path, contents).unwrap();
            Self { path, original }
        }
    }

    impl Drop for FileRestoreGuard {
        fn drop(&mut self) {
            match &self.original {
                Some(contents) => {
                    let _ = std::fs::write(&self.path, contents);
                }
                None => {
                    let _ = std::fs::remove_file(&self.path);
                }
            }
        }
    }

    fn session(source: AgentKind, id: &str, project: &str, seconds: u64) -> AiSession {
        AiSession {
            source,
            id: id.to_owned(),
            title: String::new(),
            project: project.to_owned(),
            modified: UNIX_EPOCH + Duration::from_secs(seconds),
            path: PathBuf::from("native.jsonl"),
        }
    }

    #[test]
    fn a_compacted_claude_session_titles_from_its_summary_line() {
        let head = r#"{"type":"summary","summary":"修复编辑器身份条","leafUuid":"x"}
{"type":"user","message":{"content":"继续"}}"#;
        assert_eq!(claude_title(head), Some("修复编辑器身份条".to_owned()));
    }

    #[test]
    fn a_fresh_claude_session_titles_from_the_first_user_message() {
        let head = r#"{"type":"file-history-snapshot","messageId":"m1"}
{"type":"user","message":{"role":"user","content":[{"type":"text","text":"帮我查一下这个 panic 的根因"}]}}"#;
        assert_eq!(
            claude_title(head),
            Some("帮我查一下这个 panic 的根因".to_owned())
        );
    }

    #[test]
    fn injected_system_blocks_never_become_titles() {
        // 首条 user 是 <system-reminder> 注入时要跳过它取下一条真人消息。
        let head = r#"{"type":"user","message":{"content":"<system-reminder>ctx</system-reminder>"}}
{"type":"user","message":{"content":"真正的问题"}}"#;
        assert_eq!(claude_title(head), Some("真正的问题".to_owned()));
    }

    #[test]
    fn injected_instructions_and_meta_lines_never_become_titles() {
        // 用户 08-02 截图:列表里一排「# AGENTS.md instructions…」。这类
        // 注入块(还有打了 isMeta 的 /clear 回显)都挂在 user 名下,必须
        // 跳过去取后面第一条真人消息。
        let head = r##"{"type":"user","message":{"content":[{"type":"text","text":"# AGENTS.md instructions <INSTRUCTIONS> 【基础环境与规范】"}]}}
{"type":"user","isMeta":true,"message":{"content":"看着像人话的注入行"}}
{"type":"user","message":{"content":"Caveat: The messages below were generated..."}}
{"type":"user","message":{"content":"真正的第一句"}}"##;
        assert_eq!(claude_title(head), Some("真正的第一句".to_owned()));
    }

    #[test]
    fn all_five_injected_shapes_are_filtered() {
        for text in [
            "<system-reminder>ctx</system-reminder>",
            "Caveat: The messages below were generated",
            "# AGENTS.md instructions",
            "# CLAUDE.md project rules",
            "[Request interrupted by user]",
        ] {
            assert!(looks_injected(text), "{text}");
        }
        assert!(!looks_injected("正常用户输入"));
    }

    #[test]
    fn codex_agents_md_injection_is_skipped() {
        let head = r##"{"timestamp":"t","type":"session_meta","payload":{"id":"abc","cwd":"D:\\proj"}}
{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"# AGENTS.md instructions\n【基础环境与规范】"}]}}
{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"帮我看个 bug"}]}}"##;
        let (title, cwd) = codex_details(head);
        assert_eq!(title, Some("帮我看个 bug".to_owned()));
        assert_eq!(cwd, Some(r"D:\proj".to_owned()));
    }

    #[test]
    fn place_labels_shorten_to_something_recognizable() {
        let codex = session(
            AgentKind::Codex,
            "x",
            &Path::new("projects").join("slTerminal").to_string_lossy(),
            0,
        );
        assert_eq!(codex.place_label(), "slTerminal");
        let claude = AiSession {
            source: AgentKind::Claude,
            project: "D--temp-build-slTerminal".into(),
            ..codex
        };
        assert_eq!(claude.place_label(), "slTerminal");
    }

    #[test]
    fn a_truncated_trailing_line_is_ignored_not_fatal() {
        let head = "{\"type\":\"summary\",\"summary\":\"标题\"}\n{\"type\":\"user\",\"mess";
        assert_eq!(claude_title(head), Some("标题".to_owned()));
    }

    #[test]
    fn codex_rollout_yields_cwd_and_first_user_text() {
        let head = r#"{"timestamp":"t","type":"session_meta","payload":{"id":"abc","cwd":"D:\\proj"}}
{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"打包发布"}]}}"#;
        let (title, cwd) = codex_details(head);
        assert_eq!(title, Some("打包发布".to_owned()));
        assert_eq!(cwd, Some(r"D:\proj".to_owned()));
    }

    #[test]
    fn codex_ids_come_from_the_uuid_tail_only() {
        let path = Path::new(
            "/sessions/rollout-2026-08-01T12-30-00-0199a213-c2a4-7cf5-8f6b-d746fbb6e86c.jsonl",
        );
        assert_eq!(
            path.to_str().and_then(crate::session::codex_rollout_id),
            Some("0199a213-c2a4-7cf5-8f6b-d746fbb6e86c")
        );
        assert_eq!(
            Path::new("rollout-notes.jsonl")
                .to_str()
                .and_then(crate::session::codex_rollout_id),
            None
        );
    }

    #[test]
    fn resume_commands_match_each_clis_syntax() {
        let claude = session(AgentKind::Claude, "abc-123", "", 0);
        assert_eq!(
            claude.resume_command().as_deref(),
            Some("claude --resume abc-123")
        );
        assert_eq!(
            claude.fork_command().as_deref(),
            Some("claude --resume abc-123 --fork-session")
        );
        let codex = AiSession {
            source: AgentKind::Codex,
            ..claude
        };
        assert_eq!(
            codex.resume_command().as_deref(),
            Some("codex resume abc-123")
        );
        assert_eq!(codex.fork_command().as_deref(), Some("codex fork abc-123"));
    }

    #[test]
    fn native_scan_ignores_the_retired_session_registry() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("home");
        let project = home.join(".claude").join("projects").join("D--project");
        std::fs::create_dir_all(&project).unwrap();
        std::fs::write(
            project.join("native-session.jsonl"),
            r#"{"type":"summary","summary":"原生历史"}"#,
        )
        .unwrap();
        let _home = HomeEnvGuard::set(&home);
        // 测试构建的本地数据根位于进程临时根;假文件覆盖旧生产解析路径。
        let registry = crate::platform::dirs::local_dir().join("ai_sessions.json");
        assert!(
            registry.starts_with(std::env::temp_dir()),
            "测试注册表必须位于临时数据根"
        );
        let retired = r#"{"version":1,"sessions":[{"source":"claude","id":"retired-only","title":"旧注册表","last_seen":999}]}"#;
        let registry_file = FileRestoreGuard::replace(registry, retired);

        let sessions = scan(8);
        assert_eq!(
            sessions
                .iter()
                .map(|session| session.id.as_str())
                .collect::<Vec<_>>(),
            ["native-session"]
        );
        assert_eq!(sessions[0].title, "原生历史");
        assert_eq!(
            std::fs::read_to_string(&registry_file.path).unwrap(),
            retired
        );
    }

    #[test]
    fn titles_collapse_newlines_and_cap_length() {
        let long = "第一行\n第二行".to_owned() + &"字".repeat(TITLE_STORAGE_CHARS + 80);
        let title = truncate_title(&long);
        assert!(title.starts_with("第一行 第二行"));
        assert!(title.ends_with('…'));
        assert!(title.chars().count() <= TITLE_STORAGE_CHARS + 1);
    }

    #[test]
    fn only_the_head_of_a_large_transcript_is_read() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("large.jsonl");
        let mut text = String::from("{\"type\":\"summary\",\"summary\":\"头部标题\"}\n");
        text.push_str(&"x".repeat(HEAD_BYTES * 2));
        text.push_str("\n{\"type\":\"summary\",\"summary\":\"尾部标题\"}\n");
        std::fs::write(&path, text).unwrap();

        let head = read_head(&path, HEAD_BYTES).unwrap();
        assert!(head.len() <= HEAD_BYTES, "超过 64KB 头部预算");
        assert_eq!(claude_title(&head), Some("头部标题".to_owned()));
        assert!(!head.contains("尾部标题"));
    }

    #[test]
    fn relative_labels_bucket_by_minutes_hours_and_days() {
        let now = UNIX_EPOCH + Duration::from_secs(10 * 86_400);
        for (seconds, expected) in [
            (0, "刚刚"),
            (60, "1 分钟前"),
            (59 * 60, "59 分钟前"),
            (60 * 60, "1 小时前"),
            (1439 * 60, "23 小时前"),
            (1440 * 60, "1 天前"),
            (5 * 1440 * 60, "5 天前"),
        ] {
            let modified = now - Duration::from_secs(seconds);
            assert_eq!(relative_label_at(modified, now), expected, "{seconds}s");
        }
    }
}
