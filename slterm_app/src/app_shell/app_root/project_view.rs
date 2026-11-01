//! 项目运行视图：只持面板实例、项目焦点容器与项目内瞬时焦点。

use std::collections::BTreeMap;

use gpui::{Context, FocusHandle};
use slterm_workspace::PanelId;

use super::focus_manager::FocusRegistration;
use super::panel_host::PanelHost;

pub(crate) struct ProjectView {
    pub(crate) panels: BTreeMap<PanelId, PanelHost>,
    pub(crate) focused_panel: Option<PanelId>,
    pub(super) focus_handle: FocusHandle,
    pub(super) focus_registration: Option<FocusRegistration>,
}

impl ProjectView {
    pub(super) fn new(cx: &mut Context<'_, Self>) -> Self {
        Self {
            panels: BTreeMap::new(),
            focused_panel: None,
            focus_handle: cx.focus_handle(),
            focus_registration: None,
        }
    }
}
