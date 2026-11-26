//! AppRoot 的 Document 打开、同步编辑、保存与分页面板所有权。

#![allow(dead_code)] // 消费点 = 后续 Explorer 与真实 OpenFile 移交入口。
use std::collections::BTreeSet;
use std::io;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

use gpui::{App, AppContext as _, Context, EntityId, FocusHandle, Focusable as _, Window};
use slterm_document::{DocumentId, DocumentOpenOutcome, EditApplyOutcome, EditTransactionError};
use slterm_path::{CanonicalPath, PathGrant};
use slterm_runtime_protocol::JsonRpcError;
use slterm_workspace::{PanelGroupId, PanelId, ProjectId, ProjectRegistryError, ViewKey};

use super::AppRoot;
use super::commands::AppRootError;
use super::focus_manager::FocusToken;
use super::panel_host::{EditorContent, MarkdownContent, PanelContent, PanelHost};
use super::task_service::{CancellationToken, TaskId};
use crate::app_shell::RuntimeMutationReply;
use crate::app_shell::documents::{
    DocumentOpenError, OpenedDocument, OpenedDocumentBody, ResolvedDocument,
};
use crate::app_shell::markdown::{MarkdownView, MarkdownViewEvent};
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
    pub(super) path: CanonicalPath,
    activate_on_open: bool,
    completion: Option<DocumentOpenCompletion>,
    resolved: Option<ResolvedDocument>,
    markdown_fragment: Option<String>,
    session_read_grant: Option<PathGrant>,
    /// 打开方附加的只读来源（如 Agent 根能力）；打开与保存授权并集共用。
    pub(super) read_grants: Vec<PathGrant>,
    /// 用户显式选择的编码重开；None = BOM 与本地检测。
    encoding: Option<slterm_document::DocumentEncoding>,
}

impl DocumentOpenRequest {
    /// 进入编码选择面：清除上一次显式编码，保留路径、授权与落点事实。
    pub(super) fn prepare_for_encoding_prompt(&mut self) {
        self.encoding = None;
        self.completion = None;
    }

    /// 编码选择面选定的重开：同一请求换编码重新进读取链。
    pub(super) fn reopen_with_encoding(&mut self, encoding: slterm_document::DocumentEncoding) {
        self.encoding = Some(encoding);
        self.completion = None;
    }
}

/// 一次面板安装的落点与焦点语义，避免各视图拆开传递开关。
pub(super) struct DocumentPanelTarget {
    pub(super) project_id: ProjectId,
    pub(super) view: ViewKey,
    pub(super) focus: bool,
    pub(super) activate_project: bool,
}

enum DocumentEditPanel {
    Editor(gpui::Entity<DocumentTextView>),
    Markdown(gpui::Entity<MarkdownView>),
    Diff(gpui::Entity<super::diff::DiffView>),
}

/// 保存完成回执的封闭错误：不携带路径或底层错误文本。
fn runtime_save_error(closing: bool) -> JsonRpcError {
    if closing {
        crate::app_shell::control_error(
            slterm_runtime_protocol::DomainErrorCode::Closing,
            -32004,
            "closing",
        )
    } else {
        crate::app_shell::control_error(
            slterm_runtime_protocol::DomainErrorCode::NotReady,
            -32001,
            "document save not ready",
        )
    }
}

fn document_open_io(error: io::Error) -> AppRootError {
    AppRootError::DocumentOpen(DocumentOpenError::Io(error))
}

/// 编码歧义（未知或所选编码不合法）可由用户显式选择；二进制与其他失败不可。
fn encoding_reopen_supported(error: &DocumentOpenError) -> bool {
    matches!(error, DocumentOpenError::Io(error) if error.kind() == io::ErrorKind::Unsupported)
}

impl AppRoot {
    pub(super) fn document_read_grants(&self, document_id: DocumentId) -> Vec<PathGrant> {
        let mut grants = self.document_grants(document_id);
        if let Some(grant) = self.document_session_read_grants.get(&document_id) {
            grants.push(grant.clone());
        }
        grants
    }

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

    pub(super) fn prepare_document_open(
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
            resolved: None,
            markdown_fragment: None,
            session_read_grant: None,
            read_grants: Vec::new(),
            encoding: None,
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

    pub(super) fn queue_resolved_document_open(
        &mut self,
        mut request: DocumentOpenRequest,
        resolved: ResolvedDocument,
        markdown_fragment: Option<String>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        request.session_read_grant = resolved.session_read_grant();
        request.resolved = Some(resolved);
        request.markdown_fragment = markdown_fragment;
        self.queue_document_open(request, None, window, cx);
    }

    fn start_next_document_open(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        if self.close_in_progress || self.application_close.is_some() || self.close_completed {
            self.pending_document_opens.clear();
            return;
        }
        if self.document_open_active || self.tasks.is_registered(TaskId::DocumentOpen) {
            return;
        }
        let Some(mut request) = self.pending_document_opens.pop_front() else {
            return;
        };
        self.document_open_active = true;
        if let Some(resolved) = request.resolved.take() {
            self.start_resolved_document_open(request, resolved, window, cx);
            return;
        }
        let service = self.document_open_service.clone();
        let executor = cx.background_executor().clone();
        let window_handle = window.window_handle();
        self.tasks
            .register_with(TaskId::DocumentOpen, CancellationToken::new(), move || {
                let resolving = executor.spawn(async move {
                    let result = service.resolve_with_grants(
                        &request.project_root,
                        &request.path,
                        &request.read_grants,
                    );
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

    pub(super) fn finish_document_resolve(
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
        self.tasks.complete(TaskId::DocumentOpen);
        self.start_resolved_document_open(request, resolved, window, cx);
    }

    fn start_resolved_document_open(
        &mut self,
        request: DocumentOpenRequest,
        resolved: ResolvedDocument,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if let Some(id) = self.documents.existing(&resolved.location) {
            let needs_paged_body = self
                .documents
                .session(id)
                .is_some_and(|session| session.paged().is_some())
                && !self.paged_documents.contains_key(&id);
            if !needs_paged_body {
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
        }
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
        #[cfg(all(test, feature = "gpui-test-support"))]
        if self.test_document_read_hold.as_ref() == Some(resolved.location().canonical_path()) {
            self.held_document_read = Some((request, resolved));
            self.tasks
                .register(
                    TaskId::DocumentOpen,
                    CancellationToken::new(),
                    cx.spawn(async move |_, _| {}),
                )
                .expect("读取闸门前 DocumentOpen 回执已结算");
            return;
        }
        let service = self.document_open_service.clone();
        let executor = cx.background_executor().clone();
        let window_handle = window.window_handle();
        let requested_encoding = request.encoding;
        self.tasks
            .register_with(TaskId::DocumentOpen, CancellationToken::new(), move || {
                let reading = executor.spawn(async move {
                    let result = service.load(resolved, requested_encoding);
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

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(super) fn release_held_document_read(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.test_document_read_hold = None;
        let Some((request, resolved)) = self.held_document_read.take() else {
            return;
        };
        self.tasks.complete(TaskId::DocumentOpen);
        self.document_open_active = true;
        self.start_document_read(request, resolved, window, cx);
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
        // 交互式打开才有编码选择面；handover 与 Runtime 必须保留唯一回执。
        let promptable = completion.is_none();
        let opened_path = request.path.clone();
        let markdown_fragment = request.markdown_fragment.take();
        let outcome = match result {
            Ok(opened) => {
                let session_read_grant = request.session_read_grant.take();
                let id = opened.id;
                let is_new = opened.outcome == DocumentOpenOutcome::Opened;
                let installed = self.install_opened_document(request, opened, window, cx);
                if installed.is_ok() {
                    if let Some(grant) = session_read_grant {
                        self.document_session_read_grants.insert(id, grant);
                    }
                    self.scroll_markdown_fragment(id, markdown_fragment.as_deref(), cx);
                }
                if installed.is_err() && is_new {
                    self.documents.remove(id);
                }
                installed
            }
            Err(error) => {
                // 编码重开保留 Markdown 锚点，重新打开后仍落到同一片段。
                request.markdown_fragment = markdown_fragment;
                if promptable
                    && encoding_reopen_supported(&error)
                    && let Err(open_error) = self.open_encoding_prompt(request, window, cx)
                {
                    log::warn!("编码选择面打开失败: {open_error}");
                }
                Err(AppRootError::DocumentOpen(error))
            }
        };
        // 在途期间的项目关闭提交也必须与返回后的聚合表对账。
        self.reconcile_document_registry(cx);
        if let Err(error) = &outcome {
            log::warn!("Document 打开未提交: {error}");
        }
        if outcome.is_ok() {
            self.flush_pending_commit_diff(&opened_path, window, cx);
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
        if let Some(OpenedDocumentBody::Paged(document)) = &opened.body {
            self.paged_documents.insert(opened.id, document.clone());
        }
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
            let is_recovery_placeholder = self.projects.get(&project_id).is_some_and(|project| {
                project
                    .read(cx)
                    .panels
                    .get(&panel_id)
                    .is_some_and(|panel| matches!(panel.content, PanelContent::Recovery(_)))
            });
            if is_recovery_placeholder {
                // 重新打开同一文件时把恢复占位替换为真实视图，占位不是死端。
                let paged = match opened.body {
                    Some(OpenedDocumentBody::Paged(document)) => {
                        self.paged_documents.insert(opened.id, document.clone());
                        Some(document)
                    }
                    None => self.paged_documents.get(&opened.id).cloned(),
                    Some(OpenedDocumentBody::Editable(_)) => None,
                };
                self.replace_recovery_panel(project_id, panel_id, view.clone(), paged, window, cx)?;
            }
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
            self.install_markdown_panel(target, opened.id, window, cx)?
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

    pub(super) fn build_source_content(
        &mut self,
        target: &DocumentPanelTarget,
        document_id: DocumentId,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(PanelContent, FocusHandle), AppRootError> {
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
        let build = move |window: &mut Window, cx: &mut Context<'_, Self>| {
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
                    DocumentTextEvent::OpenGitDiff { line } => {
                        if let Err(error) =
                            root.open_editor_git_diff(project_id, document_id, *line, window, cx)
                        {
                            log::warn!("打开 Editor Git Diff 失败: {error}");
                        }
                    }
                    DocumentTextEvent::ToggleLineEnding => {
                        root.toggle_document_line_ending(document_id, cx);
                    }
                    DocumentTextEvent::ToggleManualReadOnly => {
                        root.toggle_document_manual_read_only(document_id, cx);
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
        };
        Ok(build(window, cx))
    }

    fn install_source_panel(
        &mut self,
        target: DocumentPanelTarget,
        document_id: DocumentId,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<PanelId, AppRootError> {
        let built = self.build_source_content(&target, document_id, window, cx)?;
        self.install_panel(target, window, cx, move |_, _| built)
    }

    /// Diff 复合视图：工作区侧复用来源 Editor 与专属 ViewState 租约，固定 HEAD 由后台刷新。
    pub(super) fn build_diff_content(
        &mut self,
        target: &DocumentPanelTarget,
        document_id: DocumentId,
        target_line: Option<usize>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(PanelContent, FocusHandle), AppRootError> {
        let (right, _) = self.build_source_content(target, document_id, window, cx)?;
        let PanelContent::Editor(right) = right else {
            return Err(AppRootError::MissingPanel);
        };
        let cache = self.page_cache.clone();
        let root = cx.entity().downgrade();
        let view = cx
            .new(|cx| super::diff::DiffView::new(root, document_id, right, cache, target_line, cx));
        let focus = view.read(cx).focus_handle(cx);
        Ok((PanelContent::Diff(view), focus))
    }

    pub(super) fn build_markdown_content(
        &mut self,
        target: &DocumentPanelTarget,
        document_id: DocumentId,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(PanelContent, FocusHandle), AppRootError> {
        let session = self
            .documents
            .session(document_id)
            .cloned()
            .filter(|session| session.text().is_some())
            .ok_or(AppRootError::MissingPanel)?;
        let syntax = self.document_syntax(document_id, slterm_document::LanguageId::Markdown)?;
        let view_key = target.view.clone();
        let view_states = self.side_view_persistence.editor_view_states();
        let render_services = self.markdown_render_services.clone();
        let resource_grants = self.document_grants(document_id);
        let root = cx.entity().downgrade();
        let editor = cx.new(|cx| {
            MarkdownView::new(
                &session,
                Box::new(move |request, origin, app| {
                    let root = root
                        .upgrade()
                        .ok_or(EditTransactionError::DocumentNotFound(document_id))?;
                    root.update(app, |root, cx| {
                        root.apply_document_edit(document_id, origin, request, cx)
                    })
                }),
                syntax,
                render_services,
                resource_grants,
                crate::app_shell::text_view::DocumentViewStateBinding::new(view_key, view_states),
                cx,
            )
        });
        let save_subscription = cx.subscribe_in(
            &editor,
            window,
            move |root, _, event: &MarkdownViewEvent, window, cx| match event {
                MarkdownViewEvent::Save => root.save_document(document_id, cx),
                MarkdownViewEvent::OpenLink(link) => {
                    root.open_markdown_link(document_id, link.clone(), window, cx);
                }
            },
        );
        let focus_handle = editor.read(cx).focus_handle(cx);
        Ok((
            PanelContent::Markdown(MarkdownContent::new(editor, save_subscription)),
            focus_handle,
        ))
    }

    fn install_markdown_panel(
        &mut self,
        target: DocumentPanelTarget,
        document_id: DocumentId,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<PanelId, AppRootError> {
        let built = self.build_markdown_content(&target, document_id, window, cx)?;
        self.install_panel(target, window, cx, move |_, _| built)
    }

    pub(super) fn build_paged_content(
        &mut self,
        target: &DocumentPanelTarget,
        document_id: slterm_document::DocumentId,
        reader: crate::app_shell::paged_document::PageReader,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(PanelContent, FocusHandle), AppRootError> {
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
        let build = move |_: &mut Window, cx: &mut Context<'_, Self>| {
            let paged = cx.new(|cx| {
                PagedTextView::new(&session, cache, reader, request, view_key, view_states, cx)
            });
            let focus_handle = paged.read(cx).focus_handle(cx);
            (PanelContent::Paged(paged), focus_handle)
        };
        Ok(build(window, cx))
    }

    fn install_paged_panel(
        &mut self,
        target: DocumentPanelTarget,
        document_id: slterm_document::DocumentId,
        reader: crate::app_shell::paged_document::PageReader,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<PanelId, AppRootError> {
        let built = self.build_paged_content(&target, document_id, reader, window, cx)?;
        self.install_panel(target, window, cx, move |_, _| built)
    }

    /// 外部重载的正文形态变化复用同一内容构造器，不新建布局或 Panel 身份。
    pub(super) fn build_external_content(
        &mut self,
        project_id: ProjectId,
        view: ViewKey,
        document_id: DocumentId,
        paged: Option<&Arc<crate::app_shell::paged_document::PagedDocument>>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(PanelContent, FocusHandle), AppRootError> {
        let target = DocumentPanelTarget {
            project_id,
            view,
            focus: false,
            activate_project: false,
        };
        if self
            .documents
            .session(document_id)
            .is_some_and(|session| session.paged().is_some())
        {
            let reader = paged.ok_or(AppRootError::MissingPanel)?.page_reader();
            self.build_paged_content(&target, document_id, reader, window, cx)
        } else if matches!(target.view, ViewKey::Markdown { .. }) {
            self.build_markdown_content(&target, document_id, window, cx)
        } else {
            self.build_source_content(&target, document_id, window, cx)
        }
    }

    /// 用真实正文视图替换恢复占位：复用同一 PanelId／ViewKey 与内容构造器。
    fn replace_recovery_panel(
        &mut self,
        project_id: ProjectId,
        panel_id: PanelId,
        view: ViewKey,
        paged: Option<std::sync::Arc<crate::app_shell::paged_document::PagedDocument>>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        let document_id = match &view {
            ViewKey::Editor { document_id, .. }
            | ViewKey::Markdown { document_id, .. }
            | ViewKey::Diff { document_id, .. } => *document_id,
            _ => return Err(AppRootError::MissingPanel),
        };
        let diff = matches!(&view, ViewKey::Diff { .. });
        let (content, focus) = if diff {
            let target = DocumentPanelTarget {
                project_id,
                view: view.clone(),
                focus: false,
                activate_project: false,
            };
            self.build_diff_content(&target, document_id, None, window, cx)?
        } else {
            self.build_external_content(project_id, view, document_id, paged.as_ref(), window, cx)?
        };
        let parent = self.project_focus_token(project_id, cx)?;
        let registration = self.focus_manager.register(
            &focus,
            parent,
            Some(project_id),
            self.snapshot.registry().active() == Some(project_id),
        )?;
        let project = self
            .projects
            .get(&project_id)
            .cloned()
            .ok_or(ProjectRegistryError::ProjectNotFound)?;
        project.update(cx, |project, _| {
            let panel = project
                .panels
                .get_mut(&panel_id)
                .expect("恢复占位面板必须存活到替换");
            panel.content = content;
        });
        if let Some(previous) = self.panel_focus.insert(panel_id, registration) {
            let _ = self.focus_manager.unregister(previous.token());
        }
        if diff && let Ok(view) = self.diff_view(panel_id, cx) {
            let owner = super::task_service::ViewTaskOwner::Panel(panel_id);
            if let Some(ticket) = self.tasks.request_view_refresh(owner) {
                self.schedule_diff_head(panel_id, view, ticket, false, cx);
            }
        }
        self.panel_subscriptions.remove(&panel_id);
        Ok(())
    }

    fn authorization_placeholder(
        &self,
        project_id: ProjectId,
        panel_id: PanelId,
        cx: &App,
    ) -> Option<gpui::Entity<super::document_recovery::DocumentRecoveryView>> {
        let project = self.projects.get(&project_id)?.read(cx);
        let PanelContent::Recovery(view) = &project.panels.get(&panel_id)?.content else {
            return None;
        };
        (view.read(cx).kind() == super::document_recovery::DocumentRecoveryKind::Authorization)
            .then(|| view.clone())
    }

    /// 根外授权占位的唯一生产入口：前台只取记录与精确会话授权，真实读盘走后台。
    pub(super) fn start_document_reauthorization(
        &mut self,
        project_id: ProjectId,
        view: ViewKey,
        document_id: DocumentId,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let task_id = TaskId::DocumentReauthorize(document_id);
        if self.tasks.is_registered(task_id) {
            return;
        }
        let Ok(panel_id) = self.panel_for_view(project_id, &view, cx) else {
            return;
        };
        if self
            .authorization_placeholder(project_id, panel_id, cx)
            .is_none()
        {
            return;
        }
        let Some((location, grant)) = self
            .side_view_persistence
            .prepare_document_reauthorization(document_id)
        else {
            // 正文已读入而占位替换曾失败：直接从现有会话重建全部相关面板，不重复读盘。
            self.replace_authorization_placeholders(
                document_id,
                Some((project_id, panel_id)),
                window,
                cx,
            );
            return;
        };
        let executor = cx.background_executor().clone();
        let window_handle = window.window_handle();
        self.tasks
            .register_with(task_id, CancellationToken::new(), move || {
                let reading = executor.spawn(async move {
                    crate::persistence::document_snapshot::recovery::read_reauthorized_document(
                        &location, grant,
                    )
                });
                cx.spawn(async move |this, cx| {
                    let result = reading.await;
                    let _ = window_handle.update(cx, |_, window, cx| {
                        let _ = this.update(cx, |root, cx| {
                            root.finish_document_reauthorization(
                                project_id,
                                view,
                                document_id,
                                result,
                                window,
                                cx,
                            );
                        });
                    });
                })
            })
            .expect("重授权任务必须按 DocumentId 唯一");
    }

    pub(super) fn finish_document_reauthorization(
        &mut self,
        project_id: ProjectId,
        view: ViewKey,
        document_id: DocumentId,
        result: io::Result<crate::persistence::document_snapshot::recovery::ReauthorizedDocument>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.tasks
            .complete(TaskId::DocumentReauthorize(document_id));
        if self.close_in_progress || self.application_close.is_some() || self.close_completed {
            return;
        }
        if !self.snapshot.documents().any(|(id, _)| id == document_id) {
            return;
        }
        let Ok(panel_id) = self.panel_for_view(project_id, &view, cx) else {
            return;
        };
        let Some(recovery) = self.authorization_placeholder(project_id, panel_id, cx) else {
            return;
        };
        let prepared = match result {
            Ok(prepared) => prepared,
            Err(error) => {
                log::warn!("Document 重授权读取失败: {}", error.kind());
                recovery.update(cx, |view, cx| view.show_reauthorization_failed(cx));
                return;
            }
        };
        if let Err(error) = self.side_view_persistence.finish_document_reauthorization(
            &mut self.documents,
            document_id,
            prepared,
        ) {
            log::warn!("Document 重授权结算失败: {}", error.kind());
            recovery.update(cx, |view, cx| view.show_reauthorization_failed(cx));
            return;
        }
        self.replace_authorization_placeholders(
            document_id,
            Some((project_id, panel_id)),
            window,
            cx,
        );
    }

    /// 同一次显式授权必须恢复该 Document 的全部占位视图；分页正文无 reader 时切专用占位。
    fn replace_authorization_placeholders(
        &mut self,
        document_id: DocumentId,
        focus: Option<(ProjectId, PanelId)>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let mut targets = Vec::new();
        for (project_id, project) in &self.projects {
            let project = project.read(cx);
            for panel in project.panels.values() {
                if super::runtime_snapshot::view_document_id(&panel.view_key) != Some(document_id) {
                    continue;
                }
                let PanelContent::Recovery(view) = &panel.content else {
                    continue;
                };
                if view.read(cx).kind()
                    != super::document_recovery::DocumentRecoveryKind::Authorization
                {
                    continue;
                }
                targets.push((*project_id, panel.id, panel.view_key.clone(), view.clone()));
            }
        }
        if self
            .documents
            .session(document_id)
            .is_some_and(|session| session.paged().is_some())
        {
            for (_, _, _, view) in targets {
                view.update(cx, |view, cx| {
                    view.set_kind(super::document_recovery::DocumentRecoveryKind::Paged, cx);
                });
            }
        } else {
            for (project_id, panel_id, view_key, view) in targets {
                if let Err(error) =
                    self.replace_recovery_panel(project_id, panel_id, view_key, None, window, cx)
                {
                    log::warn!("恢复占位替换失败: {error}");
                    view.update(cx, |view, cx| view.show_reauthorization_failed(cx));
                }
            }
        }
        if let Some((project_id, panel_id)) = focus {
            self.focus_recovered_panel(project_id, panel_id, window, cx);
        }
    }

    fn focus_recovered_panel(
        &mut self,
        project_id: ProjectId,
        panel_id: PanelId,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(token) = self
            .panel_focus
            .get(&panel_id)
            .map(super::focus_manager::FocusRegistration::token)
        else {
            return;
        };
        if let Err(error) = self.focus_panel_for_open(project_id, token, false, window, cx) {
            log::warn!("恢复面板送焦失败: {error}");
            return;
        }
        if let Some(project) = self.projects.get(&project_id) {
            project.update(cx, |project, _| project.focused_panel = Some(panel_id));
        }
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

    pub(super) fn install_panel(
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
        let is_editor = matches!(&view, ViewKey::Editor { .. });
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
        if is_editor {
            self.schedule_editor_git(panel_id, true, cx);
        }
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
            self.document_autosave.on_edit(document_id, Instant::now());
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
                        .filter_map(|panel| {
                            let matches_document = matches!(&panel.view_key, ViewKey::Editor { document_id: actual, .. } | ViewKey::Markdown { document_id: actual, .. } | ViewKey::Diff { document_id: actual, .. } if *actual == document_id);
                            if !matches_document {
                                return None;
                            }
                            match &panel.content {
                                PanelContent::Editor(editor)
                                    if editor.view.entity_id() != origin =>
                                {
                                    Some(DocumentEditPanel::Editor(editor.view.clone()))
                                }
                                PanelContent::Markdown(markdown) => {
                                    Some(DocumentEditPanel::Markdown(markdown.view.clone()))
                                }
                                PanelContent::Diff(diff) => {
                                    Some(DocumentEditPanel::Diff(diff.clone()))
                                }
                                _ => None,
                            }
                        })
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            for view in views {
                match view {
                    DocumentEditPanel::Editor(view) => {
                        view.update(cx, |view, cx| match &request {
                            DocumentEditRequest::Edit(transaction, _) => {
                                view.apply_edit_snapshot(&session, transaction, cx);
                            }
                            DocumentEditRequest::Travel { .. } => view.apply_snapshot(&session, cx),
                        });
                    }
                    DocumentEditPanel::Markdown(view) => {
                        view.update(cx, |view, cx| match &request {
                            DocumentEditRequest::Edit(transaction, _) => {
                                view.apply_edit_snapshot(&session, transaction, origin, cx);
                            }
                            DocumentEditRequest::Travel { .. } => view.apply_snapshot(&session, cx),
                        });
                    }
                    DocumentEditPanel::Diff(view) => {
                        view.update(cx, |view, cx| match &request {
                            DocumentEditRequest::Edit(transaction, _) => {
                                view.apply_shared_edit(&session, transaction, origin, cx);
                            }
                            DocumentEditRequest::Travel { .. } => {
                                view.apply_shared_snapshot(&session, origin, cx)
                            }
                        });
                    }
                }
            }
        }
        if outcome == EditApplyOutcome::Applied {
            // 脏正文进入崩溃快照队列；撤销回干净基线会排入删除。
            self.sync_document_snapshot(&session);
        }
        self.refresh_editor_git_for_document(document_id, cx);
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

    pub(super) fn poll_document_autosave(&mut self, cx: &mut Context<'_, Self>) {
        self.sync_document_autosave_enabled(cx);
        let ready = self.document_autosave.tick(&self.documents, Instant::now());
        for document_id in ready {
            self.save_document(document_id, cx);
        }
    }

    pub(super) fn sync_document_autosave_enabled(&mut self, cx: &App) {
        if let Some(settings) = cx.try_global::<crate::app_shell::config::Settings>() {
            self.document_autosave
                .set_enabled(settings.document_autosave);
        }
    }

    pub(super) fn save_document(&mut self, document_id: DocumentId, cx: &mut Context<'_, Self>) {
        if self.close_in_progress || self.application_close.is_some() || self.close_completed {
            self.finish_runtime_save_waiters(document_id, Err(runtime_save_error(true)));
            return;
        }
        let task_id = TaskId::DocumentSave(document_id);
        if self.tasks.is_registered(task_id)
            || self
                .tasks
                .is_registered(TaskId::DocumentExternal(document_id))
            || self.pending_document_relocations.contains_key(&document_id)
        {
            self.pending_document_saves.insert(document_id);
            return;
        }
        self.pending_document_saves.remove(&document_id);
        if self
            .documents
            .session(document_id)
            .is_none_or(|session| !session.is_dirty())
        {
            self.finish_runtime_save_waiters(document_id, Ok(()));
            return;
        }
        let grants = self.document_grants(document_id);
        let current_generation = self.current_authorization_generation();
        let request = match self.documents.begin_save(document_id, current_generation) {
            Ok(request) => request,
            Err(error) => {
                log::warn!("Document 保存请求未建立: {error}");
                super::events::notify_document_save_state(error, cx);
                self.finish_runtime_save_waiters(document_id, Err(runtime_save_error(false)));
                return;
            }
        };
        let save_generation = request.save_generation();
        let attempted_revision = request.text_revision();
        let executor = cx.background_executor().clone();
        if let Err(error) = self
            .tasks
            .register_with(task_id, CancellationToken::new(), move || {
                let background = executor.spawn(async move {
                    let current = crate::app_shell::authorization::current_generation();
                    crate::app_shell::documents::DocumentSaveService::default()
                        .save(request, &grants, current)
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
                                    root.document_autosave.save_failed(
                                        &root.documents,
                                        document_id,
                                        attempted_revision,
                                    );
                                    super::events::notify_document_save_state(error, cx);
                                    root.finish_runtime_save_waiters(
                                        document_id,
                                        Err(runtime_save_error(false)),
                                    );
                                } else {
                                    // 保存清脏后冗余快照排入删除；失败路径保留现场。
                                    root.reconcile_saved_document(document_id);
                                    if !root.pending_document_saves.contains(&document_id) {
                                        // 仍有排队保存时不提前回执，由后续保存尝试落定。
                                        root.finish_runtime_save_waiters(document_id, Ok(()));
                                    }
                                }
                            }
                            Err(error) => {
                                if let Err(error) =
                                    root.documents.fail_save(document_id, save_generation)
                                {
                                    log::warn!("Document 保存失败闸门未释放: {error}");
                                }
                                root.document_autosave.save_failed(
                                    &root.documents,
                                    document_id,
                                    attempted_revision,
                                );
                                log::warn!("Document 保存失败: {error}");
                                super::events::notify_document_save_error(&error, cx);
                                root.finish_runtime_save_waiters(
                                    document_id,
                                    Err(runtime_save_error(false)),
                                );
                            }
                        }
                        if root.pending_document_saves.remove(&document_id) {
                            root.save_document(document_id, cx);
                        }
                        root.resume_blocked_batch_saves(cx);
                        root.resume_pending_application_close(cx);
                        cx.notify();
                    });
                })
            })
        {
            let _ = self.documents.fail_save(document_id, save_generation);
            self.document_autosave
                .save_failed(&self.documents, document_id, attempted_revision);
            log::warn!("Document 保存任务未登记: {error:?}");
            self.finish_runtime_save_waiters(document_id, Err(runtime_save_error(false)));
        }
    }

    /// 行尾切换写入共享 Document；保存时统一为所选行尾。
    pub(super) fn toggle_document_line_ending(
        &mut self,
        document_id: DocumentId,
        cx: &mut Context<'_, Self>,
    ) {
        if self.ensure_not_closing().is_err() {
            return;
        }
        let Some(next) = self.documents.session(document_id).map(|session| {
            match session.encoding().line_ending {
                slterm_document::LineEnding::Lf => slterm_document::LineEnding::Crlf,
                slterm_document::LineEnding::Crlf => slterm_document::LineEnding::Lf,
            }
        }) else {
            return;
        };
        if let Err(error) = self.documents.set_line_ending(document_id, next) {
            log::warn!("行尾切换未生效: {error}");
            return;
        }
        self.broadcast_document_metadata(document_id, cx);
    }

    /// 手动只读是瞬态；开启只允许干净可编辑正文。
    pub(super) fn toggle_document_manual_read_only(
        &mut self,
        document_id: DocumentId,
        cx: &mut Context<'_, Self>,
    ) {
        if self.ensure_not_closing().is_err() {
            return;
        }
        let Some(enable) = self
            .documents
            .session(document_id)
            .map(|session| !session.is_manually_read_only())
        else {
            return;
        };
        if let Err(error) = self.documents.set_manual_read_only(document_id, enable) {
            log::warn!("手动只读切换未生效: {error}");
            return;
        }
        self.broadcast_document_metadata(document_id, cx);
    }

    /// 元数据变更广播到全部引用视图，不重建正文或 Document。
    fn broadcast_document_metadata(&mut self, document_id: DocumentId, cx: &mut Context<'_, Self>) {
        let Some(session) = self.documents.session(document_id).cloned() else {
            return;
        };
        let mut editors = Vec::new();
        let mut markdowns = Vec::new();
        let mut diffs = Vec::new();
        for project in self.projects.values() {
            for panel in project.read(cx).panels.values() {
                let matches_document = matches!(
                    &panel.view_key,
                    ViewKey::Editor { document_id: actual, .. }
                        | ViewKey::Markdown { document_id: actual, .. }
                        | ViewKey::Diff { document_id: actual, .. }
                        if *actual == document_id
                );
                if !matches_document {
                    continue;
                }
                match &panel.content {
                    PanelContent::Editor(editor) => editors.push(editor.view.clone()),
                    PanelContent::Markdown(markdown) => markdowns.push(markdown.view.clone()),
                    PanelContent::Diff(diff) => diffs.push(diff.clone()),
                    _ => {}
                }
            }
        }
        for editor in editors {
            editor.update(cx, |view, cx| view.apply_document_metadata(&session, cx));
        }
        for markdown in markdowns {
            markdown.update(cx, |view, cx| view.apply_document_metadata(&session, cx));
        }
        for diff in diffs {
            diff.update(cx, |view, cx| view.apply_document_metadata(&session, cx));
        }
        cx.notify();
    }

    /// 保存链唯一出口：一次性回执所有 Runtime `panel.save` 等待者。
    fn finish_runtime_save_waiters(
        &mut self,
        document_id: DocumentId,
        result: Result<(), JsonRpcError>,
    ) {
        let Some(waiters) = self.runtime_save_waiters.remove(&document_id) else {
            return;
        };
        for waiter in waiters {
            let _ = waiter.try_send(Ok(RuntimeMutationReply::Done(result.clone())));
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
        if matches!(&target.view, ViewKey::Markdown { .. }) {
            self.install_markdown_panel(target, document_id, window, cx)
        } else {
            self.install_source_panel(target, document_id, window, cx)
        }
    }

    pub(super) fn restore_recovery_panel(
        &mut self,
        project_id: ProjectId,
        view: ViewKey,
        document_id: DocumentId,
        kind: super::document_recovery::DocumentRecoveryKind,
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
        let recovery = cx.new(|cx| super::document_recovery::DocumentRecoveryView::new(kind, cx));
        let event_view = view.clone();
        let subscription = cx.subscribe_in(&recovery, window, move |root, _, event, window, cx| {
            if matches!(
                event,
                super::document_recovery::DocumentRecoveryEvent::Reauthorize
            ) {
                root.start_document_reauthorization(
                    project_id,
                    event_view.clone(),
                    document_id,
                    window,
                    cx,
                );
            }
        });
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
        self.panel_subscriptions.insert(panel_id, subscription);
        Ok(panel_id)
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
        self.document_session_read_grants
            .retain(|id, _| referenced.contains(id));
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
        self.pending_document_relocations
            .retain(|id, _| referenced.contains(id));
        self.pending_document_refreshes
            .retain(|id| referenced.contains(id));
        let cancelled = self
            .active_document_relocations
            .keys()
            .copied()
            .filter(|id| !referenced.contains(id))
            .collect::<Vec<_>>();
        for id in cancelled {
            self.tasks.cancel(TaskId::DocumentExternal(id));
        }
        self.active_document_relocations
            .retain(|id, _| referenced.contains(id));
        self.last_document_relocation_targets
            .retain(|id, _| referenced.contains(id));
        self.document_relocation_aliases
            .retain(|_, id| referenced.contains(id));
        self.reconcile_fs_watch();
    }
}
