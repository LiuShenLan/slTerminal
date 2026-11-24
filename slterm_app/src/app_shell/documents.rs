//! Document 真实打开与保存：授权、查重、读取、聚合注册和写盘只经这些入口。

use std::io;
use std::sync::Arc;

use ropey::Rope;
use slterm_document::{
    DocumentId, DocumentMetadata, DocumentOpenOutcome, DocumentRegistry, DocumentSaveReceipt,
    DocumentSaveRequest, EDITABLE_LIMIT_BYTES, TextStateId,
};
use slterm_path::{
    CanonicalPath, Capabilities, GrantPersistence, PathGrant, PathOperation, PathPolicy,
    PathRequest,
};
use slterm_workspace::{ProjectRootStatus, ProjectSnapshot, ViewKey};
use uuid::Uuid;

use super::file_editor::document::{Document, DocumentWriteContext};
use super::paged_document::PagedDocument;
use crate::platform::document_paths::{DocumentPathResolver, SystemDocumentPathResolver};
use crate::text_document::SaveError;

pub(crate) enum OpenedDocumentBody {
    Editable(Box<Document>),
    Paged(Arc<PagedDocument>),
}

pub(crate) struct OpenedDocument {
    pub(crate) id: DocumentId,
    pub(crate) outcome: DocumentOpenOutcome,
    /// 只有新聚合移交已读内容；既有聚合继续复用原正文与读取端口。
    pub(crate) body: Option<OpenedDocumentBody>,
}

pub(crate) struct ResolvedDocument {
    pub(crate) location: slterm_path::ResolvedPath,
    grants: Vec<PathGrant>,
}

impl ResolvedDocument {
    pub(crate) fn location(&self) -> &slterm_path::ResolvedPath {
        &self.location
    }

    pub(crate) fn grants(&self) -> &[PathGrant] {
        &self.grants
    }

    pub(crate) fn session_read_grant(&self) -> Option<PathGrant> {
        self.grants
            .iter()
            .find(|grant| {
                grant.persistence() == GrantPersistence::Session
                    && grant.capabilities() == Capabilities::READ
                    && grant.path() == self.location.canonical_path()
            })
            .cloned()
    }
}

pub(crate) struct LoadedDocument {
    location: slterm_path::ResolvedPath,
    body: OpenedDocumentBody,
}

/// 打开失败必须区分授权边界与普通 I/O，供 handover 映射稳定协议错误。
#[derive(Debug)]
pub(crate) enum DocumentOpenError {
    Unauthorized,
    Io(io::Error),
}

impl std::fmt::Display for DocumentOpenError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unauthorized => formatter.write_str("文档读取未获授权"),
            Self::Io(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for DocumentOpenError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Unauthorized => None,
            Self::Io(error) => Some(error),
        }
    }
}

impl From<io::Error> for DocumentOpenError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// 启动读取与所有恢复视图共享同一 Document 的 Ready 项目授权并集。
pub(crate) fn document_project_grants(
    snapshot: &ProjectSnapshot,
    id: DocumentId,
) -> Vec<PathGrant> {
    snapshot
        .registry()
        .projects()
        .iter()
        .filter(|project| {
            project.root_status() == ProjectRootStatus::Ready
                && project.layout().view_keys().iter().any(|view| match view {
                    ViewKey::Editor { document_id, .. }
                    | ViewKey::Markdown { document_id, .. }
                    | ViewKey::Diff { document_id, .. } => *document_id == id,
                    _ => false,
                })
        })
        .map(|project| PathGrant::project_root(project.root().clone()))
        .collect()
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
    /// 只在后台解析与授权；Registry 查重由前台在读取前完成。
    pub(crate) fn resolve(
        &self,
        project_root: &CanonicalPath,
        lexical: &CanonicalPath,
    ) -> Result<ResolvedDocument, DocumentOpenError> {
        self.resolve_with_grants(project_root, lexical, &[])
    }

    /// 项目根之外再叠加打开方的只读来源（如 Agent 根能力）。
    pub(crate) fn resolve_with_grants(
        &self,
        project_root: &CanonicalPath,
        lexical: &CanonicalPath,
        extra_read_grants: &[PathGrant],
    ) -> Result<ResolvedDocument, DocumentOpenError> {
        let resolved = self
            .resolver
            .resolve(lexical)
            .map_err(DocumentOpenError::Io)?;
        let request = PathRequest {
            operation: PathOperation::Read,
            lexical_path: lexical.as_path().to_path_buf(),
            root_hints: Vec::new(),
            session_grant: None,
            expected_authorization_generation: 0,
        };
        let mut grants = vec![PathGrant::project_root(project_root.clone())];
        grants.extend(extra_read_grants.iter().cloned());
        let authorization = PathPolicy::authorize(&request, &resolved, &grants, 0)
            .map_err(|_| DocumentOpenError::Unauthorized)?;
        Ok(ResolvedDocument {
            location: authorization.resolved_path().clone(),
            grants,
        })
    }

    /// 显式本地链接：持久授权优先，未覆盖目标只授予精确文件的会话读能力。
    pub(crate) fn resolve_link(
        &self,
        lexical: &CanonicalPath,
        persistent_grants: &[PathGrant],
        current_authorization_generation: u64,
    ) -> Result<ResolvedDocument, DocumentOpenError> {
        let resolved = self
            .resolver
            .resolve(lexical)
            .map_err(DocumentOpenError::Io)?;
        let request = PathRequest {
            operation: PathOperation::Read,
            lexical_path: lexical.as_path().to_path_buf(),
            root_hints: Vec::new(),
            session_grant: None,
            expected_authorization_generation: current_authorization_generation,
        };
        let authorization = PathPolicy::authorize(
            &request,
            &resolved,
            persistent_grants,
            current_authorization_generation,
        );
        match authorization {
            Ok(authorization) => Ok(ResolvedDocument {
                location: authorization.resolved_path().clone(),
                grants: persistent_grants.to_vec(),
            }),
            Err(slterm_path::PathDenial::Unauthorized(PathOperation::Read)) => {
                let grant = PathGrant::session_file_read(resolved.canonical_path().clone());
                let request = PathRequest {
                    session_grant: Some(grant.clone()),
                    ..request
                };
                let authorization = PathPolicy::authorize(
                    &request,
                    &resolved,
                    persistent_grants,
                    current_authorization_generation,
                )
                .map_err(|_| DocumentOpenError::Unauthorized)?;
                Ok(ResolvedDocument {
                    location: authorization.resolved_path().clone(),
                    grants: vec![grant],
                })
            }
            Err(_) => Err(DocumentOpenError::Unauthorized),
        }
    }

    /// 只在后台读取已授权位置；不接触或移走 Registry。
    pub(crate) fn load(&self, resolved: ResolvedDocument) -> io::Result<LoadedDocument> {
        let location = resolved.location;
        let initial_bytes = std::fs::metadata(location.canonical_path().as_path())?.len();
        let body = if initial_bytes > EDITABLE_LIMIT_BYTES {
            let document = Arc::new(PagedDocument::open(&location, None)?);
            if document.index().total_bytes() != initial_bytes {
                return Err(io::Error::other("文件在打开期间改变了长度"));
            }
            OpenedDocumentBody::Paged(document)
        } else {
            let document = Document::load(location.canonical_path().as_path())?.with_write_context(
                DocumentWriteContext::new(resolved.grants.clone(), 0, 0, self.resolver.clone()),
            );
            if document.location() != &location {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "读取期间文件身份已变化",
                ));
            }
            if document.total_bytes > EDITABLE_LIMIT_BYTES {
                return Err(io::Error::other("文件在打开期间增长为分页内容"));
            }
            OpenedDocumentBody::Editable(Box::new(document))
        };
        Ok(LoadedDocument { location, body })
    }

    /// 前台纯注册；读取后再次查重，绝不覆盖已存活正文。
    pub(crate) fn register(
        registry: &mut DocumentRegistry,
        loaded: LoadedDocument,
    ) -> io::Result<OpenedDocument> {
        if let Some(id) = registry.existing(&loaded.location) {
            return Ok(OpenedDocument {
                id,
                outcome: DocumentOpenOutcome::Existing,
                body: None,
            });
        }
        let id = DocumentId::from_uuid(Uuid::now_v7()).expect("系统生成的 DocumentId 必须为 v7");
        let state =
            TextStateId::from_uuid(Uuid::now_v7()).expect("系统生成的 TextStateId 必须为 v7");
        let (id, outcome) = match &loaded.body {
            OpenedDocumentBody::Editable(document) => registry.open(
                id,
                loaded.location,
                Rope::from_str(&document.text),
                state,
                document.bytes.clone(),
                DocumentMetadata {
                    encoding: document.encoding.clone(),
                    read_only: document.read_only,
                },
            ),
            OpenedDocumentBody::Paged(document) => registry.open(
                id,
                loaded.location,
                document.source(),
                state,
                Arc::default(),
                DocumentMetadata {
                    encoding: document.encoding().clone(),
                    read_only: true,
                },
            ),
        }
        .map_err(io::Error::other)?;
        Ok(OpenedDocument {
            id,
            outcome,
            body: (outcome == DocumentOpenOutcome::Opened).then_some(loaded.body),
        })
    }

    #[cfg(test)]
    pub(crate) fn open(
        &self,
        registry: &mut DocumentRegistry,
        project_root: &CanonicalPath,
        lexical: &CanonicalPath,
    ) -> Result<OpenedDocument, DocumentOpenError> {
        let resolved = self.resolve(project_root, lexical)?;
        let loaded = self.load(resolved)?;
        Self::register(registry, loaded).map_err(DocumentOpenError::Io)
    }
}

/// 另存为对既有磁盘目标的显式用户决定。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ExistingTargetPolicy {
    RequireConfirmation,
    OverwriteConfirmed,
}

/// 后台 Document 保存：只消费前台捕获的不可变请求，不回读实时正文。
pub(crate) struct DocumentSaveService {
    resolver: Arc<dyn DocumentPathResolver>,
}

impl Default for DocumentSaveService {
    fn default() -> Self {
        Self {
            resolver: Arc::new(SystemDocumentPathResolver),
        }
    }
}

impl DocumentSaveService {
    #[cfg(test)]
    fn with_resolver(resolver: Arc<dyn DocumentPathResolver>) -> Self {
        Self { resolver }
    }

    /// 编码、写前复核、临时文件与原子替换均只允许在后台执行体调用。
    pub(crate) fn save(
        &self,
        request: DocumentSaveRequest,
        grants: &[PathGrant],
        current_authorization_generation: u64,
    ) -> Result<DocumentSaveReceipt, SaveError> {
        let write = DocumentWriteContext::new(
            grants.to_vec(),
            request.authorization_generation(),
            current_authorization_generation,
            self.resolver.clone(),
        );
        Document::save_request(request, write)
    }

    /// 另存为只使用用户选定目标的单文件写授权；打开目标在写前拒绝。
    pub(crate) fn save_as(
        &self,
        registry: &DocumentRegistry,
        request: DocumentSaveRequest,
        target: CanonicalPath,
        current_authorization_generation: u64,
        existing_target: ExistingTargetPolicy,
    ) -> Result<DocumentSaveReceipt, SaveError> {
        let resolved = self.resolver.resolve_write_target(&target)?;
        let session_grant = PathGrant::save_as_target(resolved.canonical_path().clone());
        let path_request = PathRequest {
            operation: PathOperation::Write,
            lexical_path: target.as_path().to_path_buf(),
            root_hints: Vec::new(),
            session_grant: Some(session_grant),
            expected_authorization_generation: request.authorization_generation(),
        };
        let authorization = PathPolicy::authorize(
            &path_request,
            &resolved,
            &[],
            current_authorization_generation,
        )
        .map_err(|_| SaveError::Unauthorized)?;
        let resolved = authorization.resolved_path().clone();
        if registry.existing(&resolved).is_some() {
            return Err(SaveError::TargetOpen);
        }
        if resolved.file_identity().is_some()
            && existing_target == ExistingTargetPolicy::RequireConfirmation
        {
            return Err(SaveError::TargetExists);
        }
        let write = DocumentWriteContext::new(
            Vec::new(),
            request.authorization_generation(),
            current_authorization_generation,
            self.resolver.clone(),
        );
        Document::save_as_request(request, resolved, write)
    }
}

#[cfg(test)]
mod documents_tests {
    use std::time::Instant;

    use slterm_document::{
        DocumentOpenOutcome, DocumentRegistry, DocumentSaveRequest, EditOrigin, EditTransaction,
        MergePolicy, TextEdit,
    };
    use slterm_path::{PathGrant, ResolvedPath};

    use super::*;

    fn edited_save_request(
        registry: &mut DocumentRegistry,
        id: slterm_document::DocumentId,
        replacement: &str,
        authorization_generation: u64,
    ) -> DocumentSaveRequest {
        let revision = registry.session(id).unwrap().text_revision();
        let end = registry.session(id).unwrap().text().unwrap().len();
        registry
            .apply(
                EditTransaction {
                    document_id: id,
                    expected_text_revision: revision,
                    edits: vec![TextEdit::new(0..end, replacement)],
                    origin: EditOrigin::ReplaceAll,
                    merge_policy: MergePolicy::Disallowed,
                },
                TextStateId::from_uuid(Uuid::now_v7()).unwrap(),
                Instant::now(),
            )
            .unwrap();
        registry.begin_save(id, authorization_generation).unwrap()
    }

    #[test]
    fn large_file_opens_as_paged_source_without_prefix_rope() {
        let temp = tempfile::tempdir().unwrap();
        let root = CanonicalPath::new(dunce::canonicalize(temp.path()).unwrap()).unwrap();
        let file = temp.path().join("large.txt");
        let mut bytes = b"paged-start\n".to_vec();
        bytes.resize(slterm_document::EDITABLE_LIMIT_BYTES as usize + 1, b'x');
        std::fs::write(&file, bytes).unwrap();
        let mut registry = DocumentRegistry::new();

        let opened = DocumentOpenService::default()
            .open(&mut registry, &root, &CanonicalPath::new(&file).unwrap())
            .unwrap();

        let session = registry.session(opened.id).unwrap();
        assert!(session.text().is_none());
        assert!(session.paged().is_some());
        assert!(session.is_read_only());
        assert_eq!(
            session.paged().unwrap().index().total_bytes(),
            slterm_document::EDITABLE_LIMIT_BYTES + 1
        );
    }

    #[test]
    fn path_capabilities_union_for_save() {
        let temp = tempfile::tempdir().unwrap();
        let root_path = dunce::canonicalize(temp.path()).unwrap();
        let nested_path = root_path.join("nested");
        std::fs::create_dir(&nested_path).unwrap();
        let file = nested_path.join("file.txt");
        std::fs::write(&file, "old").unwrap();
        let root = CanonicalPath::new(&root_path).unwrap();
        let nested = CanonicalPath::new(&nested_path).unwrap();
        let mut registry = DocumentRegistry::new();
        let opened = DocumentOpenService::default()
            .open(&mut registry, &root, &CanonicalPath::new(&file).unwrap())
            .unwrap();
        let request = edited_save_request(&mut registry, opened.id, "new", 9);
        let previous_identity = registry
            .session(opened.id)
            .unwrap()
            .location()
            .file_identity()
            .unwrap();
        let grants = [
            PathGrant::repo_root(root),
            PathGrant::agent_root(nested, true, false),
        ];

        let receipt = DocumentSaveService::default()
            .save(request, &grants, 9)
            .unwrap();
        registry.complete_save(receipt).unwrap();

        assert_eq!(std::fs::read_to_string(&file).unwrap(), "new");
        assert!(!registry.session(opened.id).unwrap().is_dirty());
        assert_eq!(
            registry
                .session(opened.id)
                .unwrap()
                .disk_baseline()
                .as_ref(),
            b"new"
        );
        let location = registry.session(opened.id).unwrap().location();
        assert_ne!(location.file_identity(), Some(previous_identity));
        assert_eq!(registry.existing(location), Some(opened.id));
        let old_location =
            ResolvedPath::new(location.canonical_path().clone(), Some(previous_identity));
        assert_eq!(registry.existing(&old_location), None);
    }

    struct RedirectResolver {
        target: ResolvedPath,
    }

    impl DocumentPathResolver for RedirectResolver {
        fn resolve(&self, _lexical: &CanonicalPath) -> io::Result<ResolvedPath> {
            Ok(self.target.clone())
        }
    }

    #[test]
    fn path_rechecks_redirect_before_write() {
        let temp = tempfile::tempdir().unwrap();
        let root_path = dunce::canonicalize(temp.path()).unwrap();
        let project_path = root_path.join("project");
        std::fs::create_dir(&project_path).unwrap();
        let file = project_path.join("file.txt");
        let outside = root_path.join("outside.txt");
        std::fs::write(&file, "old").unwrap();
        std::fs::write(&outside, "outside").unwrap();
        let root = CanonicalPath::new(&project_path).unwrap();
        let mut registry = DocumentRegistry::new();
        let opened = DocumentOpenService::default()
            .open(&mut registry, &root, &CanonicalPath::new(&file).unwrap())
            .unwrap();
        let request = edited_save_request(&mut registry, opened.id, "new", 4);
        let outside_file = std::fs::File::open(&outside).unwrap();
        let target = ResolvedPath::new(
            CanonicalPath::new(dunce::canonicalize(&outside).unwrap()).unwrap(),
            Some(crate::atomic_file::file_identity(&outside_file).unwrap()),
        );
        let service = DocumentSaveService::with_resolver(Arc::new(RedirectResolver { target }));

        let result = service.save(request, &[PathGrant::project_root(root)], 4);

        assert!(matches!(
            result,
            Err(crate::text_document::SaveError::Unauthorized)
        ));
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "old");
        assert_eq!(std::fs::read_to_string(&outside).unwrap(), "outside");
        assert!(registry.session(opened.id).unwrap().is_saving());
        registry
            .fail_save(opened.id, 1)
            .expect("失败回执只释放在途保存");
        assert!(registry.session(opened.id).unwrap().is_dirty());
    }

    #[test]
    fn unrepresentable_save_preserves_disk_and_dirty_state() {
        let temp = tempfile::tempdir().unwrap();
        let root_path = dunce::canonicalize(temp.path()).unwrap();
        let file = root_path.join("gbk.txt");
        std::fs::write(&file, b"old").unwrap();
        let root = CanonicalPath::new(&root_path).unwrap();
        let handle = std::fs::File::open(&file).unwrap();
        let location = ResolvedPath::new(
            CanonicalPath::new(dunce::canonicalize(&file).unwrap()).unwrap(),
            Some(crate::atomic_file::file_identity(&handle).unwrap()),
        );
        drop(handle);
        let id = slterm_document::DocumentId::from_uuid(Uuid::now_v7()).unwrap();
        let state = TextStateId::from_uuid(Uuid::now_v7()).unwrap();
        let mut registry = DocumentRegistry::new();
        registry
            .open(
                id,
                location,
                ropey::Rope::from_str("old"),
                state,
                Arc::from(b"old".as_slice()),
                slterm_document::DocumentMetadata {
                    encoding: slterm_document::TextEncoding {
                        encoding: slterm_document::DocumentEncoding::Gbk,
                        bom: false,
                        line_ending: slterm_document::LineEnding::Lf,
                        trailing_newline: false,
                    },
                    read_only: false,
                },
            )
            .unwrap();
        let request = edited_save_request(&mut registry, id, "😀", 0);

        let result = DocumentSaveService::default().save(
            request,
            &[PathGrant::project_root(root.clone())],
            0,
        );

        assert!(matches!(
            result,
            Err(crate::text_document::SaveError::Unrepresentable)
        ));
        assert_eq!(std::fs::read(&file).unwrap(), b"old");
        registry.fail_save(id, 1).unwrap();
        assert!(registry.session(id).unwrap().is_dirty());
        assert_eq!(
            registry.session(id).unwrap().text().unwrap().to_string(),
            "😀"
        );

        let request = edited_save_request(&mut registry, id, "中文", 0);
        let receipt = DocumentSaveService::default()
            .save(request, &[PathGrant::project_root(root)], 0)
            .unwrap();
        registry.complete_save(receipt).unwrap();
        let encoded = [0xd6, 0xd0, 0xce, 0xc4];
        assert_eq!(std::fs::read(&file).unwrap(), encoded);
        assert_eq!(
            registry.session(id).unwrap().disk_baseline().as_ref(),
            encoded
        );
        assert!(!registry.session(id).unwrap().is_dirty());
    }

    #[test]
    fn document_save_as_rejects_open_target() {
        let temp = tempfile::tempdir().unwrap();
        let root = CanonicalPath::new(dunce::canonicalize(temp.path()).unwrap()).unwrap();
        let source = temp.path().join("source.txt");
        let target = temp.path().join("target.txt");
        std::fs::write(&source, "source").unwrap();
        std::fs::write(&target, "target").unwrap();
        let opener = DocumentOpenService::default();
        let mut registry = DocumentRegistry::new();
        let source_open = opener
            .open(&mut registry, &root, &CanonicalPath::new(&source).unwrap())
            .unwrap();
        let target_open = opener
            .open(&mut registry, &root, &CanonicalPath::new(&target).unwrap())
            .unwrap();
        let source_location = registry.session(source_open.id).unwrap().location().clone();
        let request = edited_save_request(&mut registry, source_open.id, "draft", 0);

        let result = DocumentSaveService::default().save_as(
            &registry,
            request,
            CanonicalPath::new(&target).unwrap(),
            0,
            ExistingTargetPolicy::RequireConfirmation,
        );

        assert!(matches!(
            result,
            Err(crate::text_document::SaveError::TargetOpen)
        ));
        assert_eq!(std::fs::read_to_string(&source).unwrap(), "source");
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "target");
        assert_eq!(registry.len(), 2);
        assert_eq!(
            registry.session(source_open.id).unwrap().location(),
            &source_location
        );
        assert!(registry.session(target_open.id).is_some());
        registry.fail_save(source_open.id, 1).unwrap();
        assert!(registry.session(source_open.id).unwrap().is_dirty());
    }

    #[test]
    fn document_save_as_rebinds_without_rebuilding() {
        let temp = tempfile::tempdir().unwrap();
        let root_path = dunce::canonicalize(temp.path()).unwrap();
        let root = CanonicalPath::new(&root_path).unwrap();
        let source = root_path.join("source.txt");
        let target = root_path.join("target.txt");
        std::fs::write(&source, "source").unwrap();
        let mut registry = DocumentRegistry::new();
        let opened = DocumentOpenService::default()
            .open(&mut registry, &root, &CanonicalPath::new(&source).unwrap())
            .unwrap();
        let request = edited_save_request(&mut registry, opened.id, "draft", 0);
        let current = TextStateId::from_uuid(Uuid::now_v7()).unwrap();
        registry
            .apply(
                EditTransaction {
                    document_id: opened.id,
                    expected_text_revision: request.text_revision(),
                    edits: vec![TextEdit::new(5..5, " newer")],
                    origin: EditOrigin::UserInput,
                    merge_policy: MergePolicy::Adjacent,
                },
                current,
                Instant::now(),
            )
            .unwrap();

        let receipt = DocumentSaveService::default()
            .save_as(
                &registry,
                request,
                CanonicalPath::new(&target).unwrap(),
                0,
                ExistingTargetPolicy::RequireConfirmation,
            )
            .unwrap();
        registry.complete_save_as(receipt).unwrap();

        assert_eq!(std::fs::read_to_string(&source).unwrap(), "source");
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "draft");
        let session = registry.session(opened.id).unwrap();
        assert_eq!(session.id(), opened.id);
        assert_eq!(session.text().unwrap().to_string(), "draft newer");
        assert_eq!(session.location().canonical_path().as_path(), target);
        assert_eq!(session.disk_baseline().as_ref(), b"draft");
        assert_eq!(session.current_state(), current);
        assert!(session.is_dirty());
        assert_eq!(registry.existing(session.location()), Some(opened.id));
        registry.undo(opened.id).unwrap();
        assert!(!registry.session(opened.id).unwrap().is_dirty());
    }

    #[test]
    fn document_save_as_existing_target_requires_confirmation() {
        let temp = tempfile::tempdir().unwrap();
        let root = CanonicalPath::new(dunce::canonicalize(temp.path()).unwrap()).unwrap();
        let source = temp.path().join("source.txt");
        let target = temp.path().join("target.txt");
        std::fs::write(&source, "source").unwrap();
        std::fs::write(&target, "target").unwrap();
        let mut registry = DocumentRegistry::new();
        let opened = DocumentOpenService::default()
            .open(&mut registry, &root, &CanonicalPath::new(&source).unwrap())
            .unwrap();
        let request = edited_save_request(&mut registry, opened.id, "draft", 0);

        let result = DocumentSaveService::default().save_as(
            &registry,
            request,
            CanonicalPath::new(&target).unwrap(),
            0,
            ExistingTargetPolicy::RequireConfirmation,
        );

        assert!(matches!(
            result,
            Err(crate::text_document::SaveError::TargetExists)
        ));
        assert_eq!(std::fs::read_to_string(&source).unwrap(), "source");
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "target");
        registry.fail_save(opened.id, 1).unwrap();

        let request = registry.begin_save(opened.id, 0).unwrap();
        let receipt = DocumentSaveService::default()
            .save_as(
                &registry,
                request,
                CanonicalPath::new(&target).unwrap(),
                0,
                ExistingTargetPolicy::OverwriteConfirmed,
            )
            .unwrap();
        registry.complete_save_as(receipt).unwrap();
        assert_eq!(std::fs::read_to_string(&source).unwrap(), "source");
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "draft");
        assert!(!registry.session(opened.id).unwrap().is_dirty());
    }

    #[test]
    fn save_requires_write_capability_not_mere_read_access() {
        let temp = tempfile::tempdir().unwrap();
        let root = CanonicalPath::new(dunce::canonicalize(temp.path()).unwrap()).unwrap();
        let file = temp.path().join("file.txt");
        std::fs::write(&file, "old").unwrap();
        let mut registry = DocumentRegistry::new();
        let opened = DocumentOpenService::default()
            .open(&mut registry, &root, &CanonicalPath::new(&file).unwrap())
            .unwrap();
        let request = edited_save_request(&mut registry, opened.id, "new", 0);

        let result = DocumentSaveService::default().save(request, &[PathGrant::repo_root(root)], 0);

        assert!(matches!(
            result,
            Err(crate::text_document::SaveError::Unauthorized)
        ));
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "old");
        registry.fail_save(opened.id, 1).unwrap();
        assert!(registry.session(opened.id).unwrap().is_dirty());
    }

    #[test]
    fn save_rejects_changed_authorization_generation_without_writing() {
        let temp = tempfile::tempdir().unwrap();
        let root = CanonicalPath::new(dunce::canonicalize(temp.path()).unwrap()).unwrap();
        let file = temp.path().join("file.txt");
        std::fs::write(&file, "old").unwrap();
        let mut registry = DocumentRegistry::new();
        let opened = DocumentOpenService::default()
            .open(&mut registry, &root, &CanonicalPath::new(&file).unwrap())
            .unwrap();
        let request = edited_save_request(&mut registry, opened.id, "new", 4);

        let result =
            DocumentSaveService::default().save(request, &[PathGrant::project_root(root)], 5);

        assert!(matches!(
            result,
            Err(crate::text_document::SaveError::Unauthorized)
        ));
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "old");
        registry.fail_save(opened.id, 1).unwrap();
        assert!(registry.session(opened.id).unwrap().is_dirty());
    }

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
        assert!(second.body.is_none());
        assert_eq!(registry.len(), 1);
        assert_eq!(
            registry
                .session(first.id)
                .unwrap()
                .text()
                .unwrap()
                .to_string(),
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
    fn editor_encoding_and_binary_routing() {
        let temp = tempfile::tempdir().unwrap();
        let root = CanonicalPath::new(temp.path()).unwrap();
        let gbk = temp.path().join("gbk.txt");
        let invalid = temp.path().join("invalid.txt");
        let binary = temp.path().join("binary.txt");
        std::fs::write(&gbk, [0xd6, 0xd0, 0xce, 0xc4]).unwrap();
        std::fs::write(&invalid, [0xff]).unwrap();
        std::fs::write(&binary, b"text\0binary").unwrap();
        let content = slterm_document::DocumentContent::decode(
            std::fs::read(&gbk).unwrap(),
            4,
            Some(slterm_document::DocumentEncoding::Gbk),
        );
        assert!(matches!(
            content,
            slterm_document::DocumentContent::Editable(text) if text.text == "中文"
        ));

        let service = DocumentOpenService::default();
        let mut registry = DocumentRegistry::new();
        for path in [invalid, binary] {
            let result = service.open(&mut registry, &root, &CanonicalPath::new(path).unwrap());
            assert!(result.is_err());
            assert!(registry.is_empty());
        }
    }

    #[test]
    fn real_open_preserves_read_only_contract() {
        let temp = tempfile::tempdir().unwrap();
        let root = CanonicalPath::new(temp.path()).unwrap();
        let file = temp.path().join("readonly.txt");
        std::fs::write(&file, "fixed").unwrap();
        let mut permissions = std::fs::metadata(&file).unwrap().permissions();
        permissions.set_readonly(true);
        std::fs::set_permissions(&file, permissions).unwrap();
        let service = DocumentOpenService::default();
        let mut registry = DocumentRegistry::new();

        let opened = service
            .open(&mut registry, &root, &CanonicalPath::new(&file).unwrap())
            .unwrap();

        assert!(registry.session(opened.id).unwrap().is_read_only());
        #[allow(clippy::permissions_set_readonly_false)]
        {
            let mut permissions = std::fs::metadata(&file).unwrap().permissions();
            permissions.set_readonly(false);
            std::fs::set_permissions(&file, permissions).unwrap();
        }
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
        assert!(matches!(error, DocumentOpenError::Unauthorized));
        assert!(registry.is_empty());
    }
}
