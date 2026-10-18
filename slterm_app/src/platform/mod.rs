//! 平台适配层(Windows-only;unix/macOS 臂随定位约束消亡,不撒 cfg)。
//!
//! 业务 `#[cfg(windows)]` 仅允许出现在 slterm_terminal 的 pty 模块(硬约束 9);
//! 本层是 Windows 原生代码的直接落点,不再带平台分支。

pub mod dirs;
pub(crate) mod elevation;
pub(crate) mod keyboard;
// 系统 locale 探测源文件(src/platform/locale.rs)经 `i18n::locale` 的
// `#[path]` 复编进查询核心;此处不再二次挂模块(独立合同 workspace 复编
// 同一份实现,不引入应用壳)。
pub(crate) mod process;
pub(crate) mod process_snapshot;
pub(crate) mod startup;
