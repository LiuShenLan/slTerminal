//! 应用级 Document 自动保存调度；不持有正文或第二份编辑状态。

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use slterm_document::{DocumentId, DocumentRegistry};
use slterm_workspace::{ViewKey, ViewKeyKind};

use super::config::Settings;

const IDLE_DELAY: Duration = Duration::from_secs(1);

#[derive(Default)]
struct DocumentActivity {
    last_edit: Option<Instant>,
    focused_views: Vec<ViewKey>,
    failed_revision: Option<u64>,
}

/// 全局开关与所有编辑视图共享的 Document 调度器。
#[derive(Default)]
pub(crate) struct DocumentAutosaveCoordinator {
    enabled: bool,
    activities: BTreeMap<DocumentId, DocumentActivity>,
}

impl DocumentAutosaveCoordinator {
    pub(crate) fn from_settings(settings: &Settings) -> Self {
        Self {
            enabled: settings.document_autosave,
            activities: BTreeMap::new(),
        }
    }

    pub(crate) fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub(crate) fn on_edit(&mut self, id: DocumentId, at: Instant) {
        let activity = self.activities.entry(id).or_default();
        activity.last_edit = Some(at);
        activity.failed_revision = None;
    }

    pub(crate) fn on_focus(&mut self, id: DocumentId, view: ViewKey) {
        if !is_editing_view(&view) {
            return;
        }
        let activity = self.activities.entry(id).or_default();
        if !activity.focused_views.contains(&view) {
            activity.focused_views.push(view);
        }
    }

    /// 只有实际移除一个焦点引用且全部编辑视图均失焦才立即触发。
    pub(crate) fn on_blur(
        &mut self,
        id: DocumentId,
        view: ViewKey,
        registry: &DocumentRegistry,
    ) -> Vec<DocumentId> {
        if !is_editing_view(&view) {
            return Vec::new();
        }
        let all_blurred = {
            let activity = self.activities.entry(id).or_default();
            let previous = activity.focused_views.len();
            activity.focused_views.retain(|focused| focused != &view);
            previous != activity.focused_views.len() && activity.focused_views.is_empty()
        };
        if all_blurred && self.eligible(registry, id) {
            vec![id]
        } else {
            Vec::new()
        }
    }

    pub(crate) fn tick(&self, registry: &DocumentRegistry, now: Instant) -> Vec<DocumentId> {
        self.activities
            .iter()
            .filter(|(_, activity)| {
                activity
                    .last_edit
                    .is_some_and(|last| now.duration_since(last) >= IDLE_DELAY)
            })
            .map(|(id, _)| *id)
            .filter(|id| self.eligible(registry, *id))
            .collect()
    }

    pub(crate) fn on_last_reference(
        &self,
        id: DocumentId,
        registry: &DocumentRegistry,
    ) -> Vec<DocumentId> {
        if self.eligible(registry, id) {
            vec![id]
        } else {
            Vec::new()
        }
    }

    pub(crate) fn on_exit(&self, registry: &DocumentRegistry) -> Vec<DocumentId> {
        self.activities
            .keys()
            .copied()
            .filter(|id| self.eligible(registry, *id))
            .collect()
    }

    /// 旧保存失败不得压住在其后产生的新 revision。
    pub(crate) fn save_failed(
        &mut self,
        registry: &DocumentRegistry,
        id: DocumentId,
        attempted_revision: u64,
    ) {
        if registry
            .session(id)
            .is_some_and(|session| session.text_revision() == attempted_revision)
        {
            self.activities.entry(id).or_default().failed_revision = Some(attempted_revision);
        }
    }

    fn eligible(&self, registry: &DocumentRegistry, id: DocumentId) -> bool {
        if !self.enabled {
            return false;
        }
        let Some(activity) = self.activities.get(&id) else {
            return false;
        };
        if activity.failed_revision.is_some() {
            return false;
        }
        registry.session(id).is_some_and(|session| {
            session.is_dirty()
                && !session.is_read_only()
                && !session.is_saving()
                && !session.is_read_revoked()
                && session.external_conflict().is_none()
        })
    }
}

fn is_editing_view(view: &ViewKey) -> bool {
    matches!(
        view.kind(),
        ViewKeyKind::Editor | ViewKeyKind::Markdown | ViewKeyKind::Diff
    )
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;
    use std::time::{Duration, Instant};

    use ropey::Rope;
    use slterm_document::{
        DocumentId, DocumentMetadata, DocumentRegistry, EditOrigin, EditTransaction, MergePolicy,
        TextEdit, TextStateId,
    };
    use slterm_path::{CanonicalPath, ResolvedPath};
    use slterm_workspace::{ProjectId, ViewKey};

    use super::DocumentAutosaveCoordinator;
    use crate::app_shell::config::Settings;

    const DOCUMENT: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2a";
    const PROJECT: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b";
    const STATE_A: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0f2a";
    const STATE_B: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0f2b";
    const STATE_C: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0f2c";

    fn dirty_document() -> (DocumentRegistry, DocumentId) {
        let id = DocumentId::from_str(DOCUMENT).unwrap();
        let mut registry = DocumentRegistry::new();
        registry
            .open(
                id,
                ResolvedPath::new(CanonicalPath::new("D:/work/file.txt").unwrap(), None),
                Rope::from_str("old"),
                TextStateId::from_str(STATE_A).unwrap(),
                std::sync::Arc::from(b"old".as_slice()),
                DocumentMetadata::default(),
            )
            .unwrap();
        edit(&mut registry, id, "new", STATE_B);
        (registry, id)
    }

    fn edit(registry: &mut DocumentRegistry, id: DocumentId, text: &str, state: &str) {
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
                TextStateId::from_str(state).unwrap(),
                Instant::now(),
            )
            .unwrap();
    }

    fn view(kind: slterm_workspace::ViewKeyKind, document: DocumentId) -> ViewKey {
        let project_id = ProjectId::from_str(PROJECT).unwrap();
        match kind {
            slterm_workspace::ViewKeyKind::Editor => ViewKey::Editor {
                project_id,
                document_id: document,
            },
            slterm_workspace::ViewKeyKind::Markdown => ViewKey::Markdown {
                project_id,
                document_id: document,
            },
            slterm_workspace::ViewKeyKind::Diff => ViewKey::Diff {
                project_id,
                document_id: document,
            },
            _ => panic!("测试只需要编辑视图"),
        }
    }

    #[test]
    fn document_autosave_waits_for_all_views_to_blur() {
        let (registry, id) = dirty_document();
        let mut autosave = DocumentAutosaveCoordinator::from_settings(&Settings {
            document_autosave: true,
            ..Settings::default()
        });
        let editor = view(slterm_workspace::ViewKeyKind::Editor, id);
        let markdown = view(slterm_workspace::ViewKeyKind::Markdown, id);
        autosave.on_edit(id, Instant::now());
        autosave.on_focus(id, editor.clone());
        autosave.on_focus(id, markdown.clone());

        assert!(autosave.on_blur(id, editor, &registry).is_empty());
        assert_eq!(autosave.on_blur(id, markdown.clone(), &registry), vec![id]);
        assert!(
            autosave.on_blur(id, markdown, &registry).is_empty(),
            "同一视图的重复失焦不得重复触发"
        );
    }

    #[test]
    fn settings_document_autosave_is_global() {
        let (registry, id) = dirty_document();
        let mut autosave = DocumentAutosaveCoordinator::from_settings(&Settings {
            document_autosave: true,
            ..Settings::default()
        });
        autosave.on_edit(id, Instant::now());
        for kind in [
            slterm_workspace::ViewKeyKind::Editor,
            slterm_workspace::ViewKeyKind::Markdown,
            slterm_workspace::ViewKeyKind::Diff,
        ] {
            autosave.on_focus(id, view(kind, id));
        }
        for kind in [
            slterm_workspace::ViewKeyKind::Editor,
            slterm_workspace::ViewKeyKind::Markdown,
        ] {
            assert!(autosave.on_blur(id, view(kind, id), &registry).is_empty());
        }
        assert_eq!(
            autosave.on_blur(id, view(slterm_workspace::ViewKeyKind::Diff, id), &registry),
            vec![id],
            "三类编辑视图必须共享同一 Document 触发器"
        );
    }

    #[test]
    fn idle_save_and_failure_retry_follow_document_revision() {
        let (mut registry, id) = dirty_document();
        let start = Instant::now();
        let mut autosave = DocumentAutosaveCoordinator::from_settings(&Settings {
            document_autosave: true,
            ..Settings::default()
        });
        autosave.on_edit(id, start);
        assert!(
            autosave
                .tick(&registry, start + Duration::from_millis(999))
                .is_empty()
        );
        assert_eq!(
            autosave.tick(&registry, start + Duration::from_secs(1)),
            vec![id]
        );

        let request = registry.begin_save(id, 0).unwrap();
        edit(&mut registry, id, "newer", STATE_C);
        autosave.on_edit(id, start + Duration::from_secs(2));
        assert!(
            autosave
                .tick(&registry, start + Duration::from_secs(3))
                .is_empty()
        );
        registry.fail_save(id, request.save_generation()).unwrap();
        autosave.save_failed(&registry, id, request.text_revision());
        assert_eq!(
            autosave.tick(&registry, start + Duration::from_secs(4)),
            vec![id],
            "旧保存失败不得抑制保存期间产生的新编辑"
        );
    }

    #[test]
    fn conflict_and_read_revoke_pause_every_automatic_trigger() {
        for read_revoked in [false, true] {
            let (mut registry, id) = dirty_document();
            let start = Instant::now();
            let mut autosave = DocumentAutosaveCoordinator::from_settings(&Settings {
                document_autosave: true,
                ..Settings::default()
            });
            autosave.on_edit(id, start);
            let request = registry.external_request(id, 0).unwrap();
            let change = if read_revoked {
                let location = request.location().clone();
                slterm_document::DocumentExternalChange::ReadRevoked { request, location }
            } else {
                slterm_document::DocumentExternalChange::Deleted(request)
            };
            registry.apply_external(change, start).unwrap();

            assert!(
                autosave
                    .tick(&registry, start + Duration::from_secs(1))
                    .is_empty()
            );
            assert!(autosave.on_last_reference(id, &registry).is_empty());
            assert!(autosave.on_exit(&registry).is_empty());
        }
    }

    #[test]
    fn last_reference_and_exit_share_global_pause_rules() {
        let (registry, id) = dirty_document();
        let mut autosave = DocumentAutosaveCoordinator::from_settings(&Settings::default());
        autosave.on_edit(id, Instant::now());
        assert!(autosave.on_last_reference(id, &registry).is_empty());
        assert!(autosave.on_exit(&registry).is_empty());

        autosave.set_enabled(true);
        assert_eq!(autosave.on_last_reference(id, &registry), vec![id]);
        assert_eq!(autosave.on_exit(&registry), vec![id]);
    }
}
