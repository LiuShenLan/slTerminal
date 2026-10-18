//! 有界原生 transcript 投影。无历史目录扫描,无「屏幕 → 消息」猜测。
//!
//! 只读本地原生会话文件:头部 256KB 用于身份校验,分页窗口 1MB;单消息
//! 48KB、输出 128KB、消息数 160 条封顶。SSH/WSL 远程读取臂随平台裁剪
//! 整支不迁;消费归阅读器。

use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use serde::Serialize;
use serde_json::Value;

use crate::ai_agents::AgentKind;

const HEAD_BYTES: usize = 256 * 1024;
const PAGE_BYTES: usize = 1024 * 1024;
const MESSAGE_BYTES: usize = 48 * 1024;
const OUTPUT_BYTES: usize = 128 * 1024;
const MESSAGE_COUNT: usize = 160;

/// 投影前必须先校验的身份:CLI 类型 + 原生会话 id。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ConversationIdentity {
    pub source: AgentKind,
    pub session_id: String,
}

/// 一页投影结果。`before` 是继续向前的游标;`truncated` 显式携带(命中
/// 任一预算即真),调用方不得把截断当完整历史。
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(super) struct Conversation {
    pub messages: Vec<Message>,
    pub before: Option<u64>,
    pub truncated: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(super) struct Message {
    pub id: String,
    pub role: &'static str,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    pub complete: bool,
    pub truncated: bool,
    #[serde(skip)]
    paired: bool,
    #[serde(skip)]
    event: bool,
}

struct Capture {
    head: Vec<u8>,
    bytes: Vec<u8>,
    start: u64,
    end: u64,
}

/// 投影本地原生会话文件的最新一页;身份与路径不一致时拒绝投影。
pub(super) fn project(path: &Path, identity: &ConversationIdentity) -> Option<Conversation> {
    let capture = capture_file(path, None)?;
    if !validate_source(identity, path, &capture.head) {
        return None;
    }
    let (messages, before, truncated) = project_records(identity, &capture)?;
    Some(Conversation {
        messages,
        before: before.filter(|cursor| *cursor < capture.end),
        truncated,
    })
}

fn capture_file(path: &Path, before: Option<u64>) -> Option<Capture> {
    let mut file = File::open(path).ok()?;
    let metadata = file.metadata().ok()?;
    if !metadata.is_file() {
        return None;
    }
    let end = before.unwrap_or(metadata.len());
    if end > metadata.len() {
        // 请求的游标超出当前文件:文件已变更,拒绝给半页。
        return None;
    }
    let start = end.saturating_sub(PAGE_BYTES as u64);
    let mut head = Vec::new();
    (&mut file)
        .take(HEAD_BYTES as u64)
        .read_to_end(&mut head)
        .ok()?;
    file.seek(SeekFrom::Start(start.saturating_sub(1))).ok()?;
    let mut bytes = Vec::new();
    (&mut file)
        .take(end - start.saturating_sub(1))
        .read_to_end(&mut bytes)
        .ok()?;
    // 读取期间文件被追加/改写时放弃本页:revision 语义不完整时不投影。
    let after = file.metadata().ok()?;
    if metadata.len() != after.len() || metadata.modified().ok() != after.modified().ok() {
        return None;
    }
    Some(Capture {
        head,
        bytes,
        start,
        end,
    })
}

/// 身份校验:codex 首行 session_meta 的 id、claude 文件名主干必须与身份一致。
fn validate_source(identity: &ConversationIdentity, path: &Path, head: &[u8]) -> bool {
    match identity.source {
        AgentKind::Codex => head
            .split(|byte| *byte == b'\n')
            .next()
            .and_then(|line| serde_json::from_slice::<Value>(line).ok())
            .is_some_and(|record| {
                record["type"] == "session_meta" && record["payload"]["id"] == identity.session_id
            }),
        AgentKind::Claude => {
            path.file_name()
                .and_then(|name| name.to_str())
                .and_then(|name| name.strip_suffix(".jsonl"))
                == Some(identity.session_id.as_str())
        }
        _ => false,
    }
}

fn project_records(
    identity: &ConversationIdentity,
    capture: &Capture,
) -> Option<(Vec<Message>, Option<u64>, bool)> {
    let mut bytes = capture.bytes.as_slice();
    let mut offset = capture.start;
    if capture.start > 0 {
        if bytes.first() == Some(&b'\n') {
            bytes = &bytes[1..];
        } else if let Some(end) = bytes.iter().position(|byte| *byte == b'\n') {
            bytes = &bytes[end + 1..];
            offset = capture.start - 1 + end as u64 + 1;
        } else {
            return Some((Vec::new(), Some(capture.start), true));
        }
    }
    let cursor = (offset > 0).then_some(offset);
    let mut messages = Vec::new();
    let mut calls = HashMap::new();
    let mut truncated = false;
    for line in bytes.split_inclusive(|byte| *byte == b'\n') {
        let id = offset.to_string();
        offset += line.len() as u64;
        // CLI 正在追加的最后半行等待下次读取,不把不完整 JSON 当作对话内容。
        if !line.ends_with(b"\n") {
            break;
        }
        let Ok(record) = serde_json::from_slice::<Value>(line) else {
            truncated = true;
            continue;
        };
        if identity.source == AgentKind::Codex {
            codex(&record, &id, &mut messages, &mut calls);
        } else {
            claude(&record, identity, &id, &mut messages, &mut calls)?;
        }
    }
    let mut kept = 0;
    let mut first = messages.len();
    let length = messages.len();
    for (index, message) in messages.iter_mut().enumerate().rev() {
        message.truncated |= trim(&mut message.text, MESSAGE_BYTES);
        if let Some(detail) = &mut message.detail {
            message.truncated |= trim(detail, MESSAGE_BYTES);
        }
        let size = message.text.len() + message.detail.as_ref().map_or(0, String::len);
        if kept + size > OUTPUT_BYTES || length - first >= MESSAGE_COUNT {
            break;
        }
        kept += size;
        first = index;
    }
    let next = if first > 0 {
        truncated = true;
        messages
            .get(first)
            .and_then(|message| message.id.split(':').next()?.parse().ok())
            .or(cursor)
    } else {
        cursor
    };
    messages.drain(..first);
    Some((messages, next, truncated))
}

fn trim(text: &mut String, maximum: usize) -> bool {
    if text.len() <= maximum {
        return false;
    }
    let mut end = maximum;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text.truncate(end);
    true
}

fn text_content(value: &Value) -> String {
    if let Some(text) = value.as_str() {
        return text.to_owned();
    }
    value
        .as_array()
        .map(|parts| {
            parts
                .iter()
                .filter_map(|part| {
                    matches!(
                        part["type"].as_str(),
                        Some("text" | "Text" | "input_text" | "output_text")
                    )
                    .then(|| part["text"].as_str())
                    .flatten()
                })
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
}

fn push_message(
    messages: &mut Vec<Message>,
    id: String,
    role: &'static str,
    text: String,
    event: bool,
) {
    if text.trim().is_empty() {
        return;
    }
    // codex 同时落 response_item 与 event_msg;只合并互为副本的两种来源,
    // 保留用户重复发送。
    if let Some(previous) = messages.iter_mut().rev().find(|message| {
        message.role == role && message.text == text && message.event != event && !message.paired
    }) {
        previous.paired = true;
        return;
    }
    messages.push(Message {
        id,
        role,
        text,
        name: None,
        detail: None,
        complete: true,
        truncated: false,
        paired: false,
        event,
    });
}

fn tool(
    messages: &mut Vec<Message>,
    calls: &mut HashMap<String, usize>,
    id: String,
    call: &str,
    name: &str,
    input: String,
) {
    calls.insert(call.to_owned(), messages.len());
    messages.push(Message {
        id,
        role: "tool",
        text: input,
        name: Some(name.to_owned()),
        detail: None,
        complete: false,
        truncated: false,
        paired: false,
        event: false,
    });
}

fn result(
    messages: &mut Vec<Message>,
    calls: &HashMap<String, usize>,
    id: String,
    call: &str,
    output: String,
) {
    if let Some(message) = calls.get(call).and_then(|index| messages.get_mut(*index)) {
        message.detail = Some(output);
        message.complete = true;
    } else {
        messages.push(Message {
            id,
            role: "tool",
            text: String::new(),
            name: None,
            detail: Some(output),
            complete: true,
            truncated: false,
            paired: false,
            event: false,
        });
    }
}

fn codex(
    record: &Value,
    id: &str,
    messages: &mut Vec<Message>,
    calls: &mut HashMap<String, usize>,
) {
    let payload = &record["payload"];
    match (record["type"].as_str(), payload["type"].as_str()) {
        (Some("response_item"), Some("message"))
            if payload["role"] == "assistant" && payload["channel"] != "analysis" =>
        {
            // 原始 user 记录还包含环境和 AGENTS 上下文;真实输入以用户消息
            // 事件为准,不按正文前缀过滤,避免误删用户主动发送的 XML/Markdown。
            push_message(
                messages,
                id.into(),
                "assistant",
                text_content(&payload["content"]),
                false,
            );
        }
        (Some("event_msg"), Some("item_completed")) => {
            let item = &payload["item"];
            let role = match item["type"].as_str() {
                Some("UserMessage") => "user",
                Some("AgentMessage") if item["phase"] != "analysis" => "assistant",
                _ => return,
            };
            push_message(
                messages,
                id.into(),
                role,
                text_content(&item["content"]),
                true,
            );
        }
        (Some("event_msg"), Some("user_message")) => push_message(
            messages,
            id.into(),
            "user",
            payload["message"].as_str().unwrap_or_default().into(),
            true,
        ),
        (Some("event_msg"), Some("agent_message")) => push_message(
            messages,
            id.into(),
            "assistant",
            payload["message"].as_str().unwrap_or_default().into(),
            true,
        ),
        (Some("response_item"), Some("function_call" | "custom_tool_call")) => {
            tool(
                messages,
                calls,
                id.into(),
                payload["call_id"].as_str().unwrap_or(id),
                payload["name"].as_str().unwrap_or("tool"),
                payload["arguments"]
                    .as_str()
                    .or_else(|| payload["input"].as_str())
                    .unwrap_or_default()
                    .into(),
            );
        }
        (Some("response_item"), Some("function_call_output" | "custom_tool_call_output")) => {
            result(
                messages,
                calls,
                id.into(),
                payload["call_id"].as_str().unwrap_or_default(),
                text_content(&payload["output"]),
            );
        }
        _ => {}
    }
}

fn claude(
    record: &Value,
    identity: &ConversationIdentity,
    id: &str,
    messages: &mut Vec<Message>,
    calls: &mut HashMap<String, usize>,
) -> Option<()> {
    let role = match record["type"].as_str() {
        Some("user") => "user",
        Some("assistant") => "assistant",
        _ => return Some(()),
    };
    if record["isSidechain"] == true {
        return Some(());
    }
    if record["sessionId"] != identity.session_id {
        // 记录属于别的会话:整页拒绝,不混投影。
        return None;
    }
    let content = &record["message"]["content"];
    if let Some(text) = content.as_str() {
        push_message(messages, id.into(), role, text.into(), false);
    }
    if let Some(parts) = content.as_array() {
        for (index, part) in parts.iter().enumerate() {
            let block_id = format!("{id}:{index}");
            match part["type"].as_str() {
                Some("text") => push_message(
                    messages,
                    block_id,
                    role,
                    part["text"].as_str().unwrap_or_default().into(),
                    false,
                ),
                Some("tool_use") => tool(
                    messages,
                    calls,
                    block_id,
                    part["id"].as_str().unwrap_or(id),
                    part["name"].as_str().unwrap_or("tool"),
                    serde_json::to_string_pretty(&part["input"]).unwrap_or_default(),
                ),
                Some("tool_result") => result(
                    messages,
                    calls,
                    block_id,
                    part["tool_use_id"].as_str().unwrap_or_default(),
                    text_content(&part["content"]),
                ),
                _ => {}
            }
        }
    }
    Some(())
}

#[cfg(test)]
mod conversation_tests {
    use super::*;
    use serde_json::json;
    use std::io::Write;

    const THREAD: &str = "0199a213-c2a4-7cf5-8f6b-d746fbb6e86c";

    fn identity(source: AgentKind) -> ConversationIdentity {
        ConversationIdentity {
            source,
            session_id: THREAD.into(),
        }
    }

    fn fixture(
        identity: &ConversationIdentity,
        records: &[Value],
    ) -> (tempfile::TempDir, std::path::PathBuf) {
        let directory = tempfile::tempdir().unwrap();
        let path = directory
            .path()
            .join(format!("{}.jsonl", identity.session_id));
        let mut file = File::create(&path).unwrap();
        for record in records {
            writeln!(file, "{record}").unwrap();
        }
        (directory, path)
    }

    #[test]
    fn native_codex_messages_deduplicate_events_and_keep_tools_without_reasoning() {
        let identity = identity(AgentKind::Codex);
        let (_dir, path) = fixture(
            &identity,
            &[
                json!({"type":"session_meta","payload":{"id":identity.session_id}}),
                json!({"type":"event_msg","payload":{"type":"user_message","message":"中文测试🙂"}}),
                json!({"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"中文测试🙂"}]}}),
                json!({"type":"response_item","payload":{"type":"reasoning","summary":[{"text":"private reasoning"}]}}),
                json!({"type":"response_item","payload":{"type":"function_call","name":"exec","call_id":"tool-1","arguments":"{\"cmd\":\"pwd\"}"}}),
                json!({"type":"response_item","payload":{"type":"function_call_output","call_id":"tool-1","output":"/project"}}),
                json!({"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"## 回答\n`done`"}]}}),
                json!({"type":"event_msg","payload":{"type":"agent_message","message":"## 回答\n`done`"}}),
                json!({"type":"event_msg","payload":{"type":"user_message","message":"中文测试🙂"}}),
            ],
        );
        let conversation = project(&path, &identity).unwrap();
        assert_eq!(conversation.messages.len(), 4);
        assert_eq!(conversation.messages[0].text, "中文测试🙂");
        assert_eq!(conversation.messages[1].detail.as_deref(), Some("/project"));
        assert!(conversation.messages[1].complete);
        assert_eq!(conversation.messages[2].role, "assistant");
        assert!(conversation.before.is_none());
        assert!(
            !serde_json::to_string(&conversation.messages)
                .unwrap()
                .contains("private reasoning")
        );
        let mut other = identity.clone();
        other.session_id = "another-thread".into();
        assert!(project(&path, &other).is_none());
    }

    #[test]
    fn codex_completed_items_hide_injected_context_but_preserve_real_markup_and_repeats() {
        let identity = identity(AgentKind::Codex);
        let input = "# AGENTS.md instructions\n<environment_context>用户正文</environment_context>";
        let (_dir, path) = fixture(
            &identity,
            &[
                json!({"type":"session_meta","payload":{"id":identity.session_id}}),
                json!({"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"<environment_context>internal fixture context</environment_context>"}]}}),
                json!({"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":input}]}}),
                json!({"type":"event_msg","payload":{"type":"item_completed","item":{"type":"UserMessage","content":[{"type":"text","text":input}]}}}),
                json!({"type":"event_msg","payload":{"type":"item_completed","item":{"type":"Reasoning","raw_content":["private reasoning"]}}}),
                json!({"type":"event_msg","payload":{"type":"item_completed","item":{"type":"AgentMessage","phase":"final_answer","content":[{"type":"Text","text":"## 回答"}]}}}),
                json!({"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"## 回答"}]}}),
                json!({"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":input}]}}),
                json!({"type":"event_msg","payload":{"type":"item_completed","item":{"type":"UserMessage","content":[{"type":"text","text":input}]}}}),
            ],
        );
        let conversation = project(&path, &identity).unwrap();
        assert!(!conversation.truncated);
        assert_eq!(conversation.messages.len(), 3);
        assert_eq!(conversation.messages[0].role, "user");
        assert_eq!(conversation.messages[0].text, input);
        assert_eq!(conversation.messages[1].role, "assistant");
        assert_eq!(conversation.messages[1].text, "## 回答");
        assert_eq!(conversation.messages[2].text, input);
        assert_ne!(conversation.messages[0].id, conversation.messages[2].id);
    }

    #[test]
    fn codex_legacy_user_events_do_not_promote_raw_context_or_analysis() {
        let identity = identity(AgentKind::Codex);
        let (_dir, path) = fixture(
            &identity,
            &[
                json!({"type":"session_meta","payload":{"id":identity.session_id}}),
                json!({"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"internal context"}]}}),
                json!({"type":"event_msg","payload":{"type":"user_message","message":"real question"}}),
                json!({"type":"response_item","payload":{"type":"message","role":"assistant","channel":"analysis","content":[{"type":"output_text","text":"private reasoning"}]}}),
                json!({"type":"event_msg","payload":{"type":"item_completed","item":{"type":"AgentMessage","phase":"analysis","content":[{"type":"Text","text":"private reasoning"}]}}}),
                json!({"type":"event_msg","payload":{"type":"agent_message","message":"answer"}}),
            ],
        );
        let conversation = project(&path, &identity).unwrap();
        assert_eq!(
            conversation
                .messages
                .iter()
                .map(|message| message.text.as_str())
                .collect::<Vec<_>>(),
            ["real question", "answer"]
        );
    }

    #[test]
    fn claude_records_require_the_owner_and_partial_writes_wait_for_a_newline() {
        let identity = identity(AgentKind::Claude);
        let (_dir, path) = fixture(
            &identity,
            &[
                json!({"type":"user","sessionId":identity.session_id,"message":{"content":"question"}}),
                json!({"type":"assistant","sessionId":identity.session_id,"message":{"content":[{"type":"text","text":"answer"},{"type":"tool_use","id":"x","name":"Read","input":{"path":"src/main.rs"}}]}}),
                json!({"type":"user","sessionId":identity.session_id,"message":{"content":[{"type":"tool_result","tool_use_id":"x","content":[{"type":"text","text":"file text"}]}]}}),
            ],
        );
        File::options()
            .append(true)
            .open(&path)
            .unwrap()
            .write_all(b"{\"type\":\"assistant\"")
            .unwrap();
        let conversation = project(&path, &identity).unwrap();
        assert_eq!(conversation.messages.len(), 3);
        assert_eq!(
            conversation.messages[2].detail.as_deref(),
            Some("file text")
        );
        let mut other = identity.clone();
        other.session_id = "other".into();
        assert!(project(&path, &other).is_none());
    }

    #[test]
    fn pages_obey_head_page_message_and_output_budgets() {
        let identity = identity(AgentKind::Codex);
        // 超过 1MB 的填充 + 200 条短消息:分页窗口与消息数双预算都必须生效。
        let mut records = vec![json!({"type":"session_meta","payload":{"id":identity.session_id}})];
        records.push(json!({"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"🙂".repeat(400_000)}]}}));
        for index in 0..200 {
            records.push(json!({"type":"event_msg","payload":{"type":"agent_message","message":format!("message-{index:03}-{}", "x".repeat(512))}}));
        }
        let (_dir, path) = fixture(&identity, &records);
        let capture = capture_file(&path, None).unwrap();
        assert!(capture.head.len() <= HEAD_BYTES, "头部 capture 超预算");
        assert!(capture.bytes.len() <= PAGE_BYTES + 1, "分页 capture 超预算");

        let conversation = project(&path, &identity).unwrap();
        assert!(!conversation.messages.is_empty());
        assert!(conversation.messages.len() <= MESSAGE_COUNT);
        let output: usize = conversation
            .messages
            .iter()
            .map(|message| message.text.len() + message.detail.as_ref().map_or(0, String::len))
            .sum();
        assert!(output <= OUTPUT_BYTES, "输出超预算: {output}");
        assert!(conversation.truncated);
        assert!(conversation.before.is_some());
    }

    #[test]
    fn oversized_messages_are_trimmed_on_char_boundaries() {
        let identity = identity(AgentKind::Codex);
        let text = "中🙂".repeat(100_000);
        let (_dir, path) = fixture(
            &identity,
            &[
                json!({"type":"session_meta","payload":{"id":identity.session_id}}),
                json!({"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":text}]}}),
            ],
        );
        let conversation = project(&path, &identity).unwrap();
        let message = &conversation.messages[0];
        assert!(message.truncated);
        assert!(message.text.len() <= MESSAGE_BYTES);
        assert!(text.starts_with(&message.text));
        assert!(message.text.is_char_boundary(message.text.len()));
    }

    #[test]
    fn identity_must_match_the_path_or_file_header() {
        let identity = identity(AgentKind::Claude);
        let (_dir, path) = fixture(&identity, &[]);
        assert!(
            project(
                &path,
                &ConversationIdentity {
                    source: AgentKind::Codex,
                    session_id: identity.session_id.clone(),
                }
            )
            .is_none()
        );
        assert!(
            project(
                &path,
                &ConversationIdentity {
                    source: AgentKind::Aider,
                    session_id: identity.session_id.clone(),
                }
            )
            .is_none()
        );
    }
}
