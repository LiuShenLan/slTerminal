//! 助手回答投影。
//!
//! 本模块落地范围:`AssistantAnswer` 三态本体(hook 事件协议解析的
//! 直接产出物)+ `AnswerInbox` 按 `(provider, session)` 收编归属 +
//! `conversation` 有界 transcript 投影。挂 pane 宿主归工作区,展示归阅读器;
//! 原则不变:不从屏幕猜测回答,只投影 provider 给出的原文。

use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

use serde_json::Value;

// 有界 transcript 投影(消费归阅读器);本模块仅落投影与预算契约。
#[allow(dead_code)]
mod conversation;

pub const MAX_ANSWER_BYTES: usize = 128 * 1024;

/// 回合完成时 hook 携带的回答原文。三态语义:
/// - `Complete`:provider 给了完整原文,逐字投影;
/// - `Missing`:字段缺席或空白,绝不退化成屏幕猜测;
/// - `TooLarge`:原文超上限,报尺寸而不是截断(截断会把代码/公式拦腰切断,
///   比「没有投影」更难发现)。
#[derive(Clone, PartialEq, Eq, Hash)]
pub enum AssistantAnswer {
    Complete(Arc<str>),
    Missing,
    TooLarge { bytes: usize },
}

impl AssistantAnswer {
    /// 按 source 与事件名取字段:每家 provider 的回答字段名与事件形状不同,
    /// 取不到/取错名一律 `Missing`,不从屏幕上猜。
    pub fn from_hook(source: &str, payload: &Value) -> Option<Self> {
        let field = match source {
            "claude" if payload.get("hook_event_name")?.as_str()? == "Stop" => {
                "last_assistant_message"
            }
            "codex" if payload.get("hook_event_name").and_then(Value::as_str) == Some("Stop") => {
                "last_assistant_message"
            }
            "codex"
                if payload.get("type").and_then(Value::as_str) == Some("agent-turn-complete") =>
            {
                "last-assistant-message"
            }
            _ => return None,
        };
        Some(match payload.get(field).and_then(Value::as_str) {
            Some(text) if text.len() > MAX_ANSWER_BYTES => Self::TooLarge { bytes: text.len() },
            Some(text) if !text.trim().is_empty() => Self::Complete(Arc::from(text)),
            _ => Self::Missing,
        })
    }

    pub fn source(&self) -> Option<&Arc<str>> {
        match self {
            Self::Complete(source) => Some(source),
            _ => None,
        }
    }

    pub fn notice(&self) -> Option<String> {
        match self {
            Self::Complete(_) => None,
            Self::Missing => Some("未收到完整回答原文;保留终端内容,不从屏幕猜测。".into()),
            Self::TooLarge { bytes } => Some(format!(
                "回答原文共 {bytes} 字节,超过 {} KiB 阅读上限;未截断渲染,请在终端查看。",
                MAX_ANSWER_BYTES / 1024
            )),
        }
    }
}

/// 回答原文绝不进 Debug 日志:内容可能含代码与选区正文,日志只记尺寸。
impl fmt::Debug for AssistantAnswer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Complete(source) => formatter
                .debug_struct("Complete")
                .field("bytes", &source.len())
                .finish_non_exhaustive(),
            Self::Missing => formatter.write_str("Missing"),
            Self::TooLarge { bytes } => formatter
                .debug_struct("TooLarge")
                .field("bytes", bytes)
                .finish(),
        }
    }
}

/// 展示用回答快照:归属 + 单调接收序 + 内容 + 可选工作目录。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnswerSnapshot {
    pub provider: String,
    pub session_id: String,
    pub received_sequence: u64,
    pub content: AssistantAnswer,
    pub cwd: Option<PathBuf>,
}

/// 每 pane 一个的回答收件箱:按 `(provider, session)` 归属回答,
/// `received_sequence` 单调去旧,已关闭会话不再接收迟到回答。
#[derive(Default)]
pub struct AnswerInbox {
    owner: Option<(String, String)>,
    closed: bool,
    last_sequence: u64,
    pub latest: Option<AnswerSnapshot>,
}

impl AnswerInbox {
    /// 收编一条 hook 事件;返回是否产出了新的回答快照。
    pub fn observe(&mut self, event: &crate::ai_hook::AiHookEvent, pane_id: u64) -> bool {
        use crate::ai_hook::AiHookKind;

        // pane 与 provider 双重归属:焦点兜底(pane=None)事件不得供阅读器
        // 正文,否则两个 pane 的回答会串台。
        if event.pane != Some(pane_id)
            || !matches!(event.source.as_str(), "claude" | "codex")
            || event.received_sequence <= self.last_sequence
        {
            return false;
        }
        let Some(session_id) = event.session_id.as_deref().filter(|id| !id.is_empty()) else {
            return false;
        };
        let identity = (event.source.clone(), session_id.to_owned());
        let starts_session = matches!(
            event.kind,
            AiHookKind::SessionStart | AiHookKind::PromptSubmit
        );
        // 会话切换后,旧会话的迟到回答不跨绑;只有显式开始新会话能换 owner。
        if self.owner.as_ref().is_some_and(|owner| owner != &identity) && !starts_session {
            return false;
        }
        if self.closed && !starts_session {
            return false;
        }
        if self.owner.as_ref() != Some(&identity) || (starts_session && self.closed) {
            self.latest = None;
        }
        self.owner = Some(identity);
        self.last_sequence = event.received_sequence;
        if starts_session {
            self.closed = false;
        }
        if event.kind == AiHookKind::SessionEnd {
            self.closed = true;
        }
        if let Some(content) = event.answer.clone() {
            self.latest = Some(AnswerSnapshot {
                provider: event.source.clone(),
                session_id: session_id.to_owned(),
                received_sequence: event.received_sequence,
                content,
                cwd: event.answer_cwd.clone(),
            });
            return true;
        }
        false
    }

    /// 会话关闭:之后的迟到回答不再收编,直到下一次 SessionStart。
    pub fn close(&mut self) {
        self.closed = self.owner.is_some();
    }

    /// 新命令开始:清 owner,但保留上一条回答(不提前擦除阅读器内容)。
    pub fn begin_command(&mut self) {
        self.owner = None;
        self.closed = false;
    }
}

#[cfg(test)]
mod answer_tests {
    use super::*;
    use serde_json::json;

    fn payload(provider: &str, text: &str) -> Value {
        match provider {
            "claude" => json!({"hook_event_name": "Stop", "last_assistant_message": text}),
            "codex" => json!({"type": "agent-turn-complete", "last-assistant-message": text}),
            _ => unreachable!(),
        }
    }

    #[test]
    fn both_providers_preserve_complete_utf8_and_math_verbatim() {
        let source = format!(
            "{}\n$$\n\\widehat{{f}}(\\xi)\n=\n+ \\int_0^1 x\\,dx\n\\begin{{cases}}a&b\\\\\nc&d\\end{{cases}}\n$$\n",
            "中文与 $HOME 和代码 `$$`。".repeat(500)
        );
        for provider in ["claude", "codex"] {
            let answer = AssistantAnswer::from_hook(provider, &payload(provider, &source)).unwrap();
            assert_eq!(answer.source().unwrap().as_ref(), source);
        }
    }

    #[test]
    fn oversized_answers_are_reported_not_cut_into_partial_formulas() {
        for provider in ["claude", "codex"] {
            let source = "a".repeat(MAX_ANSWER_BYTES + 1);
            let answer = AssistantAnswer::from_hook(provider, &payload(provider, &source)).unwrap();
            assert_eq!(
                answer,
                AssistantAnswer::TooLarge {
                    bytes: source.len()
                }
            );
            assert!(answer.source().is_none());
            assert!(answer.notice().unwrap().contains("未截断"));
        }
    }

    #[test]
    fn absent_wrongly_named_or_empty_fields_never_become_screen_guesses() {
        for provider in ["claude", "codex"] {
            assert_eq!(
                AssistantAnswer::from_hook(provider, &payload(provider, " \n")),
                Some(AssistantAnswer::Missing)
            );
        }
        assert_eq!(
            AssistantAnswer::from_hook(
                "claude",
                &json!({"hook_event_name": "Stop", "last-assistant-message": "wrong"})
            ),
            Some(AssistantAnswer::Missing)
        );
        assert_eq!(
            AssistantAnswer::from_hook(
                "codex",
                &json!({"type": "agent-turn-complete", "last_assistant_message": "wrong"})
            ),
            Some(AssistantAnswer::Missing)
        );
        assert!(
            AssistantAnswer::from_hook(
                "claude",
                &json!({"hook_event_name": "Notification", "last_assistant_message": "wrong"})
            )
            .is_none()
        );
    }

    #[test]
    fn debug_output_never_contains_answer_text() {
        let answer = AssistantAnswer::Complete(Arc::from("private-source-secret"));
        assert!(!format!("{answer:?}").contains("private-source-secret"));
        assert!(AssistantAnswer::Missing.notice().is_some());
    }

    fn event(
        provider: &str,
        session: &str,
        pane: u64,
        sequence: u64,
        kind: crate::ai_hook::AiHookKind,
    ) -> crate::ai_hook::AiHookEvent {
        use crate::ai_hook::AiHookKind;
        let body = if provider == "claude" {
            json!({
                "hook_event_name": "Stop",
                "session_id": session,
                "last_assistant_message": "answer",
            })
        } else {
            json!({
                "type": "agent-turn-complete",
                "thread-id": session,
                "last-assistant-message": "answer",
            })
        };
        let wire = format!("slterm-hook/1 source={provider} pane={pane}\n{body}");
        let mut event = crate::ai_hook::parse_envelope(wire.as_bytes()).unwrap();
        event.received_sequence = sequence;
        event.kind = kind;
        if kind != AiHookKind::TurnDone {
            event.answer = None;
        }
        event
    }

    #[test]
    fn panes_sessions_and_closed_sessions_cannot_cross_bind_answers() {
        use crate::ai_hook::AiHookKind::*;
        for provider in ["claude", "codex"] {
            let mut inbox = AnswerInbox::default();
            assert!(!inbox.observe(&event(provider, "old", 2, 1, TurnDone), 1));
            assert!(inbox.observe(&event(provider, "old", 1, 2, TurnDone), 1));
            inbox.observe(&event(provider, "new", 1, 3, SessionStart), 1);
            assert!(inbox.latest.is_none());
            assert!(!inbox.observe(&event(provider, "old", 1, 4, TurnDone), 1));
            assert!(inbox.observe(&event(provider, "new", 1, 5, TurnDone), 1));
            inbox.observe(&event(provider, "new", 1, 6, SessionEnd), 1);
            assert!(!inbox.observe(&event(provider, "new", 1, 7, TurnDone), 1));
            assert_eq!(inbox.latest.as_ref().unwrap().received_sequence, 5);
        }
    }

    #[test]
    fn fallback_focus_and_out_of_order_events_do_not_supply_reader_content() {
        use crate::ai_hook::AiHookKind::TurnDone;
        let mut inbox = AnswerInbox::default();
        let mut missing_pane = event("codex", "thread", 1, 1, TurnDone);
        missing_pane.pane = None;
        assert!(!inbox.observe(&missing_pane, 1));
        assert!(inbox.observe(&event("codex", "thread", 1, 3, TurnDone), 1));
        assert!(!inbox.observe(&event("codex", "thread", 1, 2, TurnDone), 1));
        inbox.close();
        assert!(!inbox.observe(&event("codex", "thread", 1, 4, TurnDone), 1));
    }

    #[test]
    fn new_cli_command_accepts_another_codex_session_without_erasing_last_answer_early() {
        use crate::ai_hook::AiHookKind::TurnDone;
        let mut inbox = AnswerInbox::default();
        assert!(inbox.observe(&event("codex", "old", 1, 1, TurnDone), 1));
        inbox.close();
        inbox.begin_command();
        assert_eq!(inbox.latest.as_ref().unwrap().session_id, "old");
        assert!(inbox.observe(&event("codex", "new", 1, 2, TurnDone), 1));
        assert_eq!(inbox.latest.as_ref().unwrap().session_id, "new");
        assert!(!inbox.observe(&event("codex", "old", 1, 3, TurnDone), 1));
    }

    #[test]
    fn image_directory_is_captured_with_the_completed_answer() {
        use crate::ai_hook::AiHookKind::TurnDone;
        let directory = tempfile::tempdir().unwrap();
        for provider in ["claude", "codex"] {
            let mut completion = event(provider, "session", 1, 1, TurnDone);
            completion.answer_cwd = Some(directory.path().to_path_buf());
            let mut inbox = AnswerInbox::default();
            assert!(inbox.observe(&completion, 1));
            completion.answer_cwd = None;
            assert_eq!(
                inbox.latest.as_ref().unwrap().cwd.as_deref(),
                Some(directory.path())
            );
        }
    }
}
