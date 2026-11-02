//! AppRoot 渲染：只读项目快照与运行面板，不复制布局树。

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, Context, InteractiveElement as _, IntoElement, MouseButton, ParentElement as _,
    Render, Styled as _, Window, div, px, relative,
};
use gpui_component::ActiveTheme as _;
use slterm_split::{DIVIDER_GAP, SplitDirection, SplitTree};
use slterm_workspace::{LogicalSize, PanelGroupId, ProjectLayout, ViewKey};

use super::command_registry::{
    CommandId, CommandOutcome, CommandParameters, CommandSource, CommandTarget,
};
use super::project_view::ProjectView;
use super::{
    ActivateAgentFiles, ActivateCommit, ActivateExplorer, ActivateNav, AppRoot,
    CloseActiveTerminal, FocusActivityBar, NewTerminal, SplitDown, SplitRight, ToggleSidebar,
};

impl AppRoot {
    /// 分屏空间校验只使用项目内容区，不把活动栏和侧栏宽度算给面板。
    pub(super) fn project_viewport(&self, window: &Window) -> LogicalSize {
        let viewport = window.viewport_size();
        let width = f32::from(viewport.width)
            - super::sidebar::ACTIVITY_BAR_WIDTH
            - self.visible_sidebar_width(f32::from(viewport.width));
        LogicalSize::new(width.max(0.0), viewport.height.into())
    }

    /// 只铺陈活跃项目；隐藏项目的实体仍由 ProjectView 强持。
    fn render_active_project(&self, cx: &Context<'_, Self>) -> AnyElement {
        let Some(project_id) = self.snapshot.registry().active() else {
            return render_empty();
        };
        let Some(project) = self.snapshot.registry().project(project_id) else {
            return render_empty();
        };
        let Some(project_entity) = self.projects.get(&project_id) else {
            return render_empty();
        };
        let project_view = project_entity.read(cx);
        let layout = project.layout();
        let content = layout
            .tree()
            .map(|tree| self.render_split_node(tree, layout, project_view, &mut Vec::new(), cx))
            .unwrap_or_else(render_empty);
        div()
            .size_full()
            .min_w_0()
            .min_h_0()
            .track_focus(&project_view.focus_handle)
            .debug_selector(|| "app-root-content".to_owned())
            .child(content)
            .into_any_element()
    }

    fn render_split_node(
        &self,
        node: &SplitTree<PanelGroupId>,
        layout: &ProjectLayout,
        project: &ProjectView,
        path: &mut Vec<bool>,
        cx: &Context<'_, Self>,
    ) -> AnyElement {
        match node {
            SplitTree::Leaf(group_id) => render_group(*group_id, layout, project, cx),
            SplitTree::Split {
                direction,
                ratio,
                first,
                second,
                ..
            } => {
                let direction = *direction;
                let ratio = *ratio;
                path.push(false);
                let first = self.render_split_node(first, layout, project, path, cx);
                path.pop();
                path.push(true);
                let second = self.render_split_node(second, layout, project, path, cx);
                path.pop();
                let divider_path = format!("{path:?}");
                let divider = div()
                    .flex_shrink_0()
                    .debug_selector(move || format!("app-root-divider-{divider_path}"))
                    .map(|bar| match direction {
                        SplitDirection::LeftRight => bar.w(px(DIVIDER_GAP)).h_full(),
                        SplitDirection::TopBottom => bar.h(px(DIVIDER_GAP)).w_full(),
                    })
                    .bg(cx.theme().border.opacity(0.35));
                div()
                    .size_full()
                    .min_w_0()
                    .min_h_0()
                    .flex()
                    .map(|element| match direction {
                        SplitDirection::LeftRight => element.flex_row(),
                        SplitDirection::TopBottom => element.flex_col(),
                    })
                    .child(
                        div()
                            .min_w_0()
                            .min_h_0()
                            .map(|element| match direction {
                                SplitDirection::LeftRight => element.h_full().w(relative(ratio)),
                                SplitDirection::TopBottom => element.w_full().h(relative(ratio)),
                            })
                            .child(first),
                    )
                    .child(divider)
                    .child(div().flex_1().min_w_0().min_h_0().child(second))
                    .into_any_element()
            }
        }
    }
}

fn render_group(
    group_id: PanelGroupId,
    layout: &ProjectLayout,
    project: &ProjectView,
    cx: &Context<'_, AppRoot>,
) -> AnyElement {
    let active = layout.group(group_id).map(|group| group.active());
    let terminal = active.and_then(|active| {
        project
            .panels
            .values()
            .find(|panel| &panel.view_key == active)
            .map(|panel| panel.terminal.clone())
    });
    let terminal_id = active.and_then(|active| match active {
        ViewKey::Terminal { terminal_id, .. } => Some(*terminal_id),
        _ => None,
    });
    let terminal_selector = terminal
        .as_ref()
        .and_then(|_| terminal_id.map(|id| format!("app-root-terminal-{id}")));
    div()
        .size_full()
        .min_w_0()
        .min_h_0()
        .relative()
        .debug_selector(move || format!("app-root-group-{group_id}"))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, _, window, cx| {
                if let Some(terminal_id) = terminal_id
                    && let Err(error) = this.activate_terminal(terminal_id, window, cx)
                {
                    log::warn!("鼠标激活终端失败: {error}");
                }
            }),
        )
        .children(terminal.map(|terminal| {
            div()
                .size_full()
                .min_w_0()
                .min_h_0()
                .debug_selector(move || {
                    terminal_selector
                        .clone()
                        .unwrap_or_else(|| "app-root-view".to_owned())
                })
                .child(terminal)
        }))
        .into_any_element()
}

fn render_empty() -> AnyElement {
    div()
        .size_full()
        .debug_selector(|| "app-root-empty".to_owned())
        .into_any_element()
}

impl Render for AppRoot {
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        div()
            .size_full()
            .min_w_0()
            .min_h_0()
            .track_focus(&self.workspace_focus)
            .on_action(cx.listener(|this, _: &NewTerminal, window, cx| {
                let shell = this.shell.clone();
                if let Err(error) = this.execute_command(
                    CommandId::NewTerminal,
                    CommandSource::Keyboard,
                    CommandTarget::Focused,
                    Some(CommandParameters::NewTerminal {
                        shell,
                        project_root: None,
                        cwd: None,
                    }),
                    window,
                    cx,
                ) {
                    log::warn!("新建终端命令失败: {error}");
                }
            }))
            .on_action(cx.listener(|this, _: &SplitRight, window, cx| {
                let viewport = this.project_viewport(window);
                let shell = this.shell.clone();
                if let Err(error) = this.execute_command(
                    CommandId::SplitRight,
                    CommandSource::Keyboard,
                    CommandTarget::Focused,
                    Some(CommandParameters::Split {
                        ratio: 0.5,
                        viewport,
                        shell,
                    }),
                    window,
                    cx,
                ) {
                    log::warn!("横向拆分终端命令失败: {error}");
                }
            }))
            .on_action(cx.listener(|this, _: &SplitDown, window, cx| {
                let viewport = this.project_viewport(window);
                let shell = this.shell.clone();
                if let Err(error) = this.execute_command(
                    CommandId::SplitDown,
                    CommandSource::Keyboard,
                    CommandTarget::Focused,
                    Some(CommandParameters::Split {
                        ratio: 0.5,
                        viewport,
                        shell,
                    }),
                    window,
                    cx,
                ) {
                    log::warn!("纵向拆分终端命令失败: {error}");
                }
            }))
            .on_action(cx.listener(|this, _: &ToggleSidebar, window, cx| {
                if let Err(error) = this.execute_command(
                    CommandId::ToggleSidebar,
                    CommandSource::Keyboard,
                    CommandTarget::Focused,
                    None,
                    window,
                    cx,
                ) {
                    log::warn!("切换侧栏命令失败: {error}");
                }
            }))
            .on_action(cx.listener(|this, _: &ActivateNav, window, cx| {
                if let Err(error) = this.execute_command(
                    CommandId::ActivateSideView,
                    CommandSource::Keyboard,
                    CommandTarget::SideView(slterm_workspace::SideViewId::Nav),
                    None,
                    window,
                    cx,
                ) {
                    log::warn!("激活导航视图命令失败: {error}");
                }
            }))
            .on_action(cx.listener(|this, _: &ActivateExplorer, window, cx| {
                if let Err(error) = this.execute_command(
                    CommandId::ActivateSideView,
                    CommandSource::Keyboard,
                    CommandTarget::SideView(slterm_workspace::SideViewId::Explorer),
                    None,
                    window,
                    cx,
                ) {
                    log::warn!("激活文件视图命令失败: {error}");
                }
            }))
            .on_action(cx.listener(|this, _: &ActivateCommit, window, cx| {
                if let Err(error) = this.execute_command(
                    CommandId::ActivateSideView,
                    CommandSource::Keyboard,
                    CommandTarget::SideView(slterm_workspace::SideViewId::Commit),
                    None,
                    window,
                    cx,
                ) {
                    log::warn!("激活提交视图命令失败: {error}");
                }
            }))
            .on_action(cx.listener(|this, _: &ActivateAgentFiles, window, cx| {
                if let Err(error) = this.execute_command(
                    CommandId::ActivateSideView,
                    CommandSource::Keyboard,
                    CommandTarget::SideView(slterm_workspace::SideViewId::AgentFiles),
                    None,
                    window,
                    cx,
                ) {
                    log::warn!("激活 Agent 文件视图命令失败: {error}");
                }
            }))
            .on_action(cx.listener(|this, _: &FocusActivityBar, window, cx| {
                if let Err(error) = this.execute_command(
                    CommandId::FocusActivityBar,
                    CommandSource::Keyboard,
                    CommandTarget::Focused,
                    None,
                    window,
                    cx,
                ) {
                    log::warn!("聚焦活动栏命令失败: {error}");
                }
            }))
            .on_action(cx.listener(|this, _: &CloseActiveTerminal, window, cx| {
                match this.execute_command(
                    CommandId::CloseActiveTerminal,
                    CommandSource::Keyboard,
                    CommandTarget::Focused,
                    None,
                    window,
                    cx,
                ) {
                    Ok(CommandOutcome::TerminalClosed(record)) => drop(record),
                    Ok(_) => unreachable!("关闭终端命令结果类型不匹配"),
                    Err(error) => log::warn!("关闭聚焦终端命令失败: {error}"),
                }
            }))
            .bg(cx.theme().background)
            .flex()
            .flex_row()
            .child(self.render_activity_bar(cx))
            .children(self.render_sidebar(window, cx))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .child(self.render_active_project(cx)),
            )
            .children(self.render_sidebar_drag_overlay(cx))
    }
}
