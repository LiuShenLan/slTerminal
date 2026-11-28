//! 导航视图：项目列表的显示、激活与移除入口。
//!
//! 激活与移除只派发到 AppRoot 的统一命令链（`activate_project`／
//! `remove_project`），不在视图内复制守卫、布局或关闭逻辑。

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, Context, DismissEvent, FocusHandle, Focusable as _, InteractiveElement as _,
    IntoElement, MouseButton, ParentElement as _, StatefulInteractiveElement as _, Styled as _,
    Window, anchored, deferred, div, point, px,
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
    /// 来源文件：删除隔离的唯一输入，不按标题或其他行猜测。
    pub(super) source_path: std::path::PathBuf,
}

pub(super) struct NavRuntime {
    pub(super) projects: Vec<NavProjectRow>,
    sessions: Vec<NavHistoryRow>,
    quarantine_batches: Vec<super::nav_quarantine::NavQuarantineBatch>,
}

/// 未归属会话创建项目前的显式确认；只描述目标根，不预建项目或终端。
pub(super) struct NavRestoreConfirm {
    row: usize,
    cwd: CanonicalPath,
    resume: Option<String>,
    focus_handle: FocusHandle,
    registration: super::focus_manager::FocusRegistration,
}

impl NavRuntime {
    pub(super) fn new() -> Self {
        Self {
            projects: Vec::new(),
            sessions: Vec::new(),
            quarantine_batches: Vec::new(),
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
                .on_mouse_down(
                    MouseButton::Right,
                    cx.listener(move |view, event: &gpui::MouseDownEvent, window, cx| {
                        view.nav_open_history_menu(index, Some(event.position), window, cx);
                    }),
                )
                .child(label),
        );
    }
    let quarantine_entry = (!runtime.quarantine_batches.is_empty()).then(|| {
        let language = cx.global::<crate::app_shell::config::Settings>().language;
        div()
            .id("nav-quarantine-open")
            .debug_selector(|| "nav-quarantine-open".to_owned())
            .h(px(22.0))
            .px_2()
            .flex()
            .items_center()
            .cursor_pointer()
            .text_color(cx.theme().muted_foreground)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|view, _, window, cx| {
                    view.nav_open_quarantine(window, cx);
                }),
            )
            .child(format!(
                "{} ({})",
                language.text(Message::NavQuarantineOpen),
                runtime.quarantine_batches.len()
            ))
    });
    let list = list
        .children(quarantine_entry)
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

    pub(super) fn nav_set_quarantine(
        &mut self,
        batches: Vec<super::nav_quarantine::NavQuarantineBatch>,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(runtime) = &mut self.nav else {
            return;
        };
        runtime.quarantine_batches = batches;
        cx.notify();
    }

    fn nav_open_history_menu(
        &mut self,
        index: usize,
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
                    if let Err(error) = root.open_nav_history_menu(index, position, window, cx) {
                        log::debug!("导航历史菜单打开失败: {error:?}");
                    }
                });
            });
        });
    }

    fn nav_open_quarantine(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let Some(root_handle) = self.root_handle.clone() else {
            return;
        };
        let window_handle = window.window_handle();
        cx.defer(move |cx| {
            let _ = window_handle.update(cx, |_, window, cx| {
                let _ = root_handle.update(cx, |root, cx| {
                    if let Err(error) = root.open_nav_quarantine(window, cx) {
                        log::debug!("隔离区打开失败: {error}");
                    }
                });
            });
        });
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
        self.sync_nav_quarantine(cx);
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

    /// 历史行菜单：右键与键盘入口共用同一菜单实体；删除在收合后走统一命令。
    pub(super) fn open_nav_history_menu(
        &mut self,
        index: usize,
        position: Option<gpui::Point<gpui::Pixels>>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), super::commands::AppRootError> {
        if self.nav_history_menu.is_some() {
            self.close_nav_history_menu(window, cx)?;
        }
        let Some(source) = self
            .nav_history
            .get(index)
            .map(|row| row.source_path.clone())
        else {
            return Err(super::commands::AppRootError::MissingPanel);
        };
        let trigger = self.focus_manager.workspace_token();
        let language = cx.global::<crate::app_shell::config::Settings>().language;
        let root = cx.weak_entity();
        let menu = PopupMenu::build(window, cx, |menu, _, _| {
            menu.item(
                PopupMenuItem::new(language.text(Message::NavHistoryMenuDelete)).on_click(
                    move |_, window, cx| {
                        let source = source.clone();
                        let _ = root.update(cx, |this, cx| {
                            if let Err(error) = this.close_nav_history_menu(window, cx) {
                                log::warn!("导航历史菜单关闭失败: {error:?}");
                            }
                            let Some(index) = this
                                .nav_history
                                .iter()
                                .position(|row| row.source_path == source)
                            else {
                                return;
                            };
                            if let Err(error) = this.handle_nav_delete_history(index, window, cx) {
                                log::debug!("导航历史删除未开始: {error}");
                            }
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
                if let Err(error) = this.close_nav_history_menu(window, cx) {
                    log::warn!("导航历史菜单关闭失败: {error:?}");
                }
            });
        });
        self.nav_history_menu = Some(super::NavHistoryMenu {
            position,
            menu,
            registration,
            _dismiss: dismiss,
        });
        cx.notify();
        Ok(())
    }

    pub(super) fn close_nav_history_menu(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), super::focus_manager::FocusError> {
        let Some(state) = self.nav_history_menu.take() else {
            return Ok(());
        };
        let close = self.focus_manager.close_modal(window, cx);
        let unregister = self.focus_manager.unregister(state.registration.token());
        cx.notify();
        close?;
        unregister?;
        Ok(())
    }

    /// 测试注入 home：导航扫描改读该目录，不触碰进程环境或真实用户目录。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn set_session_home_for_test(&mut self, home: std::path::PathBuf) {
        self.session_home_for_test = Some(home);
    }

    /// 扫描入口：同一时刻至多一个在途扫描；旧代次回执丢弃。
    pub(super) fn refresh_nav_history(&mut self, cx: &mut Context<'_, Self>) {
        if self.tasks.is_registered(TaskId::NavHistoryScan) {
            return;
        }
        self.nav_history_generation = self.nav_history_generation.wrapping_add(1);
        let generation = self.nav_history_generation;
        let executor = cx.background_executor().clone();
        #[cfg(all(test, feature = "gpui-test-support"))]
        let session_home = self.session_home_for_test.clone();
        let registration = self.tasks.register_with(
            TaskId::NavHistoryScan,
            CancellationToken::new(),
            move || {
                let scanned = executor.spawn(async move {
                    #[cfg(all(test, feature = "gpui-test-support"))]
                    if let Some(home) = session_home {
                        return crate::ai_sessions::scan_at(&home, 200);
                    }
                    crate::ai_sessions::scan(200)
                });
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
                let source_path = session.source_path().to_path_buf();
                NavHistoryRow {
                    title: session.title.clone(),
                    cli: session.source.display_name().to_owned(),
                    project: attributed,
                    project_name,
                    cwd: session.cwd,
                    resume,
                    modified: session.modified,
                    source_path,
                }
            })
            .filter(|row| !self.quarantine.hidden_sources.contains(&row.source_path))
            .collect();
        self.sync_nav_history(cx);
    }

    pub(super) fn sync_nav_history(&mut self, cx: &mut Context<'_, Self>) {
        let Some(view) = self
            .side_views
            .instance(slterm_workspace::SideViewId::Nav)
            .cloned()
        else {
            return;
        };
        let rows = self.nav_history.clone();
        view.update(cx, |view, cx| view.nav_set_history(rows, cx));
        self.sync_nav_quarantine(cx);
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
            let Some(cwd) = row.cwd.clone() else {
                return Err(super::commands::AppRootError::MissingPanel);
            };
            match navigation::resolve_restore_project(None, &cwd) {
                navigation::RestoreProject::RequiresConfirmation(root) => {
                    self.open_nav_restore_confirm(index, root, row.resume.clone(), window, cx)?;
                }
                navigation::RestoreProject::Reuse(_) => {
                    unreachable!("未归属会话不能复用既有项目")
                }
            }
            return Ok(());
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

    fn open_nav_restore_confirm(
        &mut self,
        row: usize,
        cwd: CanonicalPath,
        resume: Option<String>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), super::commands::AppRootError> {
        if self.nav_restore_confirm.is_some() {
            return Ok(());
        }
        let focus_handle = cx.focus_handle();
        let registration = self.focus_manager.register(
            &focus_handle,
            self.focus_manager.workspace_token(),
            None,
            true,
        )?;
        if let Err(error) = self.focus_manager.open_modal(
            self.focus_manager.workspace_token(),
            self.focus_manager.workspace_token(),
            registration.token(),
            window,
            cx,
        ) {
            let _ = self.focus_manager.unregister(registration.token());
            return Err(error.into());
        }
        self.nav_restore_confirm = Some(NavRestoreConfirm {
            row,
            cwd,
            resume,
            focus_handle,
            registration,
        });
        cx.notify();
        Ok(())
    }

    pub(super) fn cancel_nav_restore_confirm(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), super::commands::AppRootError> {
        let Some(confirm) = self.nav_restore_confirm.take() else {
            return Ok(());
        };
        let close = self.focus_manager.close_modal(window, cx);
        let unregister = self.focus_manager.unregister(confirm.registration.token());
        cx.notify();
        close?;
        unregister?;
        Ok(())
    }

    /// 确认后经统一项目打开 FIFO 创建项目，完成回执里再建终端并注入恢复命令。
    pub(super) fn accept_nav_restore_confirm(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), super::commands::AppRootError> {
        let Some(confirm) = self.nav_restore_confirm.take() else {
            return Ok(());
        };
        let close = self.focus_manager.close_modal(window, cx);
        let unregister = self.focus_manager.unregister(confirm.registration.token());
        cx.notify();
        close?;
        unregister?;
        let cwd = confirm.cwd;
        let target = cwd.clone();
        let resume = confirm.resume;
        self.open_project_with_completion(cwd, window, cx, move |result, root, window, cx| {
            let Ok(project_id) = result else {
                log::warn!("未归属会话创建项目失败");
                super::events::notify_project_relocation_failed(cx);
                return;
            };
            let Some(window) = window else {
                return;
            };
            let panel = match root.new_terminal_from_request(
                root.shell.clone(),
                Some(
                    root.snapshot
                        .registry()
                        .project(project_id)
                        .expect("项目必须存在")
                        .root()
                        .clone(),
                ),
                Some(target.clone()),
                window,
                cx,
            ) {
                Ok(panel) => panel,
                Err(error) => {
                    log::warn!("未归属会话恢复建终端失败: {error}");
                    super::events::notify_project_relocation_failed(cx);
                    return;
                }
            };
            let Some(command) = resume else {
                return;
            };
            let Some(terminal_id) = root.terminal_identity(panel, cx) else {
                return;
            };
            if let Some((_, view)) = root.terminal_panel(project_id, terminal_id, cx) {
                view.update(cx, |view, cx| {
                    view.schedule_agent_command(command, None, cx);
                });
            }
        });
        Ok(())
    }

    /// 未归属确认面：遮罩拦截背景命中，Escape 等同取消。
    pub(super) fn render_nav_restore_confirm(&self, cx: &Context<'_, Self>) -> Option<AnyElement> {
        let confirm = self.nav_restore_confirm.as_ref()?;
        let _ = confirm.row;
        let language = cx.global::<crate::app_shell::config::Settings>().language;
        let detail = language.format(
            Message::NavRestoreConfirmDetail,
            &[("root", &confirm.cwd.as_path().display().to_string())],
        );
        let dialog = div()
            .debug_selector(|| "nav-restore-confirm".to_owned())
            .track_focus(&confirm.focus_handle)
            .on_key_down(cx.listener(|root, event: &gpui::KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape"
                    && let Err(error) = root.cancel_nav_restore_confirm(window, cx)
                {
                    log::warn!("导航恢复确认取消失败: {error:?}");
                }
            }))
            .w(px(420.0))
            .p_4()
            .flex()
            .flex_col()
            .gap_3()
            .bg(cx.theme().background)
            .border_1()
            .border_color(cx.theme().border)
            .text_color(cx.theme().foreground)
            .child(
                div()
                    .text_sm()
                    .child(language.text(Message::NavRestoreConfirmTitle)),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(detail),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .justify_end()
                    .gap_2()
                    .child(
                        div()
                            .id("nav-restore-cancel")
                            .debug_selector(|| "nav-restore-cancel".to_owned())
                            .cursor_pointer()
                            .px_2()
                            .child(language.text(Message::EditorCancel))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|root, _, window, cx| {
                                    if let Err(error) = root.cancel_nav_restore_confirm(window, cx)
                                    {
                                        log::warn!("导航恢复确认取消失败: {error:?}");
                                    }
                                }),
                            ),
                    )
                    .child(
                        div()
                            .id("nav-restore-accept")
                            .debug_selector(|| "nav-restore-accept".to_owned())
                            .cursor_pointer()
                            .px_2()
                            .child(language.text(Message::NavRestoreConfirmAccept))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|root, _, window, cx| {
                                    if let Err(error) = root.accept_nav_restore_confirm(window, cx)
                                    {
                                        log::warn!("导航恢复确认失败: {error:?}");
                                    }
                                }),
                            ),
                    ),
            );
        Some(
            div()
                .absolute()
                .inset_0()
                .occlude()
                .bg(cx.theme().background.opacity(0.7))
                .child(
                    div()
                        .absolute()
                        .top(px(120.0))
                        .left_0()
                        .right_0()
                        .flex()
                        .justify_center()
                        .child(dialog),
                )
                .into_any_element(),
        )
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

    pub(super) fn render_nav_history_menu(&self) -> Option<AnyElement> {
        let state = self.nav_history_menu.as_ref()?;
        let mut anchor = anchored().snap_to_window_with_margin(px(8.0));
        anchor = match state.position {
            Some(position) => anchor.position(position),
            None => anchor.offset(point(px(8.0), px(22.0))),
        };
        Some(
            deferred(
                anchor.child(
                    div()
                        .debug_selector(|| "nav-history-menu".to_owned())
                        .child(state.menu.clone()),
                ),
            )
            .with_priority(1)
            .into_any_element(),
        )
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
