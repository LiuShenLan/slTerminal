//! Document 真实打开：授权、查重、读取与聚合注册只经这一入口。

use std::io;
use std::sync::Arc;

use ropey::Rope;
use slterm_document::{DocumentId, DocumentOpenOutcome, DocumentRegistry, TextStateId};
use slterm_path::{CanonicalPath, PathOperation, PathPolicy, PathRequest};
use uuid::Uuid;

use super::file_editor::document::Document;
use crate::platform::document_paths::{DocumentPathResolver, SystemDocumentPathResolver};

pub(crate) struct OpenedDocument {
    pub(crate) id: DocumentId,
    pub(crate) outcome: DocumentOpenOutcome,
    /// 只有新聚合需要把已读快照移交给编辑器；既有聚合不得重读替换正文。
    pub(crate) document: Option<Document>,
}

pub(crate) struct DocumentOpenService {
    resolver: Arc<dyn DocumentPathResolver>,
}

impl Default for DocumentOpenService {
    fn default() -> Self {
        Self {
            resolver: Arc::new(SystemDocumentPathResolver),
        }
    }
}

impl DocumentOpenService {
    /// 阻塞读取只允许在后台执行体调用；前台只消费此完整结果。
    pub(crate) fn open(
        &self,
        registry: &mut DocumentRegistry,
        project_root: &CanonicalPath,
        lexical: &CanonicalPath,
    ) -> io::Result<OpenedDocument> {
        let resolved = self.resolver.resolve(lexical)?;
        let request = PathRequest {
            operation: PathOperation::Read,
            lexical_path: lexical.as_path().to_path_buf(),
            root_hints: Vec::new(),
            session_grant: None,
            expected_authorization_generation: 0,
        };
        let authorization = PathPolicy::authorize(
            &request,
            &resolved,
            &[slterm_path::PathGrant::project_root(project_root.clone())],
            0,
        )
        .map_err(|_| io::Error::new(io::ErrorKind::PermissionDenied, "文档读取未获授权"))?;
        let resolved = authorization.resolved_path().clone();
        if let Some(id) = registry.existing(&resolved) {
            return Ok(OpenedDocument {
                id,
                outcome: DocumentOpenOutcome::Existing,
                document: None,
            });
        }

        let document = Document::load(resolved.canonical_path().as_path())?;
        if document.location() != &resolved {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "读取期间文件身份已变化",
            ));
        }
        let id = DocumentId::from_uuid(Uuid::now_v7()).expect("系统生成的 DocumentId 必须为 v7");
        let state =
            TextStateId::from_uuid(Uuid::now_v7()).expect("系统生成的 TextStateId 必须为 v7");
        let text = Rope::from_str(&document.text);
        let (id, outcome) = registry
            .open(id, resolved, text, state)
            .map_err(io::Error::other)?;
        Ok(OpenedDocument {
            id,
            outcome,
            document: (outcome == DocumentOpenOutcome::Opened).then_some(document),
        })
    }
}

#[cfg(test)]
mod documents_tests {
    use slterm_document::DocumentOpenOutcome;

    use super::*;

    #[test]
    fn path_file_identity_deduplicates_document() {
        let temp = tempfile::tempdir().unwrap();
        let root = CanonicalPath::new(temp.path()).unwrap();
        let file = temp.path().join("file.txt");
        let alias = temp.path().join("alias.txt");
        std::fs::write(&file, "shared").unwrap();
        std::fs::hard_link(&file, &alias).unwrap();
        let service = DocumentOpenService::default();
        let mut registry = DocumentRegistry::new();

        let first = service
            .open(&mut registry, &root, &CanonicalPath::new(file).unwrap())
            .unwrap();
        let second = service
            .open(&mut registry, &root, &CanonicalPath::new(alias).unwrap())
            .unwrap();

        assert_eq!(first.outcome, DocumentOpenOutcome::Opened);
        assert_eq!(second.outcome, DocumentOpenOutcome::Existing);
        assert_eq!(second.id, first.id);
        assert!(second.document.is_none());
        assert_eq!(registry.len(), 1);
        assert_eq!(
            registry.session(first.id).unwrap().text().to_string(),
            "shared"
        );
    }

    #[test]
    fn failed_real_open_does_not_register_a_document() {
        let temp = tempfile::tempdir().unwrap();
        let root = CanonicalPath::new(temp.path()).unwrap();
        let missing = CanonicalPath::new(temp.path().join("missing.txt")).unwrap();
        let service = DocumentOpenService::default();
        let mut registry = DocumentRegistry::new();

        let result = service.open(&mut registry, &root, &missing);

        assert!(result.is_err());
        assert!(registry.is_empty());
    }

    #[test]
    fn real_open_rejects_target_outside_project() {
        let temp = tempfile::tempdir().unwrap();
        let root_path = temp.path().join("project");
        std::fs::create_dir(&root_path).unwrap();
        let root = CanonicalPath::new(&root_path).unwrap();
        let outside = temp.path().join("outside.txt");
        std::fs::write(&outside, "outside").unwrap();
        let service = DocumentOpenService::default();
        let mut registry = DocumentRegistry::new();

        let result = service.open(&mut registry, &root, &CanonicalPath::new(outside).unwrap());

        let error = match result {
            Ok(_) => panic!("根外读取不得成功"),
            Err(error) => error,
        };
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        assert!(registry.is_empty());
    }
}
