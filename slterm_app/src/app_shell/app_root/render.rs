//! AppRoot 渲染：只读项目快照与运行面板，不复制布局树。

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, Context, InteractiveElement as _, IntoElement, ParentElement as _, Render,
    Styled as _, Window, div, px, relative,
};
use gpui_component::ActiveTheme as _;
use slterm_split::{DIVIDER_GAP, SplitDirection, SplitTree};
use slterm_workspace::{PanelGroupId, ProjectLayout, ViewKey};

use super::AppRoot;
use super::project_view::ProjectView;

impl AppRoot {
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
        let Some(tree) = layout.tree() else {
            return render_empty();
        };
        div()
            .size_full()
            .min_w_0()
            .min_h_0()
            .debug_selector(|| "app-root-content".to_owned())
            .child(self.render_split_node(tree, layout, project_view, &mut Vec::new(), cx))
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
            SplitTree::Leaf(group_id) => render_group(*group_id, layout, project),
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
) -> AnyElement {
    let active = layout.group(group_id).map(|group| group.active());
    let terminal = active.and_then(|active| {
        project
            .panels
            .values()
            .find(|panel| &panel.view_key == active)
            .map(|panel| panel.terminal.clone())
    });
    let terminal_selector = terminal.as_ref().and_then(|_| match active {
        Some(ViewKey::Terminal { terminal_id, .. }) => {
            Some(format!("app-root-terminal-{terminal_id}"))
        }
        _ => None,
    });
    div()
        .size_full()
        .min_w_0()
        .min_h_0()
        .relative()
        .debug_selector(move || format!("app-root-group-{group_id}"))
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
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        div()
            .size_full()
            .min_w_0()
            .min_h_0()
            .bg(cx.theme().background)
            .child(self.render_active_project(cx))
    }
}
