//! 项目元数据命令与真实路径解析后的打开链。

use gpui::{Context, Window};
use slterm_path::CanonicalPath;
use slterm_workspace::{AddProjectOutcome, ProjectId};
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
