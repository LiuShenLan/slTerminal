//! 启动维护期协调：只消费 persistence 的规范事实。

pub(crate) mod abandon;
pub(crate) mod maintenance;
pub(crate) mod recovery;

#[allow(unused_imports)] // 生产消费点归诊断窗后台放弃入口。
pub(crate) use abandon::{AbandonOutcome, abandon_maintenance_blockers};
#[allow(unused_imports)] // 生产消费点归后续启动链。
pub(crate) use maintenance::{
    JournalRecoverer, JournalRecovererRegistry, MaintenanceError, MaintenanceOutcome,
    MaintenanceRequestError, MaintenanceRequestRecoverer, MaintenanceRequestRecovererRegistry,
    MaintenanceRunner, MaintenanceStage,
};
pub(crate) use recovery::{
    StartupRecovery, WorkspaceOpenError, WorkspaceRecoveryNotice, open_workspace_with_recovery,
};
