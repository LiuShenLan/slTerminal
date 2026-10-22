//! `runtime.handover` 线缆类型、版本常量与平台无关校验。
//!
//! 单实例移交一次只含一个 [`HandoffOp`]；载荷以根路径/cwd 定位目标，不携带
//! `ProjectId`/`PanelId` 等内部 ID。类型只借用调用方的字符串，不引入 serde
//! 等生产依赖；Windows 路径合法性归平台层校验。

use std::fmt;

/// 当前移交请求 schema 版本；接收端拒绝不匹配的版本。
pub const HANDOFF_SCHEMA_VERSION: u16 = 1;

/// 机器可读 JSON Schema（2020-12），与 [`HandoffRequest`] 字段一一对应。
pub const HANDOFF_SCHEMA_JSON: &str = include_str!("handover.schema.json");

/// 内部资源 ID 前缀；路径字段出现这些前缀即视为把 ID 当路径传。
const INTERNAL_ID_PREFIXES: [&str; 3] = ["project:", "group:", "panel:"];

/// 单实例移交请求。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HandoffRequest<'a> {
    /// schema 版本，必须等于 [`HANDOFF_SCHEMA_VERSION`]。
    pub schema: u16,
    /// 幂等键：接收端按它缓存首次结果，同一键不重复执行。
    pub request_id: &'a str,
    /// 本次移交的唯一操作。
    pub op: HandoffOp<'a>,
}

/// 移交操作：一次请求只允许其中一个，线缆以 `kind` 判别。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandoffOp<'a> {
    /// 聚焦已有窗口/项目（`kind = "focus"`）。
    Focus,
    /// 打开已登记项目（`kind = "open_project"`）；`project_root` 是项目根路径，
    /// 不是内部资源 ID。
    OpenProject {
        /// 已登记项目根的路径。
        project_root: &'a str,
    },
    /// 打开文件（`kind = "open_file"`）。
    OpenFile {
        /// 所属项目根路径；省略时按活跃项目/已登记根最深匹配。
        project_root: Option<&'a str>,
        /// 文件路径，不是内部资源 ID。
        path: &'a str,
    },
    /// 新建终端（`kind = "new_terminal"`）。
    NewTerminal {
        /// 目标项目根路径；省略时使用活跃项目。
        project_root: Option<&'a str>,
        /// 新终端工作目录路径；省略时使用项目默认工作目录。
        cwd: Option<&'a str>,
    },
}

/// 移交结果：按 `request_id` 去重后返回首次结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandoffReply<'a> {
    /// 接收端接受并完成了请求（线缆 `kind = "accepted"`）。
    Accepted {
        /// 对应请求的幂等键。
        request_id: &'a str,
    },
    /// 接收端拒绝请求（线缆 `kind = "rejected"`）。
    Rejected {
        /// 对应请求的幂等键。
        request_id: &'a str,
        /// 拒绝原因。
        reason: HandoffReject,
    },
    /// 接收端正忙，可稍后重试（线缆 `kind = "busy"`）。
    Busy {
        /// 对应请求的幂等键。
        request_id: &'a str,
    },
}

/// 移交拒绝原因，线缆取 snake_case 字符串。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandoffReject {
    /// schema 版本未知。
    UnknownSchema,
    /// 路径非法（平台层判定）。
    InvalidPath,
    /// 目标项目未登记。
    ProjectNotFound,
    /// 目标项目尚未就绪。
    ProjectNotReady,
    /// PTY 数量达到上限。
    PtyLimit,
    /// 请求的操作不受支持。
    Unsupported,
}

/// 平台无关的载荷校验失败原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandoffValidationError {
    /// schema 与当前版本不一致。
    UnknownSchema,
    /// `request_id` 为空。
    EmptyRequestId,
    /// `project_root` 为空。
    EmptyProjectRoot,
    /// `path` 为空。
    EmptyPath,
    /// `cwd` 为空。
    EmptyCwd,
    /// 路径字段以内部资源 ID 前缀（`project:`/`group:`/`panel:`）开头。
    InternalId,
}

impl fmt::Display for HandoffValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::UnknownSchema => "handover schema version is unsupported",
            Self::EmptyRequestId => "handover request_id is empty",
            Self::EmptyProjectRoot => "handover project_root is empty",
            Self::EmptyPath => "handover path is empty",
            Self::EmptyCwd => "handover cwd is empty",
            Self::InternalId => "handover path field contains an internal resource ID",
        };
        f.write_str(message)
    }
}

impl std::error::Error for HandoffValidationError {}

impl HandoffRequest<'_> {
    /// 校验线缆载荷中与平台无关的约束。
    ///
    /// `schema` 必须等于 [`HANDOFF_SCHEMA_VERSION`]；`request_id` 与提供的
    /// 路径字段必须非空，且路径字段不得是内部资源 ID。Windows 路径合法性
    /// 由平台层校验，本函数不判断。
    pub fn validate(&self) -> Result<(), HandoffValidationError> {
        if self.schema != HANDOFF_SCHEMA_VERSION {
            return Err(HandoffValidationError::UnknownSchema);
        }
        if self.request_id.is_empty() {
            return Err(HandoffValidationError::EmptyRequestId);
        }

        match &self.op {
            HandoffOp::Focus => Ok(()),
            HandoffOp::OpenProject { project_root } => {
                validate_path(project_root, HandoffValidationError::EmptyProjectRoot)
            }
            HandoffOp::OpenFile { project_root, path } => {
                if let Some(project_root) = project_root {
                    validate_path(project_root, HandoffValidationError::EmptyProjectRoot)?;
                }
                validate_path(path, HandoffValidationError::EmptyPath)
            }
            HandoffOp::NewTerminal { project_root, cwd } => {
                if let Some(project_root) = project_root {
                    validate_path(project_root, HandoffValidationError::EmptyProjectRoot)?;
                }
                if let Some(cwd) = cwd {
                    validate_path(cwd, HandoffValidationError::EmptyCwd)?;
                }
                Ok(())
            }
        }
    }
}

/// 校验单个路径字段：非空且不是内部资源 ID 形态。
fn validate_path(
    value: &str,
    empty_error: HandoffValidationError,
) -> Result<(), HandoffValidationError> {
    if value.is_empty() {
        return Err(empty_error);
    }
    if INTERNAL_ID_PREFIXES
        .iter()
        .any(|prefix| value.starts_with(prefix))
    {
        return Err(HandoffValidationError::InternalId);
    }
    Ok(())
}
