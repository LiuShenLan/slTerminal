//! Document 外部变化请求、共享冲突与位置重绑。

use std::fmt;
use std::sync::Arc;
use std::time::Instant;

use slterm_path::ResolvedPath;

use crate::{DocumentBody, DocumentId, DocumentMetadata, DocumentRegistry, TextStateId};

/// 后台刷新只消费这份基线快照；迟到结果不得作用于新正文或新位置。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocumentExternalRequest {
    document_id: DocumentId,
    text_revision: u64,
    text_state: TextStateId,
    disk_baseline: Arc<[u8]>,
    location: ResolvedPath,
    source_generation: Option<u64>,
    authorization_generation: u64,
}

impl DocumentExternalRequest {
    pub const fn document_id(&self) -> DocumentId {
        self.document_id
    }

    pub const fn text_revision(&self) -> u64 {
        self.text_revision
    }

    pub const fn text_state(&self) -> TextStateId {
        self.text_state
    }

    pub fn disk_baseline(&self) -> &Arc<[u8]> {
        &self.disk_baseline
    }

    pub fn location(&self) -> &ResolvedPath {
        &self.location
    }

    pub const fn source_generation(&self) -> Option<u64> {
        self.source_generation
    }

    pub const fn authorization_generation(&self) -> u64 {
        self.authorization_generation
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExternalConflictKind {
    Modified,
    Deleted,
}

/// 所有引用视图共享的外部冲突事实；修改冲突携带独立只读磁盘快照。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocumentConflict {
    kind: ExternalConflictKind,
    disk_baseline: Option<Arc<[u8]>>,
    location: ResolvedPath,
}

impl DocumentConflict {
    pub fn modified(location: ResolvedPath, disk_baseline: Arc<[u8]>) -> Self {
        Self {
            kind: ExternalConflictKind::Modified,
            disk_baseline: Some(disk_baseline),
            location,
        }
    }

    pub fn modified_paged(location: ResolvedPath) -> Self {
        Self {
            kind: ExternalConflictKind::Modified,
            disk_baseline: None,
            location,
        }
    }

    pub fn deleted(location: ResolvedPath) -> Self {
        Self {
            kind: ExternalConflictKind::Deleted,
            disk_baseline: None,
            location,
        }
    }

    pub const fn kind(&self) -> ExternalConflictKind {
        self.kind
    }

    pub fn disk_baseline(&self) -> Option<&Arc<[u8]>> {
        self.disk_baseline.as_ref()
    }

    pub fn location(&self) -> &ResolvedPath {
        &self.location
    }
}

#[derive(Clone, Debug)]
pub enum DocumentExternalChange {
    Reload {
        request: DocumentExternalRequest,
        content: DocumentBody,
        disk_baseline: Arc<[u8]>,
        metadata: DocumentMetadata,
        location: ResolvedPath,
        new_state: TextStateId,
    },
    Conflict {
        request: DocumentExternalRequest,
        conflict: DocumentConflict,
    },
    Deleted(DocumentExternalRequest),
    ReadRevoked {
        document_id: DocumentId,
    },
    Relocated {
        request: DocumentExternalRequest,
        location: ResolvedPath,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExternalApplyOutcome {
    Reloaded,
    Conflict,
    Deleted,
    ReadRevoked,
    Relocated,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExternalChangeError {
    DocumentNotFound(DocumentId),
    ReadRevoked,
    StaleExternalChange,
    RevisionExhausted,
    SourceGenerationExhausted,
    StateConflict,
    TargetOccupied,
    IdentityMismatch,
}

impl fmt::Display for ExternalChangeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::DocumentNotFound(_) => "Document 不存在",
            Self::ReadRevoked => "Document 读取授权已撤销",
            Self::StaleExternalChange => "外部变化结果已过期",
            Self::RevisionExhausted => "文本 revision 已耗尽",
            Self::SourceGenerationExhausted => "分页源代次已耗尽",
            Self::StateConflict => "外部重载复用了既有 TextStateId",
            Self::TargetOccupied => "目标位置已关联其他 Document",
            Self::IdentityMismatch => "重定位目标不是原文件",
        })
    }
}

impl std::error::Error for ExternalChangeError {}

impl DocumentRegistry {
    pub fn external_request(
        &self,
        id: DocumentId,
        authorization_generation: u64,
    ) -> Result<DocumentExternalRequest, ExternalChangeError> {
        let session = self
            .session(id)
            .ok_or(ExternalChangeError::DocumentNotFound(id))?;
        if session.read_revoked {
            return Err(ExternalChangeError::ReadRevoked);
        }
        Ok(DocumentExternalRequest {
            document_id: session.id,
            text_revision: session.text_revision,
            text_state: session.current_state,
            disk_baseline: session.disk_baseline.clone(),
            location: session.location.clone(),
            source_generation: session.paged().map(|source| source.source_generation()),
            authorization_generation,
        })
    }

    fn validate_external_request(
        &self,
        request: &DocumentExternalRequest,
    ) -> Result<(), ExternalChangeError> {
        let session = self
            .session(request.document_id)
            .ok_or(ExternalChangeError::DocumentNotFound(request.document_id))?;
        if session.text_revision != request.text_revision
            || session.current_state != request.text_state
            || session.disk_baseline != request.disk_baseline
            || session.paged().map(|source| source.source_generation()) != request.source_generation
        {
            return Err(ExternalChangeError::StaleExternalChange);
        }
        Ok(())
    }

    pub fn apply_external(
        &mut self,
        change: DocumentExternalChange,
        at: Instant,
    ) -> Result<ExternalApplyOutcome, ExternalChangeError> {
        match change {
            DocumentExternalChange::ReadRevoked { document_id } => {
                let session = self
                    .session_mut(document_id)
                    .ok_or(ExternalChangeError::DocumentNotFound(document_id))?;
                session.read_revoked = true;
                Ok(ExternalApplyOutcome::ReadRevoked)
            }
            DocumentExternalChange::Deleted(request) => {
                self.validate_external_request(&request)?;
                let conflict = DocumentConflict::deleted(request.location.clone());
                let session = self.sessions_mut(request.document_id)?;
                session.external_conflict = Some(conflict);
                Ok(ExternalApplyOutcome::Deleted)
            }
            DocumentExternalChange::Conflict { request, conflict } => {
                self.validate_external_request(&request)?;
                let session = self.sessions_mut(request.document_id)?;
                session.external_conflict = Some(conflict);
                Ok(ExternalApplyOutcome::Conflict)
            }
            DocumentExternalChange::Relocated { request, location } => {
                self.validate_external_request(&request)?;
                if self
                    .existing(&location)
                    .is_some_and(|existing| existing != request.document_id)
                {
                    return Err(ExternalChangeError::TargetOccupied);
                }
                match (request.location.file_identity(), location.file_identity()) {
                    (Some(original), Some(current)) if original == current => {}
                    (None, _) => {}
                    _ => return Err(ExternalChangeError::IdentityMismatch),
                }
                let id = request.document_id;
                let session = self.sessions_mut(id)?;
                session.location = location;
                session.external_conflict = None;
                self.rebind_path(id);
                let identity = self
                    .session(id)
                    .and_then(|session| session.location.file_identity());
                self.refresh_identity(id, identity);
                Ok(ExternalApplyOutcome::Relocated)
            }
            DocumentExternalChange::Reload {
                request,
                content,
                disk_baseline,
                metadata,
                location,
                new_state,
            } => {
                self.validate_external_request(&request)?;
                let id = request.document_id;
                if request.location.canonical_path() != location.canonical_path() {
                    if self
                        .existing(&location)
                        .is_some_and(|existing| existing != id)
                    {
                        return Err(ExternalChangeError::TargetOccupied);
                    }
                    match (request.location.file_identity(), location.file_identity()) {
                        (Some(original), Some(current)) if original == current => {}
                        (None, _) => {}
                        _ => return Err(ExternalChangeError::IdentityMismatch),
                    }
                }
                let session = self.sessions_mut(id)?;
                if session.is_dirty() || session.is_saving() || session.external_conflict.is_some()
                {
                    session.external_conflict = Some(match &content {
                        DocumentBody::Editable(_) => {
                            DocumentConflict::modified(location, disk_baseline)
                        }
                        DocumentBody::Paged(_) => DocumentConflict::modified_paged(location),
                    });
                    return Ok(ExternalApplyOutcome::Conflict);
                }
                if session.known_states.contains(&new_state) {
                    return Err(ExternalChangeError::StateConflict);
                }
                let content_changed = match (&session.body, &content) {
                    (DocumentBody::Editable(current), DocumentBody::Editable(next)) => {
                        current != next
                    }
                    (DocumentBody::Paged(current), DocumentBody::Paged(next)) => {
                        current.index() != next.index()
                            || current.fingerprint() != next.fingerprint()
                    }
                    _ => true,
                };
                if content_changed && session.text_revision == u64::MAX {
                    return Err(ExternalChangeError::RevisionExhausted);
                }
                let previous_identity = session.location.file_identity();
                let location_changed = session.location != location;
                if content_changed {
                    let next_generation = if matches!(content, DocumentBody::Paged(_)) {
                        Some(
                            session
                                .paged_generation
                                .checked_add(1)
                                .ok_or(ExternalChangeError::SourceGenerationExhausted)?,
                        )
                    } else {
                        None
                    };
                    let next_body = match content {
                        DocumentBody::Editable(text) => DocumentBody::Editable(text),
                        DocumentBody::Paged(source) => DocumentBody::Paged(
                            source.with_generation(next_generation.expect("分页代次已分配")),
                        ),
                    };
                    match (&session.body, &next_body) {
                        (DocumentBody::Editable(before), DocumentBody::Editable(after)) => {
                            session.history.record_reload(
                                before,
                                after,
                                session.current_state,
                                new_state,
                                at,
                            );
                        }
                        _ => session.history.reset(),
                    }
                    session.body = next_body;
                    if let Some(generation) = next_generation {
                        session.paged_generation = generation;
                    }
                    session.current_state = new_state;
                    session.saved_state = new_state;
                    session.known_states.insert(new_state);
                    session.text_revision += 1;
                } else {
                    session.saved_state = session.current_state;
                    session.history.barrier();
                }
                session.disk_baseline = disk_baseline;
                session.metadata = metadata;
                session.location = location;
                session.external_conflict = None;
                let current_identity = session.location.file_identity();
                if location_changed {
                    self.rebind_path(id);
                }
                if previous_identity != current_identity {
                    self.refresh_identity(id, current_identity);
                }
                Ok(ExternalApplyOutcome::Reloaded)
            }
        }
    }

    fn sessions_mut(
        &mut self,
        id: DocumentId,
    ) -> Result<&mut crate::DocumentSession, ExternalChangeError> {
        self.session_mut(id)
            .ok_or(ExternalChangeError::DocumentNotFound(id))
    }
}
