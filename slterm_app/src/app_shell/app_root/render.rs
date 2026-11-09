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
    ActivateAgentFiles, ActivateCommit, ActivateExplorer, ActivateNav, AdjustSplitDown,
    AdjustSplitLeft, AdjustSplitRight, AdjustSplitUp, AppRoot, CloseActiveTerminal,
    FocusActivityBar, FocusNextPanel, FocusPreviousPanel, NewTerminal, OpenLayoutMenu,
    OpenSettings, SplitDown, SplitRight, ToggleSidebar,
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
            .map(|tree| {
                self.render_split_node(tree, project_id, layout, project_view, &mut Vec::new(), cx)
            })
            .unwrap_or_else(render_empty);
        div()
            .size_full()
            .min_w_0()
            .min_h_0()
            .track_focus(&project_view.focus_handle)
            .key_context(super::layout::PROJECT_LAYOUT_KEY_CONTEXT)
            .debug_selector(|| "app-root-content".to_owned())
            .child(content)
            .into_any_element()
    }

    fn render_split_node(
        &self,
        node: &SplitTree<PanelGroupId>,
        project_id: slterm_workspace::ProjectId,
        layout: &ProjectLayout,
        project: &ProjectView,
        path: &mut Vec<bool>,
        cx: &Context<'_, Self>,
    ) -> AnyElement {
        match node {
            SplitTree::Leaf(group_id) => self.render_group(*group_id, layout, project, cx),
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
                let first = self.render_split_node(first, project_id, layout, project, path, cx);
                path.pop();
                path.push(true);
                let second = self.render_split_node(second, project_id, layout, project, path, cx);
                path.pop();
                let divider_path = format!("{path:?}");
                let split_path = path.clone();
                let divider_focus = self
                    .split_dividers
                    .get(&(project_id, split_path.clone()))
                    .expect("渲染前必须已注册当前分隔线")
                    .handle
                    .clone();
                let left_path = split_path.clone();
                let right_path = split_path.clone();
                let up_path = split_path.clone();
                let down_path = split_path.clone();
                let divider = div()
                    .flex_shrink_0()
                    .track_focus(&divider_focus)
                    .key_context(super::layout::SPLIT_DIVIDER_KEY_CONTEXT)
                    .focus_visible(|style| style.bg(cx.theme().accent))
                    .on_action(cx.listener(move |this, _: &AdjustSplitLeft, window, cx| {
                        this.resize_split_action(left_path.clone(), ratio, -0.02, window, cx);
                    }))
                    .on_action(cx.listener(move |this, _: &AdjustSplitRight, window, cx| {
                        this.resize_split_action(right_path.clone(), ratio, 0.02, window, cx);
                    }))
                    .on_action(cx.listener(move |this, _: &AdjustSplitUp, window, cx| {
                        this.resize_split_action(up_path.clone(), ratio, -0.02, window, cx);
                    }))
                    .on_action(cx.listener(move |this, _: &AdjustSplitDown, window, cx| {
                        this.resize_split_action(down_path.clone(), ratio, 0.02, window, cx);
                    }))
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

impl AppRoot {
    fn render_group(
        &self,
        group_id: PanelGroupId,
        layout: &ProjectLayout,
        project: &ProjectView,
        cx: &Context<'_, AppRoot>,
    ) -> AnyElement {
        let active = layout.group(group_id).map(|group| group.active());
        let content = active.and_then(|active| {
            project
                .panels
                .values()
                .find(|panel| &panel.view_key == active)
                .map(|panel| match &panel.content {
                    super::panel_host::PanelContent::Terminal(terminal) => {
                        terminal.clone().into_any_element()
                    }
                    super::panel_host::PanelContent::Editor(editor) => {
                        editor.clone().into_any_element()
                    }
                })
        });
        let terminal_id = active.and_then(|active| match active {
            ViewKey::Terminal { terminal_id, .. } => Some(*terminal_id),
            _ => None,
        });
        let terminal_selector = terminal_id.map(|id| format!("app-root-terminal-{id}"));
        let tab_bar = self.render_tab_bar(layout.project_id(), group_id, layout, cx);
        div()
            .size_full()
            .min_w_0()
            .min_h_0()
            .relative()
            .flex()
            .flex_col()
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
            .child(tab_bar)
            .children(content.map(|content| {
                div()
                    .flex_1()
                    .min_w_0()
                    .min_h_0()
                    .debug_selector(move || {
                        terminal_selector
                            .clone()
                            .unwrap_or_else(|| "app-root-view".to_owned())
                    })
                    .child(content)
            }))
            .children(self.render_layout_menu(group_id))
            .into_any_element()
    }
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
            .on_action(cx.listener(|this, _: &OpenSettings, window, cx| {
                if let Err(error) = this.execute_command(
                    CommandId::OpenSettings,
                    CommandSource::Keyboard,
                    CommandTarget::Focused,
                    None,
                    window,
                    cx,
                ) {
                    log::warn!("打开设置浮层命令失败: {error}");
                }
            }))
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
            .on_action(cx.listener(|this, _: &OpenLayoutMenu, window, cx| {
                if let Err(error) = this.execute_command(
                    CommandId::OpenLayoutMenu,
                    CommandSource::Keyboard,
                    CommandTarget::Focused,
                    None,
                    window,
                    cx,
                ) {
                    log::warn!("打开布局菜单失败: {error}");
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
            .on_action(cx.listener(|this, _: &FocusNextPanel, window, cx| {
                if let Err(error) = this.execute_command(
                    CommandId::FocusNextPanel,
                    CommandSource::Keyboard,
                    CommandTarget::Focused,
                    None,
                    window,
                    cx,
                ) {
                    log::warn!("循环到下一面板失败: {error}");
                }
            }))
            .on_action(cx.listener(|this, _: &FocusPreviousPanel, window, cx| {
                if let Err(error) = this.execute_command(
                    CommandId::FocusPreviousPanel,
                    CommandSource::Keyboard,
                    CommandTarget::Focused,
                    None,
                    window,
                    cx,
                ) {
                    log::warn!("循环到上一面板失败: {error}");
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
            .children(self.render_settings_overlay(cx))
    }
}
