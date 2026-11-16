//! AppRoot 的 Document 打开、同步编辑、保存与分页面板所有权。

#![allow(dead_code)] // 消费点 = 后续 Explorer 与真实 OpenFile 移交入口。
use std::collections::BTreeSet;
use std::io;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

use gpui::{App, AppContext as _, Context, EntityId, FocusHandle, Focusable as _, Window};
use slterm_document::{DocumentId, DocumentOpenOutcome, EditApplyOutcome, EditTransactionError};
use slterm_path::CanonicalPath;
use slterm_workspace::{PanelGroupId, PanelId, ProjectId, ProjectRegistryError, ViewKey};

use super::AppRoot;
use super::commands::AppRootError;
use super::focus_manager::FocusToken;
use super::panel_host::{EditorContent, PanelContent, PanelHost};
use super::task_service::{CancellationToken, TaskId};
use crate::app_shell::documents::{
    DocumentOpenError, OpenedDocument, OpenedDocumentBody, ResolvedDocument,
};
use crate::app_shell::file_editor::TextFileView;
use crate::app_shell::text_view::{
    DocumentEditRequest, DocumentEditResponse, DocumentSyntax, DocumentTextEvent,
    DocumentTextSnapshot, DocumentTextView, PagedTextView,
};

pub(super) type DocumentOpenCompletion =
    Box<dyn FnOnce(Result<(), AppRootError>, &mut Window, &mut Context<'_, AppRoot>) + Send>;

pub(super) struct DocumentOpenRequest {
    project_id: ProjectId,
    target_group: Option<PanelGroupId>,
    project_root: CanonicalPath,
    path: CanonicalPath,
    activate_on_open: bool,
    completion: Option<DocumentOpenCompletion>,
}

/// 一次面板安装的落点与焦点语义，避免各视图拆开传递开关。
struct DocumentPanelTarget {
    project_id: ProjectId,
    view: ViewKey,
    focus: bool,
    activate_project: bool,
}

fn document_open_io(error: io::Error) -> AppRootError {
    AppRootError::DocumentOpen(DocumentOpenError::Io(error))
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
        let request = self.prepare_document_open(project_id, Some(target_group), path, false)?;
        self.queue_document_open(request, None, window, cx);
        Ok(())
    }

    /// OpenFile 只选择覆盖目标的最深既有根；隐藏项目与打开候选一次提交。
    pub(super) fn prepare_document_handover(
        &mut self,
        path: CanonicalPath,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<DocumentOpenRequest, AppRootError> {
        self.sync_focus(window, cx);
        let project_id = self
            .snapshot
            .registry()
            .projects()
            .iter()
            .filter(|project| path.as_path().starts_with(project.root().as_path()))
            .max_by_key(|project| project.root().as_path().components().count())
            .map(|project| project.id())
            .ok_or(AppRootError::OpenFileTargetUnauthorized)?;
        let target_group = if self.snapshot.registry().active() == Some(project_id) {
            self.focused_group(project_id, cx)?
        } else {
            self.hidden_target_group(project_id, cx)?
        };
        self.prepare_document_open(project_id, target_group, path, true)
    }

    fn prepare_document_open(
        &mut self,
        project_id: ProjectId,
        target_group: Option<PanelGroupId>,
        path: CanonicalPath,
        activate_on_open: bool,
    ) -> Result<DocumentOpenRequest, AppRootError> {
        self.ensure_not_closing()?;
        self.ensure_project_snapshot_command_allowed()?;
        self.ensure_project_snapshot_recovery_allowed()?;
        if !activate_on_open && self.snapshot.registry().active() != Some(project_id) {
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
            return Err(document_open_io(io::Error::new(
                io::ErrorKind::InvalidInput,
                "文件类型不支持应用内文本编辑",
            )));
        }
        Ok(DocumentOpenRequest {
            project_id,
            target_group,
            project_root,
            path,
            activate_on_open,
            completion: None,
        })
    }

    /// 准备成功后才移交完成回调；入队本身不再产生领域失败。
    pub(super) fn queue_document_open(
        &mut self,
        mut request: DocumentOpenRequest,
        completion: Option<DocumentOpenCompletion>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        request.completion = completion;
        self.pending_document_opens.push_back(request);
        self.start_next_document_open(window, cx);
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
        let service = self.document_open_service.clone();
        let executor = cx.background_executor().clone();
        let window_handle = window.window_handle();
        self.document_open_active = true;
        self.tasks
            .register_with(TaskId::DocumentOpen, CancellationToken::new(), move || {
                let resolving = executor.spawn(async move {
                    let result = service.resolve(&request.project_root, &request.path);
                    (request, result)
                });
                cx.spawn(async move |this, cx| {
                    let (request, result) = resolving.await;
                    let _ = window_handle.update(cx, |_, window, cx| {
                        let _ = this.update(cx, |root, cx| {
                            root.finish_document_resolve(request, result, window, cx);
                        });
                    });
                })
            })
            .expect("DocumentOpen 单任务登记必须唯一");
    }

    fn finish_document_resolve(
        &mut self,
        request: DocumentOpenRequest,
        result: Result<ResolvedDocument, DocumentOpenError>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let resolved = match result {
            Ok(resolved) => resolved,
            Err(error) => {
                self.finish_document_open(request, Err(error), window, cx);
                return;
            }
        };
        if let Some(id) = self.documents.existing(&resolved.location) {
            self.finish_document_open(
                request,
                Ok(OpenedDocument {
                    id,
                    outcome: DocumentOpenOutcome::Existing,
                    body: None,
                }),
                window,
                cx,
            );
            return;
        }
        self.tasks.complete(TaskId::DocumentOpen);
        self.start_document_read(request, resolved, window, cx);
    }

    fn start_document_read(
        &mut self,
        mut request: DocumentOpenRequest,
        resolved: ResolvedDocument,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.close_in_progress || self.application_close.is_some() || self.close_completed {
            self.document_open_active = false;
            self.pending_document_opens.clear();
            self.reconcile_document_registry(cx);
            if let Some(completion) = request.completion.take() {
                completion(Err(AppRootError::Closing), window, cx);
            }
            cx.notify();
            return;
        }
        let service = self.document_open_service.clone();
        let executor = cx.background_executor().clone();
        let window_handle = window.window_handle();
        self.tasks
            .register_with(TaskId::DocumentOpen, CancellationToken::new(), move || {
                let reading = executor.spawn(async move {
                    let result = service.load(resolved);
                    (request, result)
                });
                cx.spawn(async move |this, cx| {
                    let (request, result) = reading.await;
                    let _ = window_handle.update(cx, |_, window, cx| {
                        let _ = this.update(cx, |root, cx| {
                            root.finish_document_read(request, result, window, cx);
                        });
                    });
                })
            })
            .expect("DocumentOpen 读取任务必须唯一");
    }

    fn finish_document_read(
        &mut self,
        request: DocumentOpenRequest,
        result: io::Result<crate::app_shell::documents::LoadedDocument>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let result = result.and_then(|loaded| {
            crate::app_shell::documents::DocumentOpenService::register(&mut self.documents, loaded)
        });
        self.finish_document_open(request, result.map_err(DocumentOpenError::Io), window, cx);
    }

    fn finish_document_open(
        &mut self,
        mut request: DocumentOpenRequest,
        result: Result<OpenedDocument, DocumentOpenError>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.tasks.complete(TaskId::DocumentOpen);
        self.document_open_active = false;
        if self.close_in_progress || self.application_close.is_some() || self.close_completed {
            if let Ok(opened) = &result
                && opened.outcome == DocumentOpenOutcome::Opened
            {
                self.documents.remove(opened.id);
            }
            self.pending_document_opens.clear();
            self.reconcile_document_registry(cx);
            if let Some(completion) = request.completion.take() {
                completion(Err(AppRootError::Closing), window, cx);
            }
            cx.notify();
            return;
        }
        let completion = request.completion.take();
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
        self.reconcile_document_registry(cx);
        if let Err(error) = &outcome {
            log::warn!("Document 打开未提交: {error}");
        }
        if let Some(completion) = completion {
            completion(outcome, window, cx);
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
        let activate_project =
            request.activate_on_open && self.snapshot.registry().active() != Some(project_id);
        if !request.activate_on_open && self.snapshot.registry().active() != Some(project_id) {
            return Err(AppRootError::ProjectNotActive);
        }
        let project = self
            .snapshot
            .registry()
            .project(project_id)
            .ok_or(ProjectRegistryError::ProjectNotFound)?;
        if project.root() != &request.project_root {
            return Err(document_open_io(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "项目根在打开期间已变化",
            )));
        }
        // 空项目的首组在请求完成时按最新布局解析，排队请求不得持有过期组 ID。
        let target_group = match request.target_group {
            Some(group) => group,
            None => {
                let existing = if self.snapshot.registry().active() == Some(project_id) {
                    self.focused_group(project_id, cx)?
                } else {
                    self.hidden_target_group(project_id, cx)?
                };
                existing.unwrap_or_else(|| {
                    PanelGroupId::from_uuid(uuid::Uuid::now_v7())
                        .expect("系统生成的 UUID 必须为 v7")
                })
            }
        };
        let location = self
            .documents
            .session(opened.id)
            .ok_or(AppRootError::MissingPanel)?
            .location()
            .clone();
        let mut candidate = self.snapshot.clone();
        if activate_project {
            candidate.registry_mut().activate_project(project_id)?;
        }
        let (candidate, view) =
            candidate.with_opened_document(project_id, target_group, opened.id, location)?;

        if let Ok(panel_id) = self.panel_for_view(project_id, &view, cx) {
            let token = self
                .panel_focus
                .get(&panel_id)
                .map(super::focus_manager::FocusRegistration::token)
                .ok_or(AppRootError::MissingPanel)?;
            self.focus_panel_for_open(project_id, token, activate_project, window, cx)?;
            self.projects
                .get(&project_id)
                .cloned()
                .ok_or(ProjectRegistryError::ProjectNotFound)?
                .update(cx, |project, _| project.focused_panel = Some(panel_id));
            self.commit_project_snapshot(candidate, cx);
            if activate_project {
                self.restore_project_panels(project_id, window, cx);
            }
            self.sync_project_visibility(cx);
            return Ok(());
        }

        let target = DocumentPanelTarget {
            project_id,
            view,
            focus: true,
            activate_project,
        };
        let panel_id = if self
            .documents
            .session(opened.id)
            .is_some_and(|session| session.paged().is_some())
        {
            let (reader, pending) = match opened.body {
                Some(OpenedDocumentBody::Paged(document)) => {
                    let reader = document.page_reader();
                    (reader, Some(document))
                }
                None => (
                    self.paged_documents
                        .get(&opened.id)
                        .ok_or(AppRootError::MissingPanel)?
                        .page_reader(),
                    None,
                ),
                Some(OpenedDocumentBody::Editable(_)) => {
                    return Err(document_open_io(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "分页 Document 收到可编辑正文",
                    )));
                }
            };
            let panel = self.install_paged_panel(target, opened.id, reader, window, cx)?;
            if let Some(document) = pending {
                self.paged_documents.insert(opened.id, document);
            }
            panel
        } else if matches!(&target.view, ViewKey::Markdown { .. }) {
            let document = match opened.body {
                Some(OpenedDocumentBody::Editable(document)) => *document,
                None => self
                    .saved_document(opened.id, cx)
                    .ok_or(AppRootError::MissingPanel)?,
                Some(OpenedDocumentBody::Paged(_)) => {
                    return Err(document_open_io(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "可编辑 Document 收到分页正文",
                    )));
                }
            };
            self.install_markdown_panel(target, document, window, cx)?
        } else {
            if matches!(opened.body, Some(OpenedDocumentBody::Paged(_))) {
                return Err(document_open_io(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "可编辑 Document 收到分页正文",
                )));
            }
            self.install_source_panel(target, opened.id, window, cx)?
        };
        self.projects
            .get(&project_id)
            .cloned()
            .ok_or(ProjectRegistryError::ProjectNotFound)?
            .update(cx, |project, _| project.focused_panel = Some(panel_id));
        self.commit_project_snapshot(candidate, cx);
        if activate_project {
            self.restore_project_panels(project_id, window, cx);
        }
        self.sync_project_visibility(cx);
        Ok(())
    }

    /// 语法缓存按 Document 与语言隔离；单视图覆盖不改写其他视图的 worker。
    pub(super) fn document_syntax(
        &mut self,
        document_id: DocumentId,
        language: slterm_document::LanguageId,
    ) -> Result<Option<Rc<std::cell::RefCell<DocumentSyntax>>>, AppRootError> {
        if language == slterm_document::LanguageId::PlainText {
            return Ok(None);
        }
        let key = (document_id, language);
        if let Some(syntax) = self.document_syntax.get(&key) {
            return Ok(Some(syntax.clone()));
        }
        let session = self
            .documents
            .session(document_id)
            .cloned()
            .ok_or(AppRootError::MissingPanel)?;
        let syntax = DocumentSyntax::new(&session, language)
            .map(|syntax| Rc::new(std::cell::RefCell::new(syntax)));
        if let Some(syntax) = &syntax {
            self.document_syntax.insert(key, syntax.clone());
        }
        Ok(syntax)
    }

    fn install_source_panel(
        &mut self,
        target: DocumentPanelTarget,
        document_id: DocumentId,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<PanelId, AppRootError> {
        let session = self
            .documents
            .session(document_id)
            .cloned()
            .filter(|session| session.text().is_some())
            .ok_or(AppRootError::MissingPanel)?;
        let automatic_language = slterm_document::LanguageRegistry::new()
            .for_path(session.location().canonical_path().as_path())
            .unwrap_or(slterm_document::LanguageId::PlainText);
        let project_id = target.project_id;
        let view_key = target.view.clone();
        let view_states = self.side_view_persistence.editor_view_states();
        let language = view_states
            .state(&view_key)
            .language_override()
            .unwrap_or(automatic_language);
        let syntax = self.document_syntax(document_id, language)?;
        let root = cx.entity().downgrade();
        let panel = self.install_panel(target, window, cx, move |window, cx| {
            let edit_root = root.clone();
            let selector_key = view_key.clone();
            let editor = cx.new(|cx| {
                DocumentTextView::new(
                    &session,
                    Box::new(move |request, origin, app| {
                        let root = edit_root
                            .upgrade()
                            .ok_or(EditTransactionError::DocumentNotFound(document_id))?;
                        root.update(app, |root, cx| {
                            root.apply_document_edit(document_id, origin, request, cx)
                        })
                    }),
                    automatic_language,
                    syntax,
                    view_key,
                    view_states,
                    cx,
                )
            });
            let save_subscription = cx.subscribe_in(
                &editor,
                window,
                move |root, editor, event: &DocumentTextEvent, window, cx| match event {
                    DocumentTextEvent::Save => root.save_document(document_id, cx),
                    DocumentTextEvent::OpenInSystem => {
                        root.open_document_in_system(document_id, cx);
                    }
                    DocumentTextEvent::OpenLanguageSelector { position } => {
                        if let Err(error) = root.open_language_selector(
                            project_id,
                            selector_key.clone(),
                            editor.clone(),
                            *position,
                            window,
                            cx,
                        ) {
                            log::warn!("打开 Editor 语言选择器失败: {error}");
                        }
                    }
                },
            );
            let focus_handle = editor.read(cx).focus_handle(cx);
            (
                PanelContent::Editor(EditorContent::new(editor, save_subscription)),
                focus_handle,
            )
        })?;
        Ok(panel)
    }

    fn install_markdown_panel(
        &mut self,
        target: DocumentPanelTarget,
        document: crate::app_shell::file_editor::document::Document,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<PanelId, AppRootError> {
        self.install_panel(target, window, cx, |window, cx| {
            let path = document.location().canonical_path().as_path();
            let language = crate::app_shell::code_tab::language_for_path(path);
            let editor =
                cx.new(|cx| TextFileView::from_document(document, language, true, window, cx));
            let focus_handle = editor.read(cx).focus_handle(cx);
            (PanelContent::Markdown(editor), focus_handle)
        })
    }

    fn install_paged_panel(
        &mut self,
        target: DocumentPanelTarget,
        document_id: slterm_document::DocumentId,
        reader: crate::app_shell::paged_document::PageReader,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<PanelId, AppRootError> {
        let session = self
            .documents
            .session(document_id)
            .cloned()
            .ok_or(AppRootError::MissingPanel)?;
        let request = self
            .documents
            .text_request(document_id, 9)
            .map_err(|_| AppRootError::MissingPanel)?;
        let cache = self.page_cache.clone();
        let view_key = target.view.clone();
        let view_states = self.side_view_persistence.editor_view_states();
        self.install_panel(target, window, cx, move |_, cx| {
            let paged = cx.new(|cx| {
                PagedTextView::new(&session, cache, reader, request, view_key, view_states, cx)
            });
            let focus_handle = paged.read(cx).focus_handle(cx);
            (PanelContent::Paged(paged), focus_handle)
        })
    }

    fn focus_panel_for_open(
        &mut self,
        project_id: ProjectId,
        focus_token: FocusToken,
        activate_project: bool,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        if !activate_project {
            self.focus_manager.request_focus(focus_token, window, cx)?;
            return Ok(());
        }
        self.focus_manager
            .ensure_project_activation_allowed(project_id, focus_token)?;
        let previous_active = self.snapshot.registry().active();
        let previous_focus = window.focused(cx);
        self.sync_focus_visibility(Some(project_id), cx);
        let focused = self
            .focus_manager
            .present_nonmodal(focus_token)
            .and_then(|_| {
                self.focus_manager
                    .activate_project(project_id, focus_token, window, cx)
            })
            .and_then(|_| self.focus_manager.request_focus(focus_token, window, cx));
        if let Err(error) = focused {
            self.restore_focus_after_failed_activation(previous_active, previous_focus, window, cx);
            return Err(error.into());
        }
        Ok(())
    }

    fn install_panel(
        &mut self,
        target: DocumentPanelTarget,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
        build: impl FnOnce(&mut Window, &mut Context<'_, Self>) -> (PanelContent, FocusHandle),
    ) -> Result<PanelId, AppRootError> {
        let DocumentPanelTarget {
            project_id,
            view,
            focus,
            activate_project,
        } = target;
        let project = self
            .projects
            .get(&project_id)
            .cloned()
            .ok_or(ProjectRegistryError::ProjectNotFound)?;
        let panel_id = self.panel_ids.allocate()?;
        let parent = self.project_focus_token(project_id, cx)?;
        let (content, focus_handle) = build(window, cx);
        let visible = !activate_project && self.snapshot.registry().active() == Some(project_id);
        let focus_registration =
            self.focus_manager
                .register(&focus_handle, parent, Some(project_id), visible)?;
        if focus
            && let Err(error) = self.focus_panel_for_open(
                project_id,
                focus_registration.token(),
                activate_project,
                window,
                cx,
            )
        {
            let _ = self.focus_manager.unregister(focus_registration.token());
            return Err(error);
        }
        project.update(cx, |project, _| {
            project.panels.insert(
                panel_id,
                PanelHost {
                    id: panel_id,
                    view_key: view,
                    content,
                },
            );
        });
        self.panel_focus.insert(panel_id, focus_registration);
        Ok(panel_id)
    }

    fn apply_document_edit(
        &mut self,
        document_id: DocumentId,
        origin: EntityId,
        request: DocumentEditRequest,
        cx: &mut Context<'_, Self>,
    ) -> Result<DocumentEditResponse, EditTransactionError> {
        let before = self
            .documents
            .session(document_id)
            .and_then(|session| session.text().cloned());
        let outcome = match &request {
            DocumentEditRequest::Edit(transaction, state) => {
                self.documents
                    .apply(transaction.clone(), *state, Instant::now())?
            }
            DocumentEditRequest::Travel { redo } => {
                if *redo {
                    self.documents.redo(document_id)?
                } else {
                    self.documents.undo(document_id)?
                }
            }
        };
        let session = self
            .documents
            .session(document_id)
            .cloned()
            .ok_or(EditTransactionError::DocumentNotFound(document_id))?;
        let response = DocumentEditResponse {
            snapshot: DocumentTextSnapshot::from_session(&session),
            outcome,
        };
        if outcome == EditApplyOutcome::Applied {
            // 非当前语言的缓存同样跟进正文，切回时不得发布旧 revision。
            let after = session.text().expect("普通编辑只接受 Rope").clone();
            let revision = session.text_revision();
            for ((actual, _), syntax) in &self.document_syntax {
                if *actual != document_id {
                    continue;
                }
                match &request {
                    DocumentEditRequest::Edit(transaction, _) => {
                        if let Some(before) = &before {
                            syntax
                                .borrow_mut()
                                .edit(before, after.clone(), revision, transaction);
                        }
                    }
                    DocumentEditRequest::Travel { .. } => {
                        syntax.borrow_mut().rebuild(after.clone(), revision);
                    }
                }
            }
            let views = self
                .projects
                .values()
                .flat_map(|project| {
                    project
                        .read(cx)
                        .panels
                        .values()
                        .filter_map(|panel| match &panel.content {
                            PanelContent::Editor(editor)
                                if editor.view.entity_id() != origin
                                    && matches!(&panel.view_key, ViewKey::Editor { document_id: actual, .. } if *actual == document_id) =>
                            {
                                Some(editor.view.clone())
                            }
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            for view in views {
                view.update(cx, |view, cx| match &request {
                    DocumentEditRequest::Edit(transaction, _) => {
                        view.apply_edit_snapshot(&session, transaction, cx);
                    }
                    DocumentEditRequest::Travel { .. } => view.apply_snapshot(&session, cx),
                });
            }
        }
        Ok(response)
    }

    /// HTML 系统打开只读取当前磁盘位置，不触发保存；平台调用留在后台任务。
    fn open_document_in_system(&mut self, document_id: DocumentId, cx: &mut Context<'_, Self>) {
        if self.ensure_not_closing().is_err() {
            return;
        }
        let Some(path) = self
            .documents
            .session(document_id)
            .map(|session| session.location().canonical_path().clone())
        else {
            return;
        };
        let task_id = TaskId::DocumentSystemOpen(document_id);
        if self.tasks.is_registered(task_id) {
            return;
        }
        let opener = self.system_open.clone();
        let executor = cx.background_executor().clone();
        let registration = self
            .tasks
            .register_with(task_id, CancellationToken::new(), move || {
                let open = executor.spawn(async move { opener(path) });
                cx.spawn(async move |this, cx| {
                    open.await;
                    let _ = this.update(cx, |root, _| root.tasks.complete(task_id));
                })
            });
        if registration.is_err() {
            log::debug!("document system open already scheduled");
        }
    }

    pub(super) fn save_document(&mut self, document_id: DocumentId, cx: &mut Context<'_, Self>) {
        if self.close_in_progress || self.application_close.is_some() || self.close_completed {
            return;
        }
        let task_id = TaskId::DocumentSave(document_id);
        if self.tasks.is_registered(task_id) {
            self.pending_document_saves.insert(document_id);
            return;
        }
        self.pending_document_saves.remove(&document_id);
        if self
            .documents
            .session(document_id)
            .is_none_or(|session| !session.is_dirty())
        {
            return;
        }
        let grants =
            crate::app_shell::documents::document_project_grants(&self.snapshot, document_id);
        let request = match self.documents.begin_save(document_id, 0) {
            Ok(request) => request,
            Err(error) => {
                log::warn!("Document 保存请求未建立: {error}");
                super::events::notify_document_save_state(error, cx);
                return;
            }
        };
        let save_generation = request.save_generation();
        let executor = cx.background_executor().clone();
        if let Err(error) = self
            .tasks
            .register_with(task_id, CancellationToken::new(), move || {
                let background = executor.spawn(async move {
                    crate::app_shell::documents::DocumentSaveService::default()
                        .save(request, &grants, 0)
                });
                cx.spawn(async move |this, cx| {
                    let result = background.await;
                    let _ = this.update(cx, |root, cx| {
                        root.tasks.complete(task_id);
                        match result {
                            Ok(receipt) => {
                                if let Err(error) = root.documents.complete_save(receipt) {
                                    log::warn!("Document 保存回执未采纳: {error}");
                                    let _ = root.documents.fail_save(document_id, save_generation);
                                    super::events::notify_document_save_state(error, cx);
                                }
                            }
                            Err(error) => {
                                if let Err(error) =
                                    root.documents.fail_save(document_id, save_generation)
                                {
                                    log::warn!("Document 保存失败闸门未释放: {error}");
                                }
                                log::warn!("Document 保存失败: {error}");
                                super::events::notify_document_save_error(&error, cx);
                            }
                        }
                        if root.pending_document_saves.remove(&document_id) {
                            root.save_document(document_id, cx);
                        }
                        root.resume_blocked_batch_saves(cx);
                        cx.notify();
                    });
                })
            })
        {
            let _ = self.documents.fail_save(document_id, save_generation);
            log::warn!("Document 保存任务未登记: {error:?}");
        }
    }

    pub(super) fn restore_document_panel(
        &mut self,
        project_id: ProjectId,
        view: ViewKey,
        document_id: slterm_document::DocumentId,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<PanelId, AppRootError> {
        let target = DocumentPanelTarget {
            project_id,
            view,
            focus: false,
            activate_project: false,
        };
        if !matches!(&target.view, ViewKey::Markdown { .. }) {
            return self.install_source_panel(target, document_id, window, cx);
        }
        let grants =
            crate::app_shell::documents::document_project_grants(&self.snapshot, document_id);
        let session = self
            .documents
            .session(document_id)
            .ok_or(AppRootError::MissingPanel)?;
        let write = (!session.is_read_revoked()).then(|| {
            crate::app_shell::file_editor::document::DocumentWriteContext::new(
                grants,
                0,
                0,
                Arc::new(crate::platform::document_paths::SystemDocumentPathResolver),
            )
        });
        let document =
            crate::app_shell::file_editor::document::Document::from_recovered(session, write)
                .ok_or(AppRootError::MissingPanel)?;
        self.install_markdown_panel(target, document, window, cx)
    }

    pub(super) fn restore_recovery_panel(
        &mut self,
        project_id: ProjectId,
        view: ViewKey,
        kind: super::document_recovery::DocumentRecoveryKind,
        cx: &mut Context<'_, Self>,
    ) -> Result<PanelId, AppRootError> {
        let project = self
            .projects
            .get(&project_id)
            .cloned()
            .ok_or(ProjectRegistryError::ProjectNotFound)?;
        let panel_id = self.panel_ids.allocate()?;
        let parent = self.project_focus_token(project_id, cx)?;
        let recovery = cx.new(|cx| super::document_recovery::DocumentRecoveryView::new(kind, cx));
        let visible = self.snapshot.registry().active() == Some(project_id);
        let focus = self.focus_manager.register(
            &recovery.read(cx).focus_handle,
            parent,
            Some(project_id),
            visible,
        )?;
        project.update(cx, |project, _| {
            project.panels.insert(
                panel_id,
                PanelHost {
                    id: panel_id,
                    view_key: view,
                    content: PanelContent::Recovery(recovery),
                },
            );
        });
        self.panel_focus.insert(panel_id, focus);
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
                    != (ViewKey::Markdown {
                        project_id: panel.view_key.project_id()?,
                        document_id,
                    })
                {
                    return None;
                }
                panel.markdown()?.read(cx).local_document().cloned()
            })
        })
    }

    pub(super) fn reconcile_document_registry(&mut self, cx: &mut App) {
        let referenced = self
            .snapshot
            .documents()
            .map(|(id, _)| id)
            .collect::<BTreeSet<_>>();
        for project in self.projects.values() {
            project.update(cx, |project, cx| {
                for panel in project.panels.values() {
                    let document_id = match &panel.view_key {
                        ViewKey::Editor { document_id, .. }
                        | ViewKey::Markdown { document_id, .. }
                        | ViewKey::Diff { document_id, .. } => *document_id,
                        _ => continue,
                    };
                    if !referenced.contains(&document_id)
                        && let PanelContent::Paged(view) = &panel.content
                    {
                        view.update(cx, |view, _| view.close());
                    }
                }
            });
        }
        self.documents.retain(|id| referenced.contains(&id));
        self.paged_documents.retain(|id, _| {
            if referenced.contains(id) {
                true
            } else {
                self.page_cache.borrow_mut().remove_document(*id);
                false
            }
        });
        self.document_syntax
            .retain(|(id, _), _| referenced.contains(id));
        self.pending_document_saves
            .retain(|id| referenced.contains(id));
    }
}
