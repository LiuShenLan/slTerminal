//! AppRoot 的真实 watcher 消费与 Document 外部变化闭环。

use std::sync::Arc;
use std::time::Instant;

use gpui::{App, Context, Entity, Window};
use slterm_document::{
    DocumentExternalChange, DocumentExternalRequest, DocumentId, ExternalApplyOutcome,
    ExternalChangeError,
};
use slterm_path::{CanonicalPath, ResolvedPath};

use super::AppRoot;
use super::panel_host::PanelContent;
use super::task_service::{CancellationToken, TaskId};
use crate::app_shell::document_external::{DocumentExternalResult, DocumentExternalService};

impl AppRoot {
    /// 全部项目根、存活 Document 与在途改名目标共用同一 watcher。
    pub(super) fn reconcile_fs_watch(&mut self) {
        // Document 只登记精确文件与改名别名；目录树事件由实际目录消费者登记。
        let mut files = self
            .snapshot
            .documents()
            .filter(|(id, _)| {
                self.documents
                    .session(*id)
                    .is_some_and(|session| !session.is_read_revoked())
            })
            .map(|(_, location)| location.canonical_path().as_path().to_path_buf())
            .collect::<Vec<_>>();
        files.extend(
            self.document_relocation_aliases
                .iter()
                .filter(|(_, id)| {
                    self.documents
                        .session(**id)
                        .is_some_and(|session| !session.is_read_revoked())
                })
                .map(|(path, _)| path.clone()),
        );
        files.sort();
        files.dedup();
        if let Some(watch) = &mut self.fs_watch {
            watch.retain(Vec::new(), files);
        }
    }

    /// 前台只收割事件并捕获不可变请求；真实路径解析、读取与授权均在后台完成。
    pub(super) fn poll_fs_watch(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        if self.close_in_progress || self.application_close.is_some() || self.close_completed {
            return;
        }
        let pending = self
            .fs_watch
            .as_mut()
            .map(|watch| watch.take_pending())
            .unwrap_or_default();
        for path in pending.files {
            let Some(document_id) = self
                .document_relocation_aliases
                .get(&path)
                .copied()
                .or_else(|| {
                    self.snapshot.documents().find_map(|(id, location)| {
                        (location.canonical_path().as_path() == path).then_some(id)
                    })
                })
            else {
                continue;
            };
            let Some(session) = self.documents.session(document_id) else {
                continue;
            };
            if session.is_read_revoked() {
                continue;
            }
            let target = self
                .pending_document_relocations
                .get(&document_id)
                .cloned()
                .or_else(|| self.active_document_relocations.get(&document_id).cloned())
                .or_else(|| {
                    self.last_document_relocation_targets
                        .get(&document_id)
                        .cloned()
                })
                .unwrap_or_else(|| session.location().canonical_path().clone());
            self.pending_document_refreshes.insert(document_id);
            self.pending_document_relocations
                .insert(document_id, target);
        }
        let mut changed = false;
        for (source, target) in pending.renames {
            let document_id = self
                .document_relocation_aliases
                .get(&source)
                .copied()
                .or_else(|| {
                    self.snapshot.documents().find_map(|(id, location)| {
                        (location.canonical_path().as_path() == source).then_some(id)
                    })
                });
            let Some(document_id) = document_id else {
                continue;
            };
            match CanonicalPath::new(target) {
                Ok(target) => {
                    self.pending_document_refreshes.remove(&document_id);
                    self.document_relocation_aliases.remove(&source);
                    self.pending_document_relocations
                        .insert(document_id, target.clone());
                    self.insert_relocation_alias(target.as_path().to_path_buf(), document_id);
                    changed = true;
                }
                Err(error) => log::warn!("watcher 改名目标不是规范绝对路径: {error}"),
            }
        }
        if changed {
            self.reconcile_fs_watch();
        }
        let pending = self
            .pending_document_relocations
            .keys()
            .copied()
            .collect::<Vec<_>>();
        for document_id in pending {
            self.start_document_external(document_id, window, cx);
        }
    }

    /// 目标属于其他 Document 时不得覆盖其 watcher 路由。
    fn insert_relocation_alias(&mut self, path: std::path::PathBuf, document_id: DocumentId) {
        let occupied = self
            .snapshot
            .documents()
            .any(|(id, location)| id != document_id && location.canonical_path().as_path() == path);
        if !occupied {
            self.document_relocation_aliases.insert(path, document_id);
        }
    }

    fn start_document_external(
        &mut self,
        document_id: DocumentId,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let task_id = TaskId::DocumentExternal(document_id);
        if self.tasks.is_registered(task_id)
            || self.tasks.is_registered(TaskId::DocumentSave(document_id))
        {
            return;
        }
        let Some(session) = self.documents.session(document_id) else {
            self.pending_document_relocations.remove(&document_id);
            self.pending_document_refreshes.remove(&document_id);
            return;
        };
        // 保存写盘与外部读取互斥；core 的位置与代次对账是第二道防线。
        if session.is_saving() {
            return;
        }
        if self.markdown_has_pending_io(document_id, cx) {
            return;
        }
        let Some(target) = self.pending_document_relocations.get(&document_id).cloned() else {
            return;
        };
        let refresh_only = self.pending_document_refreshes.contains(&document_id);
        if session.location().canonical_path() == &target && !refresh_only {
            self.pending_document_relocations.remove(&document_id);
            self.last_document_relocation_targets.remove(&document_id);
            self.reconcile_relocation_aliases(document_id);
            self.reconcile_fs_watch();
            self.resume_blocked_batch_saves(cx);
            if self.pending_document_saves.remove(&document_id) {
                self.save_document(document_id, cx);
            }
            return;
        }
        let request = match self.documents.external_request(document_id, 0) {
            Ok(request) => request,
            Err(ExternalChangeError::DocumentNotFound(_)) => return,
            Err(ExternalChangeError::ReadRevoked) => {
                self.pending_document_relocations.remove(&document_id);
                self.pending_document_refreshes.remove(&document_id);
                self.reconcile_fs_watch();
                return;
            }
            Err(error) => {
                log::warn!("Document 外部变化请求未建立: {error}");
                return;
            }
        };
        // 普通事件只表达“需要重读”；目标已偏离时必须走真实目标与改名链。
        let refresh_only = refresh_only && request.location().canonical_path() == &target;
        self.pending_document_relocations.remove(&document_id);
        self.pending_document_refreshes.remove(&document_id);
        self.active_document_relocations
            .insert(document_id, target.clone());
        if refresh_only {
            self.last_document_relocation_targets.remove(&document_id);
        } else {
            self.last_document_relocation_targets
                .insert(document_id, target.clone());
            self.insert_relocation_alias(target.as_path().to_path_buf(), document_id);
        }
        self.reconcile_relocation_aliases(document_id);
        self.reconcile_fs_watch();
        let grants =
            crate::app_shell::documents::document_project_grants(&self.snapshot, document_id);
        let service = self.document_external_service.clone();
        let executor = cx.background_executor().clone();
        let window_handle = window.window_handle();
        self.tasks
            .register_with(task_id, CancellationToken::new(), move || {
                let reading = executor.spawn(async move {
                    let result = if refresh_only {
                        service.refresh_request(request, &grants, 0)
                    } else {
                        service.relocate(request, &target, &grants, 0).map(Some)
                    };
                    (result, target, refresh_only)
                });
                cx.spawn(async move |this, cx| {
                    let (result, target, refresh_only) = reading.await;
                    let _ = window_handle.update(cx, |_, window, cx| {
                        let _ = this.update(cx, |root, cx| {
                            root.finish_document_external(
                                document_id,
                                target,
                                refresh_only,
                                result,
                                window,
                                cx,
                            );
                        });
                    });
                })
            })
            .expect("Document 外部任务必须按 DocumentId 唯一");
    }

    pub(super) fn finish_document_external(
        &mut self,
        document_id: DocumentId,
        target: CanonicalPath,
        refresh_only: bool,
        result: std::io::Result<Option<DocumentExternalResult>>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.tasks.complete(TaskId::DocumentExternal(document_id));
        if self.active_document_relocations.remove(&document_id) != Some(target.clone()) {
            log::warn!("Document 外部回执身份已失效");
            return;
        }
        if self.close_in_progress || self.application_close.is_some() || self.close_completed {
            self.pending_document_relocations.clear();
            self.pending_document_refreshes.clear();
            self.last_document_relocation_targets.clear();
            self.document_relocation_aliases.clear();
            self.reconcile_fs_watch();
            return;
        }
        match result {
            Ok(Some(result)) => {
                self.apply_document_external_result(
                    document_id,
                    target,
                    refresh_only,
                    result,
                    window,
                    cx,
                );
            }
            Ok(None) => {}
            Err(error) => log::warn!("Document 外部读取失败: {error}"),
        }
        self.reconcile_relocation_aliases(document_id);
        self.reconcile_fs_watch();
        self.start_document_external(document_id, window, cx);
        if !self.pending_document_relocations.contains_key(&document_id)
            && !self
                .tasks
                .is_registered(TaskId::DocumentExternal(document_id))
            && self.pending_document_saves.remove(&document_id)
        {
            self.save_document(document_id, cx);
        }
        self.resume_blocked_batch_saves(cx);
        cx.notify();
    }

    fn markdown_has_pending_io(&self, document_id: DocumentId, cx: &App) -> bool {
        self.projects.values().any(|project| {
            project.read(cx).panels.values().any(|panel| {
                matches!(&panel.view_key, slterm_workspace::ViewKey::Markdown { document_id: id, .. } if *id == document_id)
                    && matches!(&panel.content, PanelContent::Markdown(view) if view.read(cx).has_pending_io())
            })
        })
    }

    /// Markdown 旧宿主仍有独立输入草稿；脏草稿遇外部重载必须转共享冲突。
    fn protect_markdown_draft(
        &self,
        document_id: DocumentId,
        change: DocumentExternalChange,
        cx: &App,
    ) -> DocumentExternalChange {
        let dirty = self.projects.values().any(|project| {
            project.read(cx).panels.values().any(|panel| {
                matches!(&panel.view_key, slterm_workspace::ViewKey::Markdown { document_id: id, .. } if *id == document_id)
                    && matches!(&panel.content, PanelContent::Markdown(view) if view.read(cx).is_dirty())
            })
        });
        if !dirty {
            return change;
        }
        match change {
            DocumentExternalChange::Reload {
                request,
                content,
                disk_baseline,
                location,
                ..
            } => {
                let conflict = match content {
                    slterm_document::DocumentBody::Editable(_) => {
                        slterm_document::DocumentConflict::modified(location, disk_baseline)
                    }
                    slterm_document::DocumentBody::Paged(_) => {
                        slterm_document::DocumentConflict::modified_paged(location)
                    }
                };
                DocumentExternalChange::Conflict { request, conflict }
            }
            _ => change,
        }
    }

    pub(super) fn apply_document_external_result(
        &mut self,
        document_id: DocumentId,
        target: CanonicalPath,
        refresh_only: bool,
        result: DocumentExternalResult,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self
            .pending_document_relocations
            .get(&document_id)
            .is_some_and(|latest| latest != &target)
            || (!refresh_only
                && self
                    .last_document_relocation_targets
                    .get(&document_id)
                    .is_some_and(|latest| latest != &target))
        {
            return;
        }
        let DocumentExternalResult { change, paged } = result;
        let change = self.reauthorize_external_change(document_id, change);
        if self.markdown_has_pending_io(document_id, cx) {
            if refresh_only {
                self.pending_document_refreshes.insert(document_id);
            }
            self.pending_document_relocations
                .entry(document_id)
                .or_insert(target);
            return;
        }
        let change = self.protect_markdown_draft(document_id, change, cx);
        let candidate = match &change {
            DocumentExternalChange::Relocated { location, .. }
            | DocumentExternalChange::Reload { location, .. }
                if self.snapshot.document(document_id) != Some(location) =>
            {
                match self
                    .snapshot
                    .with_relocated_document(document_id, location.clone())
                {
                    Ok(candidate) => Some(candidate),
                    Err(error) => {
                        log::warn!("Document 外部变化快照候选未提交: {error}");
                        return;
                    }
                }
            }
            _ => None,
        };
        let original_documents = self.documents.clone();
        match self.documents.apply_external(change, Instant::now()) {
            Ok(outcome) => {
                if matches!(
                    outcome,
                    ExternalApplyOutcome::Relocated | ExternalApplyOutcome::Reloaded
                ) {
                    if let Err(error) = self.apply_document_location_to_views(
                        document_id,
                        outcome,
                        paged,
                        window,
                        cx,
                    ) {
                        self.renew_document_view_leases(document_id, cx);
                        self.documents = original_documents;
                        log::warn!("Document 外部变化视图提交失败，已回滚 Registry: {error}");
                        return;
                    }
                    if let Some(candidate) = candidate {
                        self.commit_project_snapshot(candidate, cx);
                    }
                }
                if outcome == ExternalApplyOutcome::ReadRevoked {
                    self.reconcile_fs_watch();
                }
            }
            Err(ExternalChangeError::StaleExternalChange) => {
                if refresh_only {
                    self.pending_document_refreshes.insert(document_id);
                }
                self.pending_document_relocations
                    .entry(document_id)
                    .or_insert(target);
            }
            Err(ExternalChangeError::DocumentNotFound(_)) => {}
            Err(error) => log::warn!("Document 外部变化回执未采纳: {error}"),
        }
    }

    /// 回投使用最新项目引用计算 grant 并集，发起时的旧授权不能延长读取权。
    pub(super) fn reauthorize_external_change(
        &self,
        document_id: DocumentId,
        change: DocumentExternalChange,
    ) -> DocumentExternalChange {
        let request_and_location = match &change {
            DocumentExternalChange::Relocated { request, location }
            | DocumentExternalChange::Reload {
                request, location, ..
            } => Some((request, location)),
            DocumentExternalChange::Conflict { request, conflict } => {
                Some((request, conflict.location()))
            }
            DocumentExternalChange::Deleted(request) => Some((request, request.location())),
            DocumentExternalChange::ReadRevoked { request, location } => Some((request, location)),
        };
        let Some((request, location)) = request_and_location else {
            return change;
        };
        let grants =
            crate::app_shell::documents::document_project_grants(&self.snapshot, document_id);
        if Self::external_location_authorized(request, location, &grants) {
            change
        } else {
            DocumentExternalChange::ReadRevoked {
                request: request.clone(),
                location: location.clone(),
            }
        }
    }

    fn external_location_authorized(
        request: &DocumentExternalRequest,
        location: &ResolvedPath,
        grants: &[slterm_path::PathGrant],
    ) -> bool {
        DocumentExternalService::authorize(request, location.canonical_path(), location, grants, 0)
    }

    /// 保留当前位置与在途目标；只有没有更新 pending 时才保留最近目标。
    fn reconcile_relocation_aliases(&mut self, document_id: DocumentId) {
        let current = self
            .documents
            .session(document_id)
            .map(|session| session.location().canonical_path().as_path().to_path_buf());
        let active = self.active_document_relocations.get(&document_id).cloned();
        let pending = self.pending_document_relocations.get(&document_id).cloned();
        let last = if pending.is_some() {
            None
        } else {
            self.last_document_relocation_targets
                .get(&document_id)
                .cloned()
        };
        self.document_relocation_aliases.retain(|path, id| {
            *id != document_id
                || Some(path) == current.as_ref()
                || active
                    .as_ref()
                    .is_some_and(|target| path == target.as_path())
                || pending
                    .as_ref()
                    .is_some_and(|target| path == target.as_path())
                || last.as_ref().is_some_and(|target| path == target.as_path())
        });
        for path in current.into_iter().chain(
            active
                .iter()
                .chain(pending.iter())
                .chain(last.iter())
                .map(|path| path.as_path().to_path_buf()),
        ) {
            self.insert_relocation_alias(path, document_id);
        }
    }

    /// 新视图构造会推进租约；失败回滚必须恢复仍存活旧视图的写资格。
    fn renew_document_view_leases(&mut self, document_id: DocumentId, cx: &mut App) {
        let panels = self
            .projects
            .values()
            .flat_map(|project| {
                project
                    .read(cx)
                    .panels
                    .values()
                    .filter_map(|panel| {
                        let matches_document = match &panel.view_key {
                            slterm_workspace::ViewKey::Editor {
                                document_id: id, ..
                            }
                            | slterm_workspace::ViewKey::Markdown {
                                document_id: id, ..
                            }
                            | slterm_workspace::ViewKey::Diff {
                                document_id: id, ..
                            } => *id == document_id,
                            _ => false,
                        };
                        if !matches_document {
                            return None;
                        }
                        match &panel.content {
                            PanelContent::Editor(editor) => {
                                Some(ExternalPanel::Editor(editor.view.clone()))
                            }
                            PanelContent::Paged(view) => Some(ExternalPanel::Paged(view.clone())),
                            _ => None,
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        for panel in panels {
            match panel {
                ExternalPanel::Editor(view) => {
                    view.update(cx, |view, _| view.renew_view_lease());
                }
                ExternalPanel::Paged(view) => {
                    view.update(cx, |view, _| view.renew_view_lease());
                }
                ExternalPanel::Markdown(_) => {}
            }
        }
    }

    fn convert_external_panel_kinds(
        &mut self,
        document_id: DocumentId,
        paged: Option<&Arc<crate::app_shell::paged_document::PagedDocument>>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), super::commands::AppRootError> {
        let session = self
            .documents
            .session(document_id)
            .cloned()
            .ok_or(super::commands::AppRootError::MissingPanel)?;
        let mismatched = self
            .projects
            .values()
            .flat_map(|project| {
                project
                    .read(cx)
                    .panels
                    .values()
                    .filter_map(|panel| {
                        let project_id = panel.view_key.project_id()?;
                        let matches_document = match &panel.view_key {
                            slterm_workspace::ViewKey::Editor {
                                document_id: id, ..
                            }
                            | slterm_workspace::ViewKey::Markdown {
                                document_id: id, ..
                            }
                            | slterm_workspace::ViewKey::Diff {
                                document_id: id, ..
                            } => *id == document_id,
                            _ => false,
                        };
                        let mismatched = match &panel.content {
                            PanelContent::Editor(_) | PanelContent::Markdown(_) => {
                                session.paged().is_some()
                            }
                            PanelContent::Paged(_) => session.text().is_some(),
                            _ => false,
                        };
                        (matches_document && mismatched).then_some((
                            project_id,
                            panel.id,
                            panel.view_key.clone(),
                        ))
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let close_language_selector = self.language_selector_menu.as_ref().is_some_and(|menu| {
            mismatched
                .iter()
                .any(|(_, _, view_key)| *view_key == menu.view_key)
        });
        let mut prepared = Vec::new();
        for (project_id, panel_id, view_key) in mismatched {
            let (content, focus) =
                self.build_external_content(project_id, view_key, document_id, paged, window, cx)?;
            prepared.push((project_id, panel_id, content, focus));
        }

        // 先注册全部新焦点句柄；任何失败都只清理新注册，不动旧 Panel 内容。
        let mut registered = Vec::new();
        for (project_id, panel_id, content, focus) in prepared {
            let result = self.project_focus_token(project_id, cx).and_then(|parent| {
                self.focus_manager
                    .register(
                        &focus,
                        parent,
                        Some(project_id),
                        self.snapshot.registry().active() == Some(project_id),
                    )
                    .map_err(Into::into)
            });
            match result {
                Ok(registration) => registered.push((project_id, panel_id, content, registration)),
                Err(error) => {
                    for (_, _, _, registration) in registered.drain(..) {
                        let _ = self.focus_manager.unregister(registration.token());
                    }
                    return Err(error);
                }
            }
        }
        if close_language_selector && let Err(error) = self.close_language_selector_menu(window, cx)
        {
            for (_, _, _, registration) in registered.drain(..) {
                let _ = self.focus_manager.unregister(registration.token());
            }
            return Err(error.into());
        }
        for (project_id, panel_id, _, registration) in &registered {
            let old_focus = self
                .projects
                .get(project_id)
                .and_then(|project| project.read(cx).panels.get(panel_id))
                .map(|panel| panel.focus_handle(cx))
                .expect("内容转换前 Panel 必须存活");
            if window.focused(cx) == Some(old_focus)
                && let Err(error) =
                    self.focus_manager
                        .request_focus(registration.token(), window, cx)
            {
                for (_, _, _, registration) in registered.drain(..) {
                    let _ = self.focus_manager.unregister(registration.token());
                }
                return Err(error.into());
            }
        }
        for (project_id, panel_id, content, registration) in registered {
            let project = self
                .projects
                .get(&project_id)
                .cloned()
                .expect("内容转换前 ProjectView 必须存活");
            project.update(cx, |project, cx| {
                let panel = project
                    .panels
                    .get_mut(&panel_id)
                    .expect("已登记的 Panel 必须存活到内容转换");
                if let PanelContent::Paged(view) = &panel.content {
                    view.update(cx, |view, _| view.close());
                }
                panel.content = content;
            });
            if let Some(previous) = self.panel_focus.insert(panel_id, registration) {
                let _ = self.focus_manager.unregister(previous.token());
            }
        }
        Ok(())
    }

    fn apply_document_location_to_views(
        &mut self,
        document_id: DocumentId,
        outcome: ExternalApplyOutcome,
        paged: Option<Arc<crate::app_shell::paged_document::PagedDocument>>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), super::commands::AppRootError> {
        let Some(session) = self.documents.session(document_id).cloned() else {
            return Ok(());
        };
        if outcome == ExternalApplyOutcome::Reloaded {
            self.convert_external_panel_kinds(document_id, paged.as_ref(), window, cx)?;
        }
        if let Some(paged) = &paged {
            self.paged_documents.insert(document_id, paged.clone());
        }
        if outcome == ExternalApplyOutcome::Reloaded {
            if let Some(text) = session.text() {
                let revision = session.text_revision();
                for ((id, _), syntax) in &self.document_syntax {
                    if *id == document_id {
                        syntax.borrow_mut().rebuild(text.clone(), revision);
                    }
                }
            } else {
                self.document_syntax.retain(|(id, _), _| *id != document_id);
            }
        }
        let panels = self
            .projects
            .values()
            .flat_map(|project| {
                project
                    .read(cx)
                    .panels
                    .values()
                    .filter_map(|panel| {
                        let matches = match &panel.view_key {
                            slterm_workspace::ViewKey::Editor {
                                document_id: id, ..
                            }
                            | slterm_workspace::ViewKey::Markdown {
                                document_id: id, ..
                            }
                            | slterm_workspace::ViewKey::Diff {
                                document_id: id, ..
                            } => *id == document_id,
                            _ => false,
                        };
                        if !matches {
                            return None;
                        }
                        match &panel.content {
                            PanelContent::Editor(editor) => {
                                Some(ExternalPanel::Editor(editor.view.clone()))
                            }
                            PanelContent::Markdown(view) => {
                                Some(ExternalPanel::Markdown(view.clone()))
                            }
                            PanelContent::Paged(view) => Some(ExternalPanel::Paged(view.clone())),
                            _ => None,
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        for panel in panels {
            match panel {
                ExternalPanel::Editor(view) => {
                    if outcome == ExternalApplyOutcome::Reloaded {
                        view.update(cx, |view, cx| view.apply_snapshot(&session, cx));
                    }
                    let automatic = slterm_document::LanguageRegistry::new()
                        .for_path(session.location().canonical_path().as_path())
                        .unwrap_or(slterm_document::LanguageId::PlainText);
                    let effective = view.read(cx).language_override().unwrap_or(automatic);
                    let syntax = match self.document_syntax(document_id, effective) {
                        Ok(syntax) => syntax,
                        Err(error) => {
                            log::warn!("外部改名后的语法 worker 未建立: {error}");
                            None
                        }
                    };
                    view.update(cx, |view, cx| {
                        view.relocate_language(automatic, syntax, cx);
                    });
                }
                ExternalPanel::Markdown(view) if outcome == ExternalApplyOutcome::Reloaded => {
                    let grants = crate::app_shell::documents::document_project_grants(
                        &self.snapshot,
                        document_id,
                    );
                    let write = (!session.is_read_revoked()).then(|| {
                        crate::app_shell::file_editor::document::DocumentWriteContext::new(
                            grants,
                            0,
                            0,
                            Arc::new(crate::platform::document_paths::SystemDocumentPathResolver),
                        )
                    });
                    if let Some(document) =
                        crate::app_shell::file_editor::document::Document::from_recovered(
                            &session, write,
                        )
                    {
                        view.update(cx, |view, cx| {
                            view.apply_external_document(document, window, cx);
                        });
                    }
                }
                ExternalPanel::Markdown(view) => {
                    view.update(cx, |view, cx| {
                        view.relocate_document(session.location().clone(), window, cx);
                    });
                }
                ExternalPanel::Paged(view) => {
                    if let Some(paged) = &paged {
                        view.update(cx, |view, cx| {
                            if outcome == ExternalApplyOutcome::Reloaded {
                                let request = self
                                    .documents
                                    .text_request(document_id, 9)
                                    .expect("已提交的分页重载必须能建立文本请求");
                                view.apply_snapshot(&session, request, paged.page_reader(), cx);
                            } else {
                                view.relocate_reader(paged.page_reader(), cx);
                            }
                        });
                    }
                }
            }
        }
        Ok(())
    }
}

/// 收集实体后再更新，避免在 ProjectView 不可变读取期间重入可变更新。
enum ExternalPanel {
    Editor(Entity<crate::app_shell::text_view::DocumentTextView>),
    Markdown(Entity<crate::app_shell::file_editor::TextFileView>),
    Paged(Entity<crate::app_shell::text_view::PagedTextView>),
}
