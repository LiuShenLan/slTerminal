//! Document 外部刷新、失权与真实文件重定位。

use std::io;
use std::sync::Arc;

use ropey::Rope;
use slterm_document::{
    DocumentBody, DocumentConflict, DocumentExternalChange, DocumentExternalRequest, DocumentId,
    DocumentRegistry, ExternalChangeError,
};
use slterm_path::{CanonicalPath, PathGrant, PathOperation, PathPolicy, PathRequest, ResolvedPath};
use uuid::Uuid;

use super::file_editor::document::Document;
use crate::platform::document_paths::{DocumentPathResolver, SystemDocumentPathResolver};

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
    fn request(
        registry: &DocumentRegistry,
        id: DocumentId,
        authorization_generation: u64,
    ) -> io::Result<Result<DocumentExternalRequest, DocumentExternalChange>> {
        match registry.external_request(id, authorization_generation) {
            Ok(request) => Ok(Ok(request)),
            Err(ExternalChangeError::ReadRevoked) => {
                Ok(Err(DocumentExternalChange::ReadRevoked { document_id: id }))
            }
            Err(error) => Err(io::Error::other(error)),
        }
    }

    fn authorize(
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
    ) -> io::Result<DocumentExternalChange> {
        let id = request.document_id();
        let resolved = match self.resolver.resolve(lexical) {
            Ok(resolved) => resolved,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(DocumentExternalChange::Deleted(request));
            }
            Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
                return Ok(DocumentExternalChange::ReadRevoked { document_id: id });
            }
            Err(error) => return Err(error),
        };
        if !Self::authorize(&request, lexical, &resolved, grants, current_generation) {
            return Ok(DocumentExternalChange::ReadRevoked { document_id: id });
        }
        let document = match Document::load(resolved.canonical_path().as_path()) {
            Ok(document) => document,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(DocumentExternalChange::Deleted(request));
            }
            Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
                return Ok(DocumentExternalChange::ReadRevoked { document_id: id });
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
                Ok(paged) => paged,
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    return Ok(DocumentExternalChange::Deleted(request));
                }
                Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
                    return Ok(DocumentExternalChange::ReadRevoked { document_id: id });
                }
                Err(error) => return Err(error),
            };
            return Ok(DocumentExternalChange::Reload {
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
            });
        }
        Ok(DocumentExternalChange::Reload {
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
        })
    }

    /// 后台刷新；None 表示真实位置与磁盘基线均未变化。
    pub(crate) fn refresh(
        &self,
        registry: &DocumentRegistry,
        id: DocumentId,
        grants: &[PathGrant],
        current_authorization_generation: u64,
    ) -> io::Result<Option<DocumentExternalChange>> {
        let request = match Self::request(registry, id, current_authorization_generation)? {
            Ok(request) => request,
            Err(change) => return Ok(Some(change)),
        };
        let expected_baseline = request.disk_baseline().clone();
        let expected_location = request.location().clone();
        let lexical = request.location().canonical_path().clone();
        let change = self.read(request, &lexical, grants, current_authorization_generation)?;
        let unchanged = match &change {
            DocumentExternalChange::Reload {
                content,
                disk_baseline,
                metadata,
                location,
                ..
            } => {
                disk_baseline == &expected_baseline
                    && location == &expected_location
                    && registry.session(id).is_some_and(|session| {
                        session.encoding() == &metadata.encoding
                            && session.is_read_only() == metadata.read_only
                            && content_matches_current(registry, id, content)
                    })
            }
            _ => false,
        };
        if unchanged {
            Ok(None)
        } else {
            Ok(Some(change))
        }
    }

    /// 外部改名按真实身份与磁盘基线重绑；内容同时变化时交给同一重载／冲突链。
    pub(crate) fn relocate(
        &self,
        registry: &DocumentRegistry,
        id: DocumentId,
        target: &CanonicalPath,
        grants: &[PathGrant],
        current_authorization_generation: u64,
    ) -> io::Result<DocumentExternalChange> {
        let request = match Self::request(registry, id, current_authorization_generation)? {
            Ok(request) => request,
            Err(change) => return Ok(change),
        };
        let resolved = self.resolver.resolve(target)?;
        if registry
            .existing(&resolved)
            .is_some_and(|existing| existing != id)
        {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "重定位目标已关联其他 Document",
            ));
        }
        let change = self.read(request, target, grants, current_authorization_generation)?;
        match change {
            DocumentExternalChange::Reload {
                request,
                content,
                disk_baseline,
                location,
                ..
            } if location.file_identity() == request.location().file_identity()
                && disk_baseline.as_ref() == request.disk_baseline().as_ref()
                && content_matches_current(registry, id, &content) =>
            {
                Ok(DocumentExternalChange::Relocated { request, location })
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
                Ok(DocumentExternalChange::Conflict { request, conflict })
            }
            change => Ok(change),
        }
    }
}

fn content_matches_current(
    registry: &DocumentRegistry,
    id: DocumentId,
    content: &DocumentBody,
) -> bool {
    match content {
        DocumentBody::Paged(next) => registry
            .session(id)
            .and_then(|session| session.paged())
            .is_some_and(|current| {
                current.index() == next.index() && current.fingerprint() == next.fingerprint()
            }),
        DocumentBody::Editable(_) => true,
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

        // 失权后不再解析或读盘；真实删除也不能把 ReadRevoked 改成 Deleted。
        std::fs::remove_file(&file).unwrap();
        let change = DocumentExternalService::default()
            .refresh(&registry, opened.id, &[PathGrant::project_root(root)], 0)
            .unwrap()
            .unwrap();
        assert!(matches!(change, DocumentExternalChange::ReadRevoked { .. }));
        registry.apply_external(change, Instant::now()).unwrap();
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

        let change = DocumentExternalService::default()
            .relocate(
                &registry,
                opened.id,
                &CanonicalPath::new(&target).unwrap(),
                &[PathGrant::project_root(root)],
                0,
            )
            .unwrap();
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
