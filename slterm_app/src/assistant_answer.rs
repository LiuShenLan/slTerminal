//! 助手回答投影(03 篇锚点模块)。
//!
//! 本步(M4.2)落地范围:`AssistantAnswer` 三态本体——hook 事件协议解析
//! (`ai_hook::protocol`)的直接产出物。`AnswerInbox` 收编归属与
//! `conversation` 有界投影归 M4.4 续写本文件;挂 pane 宿主归 05 篇。

use std::fmt;
use std::sync::Arc;

use serde_json::Value;

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
}
