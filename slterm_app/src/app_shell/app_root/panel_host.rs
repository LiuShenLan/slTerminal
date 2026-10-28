//! 运行面板宿主：只绑定运行身份、稳定视图键与终端实体。

use gpui::Entity;
use slterm_workspace::{PanelId, ViewKey};

use crate::gpui_terminal::view::TerminalView;

pub(crate) struct PanelHost {
    #[allow(dead_code)] // 新根渲染与命令接线随即消费。
    pub(crate) id: PanelId,
    pub(crate) view_key: ViewKey,
    pub(crate) terminal: Entity<TerminalView>,
}
