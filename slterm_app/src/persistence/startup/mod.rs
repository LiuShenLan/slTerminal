//! 启动维护期协调：只消费 persistence 的规范事实。

pub(crate) mod maintenance;
pub(crate) mod recovery;

#[allow(unused_imports)] // 生产消费点归后续启动链。
pub(crate) use maintenance::{
    JournalRecoverer, JournalRecovererRegistry, MaintenanceError, MaintenanceOutcome,
    MaintenanceRunner, MaintenanceStage,
};
pub(crate) use recovery::{
    StartupRecovery, WorkspaceOpenError, WorkspaceRecoveryNotice, open_workspace_with_recovery,
};
