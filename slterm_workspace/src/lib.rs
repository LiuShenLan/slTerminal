//! `slterm_workspace` 提供稳定 ID、`ViewKey`、布局 DTO 与关闭恢复的纯状态机。

#![forbid(unsafe_code)]

mod identity;
pub use identity::{PanelGroupId, ProjectId, StableIdError, TerminalId};
