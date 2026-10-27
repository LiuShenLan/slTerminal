//! 生产 SideView 实体身份。

use slterm_workspace::SideViewId;

/// 应用级侧栏视图实体，以变体区分封闭视图身份。
pub(crate) enum SideView {
    Nav,
    Explorer,
    Commit,
    AgentFiles,
}

impl SideView {
    pub(crate) const fn new(id: SideViewId) -> Self {
        match id {
            SideViewId::Nav => Self::Nav,
            SideViewId::Explorer => Self::Explorer,
            SideViewId::Commit => Self::Commit,
            SideViewId::AgentFiles => Self::AgentFiles,
        }
    }
}
