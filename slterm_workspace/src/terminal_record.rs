//! 逻辑终端的可持久元数据；不包含 PTY、输出或回滚正文。

use std::fmt;

use slterm_agents::AgentSession;
use slterm_path::CanonicalPath;

use crate::{ProjectId, TerminalId};

/// 关闭前是否仍有待恢复的启动意图；可见性由布局另行决定。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunIntent {
    Launch,
    Exited,
}

/// 与进程生命无关的用户标题和 Agent 恢复元数据。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TerminalMetadata {
    custom_title: Option<String>,
    agent: Option<AgentSession>,
}

impl TerminalMetadata {
    pub fn new(custom_title: Option<String>, agent: Option<AgentSession>) -> Self {
        Self {
            custom_title,
            agent,
        }
    }
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
    agent: Option<AgentSession>,
    run_intent: RunIntent,
}

impl TerminalRecord {
    pub fn new(
        id: TerminalId,
        project_id: ProjectId,
        number: u64,
        cwd: CanonicalPath,
        shell: CanonicalPath,
        metadata: TerminalMetadata,
        run_intent: RunIntent,
    ) -> Result<Self, TerminalRecordError> {
        let TerminalMetadata {
            custom_title,
            agent,
        } = metadata;
        validate_custom_title(custom_title.as_deref())?;
        Ok(Self {
            id,
            project_id,
            number,
            cwd,
            shell,
            custom_title,
            agent,
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

    pub fn agent(&self) -> Option<&AgentSession> {
        self.agent.as_ref()
    }

    pub const fn run_intent(&self) -> RunIntent {
        self.run_intent
    }

    pub(crate) fn set_agent(&mut self, agent: Option<AgentSession>) {
        self.agent = agent;
    }

    pub(crate) fn set_run_intent(&mut self, run_intent: RunIntent) {
        self.run_intent = run_intent;
    }

    /// 更新自定义标题；空白与超长输入保持原记录不变。
    pub(crate) fn set_custom_title(
        &mut self,
        custom_title: Option<String>,
    ) -> Result<(), TerminalRecordError> {
        validate_custom_title(custom_title.as_deref())?;
        self.custom_title = custom_title;
        Ok(())
    }
}

/// 自定义标题只拒绝空白与超过 128 个 Unicode 字符。
fn validate_custom_title(custom_title: Option<&str>) -> Result<(), TerminalRecordError> {
    if let Some(title) = custom_title {
        if title.trim().is_empty() {
            return Err(TerminalRecordError::EmptyTitle);
        }
        if title.chars().count() > 128 {
            return Err(TerminalRecordError::TitleTooLong);
        }
    }
    Ok(())
}
