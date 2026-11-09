//! 既有视图的布局命令：候选快照、稳定实例与焦点协调。

use gpui::{
    AnyElement, App, Context, DismissEvent, Entity, Focusable as _, InteractiveElement as _,
    IntoElement, ParentElement as _, WeakEntity, Window, anchored, deferred, div, px,
};
use gpui_component::menu::{PopupMenu, PopupMenuItem};
use slterm_split::{SplitDirection, SplitTree};
use slterm_workspace::{
    LayoutError, LogicalSize, PanelGroupId, PanelId, ProjectId, ProjectRegistryError,
    ProjectSnapshot, ViewKey,
};
use uuid::Uuid;

use super::command_registry::{CommandId, CommandParameters, CommandSource, CommandTarget};
use super::commands::AppRootError;
use super::focus_manager::FocusError;
use super::{AppRoot, LayoutMenu, ProjectView, SplitDividerFocus};
use crate::i18n::{Message, UiLanguage};

pub(super) const PROJECT_LAYOUT_KEY_CONTEXT: &str = "ProjectLayout";
pub(super) const SPLIT_DIVIDER_KEY_CONTEXT: &str = "layout-split-divider";

pub(super) struct SplitViewOptions {
    pub(super) view: ViewKey,
    pub(super) target_group: PanelGroupId,
    pub(super) direction: SplitDirection,
    pub(super) ratio: f32,
    pub(super) viewport: LogicalSize,
}

fn collect_split_paths(
    tree: &SplitTree<PanelGroupId>,
    path: &mut Vec<bool>,
    paths: &mut Vec<Vec<bool>>,
) {
    if let SplitTree::Split { first, second, .. } = tree {
        paths.push(path.clone());
        path.push(false);
        collect_split_paths(first, path, paths);
        path.pop();
        path.push(true);
        collect_split_paths(second, path, paths);
        path.pop();
    }
}

impl AppRoot {
    /// 在关闭候选提交前记录将被删除的真实焦点分隔线。
    pub(super) fn focused_split_divider_removed(
        &self,
        project_id: ProjectId,
        candidate: &ProjectSnapshot,
        window: &Window,
        cx: &App,
    ) -> bool {
        let Some(focused) = window.focused(cx) else {
            return false;
        };
        let mut remaining = Vec::new();
        if let Some(tree) = candidate
            .registry()
            .project(project_id)
            .and_then(|project| project.layout().tree())
        {
            collect_split_paths(tree, &mut Vec::new(), &mut remaining);
        }
        self.split_dividers.iter().any(|((owner, path), divider)| {
            *owner == project_id && divider.handle == focused && !remaining.contains(path)
        })
    }

    pub(super) fn reconcile_split_dividers(&mut self, cx: &mut App) {
        let mut current = Vec::new();
        for project in self.snapshot.registry().projects() {
            if let Some(tree) = project.layout().tree() {
                let mut paths = Vec::new();
                collect_split_paths(tree, &mut Vec::new(), &mut paths);
                current.extend(paths.into_iter().map(|path| (project.id(), path)));
            }
        }
        let stale: Vec<_> = self
            .split_dividers
            .keys()
            .filter(|key| !current.contains(key))
            .cloned()
            .collect();
        for key in stale {
            let divider = self
                .split_dividers
                .remove(&key)
                .expect("陈旧分隔线必须存在");
            self.focus_manager
                .unregister(divider.registration.token())
                .expect("分隔线注销必须对应存活注册");
        }
        for (project_id, path) in current {
            if self
                .split_dividers
                .contains_key(&(project_id, path.clone()))
            {
                continue;
            }
            let handle = cx.focus_handle().tab_stop(true);
            let parent = self
                .project_focus_token(project_id, cx)
                .expect("分隔线父项目必须已注册");
            let visible = self.snapshot.registry().active() == Some(project_id);
            let registration = self
                .focus_manager
                .register(&handle, parent, Some(project_id), visible)
                .expect("分割路径焦点注册必须唯一");
            self.split_dividers.insert(
                (project_id, path),
                SplitDividerFocus {
                    handle,
                    registration,
                },
            );
        }
    }

    pub(super) fn resize_split_action(
        &mut self,
        path: Vec<bool>,
        current_ratio: f32,
        delta: f32,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let viewport = self.project_viewport(window);
        if let Err(error) = self.execute_command(
            super::command_registry::CommandId::ResizeSplit,
            super::command_registry::CommandSource::Keyboard,
            super::command_registry::CommandTarget::Focused,
            Some(super::command_registry::CommandParameters::ResizeSplit {
                path,
                ratio: current_ratio + delta,
                viewport,
            }),
            window,
            cx,
        ) {
            log::warn!("分隔线键盘调整失败: {error}");
        }
    }

    pub(super) fn focus_split_divider_from_request(
        &mut self,
        path: Vec<bool>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        self.ensure_not_closing()?;
        self.sync_focus(window, cx);
        let project_id = self.active_project()?;
        let token = self
            .split_dividers
            .get(&(project_id, path))
            .map(|divider| divider.registration.token())
            .ok_or(AppRootError::Layout(LayoutError::SplitNotFound))?;
        self.focus_manager.request_focus(token, window, cx)?;
        Ok(())
    }
    fn layout_candidate(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(ProjectId, slterm_workspace::ProjectSnapshot), AppRootError> {
        self.ensure_not_closing()?;
        self.ensure_project_snapshot_command_allowed()?;
        self.sync_focus(window, cx);
        let project_id = self.active_project()?;
        Ok((project_id, self.snapshot.clone()))
    }

    fn panel_for_view(
        &self,
        project_id: ProjectId,
        view: &ViewKey,
        cx: &App,
    ) -> Result<PanelId, AppRootError> {
        self.projects
            .get(&project_id)
            .and_then(|project| {
                project
                    .read(cx)
                    .panels
                    .values()
                    .find(|panel| &panel.view_key == view)
                    .map(|panel| panel.id)
            })
            .ok_or(AppRootError::MissingPanel)
    }

    fn focus_layout_view(
        &mut self,
        project_id: ProjectId,
        view: &ViewKey,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        let panel_id = self.panel_for_view(project_id, view, cx)?;
        let focus_token = self
            .panel_focus
            .get(&panel_id)
            .map(super::focus_manager::FocusRegistration::token)
            .ok_or(AppRootError::MissingPanel)?;
        self.focus_manager.request_focus(focus_token, window, cx)?;
        self.projects
            .get(&project_id)
            .cloned()
            .ok_or(ProjectRegistryError::ProjectNotFound)?
            .update(cx, |project: &mut ProjectView, _| {
                project.focused_panel = Some(panel_id)
            });
        Ok(())
    }

    pub(super) fn move_view_from_request(
        &mut self,
        view: ViewKey,
        target_group: PanelGroupId,
        target_index: usize,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        let (project_id, mut candidate) = self.layout_candidate(window, cx)?;
        candidate
            .registry_mut()
            .layout_mut(project_id)
            .ok_or(ProjectRegistryError::ProjectNotFound)?
            .move_view(&view, target_group, target_index)?;
        candidate.validate()?;
        self.focus_layout_view(project_id, &view, window, cx)?;
        self.commit_project_snapshot(candidate, cx);
        cx.notify();
        Ok(())
    }

    pub(super) fn merge_groups_from_request(
        &mut self,
        source_group: PanelGroupId,
        target_group: PanelGroupId,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        let (project_id, mut candidate) = self.layout_candidate(window, cx)?;
        let layout = candidate
            .registry_mut()
            .layout_mut(project_id)
            .ok_or(ProjectRegistryError::ProjectNotFound)?;
        layout.merge_groups(source_group, target_group)?;
        let active = layout
            .group(target_group)
            .map(|group| group.active().clone())
            .ok_or(LayoutError::GroupNotFound)?;
        candidate.validate()?;
        self.focus_layout_view(project_id, &active, window, cx)?;
        self.commit_project_snapshot(candidate, cx);
        cx.notify();
        Ok(())
    }

    pub(super) fn split_view_from_request(
        &mut self,
        options: SplitViewOptions,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        let SplitViewOptions {
            view,
            target_group,
            direction,
            ratio,
            viewport,
        } = options;
        let (project_id, mut candidate) = self.layout_candidate(window, cx)?;
        let new_group_id =
            PanelGroupId::from_uuid(Uuid::now_v7()).expect("系统生成的 UUID 必须为 v7");
        candidate
            .registry_mut()
            .layout_mut(project_id)
            .ok_or(ProjectRegistryError::ProjectNotFound)?
            .split_view(
                &view,
                target_group,
                new_group_id,
                direction,
                ratio,
                viewport,
            )?;
        candidate.validate()?;
        self.focus_layout_view(project_id, &view, window, cx)?;
        self.commit_project_snapshot(candidate, cx);
        cx.notify();
        Ok(())
    }

    pub(super) fn resize_split_from_request(
        &mut self,
        path: Vec<bool>,
        ratio: f32,
        viewport: LogicalSize,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        let (project_id, mut candidate) = self.layout_candidate(window, cx)?;
        candidate
            .registry_mut()
            .layout_mut(project_id)
            .ok_or(ProjectRegistryError::ProjectNotFound)?
            .resize_split(&path, ratio, viewport)?;
        candidate.validate()?;
        self.commit_project_snapshot(candidate, cx);
        cx.notify();
        Ok(())
    }

    fn close_layout_menu(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), FocusError> {
        let Some(state) = self.layout_menu.take() else {
            return Ok(());
        };
        let close = self.focus_manager.close_modal(window, cx);
        let unregister = self.focus_manager.unregister(state.registration.token());
        cx.notify();
        close?;
        unregister?;
        Ok(())
    }

    pub(super) fn open_layout_menu_from_request(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), AppRootError> {
        self.ensure_not_closing()?;
        self.ensure_project_snapshot_command_allowed()?;
        if self.layout_menu.is_some() {
            self.close_layout_menu(window, cx)?;
        }
        self.sync_focus(window, cx);
        let project_id = self.active_project()?;
        let group = self
            .focused_group(project_id, cx)?
            .ok_or(AppRootError::MissingPanel)?;
        let active = self
            .snapshot
            .registry()
            .project(project_id)
            .and_then(|project| project.layout().group(group))
            .map(|group| group.active().clone())
            .ok_or(LayoutError::GroupNotFound)?;
        self.focus_layout_view(project_id, &active, window, cx)?;
        let panel_id = self.panel_for_view(project_id, &active, cx)?;
        let trigger = self
            .panel_focus
            .get(&panel_id)
            .map(super::focus_manager::FocusRegistration::token)
            .ok_or(AppRootError::MissingPanel)?;
        let viewport = self.project_viewport(window);
        let root = cx.weak_entity();
        let language = cx.global::<crate::app_shell::config::Settings>().language;
        let menu = self.build_layout_menu(group, viewport, root, language, window, cx)?;
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
            let _ = root.update(cx, |this, cx| {
                if let Err(error) = this.close_layout_menu(window, cx) {
                    log::warn!("布局菜单关闭失败: {error:?}");
                }
            });
        });
        self.layout_menu = Some(LayoutMenu {
            group,
            menu,
            registration,
            _dismiss: dismiss,
        });
        cx.notify();
        Ok(())
    }

    fn build_layout_menu(
        &self,
        group: PanelGroupId,
        viewport: LogicalSize,
        root: WeakEntity<Self>,
        language: UiLanguage,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Result<Entity<PopupMenu>, AppRootError> {
        let project_id = self.active_project()?;
        let project = self
            .snapshot
            .registry()
            .project(project_id)
            .ok_or(ProjectRegistryError::ProjectNotFound)?;
        let layout = project.layout();
        let groups = layout.group_ids();
        let index = groups
            .iter()
            .position(|candidate| *candidate == group)
            .ok_or(LayoutError::GroupNotFound)?;
        let previous = index.checked_sub(1).map(|index| groups[index]);
        let next = groups.get(index + 1).copied();
        let active = layout
            .group(group)
            .map(|group| group.active().clone())
            .ok_or(LayoutError::GroupNotFound)?;
        let divider = layout
            .tree()
            .and_then(|tree| find_group_path(tree, group))
            .and_then(|mut path| {
                path.pop()?;
                Some(path)
            });
        let can_split = layout.group(group).unwrap().views().len() > 1;
        if previous.is_none() && next.is_none() && !can_split && divider.is_none() {
            return Err(AppRootError::NoLayoutActions);
        }
        Ok(PopupMenu::build(window, cx, |mut menu, _, _| {
            for (target, message) in [
                (previous, Message::LayoutMenuMovePrevious),
                (next, Message::LayoutMenuMoveNext),
            ]
            .into_iter()
            .filter_map(|(target, message)| Some((target?, message)))
            {
                let target_index = layout.group(target).unwrap().views().len();
                menu = menu.item(layout_menu_item(
                    &root,
                    language,
                    message,
                    CommandId::MoveView,
                    CommandParameters::MoveView {
                        view: active.clone(),
                        target_group: target,
                        target_index,
                    },
                ));
            }
            if can_split {
                for (direction, message) in [
                    (SplitDirection::LeftRight, Message::LayoutMenuSplitRight),
                    (SplitDirection::TopBottom, Message::LayoutMenuSplitDown),
                ] {
                    menu = menu.item(layout_menu_item(
                        &root,
                        language,
                        message,
                        CommandId::SplitView,
                        CommandParameters::SplitView {
                            view: active.clone(),
                            target_group: group,
                            direction,
                            ratio: 0.5,
                            viewport,
                        },
                    ));
                }
            }
            for (target, message) in [
                (previous, Message::LayoutMenuMergePrevious),
                (next, Message::LayoutMenuMergeNext),
            ]
            .into_iter()
            .filter_map(|(target, message)| Some((target?, message)))
            {
                menu = menu.item(layout_menu_item(
                    &root,
                    language,
                    message,
                    CommandId::MergeGroups,
                    CommandParameters::MergeGroups {
                        source_group: group,
                        target_group: target,
                    },
                ));
            }
            if let Some(path) = divider {
                menu = menu.item(layout_menu_item(
                    &root,
                    language,
                    Message::LayoutMenuFocusDivider,
                    CommandId::FocusSplitDivider,
                    CommandParameters::FocusSplitDivider { path },
                ));
            }
            menu
        }))
    }

    pub(super) fn render_layout_menu(&self, group: PanelGroupId) -> Option<AnyElement> {
        let state = self
            .layout_menu
            .as_ref()
            .filter(|state| state.group == group)?;
        let anchor = anchored().snap_to_window_with_margin(px(8.0));
        Some(
            deferred(
                anchor.child(
                    div()
                        .debug_selector(|| "app-root-layout-menu".to_owned())
                        .child(state.menu.clone()),
                ),
            )
            .with_priority(1)
            .into_any_element(),
        )
    }
}

fn layout_menu_item(
    root: &WeakEntity<AppRoot>,
    language: UiLanguage,
    message: Message,
    id: CommandId,
    parameters: CommandParameters,
) -> PopupMenuItem {
    let root = root.clone();
    PopupMenuItem::new(language.text(message)).on_click(move |_, window, cx| {
        let _ = root.update(cx, |this, cx| {
            if let Err(error) = this.close_layout_menu(window, cx) {
                log::warn!("布局菜单命令前收合失败: {error:?}");
                return;
            }
            if let Err(error) = this.execute_command(
                id,
                CommandSource::Menu,
                CommandTarget::Focused,
                Some(parameters.clone()),
                window,
                cx,
            ) {
                log::warn!("布局菜单命令失败: {error}");
            }
        });
    })
}

fn find_group_path(tree: &SplitTree<PanelGroupId>, target: PanelGroupId) -> Option<Vec<bool>> {
    fn walk(tree: &SplitTree<PanelGroupId>, target: PanelGroupId, path: &mut Vec<bool>) -> bool {
        match tree {
            SplitTree::Leaf(group) => *group == target,
            SplitTree::Split { first, second, .. } => {
                path.push(false);
                if walk(first, target, path) {
                    return true;
                }
                path.pop();
                path.push(true);
                if walk(second, target, path) {
                    return true;
                }
                path.pop();
                false
            }
        }
    }

    let mut path = Vec::new();
    walk(tree, target, &mut path).then_some(path)
}
