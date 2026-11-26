//! Editor Git 行标：固定 HEAD 与共享 Document 的后台 core 差分。

use gpui::{Context, Entity};
use slterm_document::{
    DiffBinding, DiffInput, DiffSnapshot, DocumentId, TextStateId, compute_diff,
};
use slterm_workspace::{PanelId, ProjectId, ViewKey};

use super::diff::{DiffOpenRequest, diff_change_marks};
use super::panel_host::PanelContent;
use super::task_service::{CancellationToken, TaskId};
use super::{AppRoot, commands::AppRootError};
use crate::app_shell::text_view::DocumentTextView;
use crate::git::GitShowSnapshot;

#[derive(Default)]
pub(super) struct EditorGitState {
    binding: Option<DiffBinding>,
    head: Option<GitShowSnapshot>,
    generation: u64,
    pending: bool,
}

struct EditorGitComputation {
    generation: u64,
    text_state: TextStateId,
    snapshot: Option<(DiffBinding, GitShowSnapshot, DiffSnapshot)>,
}

impl AppRoot {
    fn editor_panel(
        &self,
        panel_id: PanelId,
        cx: &gpui::App,
    ) -> Option<(ProjectId, DocumentId, Entity<DocumentTextView>)> {
        self.projects.values().find_map(|project| {
            project.read(cx).panels.get(&panel_id).and_then(|panel| {
                match (&panel.view_key, &panel.content) {
                    (
                        ViewKey::Editor {
                            project_id,
                            document_id,
                        },
                        PanelContent::Editor(view),
                    ) => Some((*project_id, *document_id, view.view.clone())),
                    _ => None,
                }
            })
        })
    }

    /// 在途期间的新编辑只保留最新代次；初始 HEAD 与后续差分共用一个任务身份。
    pub(super) fn schedule_editor_git(
        &mut self,
        panel_id: PanelId,
        refresh_head: bool,
        cx: &mut Context<'_, Self>,
    ) {
        let Some((project_id, document_id, editor)) = self.editor_panel(panel_id, cx) else {
            return;
        };
        let snapshot = editor.read(cx).text_snapshot();
        let text_state = snapshot.text_state;
        let generation = {
            let state = self.editor_git.entry(panel_id).or_default();
            state.generation = state.generation.wrapping_add(1);
            state.pending = true;
            state.generation
        };
        let task_id = TaskId::EditorGit(panel_id);
        if self.tasks.is_registered(task_id) {
            return;
        }
        let (binding, head) = {
            let state = self.editor_git.get_mut(&panel_id).expect("行标状态刚创建");
            state.pending = false;
            if refresh_head {
                (None, None)
            } else {
                (state.binding.clone(), state.head.clone())
            }
        };
        let refresh_head = refresh_head || head.is_none();
        let Some(project_root) = self
            .snapshot
            .registry()
            .project(project_id)
            .map(|project| project.root().clone())
        else {
            return;
        };
        let Some(document_path) = self
            .documents
            .session(document_id)
            .map(|session| session.location().canonical_path().clone())
        else {
            return;
        };
        let grants = self.document_read_grants(document_id);
        let git = self.git.clone();
        let executor = cx.background_executor().clone();
        let load = executor.spawn(async move {
            let snapshot = (|| {
                let (binding, head) = if refresh_head {
                    let binding = git
                        .diff_binding_for_path(&project_root, &document_path, &grants, 0)
                        .ok()?;
                    let head = git
                        .show(
                            binding.repository_binding(),
                            binding.head_path(),
                            &grants,
                            0,
                            generation,
                        )
                        .ok()?;
                    (binding, head)
                } else {
                    (binding.clone()?, head.clone()?)
                };
                let head_bytes = head
                    .bytes
                    .strip_prefix(&[0xef, 0xbb, 0xbf])
                    .map_or_else(|| head.bytes.clone(), std::sync::Arc::from);
                let input = DiffInput::git(
                    head_bytes,
                    std::sync::Arc::from(snapshot.text.to_string().into_bytes()),
                    slterm_document::DiffWhitespaceMode::Exact,
                    snapshot.text_state,
                    head.commit.clone(),
                    generation,
                );
                let diff = compute_diff(&input).ok()?;
                Some((binding, head, diff))
            })();
            EditorGitComputation {
                generation,
                text_state,
                snapshot,
            }
        });
        let root = cx.entity().downgrade();
        let cancellation = CancellationToken::new();
        let cancelled = cancellation.clone();
        let registration = self.tasks.register_with(task_id, cancellation, || {
            cx.spawn(async move |_, cx| {
                let result = load.await;
                if cancelled.is_cancelled() {
                    return;
                }
                let _ = root.update(cx, |root, cx| {
                    root.finish_editor_git(panel_id, result, cx);
                });
            })
        });
        if registration.is_err() {
            log::debug!("editor git diff already scheduled");
        }
    }

    fn finish_editor_git(
        &mut self,
        panel_id: PanelId,
        result: EditorGitComputation,
        cx: &mut Context<'_, Self>,
    ) {
        self.tasks.complete(TaskId::EditorGit(panel_id));
        let Some((_, _, editor)) = self.editor_panel(panel_id, cx) else {
            self.editor_git.remove(&panel_id);
            return;
        };
        let current = self
            .editor_git
            .get(&panel_id)
            .is_some_and(|state| state.generation == result.generation);
        if current && editor.read(cx).text_snapshot().text_state == result.text_state {
            let marks = result
                .snapshot
                .as_ref()
                .map_or_else(Vec::new, |(_, _, diff)| {
                    diff_change_marks(diff)
                        .into_iter()
                        .map(|(line, kind)| crate::app_shell::text_view::GitMark { line, kind })
                        .collect()
                });
            editor.update(cx, |view, cx| view.set_git_marks(marks, cx));
        }
        let pending = {
            let state = self
                .editor_git
                .get_mut(&panel_id)
                .expect("行标状态必须随面板存活");
            if let Some((binding, head, _)) = result.snapshot {
                state.binding = Some(binding);
                state.head = Some(head);
            }
            state.pending
        };
        if pending {
            self.schedule_editor_git(panel_id, false, cx);
        }
    }

    pub(super) fn refresh_editor_git_for_document(
        &mut self,
        document_id: DocumentId,
        cx: &mut Context<'_, Self>,
    ) {
        let target = cx.entity().downgrade();
        cx.defer(move |cx| {
            let _ = target.update(cx, |root, cx| {
                let panels = root
                    .projects
                    .values()
                    .flat_map(|project| {
                        project
                            .read(cx)
                            .panels
                            .iter()
                            .filter_map(|(panel_id, panel)| {
                                (matches!(
                                    &panel.view_key,
                                    ViewKey::Editor { document_id: id, .. } if *id == document_id
                                ) && matches!(&panel.content, PanelContent::Editor(_)))
                                .then_some(*panel_id)
                            })
                            .collect::<Vec<_>>()
                    })
                    .collect::<Vec<_>>();
                for panel_id in panels {
                    root.schedule_editor_git(panel_id, false, cx);
                }
            });
        });
    }

    pub(super) fn open_editor_git_diff(
        &mut self,
        project_id: ProjectId,
        document_id: DocumentId,
        line: usize,
        window: &mut gpui::Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<PanelId, AppRootError> {
        let panel_id = self.panel_for_view(
            project_id,
            &ViewKey::Editor {
                project_id,
                document_id,
            },
            cx,
        )?;
        let binding = self
            .editor_git
            .get(&panel_id)
            .and_then(|state| state.binding.clone())
            .ok_or(AppRootError::MissingPanel)?;
        let target_group = self.focused_group(project_id, cx)?;
        self.open_diff(
            DiffOpenRequest {
                project_id,
                target_group: Some(target_group.ok_or(AppRootError::MissingPanel)?),
                document_id,
                binding: binding.repository_binding().clone(),
                path: binding.head_path().clone(),
                focus: true,
                target_line: Some(line),
            },
            window,
            cx,
        )
    }

    pub(super) fn remove_editor_git(&mut self, panel_id: PanelId) {
        self.tasks.cancel(TaskId::EditorGit(panel_id));
        self.editor_git.remove(&panel_id);
    }
}
