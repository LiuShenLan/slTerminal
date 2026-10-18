//! Runtime 写方法的审计日志。
//!
//! 只记录方法、目标与长度，不插值 prompt/paste 正文或 exec argv 内容；
//! request_id 用于与调用方回执对账。读方法不进入本模块，避免把高频轮询
//! 写进日志。

use super::super::RuntimeCommand;

pub(super) fn audit_command(request_id: &str, command: &RuntimeCommand) {
    match command {
        RuntimeCommand::SendKey {
            window_id,
            pane_id,
            key,
            modifiers,
            repeat,
        } => {
            log::info!(
                "runtime pane.send_key request_id={request_id} window_id={window_id:?} pane_id={pane_id} key={} shift={} alt={} control={} repeat={repeat}",
                key.as_str(),
                modifiers.shift,
                modifiers.alt,
                modifiers.control
            );
        }
        RuntimeCommand::AgentStart {
            window_id,
            pane_id,
            name,
            kind,
            session_id,
            worktree,
            ..
        } => {
            log::info!(
                "runtime agent.start request_id={request_id} window_id={window_id:?} pane_id={pane_id:?} name={name} kind={} resume={} worktree={}",
                kind.slug(),
                session_id.is_some(),
                worktree.is_some()
            );
        }
        RuntimeCommand::AgentFork { name, kind, .. } => {
            log::info!(
                "runtime agent.fork request_id={request_id} name={name} kind={}",
                kind.slug()
            );
        }
        RuntimeCommand::AgentPrompt {
            agent,
            generation,
            text,
            submit,
        } => {
            log::info!(
                "runtime agent.prompt request_id={request_id} agent={agent} generation={generation:?} submit={submit} prompt_bytes={}",
                text.len()
            );
        }
        RuntimeCommand::AgentPaste {
            agent,
            generation,
            text,
            submit,
        } => {
            log::info!(
                "runtime agent.paste request_id={request_id} agent={agent} generation={generation:?} submit={submit} paste_bytes={}",
                text.len()
            );
        }
        RuntimeCommand::AgentRead {
            agent,
            generation,
            lines,
        } => {
            log::info!(
                "runtime agent.read request_id={request_id} agent={agent} generation={generation:?} lines={lines}"
            );
        }
        RuntimeCommand::Exec {
            window_id,
            pane_id,
            argv,
            timeout_ms,
            max_output_bytes,
        } => {
            log::info!(
                "runtime pane.exec request_id={request_id} window_id={window_id:?} pane_id={pane_id} argv_len={} timeout_ms={timeout_ms} max_output_bytes={max_output_bytes}",
                argv.len()
            );
        }
        _ => {}
    }
}
