//! Document 外部刷新、失权与真实文件重定位。

use std::io;
use std::sync::Arc;

use ropey::Rope;
use slterm_document::{
    DocumentBody, DocumentConflict, DocumentExternalChange, DocumentExternalRequest,
};
#[cfg(test)]
use slterm_document::{DocumentId, DocumentRegistry, ExternalChangeError};
use slterm_path::{CanonicalPath, PathGrant, PathOperation, PathPolicy, PathRequest, ResolvedPath};
use uuid::Uuid;

use super::file_editor::document::Document;
use crate::platform::document_paths::{DocumentPathResolver, SystemDocumentPathResolver};

pub(crate) struct DocumentExternalResult {
    pub(crate) change: DocumentExternalChange,
    pub(crate) paged: Option<Arc<super::paged_document::PagedDocument>>,
}

impl From<DocumentExternalChange> for DocumentExternalResult {
    fn from(change: DocumentExternalChange) -> Self {
        Self {
            change,
            paged: None,
        }
    }
}

pub(crate) struct DocumentExternalService {
    resolver: Arc<dyn DocumentPathResolver>,
}

impl Default for DocumentExternalService {
    fn default() -> Self {
        Self {
            resolver: Arc::new(SystemDocumentPathResolver),
        }
    }
}

impl DocumentExternalService {
    #[cfg(test)]
    fn request(
        registry: &DocumentRegistry,
        id: DocumentId,
        authorization_generation: u64,
    ) -> io::Result<Option<DocumentExternalRequest>> {
        match registry.external_request(id, authorization_generation) {
            Ok(request) => Ok(Some(request)),
            Err(ExternalChangeError::ReadRevoked) => Ok(None),
            Err(error) => Err(io::Error::other(error)),
        }
    }

    pub(crate) fn authorize(
        request: &DocumentExternalRequest,
        lexical: &CanonicalPath,
        resolved: &ResolvedPath,
        grants: &[PathGrant],
        current_generation: u64,
    ) -> bool {
        let path_request = PathRequest {
            operation: PathOperation::Read,
            lexical_path: lexical.as_path().to_path_buf(),
            root_hints: Vec::new(),
            session_grant: None,
            expected_authorization_generation: request.authorization_generation(),
        };
        PathPolicy::authorize(&path_request, resolved, grants, current_generation).is_ok()
    }

    fn read(
        &self,
        request: DocumentExternalRequest,
        lexical: &CanonicalPath,
        grants: &[PathGrant],
        current_generation: u64,
    ) -> io::Result<DocumentExternalResult> {
        let resolved = match self.resolver.resolve(lexical) {
            Ok(resolved) => resolved,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(DocumentExternalResult::from(
                    DocumentExternalChange::Deleted(request),
                ));
            }
            Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
                let location =
                    ResolvedPath::new(lexical.clone(), request.location().file_identity());
                return Ok(DocumentExternalResult::from(
                    DocumentExternalChange::ReadRevoked { request, location },
                ));
            }
            Err(error) => return Err(error),
        };
        if !Self::authorize(&request, lexical, &resolved, grants, current_generation) {
            return Ok(DocumentExternalResult::from(
                DocumentExternalChange::ReadRevoked {
                    request,
                    location: resolved,
                },
            ));
        }
        let document = match Document::load(resolved.canonical_path().as_path()) {
            Ok(document) => document,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(DocumentExternalResult::from(
                    DocumentExternalChange::Deleted(request),
                ));
            }
            Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
                return Ok(DocumentExternalResult::from(
                    DocumentExternalChange::ReadRevoked {
                        request,
                        location: resolved,
                    },
                ));
            }
            Err(error) => return Err(error),
        };
        if document.location() != &resolved {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "外部刷新期间文件身份已变化",
            ));
        }
        if document.total_bytes > slterm_document::EDITABLE_LIMIT_BYTES {
            let paged = match super::paged_document::PagedDocument::open(
                &resolved,
                Some(document.encoding.encoding),
            ) {
                Ok(paged) => Arc::new(paged),
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    return Ok(DocumentExternalResult::from(
                        DocumentExternalChange::Deleted(request),
                    ));
                }
                Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
                    return Ok(DocumentExternalResult::from(
                        DocumentExternalChange::ReadRevoked {
                            request,
                            location: resolved,
                        },
                    ));
                }
                Err(error) => return Err(error),
            };
            let change = DocumentExternalChange::Reload {
                request,
                content: slterm_document::DocumentBody::Paged(paged.source()),
                disk_baseline: Arc::default(),
                metadata: slterm_document::DocumentMetadata {
                    encoding: paged.encoding().clone(),
                    read_only: true,
                },
                location: resolved,
                new_state: slterm_document::TextStateId::from_uuid(Uuid::now_v7())
                    .expect("系统生成的 TextStateId 必须为 v7"),
            };
            return Ok(DocumentExternalResult {
                change,
                paged: Some(paged),
            });
        }
        Ok(DocumentExternalResult::from(
            DocumentExternalChange::Reload {
                request,
                content: slterm_document::DocumentBody::Editable(Rope::from_str(&document.text)),
                disk_baseline: document.bytes.clone(),
                metadata: slterm_document::DocumentMetadata {
                    encoding: document.encoding.clone(),
                    read_only: document.read_only,
                },
                location: document.location().clone(),
                new_state: slterm_document::TextStateId::from_uuid(Uuid::now_v7())
                    .expect("系统生成的 TextStateId 必须为 v7"),
            },
        ))
    }

    /// 后台消费不可变请求；None 表示真实位置、元数据与磁盘正文均未变化。
    pub(crate) fn refresh_request(
        &self,
        request: DocumentExternalRequest,
        grants: &[PathGrant],
        current_authorization_generation: u64,
    ) -> io::Result<Option<DocumentExternalResult>> {
        let lexical = request.location().canonical_path().clone();
        let result = self.read(request, &lexical, grants, current_authorization_generation)?;
        let unchanged = match &result.change {
            DocumentExternalChange::Reload {
                request,
                content,
                disk_baseline,
                metadata,
                location,
                ..
            } => {
                disk_baseline == request.disk_baseline()
                    && location == request.location()
                    && metadata == request.metadata()
                    && content_matches_request(request, content)
            }
            _ => false,
        };
        if unchanged {
            Ok(None)
        } else {
            Ok(Some(result))
        }
    }

    #[cfg(test)]
    fn refresh(
        &self,
        registry: &DocumentRegistry,
        id: DocumentId,
        grants: &[PathGrant],
        current_authorization_generation: u64,
    ) -> io::Result<Option<DocumentExternalChange>> {
        let request = match Self::request(registry, id, current_authorization_generation)? {
            Some(request) => request,
            None => return Ok(None),
        };
        Ok(self
            .refresh_request(request, grants, current_authorization_generation)?
            .map(|result| result.change))
    }

    /// 外部改名按请求捕获的身份与磁盘基线重绑；正文比较不读取前台 Registry。
    pub(crate) fn relocate(
        &self,
        request: DocumentExternalRequest,
        target: &CanonicalPath,
        grants: &[PathGrant],
        current_authorization_generation: u64,
    ) -> io::Result<DocumentExternalResult> {
        let DocumentExternalResult { change, paged } =
            self.read(request, target, grants, current_authorization_generation)?;
        let change = match change {
            DocumentExternalChange::Reload {
                request,
                content,
                disk_baseline,
                metadata,
                location,
                ..
            } if location.file_identity() == request.location().file_identity()
                && disk_baseline.as_ref() == request.disk_baseline().as_ref()
                && &metadata == request.metadata()
                && content_matches_request(&request, &content) =>
            {
                DocumentExternalChange::Relocated { request, location }
            }
            DocumentExternalChange::Reload {
                request,
                content,
                disk_baseline,
                location,
                ..
            } if location.file_identity() != request.location().file_identity() => {
                let conflict = match content {
                    DocumentBody::Editable(_) => {
                        DocumentConflict::modified(location, disk_baseline)
                    }
                    DocumentBody::Paged(_) => DocumentConflict::modified_paged(location),
                };
                DocumentExternalChange::Conflict { request, conflict }
            }
            change => change,
        };
        Ok(DocumentExternalResult { change, paged })
    }
}

fn content_matches_request(request: &DocumentExternalRequest, content: &DocumentBody) -> bool {
    match (request.body(), content) {
        (DocumentBody::Paged(current), DocumentBody::Paged(next)) => {
            current.index() == next.index() && current.fingerprint() == next.fingerprint()
        }
        (DocumentBody::Editable(_), DocumentBody::Editable(_)) => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use slterm_document::{
        DocumentExternalChange, DocumentRegistry, EditOrigin, EditTransaction,
        ExternalApplyOutcome, MergePolicy, TextEdit, TextStateId,
    };
    use slterm_path::{CanonicalPath, PathGrant};
    use uuid::Uuid;

    use super::DocumentExternalService;
    use crate::app_shell::documents::DocumentOpenService;

    fn replace_local(registry: &mut DocumentRegistry, id: slterm_document::DocumentId, text: &str) {
        let session = registry.session(id).unwrap();
        let revision = session.text_revision();
        let end = session.text().unwrap().len();
        registry
            .apply(
                EditTransaction {
                    document_id: id,
                    expected_text_revision: revision,
                    edits: vec![TextEdit::new(0..end, text)],
                    origin: EditOrigin::ReplaceAll,
                    merge_policy: MergePolicy::Disallowed,
                },
                TextStateId::from_uuid(Uuid::now_v7()).unwrap(),
                Instant::now(),
            )
            .unwrap();
    }

    #[test]
    fn clean_external_change_reloads_and_dirty_change_conflicts() {
        let temp = tempfile::tempdir().unwrap();
        let root = CanonicalPath::new(dunce::canonicalize(temp.path()).unwrap()).unwrap();
        let file = temp.path().join("file.txt");
        std::fs::write(&file, "old").unwrap();
        let mut registry = DocumentRegistry::new();
        let opened = DocumentOpenService::default()
            .open(&mut registry, &root, &CanonicalPath::new(&file).unwrap())
            .unwrap();
        std::fs::write(&file, "outside").unwrap();
        let change = DocumentExternalService::default()
            .refresh(
                &registry,
                opened.id,
                &[PathGrant::project_root(root.clone())],
                0,
            )
            .unwrap()
            .unwrap();
        registry.apply_external(change, Instant::now()).unwrap();
        assert_eq!(
            registry
                .session(opened.id)
                .unwrap()
                .text()
                .unwrap()
                .to_string(),
            "outside"
        );
        assert!(!registry.session(opened.id).unwrap().is_dirty());

        replace_local(&mut registry, opened.id, "local");
        std::fs::write(&file, "other").unwrap();
        let change = DocumentExternalService::default()
            .refresh(&registry, opened.id, &[PathGrant::project_root(root)], 0)
            .unwrap()
            .unwrap();
        let outcome = registry.apply_external(change, Instant::now()).unwrap();
        assert_eq!(outcome, ExternalApplyOutcome::Conflict);
        assert_eq!(
            registry
                .session(opened.id)
                .unwrap()
                .text()
                .unwrap()
                .to_string(),
            "local"
        );
        assert_eq!(
            registry
                .session(opened.id)
                .unwrap()
                .external_conflict()
                .unwrap()
                .disk_baseline()
                .unwrap()
                .as_ref(),
            b"other"
        );
    }

    #[test]
    fn external_permission_change_updates_read_only_without_rebuilding_text() {
        let temp = tempfile::tempdir().unwrap();
        let root = CanonicalPath::new(dunce::canonicalize(temp.path()).unwrap()).unwrap();
        let file = temp.path().join("file.txt");
        std::fs::write(&file, "same").unwrap();
        let mut registry = DocumentRegistry::new();
        let opened = DocumentOpenService::default()
            .open(&mut registry, &root, &CanonicalPath::new(&file).unwrap())
            .unwrap();
        let mut permissions = std::fs::metadata(&file).unwrap().permissions();
        permissions.set_readonly(true);
        std::fs::set_permissions(&file, permissions).unwrap();

        let change = DocumentExternalService::default()
            .refresh(&registry, opened.id, &[PathGrant::project_root(root)], 0)
            .unwrap()
            .unwrap();
        registry.apply_external(change, Instant::now()).unwrap();

        let session = registry.session(opened.id).unwrap();
        assert!(session.is_read_only());
        assert_eq!(session.text().unwrap().to_string(), "same");
        assert_eq!(session.text_revision(), 0);
        #[allow(clippy::permissions_set_readonly_false)]
        {
            let mut permissions = std::fs::metadata(&file).unwrap().permissions();
            permissions.set_readonly(false);
            std::fs::set_permissions(&file, permissions).unwrap();
        }
    }

    #[test]
    fn document_retains_content_after_read_revoke() {
        let temp = tempfile::tempdir().unwrap();
        let root = CanonicalPath::new(dunce::canonicalize(temp.path()).unwrap()).unwrap();
        let file = temp.path().join("file.txt");
        std::fs::write(&file, "content").unwrap();
        let mut registry = DocumentRegistry::new();
        let opened = DocumentOpenService::default()
            .open(&mut registry, &root, &CanonicalPath::new(&file).unwrap())
            .unwrap();
        replace_local(&mut registry, opened.id, "local");

        let change = DocumentExternalService::default()
            .refresh(&registry, opened.id, &[], 0)
            .unwrap()
            .unwrap();
        assert!(matches!(change, DocumentExternalChange::ReadRevoked { .. }));
        registry.apply_external(change, Instant::now()).unwrap();
        assert!(registry.session(opened.id).unwrap().is_read_revoked());
        assert_eq!(
            registry
                .session(opened.id)
                .unwrap()
                .text()
                .unwrap()
                .to_string(),
            "local"
        );

        // 失权后不再解析、读盘或生成无新事实的重复事件。
        std::fs::remove_file(&file).unwrap();
        let change = DocumentExternalService::default()
            .refresh(&registry, opened.id, &[PathGrant::project_root(root)], 0)
            .unwrap();
        assert!(change.is_none());
        assert_eq!(
            registry
                .session(opened.id)
                .unwrap()
                .text()
                .unwrap()
                .to_string(),
            "local"
        );
    }

    #[test]
    fn external_delete_marks_shared_conflict_without_clearing_text() {
        let temp = tempfile::tempdir().unwrap();
        let root = CanonicalPath::new(dunce::canonicalize(temp.path()).unwrap()).unwrap();
        let file = temp.path().join("file.txt");
        std::fs::write(&file, "content").unwrap();
        let mut registry = DocumentRegistry::new();
        let opened = DocumentOpenService::default()
            .open(&mut registry, &root, &CanonicalPath::new(&file).unwrap())
            .unwrap();
        std::fs::remove_file(&file).unwrap();

        let change = DocumentExternalService::default()
            .refresh(
                &registry,
                opened.id,
                &[PathGrant::project_root(root.clone())],
                0,
            )
            .unwrap()
            .unwrap();
        let outcome = registry.apply_external(change, Instant::now()).unwrap();

        assert_eq!(outcome, ExternalApplyOutcome::Deleted);
        assert_eq!(
            registry
                .session(opened.id)
                .unwrap()
                .text()
                .unwrap()
                .to_string(),
            "content"
        );
        assert_eq!(
            registry
                .session(opened.id)
                .unwrap()
                .external_conflict()
                .unwrap()
                .kind(),
            slterm_document::ExternalConflictKind::Deleted
        );

        std::fs::write(&file, "recreated").unwrap();
        let change = DocumentExternalService::default()
            .refresh(&registry, opened.id, &[PathGrant::project_root(root)], 0)
            .unwrap()
            .unwrap();
        let outcome = registry.apply_external(change, Instant::now()).unwrap();
        assert_eq!(outcome, ExternalApplyOutcome::Conflict);
        assert_eq!(
            registry
                .session(opened.id)
                .unwrap()
                .text()
                .unwrap()
                .to_string(),
            "content"
        );
        assert_eq!(
            registry
                .session(opened.id)
                .unwrap()
                .external_conflict()
                .unwrap()
                .kind(),
            slterm_document::ExternalConflictKind::Modified
        );
    }

    #[test]
    fn document_location_change_does_not_rebuild() {
        let temp = tempfile::tempdir().unwrap();
        let root = CanonicalPath::new(dunce::canonicalize(temp.path()).unwrap()).unwrap();
        let source = temp.path().join("source.txt");
        let target = temp.path().join("renamed.txt");
        std::fs::write(&source, "same").unwrap();
        let mut registry = DocumentRegistry::new();
        let opened = DocumentOpenService::default()
            .open(&mut registry, &root, &CanonicalPath::new(&source).unwrap())
            .unwrap();
        let revision = registry.session(opened.id).unwrap().text_revision();
        std::fs::rename(&source, &target).unwrap();

        let request = registry.external_request(opened.id, 0).unwrap();
        let change = DocumentExternalService::default()
            .relocate(
                request,
                &CanonicalPath::new(&target).unwrap(),
                &[PathGrant::project_root(root)],
                0,
            )
            .unwrap()
            .change;
        let outcome = registry.apply_external(change, Instant::now()).unwrap();

        assert_eq!(outcome, ExternalApplyOutcome::Relocated);
        let session = registry.session(opened.id).unwrap();
        assert_eq!(session.id(), opened.id);
        assert_eq!(session.text_revision(), revision);
        assert_eq!(session.text().unwrap().to_string(), "same");
        assert_eq!(session.location().canonical_path().as_path(), target);
        assert_eq!(registry.existing(session.location()), Some(opened.id));
    }

    #[test]
    fn external_rename_with_permission_change_is_not_pure_relocation() {
        let temp = tempfile::tempdir().unwrap();
        let root = CanonicalPath::new(dunce::canonicalize(temp.path()).unwrap()).unwrap();
        let source = temp.path().join("source.txt");
        let target = temp.path().join("renamed.txt");
        std::fs::write(&source, "same").unwrap();
        let mut registry = DocumentRegistry::new();
        let opened = DocumentOpenService::default()
            .open(&mut registry, &root, &CanonicalPath::new(&source).unwrap())
            .unwrap();
        let mut permissions = std::fs::metadata(&source).unwrap().permissions();
        permissions.set_readonly(true);
        std::fs::set_permissions(&source, permissions).unwrap();
        let request = registry.external_request(opened.id, 0).unwrap();
        std::fs::rename(&source, &target).unwrap();

        let change = DocumentExternalService::default()
            .relocate(
                request,
                &CanonicalPath::new(&target).unwrap(),
                &[PathGrant::project_root(root)],
                0,
            )
            .unwrap()
            .change;
        assert!(matches!(change, DocumentExternalChange::Reload { .. }));
        registry.apply_external(change, Instant::now()).unwrap();
        assert!(registry.session(opened.id).unwrap().is_read_only());
        assert_eq!(
            registry
                .session(opened.id)
                .unwrap()
                .location()
                .canonical_path()
                .as_path(),
            target
        );

        #[allow(clippy::permissions_set_readonly_false)]
        {
            let mut permissions = std::fs::metadata(&target).unwrap().permissions();
            permissions.set_readonly(false);
            std::fs::set_permissions(&target, permissions).unwrap();
        }
    }

    #[test]
    fn external_rename_identity_replacement_conflicts_without_rebinding() {
        let temp = tempfile::tempdir().unwrap();
        let root = CanonicalPath::new(dunce::canonicalize(temp.path()).unwrap()).unwrap();
        let source = temp.path().join("source.txt");
        let target = temp.path().join("renamed.txt");
        std::fs::write(&source, "original").unwrap();
        let mut registry = DocumentRegistry::new();
        let opened = DocumentOpenService::default()
            .open(&mut registry, &root, &CanonicalPath::new(&source).unwrap())
            .unwrap();
        let request = registry.external_request(opened.id, 0).unwrap();
        std::fs::rename(&source, &target).unwrap();
        std::fs::remove_file(&target).unwrap();
        std::fs::write(&target, "replacement").unwrap();

        let change = DocumentExternalService::default()
            .relocate(
                request,
                &CanonicalPath::new(&target).unwrap(),
                &[PathGrant::project_root(root)],
                0,
            )
            .unwrap()
            .change;
        assert!(matches!(change, DocumentExternalChange::Conflict { .. }));
        let outcome = registry.apply_external(change, Instant::now()).unwrap();

        assert_eq!(outcome, ExternalApplyOutcome::Conflict);
        assert_eq!(
            registry.session(opened.id).unwrap().text().unwrap(),
            "original"
        );
        assert_eq!(
            registry
                .session(opened.id)
                .unwrap()
                .location()
                .canonical_path()
                .as_path(),
            source
        );
    }

    #[test]
    fn editor_large_file_dirty_preserves_buffer() {
        fn grow(path: &std::path::Path) {
            use std::io::Write as _;

            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .truncate(true)
                .open(path)
                .unwrap();
            let chunk = vec![b'x'; 1024 * 1024];
            let mut written = 0u64;
            while written < slterm_document::EDITABLE_LIMIT_BYTES + 1 {
                file.write_all(&chunk).unwrap();
                written += chunk.len() as u64;
            }
        }

        let temp = tempfile::tempdir().unwrap();
        let root = CanonicalPath::new(dunce::canonicalize(temp.path()).unwrap()).unwrap();
        let path = temp.path().join("large.txt");
        std::fs::write(&path, "old").unwrap();
        let opener = DocumentOpenService::default();
        let mut registry = DocumentRegistry::new();
        let opened = opener
            .open(&mut registry, &root, &CanonicalPath::new(&path).unwrap())
            .unwrap();
        grow(&path);
        let change = DocumentExternalService::default()
            .refresh(
                &registry,
                opened.id,
                &[PathGrant::project_root(root.clone())],
                0,
            )
            .unwrap()
            .unwrap();
        assert!(matches!(
            &change,
            DocumentExternalChange::Reload {
                content: slterm_document::DocumentBody::Paged(_),
                ..
            }
        ));
        registry.apply_external(change, Instant::now()).unwrap();
        let total = registry
            .session(opened.id)
            .unwrap()
            .paged()
            .unwrap()
            .index()
            .total_bytes();
        assert!(total > slterm_document::EDITABLE_LIMIT_BYTES);
        assert!(
            DocumentExternalService::default()
                .refresh(
                    &registry,
                    opened.id,
                    &[PathGrant::project_root(root.clone())],
                    0,
                )
                .unwrap()
                .is_none()
        );

        std::fs::write(&path, "old").unwrap();
        let mut dirty = DocumentRegistry::new();
        let opened = opener
            .open(&mut dirty, &root, &CanonicalPath::new(&path).unwrap())
            .unwrap();
        replace_local(&mut dirty, opened.id, "draft");
        grow(&path);
        let change = DocumentExternalService::default()
            .refresh(&dirty, opened.id, &[PathGrant::project_root(root)], 0)
            .unwrap()
            .unwrap();
        dirty.apply_external(change, Instant::now()).unwrap();
        let session = dirty.session(opened.id).unwrap();
        assert_eq!(session.text().unwrap().to_string(), "draft");
        assert!(session.external_conflict().is_some());
        assert!(dirty.begin_save(opened.id, 0).is_ok());
    }
}
