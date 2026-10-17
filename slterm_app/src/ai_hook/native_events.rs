//! 把事件名放在 helper argv 里的 native 生命周期 hook(cursor/copilot/grok)。
use serde_json::Value;

use super::payload::{attention_message, context_string};
use super::{AiHookKind, AiTurnOutcome};

pub(super) fn parse(
    source: &str,
    event: &str,
    payload: &Value,
) -> Option<(AiHookKind, Option<String>)> {
    let kind = match event {
        "session-start" => AiHookKind::SessionStart,
        "prompt" => AiHookKind::PromptSubmit,
        "tool-complete" => AiHookKind::ToolComplete,
        "done" => AiHookKind::TurnDone,
        "failed" if source == "grok" => AiHookKind::TurnDone,
        "error"
            if source == "copilot" && payload.get("recoverable") == Some(&Value::Bool(false)) =>
        {
            AiHookKind::TurnDone
        }
        "session-end" => AiHookKind::SessionEnd,
        "notification" => {
            let category =
                context_string(payload, &["notification_type", "notificationType", "type"]);
            let actionable = category
                .as_deref()
                .is_some_and(|value| value == "elicitation_dialog" || value == "permission_prompt");
            if !actionable {
                return None;
            }
            return Some((AiHookKind::NeedsAttention, attention_message(payload)));
        }
        _ => return None,
    };
    Some((kind, None))
}

pub(super) fn outcome(source: &str, event: &str, payload: &Value) -> AiTurnOutcome {
    if matches!(event, "failed" | "error") {
        return AiTurnOutcome::Failed;
    }
    match source {
        "cursor" => match payload.get("status").and_then(Value::as_str) {
            Some("completed") => AiTurnOutcome::Succeeded,
            Some("aborted") => AiTurnOutcome::Cancelled,
            Some("error") => AiTurnOutcome::Failed,
            _ => AiTurnOutcome::Unknown,
        },
        "copilot" => match payload.get("stopReason").and_then(Value::as_str) {
            Some("end_turn") => AiTurnOutcome::Succeeded,
            _ => AiTurnOutcome::Unknown,
        },
        "grok" if event == "done" => AiTurnOutcome::Succeeded,
        _ => AiTurnOutcome::Unknown,
    }
}
