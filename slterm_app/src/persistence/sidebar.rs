//! 应用级侧栏组织状态的 SQLite 单通道；规范 JSON 只由 core 编解码。

use std::fmt;

use rusqlite::{OptionalExtension as _, TransactionBehavior};
use slterm_sidebar::{SidebarDtoError, SidebarRecovery, SidebarRecoveryError, SidebarState};

use super::database::DatabaseError;
use super::workspace::WorkspaceDatabase;

/// 侧栏组织状态读写失败；展示与日志不包含原始 JSON。
#[derive(Debug)]
pub(crate) enum SidebarStateStoreError {
    Database(DatabaseError),
    Dto(SidebarDtoError),
    Recovery(SidebarRecoveryError),
}

impl fmt::Display for SidebarStateStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(_) => formatter.write_str("侧栏组织状态数据库访问失败"),
            Self::Dto(_) => formatter.write_str("侧栏组织状态编码失败"),
            Self::Recovery(error) => fmt::Display::fmt(error, formatter),
        }
    }
}

impl std::error::Error for SidebarStateStoreError {}

impl From<DatabaseError> for SidebarStateStoreError {
    fn from(error: DatabaseError) -> Self {
        Self::Database(error)
    }
}

impl From<rusqlite::Error> for SidebarStateStoreError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Database(error.into())
    }
}

impl From<SidebarDtoError> for SidebarStateStoreError {
    fn from(error: SidebarDtoError) -> Self {
        Self::Dto(error)
    }
}

impl From<SidebarRecoveryError> for SidebarStateStoreError {
    fn from(error: SidebarRecoveryError) -> Self {
        Self::Recovery(error)
    }
}

impl WorkspaceDatabase {
    /// 单行 upsert 只更新侧栏列，不重写活跃项目或项目布局。
    pub(crate) fn save_sidebar_state(
        &mut self,
        state: &SidebarState,
    ) -> Result<(), SidebarStateStoreError> {
        let json = state.to_canonical_json()?;
        let transaction = self
            .database
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        transaction.execute(
            "INSERT INTO app_state (singleton, active_project_id, sidebar_json) \
             VALUES (1, NULL, ?1) \
             ON CONFLICT(singleton) DO UPDATE SET sidebar_json = excluded.sidebar_json",
            [json],
        )?;
        transaction.commit()?;
        Ok(())
    }

    /// 无记录与整体损坏分流；字段级修复连同警告交给启动恢复汇总。
    pub(crate) fn sidebar_state(&self) -> Result<Option<SidebarRecovery>, SidebarStateStoreError> {
        let json = self
            .database
            .connection
            .query_row(
                "SELECT sidebar_json FROM app_state WHERE singleton = 1",
                [],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()?
            .flatten();
        json.map(|json| SidebarRecovery::from_json(&json).map_err(Into::into))
            .transpose()
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr as _;

    use slterm_path::AppPaths;
    use slterm_workspace::{ProjectId, ProjectRegistry, ProjectSnapshot};

    use super::*;

    #[test]
    fn sidebar_and_project_writes_preserve_each_other() {
        let temp = tempfile::tempdir().unwrap();
        let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
        let project_id = ProjectId::from_str("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2a").unwrap();
        let mut registry = ProjectRegistry::new();
        registry
            .add_project(
                project_id,
                slterm_path::CanonicalPath::new("D:/work/sidebar").unwrap(),
            )
            .unwrap();
        let snapshot = ProjectSnapshot::restore(registry, Vec::new()).unwrap();
        let mut store = WorkspaceDatabase::open(&paths).unwrap();
        store.save_project_snapshot(&snapshot).unwrap();

        let mut sidebar = SidebarState::default();
        sidebar.set_width(333);
        store.save_sidebar_state(&sidebar).unwrap();
        assert_eq!(store.load_project_snapshot().unwrap(), snapshot);

        store.save_project_snapshot(&snapshot).unwrap();
        assert_eq!(store.sidebar_state().unwrap().unwrap().state, sidebar);
        assert_eq!(store.load_project_snapshot().unwrap(), snapshot);
    }
}
