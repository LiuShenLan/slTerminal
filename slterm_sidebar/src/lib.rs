//! `slterm_sidebar` 提供纯文件树与侧栏状态。

#![forbid(unsafe_code)]

mod side_view_state;
#[cfg(test)]
mod side_view_state_tests;
mod sidebar_state;
#[cfg(test)]
mod sidebar_state_dto_tests;
#[cfg(test)]
mod sidebar_state_recovery_tests;
#[cfg(test)]
mod sidebar_state_tests;

pub use side_view_state::{SideViewState, side_view_state_schema};
pub use sidebar_state::{
    ButtonMovement, SideSlot, SidebarDtoError, SidebarError, SidebarRecovery, SidebarRecoveryError,
    SidebarRecoveryWarning, SidebarState, SidebarTransition,
};
