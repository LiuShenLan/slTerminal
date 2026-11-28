//! Agent 文件树操作：复用 `FileOperationService`、`PasteService`、授权代次与
//! AppRoot 意图链；所有阻塞 I/O 只在后台执行体运行。
//!
//! 树实体只产生已解析意图，菜单不构成授权；每次计划与执行都重新判定
//! Agent 根能力并读取最新授权代次。

use std::sync::Arc;

use gpui::{
    AnyElement, Context, InteractiveElement as _, IntoElement, MouseButton, ParentElement as _,
    Styled as _, Window, div, px,
};
use gpui_component::ActiveTheme as _;
use slterm_path::{CanonicalPath, PathGrant, PathOperation, PathPolicy, PathRequest};
use slterm_settings::AgentRootId;
use slterm_sidebar::{FileClipboardMode, FileTreeRoot, PasteTarget};
use slterm_workspace::{NewTerminalSpec, PanelGroupId, SideViewId, TerminalId};

use super::AppRoot;
use super::agent_files::AgentFilesIntent;
use super::task_service::{CancellationToken, TaskId};
use crate::app_shell::paste_service::{PasteConflictDecision, PasteService, PreparedPaste};
use crate::i18n::Message;

/// 已预检、等待冲突决定的粘贴；取消时零修改。
pub(super) struct AgentFilesPastePending {
    prepared: PreparedPaste,
    grants: Vec<PathGrant>,
    mode: FileClipboardMode,
    target_dir: CanonicalPath,
    source_dirs: Vec<CanonicalPath>,
    rejections: usize,
}

impl AppRoot {
    /// Agent 树意图统一入口：菜单已做展示裁量，这里重新判定真实能力。
    pub(super) fn handle_agent_files_intent(
        &mut self,
        intent: AgentFilesIntent,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        match intent {
            AgentFilesIntent::CreateFile {
                root_id,
                directory,
                name,
            } => self.start_agent_files_create(root_id, directory, name, false, cx),
            AgentFilesIntent::CreateFolder {
                root_id,
                directory,
                name,
            } => self.start_agent_files_create(root_id, directory, name, true, cx),
            AgentFilesIntent::Rename { root_id, from, to } => {
                self.start_agent_files_rename(root_id, from, to, cx)
            }
            AgentFilesIntent::Delete { items } => self.start_agent_files_delete(items, cx),
            AgentFilesIntent::Paste { root_id, directory } => {
                self.start_agent_files_paste(root_id, directory, cx)
            }
            AgentFilesIntent::OpenTerminal { root_id, cwd } => {
                self.start_agent_files_terminal(root_id, cwd, window, cx)
            }
        }
    }

    /// 目标与父目录同时纳入授权：新建只要求目标根的 Write。
    fn agent_operation_grants(&self, paths: &[&CanonicalPath]) -> Vec<PathGrant> {
        let mut grants = Vec::new();
        for path in paths {
            for grant in self.agent_root_grants_for_path(path) {
                if !grants.contains(&grant) {
                    grants.push(grant);
                }
            }
        }
        grants
    }

    fn start_agent_files_create(
        &mut self,
        root_id: AgentRootId,
        directory: CanonicalPath,
        name: String,
        directory_entry: bool,
        cx: &mut Context<'_, Self>,
    ) {
        if self.tasks.is_registered(TaskId::AgentFilesOperation) {
            return;
        }
        let target = match CanonicalPath::new(directory.as_path().join(&name)) {
            Ok(target) => target,
            Err(_) => {
                self.notify_agent_files(Some(Message::SidebarAgentInvalidName), Vec::new(), cx);
                return;
            }
        };
        let grants = self.agent_operation_grants(&[&directory, &target]);
        if grants.is_empty() {
            self.notify_agent_files(Some(Message::SidebarAgentMenuWriteDenied), Vec::new(), cx);
            return;
        }
        let service = Arc::clone(&self.file_operations);
        let refresh = vec![directory.clone(), target.clone()];
        let task = cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let generation = crate::app_shell::authorization::current_generation();
                    let plan = if directory_entry {
                        service.plan_create_directory(
                            FileTreeRoot::Agent(root_id),
                            &target,
                            &grants,
                            generation,
                        )?
                    } else {
                        service.plan_create_file(
                            FileTreeRoot::Agent(root_id),
                            &target,
                            &grants,
                            generation,
                        )?
                    };
                    service.execute(
                        &plan,
                        &grants,
                        crate::app_shell::authorization::current_generation(),
                    )
                })
                .await;
            let _ = this.update(cx, |root, cx| {
                root.tasks.complete(TaskId::AgentFilesOperation);
                let notice = result.err().map(|_| Message::SidebarAgentOpFailed);
                root.notify_agent_files(notice, refresh, cx);
            });
        });
        if let Err(error) =
            self.tasks
                .register(TaskId::AgentFilesOperation, CancellationToken::new(), task)
        {
            log::warn!("Agent 文件操作任务注册失败: {error:?}");
        }
    }

    fn start_agent_files_rename(
        &mut self,
        root_id: AgentRootId,
        from: CanonicalPath,
        to: CanonicalPath,
        cx: &mut Context<'_, Self>,
    ) {
        if self.tasks.is_registered(TaskId::AgentFilesOperation) {
            return;
        }
        let grants = self.agent_operation_grants(&[&from, &to]);
        if grants.is_empty() {
            self.notify_agent_files(Some(Message::SidebarAgentMenuWriteDenied), Vec::new(), cx);
            return;
        }
        let service = Arc::clone(&self.file_operations);
        let mut refresh = vec![to.clone()];
        for path in [&from, &to] {
            if let Some(parent) = path.as_path().parent()
                && let Ok(parent) = CanonicalPath::new(parent.to_path_buf())
                && !refresh.contains(&parent)
            {
                refresh.push(parent);
            }
        }
        let task = cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let generation = crate::app_shell::authorization::current_generation();
                    let plan = service.plan_rename(
                        FileTreeRoot::Agent(root_id),
                        &from,
                        &to,
                        &grants,
                        generation,
                    )?;
                    service.execute(
                        &plan,
                        &grants,
                        crate::app_shell::authorization::current_generation(),
                    )
                })
                .await;
            let _ = this.update(cx, |root, cx| {
                root.tasks.complete(TaskId::AgentFilesOperation);
                let notice = result.err().map(|_| Message::SidebarAgentOpFailed);
                root.notify_agent_files(notice, refresh, cx);
            });
        });
        if let Err(error) =
            self.tasks
                .register(TaskId::AgentFilesOperation, CancellationToken::new(), task)
        {
            log::warn!("Agent 文件操作任务注册失败: {error:?}");
        }
    }

    /// 批量删除逐项串行；成功项不回滚，失败项汇总但保留现场。
    fn start_agent_files_delete(
        &mut self,
        items: Vec<(AgentRootId, CanonicalPath)>,
        cx: &mut Context<'_, Self>,
    ) {
        if self.tasks.is_registered(TaskId::AgentFilesOperation) {
            return;
        }
        let service = Arc::clone(&self.file_operations);
        let mut refresh = Vec::new();
        let mut plans = Vec::new();
        let mut rejected = 0;
        for (root_id, path) in items {
            let grants = self.agent_operation_grants(&[&path]);
            if grants.is_empty() {
                // 多选跨根时逐项判定；无权项计入部分失败，不静默当作成功。
                rejected += 1;
                continue;
            }
            plans.push((root_id, path.clone(), grants));
            if let Some(parent) = path.as_path().parent()
                && let Ok(parent) = CanonicalPath::new(parent.to_path_buf())
                && !refresh.contains(&parent)
            {
                refresh.push(parent);
            }
        }
        if plans.is_empty() {
            self.notify_agent_files(Some(Message::SidebarAgentMenuWriteDenied), refresh, cx);
            return;
        }
        let task = cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let mut failures = rejected;
                    for (root_id, path, grants) in plans {
                        let generation = crate::app_shell::authorization::current_generation();
                        let plan = match service.plan_delete(
                            FileTreeRoot::Agent(root_id),
                            &path,
                            &grants,
                            generation,
                        ) {
                            Ok(plan) => plan,
                            Err(_) => {
                                failures += 1;
                                continue;
                            }
                        };
                        if service
                            .execute(
                                &plan,
                                &grants,
                                crate::app_shell::authorization::current_generation(),
                            )
                            .is_err()
                        {
                            failures += 1;
                        }
                    }
                    failures
                })
                .await;
            let _ = this.update(cx, |root, cx| {
                root.tasks.complete(TaskId::AgentFilesOperation);
                let notice = (result > 0).then_some(Message::SidebarAgentOpPartial);
                root.notify_agent_files(notice, refresh, cx);
            });
        });
        if let Err(error) =
            self.tasks
                .register(TaskId::AgentFilesOperation, CancellationToken::new(), task)
        {
            log::warn!("Agent 文件操作任务注册失败: {error:?}");
        }
    }

    /// 粘贴先做零写入预检；有冲突时等待用户决定，取消零修改。
    fn start_agent_files_paste(
        &mut self,
        root_id: AgentRootId,
        directory: CanonicalPath,
        cx: &mut Context<'_, Self>,
    ) {
        // 粘贴预检、待决冲突与执行共享同一闸门，保证多次粘贴串行。
        if self.tasks.is_registered(TaskId::AgentFilesPastePrepare)
            || self.tasks.is_registered(TaskId::AgentFilesPasteExecute)
            || self.agent_files_paste.is_some()
        {
            return;
        }
        let Some(clipboard) = self.file_clipboard.clone() else {
            self.notify_agent_files(Some(Message::SidebarAgentClipboardEmpty), Vec::new(), cx);
            return;
        };
        let target = PasteTarget {
            root: FileTreeRoot::Agent(root_id),
            directory: directory.clone(),
        };
        let plan = clipboard.plan_paste(target);
        if plan.items.is_empty() {
            self.notify_agent_files(Some(Message::SidebarAgentOpFailed), Vec::new(), cx);
            return;
        }
        let mut source_dirs = Vec::new();
        let mut paths: Vec<&CanonicalPath> = Vec::new();
        for item in &plan.items {
            paths.push(&item.source.path);
            paths.push(&item.target);
            if let Some(parent) = item.source.path.as_path().parent()
                && let Ok(parent) = CanonicalPath::new(parent.to_path_buf())
                && !source_dirs.contains(&parent)
            {
                source_dirs.push(parent);
            }
        }
        let grants = self.agent_operation_grants(&paths);
        let mode = plan.mode;
        let rejections = plan.rejections.len();
        let Some(service) = self.file_paste_service() else {
            self.notify_agent_files(Some(Message::SidebarAgentOpFailed), Vec::new(), cx);
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
                root.tasks.complete(TaskId::AgentFilesPastePrepare);
                let pending = AgentFilesPastePending {
                    prepared,
                    grants,
                    mode,
                    target_dir: directory,
                    source_dirs,
                    rejections,
                };
                if pending.prepared.conflicts().is_empty() {
                    root.start_agent_files_paste_execute(pending, PasteConflictDecision::Skip, cx);
                } else {
                    root.agent_files_paste = Some(pending);
                    cx.notify();
                }
            });
        });
        if let Err(error) = self.tasks.register(
            TaskId::AgentFilesPastePrepare,
            CancellationToken::new(),
            task,
        ) {
            log::warn!("Agent 粘贴预检任务注册失败: {error:?}");
        }
    }

    /// Explorer 与 Agent 文件树共用的粘贴执行体；Journal 只在有 writer 时启用。
    pub(super) fn file_paste_service(&self) -> Option<PasteService> {
        let operations = (*self.file_operations).clone();
        match (&self.project_writer, &self.store_epoch) {
            (Some(writer), Some(epoch)) if !epoch.is_empty() => {
                let journal: Arc<dyn crate::app_shell::paste_journal::PasteJournal> =
                    Arc::new(writer.clone());
                Some(PasteService::new(operations, journal, epoch.clone()))
            }
            _ => {
                #[cfg(test)]
                {
                    Some(PasteService::without_journal(operations))
                }
                #[cfg(not(test))]
                {
                    let _ = operations;
                    None
                }
            }
        }
    }

    fn start_agent_files_paste_execute(
        &mut self,
        pending: AgentFilesPastePending,
        decision: PasteConflictDecision,
        cx: &mut Context<'_, Self>,
    ) {
        if self.tasks.is_registered(TaskId::AgentFilesPasteExecute) {
            return;
        }
        let Some(service) = self.file_paste_service() else {
            self.notify_agent_files(Some(Message::SidebarAgentOpFailed), Vec::new(), cx);
            return;
        };
        let AgentFilesPastePending {
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
                root.tasks.complete(TaskId::AgentFilesPasteExecute);
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
                        // 未移动的剪切项保留剪切态，便于用户修正后重试。
                        remaining.push(item.item.source.clone());
                    }
                }
                remaining.extend(
                    report
                        .rejections
                        .iter()
                        .map(|rejection| rejection.source.clone()),
                );
                if mode == FileClipboardMode::Cut {
                    root.file_clipboard = if remaining.is_empty() {
                        None
                    } else {
                        Some(slterm_sidebar::FileClipboard::new(
                            FileClipboardMode::Cut,
                            remaining,
                        ))
                    };
                }
                let notice = (failures > 0).then_some(Message::SidebarAgentOpPartial);
                root.notify_agent_files(notice, refresh, cx);
            });
        });
        if let Err(error) = self.tasks.register(
            TaskId::AgentFilesPasteExecute,
            CancellationToken::new(),
            task,
        ) {
            log::warn!("Agent 粘贴执行任务注册失败: {error:?}");
        }
    }

    /// 冲突决定：取消零修改；跳过与替换都复用同一逐项执行体。
    pub(super) fn agent_files_paste_decide(
        &mut self,
        decision: PasteConflictDecision,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(pending) = self.agent_files_paste.take() else {
            return;
        };
        self.start_agent_files_paste_execute(pending, decision, cx);
        cx.notify();
    }

    pub(super) fn agent_files_paste_cancel(&mut self, cx: &mut Context<'_, Self>) {
        self.agent_files_paste = None;
        cx.notify();
    }

    /// 覆盖该 cwd 且显式允许终端的自定义根真实目录。
    fn agent_terminal_grant_root(&self, cwd: &CanonicalPath) -> Option<CanonicalPath> {
        let registry = self.agent_root_registry.as_ref()?;
        registry
            .custom_roots_with_real()
            .find(|(root, real)| {
                root.capabilities().allows_terminal() && cwd.as_path().starts_with(real.as_path())
            })
            .map(|(_, real)| real.clone())
    }

    fn start_agent_files_terminal(
        &mut self,
        _root_id: AgentRootId,
        cwd: CanonicalPath,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.tasks.is_registered(TaskId::AgentFilesTerminalCwd) {
            return;
        }
        let Some(grant_root) = self.agent_terminal_grant_root(&cwd) else {
            self.notify_agent_files(
                Some(Message::SidebarAgentMenuTerminalDenied),
                Vec::new(),
                cx,
            );
            return;
        };
        let grants = self.agent_root_grants_for_path(&cwd);
        let resolver = Arc::clone(&self.path_resolver);
        let window_handle = window.window_handle();
        let verify_cwd = cwd.clone();
        let registration = self.tasks.register_with(
            TaskId::AgentFilesTerminalCwd,
            CancellationToken::new(),
            move || {
                let verifying = cx.background_executor().spawn(async move {
                    let resolved = resolver.resolve(&verify_cwd).map_err(|_| ())?;
                    let request = PathRequest {
                        operation: PathOperation::TerminalCwd,
                        lexical_path: verify_cwd.as_path().to_path_buf(),
                        root_hints: Vec::new(),
                        session_grant: None,
                        expected_authorization_generation:
                            crate::app_shell::authorization::current_generation(),
                    };
                    PathPolicy::authorize(
                        &request,
                        &resolved,
                        &grants,
                        crate::app_shell::authorization::current_generation(),
                    )
                    .map_err(|_| ())?;
                    Ok::<CanonicalPath, ()>(resolved.canonical_path().clone())
                });
                cx.spawn(async move |this, cx| {
                    let result = verifying.await;
                    let _ = window_handle.update(cx, |_, window, cx| {
                        let _ = this.update(cx, |root, cx| {
                            root.tasks.complete(TaskId::AgentFilesTerminalCwd);
                            match result {
                                Ok(real) => {
                                    root.finish_agent_files_terminal(grant_root, real, window, cx)
                                }
                                Err(()) => root.notify_agent_files(
                                    Some(Message::SidebarAgentMenuTerminalDenied),
                                    Vec::new(),
                                    cx,
                                ),
                            }
                        });
                    });
                })
            },
        );
        if let Err(error) = registration {
            log::warn!("Agent 终端 cwd 任务注册失败: {error:?}");
        }
    }

    fn finish_agent_files_terminal(
        &mut self,
        grant_root: CanonicalPath,
        cwd: CanonicalPath,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        // 复核终端能力与真实根仍在；失权时不回退项目根。
        if self.agent_terminal_grant_root(&cwd).as_ref() != Some(&grant_root) {
            self.notify_agent_files(
                Some(Message::SidebarAgentMenuTerminalDenied),
                Vec::new(),
                cx,
            );
            return;
        }
        let Ok(project_id) = self.active_project() else {
            self.notify_agent_files(Some(Message::SidebarAgentNoProject), Vec::new(), cx);
            return;
        };
        let target_group = match self.focused_group(project_id, cx) {
            Ok(Some(group)) => group,
            Ok(None) => {
                PanelGroupId::from_uuid(uuid::Uuid::now_v7()).expect("系统生成的 UUID 必须为 v7")
            }
            Err(_) => return,
        };
        let terminal_id =
            TerminalId::from_uuid(uuid::Uuid::now_v7()).expect("系统生成的 UUID 必须为 v7");
        let spec = NewTerminalSpec {
            project_id,
            terminal_id,
            target_group,
            shell: self.shell.clone(),
            cwd: Some(cwd),
            cwd_root_grant: Some(grant_root),
            custom_title: None,
        };
        let notice = self
            .new_terminal_with_identity(spec, window, cx)
            .err()
            .map(|_| Message::SidebarAgentOpFailed);
        self.notify_agent_files(notice, Vec::new(), cx);
    }

    /// 统一回投：刷新受影响目录并显示封闭提示；视图不存在时无副作用。
    fn notify_agent_files(
        &mut self,
        notice: Option<Message>,
        refresh: Vec<CanonicalPath>,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(view) = self.side_views.instance(SideViewId::AgentFiles).cloned() else {
            return;
        };
        view.update(cx, |view, cx| {
            if !refresh.is_empty() {
                view.agent_files_refresh_paths(&refresh, cx);
            }
            view.agent_files_set_notice(notice, cx);
        });
    }
}

/// 粘贴冲突决定面；只有存在冲突预检结果时出现。
pub(super) fn render_agent_files_paste_dialog(
    root: &AppRoot,
    cx: &mut Context<'_, AppRoot>,
) -> Option<AnyElement> {
    let pending = root.agent_files_paste.as_ref()?;
    let language = cx.global::<crate::app_shell::config::Settings>().language;
    let conflicts = pending.prepared.conflicts().len().to_string();
    Some(
        div()
            .absolute()
            .inset_0()
            .occlude()
            .bg(cx.theme().background.opacity(0.7))
            .debug_selector(|| "agent-files-paste-dialog".to_owned())
            .child(
                div()
                    .absolute()
                    .top(px(90.0))
                    .left_0()
                    .right_0()
                    .flex()
                    .justify_center()
                    .child(
                        div()
                            .w(px(520.0))
                            .p_4()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .bg(cx.theme().background)
                            .border_1()
                            .border_color(cx.theme().border)
                            .text_color(cx.theme().foreground)
                            .child(
                                div()
                                    .text_sm()
                                    .child(language.text(Message::SidebarAgentPasteConflictTitle)),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(language.text(Message::SidebarAgentPasteConflictBody)),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(conflicts),
                            )
                            .child(
                                div()
                                    .flex()
                                    .justify_end()
                                    .gap_3()
                                    .child(
                                        div()
                                            .id("agent-files-paste-cancel")
                                            .debug_selector(|| {
                                                "agent-files-paste-cancel".to_owned()
                                            })
                                            .px_1()
                                            .text_xs()
                                            .cursor_pointer()
                                            .text_color(cx.theme().foreground)
                                            .child(language.text(Message::SidebarAgentPasteCancel))
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                cx.listener(|root, _, _, cx| {
                                                    root.agent_files_paste_cancel(cx);
                                                }),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .id("agent-files-paste-skip")
                                            .debug_selector(|| "agent-files-paste-skip".to_owned())
                                            .px_1()
                                            .text_xs()
                                            .cursor_pointer()
                                            .text_color(cx.theme().accent)
                                            .child(language.text(Message::SidebarAgentPasteSkip))
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                cx.listener(|root, _, _, cx| {
                                                    root.agent_files_paste_decide(
                                                        PasteConflictDecision::Skip,
                                                        cx,
                                                    );
                                                }),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .id("agent-files-paste-replace")
                                            .debug_selector(|| {
                                                "agent-files-paste-replace".to_owned()
                                            })
                                            .px_1()
                                            .text_xs()
                                            .cursor_pointer()
                                            .text_color(cx.theme().danger)
                                            .child(language.text(Message::SidebarAgentPasteReplace))
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                cx.listener(|root, _, _, cx| {
                                                    root.agent_files_paste_decide(
                                                        PasteConflictDecision::Replace,
                                                        cx,
                                                    );
                                                }),
                                            ),
                                    ),
                            ),
                    ),
            )
            .into_any_element(),
    )
}
