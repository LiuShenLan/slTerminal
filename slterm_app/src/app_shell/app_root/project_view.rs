//! 项目运行视图：只持面板实例与项目内瞬时焦点。

use std::collections::BTreeMap;

use slterm_workspace::PanelId;

use super::panel_host::PanelHost;

#[derive(Default)]
pub(crate) struct ProjectView {
    pub(crate) panels: BTreeMap<PanelId, PanelHost>,
    pub(crate) focused_panel: Option<PanelId>,
}
