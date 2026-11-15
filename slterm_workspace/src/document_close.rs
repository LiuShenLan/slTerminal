//! 从统一工作区快照收集 Document 关闭对象与内容决定。

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use slterm_document::{DocumentId, DocumentRegistry};

use crate::{CloseDecision, CloseObject, CloseTarget, ProjectSnapshot, ViewKey};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DocumentCloseError {
    DuplicateView,
    ViewNotFound,
    MissingDocument(DocumentId),
    MissingDecision(DocumentId),
    ExternalReference(DocumentId),
    UnexpectedDecision(DocumentId),
}

impl fmt::Display for DocumentCloseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateView => formatter.write_str("关闭集合包含重复视图"),
            Self::ViewNotFound => formatter.write_str("关闭视图不在当前工作区快照中"),
            Self::MissingDocument(_) => formatter.write_str("关闭视图缺少运行 Document"),
            Self::MissingDecision(_) => {
                formatter.write_str("最后引用的脏 Document 缺少保存或丢弃决定")
            }
            Self::ExternalReference(_) => {
                formatter.write_str("关闭集合外仍有引用，禁止丢弃 Document")
            }
            Self::UnexpectedDecision(_) => formatter.write_str("该 Document 不需要内容决定"),
        }
    }
}

impl std::error::Error for DocumentCloseError {}

/// 按 Document 去重收集；返回对象直接进入统一 CloseIntent<CloseTarget>。
pub fn collect_document_close(
    snapshot: &ProjectSnapshot,
    registry: &DocumentRegistry,
    closing: &[ViewKey],
    decisions: &BTreeMap<DocumentId, CloseDecision>,
) -> Result<Vec<CloseObject<CloseTarget>>, DocumentCloseError> {
    let mut unique = Vec::new();
    for view in closing {
        if unique.contains(view) {
            return Err(DocumentCloseError::DuplicateView);
        }
        unique.push(view.clone());
    }
    let all_views = snapshot
        .registry()
        .projects()
        .iter()
        .flat_map(|project| project.layout().view_keys())
        .collect::<Vec<_>>();
    if unique.iter().any(|view| !all_views.contains(view)) {
        return Err(DocumentCloseError::ViewNotFound);
    }
    let closing_documents = unique
        .iter()
        .filter_map(view_document)
        .collect::<BTreeSet<_>>();
    if let Some(id) = decisions.keys().find(|id| !closing_documents.contains(id)) {
        return Err(DocumentCloseError::UnexpectedDecision(*id));
    }

    closing_documents
        .into_iter()
        .map(|id| {
            let session = registry
                .session(id)
                .ok_or(DocumentCloseError::MissingDocument(id))?;
            let has_external_reference = all_views
                .iter()
                .any(|view| view_document(view) == Some(id) && !unique.contains(view));
            let supplied = decisions.get(&id).copied();
            let decision = if has_external_reference {
                match supplied {
                    None | Some(CloseDecision::NotRequired) => CloseDecision::NotRequired,
                    Some(CloseDecision::Discard) => {
                        return Err(DocumentCloseError::ExternalReference(id));
                    }
                    Some(CloseDecision::Save) => {
                        return Err(DocumentCloseError::UnexpectedDecision(id));
                    }
                }
            } else if session.is_dirty() {
                match supplied {
                    Some(CloseDecision::Save | CloseDecision::Discard) => supplied.unwrap(),
                    _ => return Err(DocumentCloseError::MissingDecision(id)),
                }
            } else {
                match supplied {
                    None | Some(CloseDecision::NotRequired) => CloseDecision::NotRequired,
                    _ => return Err(DocumentCloseError::UnexpectedDecision(id)),
                }
            };
            Ok(CloseObject {
                id: CloseTarget::Document(id),
                generation: session.text_revision(),
                decision,
            })
        })
        .collect()
}

fn view_document(view: &ViewKey) -> Option<DocumentId> {
    match view {
        ViewKey::Editor { document_id, .. }
        | ViewKey::Markdown { document_id, .. }
        | ViewKey::Diff { document_id, .. } => Some(*document_id),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::str::FromStr;
    use std::time::Instant;

    use crate::{
        CloseCoordinator, CloseDecision, CloseEffect, CloseIntent, CloseScope, PanelGroup,
        PanelGroupId, Project, ProjectId, ProjectLayout, ProjectRegistry, ProjectRootStatus,
        ProjectSnapshot, ViewKey,
    };
    use ropey::Rope;
    use slterm_document::{
        DocumentId, DocumentMetadata, DocumentRegistry, EditOrigin, EditTransaction, MergePolicy,
        TextEdit, TextStateId,
    };
    use slterm_path::{CanonicalPath, ResolvedPath};

    use super::{CloseTarget, collect_document_close};

    const DOCUMENT: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2a";
    const PROJECT_A: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b";
    const PROJECT_B: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2c";
    const GROUP_A: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2d";
    const GROUP_B: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2e";
    const STATE_A: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0f2a";
    const STATE_B: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0f2b";

    fn ids() -> (DocumentId, ProjectId, ProjectId) {
        (
            DocumentId::from_str(DOCUMENT).unwrap(),
            ProjectId::from_str(PROJECT_A).unwrap(),
            ProjectId::from_str(PROJECT_B).unwrap(),
        )
    }

    fn views() -> (ViewKey, ViewKey) {
        let (document, first, second) = ids();
        (
            ViewKey::Editor {
                project_id: first,
                document_id: document,
            },
            ViewKey::Markdown {
                project_id: second,
                document_id: document,
            },
        )
    }

    fn snapshot() -> ProjectSnapshot {
        let (document, first, second) = ids();
        let (editor, markdown) = views();
        let location = ResolvedPath::new(CanonicalPath::new("D:/work/shared.txt").unwrap(), None);
        let projects = [(first, GROUP_A, editor), (second, GROUP_B, markdown)]
            .into_iter()
            .map(|(project, group, view)| {
                Project::restore(
                    project,
                    CanonicalPath::new(format!("D:/work/{project}")).unwrap(),
                    project.to_string(),
                    ProjectLayout::new(
                        project,
                        PanelGroupId::from_str(group).unwrap(),
                        PanelGroup::new(vec![view.clone()], view).unwrap(),
                    )
                    .unwrap(),
                    ProjectRootStatus::Ready,
                    0,
                )
                .unwrap()
            })
            .collect();
        ProjectSnapshot::restore_with_documents(
            ProjectRegistry::restore(projects, Some(first)).unwrap(),
            Vec::new(),
            vec![(document, location)],
        )
        .unwrap()
    }

    fn documents(dirty: bool) -> DocumentRegistry {
        let (id, _, _) = ids();
        let mut registry = DocumentRegistry::new();
        registry
            .open(
                id,
                ResolvedPath::new(CanonicalPath::new("D:/work/shared.txt").unwrap(), None),
                Rope::from_str("old"),
                TextStateId::from_str(STATE_A).unwrap(),
                std::sync::Arc::from(b"old".as_slice()),
                DocumentMetadata::default(),
            )
            .unwrap();
        if dirty {
            registry
                .apply(
                    EditTransaction {
                        document_id: id,
                        expected_text_revision: 0,
                        edits: vec![TextEdit::new(0..3, "new")],
                        origin: EditOrigin::ReplaceAll,
                        merge_policy: MergePolicy::Disallowed,
                    },
                    TextStateId::from_str(STATE_B).unwrap(),
                    Instant::now(),
                )
                .unwrap();
        }
        registry
    }

    #[test]
    fn document_last_reference_close_guard() {
        let snapshot = snapshot();
        let (editor, markdown) = views();
        let registry = documents(true);
        let (id, _, _) = ids();

        let partial = collect_document_close(
            &snapshot,
            &registry,
            std::slice::from_ref(&editor),
            &BTreeMap::new(),
        )
        .unwrap();
        assert_eq!(partial.len(), 1);
        assert_eq!(partial[0].id, CloseTarget::Document(id));
        assert_eq!(partial[0].decision, CloseDecision::NotRequired);

        let all = collect_document_close(
            &snapshot,
            &registry,
            &[editor, markdown],
            &BTreeMap::from([(id, CloseDecision::Save)]),
        )
        .unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].decision, CloseDecision::Save);

        let clean = documents(false);
        let (editor, markdown) = views();
        let all = collect_document_close(&snapshot, &clean, &[editor, markdown], &BTreeMap::new())
            .unwrap();
        assert_eq!(all[0].decision, CloseDecision::NotRequired);
    }

    #[test]
    fn document_discard_respects_external_reference() {
        let snapshot = snapshot();
        let registry = documents(true);
        let (id, _, _) = ids();
        let (editor, _) = views();

        assert!(
            collect_document_close(
                &snapshot,
                &registry,
                &[editor],
                &BTreeMap::from([(id, CloseDecision::Discard)]),
            )
            .is_err()
        );
        assert_eq!(registry.session(id).unwrap().text().to_string(), "new");
        assert!(registry.session(id).unwrap().is_dirty());
    }

    #[test]
    fn document_close_uses_shared_save_stage() {
        let snapshot = snapshot();
        let registry = documents(true);
        let (id, _, _) = ids();
        let (editor, markdown) = views();
        let objects = collect_document_close(
            &snapshot,
            &registry,
            &[editor, markdown],
            &BTreeMap::from([(id, CloseDecision::Save)]),
        )
        .unwrap();
        let intent = CloseIntent::new(objects, [7; 32]).unwrap();
        let mut coordinator = CloseCoordinator::new();
        let attempt = coordinator
            .begin(CloseScope::ApplicationExit)
            .unwrap()
            .attempt()
            .unwrap();

        let effect = coordinator.collected(attempt, intent).unwrap();

        assert!(matches!(effect, CloseEffect::Save { .. }));
    }

    #[test]
    fn dirty_last_reference_requires_save_or_discard() {
        let snapshot = snapshot();
        let registry = documents(true);
        let (editor, markdown) = views();
        let closing = vec![editor, markdown];

        assert!(collect_document_close(&snapshot, &registry, &closing, &BTreeMap::new(),).is_err());
    }
}
