//! `slterm_sidebar` 提供纯文件树与侧栏状态。

#![forbid(unsafe_code)]

mod sidebar_state;
#[cfg(test)]
mod sidebar_state_tests;

pub use sidebar_state::{ButtonMovement, SideSlot, SidebarError, SidebarState, SidebarTransition};
