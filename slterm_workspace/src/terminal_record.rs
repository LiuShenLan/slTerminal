//! 逻辑终端的可持久元数据；不包含 PTY、输出或回滚正文。

use std::fmt;

use slterm_path::CanonicalPath;

use crate::{ProjectId, TerminalId};

/// 关闭前是否仍有待恢复的启动意图；可见性由布局另行决定。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunIntent {
    Launch,
    Exited,
}

/// 自定义标题输入或持久记录非法。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerminalRecordError {
    EmptyTitle,
    TitleTooLong,
}

impl fmt::Display for TerminalRecordError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::EmptyTitle => "终端自定义标题不能为空",
            Self::TitleTooLong => "终端自定义标题过长",
        })
    }
}

impl std::error::Error for TerminalRecordError {}

/// 一条逻辑终端记录；编号在所属项目内稳定且不复用。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TerminalRecord {
    id: TerminalId,
    project_id: ProjectId,
    number: u64,
    cwd: CanonicalPath,
    shell: CanonicalPath,
    custom_title: Option<String>,
    run_intent: RunIntent,
}

impl TerminalRecord {
    pub fn new(
        id: TerminalId,
        project_id: ProjectId,
        number: u64,
        cwd: CanonicalPath,
        shell: CanonicalPath,
        custom_title: Option<String>,
        run_intent: RunIntent,
    ) -> Result<Self, TerminalRecordError> {
        if let Some(title) = &custom_title {
            if title.trim().is_empty() {
                return Err(TerminalRecordError::EmptyTitle);
            }
            if title.chars().count() > 128 {
                return Err(TerminalRecordError::TitleTooLong);
            }
        }
        Ok(Self {
            id,
            project_id,
            number,
            cwd,
            shell,
            custom_title,
            run_intent,
        })
    }

    pub const fn id(&self) -> TerminalId {
        self.id
    }

    pub const fn project_id(&self) -> ProjectId {
        self.project_id
    }

    pub const fn number(&self) -> u64 {
        self.number
    }

    pub fn cwd(&self) -> &CanonicalPath {
        &self.cwd
    }

    pub fn shell(&self) -> &CanonicalPath {
        &self.shell
    }

    pub fn custom_title(&self) -> Option<&str> {
        self.custom_title.as_deref()
    }

    pub const fn run_intent(&self) -> RunIntent {
        self.run_intent
    }
}
