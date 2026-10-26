//! `slterm_workspace` 提供稳定 ID、`ViewKey`、`ViewState`、布局 DTO 与关闭恢复的纯状态机。

#![forbid(unsafe_code)]

mod diff_binding;
mod identity;
mod panel;
mod project_layout;
#[cfg(test)]
mod project_layout_tests;
mod repository_dto;
mod side_view;
mod view_key;
mod view_state;
pub use diff_binding::{
    DiffBindingError, diff_binding_from_canonical_json, diff_binding_to_canonical_json,
};
pub use identity::{PanelGroupId, ProjectId, StableIdError, TerminalId};
pub use panel::{PanelId, PanelIdAllocator, PanelIdExhausted};
pub use project_layout::{LayoutDtoError, LayoutError, LogicalSize, PanelGroup, ProjectLayout};
pub use side_view::{SideViewId, UnknownSideViewId};
pub use view_key::{ViewKey, ViewKeyError, ViewKeyKind};
pub use view_state::{
    InvalidViewState, ViewState, ViewStateError, ViewStateRegistry, ViewStateRegistryError,
    ViewStateSchema, ViewStateScope,
};
