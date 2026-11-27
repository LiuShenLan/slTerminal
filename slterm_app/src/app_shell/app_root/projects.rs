//! 项目元数据命令与真实路径解析后的打开链。

use gpui::{Context, Window};
use slterm_path::CanonicalPath;
use slterm_workspace::{AddProjectOutcome, ProjectId, ProjectSnapshot};
use uuid::Uuid;

use super::AppRoot;
use super::commands::AppRootError;
use super::task_service::{CancellationToken, TaskId};

type ProjectOpenCompletion = Box<
    dyn FnOnce(
        Result<ProjectId, AppRootError>,
        &mut AppRoot,
        Option<&mut Window>,
        &mut Context<'_, AppRoot>,
    ),
>;

pub(super) struct ProjectOpenRequest {
    id: Uuid,
    root: CanonicalPath,
    completion: ProjectOpenCompletion,
}

impl AppRoot {
    /// 项目根重定位的基础门禁：关闭／快照屏障与焦点预检。
    fn ensure_relocation_base_allowed(&self) -> Result<(), AppRootError> {
        self.ensure_not_closing()?;
        self.ensure_project_snapshot_command_allowed()?;
        self.ensure_project_snapshot_recovery_allowed()?;
        self.focus_manager.ensure_activation_allowed()?;
        Ok(())
    }

    /// 提交门禁额外要求 Hooks 草稿已由决定面处置。
    fn ensure_relocation_allowed(&self, project_id: ProjectId) -> Result<(), AppRootError> {
        self.ensure_relocation_base_allowed()?;
        if self.hook_drafts.project_has_drafts(project_id) {
            return Err(AppRootError::HooksDraftPending);
        }
        Ok(())
    }

    /// 只替换根关联并校验候选；PTY cwd、Document、Diff 与 GitShow 绑定一律不动。
    fn relocation_candidate(
        &self,
        project_id: ProjectId,
        new_root: CanonicalPath,
    ) -> Result<ProjectSnapshot, AppRootError> {
        let mut candidate = self.snapshot.clone();
        candidate
            .registry_mut()
            .relocate_project(project_id, new_root)?;
        candidate.validate()?;
        Ok(candidate)
    }

    /// 已选定真实目标后的重定位：有 writer 时必须等事务回执才采纳候选。
    pub(super) fn relocate_project_from_request(
        &mut self,
        project_id: ProjectId,
        new_root: CanonicalPath,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        self.ensure_relocation_allowed(project_id)?;
        let candidate = self.relocation_candidate(project_id, new_root)?;
        let Some(writer) = self.project_writer.clone() else {
            self.advance_authorization_generation();
            self.commit_project_snapshot(candidate, cx);
            cx.notify();
            return Ok(());
        };
        let response = writer
            .commit_project_snapshot(&candidate)
            .map_err(AppRootError::ProjectRelocationWriter)?;
        self.project_relocation_in_flight = Some(project_id);
        let task = TaskId::ProjectRelocation(project_id);
        let executor = cx.background_executor().clone();
        let registration = self
            .tasks
            .register_with(task, CancellationToken::new(), move || {
                let receipt = executor.spawn(async move { response.recv() });
                cx.spawn(async move |this, cx| {
                    let result = receipt.await;
                    cx.update(|cx| {
                        let _ = this.update(cx, |root, cx| {
                            root.finish_project_relocation_commit(
                                project_id, candidate, result, cx,
                            );
                        });
                    });
                })
            });
        if registration.is_err() {
            self.project_relocation_in_flight = None;
            log::warn!("项目根重定位回执任务未能登记");
            return Err(AppRootError::ProjectRelocationInFlight);
        }
        Ok(())
    }

    fn finish_project_relocation_commit(
        &mut self,
        project_id: ProjectId,
        candidate: ProjectSnapshot,
        result: Result<
            Result<(), crate::persistence::project::ProjectStoreError>,
            std::sync::mpsc::RecvError,
        >,
        cx: &mut Context<'_, Self>,
    ) {
        self.tasks.complete(TaskId::ProjectRelocation(project_id));
        self.project_relocation_in_flight = None;
        if result.is_ok_and(|inner| inner.is_ok()) {
            self.advance_authorization_generation();
            self.commit_project_snapshot(candidate, cx);
            cx.notify();
        } else {
            // 事务失败不改根关联，也不推进授权代次。
            log::warn!("项目根重定位事务失败或回执丢失");
            super::events::notify_project_relocation_failed(cx);
        }
    }

    /// 导航菜单入口：目录选择与真实解析都在后台，前台只接收候选根。
    pub(super) fn start_project_relocation(
        &mut self,
        project_id: ProjectId,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.ensure_relocation_base_allowed().is_err() {
            return;
        }
        let Some(current_root) = self
            .snapshot
            .registry()
            .project(project_id)
            .map(|project| project.root().clone())
        else {
            return;
        };
        let task = TaskId::ProjectRelocation(project_id);
        if self.tasks.is_registered(task) {
            return;
        }
        let windows = self.windows.clone();
        let resolver = self.project_paths.clone();
        let owner = crate::windowing::focus::window_hwnd(cx);
        let executor = cx.background_executor().clone();
        let window_handle = window.window_handle();
        let registration = self
            .tasks
            .register_with(task, CancellationToken::new(), move || {
                let resolved = executor.spawn(async move {
                    let Some(target) = windows.pick_folder(owner, Some(current_root.as_path()))?
                    else {
                        return Ok(None);
                    };
                    let lexical = CanonicalPath::new(target).map_err(|error| {
                        std::io::Error::new(std::io::ErrorKind::InvalidInput, error)
                    })?;
                    let resolved = resolver.resolve(&lexical)?;
                    Ok::<_, std::io::Error>(Some(resolved))
                });
                cx.spawn(async move |this, cx| {
                    let resolved = resolved.await;
                    let _ = window_handle.update(cx, |_, window, cx| {
                        let _ = this.update(cx, |root, cx| {
                            root.tasks.complete(task);
                            root.finish_project_relocation_target(project_id, resolved, window, cx);
                        });
                    });
                })
            });
        if registration.is_err() {
            log::debug!("项目根重定位选择任务未登记");
        }
    }

    fn finish_project_relocation_target(
        &mut self,
        project_id: ProjectId,
        resolved: std::io::Result<Option<CanonicalPath>>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.ensure_not_closing().is_err() || self.project_relocation_in_flight.is_some() {
            return;
        }
        let target = match resolved {
            Ok(Some(target)) => target,
            Ok(None) => return,
            Err(error) => {
                log::warn!("项目根目录选择或真实解析失败: {error}");
                super::events::notify_project_relocation_failed(cx);
                return;
            }
        };
        if self.snapshot.registry().project(project_id).is_none() {
            return;
        }
        if self.hook_drafts.project_has_drafts(project_id) {
            self.pending_project_relocation = Some((project_id, target));
            if let Err(error) = self.open_relocation_decision(project_id, window, cx) {
                self.pending_project_relocation = None;
                log::warn!("项目根重定位决定面打开失败: {error:?}");
                super::events::notify_project_relocation_failed(cx);
            }
            return;
        }
        if let Err(error) = self.relocate_project_from_request(project_id, target, cx) {
            log::warn!("项目根重定位请求失败: {error}");
            super::events::notify_project_relocation_failed(cx);
        }
    }

    /// Hooks 草稿处置面：重定位只允许丢弃或取消，保存仍须在 Hooks 页完成。
    fn open_relocation_decision(
        &mut self,
        project_id: ProjectId,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        self.open_close_decision(
            super::close_decision::CloseDecisionScope::RelocateProject { project_id },
            Vec::new(),
            Vec::new(),
            true,
            window,
            cx,
        )
    }

    pub(super) fn rename_project_from_request(
        &mut self,
        project_id: ProjectId,
        name: String,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        self.ensure_not_closing()?;
        self.ensure_project_snapshot_command_allowed()?;
        let mut candidate = self.snapshot.clone();
        candidate.registry_mut().rename_project(project_id, &name)?;
        candidate.validate()?;
        // 重命名不改变任何根路径，授权代次与在途保存都必须保持有效。
        self.commit_project_snapshot(candidate, cx);
        cx.notify();
        Ok(())
    }

    pub(super) fn move_project_from_request(
        &mut self,
        project_id: ProjectId,
        new_index: usize,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        self.ensure_not_closing()?;
        self.ensure_project_snapshot_command_allowed()?;
        let mut candidate = self.snapshot.clone();
        candidate
            .registry_mut()
            .move_project(project_id, new_index)?;
        candidate.validate()?;
        // 排序不改变任何根路径，授权代次与在途保存都必须保持有效。
        self.commit_project_snapshot(candidate, cx);
        cx.notify();
        Ok(())
    }

    /// 请求按进入顺序串行解析；无论预检、关闭或窗口失效，completion 都只结算一次。
    pub(crate) fn open_project_with_completion(
        &mut self,
        root: CanonicalPath,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
        completion: impl FnOnce(
            Result<ProjectId, AppRootError>,
            &mut Self,
            Option<&mut Window>,
            &mut Context<'_, Self>,
        ) + 'static,
    ) {
        let completion: ProjectOpenCompletion = Box::new(completion);
        if self.project_open_draining {
            completion(Err(AppRootError::Closing), self, Some(window), cx);
            return;
        }
        if let Err(error) = self
            .ensure_not_closing()
            .and_then(|()| self.ensure_project_snapshot_command_allowed())
            .and_then(|()| {
                self.focus_manager
                    .ensure_activation_allowed()
                    .map_err(Into::into)
            })
        {
            completion(Err(error), self, Some(window), cx);
            return;
        }
        self.pending_project_opens.push_back(ProjectOpenRequest {
            id: Uuid::now_v7(),
            root,
            completion,
        });
        self.start_next_project_open(Some(window), cx);
    }

    fn start_next_project_open(&mut self, window: Option<&mut Window>, cx: &mut Context<'_, Self>) {
        if self.active_project_open.is_some() {
            return;
        }
        if window.is_none() || self.ensure_not_closing().is_err() {
            self.cancel_pending_project_opens(cx);
            return;
        }
        let Some(request) = self.pending_project_opens.front() else {
            return;
        };
        let request_id = request.id;
        let path = request.root.clone();
        let window_handle = window.expect("可打开项目时窗口必须存活").window_handle();
        let resolver = self.project_paths.clone();
        let executor = cx.background_executor().clone();
        self.active_project_open = Some(request_id);
        self.tasks
            .register_with(
                TaskId::ProjectOpen(request_id),
                CancellationToken::new(),
                || {
                    let resolved = executor.spawn(async move { resolver.resolve(&path) });
                    cx.spawn(async move |root_entity, cx| {
                        let resolved = resolved.await;
                        cx.update(|cx| {
                            let mut resolved = Some(resolved);
                            let updated = window_handle.update(cx, |_, window, cx| {
                                let _ = root_entity.update(cx, |root, cx| {
                                    root.finish_project_open(
                                        request_id,
                                        resolved.take().expect("解析结果只能消费一次"),
                                        Some(window),
                                        cx,
                                    );
                                });
                            });
                            if updated.is_err() {
                                let _ = root_entity.update(cx, |root, cx| {
                                    root.finish_project_open(
                                        request_id,
                                        resolved.take().expect("失败窗口不得消费解析结果"),
                                        None,
                                        cx,
                                    );
                                });
                            }
                        });
                    })
                },
            )
            .expect("活动项目打开任务必须唯一");
    }

    fn finish_project_open(
        &mut self,
        request_id: Uuid,
        resolved: Result<CanonicalPath, std::io::Error>,
        mut window: Option<&mut Window>,
        cx: &mut Context<'_, Self>,
    ) {
        self.tasks.complete(TaskId::ProjectOpen(request_id));
        if self.active_project_open != Some(request_id) {
            return;
        }
        self.active_project_open = None;
        let request = self
            .pending_project_opens
            .pop_front()
            .expect("活动打开请求必须位于队首");
        let result = if self.ensure_not_closing().is_ok()
            && let Some(window) = window.as_deref_mut()
        {
            resolved
                .map_err(AppRootError::ProjectPath)
                .and_then(|path| self.open_resolved_project(path, window, cx))
        } else {
            Err(AppRootError::Closing)
        };
        (request.completion)(result, self, window.as_deref_mut(), cx);
        // completion 可以重入打开；若它已启动后继，此处不得重复启动同一队首。
        self.start_next_project_open(window, cx);
    }

    pub(super) fn cancel_pending_project_opens(&mut self, cx: &mut Context<'_, Self>) {
        if self.project_open_draining {
            return;
        }
        self.project_open_draining = true;
        self.active_project_open = None;
        while let Some(request) = self.pending_project_opens.pop_front() {
            self.tasks.cancel(TaskId::ProjectOpen(request.id));
            (request.completion)(Err(AppRootError::Closing), self, None, cx);
        }
        self.project_open_draining = false;
    }

    fn open_resolved_project(
        &mut self,
        root: CanonicalPath,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<ProjectId, AppRootError> {
        let requested = ProjectId::from_uuid(Uuid::now_v7()).expect("系统生成的 UUID 必须为 v7");
        self.add_project(requested, root, window, cx)
            .map(|outcome| match outcome {
                AddProjectOutcome::Added(project_id) | AddProjectOutcome::Existing(project_id) => {
                    project_id
                }
            })
    }

    pub(crate) fn open_initial_project(
        &mut self,
        root: CanonicalPath,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.open_project_with_completion(root, window, cx, |result, root, window, cx| match result
        {
            Ok(_) => {
                let shell = root.shell.clone();
                if let Some(window) = window
                    && let Err(error) = root.new_terminal(shell, window, cx)
                {
                    log::warn!("初始项目终端启动失败: {error}");
                }
            }
            Err(error) => log::warn!("初始项目真实路径解析或打开失败: {error}"),
        })
    }
}
