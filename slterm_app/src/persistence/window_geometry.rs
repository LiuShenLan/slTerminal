//! 窗口几何的按机器单通道：machine_key 主键的行内规范 JSON。
//!
//! 几何是纯展示偏好，损坏时按无记录处理，不阻断启动或其他持久事实。

use std::fmt;

use rusqlite::OptionalExtension as _;

use super::database::DatabaseError;
use super::workspace::WorkspaceDatabase;
use crate::windowing::geometry::WindowGeometry;

/// 当前几何 DTO 版本；超前版本不解读。
const WINDOW_GEOMETRY_DTO_VERSION: i64 = 1;

/// 窗口几何读写失败；展示与日志不包含原始 JSON。
#[derive(Debug)]
pub(crate) enum WindowGeometryStoreError {
    Database(DatabaseError),
    Encode,
}

impl fmt::Display for WindowGeometryStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(_) => formatter.write_str("窗口几何数据库访问失败"),
            Self::Encode => formatter.write_str("窗口几何编码失败"),
        }
    }
}

impl std::error::Error for WindowGeometryStoreError {}

impl From<DatabaseError> for WindowGeometryStoreError {
    fn from(error: DatabaseError) -> Self {
        Self::Database(error)
    }
}

impl From<rusqlite::Error> for WindowGeometryStoreError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Database(error.into())
    }
}

impl WorkspaceDatabase {
    /// 单行 upsert；同一机器只保留最新几何。
    pub(crate) fn save_window_geometry(
        &mut self,
        machine_key: &str,
        geometry: &WindowGeometry,
    ) -> Result<(), WindowGeometryStoreError> {
        let json = serde_json::to_string(geometry).map_err(|_| WindowGeometryStoreError::Encode)?;
        self.database.connection.execute(
            "INSERT INTO window_geometry (machine_key, dto_version, geometry_json) \
             VALUES (?1, ?2, ?3) \
             ON CONFLICT(machine_key) DO UPDATE SET \
             dto_version = excluded.dto_version, geometry_json = excluded.geometry_json",
            rusqlite::params![machine_key, WINDOW_GEOMETRY_DTO_VERSION, json],
        )?;
        Ok(())
    }

    /// 读取本机几何；无记录、版本超前与损坏 JSON 一律按无记录处理。
    pub(crate) fn window_geometry(
        &self,
        machine_key: &str,
    ) -> Result<Option<WindowGeometry>, WindowGeometryStoreError> {
        let row = self
            .database
            .connection
            .query_row(
                "SELECT dto_version, geometry_json FROM window_geometry WHERE machine_key = ?1",
                [machine_key],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?;
        let Some((version, json)) = row else {
            return Ok(None);
        };
        if version != WINDOW_GEOMETRY_DTO_VERSION {
            log::warn!("window geometry dto version unsupported: {version}");
            return Ok(None);
        }
        match serde_json::from_str::<WindowGeometry>(&json) {
            Ok(geometry) => Ok(Some(geometry)),
            Err(_) => {
                // 纯展示偏好：损坏按无记录处理，不覆盖用户行也不阻断启动。
                log::warn!("window geometry record is corrupted; using default window bounds");
                Ok(None)
            }
        }
    }
}

#[cfg(test)]
mod window_geometry_tests {
    use slterm_path::AppPaths;

    use super::*;

    fn geometry(x: f32, maximized: bool) -> WindowGeometry {
        WindowGeometry {
            x,
            y: 40.0,
            width: 1000.0,
            height: 700.0,
            maximized,
            display: None,
        }
    }

    /// 同一库按机器隔离，upsert 只覆盖本机行。
    #[test]
    fn window_geometry_round_trips_per_machine() {
        let temp = tempfile::tempdir().unwrap();
        let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
        let mut store = WorkspaceDatabase::open(&paths).unwrap();
        assert!(store.window_geometry("machine-a").unwrap().is_none());

        store
            .save_window_geometry("machine-a", &geometry(10.0, false))
            .unwrap();
        store
            .save_window_geometry("machine-b", &geometry(20.0, true))
            .unwrap();
        assert_eq!(store.window_geometry("machine-a").unwrap().unwrap().x, 10.0);
        assert_eq!(store.window_geometry("machine-b").unwrap().unwrap().x, 20.0);

        store
            .save_window_geometry("machine-a", &geometry(30.0, true))
            .unwrap();
        let reloaded = store.window_geometry("machine-a").unwrap().unwrap();
        assert_eq!(reloaded.x, 30.0);
        assert!(reloaded.maximized);
        assert_eq!(store.window_geometry("machine-b").unwrap().unwrap().x, 20.0);
    }

    /// 损坏记录按无记录处理，不改写用户行。
    #[test]
    fn corrupted_window_geometry_is_ignored() {
        let temp = tempfile::tempdir().unwrap();
        let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
        let mut store = WorkspaceDatabase::open(&paths).unwrap();
        store
            .save_window_geometry("machine-a", &geometry(10.0, false))
            .unwrap();
        store
            .database
            .connection
            .execute(
                "UPDATE window_geometry SET geometry_json = 'not-json' WHERE machine_key = 'machine-a'",
                [],
            )
            .unwrap();

        assert!(store.window_geometry("machine-a").unwrap().is_none());
        let stored: String = store
            .database
            .connection
            .query_row(
                "SELECT geometry_json FROM window_geometry WHERE machine_key = 'machine-a'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(stored, "not-json", "损坏材料不得被静默覆盖");
    }
}
