//! 导航视图：项目列表的显示、激活与移除入口。
//!
//! 激活与移除只派发到 AppRoot 的统一命令链（`activate_project`／
//! `remove_project`），不在视图内复制守卫、布局或关闭逻辑。

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AppContext as _, Context, InteractiveElement as _, IntoElement, MouseButton,
    ParentElement as _, StatefulInteractiveElement as _, Styled as _, Window, div, px,
};
use gpui_component::ActiveTheme as _;
use slterm_workspace::ProjectId;

use super::AppRoot;
use super::side_view::SideView;

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
}
