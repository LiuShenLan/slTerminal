//! `slterm_path` 提供纯 `AppPaths`、路径与授权规则。
//!
//! 本 crate 不读取环境变量，也不执行 I/O。

#![forbid(unsafe_code)]

mod app_paths;
mod policy;
pub use app_paths::{AppPaths, AppPathsError};
pub use policy::{
    CanonicalPath, Capabilities, FileIdentity, GrantPersistence, PathAuthorization, PathDenial,
    PathError, PathGrant, PathOperation, PathPolicy, PathRequest, ResolvedPath,
};
