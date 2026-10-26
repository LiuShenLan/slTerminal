//! 两个 SQLite 库共用的连接与存储合同。

use std::collections::BTreeMap;
use std::fmt;
use std::fs::OpenOptions;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::{Connection, TransactionBehavior, params};

/// 当前连接允许的持久化操作。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DatabaseAccess {
    ReadWrite,
    ReadOnlyHigherVersion,
}

/// 持久化适配错误；展示层不接收底层 SQL、路径或 SQLite 消息。
#[derive(Debug)]
pub(crate) enum DatabaseError {
    WrongApplication,
    IncompatibleSchema,
    Corrupt,
    StorageContract,
    BackupTargetExists,
    Sqlite(rusqlite::Error),
    Io(io::Error),
}

impl fmt::Display for DatabaseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::WrongApplication => "数据库不属于本应用",
            Self::IncompatibleSchema => "数据库 schema 不受支持",
            Self::Corrupt => "数据库完整性校验失败",
            Self::StorageContract => "数据库存储合同不满足",
            Self::BackupTargetExists => "备份临时目标已存在",
            Self::Sqlite(_) => "SQLite 操作失败",
            Self::Io(_) => "持久化 I/O 失败",
        })
    }
}

impl std::error::Error for DatabaseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Sqlite(error) => Some(error),
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<rusqlite::Error> for DatabaseError {
    fn from(error: rusqlite::Error) -> Self {
        let is_corrupt = matches!(
            &error,
            rusqlite::Error::SqliteFailure(code, _)
                if matches!(
                    code.extended_code & 0xff,
                    rusqlite::ffi::SQLITE_CORRUPT | rusqlite::ffi::SQLITE_NOTADB
                )
        );
        if is_corrupt {
            Self::Corrupt
        } else {
            Self::Sqlite(error)
        }
    }
}

impl From<io::Error> for DatabaseError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

pub(super) struct SchemaObject {
    pub(super) kind: &'static str,
    pub(super) name: &'static str,
    pub(super) sql: &'static str,
}

pub(super) struct DatabaseSpec {
    pub(super) application_id: i32,
    pub(super) schema_version: u32,
    pub(super) objects: &'static [SchemaObject],
    pub(super) meta_table: &'static str,
}

pub(super) struct Database {
    pub(super) connection: Connection,
    access: DatabaseAccess,
    schema_version: u32,
    spec: &'static DatabaseSpec,
}

impl Database {
    pub(super) fn open(path: &Path, spec: &'static DatabaseSpec) -> Result<Self, DatabaseError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut connection = Connection::open(path)?;
        connection.busy_timeout(Duration::from_secs(2))?;
        connection.pragma_update(None, "foreign_keys", true)?;

        let application_id = pragma_i64(&connection, "application_id")?;
        let raw_version = pragma_i64(&connection, "user_version")?;
        let schema_version =
            u32::try_from(raw_version).map_err(|_| DatabaseError::IncompatibleSchema)?;
        let object_count = scalar_i64(
            &connection,
            "SELECT COUNT(*) FROM sqlite_master WHERE name NOT LIKE 'sqlite_%'",
            [],
        )?;
        let is_new = application_id == 0 && schema_version == 0 && object_count == 0;

        let access = if is_new {
            initialize(&mut connection, spec)?;
            DatabaseAccess::ReadWrite
        } else {
            if application_id != i64::from(spec.application_id) {
                return Err(DatabaseError::WrongApplication);
            }
            let access = if schema_version > spec.schema_version {
                connection.pragma_update(None, "query_only", true)?;
                DatabaseAccess::ReadOnlyHigherVersion
            } else if schema_version < spec.schema_version {
                return Err(DatabaseError::IncompatibleSchema);
            } else {
                DatabaseAccess::ReadWrite
            };
            validate_storage(&connection)?;
            if access == DatabaseAccess::ReadWrite {
                validate_schema(&connection, spec)?;
            }
            access
        };

        let schema_version = if is_new {
            spec.schema_version
        } else {
            schema_version
        };
        Ok(Self {
            connection,
            access,
            schema_version,
            spec,
        })
    }

    pub(crate) const fn access(&self) -> DatabaseAccess {
        self.access
    }

    pub(crate) const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    pub(crate) fn backup_to_new(&self, destination: &Path) -> Result<(), DatabaseError> {
        // 先保留临时文件名，消除“检查不存在”与 SQLite 打开之间的覆盖竞态。
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(destination)
        {
            Ok(file) => drop(file),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                return Err(DatabaseError::BackupTargetExists);
            }
            Err(error) => return Err(DatabaseError::Io(error)),
        }

        let result = self.connection.backup(
            rusqlite::MAIN_DB,
            destination,
            None::<fn(rusqlite::backup::Progress)>,
        );
        if let Err(error) = result {
            cleanup_backup(destination);
            return Err(error.into());
        }

        // 副本必须经同一打开合同复核；失败只留下不可误用的临时残片并报告错误。
        match Self::open(destination, self.spec) {
            Ok(copy)
                if copy.access() == self.access()
                    && copy.schema_version() == self.schema_version() =>
            {
                Ok(())
            }
            Ok(_) => {
                cleanup_backup(destination);
                Err(DatabaseError::IncompatibleSchema)
            }
            Err(error) => {
                cleanup_backup(destination);
                Err(error)
            }
        }
    }
}

fn initialize(connection: &mut Connection, spec: &DatabaseSpec) -> Result<(), DatabaseError> {
    let mode = connection.query_row("PRAGMA journal_mode = WAL", [], |row| {
        row.get::<_, String>(0)
    })?;
    if !mode.eq_ignore_ascii_case("wal") {
        return Err(DatabaseError::StorageContract);
    }

    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(DatabaseError::from)?;
    transaction.pragma_update(None, "application_id", spec.application_id)?;
    transaction.pragma_update(None, "user_version", spec.schema_version)?;
    for object in spec.objects {
        transaction.execute_batch(object.sql)?;
    }
    transaction.commit()?;
    validate_storage(connection)?;
    validate_schema(connection, spec)
}

fn validate_storage(connection: &Connection) -> Result<(), DatabaseError> {
    let integrity = connection
        .prepare("PRAGMA integrity_check")?
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    if integrity != ["ok"] {
        return Err(DatabaseError::Corrupt);
    }

    let mut statement = connection.prepare("PRAGMA foreign_key_check")?;
    let mut rows = statement.query([])?;
    if rows.next()?.is_some() {
        return Err(DatabaseError::Corrupt);
    }

    let mode = connection.query_row("PRAGMA journal_mode", [], |row| row.get::<_, String>(0))?;
    if !mode.eq_ignore_ascii_case("wal") {
        return Err(DatabaseError::StorageContract);
    }
    Ok(())
}

fn validate_schema(connection: &Connection, spec: &DatabaseSpec) -> Result<(), DatabaseError> {
    let actual = connection
        .prepare(
            "SELECT name, type, COALESCE(sql, '') FROM sqlite_master \
             WHERE name NOT LIKE 'sqlite_%' \
             AND type IN ('table', 'index', 'trigger', 'view') ORDER BY name",
        )?
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                (row.get::<_, String>(1)?, row.get::<_, String>(2)?),
            ))
        })?
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    let expected = spec
        .objects
        .iter()
        .map(|object| (object.name, (object.kind, object.sql)))
        .collect::<BTreeMap<_, _>>();
    if actual.len() != expected.len()
        || actual.iter().any(|(name, definition)| {
            expected.get(name.as_str()) != Some(&(definition.0.as_str(), definition.1.as_str()))
        })
    {
        return Err(DatabaseError::IncompatibleSchema);
    }

    let sql = format!(
        "SELECT COUNT(*) FROM {} WHERE singleton <> 1 OR schema_version <> ?1",
        spec.meta_table
    );
    let invalid_rows = scalar_i64(connection, &sql, params![spec.schema_version])?;
    if invalid_rows != 0 {
        return Err(DatabaseError::IncompatibleSchema);
    }
    Ok(())
}

fn pragma_i64(connection: &Connection, name: &str) -> Result<i64, DatabaseError> {
    scalar_i64(connection, &format!("PRAGMA {name}"), [])
}

fn scalar_i64<P: rusqlite::Params>(
    connection: &Connection,
    sql: &str,
    params: P,
) -> Result<i64, DatabaseError> {
    Ok(connection.query_row(sql, params, |row| row.get(0))?)
}

fn cleanup_backup(destination: &Path) {
    for suffix in ["", "-wal", "-shm"] {
        let mut path = destination.as_os_str().to_owned();
        path.push(suffix);
        let _ = std::fs::remove_file(PathBuf::from(path));
    }
}
