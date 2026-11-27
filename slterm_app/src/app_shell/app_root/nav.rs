//! 导航视图：项目列表的显示、激活与移除入口。
//!
//! 激活与移除只派发到 AppRoot 的统一命令链（`activate_project`／
//! `remove_project`），不在视图内复制守卫、布局或关闭逻辑。

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, Context, DismissEvent, Focusable as _, InteractiveElement as _, IntoElement,
    MouseButton, ParentElement as _, StatefulInteractiveElement as _, Styled as _, Window,
    anchored, deferred, div, point, px,
};
use gpui_component::ActiveTheme as _;
use gpui_component::menu::{PopupMenu, PopupMenuItem};
use slterm_path::CanonicalPath;
use slterm_workspace::ProjectId;

use super::AppRoot;
use super::navigation::{self, deepest_project_for_cwd};
use super::side_view::SideView;
use super::task_service::{CancellationToken, TaskId};
use crate::i18n::Message;

pub(super) struct NavProjectRow {
    pub(super) id: ProjectId,
    pub(super) display_name: String,
    pub(super) active: bool,
}

/// 导航历史行：cwd 归属与恢复命令在扫描回投时确定，不进入持久状态。
#[derive(Clone, Debug)]
pub(super) struct NavHistoryRow {
    pub(super) title: String,
    pub(super) cli: String,
    pub(super) project: Option<ProjectId>,
    pub(super) project_name: String,
    pub(super) cwd: Option<CanonicalPath>,
    pub(super) resume: Option<String>,
    pub(super) modified: std::time::SystemTime,
}

pub(super) struct NavRuntime {
    pub(super) projects: Vec<NavProjectRow>,
    sessions: Vec<NavHistoryRow>,
}

impl NavRuntime {
    pub(super) fn new() -> Self {
        Self {
            projects: Vec::new(),
            sessions: Vec::new(),
        }
    }
}

/// 项目行 + 移除按钮；点击项目激活，点击 × 走统一移除命令。
pub(super) fn render_nav(view: &mut SideView, cx: &mut Context<'_, SideView>) -> gpui::AnyElement {
    let Some(runtime) = &view.nav else {
        return div().size_full().into_any_element();
    };
    let rows = runtime
        .projects
        .iter()
        .map(|row| (row.id, row.display_name.clone(), row.active))
        .collect::<Vec<_>>();

    let mut list = div()
        .id("nav-list")
        .flex()
        .flex_col()
        .flex_grow(1.0)
        .min_h_0()
        .overflow_y_scroll()
        .text_sm();
    for (index, (id, display_name, active)) in rows.into_iter().enumerate() {
        list = list.child(
            div()
                .id(("nav-project", index))
                .debug_selector(move || format!("nav-project-{index}"))
                .h(px(22.0))
                .px_2()
                .flex()
                .items_center()
                .when(active, |element| element.text_color(cx.theme().foreground))
                .when(!active, |element| {
                    element.text_color(cx.theme().muted_foreground)
                })
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |view, _, window, cx| {
                        view.nav_activate(id, window, cx);
                    }),
                )
                .on_mouse_down(
                    MouseButton::Right,
                    cx.listener(move |view, event: &gpui::MouseDownEvent, window, cx| {
                        view.nav_open_project_menu(id, Some(event.position), window, cx);
                    }),
                )
                .child(div().flex_grow(1.0).child(display_name))
                .child(
                    div()
                        .id(("nav-remove", index))
                        .px_1()
                        .cursor_pointer()
                        .debug_selector(move || format!("nav-remove-{index}"))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |view, _, window, cx| {
                                view.nav_remove(id, window, cx);
                            }),
                        )
                        .child("×"),
                ),
        );
    }
    let sessions = runtime
        .sessions
        .iter()
        .enumerate()
        .map(|(index, row)| (index, row.clone()))
        .collect::<Vec<_>>();
    let mut history = div()
        .id("nav-history")
        .flex()
        .flex_col()
        .text_sm()
        .debug_selector(|| "nav-history".to_owned());
    for (index, row) in sessions {
        let project_hint = if row.project.is_some() {
            row.project_name.clone()
        } else {
            "未归属".to_owned()
        };
        let label = format!(
            "{} · {} · {} · {}",
            row.title,
            row.cli,
            project_hint,
            crate::ai_sessions::relative_label(row.modified)
        );
        history = history.child(
            div()
                .id(("nav-history-row", index))
                .debug_selector(move || format!("nav-history-row-{index}"))
                .h(px(22.0))
                .px_2()
                .flex()
                .items_center()
                .text_color(cx.theme().muted_foreground)
                .when(row.project.is_some(), |element| element.cursor_pointer())
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |view, event: &gpui::MouseDownEvent, window, cx| {
                        if event.click_count == 2 {
                            view.nav_restore_history(index, window, cx);
                        }
                    }),
                )
                .child(label),
        );
    }
    let list = list
        .children((!runtime.sessions.is_empty()).then(|| {
            div()
                .px_2()
                .pt_2()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child("历史会话")
        }))
        .child(history);
    div()
        .id("nav-panel")
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .debug_selector(|| "nav-panel".to_owned())
        .child(list)
        .into_any_element()
}

impl SideView {
    pub(super) fn ensure_nav(&mut self) {
        if self.nav.is_none() {
            self.nav = Some(NavRuntime::new());
        }
    }

    pub(super) fn nav_set_projects(
        &mut self,
        projects: Vec<(ProjectId, String, bool)>,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(runtime) = &mut self.nav else {
            return;
        };
        runtime.projects = projects
            .into_iter()
            .map(|(id, display_name, active)| NavProjectRow {
                id,
                display_name,
                active,
            })
            .collect();
        cx.notify();
    }

    fn nav_open_project_menu(
        &mut self,
        id: ProjectId,
        position: Option<gpui::Point<gpui::Pixels>>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(root_handle) = self.root_handle.clone() else {
            return;
        };
        let window_handle = window.window_handle();
        cx.defer(move |cx| {
            let _ = window_handle.update(cx, |_, window, cx| {
                let _ = root_handle.update(cx, |root, cx| {
                    if let Err(error) = root.open_nav_project_menu(id, position, window, cx) {
                        log::warn!("导航项目菜单打开失败: {error:?}");
                    }
                });
            });
        });
    }

    pub(super) fn nav_set_history(
        &mut self,
        sessions: Vec<NavHistoryRow>,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(runtime) = &mut self.nav else {
            return;
        };
        runtime.sessions = sessions;
        cx.notify();
    }

    fn nav_restore_history(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(root_handle) = self.root_handle.clone() else {
            return;
        };
        let window_handle = window.window_handle();
        cx.defer(move |cx| {
            let _ = window_handle.update(cx, |_, window, cx| {
                let _ = root_handle.update(cx, |root, cx| {
                    if let Err(error) = root.handle_nav_restore_history(index, window, cx) {
                        log::debug!("导航历史恢复未开始: {error}");
                    }
                });
            });
        });
    }

    fn nav_activate(&mut self, id: ProjectId, window: &mut Window, cx: &mut Context<'_, Self>) {
        let Some(root_handle) = self.root_handle.clone() else {
            return;
        };
        let window_handle = window.window_handle();
        cx.defer(move |cx| {
            let _ = window_handle.update(cx, |_, window, cx| {
                let _ = root_handle.update(cx, |root, cx| {
                    let _ = root.handle_nav_activate(id, window, cx);
                });
            });
        });
    }

    fn nav_remove(&mut self, id: ProjectId, window: &mut Window, cx: &mut Context<'_, Self>) {
        let Some(root_handle) = self.root_handle.clone() else {
            return;
        };
        let window_handle = window.window_handle();
        cx.defer(move |cx| {
            let _ = window_handle.update(cx, |_, window, cx| {
                let _ = root_handle.update(cx, |root, cx| {
                    let _ = root.handle_nav_remove(id, window, cx);
                });
            });
        });
    }
}

impl AppRoot {
    /// 项目列表同步：名称与活跃项来自唯一快照。
    pub(super) fn sync_nav_projects(&mut self, cx: &mut Context<'_, Self>) {
        let Some(view) = self
            .side_views
            .instance(slterm_workspace::SideViewId::Nav)
            .cloned()
        else {
            return;
        };
        let active = self.snapshot.registry().active();
        let projects = self
            .snapshot
            .registry()
            .projects()
            .iter()
            .map(|project| {
                (
                    project.id(),
                    project.display_name().to_owned(),
                    Some(project.id()) == active,
                )
            })
            .collect::<Vec<_>>();
        view.update(cx, |view, cx| {
            view.ensure_nav();
            view.nav_set_projects(projects, cx);
        });
        self.refresh_nav_history(cx);
    }

    /// 导航激活入口：与命令目录共用同一激活链。
    pub(super) fn handle_nav_activate(
        &mut self,
        id: ProjectId,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), super::commands::AppRootError> {
        self.activate_project(id, window, cx)
    }

    /// 导航移除入口：与项目关闭共用同一守卫、删除意图与持久阶段。
    pub(super) fn handle_nav_remove(
        &mut self,
        id: ProjectId,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), super::commands::AppRootError> {
        self.remove_project(id, window, cx)
    }

    /// 项目行菜单：右键与后续键盘入口共用同一菜单实体与收合订阅。
    pub(super) fn open_nav_project_menu(
        &mut self,
        project_id: ProjectId,
        position: Option<gpui::Point<gpui::Pixels>>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), super::commands::AppRootError> {
        if self.nav_project_menu.is_some() {
            self.close_nav_project_menu(window, cx)?;
        }
        if self.snapshot.registry().project(project_id).is_none() {
            return Err(super::commands::AppRootError::MissingPanel);
        }
        let trigger = self.focus_manager.workspace_token();
        let language = cx.global::<crate::app_shell::config::Settings>().language;
        let root = cx.weak_entity();
        let menu = PopupMenu::build(window, cx, |menu, _, _| {
            menu.item(
                PopupMenuItem::new(language.text(Message::NavProjectMenuRelocate)).on_click(
                    move |_, window, cx| {
                        let _ = root.update(cx, |this, cx| {
                            if let Err(error) = this.close_nav_project_menu(window, cx) {
                                log::warn!("导航项目菜单关闭失败: {error:?}");
                            }
                            this.start_project_relocation(project_id, window, cx);
                        });
                    },
                ),
            )
        });
        let registration =
            self.focus_manager
                .register(&menu.focus_handle(cx), trigger, None, true)?;
        if let Err(error) =
            self.focus_manager
                .open_modal(trigger, trigger, registration.token(), window, cx)
        {
            let _ = self.focus_manager.unregister(registration.token());
            return Err(error.into());
        }
        let root = cx.weak_entity();
        let dismiss = window.subscribe(&menu, cx, move |_, _: &DismissEvent, window, cx| {
            let _ = root.update(cx, |this, cx| {
                if let Err(error) = this.close_nav_project_menu(window, cx) {
                    log::warn!("导航项目菜单关闭失败: {error:?}");
                }
            });
        });
        self.nav_project_menu = Some(super::NavProjectMenu {
            position,
            menu,
            registration,
            _dismiss: dismiss,
        });
        cx.notify();
        Ok(())
    }

    pub(super) fn close_nav_project_menu(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), super::focus_manager::FocusError> {
        let Some(state) = self.nav_project_menu.take() else {
            return Ok(());
        };
        let close = self.focus_manager.close_modal(window, cx);
        let unregister = self.focus_manager.unregister(state.registration.token());
        cx.notify();
        close?;
        unregister?;
        Ok(())
    }

    /// 扫描入口：同一时刻至多一个在途扫描；旧代次回执丢弃。
    pub(super) fn refresh_nav_history(&mut self, cx: &mut Context<'_, Self>) {
        if self.tasks.is_registered(TaskId::NavHistoryScan) {
            return;
        }
        self.nav_history_generation = self.nav_history_generation.wrapping_add(1);
        let generation = self.nav_history_generation;
        let executor = cx.background_executor().clone();
        let registration = self.tasks.register_with(
            TaskId::NavHistoryScan,
            CancellationToken::new(),
            move || {
                let scanned = executor.spawn(async move { crate::ai_sessions::scan(200) });
                cx.spawn(async move |this, cx| {
                    let scanned = scanned.await;
                    cx.update(|cx| {
                        let _ = this.update(cx, |root, cx| {
                            root.finish_nav_history_scan(generation, scanned, cx);
                        });
                    });
                })
            },
        );
        if registration.is_err() {
            log::debug!("导航历史扫描任务未登记");
        }
    }

    fn finish_nav_history_scan(
        &mut self,
        generation: u64,
        sessions: Vec<crate::ai_sessions::AiSession>,
        cx: &mut Context<'_, Self>,
    ) {
        self.tasks.complete(TaskId::NavHistoryScan);
        if generation != self.nav_history_generation {
            return;
        }
        let projects = self
            .snapshot
            .registry()
            .projects()
            .iter()
            .map(|project| {
                (
                    project.id(),
                    project.root().clone(),
                    project.display_name().to_owned(),
                )
            })
            .collect::<Vec<_>>();
        self.nav_history = sessions
            .into_iter()
            .map(|session| {
                let resume = session.resume_command();
                let attributed = session.cwd.as_ref().and_then(|cwd| {
                    deepest_project_for_cwd(projects.iter().map(|(id, root, _)| (*id, root)), cwd)
                });
                let project_name = attributed
                    .and_then(|id| {
                        projects
                            .iter()
                            .find(|(project, _, _)| *project == id)
                            .map(|(_, _, name)| name.clone())
                    })
                    .unwrap_or_default();
                NavHistoryRow {
                    title: session.title.clone(),
                    cli: session.source.display_name().to_owned(),
                    project: attributed,
                    project_name,
                    cwd: session.cwd,
                    resume,
                    modified: session.modified,
                }
            })
            .collect();
        self.sync_nav_history(cx);
    }

    fn sync_nav_history(&mut self, cx: &mut Context<'_, Self>) {
        let Some(view) = self
            .side_views
            .instance(slterm_workspace::SideViewId::Nav)
            .cloned()
        else {
            return;
        };
        let rows = self.nav_history.clone();
        view.update(cx, |view, cx| view.nav_set_history(rows, cx));
    }

    /// 导航历史恢复：已归属会话直接复用项目；未归属会话需要显式确认。
    pub(super) fn handle_nav_restore_history(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), super::commands::AppRootError> {
        let Some(row) = self.nav_history.get(index).cloned() else {
            return Err(super::commands::AppRootError::MissingPanel);
        };
        let Some(project_id) = row.project else {
            // 未归属会话不得静默落到活跃项目：确认面尚未接线。
            return Err(super::commands::AppRootError::MissingPanel);
        };
        let Some(saved) = row.cwd.clone() else {
            return Err(super::commands::AppRootError::MissingPanel);
        };
        let Some(project_root) = self
            .snapshot
            .registry()
            .project(project_id)
            .map(|project| project.root().clone())
        else {
            return Err(super::commands::AppRootError::MissingPanel);
        };
        self.activate_project(project_id, window, cx)?;
        let task = TaskId::NavHistoryRestore;
        if self.tasks.is_registered(task) {
            return Ok(());
        }
        let resolver = self.project_paths.clone();
        let executor = cx.background_executor().clone();
        let window_handle = window.window_handle();
        let resume = row.resume.clone();
        let registration = self
            .tasks
            .register_with(task, CancellationToken::new(), move || {
                let saved_check = saved.clone();
                let root_check = project_root.clone();
                let checked = executor.spawn(async move {
                    let saved_ok = resolver.resolve(&saved_check).is_ok();
                    let root_ok = resolver.resolve(&root_check).is_ok();
                    (saved_ok, root_ok)
                });
                cx.spawn(async move |this, cx| {
                    let (saved_ok, root_ok) = checked.await;
                    let _ = window_handle.update(cx, |_, window, cx| {
                        let _ = this.update(cx, |root, cx| {
                            root.finish_nav_history_restore(
                                project_id,
                                saved,
                                project_root,
                                saved_ok,
                                root_ok,
                                resume,
                                window,
                                cx,
                            );
                        });
                    });
                })
            });
        if registration.is_err() {
            log::debug!("导航历史恢复任务未登记");
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn finish_nav_history_restore(
        &mut self,
        project_id: ProjectId,
        saved: CanonicalPath,
        project_root: CanonicalPath,
        saved_ok: bool,
        root_ok: bool,
        resume: Option<String>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.tasks.complete(TaskId::NavHistoryRestore);
        if self.ensure_not_closing().is_err() {
            return;
        }
        let cwd = match navigation::resolve_restore_cwd(&saved, saved_ok, &project_root, root_ok) {
            Ok(navigation::RestoreCwd::Saved(cwd))
            | Ok(navigation::RestoreCwd::ProjectRootFallback(cwd)) => cwd,
            Err(_) => {
                log::warn!("导航历史恢复失败：cwd 与项目根均不可用");
                super::events::notify_project_relocation_failed(cx);
                return;
            }
        };
        let panel = match self.new_terminal_from_request(
            self.shell.clone(),
            Some(project_root),
            Some(cwd),
            window,
            cx,
        ) {
            Ok(panel) => panel,
            Err(error) => {
                log::warn!("导航历史恢复建终端失败: {error}");
                super::events::notify_project_relocation_failed(cx);
                return;
            }
        };
        let Some(command) = resume else {
            return;
        };
        let Some(terminal_id) = self.terminal_identity(panel, cx) else {
            return;
        };
        if let Some((_, view)) = self.terminal_panel(project_id, terminal_id, cx) {
            view.update(cx, |view, cx| {
                view.schedule_agent_command(command, None, cx);
            });
        }
    }

    pub(super) fn render_nav_project_menu(&self) -> Option<AnyElement> {
        let state = self.nav_project_menu.as_ref()?;
        let mut anchor = anchored().snap_to_window_with_margin(px(8.0));
        anchor = match state.position {
            Some(position) => anchor.position(position),
            None => anchor.offset(point(px(8.0), px(22.0))),
        };
        Some(
            deferred(
                anchor.child(
                    div()
                        .debug_selector(|| "nav-project-menu".to_owned())
                        .child(state.menu.clone()),
                ),
            )
            .with_priority(1)
            .into_any_element(),
        )
    }
}
