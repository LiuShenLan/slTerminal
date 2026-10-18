//! native hooks(codex native / cursor / copilot / grok)与结果元数据回归。
//!
//! 迁移裁剪:上游本文件中经 `crate::notify::Notification` 断言通知形态的
//! 部分整体归通知漏斗回补——本步只钉解析与仲裁语义;通知
//! 形态的消费点尚不存在,此处不留半套断言。
use super::*;
use crate::ai_agents::{AgentStatus, AgentStatusSource};
use lifecycle::AgentActivity;
use serde_json::{Value, json};

#[test]
fn non_successful_turns_are_classified_from_result_metadata() {
    // 通知形态断言(失败 vs 静默)归通知漏斗;此处只钉 AiTurnOutcome 分类。
    for (reason, expected) in [
        ("error", AiTurnOutcome::Failed),
        ("aborted", AiTurnOutcome::Cancelled),
        ("length", AiTurnOutcome::Incomplete),
        ("stop", AiTurnOutcome::Succeeded),
        // 宿主收尾的回合(计划菜单、已完成目标)以工具调用结束:正常完成,
        // 不是截断。
        ("toolUse", AiTurnOutcome::Succeeded),
    ] {
        let payload = serde_json::json!({"kind":"done", "stop_reason":reason});
        let event =
            parse_envelope(format!("slterm-hook/1 source=pi\n{payload}").as_bytes()).unwrap();
        assert_eq!(event.turn_outcome, expected, "{reason}");
    }
}

#[test]
fn pi_result_metadata_distinguishes_legacy_unknown_and_background_work() {
    for (metadata, outcome, status) in [
        ("", AiTurnOutcome::Unspecified, AgentStatus::Done),
        (
            ",\"stop_reason\":\"stop\"",
            AiTurnOutcome::Succeeded,
            AgentStatus::Done,
        ),
        (
            ",\"stop_reason\":\"error\"",
            AiTurnOutcome::Failed,
            AgentStatus::Idle,
        ),
        (
            ",\"stop_reason\":\"aborted\"",
            AiTurnOutcome::Cancelled,
            AgentStatus::Idle,
        ),
        (
            ",\"stop_reason\":\"length\"",
            AiTurnOutcome::Incomplete,
            AgentStatus::Idle,
        ),
        (
            ",\"stop_reason\":\"toolUse\"",
            AiTurnOutcome::Succeeded,
            AgentStatus::Done,
        ),
        (
            ",\"stop_reason\":\"future\"",
            AiTurnOutcome::Unknown,
            AgentStatus::Idle,
        ),
        (
            ",\"stop_reason\":null",
            AiTurnOutcome::Unknown,
            AgentStatus::Idle,
        ),
        (
            ",\"stop_reason\":42",
            AiTurnOutcome::Unknown,
            AgentStatus::Idle,
        ),
    ] {
        let wire = format!("slterm-hook/1 source=pi\n{{\"kind\":\"done\"{metadata}}}");
        let mut event = parse_envelope(wire.as_bytes()).unwrap();
        assert_eq!(event.turn_outcome, outcome);
        let mut activity = lifecycle::AgentActivity::default();
        assert!(activity.apply_hook(&event));
        assert_eq!(activity.status(), status);
        event.background_tasks = Some(AiBackgroundTasks {
            active: 1,
            total: 1,
        });
        assert!(activity.apply_hook(&event));
        assert_eq!(
            activity.status(),
            AgentStatus::Working,
            "在飞后台任务保持 Working"
        );
    }
}

fn codex(name: &str, session: &str, turn: Option<&str>) -> AiHookEvent {
    native(
        "full",
        json!({"hook_event_name":name,"session_id":session,"turn_id":turn}),
    )
}

fn native(mode: &str, payload: Value) -> AiHookEvent {
    let wire = format!("slterm-hook/1 source=codex codex_hooks={mode}\n{payload}");
    parse_envelope(wire.as_bytes()).unwrap()
}

#[test]
fn native_codex_owns_start_permission_recovery_interrupt_and_end() {
    let mut activity = AgentActivity::default();
    for (event, expected) in [
        ("SessionStart", AgentStatus::Idle),
        ("UserPromptSubmit", AgentStatus::Working),
        ("PermissionRequest", AgentStatus::Blocked),
        ("PostToolUse", AgentStatus::Working),
        ("Interrupt", AgentStatus::Idle),
    ] {
        let hook = codex(event, "native", Some("turn1"));
        assert!(hook.capabilities().lifecycle && hook.capabilities().attention_events);
        assert!(activity.apply_hook(&hook));
        assert_eq!(activity.status(), expected);
        assert_eq!(activity.source(), AgentStatusSource::Hook);
        assert!(!activity.allows_screen());
        if event == "Interrupt" {
            assert_eq!(hook.turn_outcome, AiTurnOutcome::Cancelled);
        }
    }
    assert!(activity.apply_hook(&codex("SessionEnd", "native", None)));
    assert!(!activity.hook_seen());
    assert!(!activity.apply_hook(&codex("PostToolUse", "native", Some("turn1"))));
}

#[test]
fn native_and_notify_coexist_without_duplicate_completion_or_capability_downgrade() {
    let notify = parse_envelope(
        b"slterm-hook/1 source=codex\n{\"type\":\"agent-turn-complete\",\"thread-id\":\"native\",\"turn-id\":\"turn1\"}",
    )
    .unwrap();
    let done = codex("Stop", "native", Some("turn1"));
    assert_eq!(notify.event_id, done.event_id);
    let mut activity = AgentActivity::default();
    activity.apply_hook(&codex("UserPromptSubmit", "native", Some("turn1")));
    assert!(!activity.apply_hook(&notify));
    assert!(activity.apply_hook(&done));
    assert!(!activity.apply_hook(&notify));
    activity.apply_hook(&codex("UserPromptSubmit", "native", Some("turn2")));
    assert!(!activity.apply_hook(&done), "旧回合不能完成新工作");
    assert_eq!(activity.status(), AgentStatus::Working);
    let mut reversed = AgentActivity::default();
    assert!(reversed.apply_hook(&notify));
    assert!(
        !reversed.apply_hook(&done),
        "同一回合只完成一次,即便 notify 赢了竞态"
    );
}

#[test]
fn older_native_contract_and_notify_retain_their_actual_capabilities() {
    let hook = native(
        "turns",
        json!({"hook_event_name":"SessionStart","session_id":"old"}),
    );
    assert!(hook.capabilities().lifecycle);
    assert!(!hook.capabilities().attention_events);
    assert!(!capabilities_for("codex").lifecycle);
    for name in ["SubagentStart", "SubagentStop", "invented"] {
        let wire = format!(
            "slterm-hook/1 source=codex codex_hooks=full\n{{\"hook_event_name\":\"{name}\",\"session_id\":\"main\"}}"
        );
        assert!(parse_envelope(wire.as_bytes()).is_none());
    }
}

#[test]
fn normal_tool_hooks_inside_a_codex_subagent_do_not_change_the_primary_turn() {
    for name in [
        "PermissionRequest",
        "PostToolUse",
        "Stop",
        "UserPromptSubmit",
    ] {
        let payload = json!({"hook_event_name":name,"session_id":"main","turn_id":"main-turn","agent_id":"child","agent_type":"worker"});
        let wire = format!("slterm-hook/1 source=codex codex_hooks=full\n{payload}");
        assert!(parse_envelope(wire.as_bytes()).is_none());
    }
}

#[test]
fn codex_native_hooks_resume_the_transcript_thread_not_the_runtime_group() {
    const THREAD: &str = "0199a213-c2a4-7cf5-8f6b-d746fbb6e86c";
    for root in [
        r"C:\Users\user\.codex\sessions",
        "/home/user/.codex/sessions",
    ] {
        let transcript = format!("{root}/2026/09/21/rollout-2026-09-21T10-00-00-{THREAD}.jsonl");
        for name in ["SessionStart", "UserPromptSubmit", "Stop"] {
            let event = native(
                "full",
                json!({
                    "hook_event_name": name,
                    "session_id": "runtime-group-with-no-rollout",
                    "transcript_path": transcript,
                }),
            );
            assert_eq!(event.session_id.as_deref(), Some(THREAD));
            assert_eq!(event.session_file.as_deref(), Some(transcript.as_str()));
            // 「session_file → codex resume 命令」的消费断言随会话恢复注入
            // (AgentSession)回补;rollout id 提取本身由 crate::session 单测钉。
        }
    }
    for transcript in [
        Value::Null,
        json!("relative/rollout-invalid.jsonl"),
        json!(42),
    ] {
        let event = native(
            "full",
            json!({
                "hook_event_name": "SessionStart", "session_id": "ephemeral",
                "transcript_path": transcript,
                "session_file": format!("/sessions/rollout-date-{THREAD}.jsonl"),
            }),
        );
        assert!(event.session_id.is_none(), "临时 group 绝不能变成恢复 id");
    }
    let legacy = codex("SessionStart", "older-native-id", None);
    assert_eq!(legacy.session_id.as_deref(), Some("older-native-id"));
}

#[test]
fn claude_transcript_is_retained_without_changing_its_native_session_id() {
    let event = parse_envelope(
        b"slterm-hook/1 source=claude\n{\"hook_event_name\":\"SessionStart\",\"session_id\":\"claude-id\",\"transcript_path\":\"/home/user/.claude/projects/project/claude-id.jsonl\"}",
    )
    .unwrap();
    assert_eq!(event.session_id.as_deref(), Some("claude-id"));
    assert_eq!(
        event.session_file.as_deref(),
        Some("/home/user/.claude/projects/project/claude-id.jsonl")
    );
}

#[test]
fn codex_user_questions_are_structured_attention_and_prose_in_tools_is_not() {
    // 安装器按 CodexHookMode 写出的 matcher 断言(`installation::codex_groups`)
    // 归 M4.3;此处只钉解析侧:`request_user_input` 的结构化提问是
    // NeedsAttention,工具散文不是。
    let payload = json!({"hook_event_name":"PreToolUse", "session_id":"main", "turn_id":"turn", "tool_name":"request_user_input", "tool_input":{"questions":[{"id":"scope","question":"Which scope should be used?","options":[{"label":"Current project"},{"label":"All projects"}]}]}});
    let event = native("full", payload.clone());
    let mut activity = AgentActivity::default();
    activity.apply_hook(&codex("UserPromptSubmit", "main", Some("turn")));
    assert!(activity.apply_hook(&event));
    assert_eq!(activity.status(), AgentStatus::Blocked);
    assert_eq!(
        event.message.as_deref(),
        Some("Which scope should be used?")
    );
    assert!(!activity.allows_screen());
    assert!(activity.apply_hook(&codex("PostToolUse", "main", Some("turn"))));
    assert_eq!(activity.status(), AgentStatus::Working);
    for (mode, body) in [
        ("turns", payload),
        (
            "full",
            json!({"hook_event_name":"PreToolUse", "tool_name":"Bash", "tool_input":{"command":"echo '[y/n]'"}}),
        ),
        (
            "full",
            json!({"hook_event_name":"PreToolUse", "tool_name":"request_user_input", "tool_input":{"questions":[]}}),
        ),
    ] {
        let wire = format!("slterm-hook/1 source=codex codex_hooks={mode}\n{body}");
        assert!(parse_envelope(wire.as_bytes()).is_none());
    }
}

#[test]
fn compaction_does_not_mark_an_active_turn_idle_and_native_stop_supplies_the_answer() {
    let mut activity = AgentActivity::default();
    activity.apply_hook(&codex("UserPromptSubmit", "main", Some("turn")));
    let compacted = native(
        "full",
        json!({"hook_event_name":"SessionStart","session_id":"main","source":"compact"}),
    );
    activity.apply_hook(&compacted);
    assert_eq!(activity.status(), AgentStatus::Working);
    let done = native(
        "full",
        json!({"hook_event_name":"Stop","session_id":"main","turn_id":"turn","last_assistant_message":"complete answer"}),
    );
    assert_eq!(
        done.answer.unwrap().source().unwrap().as_ref(),
        "complete answer"
    );
}

#[test]
fn kimi_failure_and_interrupt_have_distinct_outcomes() {
    // 通知形态断言(成功/失败/静默)归通知漏斗;此处钉 outcome 与错误文案。
    for (event_name, expected) in [
        ("Stop", AiTurnOutcome::Succeeded),
        ("StopFailure", AiTurnOutcome::Failed),
        ("Interrupt", AiTurnOutcome::Cancelled),
    ] {
        let raw = format!(
            "slterm-hook/1 source=kimi pane=3\n{}",
            json!({"hook_event_name":event_name, "session_id":"kimi-1", "error":"rate_limit"})
        );
        let event = parse_envelope(raw.as_bytes()).unwrap();
        assert_eq!(event.turn_outcome, expected);
        if expected == AiTurnOutcome::Failed {
            assert_eq!(event.message.as_deref(), Some("rate_limit"));
        }
    }
}

/// Lifecycle 覆盖下屏幕毫无发言权:排队输入、compaction 等画面再怎么像
/// 「该结束了」,也只有 native Stop 能收回合。(上游本例经 `ai_agents::detect`
/// 产屏幕观察;TOML 引擎归 M4.4,此处以等价的构造 Detection 钉同一仲裁语义。)
#[test]
fn queued_input_and_compaction_keep_native_turn_running() {
    use crate::ai_agents::{AgentKind, Detection};
    let screen = |status: AgentStatus| {
        Some(Detection {
            agent: AgentKind::Codex,
            status,
            rule_id: "test.chrome".into(),
        })
    };
    let mut activity = AgentActivity::default();
    activity.apply_hook(&codex("UserPromptSubmit", "main", Some("turn")));
    activity.input_sent();
    // 队列中/压缩中/纯空闲三帧:Lifecycle 覆盖下全部没有发言权。
    for status in [
        AgentStatus::Working,
        AgentStatus::Working,
        AgentStatus::Idle,
    ] {
        for _ in 0..8 {
            assert!(!activity.observe_screen(screen(status)));
            assert_eq!(activity.status(), AgentStatus::Working);
        }
    }
    let compact = native(
        "full",
        json!({
            "hook_event_name":"SessionStart", "session_id":"main", "source":"compact"
        }),
    );
    activity.apply_hook(&compact);
    assert_eq!(activity.status(), AgentStatus::Working);
    activity.apply_hook(&codex("Stop", "main", Some("turn")));
    assert_eq!(activity.status(), AgentStatus::Done);
}

#[test]
fn claude_question_tool_failure_and_terminal_failure_have_distinct_effects() {
    let mut activity = AgentActivity::default();
    let parse = |payload: Value| {
        parse_envelope(format!("slterm-hook/1 source=claude\n{payload}").as_bytes()).unwrap()
    };
    let question = parse(
        json!({"hook_event_name":"PreToolUse", "tool_name":"AskUserQuestion",
        "tool_input":{"questions":[{"question":"Choose a scope","options":[{"label":"Current"}]}]}}),
    );
    assert_eq!(question.kind, AiHookKind::NeedsAttention);
    activity.apply_hook(&question);
    assert_eq!(activity.status(), AgentStatus::Blocked);
    let tool_failure = parse(
        json!({"hook_event_name":"PostToolUseFailure", "tool_name":"Bash", "error":"exit 1"}),
    );
    activity.apply_hook(&tool_failure);
    assert_eq!(
        activity.status(),
        AgentStatus::Working,
        "agent 可以从工具失败中恢复"
    );
    let failed = parse(json!({"hook_event_name":"StopFailure", "error":"rate_limit"}));
    activity.apply_hook(&failed);
    assert_eq!(activity.status(), AgentStatus::Idle);
    assert_eq!(failed.turn_outcome, AiTurnOutcome::Failed);
    let success = parse(json!({"hook_event_name":"Stop"}));
    activity.apply_hook(&success);
    assert_eq!(activity.status(), AgentStatus::Done);
}
