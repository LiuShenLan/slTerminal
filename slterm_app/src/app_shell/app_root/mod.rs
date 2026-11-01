//! 项目工作区运行根：唯一持有项目快照与全部项目面板实例。

mod commands;
mod events;
#[allow(dead_code)] // 焦点所有权已迁移，生产订阅由后续焦点接线装配。
mod focus_manager;
pub(crate) mod keymap;
mod panel_host;
mod project_view;
mod render;
#[allow(dead_code)] // SideView 持久状态已迁移，生产宿主由后续侧栏接线装配。
mod side_view;
#[allow(dead_code)] // 实例转换与关闭语义由 side_view 及后续生产宿主消费。
mod side_view_host;
#[cfg(all(test, feature = "gpui-test-support"))]
mod tests;

use std::collections::BTreeMap;

use gpui::{Entity, Subscription};
use slterm_path::CanonicalPath;
use slterm_workspace::{PanelId, PanelIdAllocator, ProjectId, ProjectSnapshot};

pub(crate) use commands::TerminalPanelLauncher;

gpui::actions!(
    slterm_app_root,
    [NewTerminal, SplitRight, SplitDown, CloseActiveTerminal]
);

/// 单窗口运行根；`projects` 只是强实体引用表，不复制项目注册信息。
pub(crate) struct AppRoot {
    snapshot: ProjectSnapshot,
    projects: BTreeMap<ProjectId, Entity<ProjectView>>,
    panel_ids: PanelIdAllocator,
    panel_subscriptions: BTreeMap<PanelId, Subscription>,
    launcher: Box<dyn TerminalPanelLauncher>,
    shell: CanonicalPath,
}

impl AppRoot {
    pub(crate) fn new(panel_ids: PanelIdAllocator, shell: CanonicalPath) -> Self {
        Self::with_launcher(panel_ids, shell, Box::new(commands::LocalTerminalLauncher))
    }

    fn with_launcher(
        panel_ids: PanelIdAllocator,
        shell: CanonicalPath,
        launcher: Box<dyn TerminalPanelLauncher>,
    ) -> Self {
        Self {
            snapshot: ProjectSnapshot::default(),
            projects: BTreeMap::new(),
            panel_ids,
            panel_subscriptions: BTreeMap::new(),
            launcher,
            shell,
        }
    }

    #[allow(dead_code)] // 生产快照保存与窗口切换时消费。
    pub(crate) const fn snapshot(&self) -> &ProjectSnapshot {
        &self.snapshot
    }
}

use project_view::ProjectView;
