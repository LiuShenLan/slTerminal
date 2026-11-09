//! AppRoot 的 Document 打开队列与编辑器面板原子提交。

#![allow(dead_code)] // 消费点 = 后续 Explorer 与真实 OpenFile 移交入口。

use std::collections::BTreeSet;
use std::io;

use gpui::{App, AppContext as _, Context, Window};
use slterm_document::DocumentOpenOutcome;
use slterm_path::CanonicalPath;
use slterm_workspace::{PanelGroupId, PanelId, ProjectId, ProjectRegistryError, ViewKey};

use super::AppRoot;
use super::commands::AppRootError;
use super::panel_host::{PanelContent, PanelHost};
use super::task_service::{CancellationToken, TaskId};
use crate::app_shell::documents::OpenedDocument;
use crate::app_shell::file_editor::TextFileView;

pub(super) struct DocumentOpenRequest {
    project_id: ProjectId,
    target_group: PanelGroupId,
    project_root: CanonicalPath,
    path: CanonicalPath,
}

impl AppRoot {
    /// 真实读盘串行移交后台；完成前不改变布局或面板。
    pub(crate) fn open_document(
        &mut self,
        project_id: ProjectId,
        target_group: PanelGroupId,
        path: CanonicalPath,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        self.ensure_not_closing()?;
        self.ensure_project_snapshot_command_allowed()?;
        self.ensure_project_snapshot_recovery_allowed()?;
        if self.snapshot.registry().active() != Some(project_id) {
            return Err(AppRootError::ProjectNotActive);
        }
        let project_root = self
            .snapshot
            .registry()
            .project(project_id)
            .ok_or(ProjectRegistryError::ProjectNotFound)?
            .root()
            .clone();
        let markdown = crate::app_shell::doc_tabs::markdown_viewable(path.as_path());
        if !markdown && !crate::app_shell::code_tab::viewable_file(path.as_path()) {
            return Err(AppRootError::DocumentOpen(io::Error::new(
                io::ErrorKind::InvalidInput,
                "文件类型不支持应用内文本编辑",
            )));
        }
        self.pending_document_opens.push_back(DocumentOpenRequest {
            project_id,
            target_group,
            project_root,
            path,
        });
        self.start_next_document_open(window, cx);
        Ok(())
    }

    fn start_next_document_open(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        if self.close_in_progress || self.application_close.is_some() || self.close_completed {
            self.pending_document_opens.clear();
            return;
        }
        if self.document_open_active || self.tasks.is_registered(TaskId::DocumentOpen) {
            return;
        }
        let Some(request) = self.pending_document_opens.pop_front() else {
            return;
        };
        let mut documents = std::mem::take(&mut self.documents);
        let service = self.document_open_service.clone();
        let executor = cx.background_executor().clone();
        let window_handle = window.window_handle();
        self.document_open_active = true;
        self.tasks
            .register_with(TaskId::DocumentOpen, CancellationToken::new(), move || {
                let read = executor.spawn(async move {
                    let result = service.open(&mut documents, &request.project_root, &request.path);
                    (documents, request, result)
                });
                cx.spawn(async move |this, cx| {
                    let (documents, request, result) = read.await;
                    let _ = window_handle.update(cx, |_, window, cx| {
                        let _ = this.update(cx, |root, cx| {
                            root.finish_document_open(documents, request, result, window, cx);
                        });
                    });
                })
            })
            .expect("DocumentOpen 单任务登记必须唯一");
    }

    fn finish_document_open(
        &mut self,
        documents: slterm_document::DocumentRegistry,
        request: DocumentOpenRequest,
        result: io::Result<OpenedDocument>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.tasks.complete(TaskId::DocumentOpen);
        self.document_open_active = false;
        self.documents = documents;
        if self.close_in_progress || self.application_close.is_some() || self.close_completed {
            if let Ok(opened) = &result
                && opened.outcome == DocumentOpenOutcome::Opened
            {
                self.documents.remove(opened.id);
            }
            self.pending_document_opens.clear();
            self.reconcile_document_registry();
            cx.notify();
            return;
        }
        let outcome = match result {
            Ok(opened) => {
                let id = opened.id;
                let is_new = opened.outcome == DocumentOpenOutcome::Opened;
                let installed = self.install_opened_document(request, opened, window, cx);
                if installed.is_err() && is_new {
                    self.documents.remove(id);
                }
                installed
            }
            Err(error) => Err(AppRootError::DocumentOpen(error)),
        };
        // 在途期间的项目关闭提交也必须与返回后的聚合表对账。
        self.reconcile_document_registry();
        if let Err(error) = outcome {
            log::warn!("Document 打开未提交: {error}");
        }
        self.start_next_document_open(window, cx);
        cx.notify();
    }

    fn install_opened_document(
        &mut self,
        request: DocumentOpenRequest,
        opened: OpenedDocument,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        self.ensure_not_closing()?;
        self.ensure_project_snapshot_command_allowed()?;
        self.ensure_project_snapshot_recovery_allowed()?;
        let project_id = request.project_id;
        if self.snapshot.registry().active() != Some(project_id) {
            return Err(AppRootError::ProjectNotActive);
        }
        let project = self
            .snapshot
            .registry()
            .project(project_id)
            .ok_or(ProjectRegistryError::ProjectNotFound)?;
        if project.root() != &request.project_root {
            return Err(AppRootError::DocumentOpen(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "项目根在打开期间已变化",
            )));
        }
        let location = self
            .documents
            .session(opened.id)
            .ok_or(AppRootError::MissingPanel)?
            .location()
            .clone();
        let (candidate, view) = self.snapshot.with_opened_document(
            project_id,
            request.target_group,
            opened.id,
            location,
        )?;

        if let Ok(panel_id) = self.panel_for_view(project_id, &view, cx) {
            let token = self
                .panel_focus
                .get(&panel_id)
                .map(super::focus_manager::FocusRegistration::token)
                .ok_or(AppRootError::MissingPanel)?;
            self.focus_manager.request_focus(token, window, cx)?;
            self.projects
                .get(&project_id)
                .cloned()
                .ok_or(ProjectRegistryError::ProjectNotFound)?
                .update(cx, |project, _| project.focused_panel = Some(panel_id));
            self.commit_project_snapshot(candidate, cx);
            return Ok(());
        }

        let document = match opened.document {
            Some(document) => document,
            None => self
                .saved_document(opened.id, cx)
                .ok_or(AppRootError::MissingPanel)?,
        };
        let panel_id = self.install_editor_panel(project_id, view, document, window, cx)?;
        self.projects
            .get(&project_id)
            .cloned()
            .ok_or(ProjectRegistryError::ProjectNotFound)?
            .update(cx, |project, _| project.focused_panel = Some(panel_id));
        self.commit_project_snapshot(candidate, cx);
        self.sync_project_visibility(cx);
        Ok(())
    }

    fn install_editor_panel(
        &mut self,
        project_id: ProjectId,
        view: ViewKey,
        document: crate::app_shell::file_editor::document::Document,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<PanelId, AppRootError> {
        let project = self
            .projects
            .get(&project_id)
            .cloned()
            .ok_or(ProjectRegistryError::ProjectNotFound)?;
        let panel_id = self.panel_ids.allocate()?;
        let parent = self.project_focus_token(project_id, cx)?;
        let path = document.location().canonical_path().as_path();
        let markdown = crate::app_shell::doc_tabs::markdown_viewable(path);
        let language = crate::app_shell::code_tab::language_for_path(path);
        let editor =
            cx.new(|cx| TextFileView::from_document(document, language, markdown, window, cx));
        let visible = self.snapshot.registry().active() == Some(project_id);
        let focus_registration = self.focus_manager.register(
            &editor.read(cx).focus_handle(cx),
            parent,
            Some(project_id),
            visible,
        )?;
        if let Err(error) = self
            .focus_manager
            .request_focus(focus_registration.token(), window, cx)
        {
            let _ = self.focus_manager.unregister(focus_registration.token());
            return Err(error.into());
        }
        project.update(cx, |project, _| {
            project.panels.insert(
                panel_id,
                PanelHost {
                    id: panel_id,
                    view_key: view,
                    content: PanelContent::Editor(editor),
                },
            );
        });
        self.panel_focus.insert(panel_id, focus_registration);
        Ok(panel_id)
    }

    fn saved_document(
        &self,
        document_id: slterm_document::DocumentId,
        cx: &App,
    ) -> Option<crate::app_shell::file_editor::document::Document> {
        self.projects.values().find_map(|project| {
            project.read(cx).panels.values().find_map(|panel| {
                if panel.view_key
                    != (ViewKey::Editor {
                        project_id: panel.view_key.project_id()?,
                        document_id,
                    })
                {
                    return None;
                }
                panel.editor()?.read(cx).local_document().cloned()
            })
        })
    }

    pub(super) fn reconcile_document_registry(&mut self) {
        let referenced = self
            .snapshot
            .documents()
            .map(|(id, _)| id)
            .collect::<BTreeSet<_>>();
        self.documents.retain(|id| referenced.contains(&id));
    }
}
