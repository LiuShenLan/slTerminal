//! Document 聚合：稳定身份、文件身份与规范路径去重的唯一事实源。

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;

use ropey::Rope;
use slterm_path::{CanonicalPath, FileIdentity, ResolvedPath};

use crate::{DocumentBody, DocumentId, PagedSource, TextEncoding, TextStateId};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DocumentMetadata {
    pub encoding: TextEncoding,
    pub read_only: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DocumentSession {
    pub(crate) id: DocumentId,
    pub(crate) location: ResolvedPath,
    pub(crate) body: DocumentBody,
    pub(crate) paged_generation: u64,
    pub(crate) text_revision: u64,
    pub(crate) current_state: TextStateId,
    pub(crate) saved_state: TextStateId,
    /// 最近一次保存完成的编码事实；行尾切换只改当前编码并使其进入脏态。
    pub(crate) saved_encoding: TextEncoding,
    /// 用户手动只读：瞬态、不进入快照或持久状态，只可对干净可编辑正文开启。
    pub(crate) manual_read_only: bool,
    pub(crate) disk_baseline: Arc<[u8]>,
    pub(crate) save_generation: u64,
    pub(crate) active_save: Option<u64>,
    pub(crate) external_conflict: Option<crate::DocumentConflict>,
    pub(crate) external_generation: u64,
    pub(crate) read_revoked: bool,
    pub(crate) metadata: DocumentMetadata,
    pub(crate) history: crate::edit::EditHistory,
    pub(crate) known_states: BTreeSet<TextStateId>,
}

impl DocumentSession {
    pub const fn id(&self) -> DocumentId {
        self.id
    }

    pub fn location(&self) -> &ResolvedPath {
        &self.location
    }

    pub fn text(&self) -> Option<&Rope> {
        match &self.body {
            DocumentBody::Editable(text) => Some(text),
            DocumentBody::Paged(_) => None,
        }
    }

    pub fn paged(&self) -> Option<&PagedSource> {
        match &self.body {
            DocumentBody::Paged(source) => Some(source),
            DocumentBody::Editable(_) => None,
        }
    }

    pub const fn text_revision(&self) -> u64 {
        self.text_revision
    }

    pub const fn current_state(&self) -> TextStateId {
        self.current_state
    }

    pub const fn saved_state(&self) -> TextStateId {
        self.saved_state
    }

    pub fn disk_baseline(&self) -> &Arc<[u8]> {
        &self.disk_baseline
    }

    pub const fn is_saving(&self) -> bool {
        self.active_save.is_some()
    }

    pub fn external_conflict(&self) -> Option<&crate::DocumentConflict> {
        self.external_conflict.as_ref()
    }

    pub const fn is_read_revoked(&self) -> bool {
        self.read_revoked
    }

    pub fn is_dirty(&self) -> bool {
        self.current_state != self.saved_state || self.metadata.encoding != self.saved_encoding
    }

    pub const fn is_read_only(&self) -> bool {
        self.metadata.read_only
            || self.manual_read_only
            || matches!(&self.body, DocumentBody::Paged(_))
    }

    /// 磁盘或大文件导致的持久只读原因；手动只读不在此列。
    pub const fn is_inherently_read_only(&self) -> bool {
        self.metadata.read_only || matches!(&self.body, DocumentBody::Paged(_))
    }

    pub const fn is_manually_read_only(&self) -> bool {
        self.manual_read_only
    }

    pub const fn encoding(&self) -> &TextEncoding {
        &self.metadata.encoding
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DocumentOpenOutcome {
    Opened,
    Existing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DocumentRegistryError {
    DocumentIdConflict(DocumentId),
    RecoveryTargetOccupied(DocumentId),
}

/// 文档元数据显式切换的封闭失败：不存在、只读或脏态不允许。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DocumentMetadataError {
    DocumentNotFound(DocumentId),
    ReadOnly,
    Dirty,
    NotEditable,
}

impl fmt::Display for DocumentMetadataError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::DocumentNotFound(_) => "Document 不存在",
            Self::ReadOnly => "只读 Document 不接受元数据写入",
            Self::Dirty => "存在未保存修改时不得开启手动只读",
            Self::NotEditable => "分页或不可编辑正文没有行尾切换",
        })
    }
}

impl std::error::Error for DocumentMetadataError {}

impl fmt::Display for DocumentRegistryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DocumentIdConflict(_) => formatter.write_str("DocumentId 已绑定其他存活文档"),
            Self::RecoveryTargetOccupied(_) => {
                formatter.write_str("恢复目标已关联其他 Document，拒绝合并恢复输入")
            }
        }
    }
}

impl std::error::Error for DocumentRegistryError {}

/// 全局存活 Document；文件身份优先，无法取得身份时才按规范路径去重。
#[derive(Clone, Default)]
pub struct DocumentRegistry {
    sessions: BTreeMap<DocumentId, DocumentSession>,
    identities: BTreeMap<FileIdentity, DocumentId>,
    paths: BTreeMap<CanonicalPath, DocumentId>,
}

impl DocumentRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn session(&self, id: DocumentId) -> Option<&DocumentSession> {
        self.sessions.get(&id)
    }

    pub fn ids(&self) -> impl Iterator<Item = DocumentId> + '_ {
        self.sessions.keys().copied()
    }

    pub(crate) fn session_mut(&mut self, id: DocumentId) -> Option<&mut DocumentSession> {
        self.sessions.get_mut(&id)
    }

    pub fn len(&self) -> usize {
        self.sessions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }

    /// 调用方可在读盘前查重；路径被另一文件身份替换时不得命中旧聚合。
    pub fn existing(&self, location: &ResolvedPath) -> Option<DocumentId> {
        if let Some(identity) = location.file_identity()
            && let Some(id) = self.identities.get(&identity)
        {
            return Some(*id);
        }
        let path = location.canonical_path();
        let path_match = self.paths.get(path).copied();
        match (location.file_identity(), path_match) {
            (Some(actual), Some(id))
                if self.sessions[&id]
                    .location
                    .file_identity()
                    .is_some_and(|existing| existing != actual) =>
            {
                None
            }
            _ => path_match,
        }
    }

    pub fn open(
        &mut self,
        id: DocumentId,
        location: ResolvedPath,
        text: impl Into<DocumentBody>,
        state: TextStateId,
        disk_baseline: Arc<[u8]>,
        metadata: DocumentMetadata,
    ) -> Result<(DocumentId, DocumentOpenOutcome), DocumentRegistryError> {
        if self.sessions.contains_key(&id) {
            return Err(DocumentRegistryError::DocumentIdConflict(id));
        }
        if let Some(existing) = self.existing(&location) {
            let path = location.canonical_path().clone();
            let session = self
                .sessions
                .get_mut(&existing)
                .expect("索引必须指向存活 Document");
            if session.location.file_identity().is_none() {
                session.location = ResolvedPath::new(
                    session.location.canonical_path().clone(),
                    location.file_identity(),
                );
            }
            if let Some(identity) = location.file_identity() {
                self.identities.entry(identity).or_insert(existing);
            }
            self.paths.insert(path, existing);
            return Ok((existing, DocumentOpenOutcome::Existing));
        }

        let path = location.canonical_path().clone();
        let body = text.into();
        let paged_generation = match &body {
            DocumentBody::Paged(source) => source.source_generation(),
            DocumentBody::Editable(_) => 0,
        };
        self.sessions.insert(
            id,
            DocumentSession {
                id,
                location: location.clone(),
                body,
                paged_generation,
                text_revision: 0,
                current_state: state,
                saved_state: state,
                saved_encoding: metadata.encoding.clone(),
                manual_read_only: false,
                disk_baseline,
                save_generation: 0,
                active_save: None,
                external_conflict: None,
                external_generation: 0,
                read_revoked: false,
                metadata,
                history: crate::edit::EditHistory::default(),
                known_states: BTreeSet::from([state]),
            },
        );
        if let Some(identity) = location.file_identity() {
            self.identities.insert(identity, id);
        }
        self.paths.insert(path, id);
        Ok((id, DocumentOpenOutcome::Opened))
    }

    pub(crate) fn rebind_path(&mut self, id: DocumentId) {
        self.paths.retain(|_, owner| *owner != id);
        let path = self.sessions[&id].location.canonical_path().clone();
        self.paths.insert(path, id);
    }

    /// 文件身份已归属的聚合；调用方在重绑前用它拒绝冲突目标。
    pub(crate) fn identity_owner(&self, identity: FileIdentity) -> Option<DocumentId> {
        self.identities.get(&identity).copied()
    }

    pub(crate) fn refresh_identity(&mut self, id: DocumentId, identity: Option<FileIdentity>) {
        self.identities.retain(|_, owner| *owner != id);
        if let Some(identity) = identity {
            // 身份唯一索引不得改写其他聚合；调用方应在变更前拒绝冲突，
            // 这里保留最后一道防御，冲突时只是不建立本聚合的索引。
            if self
                .identities
                .get(&identity)
                .is_none_or(|owner| *owner == id)
            {
                self.identities.insert(identity, id);
            }
        }
    }

    /// 行尾切换属于一次 Document 修改：不留撤销步，保存时统一为所选行尾。
    pub fn set_line_ending(
        &mut self,
        id: DocumentId,
        line_ending: crate::LineEnding,
    ) -> Result<(), DocumentMetadataError> {
        let session = self
            .session_mut(id)
            .ok_or(DocumentMetadataError::DocumentNotFound(id))?;
        if !matches!(&session.body, DocumentBody::Editable(_)) {
            return Err(DocumentMetadataError::NotEditable);
        }
        if session.is_read_only() {
            return Err(DocumentMetadataError::ReadOnly);
        }
        session.metadata.encoding.line_ending = line_ending;
        Ok(())
    }

    /// 手动只读只对干净可编辑正文开启，可随时关闭；不覆盖持久只读原因。
    pub fn set_manual_read_only(
        &mut self,
        id: DocumentId,
        enabled: bool,
    ) -> Result<(), DocumentMetadataError> {
        let session = self
            .session_mut(id)
            .ok_or(DocumentMetadataError::DocumentNotFound(id))?;
        if enabled {
            if session.is_inherently_read_only() {
                return Err(DocumentMetadataError::ReadOnly);
            }
            if !matches!(&session.body, DocumentBody::Editable(_)) {
                return Err(DocumentMetadataError::NotEditable);
            }
            if session.is_dirty() {
                return Err(DocumentMetadataError::Dirty);
            }
        }
        session.manual_read_only = enabled;
        Ok(())
    }

    /// 回收聚合及其全部别名索引；不存在时不产生副作用。
    pub fn remove(&mut self, id: DocumentId) -> Option<DocumentSession> {
        let session = self.sessions.remove(&id)?;
        self.identities.retain(|_, owner| *owner != id);
        self.paths.retain(|_, owner| *owner != id);
        Some(session)
    }

    /// 按当前布局引用集合回收存活聚合。
    pub fn retain(&mut self, mut keep: impl FnMut(DocumentId) -> bool) {
        let stale = self
            .sessions
            .keys()
            .copied()
            .filter(|id| !keep(*id))
            .collect::<Vec<_>>();
        for id in stale {
            self.remove(id);
        }
    }
}

#[cfg(test)]
mod registry_tests {
    use std::str::FromStr;

    use super::*;

    const DOCUMENT_A: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2a";
    const DOCUMENT_B: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b";
    const STATE: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0f2a";

    fn id(text: &str) -> DocumentId {
        DocumentId::from_str(text).unwrap()
    }

    fn state() -> TextStateId {
        TextStateId::from_str(STATE).unwrap()
    }

    fn location(path: &str, identity: Option<u128>) -> ResolvedPath {
        ResolvedPath::new(
            CanonicalPath::new(path).unwrap(),
            identity.map(|file| FileIdentity { volume: 7, file }),
        )
    }

    #[test]
    fn registry_aliases_resolve_to_the_same_document_by_file_identity() {
        let mut registry = DocumentRegistry::new();
        let first = registry
            .open(
                id(DOCUMENT_A),
                location("D:/work/a/file.txt", Some(41)),
                Rope::from_str("original"),
                state(),
                Arc::default(),
                DocumentMetadata::default(),
            )
            .unwrap();
        let alias = registry
            .open(
                id(DOCUMENT_B),
                location("D:/work/b/alias.txt", Some(41)),
                Rope::from_str("must-not-replace"),
                state(),
                Arc::default(),
                DocumentMetadata::default(),
            )
            .unwrap();

        assert_eq!(first, (id(DOCUMENT_A), DocumentOpenOutcome::Opened));
        assert_eq!(alias, (id(DOCUMENT_A), DocumentOpenOutcome::Existing));
        assert_eq!(registry.len(), 1);
        assert_eq!(
            registry
                .session(id(DOCUMENT_A))
                .unwrap()
                .text()
                .unwrap()
                .to_string(),
            "original"
        );
        assert_eq!(
            registry
                .session(id(DOCUMENT_A))
                .unwrap()
                .location()
                .canonical_path(),
            &CanonicalPath::new("D:/work/a/file.txt").unwrap()
        );
    }

    #[test]
    fn registry_enriches_path_fallback_with_file_identity() {
        let mut registry = DocumentRegistry::new();
        registry
            .open(
                id(DOCUMENT_A),
                location("D:/work/file.txt", None),
                Rope::from_str("fallback"),
                state(),
                Arc::default(),
                DocumentMetadata::default(),
            )
            .unwrap();

        let existing = registry
            .open(
                id(DOCUMENT_B),
                location("D:/work/file.txt", Some(99)),
                Rope::from_str("ignored"),
                state(),
                Arc::default(),
                DocumentMetadata::default(),
            )
            .unwrap();

        assert_eq!(existing, (id(DOCUMENT_A), DocumentOpenOutcome::Existing));
        assert_eq!(
            registry.existing(&location("D:/work/file.txt", Some(99))),
            Some(id(DOCUMENT_A))
        );
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn replaced_path_identity_does_not_inherit_old_content() {
        let mut registry = DocumentRegistry::new();
        registry
            .open(
                id(DOCUMENT_A),
                location("D:/work/file.txt", Some(1)),
                Rope::from_str("old"),
                state(),
                Arc::default(),
                DocumentMetadata::default(),
            )
            .unwrap();

        let replacement = registry
            .open(
                id(DOCUMENT_B),
                location("D:/work/file.txt", Some(2)),
                Rope::from_str("new"),
                state(),
                Arc::default(),
                DocumentMetadata::default(),
            )
            .unwrap();

        assert_eq!(replacement, (id(DOCUMENT_B), DocumentOpenOutcome::Opened));
        assert_eq!(registry.len(), 2);
        assert_eq!(
            registry
                .session(id(DOCUMENT_A))
                .unwrap()
                .text()
                .unwrap()
                .to_string(),
            "old"
        );
        assert_eq!(
            registry
                .session(id(DOCUMENT_B))
                .unwrap()
                .text()
                .unwrap()
                .to_string(),
            "new"
        );
    }

    #[test]
    fn removing_a_document_clears_every_identity_and_path_alias() {
        let mut registry = DocumentRegistry::new();
        registry
            .open(
                id(DOCUMENT_A),
                location("D:/work/a/file.txt", Some(41)),
                Rope::from_str("original"),
                state(),
                Arc::default(),
                DocumentMetadata::default(),
            )
            .unwrap();
        registry
            .open(
                id(DOCUMENT_B),
                location("D:/work/b/alias.txt", Some(41)),
                Rope::from_str("ignored"),
                state(),
                Arc::default(),
                DocumentMetadata::default(),
            )
            .unwrap();

        assert!(registry.remove(id(DOCUMENT_A)).is_some());

        assert!(registry.is_empty());
        assert_eq!(
            registry.existing(&location("D:/work/a/file.txt", Some(41))),
            None
        );
        assert_eq!(
            registry.existing(&location("D:/work/b/alias.txt", Some(41))),
            None
        );
        assert_eq!(registry.remove(id(DOCUMENT_A)), None);
    }

    #[test]
    fn retain_removes_only_documents_outside_the_reference_set() {
        let mut registry = DocumentRegistry::new();
        for (document, path) in [(DOCUMENT_A, "D:/work/a.txt"), (DOCUMENT_B, "D:/work/b.txt")] {
            registry
                .open(
                    id(document),
                    location(path, None),
                    Rope::from_str(document),
                    state(),
                    Arc::default(),
                    DocumentMetadata::default(),
                )
                .unwrap();
        }

        registry.retain(|document| document == id(DOCUMENT_A));

        assert_eq!(registry.len(), 1);
        assert!(registry.session(id(DOCUMENT_A)).is_some());
        assert_eq!(registry.existing(&location("D:/work/b.txt", None)), None);
    }

    #[test]
    fn line_ending_change_is_a_revertible_document_modification() {
        let mut registry = DocumentRegistry::new();
        registry
            .open(
                id(DOCUMENT_A),
                location("D:/work/a.txt", None),
                Rope::from_str("a"),
                state(),
                Arc::default(),
                DocumentMetadata::default(),
            )
            .unwrap();
        assert!(!registry.session(id(DOCUMENT_A)).unwrap().is_dirty());

        registry
            .set_line_ending(id(DOCUMENT_A), crate::LineEnding::Lf)
            .unwrap();
        assert_eq!(
            registry
                .session(id(DOCUMENT_A))
                .unwrap()
                .encoding()
                .line_ending,
            crate::LineEnding::Lf
        );
        assert!(
            registry.session(id(DOCUMENT_A)).unwrap().is_dirty(),
            "行尾切换属于 Document 修改"
        );
        registry
            .set_line_ending(id(DOCUMENT_A), crate::LineEnding::Crlf)
            .unwrap();
        assert!(
            !registry.session(id(DOCUMENT_A)).unwrap().is_dirty(),
            "切回已保存行尾即恢复干净"
        );
    }

    #[test]
    fn manual_read_only_requires_clean_editable_text_and_blocks_edits() {
        use crate::{
            DocumentMetadataError, EditApplyOutcome, EditOrigin, EditTransaction, MergePolicy,
            TextEdit,
        };

        let mut registry = DocumentRegistry::new();
        registry
            .open(
                id(DOCUMENT_A),
                location("D:/work/a.txt", None),
                Rope::from_str("abc"),
                state(),
                Arc::default(),
                DocumentMetadata::default(),
            )
            .unwrap();
        registry.set_manual_read_only(id(DOCUMENT_A), true).unwrap();
        let session = registry.session(id(DOCUMENT_A)).unwrap();
        assert!(session.is_read_only());
        assert!(session.is_manually_read_only());
        let blocked = registry.apply(
            EditTransaction {
                document_id: id(DOCUMENT_A),
                expected_text_revision: 0,
                edits: vec![TextEdit::new(0..1, "x")],
                origin: EditOrigin::UserInput,
                merge_policy: MergePolicy::Disallowed,
            },
            crate::TextStateId::from_str("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0f2a").unwrap(),
            std::time::Instant::now(),
        );
        assert!(blocked.is_err());
        registry
            .set_manual_read_only(id(DOCUMENT_A), false)
            .unwrap();
        assert_eq!(
            registry
                .apply(
                    EditTransaction {
                        document_id: id(DOCUMENT_A),
                        expected_text_revision: 0,
                        edits: vec![TextEdit::new(0..1, "x")],
                        origin: EditOrigin::UserInput,
                        merge_policy: MergePolicy::Disallowed,
                    },
                    crate::TextStateId::from_str("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0f2b").unwrap(),
                    std::time::Instant::now(),
                )
                .unwrap(),
            EditApplyOutcome::Applied
        );
        assert_eq!(
            registry.set_manual_read_only(id(DOCUMENT_A), true),
            Err(DocumentMetadataError::Dirty),
            "脏正文不得开启手动只读"
        );
    }

    #[test]
    fn duplicate_document_id_cannot_rebind_another_file() {
        let mut registry = DocumentRegistry::new();
        registry
            .open(
                id(DOCUMENT_A),
                location("D:/work/a.txt", Some(1)),
                Rope::from_str("a"),
                state(),
                Arc::default(),
                DocumentMetadata::default(),
            )
            .unwrap();

        let result = registry.open(
            id(DOCUMENT_A),
            location("D:/work/b.txt", Some(2)),
            Rope::from_str("b"),
            state(),
            Arc::default(),
            DocumentMetadata::default(),
        );

        assert_eq!(
            result,
            Err(DocumentRegistryError::DocumentIdConflict(id(DOCUMENT_A)))
        );
        assert_eq!(registry.len(), 1);
        assert_eq!(
            registry
                .session(id(DOCUMENT_A))
                .unwrap()
                .text()
                .unwrap()
                .to_string(),
            "a"
        );
    }
}
