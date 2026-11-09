//! 面板组页签：布局顺序、完整列表与横向滚动的运行期接线。

use gpui::{
    AnyElement, Context, DismissEvent, Focusable as _, InteractiveElement as _, IntoElement,
    ParentElement as _, Pixels, Point, Styled as _, Window, anchored, deferred, div, px,
};
use gpui_component::{
    IconName, Sizable as _,
    button::{Button, ButtonVariants as _},
    menu::{PopupMenu, PopupMenuItem},
    tab::{Tab, TabBar},
};
use slterm_workspace::{
    PanelGroupId, ProjectId, ProjectLayout, ProjectRegistryError, ProjectSnapshot, ViewKey,
};

use super::commands::AppRootError;
use super::{AppRoot, TabListMenu};

impl AppRoot {
    /// 滚动偏移属于运行期展示态；组身份与激活项仍以布局快照为准。
    pub(super) fn reconcile_tab_scrolls(&mut self, previous: Option<&ProjectSnapshot>) {
        self.tab_scrolls.retain(|(project_id, group_id), _| {
            self.snapshot
                .registry()
                .project(*project_id)
                .and_then(|project| project.layout().group(*group_id))
                .is_some()
        });
        for project in self.snapshot.registry().projects() {
            let project_id = project.id();
            let layout = project.layout();
            for group_id in layout.group_ids() {
                let group = &layout.group(group_id).expect("树中的组必须存在");
                let active_index = group
                    .views()
                    .iter()
                    .position(|view| view == group.active())
                    .expect("激活视图必须属于当前组");
                let changed = previous
                    .and_then(|snapshot| snapshot.registry().project(project_id))
                    .and_then(|project| project.layout().group(group_id))
                    .is_none_or(|old| {
                        old.views() != group.views() || old.active() != group.active()
                    });
                let scroll = self.tab_scrolls.entry((project_id, group_id)).or_default();
                if changed {
                    scroll.scroll_to_item(active_index);
                }
            }
        }
    }

    pub(super) fn render_tab_bar(
        &self,
        project_id: ProjectId,
        group_id: PanelGroupId,
        layout: &ProjectLayout,
        cx: &Context<'_, Self>,
    ) -> AnyElement {
        let group = layout.group(group_id).expect("渲染中的组必须存在");
        let views = group.views().to_vec();
        let active_index = views
            .iter()
            .position(|view| view == group.active())
            .expect("激活视图必须属于当前组");
        let scroll = self
            .tab_scrolls
            .get(&(project_id, group_id))
            .expect("渲染前必须已建立页签滚动状态")
            .clone();
        let clicked_views = views.clone();
        let root = cx.weak_entity();
        let tabs = views.into_iter().map(|view| {
            let mut tab = Tab::new().label(self.tab_title(&view));
            if let ViewKey::Terminal { terminal_id, .. } = view {
                let selector = format!("app-root-tab-{terminal_id}");
                tab = tab.child(
                    div()
                        .absolute()
                        .inset_0()
                        .debug_selector(move || selector.clone()),
                );
            }
            tab
        });
        let list_root = cx.weak_entity();
        let list_selector = format!("app-root-tab-list-{group_id}");
        let complete_list = Button::new("complete-list")
            .ghost()
            .xsmall()
            .icon(IconName::Ellipsis)
            .debug_selector(move || list_selector.clone())
            .on_click(move |event, window, cx| {
                let _ = list_root.update(cx, |root, cx| {
                    if let Err(error) = root.open_tab_list(group_id, event.position(), window, cx) {
                        log::warn!("打开完整页签列表失败: {error}");
                    }
                });
            });
        div()
            .w_full()
            .flex_shrink_0()
            .debug_selector(move || format!("app-root-tab-bar-{group_id}"))
            .child(
                TabBar::new(format!("app-root-tabs-{project_id}-{group_id}"))
                    .w_full()
                    .track_scroll(&scroll)
                    .selected_index(active_index)
                    .on_click(move |index, window, cx| {
                        let Some(view) = clicked_views.get(*index).cloned() else {
                            return;
                        };
                        let _ = root.update(cx, |root, cx| {
                            if let Err(error) = root.activate_view(view, window, cx) {
                                log::warn!("页签激活视图失败: {error}");
                            }
                        });
                    })
                    .suffix(complete_list)
                    .children(tabs),
            )
            .children(self.render_tab_list_menu(group_id))
            .into_any_element()
    }

    fn open_tab_list(
        &mut self,
        group_id: PanelGroupId,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        self.ensure_not_closing()?;
        self.ensure_project_snapshot_command_allowed()?;
        if self.tab_list_menu.is_some() {
            self.close_tab_list_menu(window, cx)?;
        }
        self.sync_focus(window, cx);
        let project_id = self.active_project()?;
        let project = self
            .snapshot
            .registry()
            .project(project_id)
            .ok_or(ProjectRegistryError::ProjectNotFound)?;
        let group = project
            .layout()
            .group(group_id)
            .ok_or(slterm_workspace::LayoutError::GroupNotFound)?;
        let active = group.active().clone();
        let views = group.views().to_vec();
        let active_index = views
            .iter()
            .position(|view| view == group.active())
            .expect("激活视图必须属于当前组");
        let titles: Vec<_> = views.iter().map(|view| self.tab_title(view)).collect();
        let panel_id = self.panel_for_view(project_id, &active, cx)?;
        let trigger = self
            .panel_focus
            .get(&panel_id)
            .map(super::focus_manager::FocusRegistration::token)
            .ok_or(AppRootError::MissingPanel)?;
        let root = cx.weak_entity();
        let menu = PopupMenu::build(window, cx, |mut menu, _, _| {
            menu = menu.scrollable(true);
            for (index, (view, title)) in views.iter().zip(titles).enumerate() {
                let root = root.clone();
                let view = view.clone();
                menu = menu.item(
                    PopupMenuItem::new(title)
                        .checked(index == active_index)
                        .on_click(move |_, window, cx| {
                            let _ = root.update(cx, |root, cx| {
                                if let Err(error) = root.close_tab_list_menu(window, cx) {
                                    log::warn!("完整页签列表关闭失败: {error:?}");
                                    return;
                                }
                                if let Err(error) = root.activate_view(view.clone(), window, cx) {
                                    log::warn!("完整列表激活视图失败: {error}");
                                }
                            });
                        }),
                );
            }
            menu
        });
        let registration =
            self.focus_manager
                .register(&menu.focus_handle(cx), trigger, Some(project_id), true)?;
        if let Err(error) =
            self.focus_manager
                .open_modal(trigger, trigger, registration.token(), window, cx)
        {
            let _ = self.focus_manager.unregister(registration.token());
            return Err(error.into());
        }
        let root = cx.weak_entity();
        let dismiss = window.subscribe(&menu, cx, move |_, _: &DismissEvent, window, cx| {
            let _ = root.update(cx, |root, cx| {
                if let Err(error) = root.close_tab_list_menu(window, cx) {
                    log::warn!("完整页签列表收合失败: {error:?}");
                }
            });
        });
        self.tab_list_menu = Some(TabListMenu {
            group: group_id,
            position,
            menu,
            registration,
            _dismiss: dismiss,
        });
        cx.notify();
        Ok(())
    }

    fn close_tab_list_menu(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), super::focus_manager::FocusError> {
        let Some(state) = self.tab_list_menu.take() else {
            return Ok(());
        };
        let close = self.focus_manager.close_modal(window, cx);
        let unregister = self.focus_manager.unregister(state.registration.token());
        cx.notify();
        close?;
        unregister?;
        Ok(())
    }

    fn render_tab_list_menu(&self, group_id: PanelGroupId) -> Option<AnyElement> {
        let state = self
            .tab_list_menu
            .as_ref()
            .filter(|state| state.group == group_id)?;
        Some(
            deferred(
                anchored()
                    .snap_to_window_with_margin(px(8.0))
                    .position(state.position)
                    .child(
                        div()
                            .debug_selector(|| "app-root-tab-list-menu".to_owned())
                            .child(state.menu.clone()),
                    ),
            )
            .with_priority(1)
            .into_any_element(),
        )
    }

    fn tab_title(&self, view: &ViewKey) -> String {
        match view {
            ViewKey::Terminal { terminal_id, .. } => self
                .snapshot
                .terminal(*terminal_id)
                .map(|record| {
                    record
                        .custom_title()
                        .map(str::to_owned)
                        .unwrap_or_else(|| format!("terminal-{}", record.number()))
                })
                .expect("布局中的终端必须有逻辑记录"),
            _ => view.kind_str().to_owned(),
        }
    }
}
