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

const MAX_TEXT_BYTES: usize = 65_536;
const COMMAND_CAPACITY: i64 = 5_000;
const DIRECTORY_CAPACITY: i64 = 2_048;
const MAX_TOTAL_DIRECTORY_RANK: f64 = 10_000.0;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CommandObservation {
    pub(crate) shell: String,
    pub(crate) command: String,
    pub(crate) last_cwd: Option<String>,
    pub(crate) at_ms: i64,
}

impl CommandObservation {
    pub(crate) fn new(
        shell: &str,
        command: &str,
        cwd: Option<&str>,
        at_ms: i64,
    ) -> Result<Self, DatabaseError> {
        let command = command.trim();
        let cwd = cwd.filter(|cwd| !cwd.is_empty());
        if !matches!(shell, "pwsh" | "powershell" | "cmd")
            || command.is_empty()
            || command.len() > MAX_TEXT_BYTES
            || command.chars().any(char::is_control)
            || cwd
                .is_some_and(|cwd| cwd.len() > MAX_TEXT_BYTES || cwd.chars().any(char::is_control))
            || at_ms < 0
        {
            return Err(DatabaseError::StorageContract);
        }
        Ok(Self {
            shell: shell.to_owned(),
            command: command.to_owned(),
            last_cwd: cwd.map(str::to_owned),
            at_ms,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DirectoryObservation {
    pub(crate) path: String,
    pub(crate) at_ms: i64,
}

impl DirectoryObservation {
    pub(crate) fn new(path: &str, at_ms: i64) -> Result<Self, DatabaseError> {
        if path.is_empty()
            || path.len() > MAX_TEXT_BYTES
            || path.chars().any(char::is_control)
            || at_ms < 0
        {
            return Err(DatabaseError::StorageContract);
        }
        Ok(Self {
            path: path.to_owned(),
            at_ms,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CommandRecord {
    pub(crate) shell: String,
    pub(crate) command: String,
    pub(crate) first_used_at_ms: i64,
    pub(crate) last_used_at_ms: i64,
    pub(crate) use_count: i64,
    pub(crate) last_cwd: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DirectoryRecord {
    pub(crate) path: String,
    pub(crate) first_used_at_ms: i64,
    pub(crate) last_used_at_ms: i64,
    pub(crate) rank: f64,
}

#[derive(Default)]
pub(crate) struct LearningSnapshot {
    pub(crate) generation: i64,
    pub(crate) commands: Vec<CommandRecord>,
    pub(crate) directories: Vec<DirectoryRecord>,
}

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

    pub(crate) fn record_command(
        &mut self,
        observation: CommandObservation,
    ) -> Result<(), DatabaseError> {
        if self.access() != DatabaseAccess::ReadWrite {
            return Err(DatabaseError::StorageContract);
        }
        let transaction = self
            .database
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        transaction.execute(
            "INSERT INTO command_history \
             (shell, command, first_used_at_ms, last_used_at_ms, use_count, last_cwd) \
             VALUES (?1, ?2, ?3, ?3, 1, ?4) \
             ON CONFLICT(shell, command) DO UPDATE SET \
             first_used_at_ms = MIN(command_history.first_used_at_ms, excluded.first_used_at_ms), \
             last_used_at_ms = MAX(command_history.last_used_at_ms, excluded.last_used_at_ms), \
             use_count = command_history.use_count + 1, \
             last_cwd = CASE WHEN excluded.last_used_at_ms >= command_history.last_used_at_ms \
                 THEN excluded.last_cwd ELSE command_history.last_cwd END",
            rusqlite::params![
                observation.shell,
                observation.command,
                observation.at_ms,
                observation.last_cwd
            ],
        )?;
        transaction.execute(
            "DELETE FROM command_history WHERE rowid IN (\
             SELECT rowid FROM command_history \
             ORDER BY last_used_at_ms ASC, shell ASC, command ASC \
             LIMIT (SELECT MAX(0, COUNT(*) - ?1) FROM command_history))",
            [COMMAND_CAPACITY],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub(crate) fn record_directory(
        &mut self,
        observation: DirectoryObservation,
    ) -> Result<(), DatabaseError> {
        if self.access() != DatabaseAccess::ReadWrite {
            return Err(DatabaseError::StorageContract);
        }
        let transaction = self
            .database
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        transaction.execute(
            "INSERT INTO directory_history \
             (path, first_used_at_ms, last_used_at_ms, rank) VALUES (?1, ?2, ?2, 1.0) \
             ON CONFLICT(path) DO UPDATE SET \
             first_used_at_ms = MIN(directory_history.first_used_at_ms, excluded.first_used_at_ms), \
             last_used_at_ms = MAX(directory_history.last_used_at_ms, excluded.last_used_at_ms), \
             rank = directory_history.rank + 1.0",
            rusqlite::params![observation.path, observation.at_ms],
        )?;
        let total_rank: f64 = transaction.query_row(
            "SELECT COALESCE(SUM(rank), 0.0) FROM directory_history",
            [],
            |row| row.get(0),
        )?;
        if total_rank > MAX_TOTAL_DIRECTORY_RANK {
            let factor = 0.9 * MAX_TOTAL_DIRECTORY_RANK / total_rank;
            transaction.execute("UPDATE directory_history SET rank = rank * ?1", [factor])?;
            transaction.execute("DELETE FROM directory_history WHERE rank < 1.0", [])?;
        }
        transaction.execute(
            "DELETE FROM directory_history WHERE rowid IN (\
             SELECT rowid FROM directory_history ORDER BY \
             rank * CASE \
                 WHEN ?2 - last_used_at_ms < 3600000 THEN 4.0 \
                 WHEN ?2 - last_used_at_ms < 86400000 THEN 2.0 \
                 WHEN ?2 - last_used_at_ms < 604800000 THEN 0.5 \
                 ELSE 0.25 END ASC, path ASC \
             LIMIT (SELECT MAX(0, COUNT(*) - ?1) FROM directory_history))",
            [DIRECTORY_CAPACITY, observation.at_ms],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub(crate) fn clear(&mut self, generation: i64) -> Result<(), DatabaseError> {
        if self.access() != DatabaseAccess::ReadWrite || generation < 0 {
            return Err(DatabaseError::StorageContract);
        }
        let transaction = self
            .database
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let updated = transaction.execute(
            "UPDATE learning_meta SET generation = ?1 \
             WHERE singleton = 1 AND generation <= ?1",
            [generation],
        )?;
        if updated != 1 {
            return Err(DatabaseError::StorageContract);
        }
        transaction.execute("DELETE FROM command_history", [])?;
        transaction.execute("DELETE FROM directory_history", [])?;
        transaction.commit()?;
        let busy =
            self.database
                .connection
                .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| {
                    row.get::<_, i64>(0)
                })?;
        if busy != 0 {
            return Err(DatabaseError::StorageContract);
        }
        self.database.connection.execute_batch("VACUUM")?;
        Ok(())
    }

    pub(crate) fn snapshot(&self) -> Result<LearningSnapshot, DatabaseError> {
        if self.access() != DatabaseAccess::ReadWrite {
            return Ok(LearningSnapshot::default());
        }
        let mut statement = self.database.connection.prepare(
            "SELECT shell, command, first_used_at_ms, last_used_at_ms, use_count, last_cwd \
             FROM command_history ORDER BY shell, command",
        )?;
        let commands = statement
            .query_map([], |row| {
                Ok(CommandRecord {
                    shell: row.get(0)?,
                    command: row.get(1)?,
                    first_used_at_ms: row.get(2)?,
                    last_used_at_ms: row.get(3)?,
                    use_count: row.get(4)?,
                    last_cwd: row.get(5)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        let directories = self
            .database
            .connection
            .prepare(
                "SELECT path, first_used_at_ms, last_used_at_ms, rank \
                 FROM directory_history ORDER BY path",
            )?
            .query_map([], |row| {
                Ok(DirectoryRecord {
                    path: row.get(0)?,
                    first_used_at_ms: row.get(1)?,
                    last_used_at_ms: row.get(2)?,
                    rank: row.get(3)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        for command in &commands {
            let validated = CommandObservation::new(
                &command.shell,
                &command.command,
                command.last_cwd.as_deref(),
                command.last_used_at_ms,
            )
            .map_err(|_| DatabaseError::Corrupt)?;
            if validated.command != command.command
                || validated.last_cwd != command.last_cwd
                || command.first_used_at_ms < 0
                || command.use_count <= 0
            {
                return Err(DatabaseError::Corrupt);
            }
        }
        for directory in &directories {
            DirectoryObservation::new(&directory.path, directory.last_used_at_ms)
                .map_err(|_| DatabaseError::Corrupt)?;
            if directory.first_used_at_ms < 0
                || !directory.rank.is_finite()
                || directory.rank <= 0.0
            {
                return Err(DatabaseError::Corrupt);
            }
        }
        let generation = self.database.connection.query_row(
            "SELECT generation FROM learning_meta WHERE singleton = 1",
            [],
            |row| row.get(0),
        )?;
        Ok(LearningSnapshot {
            generation,
            commands,
            directories,
        })
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
        assert_eq!(store.snapshot().unwrap().generation, 0);
        store
            .database
            .connection
            .execute("UPDATE learning_meta SET generation = 7", [])
            .unwrap();
        drop(store);

        let reopened = LearningDatabase::open(&paths).unwrap();
        assert_eq!(reopened.snapshot().unwrap().generation, 7);
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
    fn learning_command_upsert_preserves_usage_facts() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let mut store = LearningDatabase::open(&paths).unwrap();
        let first = CommandObservation::new("pwsh", " git status ", Some("D:/one"), 100).unwrap();
        let second = CommandObservation::new("pwsh", "git status", Some("D:/two"), 200).unwrap();
        let other_shell = CommandObservation::new("cmd", "git status", None, 150).unwrap();

        store.record_command(first).unwrap();
        store.record_command(second).unwrap();
        store.record_command(other_shell).unwrap();

        let snapshot = store.snapshot().unwrap();
        assert_eq!(snapshot.commands.len(), 2);
        let command = snapshot
            .commands
            .iter()
            .find(|command| command.shell == "pwsh")
            .unwrap();
        assert_eq!(command.command, "git status");
        assert_eq!(command.first_used_at_ms, 100);
        assert_eq!(command.last_used_at_ms, 200);
        assert_eq!(command.use_count, 2);
        assert_eq!(command.last_cwd.as_deref(), Some("D:/two"));
    }

    #[test]
    fn learning_command_capacity_evicts_oldest_in_the_same_transaction() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let mut store = LearningDatabase::open(&paths).unwrap();
        let transaction = store
            .database
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .unwrap();
        for index in 0..5_000_i64 {
            transaction
                .execute(
                    "INSERT INTO command_history \
                     (shell, command, first_used_at_ms, last_used_at_ms, use_count) \
                     VALUES ('pwsh', ?1, ?2, ?2, 1)",
                    params![format!("command-{index}"), index],
                )
                .unwrap();
        }
        transaction.commit().unwrap();

        store
            .record_command(CommandObservation::new("pwsh", "newest", None, 5_000).unwrap())
            .unwrap();

        let snapshot = store.snapshot().unwrap();
        assert_eq!(snapshot.commands.len(), 5_000);
        assert!(
            snapshot
                .commands
                .iter()
                .all(|command| command.command != "command-0"),
            "实际最旧记录: {:?}",
            snapshot
                .commands
                .iter()
                .min_by_key(|command| command.last_used_at_ms)
        );
        assert!(
            snapshot
                .commands
                .iter()
                .any(|command| command.command == "newest")
        );
        assert!(CommandObservation::new("pwsh", &"x".repeat(65_537), None, 1).is_err());
    }

    #[test]
    fn learning_directory_upsert_preserves_frecency_facts() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let mut store = LearningDatabase::open(&paths).unwrap();

        store
            .record_directory(DirectoryObservation::new("D:/repo", 100).unwrap())
            .unwrap();
        store
            .record_directory(DirectoryObservation::new("D:/repo", 200).unwrap())
            .unwrap();

        let snapshot = store.snapshot().unwrap();
        assert_eq!(snapshot.directories.len(), 1);
        let directory = &snapshot.directories[0];
        assert_eq!(directory.path, "D:/repo");
        assert_eq!(directory.first_used_at_ms, 100);
        assert_eq!(directory.last_used_at_ms, 200);
        assert_eq!(directory.rank, 2.0);
    }

    #[test]
    fn learning_directory_capacity_uses_frecency_order() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let mut store = LearningDatabase::open(&paths).unwrap();
        let transaction = store
            .database
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .unwrap();
        for index in 0..2_048_i64 {
            transaction
                .execute(
                    "INSERT INTO directory_history \
                     (path, first_used_at_ms, last_used_at_ms, rank) \
                     VALUES (?1, 0, 0, 1.0)",
                    [format!("D:/old-{index}")],
                )
                .unwrap();
        }
        transaction
            .execute(
                "INSERT INTO directory_history \
             (path, first_used_at_ms, last_used_at_ms, rank) \
             VALUES ('D:/frequent', 0, 0, 50.0)",
                [],
            )
            .unwrap();
        transaction.commit().unwrap();

        store
            .record_directory(DirectoryObservation::new("D:/new", 1).unwrap())
            .unwrap();

        let snapshot = store.snapshot().unwrap();
        assert_eq!(snapshot.directories.len(), 2_048);
        assert!(
            snapshot
                .directories
                .iter()
                .any(|directory| directory.path == "D:/frequent")
        );
        assert!(
            snapshot
                .directories
                .iter()
                .all(|directory| directory.path != "D:/old-0")
        );
    }

    #[test]
    fn learning_directory_rank_ages_and_drops_low_rank_entries() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let mut store = LearningDatabase::open(&paths).unwrap();
        store
            .database
            .connection
            .execute_batch(
                "INSERT INTO directory_history VALUES ('D:/hot', 0, 0, 10000.0); \
             INSERT INTO directory_history VALUES ('D:/cold', 0, 0, 0.5);",
            )
            .unwrap();

        store
            .record_directory(DirectoryObservation::new("D:/new", 1).unwrap())
            .unwrap();

        let snapshot = store.snapshot().unwrap();
        assert!(
            snapshot
                .directories
                .iter()
                .all(|directory| directory.path != "D:/cold")
        );
        let total: f64 = snapshot
            .directories
            .iter()
            .map(|directory| directory.rank)
            .sum();
        assert!(total <= 9_000.0 + f64::EPSILON, "实际总 rank: {total}");
    }

    #[test]
    fn learning_clear_deletes_all_and_preserves_generation() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let mut store = LearningDatabase::open(&paths).unwrap();
        store
            .record_command(CommandObservation::new("pwsh", "git status", None, 1).unwrap())
            .unwrap();
        store
            .record_directory(DirectoryObservation::new("D:/repo", 1).unwrap())
            .unwrap();

        store.clear(1).unwrap();

        let snapshot = store.snapshot().unwrap();
        assert!(snapshot.commands.is_empty());
        assert!(snapshot.directories.is_empty());
        assert_eq!(
            scalar_i64(
                &store.database.connection,
                "SELECT generation FROM learning_meta"
            ),
            1
        );
        store
            .record_command(CommandObservation::new("pwsh", "new fact", None, 2).unwrap())
            .unwrap();
        assert!(store.clear(0).is_err(), "旧代次清空不得删除新事实");
        assert_eq!(store.snapshot().unwrap().commands.len(), 1);
    }

    #[test]
    fn learning_snapshot_rejects_semantic_corruption() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let store = LearningDatabase::open(&paths).unwrap();
        store
            .database
            .connection
            .execute(
                "INSERT INTO command_history \
                 (shell, command, first_used_at_ms, last_used_at_ms, use_count) \
                 VALUES ('bash', 'echo unknown', 1, 1, 1)",
                [],
            )
            .unwrap();

        assert!(matches!(store.snapshot(), Err(DatabaseError::Corrupt)));
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
