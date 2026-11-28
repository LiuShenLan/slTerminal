// Explorer 文件操作与后台状态读取：只编排共享服务与有界任务。

use gpui::{Context, Window};
use slterm_path::{CanonicalPath, PathGrant};
use slterm_workspace::{ProjectId, ProjectRootStatus, SideViewId};

use super::super::AppRoot;
use super::super::task_service::{CancellationToken, TaskId};
use super::*;
use crate::i18n::Message;

impl AppRoot {
    /// 实例生命周期或活跃项目根变化后同步 Explorer 根；根未变时零副作用。
    pub(crate) fn sync_side_view_roots(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let active = self.snapshot.registry().active();
        let target = active.and_then(|project_id| {
            let project = self.snapshot.registry().project(project_id)?;
            (project.root_status() == ProjectRootStatus::Ready)
                .then(|| (project_id, project.root().clone()))
        });
        let weak = cx.entity().downgrade();
        let instances = SideViewId::ALL
            .into_iter()
            .filter_map(|id| self.side_views.instance(id).map(|view| (id, view.clone())))
            .collect::<Vec<_>>();
        for (id, instance) in instances {
            instance.update(cx, |view, cx| {
                view.set_root_handle(weak.clone());
                match id {
                    SideViewId::Explorer => match &target {
                        Some((project_id, root)) => {
                            view.sync_explorer_root(*project_id, root.clone(), window, cx)
                        }
                        None => view.clear_explorer_root(cx),
                    },
                    SideViewId::Commit => match &target {
                        Some((project_id, root)) => {
                            view.sync_commit_root(*project_id, root.clone(), window, cx)
                        }
                        None => view.clear_commit_root(cx),
                    },
                    SideViewId::Nav | SideViewId::AgentFiles => {}
                }
            });
        }
        self.ensure_agent_roots_loading(cx);
        self.sync_agent_files_roots(window, cx);
        self.sync_nav_projects(cx);
        self.reconcile_fs_watch();
        self.side_view_roots_generation = Some(self.snapshot_generation);
    }

    /// Explorer 意图在广播结束后执行；打开失败只保持现状，不伪造面板。
    pub(super) fn handle_explorer_intent(
        &mut self,
        intent: ExplorerIntent,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        match intent {
            ExplorerIntent::OpenDocument { project_id, path } => {
                let target_group = match self.focused_group(project_id, cx) {
                    Ok(group) => group,
                    Err(_) => return,
                };
                let request =
                    match self.prepare_document_open(project_id, target_group, path, false) {
                        Ok(request) => request,
                        Err(_) => return,
                    };
                self.queue_document_open(request, None, window, cx);
            }
            ExplorerIntent::OpenTerminal {
                project_id,
                root,
                cwd,
            } => self.start_explorer_terminal_cwd(project_id, root, cwd, window, cx),
            ExplorerIntent::RenameFile {
                project_id,
                root,
                from,
                to,
            } => self.start_explorer_rename(project_id, root, from, to, window, cx),
            ExplorerIntent::CreateEntry {
                project_id,
                root,
                directory,
                name,
                directory_entry,
            } => self.start_explorer_create(project_id, root, directory, name, directory_entry, cx),
            ExplorerIntent::DeleteEntries {
                project_id,
                root,
                items,
                permanent,
            } => self.start_explorer_delete(project_id, root, items, permanent, cx),
            ExplorerIntent::PasteEntries {
                project_id,
                root,
                directory,
            } => self.start_explorer_paste(project_id, root, directory, cx),
            ExplorerIntent::LoadStatus {
                project_id,
                root,
                generation,
            } => self.start_explorer_status(project_id, root, generation, window, cx),
        }
    }

    /// 统一回投：刷新受影响目录并显示封闭提示。
    fn notify_explorer(
        &mut self,
        notice: Option<Message>,
        refresh: Vec<CanonicalPath>,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(view) = self
            .side_views
            .instance(slterm_workspace::SideViewId::Explorer)
            .cloned()
        else {
            return;
        };
        view.update(cx, |view, cx| {
            if let Some(runtime) = &mut view.explorer {
                runtime.notice = notice;
            }
            for directory in refresh {
                view.explorer_refresh_directory(&directory, cx);
            }
            cx.notify();
        });
    }

    fn explorer_write_grants(&self, project_id: ProjectId) -> Vec<PathGrant> {
        self.snapshot
            .registry()
            .project(project_id)
            .map(|project| vec![PathGrant::project_root(project.root().clone())])
            .unwrap_or_default()
    }

    /// 新建文件/文件夹：目标与父目录同属项目写根。
    fn start_explorer_create(
        &mut self,
        project_id: ProjectId,
        _root: CanonicalPath,
        directory: CanonicalPath,
        name: String,
        directory_entry: bool,
        cx: &mut Context<'_, Self>,
    ) {
        if self.tasks.is_registered(TaskId::ExplorerFileOperation) {
            return;
        }
        let Ok(target) = CanonicalPath::new(directory.as_path().join(&name)) else {
            self.notify_explorer(Some(Message::ExplorerInvalidName), Vec::new(), cx);
            return;
        };
        let grants = self.explorer_write_grants(project_id);
        if grants.is_empty() {
            self.notify_explorer(Some(Message::ExplorerOpFailed), Vec::new(), cx);
            return;
        }
        let service = std::sync::Arc::clone(&self.file_operations);
        let refresh = vec![directory.clone()];
        let task = cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let generation = crate::app_shell::authorization::current_generation();
                    let plan = if directory_entry {
                        service.plan_create_directory(
                            slterm_sidebar::FileTreeRoot::Project(project_id),
                            &target,
                            &grants,
                            generation,
                        )?
                    } else {
                        service.plan_create_file(
                            slterm_sidebar::FileTreeRoot::Project(project_id),
                            &target,
                            &grants,
                            generation,
                        )?
                    };
                    service.execute(
                        &plan,
                        &grants,
                        crate::app_shell::authorization::current_generation(),
                    )?;
                    Ok::<(), crate::app_shell::file_operation_service::FileOperationError>(())
                })
                .await;
            let _ = this.update(cx, |root, cx| {
                root.tasks.complete(TaskId::ExplorerFileOperation);
                let notice = result.err().map(|_| Message::ExplorerOpFailed);
                root.notify_explorer(notice, refresh, cx);
            });
        });
        if let Err(error) = self.tasks.register(
            TaskId::ExplorerFileOperation,
            CancellationToken::new(),
            task,
        ) {
            log::warn!("Explorer 新建任务注册失败: {error:?}");
        }
    }

    /// 删除逐项串行；回收站不可用时保留待永久确认集合并如实提示。
    fn start_explorer_delete(
        &mut self,
        project_id: ProjectId,
        _root: CanonicalPath,
        items: Vec<CanonicalPath>,
        permanent: bool,
        cx: &mut Context<'_, Self>,
    ) {
        if self.tasks.is_registered(TaskId::ExplorerFileOperation) {
            return;
        }
        let grants = self.explorer_write_grants(project_id);
        if grants.is_empty() {
            self.notify_explorer(Some(Message::ExplorerOpFailed), Vec::new(), cx);
            return;
        }
        let mut refresh = Vec::new();
        for path in &items {
            if let Some(parent) = path.as_path().parent()
                && let Ok(parent) = CanonicalPath::new(parent.to_path_buf())
                && !refresh.contains(&parent)
            {
                refresh.push(parent);
            }
        }
        let service = std::sync::Arc::clone(&self.file_operations);
        let items_for_pending = items.clone();
        let task = cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let mut recycle_unavailable = false;
                    let mut failures = 0;
                    for path in &items {
                        let generation = crate::app_shell::authorization::current_generation();
                        let plan = if permanent {
                            service.plan_delete_permanent(
                                slterm_sidebar::FileTreeRoot::Project(project_id),
                                path,
                                &grants,
                                generation,
                            )
                        } else {
                            service.plan_delete(
                                slterm_sidebar::FileTreeRoot::Project(project_id),
                                path,
                                &grants,
                                generation,
                            )
                        };
                        match plan {
                            Ok(plan) => match service.execute(
                                &plan,
                                &grants,
                                crate::app_shell::authorization::current_generation(),
                            ) {
                                Ok(_) => {}
                                Err(crate::app_shell::file_operation_service::FileOperationError::RecycleBin(_)) => {
                                    recycle_unavailable = true;
                                    failures += 1;
                                }
                                Err(_) => failures += 1,
                            },
                            Err(_) => failures += 1,
                        }
                    }
                    (failures, recycle_unavailable)
                })
                .await;
            let _ = this.update(cx, |root, cx| {
                root.tasks.complete(TaskId::ExplorerFileOperation);
                let (failures, recycle_unavailable) = result;
                if recycle_unavailable && !permanent {
                    let pending = items_for_pending.clone();
                    if let Some(view) = root
                        .side_views
                        .instance(slterm_workspace::SideViewId::Explorer)
                        .cloned()
                    {
                        view.update(cx, |view, cx| {
                            if let Some(runtime) = &mut view.explorer {
                                runtime.pending_permanent_delete = Some(pending);
                            }
                            cx.notify();
                        });
                    }
                }
                let notice = (failures > 0).then_some(Message::ExplorerOpPartial);
                root.notify_explorer(notice, refresh, cx);
            });
        });
        if let Err(error) = self.tasks.register(
            TaskId::ExplorerFileOperation,
            CancellationToken::new(),
            task,
        ) {
            log::warn!("Explorer 删除任务注册失败: {error:?}");
        }
        let _ = permanent;
    }

    /// 粘贴预检先聚合冲突；真实执行复用同一 PasteService。
    fn start_explorer_paste(
        &mut self,
        project_id: ProjectId,
        _root: CanonicalPath,
        directory: CanonicalPath,
        cx: &mut Context<'_, Self>,
    ) {
        if self.tasks.is_registered(TaskId::ExplorerPastePrepare)
            || self.tasks.is_registered(TaskId::ExplorerPasteExecute)
            || self.explorer_paste.is_some()
        {
            return;
        }
        let Some(clipboard) = self.file_clipboard.clone() else {
            self.notify_explorer(Some(Message::ExplorerClipboardEmpty), Vec::new(), cx);
            return;
        };
        let target = slterm_sidebar::PasteTarget {
            root: slterm_sidebar::FileTreeRoot::Project(project_id),
            directory: directory.clone(),
        };
        let plan = clipboard.plan_paste(target);
        if plan.items.is_empty() {
            self.notify_explorer(Some(Message::ExplorerOpFailed), Vec::new(), cx);
            return;
        }
        let mut source_dirs = Vec::new();
        let mut grants = self.explorer_write_grants(project_id);
        for item in &plan.items {
            grants.extend(self.agent_root_grants_for_path(&item.source.path));
            if let Some(parent) = item.source.path.as_path().parent()
                && let Ok(parent) = CanonicalPath::new(parent.to_path_buf())
                && !source_dirs.contains(&parent)
            {
                source_dirs.push(parent);
            }
        }
        let mode = plan.mode;
        let rejections = plan.rejections.len();
        let Some(service) = self.file_paste_service() else {
            self.notify_explorer(Some(Message::ExplorerOpFailed), Vec::new(), cx);
            return;
        };
        let prepare_grants = grants.clone();
        let task = cx.spawn(async move |this, cx| {
            let prepared = cx
                .background_executor()
                .spawn(async move {
                    service.prepare(
                        &plan,
                        &prepare_grants,
                        crate::app_shell::authorization::current_generation(),
                    )
                })
                .await;
            let _ = this.update(cx, |root, cx| {
                root.tasks.complete(TaskId::ExplorerPastePrepare);
                let pending = ExplorerPastePending {
                    prepared,
                    grants,
                    mode,
                    target_dir: directory,
                    source_dirs,
                    rejections,
                };
                if pending.prepared.conflicts().is_empty() {
                    root.start_explorer_paste_execute(
                        pending,
                        crate::app_shell::paste_service::PasteConflictDecision::Skip,
                        cx,
                    );
                } else {
                    root.explorer_paste = Some(pending);
                    cx.notify();
                }
            });
        });
        if let Err(error) =
            self.tasks
                .register(TaskId::ExplorerPastePrepare, CancellationToken::new(), task)
        {
            log::warn!("Explorer 粘贴预检任务注册失败: {error:?}");
        }
    }

    fn start_explorer_paste_execute(
        &mut self,
        pending: ExplorerPastePending,
        decision: crate::app_shell::paste_service::PasteConflictDecision,
        cx: &mut Context<'_, Self>,
    ) {
        if self.tasks.is_registered(TaskId::ExplorerPasteExecute) {
            return;
        }
        let Some(service) = self.file_paste_service() else {
            self.notify_explorer(Some(Message::ExplorerOpFailed), Vec::new(), cx);
            return;
        };
        let ExplorerPastePending {
            prepared,
            grants,
            mode,
            target_dir,
            source_dirs,
            rejections,
        } = pending;
        let mut refresh = source_dirs;
        if !refresh.contains(&target_dir) {
            refresh.push(target_dir);
        }
        let task = cx.spawn(async move |this, cx| {
            let report = cx
                .background_executor()
                .spawn(async move {
                    service.execute(
                        prepared,
                        decision,
                        &grants,
                        crate::app_shell::authorization::current_generation(),
                    )
                })
                .await;
            let _ = this.update(cx, |root, cx| {
                root.tasks.complete(TaskId::ExplorerPasteExecute);
                if report.cancelled {
                    return;
                }
                let mut failures = rejections;
                let mut remaining = Vec::new();
                for item in &report.items {
                    match &item.outcome {
                        crate::app_shell::paste_service::PasteItemOutcome::Failed(_)
                        | crate::app_shell::paste_service::PasteItemOutcome::NeedsAttention => {
                            failures += 1;
                        }
                        _ => {}
                    }
                    if !matches!(
                        item.outcome,
                        crate::app_shell::paste_service::PasteItemOutcome::Moved
                    ) {
                        remaining.push(item.item.source.clone());
                    }
                }
                remaining.extend(
                    report
                        .rejections
                        .iter()
                        .map(|rejection| rejection.source.clone()),
                );
                if mode == slterm_sidebar::FileClipboardMode::Cut {
                    root.file_clipboard = if remaining.is_empty() {
                        None
                    } else {
                        Some(slterm_sidebar::FileClipboard::new(
                            slterm_sidebar::FileClipboardMode::Cut,
                            remaining,
                        ))
                    };
                }
                let notice = (failures > 0).then_some(Message::ExplorerOpPartial);
                root.notify_explorer(notice, refresh, cx);
            });
        });
        if let Err(error) =
            self.tasks
                .register(TaskId::ExplorerPasteExecute, CancellationToken::new(), task)
        {
            log::warn!("Explorer 粘贴执行任务注册失败: {error:?}");
        }
    }

    pub(super) fn explorer_paste_decide(
        &mut self,
        decision: crate::app_shell::paste_service::PasteConflictDecision,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(pending) = self.explorer_paste.take() else {
            return;
        };
        self.start_explorer_paste_execute(pending, decision, cx);
        cx.notify();
    }

    pub(super) fn explorer_paste_cancel(&mut self, cx: &mut Context<'_, Self>) {
        self.explorer_paste = None;
        cx.notify();
    }

    /// 状态读取先发现父仓库根，再以 ProjectRoot 与只读 RepoRoot 并集授权。
    fn start_explorer_status(
        &mut self,
        project_id: ProjectId,
        root: CanonicalPath,
        generation: u64,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.tasks.is_registered(TaskId::ExplorerStatus) {
            if let Some(view) = self
                .side_views
                .instance(slterm_workspace::SideViewId::Explorer)
                .cloned()
            {
                view.update(cx, |view, _| view.explorer_defer_status(generation));
            }
            return;
        }
        let git = std::sync::Arc::clone(&self.git);
        let executor = cx.background_executor().clone();
        let window_handle = window.window_handle();
        let project_root = root.clone();
        self.tasks
            .register_with(
                TaskId::ExplorerStatus,
                CancellationToken::new(),
                move || {
                    let loading = executor.spawn(async move {
                        let grants = status_grants(&project_root)?;
                        let snapshot = git.status(&project_root, &grants, 0)?;
                        Ok::<_, crate::git::GitError>(snapshot)
                    });
                    cx.spawn(async move |this, cx| {
                        let result = loading.await;
                        let _ = window_handle.update(cx, |_, window, cx| {
                            let _ = this.update(cx, |root, cx| {
                                root.finish_explorer_status(
                                    project_id, generation, result, window, cx,
                                );
                            });
                        });
                    })
                },
            )
            .expect("ExplorerStatus 单任务登记必须唯一");
    }

    fn finish_explorer_status(
        &mut self,
        project_id: ProjectId,
        generation: u64,
        result: Result<std::sync::Arc<crate::git::StatusSnapshot>, crate::git::GitError>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.tasks.complete(TaskId::ExplorerStatus);
        let explorer_view = self
            .side_views
            .instance(slterm_workspace::SideViewId::Explorer)
            .cloned();
        let reload = explorer_view
            .as_ref()
            .is_some_and(|view| view.update(cx, |view, _| view.explorer_take_deferred_status()));
        let current = explorer_view
            .as_ref()
            .and_then(|view| view.read(cx).explorer_status_target());
        // 回执必须同时匹配当前项目、根与代次；旧根结果不得写入新视图。
        let matches = current
            .as_ref()
            .is_some_and(|(project, _, current_generation)| {
                *project == project_id && *current_generation == generation
            });
        if matches
            && let Ok(snapshot) = result
            && let Some(view) = explorer_view.as_ref()
        {
            let repo_root = snapshot
                .worktree_root
                .as_ref()
                .map(|root| root.canonical_path().clone());
            let entries = repo_root
                .as_ref()
                .map(|root| {
                    snapshot
                        .entries
                        .iter()
                        .filter_map(|entry| {
                            let path = CanonicalPath::new(
                                root.as_path().join(
                                    entry
                                        .path
                                        .trim_matches('/')
                                        .replace('/', std::path::MAIN_SEPARATOR_STR),
                                ),
                            )
                            .ok()?;
                            Some((path, explorer_status_label(entry), entry.ignored))
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            view.update(cx, |view, cx| {
                view.explorer_set_status(repo_root, entries, cx)
            });
        }
        if reload
            && let Some((project, root, generation)) = current
            && self.snapshot.registry().project(project).is_some()
        {
            self.start_explorer_status(project, root, generation, window, cx);
        }
    }

    /// 重命名在后台重新解析、授权并执行；成功后只重读原父目录。
    fn start_explorer_rename(
        &mut self,
        project_id: ProjectId,
        root: CanonicalPath,
        from: CanonicalPath,
        to: CanonicalPath,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.tasks.is_registered(TaskId::ExplorerFileOperation) {
            return;
        }
        let service = std::sync::Arc::clone(&self.file_operations);
        let executor = cx.background_executor().clone();
        let window_handle = window.window_handle();
        let task_from = from.clone();
        let task_to = to.clone();
        self.tasks
            .register_with(
                TaskId::ExplorerFileOperation,
                CancellationToken::new(),
                move || {
                    let executing = executor.spawn(async move {
                        let grants = [PathGrant::project_root(root.clone())];
                        let generation = crate::app_shell::authorization::current_generation();
                        let plan = service.plan_rename(
                            FileTreeRoot::Project(project_id),
                            &task_from,
                            &task_to,
                            &grants,
                            generation,
                        )?;
                        service.execute(
                            &plan,
                            &grants,
                            crate::app_shell::authorization::current_generation(),
                        )
                    });
                    cx.spawn(async move |this, cx| {
                        let result = executing.await;
                        let _ = window_handle.update(cx, |_, window, cx| {
                            let _ = this.update(cx, |root, cx| {
                                root.finish_explorer_rename(
                                    project_id, from, to, result, window, cx,
                                );
                            });
                        });
                    })
                },
            )
            .expect("ExplorerFileOperation 单任务登记必须唯一");
    }

    fn finish_explorer_rename(
        &mut self,
        project_id: ProjectId,
        from: CanonicalPath,
        to: CanonicalPath,
        result: Result<
            crate::app_shell::file_operation_service::FileOperationOutcome,
            crate::app_shell::file_operation_service::FileOperationError,
        >,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.tasks.complete(TaskId::ExplorerFileOperation);
        let _ = window;
        if result.is_err() {
            return;
        }
        if self.snapshot.registry().project(project_id).is_none() {
            return;
        }
        let Some(parent) = from.as_path().parent().map(|parent| parent.to_path_buf()) else {
            return;
        };
        let Ok(parent) = CanonicalPath::new(parent) else {
            return;
        };
        if let Some(explorer) = self
            .side_views
            .instance(slterm_workspace::SideViewId::Explorer)
            .cloned()
        {
            explorer.update(cx, |view, cx| view.explorer_refresh_directory(&parent, cx));
        }
        let _ = to;
    }

    /// cwd 预检只在后台解析真实目录与授权，回前台再复核项目身份。
    fn start_explorer_terminal_cwd(
        &mut self,
        project_id: ProjectId,
        root: CanonicalPath,
        cwd: CanonicalPath,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.tasks.is_registered(TaskId::ExplorerTerminalCwd) {
            return;
        }
        let resolver = std::sync::Arc::clone(&self.path_resolver);
        let executor = cx.background_executor().clone();
        let window_handle = window.window_handle();
        let requested_root = root.clone();
        let verify_root = root.clone();
        let verify_cwd = cwd.clone();
        self.tasks
            .register_with(
                TaskId::ExplorerTerminalCwd,
                CancellationToken::new(),
                move || {
                    let verifying = executor.spawn(async move {
                        verify_terminal_cwd(&resolver, &verify_root, &verify_cwd)
                    });
                    cx.spawn(async move |this, cx| {
                        let result = verifying.await;
                        let _ = window_handle.update(cx, |_, window, cx| {
                            let _ = this.update(cx, |root, cx| {
                                root.finish_explorer_terminal_cwd(
                                    project_id,
                                    requested_root,
                                    result,
                                    window,
                                    cx,
                                );
                            });
                        });
                    })
                },
            )
            .expect("ExplorerTerminalCwd 单任务登记必须唯一");
    }

    fn finish_explorer_terminal_cwd(
        &mut self,
        project_id: ProjectId,
        requested_root: CanonicalPath,
        result: Result<CanonicalPath, ExplorerCwdError>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.tasks.complete(TaskId::ExplorerTerminalCwd);
        let Ok(cwd) = result else {
            return;
        };
        // 解析期间项目关闭或根重定位时授权不再成立。
        let Some(project) = self.snapshot.registry().project(project_id) else {
            return;
        };
        if project.root() != &requested_root
            || self.snapshot.registry().active() != Some(project_id)
        {
            return;
        }
        let _ = self.new_terminal_from_request(
            self.shell.clone(),
            Some(requested_root),
            Some(cwd),
            window,
            cx,
        );
    }
}
