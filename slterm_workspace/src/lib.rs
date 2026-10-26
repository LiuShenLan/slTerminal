//! `slterm_workspace` 提供稳定 ID、`ViewKey`、布局 DTO 与关闭恢复的纯状态机。

#![forbid(unsafe_code)]

mod identity;
mod panel;
mod side_view;
mod view_key;
pub use identity::{PanelGroupId, ProjectId, StableIdError, TerminalId};
pub use panel::{PanelId, PanelIdAllocator, PanelIdExhausted};
pub use side_view::{SideViewId, UnknownSideViewId};
pub use view_key::{ViewKey, ViewKeyError};
