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
use slterm_workspace::ProjectId;

use super::AppRoot;
use super::side_view::SideView;
use crate::i18n::Message;

pub(super) struct NavProjectRow {
    pub(super) id: ProjectId,
    pub(super) display_name: String,
    pub(super) active: bool,
}

pub(super) struct NavRuntime {
    pub(super) projects: Vec<NavProjectRow>,
}

impl NavRuntime {
    pub(super) fn new() -> Self {
        Self {
            projects: Vec::new(),
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
