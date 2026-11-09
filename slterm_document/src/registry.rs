//! Document 聚合：稳定身份、文件身份与规范路径去重的唯一事实源。

use std::collections::BTreeMap;
use std::fmt;

use ropey::Rope;
use slterm_path::{CanonicalPath, FileIdentity, ResolvedPath};

use crate::{DocumentId, TextStateId};

#[derive(Clone, Debug, PartialEq)]
pub struct DocumentSession {
    id: DocumentId,
    location: ResolvedPath,
    text: Rope,
    text_revision: u64,
    current_state: TextStateId,
    saved_state: TextStateId,
}

impl DocumentSession {
    pub const fn id(&self) -> DocumentId {
        self.id
    }

    pub fn location(&self) -> &ResolvedPath {
        &self.location
    }

    pub fn text(&self) -> &Rope {
        &self.text
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

    pub fn is_dirty(&self) -> bool {
        self.current_state != self.saved_state
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
}

impl fmt::Display for DocumentRegistryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DocumentIdConflict(_) => formatter.write_str("DocumentId 已绑定其他存活文档"),
        }
    }
}

impl std::error::Error for DocumentRegistryError {}

/// 全局存活 Document；文件身份优先，无法取得身份时才按规范路径去重。
#[derive(Default)]
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
        text: Rope,
        state: TextStateId,
    ) -> Result<(DocumentId, DocumentOpenOutcome), DocumentRegistryError> {
        if self.sessions.contains_key(&id) {
            return Err(DocumentRegistryError::DocumentIdConflict(id));
        }
        if let Some(existing) = self.existing(&location) {
            let session = self
                .sessions
                .get_mut(&existing)
                .expect("索引必须指向存活 Document");
            if let Some(identity) = location.file_identity() {
                self.identities.entry(identity).or_insert(existing);
            }
            session.location = location;
            self.paths
                .insert(session.location.canonical_path().clone(), existing);
            return Ok((existing, DocumentOpenOutcome::Existing));
        }

        let path = location.canonical_path().clone();
        self.sessions.insert(
            id,
            DocumentSession {
                id,
                location: location.clone(),
                text,
                text_revision: 0,
                current_state: state,
                saved_state: state,
            },
        );
        if let Some(identity) = location.file_identity() {
            self.identities.insert(identity, id);
        }
        self.paths.insert(path, id);
        Ok((id, DocumentOpenOutcome::Opened))
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
    fn integration_document_global_identity() {
        let mut registry = DocumentRegistry::new();
        let first = registry
            .open(
                id(DOCUMENT_A),
                location("D:/work/a/file.txt", Some(41)),
                Rope::from_str("original"),
                state(),
            )
            .unwrap();
        let alias = registry
            .open(
                id(DOCUMENT_B),
                location("D:/work/b/alias.txt", Some(41)),
                Rope::from_str("must-not-replace"),
                state(),
            )
            .unwrap();

        assert_eq!(first, (id(DOCUMENT_A), DocumentOpenOutcome::Opened));
        assert_eq!(alias, (id(DOCUMENT_A), DocumentOpenOutcome::Existing));
        assert_eq!(registry.len(), 1);
        assert_eq!(
            registry.session(id(DOCUMENT_A)).unwrap().text().to_string(),
            "original"
        );
        assert_eq!(
            registry
                .session(id(DOCUMENT_A))
                .unwrap()
                .location()
                .canonical_path(),
            &CanonicalPath::new("D:/work/b/alias.txt").unwrap()
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
            )
            .unwrap();

        let existing = registry
            .open(
                id(DOCUMENT_B),
                location("D:/work/file.txt", Some(99)),
                Rope::from_str("ignored"),
                state(),
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
            )
            .unwrap();

        let replacement = registry
            .open(
                id(DOCUMENT_B),
                location("D:/work/file.txt", Some(2)),
                Rope::from_str("new"),
                state(),
            )
            .unwrap();

        assert_eq!(replacement, (id(DOCUMENT_B), DocumentOpenOutcome::Opened));
        assert_eq!(registry.len(), 2);
        assert_eq!(
            registry.session(id(DOCUMENT_A)).unwrap().text().to_string(),
            "old"
        );
        assert_eq!(
            registry.session(id(DOCUMENT_B)).unwrap().text().to_string(),
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
            )
            .unwrap();
        registry
            .open(
                id(DOCUMENT_B),
                location("D:/work/b/alias.txt", Some(41)),
                Rope::from_str("ignored"),
                state(),
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
                )
                .unwrap();
        }

        registry.retain(|document| document == id(DOCUMENT_A));

        assert_eq!(registry.len(), 1);
        assert!(registry.session(id(DOCUMENT_A)).is_some());
        assert_eq!(registry.existing(&location("D:/work/b.txt", None)), None);
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
            )
            .unwrap();

        let result = registry.open(
            id(DOCUMENT_A),
            location("D:/work/b.txt", Some(2)),
            Rope::from_str("b"),
            state(),
        );

        assert_eq!(
            result,
            Err(DocumentRegistryError::DocumentIdConflict(id(DOCUMENT_A)))
        );
        assert_eq!(registry.len(), 1);
        assert_eq!(
            registry.session(id(DOCUMENT_A)).unwrap().text().to_string(),
            "a"
        );
    }
}
