//! `slterm_path` 提供纯 `AppPaths`、路径与授权规则。
//!
//! 本 crate 不读取环境变量，也不执行 I/O。

#![forbid(unsafe_code)]

mod app_paths;
pub use app_paths::{AppPaths, AppPathsError};
