//! 统一关闭主干中的 Document 批量保存执行体。

use std::collections::BTreeMap;

use slterm_document::{DocumentId, DocumentRegistry, SaveTransactionError};
use slterm_path::PathGrant;
use slterm_workspace::{CloseDecision, CloseIntent, CloseTarget};

use super::documents::DocumentSaveService;
use crate::text_document::SaveError;

#[allow(dead_code)] // 结构化错误值由失败断言与后续聚合提示读取。
#[derive(Debug)]
pub(crate) enum DocumentSaveFailureKind {
    RevisionMismatch,
    Save(SaveError),
    State(SaveTransactionError),
}

#[allow(dead_code)] // 失败身份与种类由测试及后续用户提示读取。
#[derive(Debug)]
pub(crate) struct DocumentSaveFailure {
    document_id: DocumentId,
    pub(crate) kind: DocumentSaveFailureKind,
}

#[allow(dead_code)] // 访问器由聚合失败测试与后续提示使用。
impl DocumentSaveFailure {
    pub(crate) const fn document_id(&self) -> DocumentId {
        self.document_id
    }
}

/// 逐项保存并聚合失败；已成功的 Document 不回滚。只允许在后台执行体调用。
pub(crate) fn execute_document_save_stage(
    registry: &mut DocumentRegistry,
    intent: &CloseIntent<CloseTarget>,
    grants: &BTreeMap<DocumentId, Vec<PathGrant>>,
    current_authorization_generation: u64,
) -> Result<(), Vec<DocumentSaveFailure>> {
    let service = DocumentSaveService::default();
    let mut failures = Vec::new();
    for object in intent.objects() {
        let CloseTarget::Document(id) = object.id else {
            continue;
        };
        if object.decision != CloseDecision::Save {
            continue;
        }
        if registry
            .session(id)
            .is_none_or(|session| session.text_revision() != object.generation)
        {
            failures.push(DocumentSaveFailure {
                document_id: id,
                kind: DocumentSaveFailureKind::RevisionMismatch,
            });
            continue;
        }
        let Some(grants) = grants.get(&id) else {
            failures.push(DocumentSaveFailure {
                document_id: id,
                kind: DocumentSaveFailureKind::Save(SaveError::Unauthorized),
            });
            continue;
        };
        let request = match registry.begin_save(id, current_authorization_generation) {
            Ok(request) => request,
            Err(error) => {
                failures.push(DocumentSaveFailure {
                    document_id: id,
                    kind: DocumentSaveFailureKind::State(error),
                });
                continue;
            }
        };
        let generation = request.save_generation();
        match service.save(request, grants, current_authorization_generation) {
            Ok(receipt) => {
                if let Err(error) = registry.complete_save(receipt) {
                    let _ = registry.fail_save(id, generation);
                    failures.push(DocumentSaveFailure {
                        document_id: id,
                        kind: DocumentSaveFailureKind::State(error),
                    });
                }
            }
            Err(error) => {
                let _ = registry.fail_save(id, generation);
                failures.push(DocumentSaveFailure {
                    document_id: id,
                    kind: DocumentSaveFailureKind::Save(error),
                });
            }
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::time::Instant;

    use slterm_document::{
        DocumentId, DocumentRegistry, EditOrigin, EditTransaction, MergePolicy, TextEdit,
        TextStateId,
    };
    use slterm_path::{CanonicalPath, PathGrant};
    use slterm_workspace::{
        CloseCoordinator, CloseDecision, CloseEffect, CloseIntent, CloseObject, CloseOutcome,
        CloseScope, CloseTarget,
    };
    use uuid::Uuid;

    use super::{DocumentSaveFailureKind, execute_document_save_stage};
    use crate::app_shell::documents::DocumentOpenService;
    use crate::text_document::SaveError;

    fn edit(registry: &mut DocumentRegistry, id: DocumentId, text: &str) {
        let session = registry.session(id).unwrap();
        let revision = session.text_revision();
        let end = session.text().len();
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
    fn workspace_batch_save_failure_cancels_close() {
        let temp = tempfile::tempdir().unwrap();
        let root = CanonicalPath::new(dunce::canonicalize(temp.path()).unwrap()).unwrap();
        let first = temp.path().join("first.txt");
        let second = temp.path().join("second.txt");
        std::fs::write(&first, "first").unwrap();
        std::fs::write(&second, "second").unwrap();
        let opener = DocumentOpenService::default();
        let mut registry = DocumentRegistry::new();
        let first_open = opener
            .open(&mut registry, &root, &CanonicalPath::new(&first).unwrap())
            .unwrap();
        let second_open = opener
            .open(&mut registry, &root, &CanonicalPath::new(&second).unwrap())
            .unwrap();
        edit(&mut registry, first_open.id, "first saved");
        edit(&mut registry, second_open.id, "second local");
        // 第二项真实外部修改必须在条件保存时失败，而不是被批量覆盖。
        std::fs::write(&second, "second external").unwrap();
        let objects = [first_open.id, second_open.id]
            .into_iter()
            .map(|id| CloseObject {
                id: CloseTarget::Document(id),
                generation: registry.session(id).unwrap().text_revision(),
                decision: CloseDecision::Save,
            })
            .collect::<Vec<_>>();
        let intent = CloseIntent::new(objects, [3; 32]).unwrap();
        let mut coordinator = CloseCoordinator::new();
        let attempt = coordinator
            .begin(CloseScope::ApplicationExit)
            .unwrap()
            .attempt()
            .unwrap();
        let CloseEffect::Save { attempt, .. } =
            coordinator.collected(attempt, intent.clone()).unwrap()
        else {
            panic!("保存决定必须进入 Save 阶段");
        };
        let grants = BTreeMap::from([
            (first_open.id, vec![PathGrant::project_root(root.clone())]),
            (second_open.id, vec![PathGrant::project_root(root)]),
        ]);

        let failures = execute_document_save_stage(&mut registry, &intent, &grants, 0).unwrap_err();

        assert_eq!(failures.len(), 1);
        assert_eq!(failures[0].document_id(), second_open.id);
        assert_eq!(std::fs::read_to_string(&first).unwrap(), "first saved");
        assert_eq!(std::fs::read_to_string(&second).unwrap(), "second external");
        assert!(!registry.session(first_open.id).unwrap().is_dirty());
        assert!(registry.session(second_open.id).unwrap().is_dirty());
        assert_eq!(
            registry.session(second_open.id).unwrap().text().to_string(),
            "second local"
        );
        coordinator.fail(attempt).unwrap();
        coordinator.cancel().unwrap();
        assert_eq!(coordinator.outcome(), Some(CloseOutcome::Cancelled));
    }

    #[test]
    fn document_batch_close_save_success() {
        let temp = tempfile::tempdir().unwrap();
        let root = CanonicalPath::new(dunce::canonicalize(temp.path()).unwrap()).unwrap();
        let path = temp.path().join("saved.txt");
        std::fs::write(&path, "before").unwrap();
        let mut registry = DocumentRegistry::new();
        let opened = DocumentOpenService::default()
            .open(&mut registry, &root, &CanonicalPath::new(&path).unwrap())
            .unwrap();
        edit(&mut registry, opened.id, "after");
        let intent = CloseIntent::new(
            vec![CloseObject {
                id: CloseTarget::Document(opened.id),
                generation: registry.session(opened.id).unwrap().text_revision(),
                decision: CloseDecision::Save,
            }],
            [4; 32],
        )
        .unwrap();
        let grants = BTreeMap::from([(opened.id, vec![PathGrant::project_root(root)])]);

        execute_document_save_stage(&mut registry, &intent, &grants, 0).unwrap();

        assert_eq!(std::fs::read_to_string(&path).unwrap(), "after");
        assert!(!registry.session(opened.id).unwrap().is_dirty());
    }

    #[test]
    fn document_batch_close_aggregates_failures_and_continues() {
        let temp = tempfile::tempdir().unwrap();
        let root = CanonicalPath::new(dunce::canonicalize(temp.path()).unwrap()).unwrap();
        let opener = DocumentOpenService::default();
        let mut registry = DocumentRegistry::new();
        let mut opened = (0..4)
            .map(|index| {
                let path = temp.path().join(format!("{index}.txt"));
                std::fs::write(&path, format!("disk {index}")).unwrap();
                let document = opener
                    .open(&mut registry, &root, &CanonicalPath::new(&path).unwrap())
                    .unwrap();
                edit(&mut registry, document.id, &format!("local {index}"));
                (document.id, path)
            })
            .collect::<Vec<_>>();
        opened.sort_by_key(|(id, _)| *id);
        // 按 CloseIntent 的 DocumentId 顺序让首项先发生真实磁盘冲突。
        std::fs::write(&opened[0].1, "external").unwrap();
        let objects = opened
            .iter()
            .enumerate()
            .map(|(index, (id, _))| {
                let revision = registry.session(*id).unwrap().text_revision();
                CloseObject {
                    id: CloseTarget::Document(*id),
                    generation: if index == 2 { revision - 1 } else { revision },
                    decision: CloseDecision::Save,
                }
            })
            .collect();
        let intent = CloseIntent::new(objects, [5; 32]).unwrap();
        // 末项缺少写授权；中间项仍必须在前后失败后成功保存。
        let grants = opened[..3]
            .iter()
            .map(|(id, _)| (*id, vec![PathGrant::project_root(root.clone())]))
            .collect();

        let failures = execute_document_save_stage(&mut registry, &intent, &grants, 0).unwrap_err();

        assert_eq!(failures.len(), 3);
        let failure = |id| {
            failures
                .iter()
                .find(|failure| failure.document_id() == id)
                .unwrap()
        };
        assert!(matches!(
            &failure(opened[0].0).kind,
            DocumentSaveFailureKind::Save(_)
        ));
        assert!(matches!(
            &failure(opened[2].0).kind,
            DocumentSaveFailureKind::RevisionMismatch
        ));
        assert!(matches!(
            &failure(opened[3].0).kind,
            DocumentSaveFailureKind::Save(SaveError::Unauthorized)
        ));
        let expected = |position: usize, prefix: &str| {
            format!(
                "{prefix} {}",
                opened[position].1.file_stem().unwrap().to_str().unwrap()
            )
        };
        assert_eq!(std::fs::read_to_string(&opened[0].1).unwrap(), "external");
        assert_eq!(
            std::fs::read_to_string(&opened[1].1).unwrap(),
            expected(1, "local")
        );
        assert_eq!(
            std::fs::read_to_string(&opened[2].1).unwrap(),
            expected(2, "disk")
        );
        assert_eq!(
            std::fs::read_to_string(&opened[3].1).unwrap(),
            expected(3, "disk")
        );
        assert!(registry.session(opened[0].0).unwrap().is_dirty());
        assert!(!registry.session(opened[1].0).unwrap().is_dirty());
        assert!(registry.session(opened[2].0).unwrap().is_dirty());
        assert!(registry.session(opened[3].0).unwrap().is_dirty());
    }
}
