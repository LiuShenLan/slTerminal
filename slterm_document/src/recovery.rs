//! Document 异常恢复与显式重授权的纯状态转换。

use std::fmt;
use std::sync::Arc;

use slterm_path::ResolvedPath;

use crate::{
    DocumentBody, DocumentConflict, DocumentId, DocumentMetadata, DocumentOpenOutcome,
    DocumentRegistry, DocumentRegistryError, TextStateId,
};

/// 启动恢复输入；只恢复文本与必要元数据，不恢复撤销树。
pub struct DocumentRecovery {
    pub location: ResolvedPath,
    pub body: DocumentBody,
    pub saved_state: TextStateId,
    pub current_state: TextStateId,
    pub disk_baseline: Arc<[u8]>,
    pub metadata: DocumentMetadata,
    pub read_revoked: bool,
    pub external_conflict: Option<DocumentConflict>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReauthorizationError {
    DocumentNotFound(DocumentId),
    NotReadRevoked,
    GenerationExhausted,
    LocationConflict(DocumentId),
}

impl fmt::Display for ReauthorizationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DocumentNotFound(_) => formatter.write_str("Document 不存在"),
            Self::NotReadRevoked => formatter.write_str("Document 当前未处于失权状态"),
            Self::GenerationExhausted => formatter.write_str("Document 外部变化代次已耗尽"),
            Self::LocationConflict(_) => formatter.write_str("重授权目标已关联其他 Document"),
        }
    }
}

impl std::error::Error for ReauthorizationError {}

impl DocumentRegistry {
    pub fn restore(
        &mut self,
        id: DocumentId,
        recovery: DocumentRecovery,
    ) -> Result<(DocumentId, DocumentOpenOutcome), DocumentRegistryError> {
        let DocumentRecovery {
            location,
            body,
            saved_state,
            current_state,
            disk_baseline,
            metadata,
            read_revoked,
            external_conflict,
        } = recovery;
        if self.existing(&location).is_some() {
            return Err(DocumentRegistryError::RecoveryTargetOccupied(id));
        }
        let result = self.open(id, location, body, current_state, disk_baseline, metadata)?;
        if result.1 == DocumentOpenOutcome::Opened {
            let session = self.session_mut(id).expect("新恢复 Document 必须存在");
            session.saved_state = saved_state;
            session.known_states.insert(saved_state);
            session.read_revoked = read_revoked;
            session.external_conflict = external_conflict;
        }
        Ok(result)
    }

    /// 只有显式重授权可清除 ReadRevoked；正文、TextStateId 与撤销历史不变。
    pub fn reauthorize(
        &mut self,
        id: DocumentId,
        location: ResolvedPath,
        disk_baseline: Arc<[u8]>,
        external_conflict: Option<DocumentConflict>,
    ) -> Result<(), ReauthorizationError> {
        if let Some(existing) = self.existing(&location)
            && existing != id
        {
            return Err(ReauthorizationError::LocationConflict(existing));
        }
        let session = self
            .session_mut(id)
            .ok_or(ReauthorizationError::DocumentNotFound(id))?;
        if !session.read_revoked {
            return Err(ReauthorizationError::NotReadRevoked);
        }
        let Some(next) = session.external_generation.checked_add(1) else {
            return Err(ReauthorizationError::GenerationExhausted);
        };
        session.external_generation = next;
        session.location = location;
        session.disk_baseline = disk_baseline;
        session.external_conflict = external_conflict;
        session.read_revoked = false;
        self.rebind_path(id);
        self.refresh_identity(
            id,
            self.session(id)
                .and_then(|session| session.location().file_identity()),
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr as _;
    use std::sync::Arc;

    use ropey::Rope;
    use slterm_path::{CanonicalPath, FileIdentity, ResolvedPath};

    use crate::{
        DocumentConflict, DocumentId, DocumentMetadata, DocumentRecovery, DocumentRegistry,
        DocumentRegistryError, EditApplyOutcome, ExternalChangeError, ReauthorizationError,
        TextStateId,
    };

    fn id() -> DocumentId {
        DocumentId::from_str("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2a").unwrap()
    }

    fn state(last: char) -> TextStateId {
        TextStateId::from_str(&format!("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0f2{last}")).unwrap()
    }

    fn location(identity: Option<u128>) -> ResolvedPath {
        ResolvedPath::new(
            CanonicalPath::new("D:/outside/file.txt").unwrap(),
            identity.map(|file| FileIdentity { volume: 7, file }),
        )
    }

    #[test]
    fn recovered_document_preserves_state_ids_without_undo_history() {
        let mut registry = DocumentRegistry::new();
        registry
            .restore(
                id(),
                DocumentRecovery {
                    location: location(None),
                    body: Rope::from_str("draft").into(),
                    saved_state: state('a'),
                    current_state: state('b'),
                    disk_baseline: Arc::default(),
                    metadata: DocumentMetadata::default(),
                    read_revoked: true,
                    external_conflict: None,
                },
            )
            .unwrap();

        let session = registry.session(id()).unwrap();
        assert_eq!(session.text().unwrap().to_string(), "draft");
        assert_eq!(session.saved_state(), state('a'));
        assert_eq!(session.current_state(), state('b'));
        assert!(session.is_dirty() && session.is_read_revoked());
        assert_eq!(registry.undo(id()), Ok(EditApplyOutcome::Unchanged));
        assert_eq!(
            registry.external_request(id(), 0),
            Err(ExternalChangeError::ReadRevoked)
        );
    }

    #[test]
    fn explicit_reauthorization_preserves_draft_and_records_disk_conflict() {
        let mut registry = DocumentRegistry::new();
        registry
            .restore(
                id(),
                DocumentRecovery {
                    location: location(None),
                    body: Rope::from_str("draft").into(),
                    saved_state: state('a'),
                    current_state: state('b'),
                    disk_baseline: Arc::default(),
                    metadata: DocumentMetadata::default(),
                    read_revoked: true,
                    external_conflict: None,
                },
            )
            .unwrap();
        let actual = location(Some(9));
        let baseline: Arc<[u8]> = Arc::from(b"changed on disk".as_slice());
        registry
            .reauthorize(
                id(),
                actual.clone(),
                baseline.clone(),
                Some(DocumentConflict::modified(actual, baseline.clone())),
            )
            .unwrap();

        let session = registry.session(id()).unwrap();
        assert!(!session.is_read_revoked());
        assert_eq!(session.text().unwrap().to_string(), "draft");
        assert!(session.is_dirty());
        assert_eq!(session.disk_baseline(), &baseline);
        assert_eq!(
            session.external_conflict().unwrap().kind(),
            crate::ExternalConflictKind::Modified
        );
        assert!(registry.external_request(id(), 0).is_ok());
        assert_eq!(
            registry.reauthorize(id(), location(None), Arc::default(), None),
            Err(ReauthorizationError::NotReadRevoked)
        );
    }

    #[test]
    fn stale_read_revoke_cannot_cancel_explicit_reauthorization() {
        let mut registry = DocumentRegistry::new();
        registry
            .open(
                id(),
                location(None),
                Rope::from_str("draft"),
                state('a'),
                Arc::from(b"disk".as_slice()),
                DocumentMetadata::default(),
            )
            .unwrap();
        let request = registry.external_request(id(), 0).unwrap();
        let revoked = crate::DocumentExternalChange::ReadRevoked {
            request: request.clone(),
            location: request.location().clone(),
        };
        registry
            .apply_external(revoked.clone(), std::time::Instant::now())
            .unwrap();
        registry
            .reauthorize(id(), location(None), Arc::from(b"disk".as_slice()), None)
            .unwrap();

        assert_eq!(
            registry.apply_external(revoked, std::time::Instant::now()),
            Err(ExternalChangeError::StaleExternalChange)
        );
        assert!(!registry.session(id()).unwrap().is_read_revoked());
    }

    #[test]
    fn reauthorization_generation_exhaustion_preserves_revoked_state() {
        let mut registry = DocumentRegistry::new();
        registry
            .open(
                id(),
                location(None),
                Rope::from_str("draft"),
                state('a'),
                Arc::from(b"disk".as_slice()),
                DocumentMetadata::default(),
            )
            .unwrap();
        let request = registry.external_request(id(), 0).unwrap();
        registry
            .apply_external(
                crate::DocumentExternalChange::ReadRevoked {
                    location: request.location().clone(),
                    request,
                },
                std::time::Instant::now(),
            )
            .unwrap();
        registry.session_mut(id()).unwrap().external_generation = u64::MAX;

        assert_eq!(
            registry.reauthorize(id(), location(Some(9)), Arc::default(), None),
            Err(ReauthorizationError::GenerationExhausted)
        );
        let session = registry.session(id()).unwrap();
        assert!(session.is_read_revoked());
        assert_eq!(session.location(), &location(None));
        assert_eq!(session.disk_baseline().as_ref(), b"disk");
    }

    #[test]
    fn recovery_rejects_occupied_target_without_discarding_either_document() {
        let mut registry = DocumentRegistry::new();
        registry
            .open(
                id(),
                location(Some(9)),
                Rope::from_str("existing"),
                state('a'),
                Arc::default(),
                DocumentMetadata::default(),
            )
            .unwrap();
        let other = DocumentId::from_str("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b").unwrap();

        assert_eq!(
            registry.restore(
                other,
                DocumentRecovery {
                    location: location(Some(9)),
                    body: Rope::from_str("must not merge").into(),
                    saved_state: state('a'),
                    current_state: state('b'),
                    disk_baseline: Arc::default(),
                    metadata: DocumentMetadata::default(),
                    read_revoked: false,
                    external_conflict: None,
                },
            ),
            Err(DocumentRegistryError::RecoveryTargetOccupied(other))
        );
        assert_eq!(
            registry.session(id()).unwrap().text().unwrap().to_string(),
            "existing"
        );
        assert!(registry.session(other).is_none());
    }
}
