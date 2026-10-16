//! 平台适配层(Windows-only;unix/macOS 臂随定位约束消亡,不撒 cfg)。
//!
//! 业务 `#[cfg(windows)]` 仅允许出现在 slterm_terminal 的 pty 模块(硬约束 9);
//! 本层是 Windows 原生代码的直接落点,不再带平台分支。

pub mod dirs;
pub(crate) mod keyboard;
pub(crate) mod startup;
