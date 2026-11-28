//! Runtime 分页续页缓存：不可变全文复用、revision 验真与游标切片。

use std::sync::Arc;

use slterm_runtime_protocol::{
    DomainErrorCode, GitDiffResponse, JsonRpcError, JsonRpcId, JsonRpcResponse, RuntimeResult,
    TerminalReadResponse,
};
use slterm_workspace::ProjectId;

use super::{RuntimeCallbacks, TerminalStatusTarget, stale_cursor_error};
use crate::local_control::git::GitDiffSnapshot;
use crate::local_control::reads::{
    CachedReadText, CursorSnapshot, PAGE_BUDGET_BYTES, ReadGeneration, TerminalReadTarget,
    TerminalTextSnapshot, cursor_part, git_cache_key, read_page, terminal_cache_key,
};

/// 无 Git diff 端口时的续页验真占位。
pub(super) fn not_ready_git_diff_probe(
    _: Option<ProjectId>,
    _: String,
) -> Result<u64, JsonRpcError> {
    Err(JsonRpcError::new(
        -32001,
        "git diff not ready",
        Some(slterm_runtime_protocol::JsonRpcErrorData {
            code: DomainErrorCode::NotReady,
        }),
    ))
}

/// 从缓存全文切出一页；游标失配或编码失败返回 None，由调用方走完整读取。
fn cached_page(
    cached: &CachedReadText,
    resource: u64,
    cursor: Option<&str>,
    budget: usize,
) -> Option<(String, Option<String>)> {
    let page = read_page(
        cached.text.as_bytes(),
        ReadGeneration {
            resource,
            generation: cached.generation,
            revision: cached.revision,
        },
        cursor,
        budget,
    )
    .ok()?;
    let data = std::str::from_utf8(&page.bytes).ok()?.to_owned();
    Some((data, page.next_cursor))
}

/// 终端续页：先用最近帧 revision 证明快照仍有效，再从缓存切片。
pub(super) fn cached_terminal_read(
    runtime: &RuntimeCallbacks,
    id: JsonRpcId,
    target: &TerminalReadTarget,
    terminal_id_text: &str,
    cursor_resource: u64,
    cursor: Option<&str>,
) -> Option<JsonRpcResponse<RuntimeResult>> {
    let cursor = cursor?;
    let part = cursor_part(cursor)?;
    if part.resource != cursor_resource {
        return Some(JsonRpcResponse::error(id, stale_cursor_error()));
    }
    let key = terminal_cache_key(
        target.terminal_id,
        target.agent_id.as_deref(),
        part.snapshot,
    );
    let cached = match runtime.read_cache.lock() {
        Ok(mut cache) => cache.get(&key),
        Err(_) => None,
    }?;
    match (runtime.wait_status)(TerminalStatusTarget {
        terminal_id: target.terminal_id,
        generation: target.generation,
        agent_id: target.agent_id.clone(),
    }) {
        Ok(current) if current == part.snapshot.revision => {
            let (data, next_cursor) =
                cached_page(&cached, cursor_resource, Some(cursor), PAGE_BUDGET_BYTES)?;
            Some(JsonRpcResponse::result(
                id,
                RuntimeResult::TerminalRead(TerminalReadResponse {
                    terminal_id: terminal_id_text.to_owned(),
                    generation: cached.generation,
                    revision: cached.revision,
                    data,
                    next_cursor,
                }),
            ))
        }
        Ok(_) => Some(JsonRpcResponse::error(id, stale_cursor_error())),
        Err(_) => {
            log::debug!("terminal revision probe unavailable; falling back to full read");
            None
        }
    }
}

/// 完整读取后登记不可变全文；后续续页直接切片。
pub(super) fn store_terminal_read(
    runtime: &RuntimeCallbacks,
    target: &TerminalReadTarget,
    cursor_resource: u64,
    snapshot: &TerminalTextSnapshot,
) {
    let Ok(mut cache) = runtime.read_cache.lock() else {
        return;
    };
    cache.insert(
        terminal_cache_key(
            target.terminal_id,
            target.agent_id.as_deref(),
            CursorSnapshot {
                generation: snapshot.generation,
                revision: snapshot.revision,
            },
        ),
        Arc::new(CachedReadText {
            resource: cursor_resource,
            generation: snapshot.generation,
            revision: snapshot.revision,
            source_fingerprint: 0,
            project_id: None,
            text: snapshot.text.clone(),
        }),
    );
}

/// Git diff 续页：源字节指纹不变时直接复用不可变文本，不重算差分。
pub(super) fn cached_git_diff(
    runtime: &RuntimeCallbacks,
    id: JsonRpcId,
    project_id: Option<ProjectId>,
    path: &str,
    cursor: Option<&str>,
) -> Option<JsonRpcResponse<RuntimeResult>> {
    let cursor = cursor?;
    let part = cursor_part(cursor)?;
    let key = git_cache_key(project_id, path, part.snapshot);
    let cached = match runtime.read_cache.lock() {
        Ok(mut cache) => cache.get(&key),
        Err(_) => None,
    }?;
    if cached.resource != part.resource {
        return None;
    }
    match (runtime.git_diff_probe)(project_id, path.to_owned()) {
        Ok(fingerprint) if fingerprint == cached.source_fingerprint => {
            let (data, next_cursor) =
                cached_page(&cached, part.resource, Some(cursor), PAGE_BUDGET_BYTES)?;
            Some(JsonRpcResponse::result(
                id,
                RuntimeResult::GitDiff(GitDiffResponse {
                    project_id: cached
                        .project_id
                        .map_or_else(String::new, |project| project.to_string()),
                    path: path.to_owned(),
                    revision: cached.revision,
                    data,
                    next_cursor,
                }),
            ))
        }
        Ok(_) => Some(JsonRpcResponse::error(id, stale_cursor_error())),
        Err(_) => {
            log::debug!("git diff probe unavailable; falling back to full diff read");
            None
        }
    }
}

/// 完整差分后登记快照；请求缺省项目与显式项目各登记一次。
pub(super) fn store_git_diff(
    runtime: &RuntimeCallbacks,
    requested_project: Option<ProjectId>,
    path: &str,
    project_id: ProjectId,
    snapshot: &GitDiffSnapshot,
    resource: u64,
) {
    let Ok(mut cache) = runtime.read_cache.lock() else {
        return;
    };
    let key_snapshot = CursorSnapshot {
        generation: 0,
        revision: snapshot.revision,
    };
    let value = Arc::new(CachedReadText {
        resource,
        generation: 0,
        revision: snapshot.revision,
        source_fingerprint: snapshot.source_fingerprint,
        project_id: Some(project_id),
        text: snapshot.text.clone(),
    });
    cache.insert(
        git_cache_key(Some(project_id), path, key_snapshot),
        value.clone(),
    );
    if requested_project != Some(project_id) {
        cache.insert(git_cache_key(requested_project, path, key_snapshot), value);
    }
}
