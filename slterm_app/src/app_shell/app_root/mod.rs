//! 项目工作区运行根：唯一持有项目快照与全部项目面板实例。

mod commands;
mod panel_host;
mod project_view;
#[cfg(all(test, feature = "gpui-test-support"))]
mod tests;

use std::collections::BTreeMap;

use gpui::{Entity, Subscription};
use slterm_workspace::{PanelId, PanelIdAllocator, ProjectId, ProjectSnapshot};

pub(crate) use commands::TerminalPanelLauncher;

/// 单窗口运行根；`projects` 只是强实体引用表，不复制项目注册信息。
pub(crate) struct AppRoot {
    snapshot: ProjectSnapshot,
    projects: BTreeMap<ProjectId, Entity<ProjectView>>,
    panel_ids: PanelIdAllocator,
    panel_subscriptions: BTreeMap<PanelId, Subscription>,
    launcher: Box<dyn TerminalPanelLauncher>,
}

impl AppRoot {
    #[allow(dead_code)] // 生产窗口切换时消费。
    pub(crate) fn new(panel_ids: PanelIdAllocator) -> Self {
        Self::with_launcher(panel_ids, Box::new(commands::LocalTerminalLauncher))
    }

    #[allow(dead_code)] // 测试启动器注入；生产窗口切换时消费。
    fn with_launcher(
        panel_ids: PanelIdAllocator,
        launcher: Box<dyn TerminalPanelLauncher>,
    ) -> Self {
        Self {
            snapshot: ProjectSnapshot::default(),
            projects: BTreeMap::new(),
            panel_ids,
            panel_subscriptions: BTreeMap::new(),
            launcher,
        }
    }

    #[allow(dead_code)] // 生产快照保存与窗口切换时消费。
    pub(crate) const fn snapshot(&self) -> &ProjectSnapshot {
        &self.snapshot
    }
}

use project_view::ProjectView;
