//! `learning.db` 的独立身份、历史表与学习代次。

use slterm_path::AppPaths;

use super::database::{Database, DatabaseAccess, DatabaseError, DatabaseSpec, SchemaObject};

const LEARNING_APPLICATION_ID: i32 = 0x534C_4C52;
const LEARNING_SCHEMA_VERSION: u32 = 1;

static LEARNING_OBJECTS: [SchemaObject; 4] = [
    SchemaObject {
        kind: "table",
        name: "learning_meta",
        sql: "CREATE TABLE learning_meta (singleton INTEGER PRIMARY KEY CHECK (singleton = 1), schema_version INTEGER NOT NULL, generation INTEGER NOT NULL CHECK (generation >= 0))",
    },
    SchemaObject {
        kind: "table",
        name: "command_history",
        sql: "CREATE TABLE command_history (shell TEXT NOT NULL CHECK (length(shell) > 0), command TEXT NOT NULL CHECK (length(CAST(command AS BLOB)) > 0 AND length(CAST(command AS BLOB)) <= 65536), first_used_at_ms INTEGER NOT NULL CHECK (first_used_at_ms >= 0), last_used_at_ms INTEGER NOT NULL CHECK (last_used_at_ms >= 0), use_count INTEGER NOT NULL CHECK (use_count > 0), last_cwd TEXT CHECK (last_cwd IS NULL OR length(CAST(last_cwd AS BLOB)) <= 65536), PRIMARY KEY (shell, command), CHECK (first_used_at_ms <= last_used_at_ms))",
    },
    SchemaObject {
        kind: "table",
        name: "directory_history",
        sql: "CREATE TABLE directory_history (path TEXT NOT NULL CHECK (length(CAST(path AS BLOB)) > 0 AND length(CAST(path AS BLOB)) <= 65536), first_used_at_ms INTEGER NOT NULL CHECK (first_used_at_ms >= 0), last_used_at_ms INTEGER NOT NULL CHECK (last_used_at_ms >= 0), rank REAL NOT NULL CHECK (rank > 0.0), PRIMARY KEY (path), CHECK (first_used_at_ms <= last_used_at_ms))",
    },
    SchemaObject {
        kind: "index",
        name: "command_history_recency",
        sql: "CREATE INDEX command_history_recency ON command_history(last_used_at_ms)",
    },
];

static LEARNING_SPEC: DatabaseSpec = DatabaseSpec {
    application_id: LEARNING_APPLICATION_ID,
    schema_version: LEARNING_SCHEMA_VERSION,
    objects: &LEARNING_OBJECTS,
    meta_table: "learning_meta",
};

/// 学习库连接；不提供灾难副本或导出入口。
pub(crate) struct LearningDatabase {
    database: Database,
}

impl LearningDatabase {
    pub(crate) fn open(paths: &AppPaths) -> Result<Self, DatabaseError> {
        let database = Database::open(&paths.learning_db_path(), &LEARNING_SPEC)?;
        if database.access() == DatabaseAccess::ReadWrite {
            database.connection.execute(
                "INSERT INTO learning_meta (singleton, schema_version, generation) \
                 SELECT 1, ?1, 0 WHERE NOT EXISTS (SELECT 1 FROM learning_meta)",
                rusqlite::params![LEARNING_SCHEMA_VERSION],
            )?;
        }
        Ok(Self { database })
    }

    pub(crate) const fn access(&self) -> DatabaseAccess {
        self.database.access()
    }

    pub(crate) const fn schema_version(&self) -> u32 {
        self.database.schema_version()
    }
}

#[cfg(test)]
mod learning_tests {
    use std::fs;

    use rusqlite::{Connection, params};
    use tempfile::TempDir;

    use super::*;

    fn fixture(temp: &TempDir) -> AppPaths {
        AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap()
    }

    fn raw_connection(paths: &AppPaths) -> Connection {
        Connection::open(paths.learning_db_path()).unwrap()
    }

    fn scalar_i64(connection: &Connection, sql: &str) -> i64 {
        connection.query_row(sql, [], |row| row.get(0)).unwrap()
    }

    fn table_names(connection: &Connection) -> Vec<String> {
        let mut statement = connection
            .prepare(
                "SELECT name FROM sqlite_master \
                 WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
            )
            .unwrap();
        statement
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
    }

    #[test]
    fn learning_open_initializes_exact_history_schema() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let store = LearningDatabase::open(&paths).unwrap();
        let connection = &store.database.connection;

        assert_eq!(
            table_names(connection),
            ["command_history", "directory_history", "learning_meta"]
        );
        assert_eq!(store.schema_version(), LEARNING_SCHEMA_VERSION);
        assert_eq!(store.access(), DatabaseAccess::ReadWrite);
        assert_eq!(
            scalar_i64(connection, "PRAGMA application_id"),
            i64::from(LEARNING_APPLICATION_ID)
        );
        assert_eq!(scalar_i64(connection, "PRAGMA user_version"), 1);
        assert_eq!(scalar_i64(connection, "PRAGMA foreign_keys"), 1);
        assert_eq!(
            connection
                .query_row("PRAGMA journal_mode", [], |row| row.get::<_, String>(0))
                .unwrap()
                .to_ascii_lowercase(),
            "wal"
        );
    }

    #[test]
    fn learning_generation_is_seeded_and_survives_reopen() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let store = LearningDatabase::open(&paths).unwrap();

        assert_eq!(
            scalar_i64(
                &store.database.connection,
                "SELECT COUNT(*) FROM learning_meta"
            ),
            1
        );
        assert_eq!(
            scalar_i64(
                &store.database.connection,
                "SELECT generation FROM learning_meta"
            ),
            0
        );
        store
            .database
            .connection
            .execute("UPDATE learning_meta SET generation = 7", [])
            .unwrap();
        drop(store);

        let reopened = LearningDatabase::open(&paths).unwrap();
        assert_eq!(
            scalar_i64(
                &reopened.database.connection,
                "SELECT generation FROM learning_meta"
            ),
            7
        );
    }

    #[test]
    fn learning_constraints_cover_dedup_time_and_utf8_limits() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let store = LearningDatabase::open(&paths).unwrap();
        let connection = &store.database.connection;
        let insert_command = |shell: &str, command: &str| {
            connection.execute(
                "INSERT INTO command_history \
                 (shell, command, first_used_at_ms, last_used_at_ms, use_count, last_cwd) \
                 VALUES (?1, ?2, 1, 2, 1, 'D:/')",
                params![shell, command],
            )
        };

        assert!(insert_command("pwsh", "git status").is_ok());
        assert!(insert_command("pwsh", "git status").is_err());
        assert!(insert_command("cmd", "git status").is_ok());
        assert!(
            connection
                .execute(
                    "INSERT INTO command_history \
                     (shell, command, first_used_at_ms, last_used_at_ms, use_count) \
                     VALUES ('pwsh', 'invalid-count', 1, 2, 0)",
                    [],
                )
                .is_err()
        );
        assert!(
            connection
                .execute(
                    "INSERT INTO command_history \
                     (shell, command, first_used_at_ms, last_used_at_ms, use_count) \
                     VALUES ('pwsh', 'invalid-time', 3, 2, 1)",
                    [],
                )
                .is_err()
        );
        let oversized = "x".repeat(65_537);
        assert!(insert_command("pwsh", &oversized).is_err());
        let oversized_count = connection
            .query_row(
                "SELECT COUNT(*) FROM command_history WHERE command = ?1",
                [&oversized],
                |row| row.get::<_, i64>(0),
            )
            .unwrap();
        assert_eq!(oversized_count, 0);
        let exact_limit = "y".repeat(65_536);
        assert!(insert_command("pwsh", &exact_limit).is_ok());
        let stored_bytes = connection
            .query_row(
                "SELECT length(CAST(command AS BLOB)) FROM command_history WHERE command = ?1",
                [&exact_limit],
                |row| row.get::<_, i64>(0),
            )
            .unwrap();
        assert_eq!(stored_bytes, 65_536);
        let multibyte_overflow = "界".repeat(21_846);
        assert!(insert_command("pwsh", &multibyte_overflow).is_err());
        assert!(
            connection
                .execute(
                    "INSERT INTO command_history \
                     (shell, command, first_used_at_ms, last_used_at_ms, use_count, last_cwd) \
                     VALUES ('pwsh', 'cwd-overflow', 1, 2, 1, ?1)",
                    [&oversized],
                )
                .is_err()
        );

        let insert_directory = |path: &str| {
            connection.execute(
                "INSERT INTO directory_history \
                 (path, first_used_at_ms, last_used_at_ms, rank) VALUES (?1, 1, 2, 1.0)",
                [path],
            )
        };
        assert!(insert_directory("D:/repo").is_ok());
        assert!(insert_directory("D:/repo").is_err());
        assert!(insert_directory(&exact_limit).is_ok());
        assert!(insert_directory(&oversized).is_err());
        assert!(insert_directory(&multibyte_overflow).is_err());
        assert!(
            connection
                .execute(
                    "INSERT INTO directory_history \
                     (path, first_used_at_ms, last_used_at_ms, rank) VALUES ('D:/invalid', 1, 2, 0.0)",
                    [],
                )
                .is_err()
        );
    }

    #[test]
    fn learning_higher_version_pauses_writes_without_downgrade() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        drop(LearningDatabase::open(&paths).unwrap());
        let connection = raw_connection(&paths);
        connection
            .pragma_update(None, "user_version", 2_u32)
            .unwrap();
        connection
            .execute_batch("CREATE TABLE future_learning (value TEXT); INSERT INTO future_learning VALUES ('kept')")
            .unwrap();
        drop(connection);
        let before = fs::read(paths.learning_db_path()).unwrap();

        let store = LearningDatabase::open(&paths).unwrap();

        assert_eq!(store.access(), DatabaseAccess::ReadOnlyHigherVersion);
        assert_eq!(store.schema_version(), 2);
        assert!(
            store
                .database
                .connection
                .execute("DELETE FROM learning_meta", [])
                .is_err()
        );
        drop(store);
        assert_eq!(fs::read(paths.learning_db_path()).unwrap(), before);
        let connection = raw_connection(&paths);
        assert_eq!(scalar_i64(&connection, "PRAGMA user_version"), 2);
        assert_eq!(
            scalar_i64(&connection, "SELECT COUNT(*) FROM future_learning"),
            1
        );
    }

    #[test]
    fn learning_corruption_pauses_writes_without_overwrite() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        fs::create_dir_all(paths.local_root()).unwrap();
        fs::write(paths.learning_db_path(), b"not-a-sqlite-database").unwrap();
        let before = fs::read(paths.learning_db_path()).unwrap();

        let result = LearningDatabase::open(&paths);

        assert!(matches!(result, Err(DatabaseError::Corrupt)));
        assert_eq!(fs::read(paths.learning_db_path()).unwrap(), before);
    }

    #[test]
    fn learning_rejects_workspace_identity_without_overwrite() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        drop(LearningDatabase::open(&paths).unwrap());
        let connection = raw_connection(&paths);
        connection
            .pragma_update(None, "application_id", 0x534C_544D_i32)
            .unwrap();
        drop(connection);
        let before = fs::read(paths.learning_db_path()).unwrap();

        let result = LearningDatabase::open(&paths);

        assert!(matches!(result, Err(DatabaseError::WrongApplication)));
        assert_eq!(fs::read(paths.learning_db_path()).unwrap(), before);
    }
}
