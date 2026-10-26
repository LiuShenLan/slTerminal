//! `workspace.db` 的固定身份、物理 schema 与在线副本入口。

use std::path::Path;

use slterm_path::AppPaths;

use super::database::{Database, DatabaseAccess, DatabaseError, DatabaseSpec, SchemaObject};

const WORKSPACE_APPLICATION_ID: i32 = 0x534C_544D;
const WORKSPACE_SCHEMA_VERSION: u32 = 1;

static WORKSPACE_OBJECTS: [SchemaObject; 21] = [
    SchemaObject {
        kind: "table",
        name: "workspace_meta",
        sql: "CREATE TABLE workspace_meta (singleton INTEGER PRIMARY KEY CHECK (singleton = 1), store_epoch TEXT NOT NULL CHECK (length(store_epoch) > 0), run_id TEXT NOT NULL CHECK (length(run_id) > 0), clean_shutdown INTEGER NOT NULL CHECK (clean_shutdown IN (0, 1)), schema_version INTEGER NOT NULL, process_id INTEGER NOT NULL CHECK (process_id >= 0), process_created_at_ms INTEGER NOT NULL CHECK (process_created_at_ms >= 0))",
    },
    SchemaObject {
        kind: "table",
        name: "projects",
        sql: "CREATE TABLE projects (project_id TEXT NOT NULL PRIMARY KEY, root TEXT NOT NULL UNIQUE, display_name TEXT NOT NULL, sort_order INTEGER NOT NULL, root_status TEXT NOT NULL, next_terminal_number INTEGER NOT NULL CHECK (next_terminal_number >= 0))",
    },
    SchemaObject {
        kind: "table",
        name: "app_state",
        sql: "CREATE TABLE app_state (singleton INTEGER PRIMARY KEY CHECK (singleton = 1), active_project_id TEXT REFERENCES projects(project_id) ON DELETE SET NULL)",
    },
    SchemaObject {
        kind: "table",
        name: "panel_groups",
        sql: "CREATE TABLE panel_groups (panel_group_id TEXT NOT NULL PRIMARY KEY, project_id TEXT NOT NULL REFERENCES projects(project_id) ON DELETE CASCADE)",
    },
    SchemaObject {
        kind: "table",
        name: "project_layouts",
        sql: "CREATE TABLE project_layouts (project_id TEXT NOT NULL PRIMARY KEY REFERENCES projects(project_id) ON DELETE CASCADE, dto_version INTEGER NOT NULL, layout_json TEXT NOT NULL, revision INTEGER NOT NULL CHECK (revision >= 0))",
    },
    SchemaObject {
        kind: "table",
        name: "documents",
        sql: "CREATE TABLE documents (document_id TEXT NOT NULL PRIMARY KEY, location_json TEXT NOT NULL, file_identity_json TEXT)",
    },
    SchemaObject {
        kind: "table",
        name: "terminals",
        sql: "CREATE TABLE terminals (terminal_id TEXT NOT NULL PRIMARY KEY, project_id TEXT NOT NULL REFERENCES projects(project_id) ON DELETE CASCADE, number INTEGER NOT NULL CHECK (number >= 0), cwd TEXT NOT NULL, shell TEXT NOT NULL, custom_title TEXT, run_intent TEXT NOT NULL, UNIQUE (project_id, number))",
    },
    SchemaObject {
        kind: "table",
        name: "view_states",
        sql: "CREATE TABLE view_states (view_key TEXT NOT NULL PRIMARY KEY, dto_version INTEGER NOT NULL, state_json TEXT NOT NULL, revision INTEGER NOT NULL CHECK (revision >= 0), scope TEXT NOT NULL CHECK (scope = 'application' OR (substr(scope, 1, 8) = 'project:' AND length(scope) > 8)))",
    },
    SchemaObject {
        kind: "table",
        name: "diff_bindings",
        sql: "CREATE TABLE diff_bindings (view_key TEXT NOT NULL PRIMARY KEY, repository_binding_json TEXT NOT NULL, head_path TEXT NOT NULL, revision INTEGER NOT NULL CHECK (revision >= 0))",
    },
    SchemaObject {
        kind: "table",
        name: "window_geometry",
        sql: "CREATE TABLE window_geometry (machine_key TEXT NOT NULL PRIMARY KEY, dto_version INTEGER NOT NULL, geometry_json TEXT NOT NULL)",
    },
    SchemaObject {
        kind: "table",
        name: "runtime_operations",
        sql: "CREATE TABLE runtime_operations (operation_id TEXT NOT NULL PRIMARY KEY, store_epoch TEXT NOT NULL, client_id TEXT NOT NULL, mutation_id TEXT NOT NULL, method TEXT NOT NULL, params_hash BLOB NOT NULL, resource_generations_json TEXT NOT NULL, status TEXT NOT NULL, result_json TEXT, error_json TEXT, journal_id TEXT REFERENCES managed_journals(journal_id), UNIQUE (store_epoch, client_id, mutation_id))",
    },
    SchemaObject {
        kind: "table",
        name: "managed_journals",
        sql: "CREATE TABLE managed_journals (journal_id TEXT NOT NULL PRIMARY KEY, kind TEXT NOT NULL, status TEXT NOT NULL, store_epoch TEXT NOT NULL, before_fingerprint BLOB, after_fingerprint BLOB)",
    },
    SchemaObject {
        kind: "table",
        name: "journal_steps",
        sql: "CREATE TABLE journal_steps (journal_id TEXT NOT NULL REFERENCES managed_journals(journal_id), step_key TEXT NOT NULL, kind TEXT NOT NULL, target_json TEXT NOT NULL, authorization_generation INTEGER NOT NULL CHECK (authorization_generation >= 0), before_fingerprint BLOB, after_fingerprint BLOB, PRIMARY KEY (journal_id, step_key))",
    },
    SchemaObject {
        kind: "table",
        name: "close_intents",
        sql: "CREATE TABLE close_intents (close_id TEXT NOT NULL, object_key TEXT NOT NULL, object_generation INTEGER NOT NULL CHECK (object_generation >= 0), decision TEXT NOT NULL, commit_status TEXT NOT NULL, PRIMARY KEY (close_id, object_key))",
    },
    SchemaObject {
        kind: "table",
        name: "quarantine_batches",
        sql: "CREATE TABLE quarantine_batches (batch_id TEXT NOT NULL PRIMARY KEY, status TEXT NOT NULL, absolute_deadline_ms INTEGER NOT NULL CHECK (absolute_deadline_ms >= 0), completed_at_ms INTEGER CHECK (completed_at_ms >= 0), fingerprint BLOB)",
    },
    SchemaObject {
        kind: "table",
        name: "quarantine_items",
        sql: "CREATE TABLE quarantine_items (batch_id TEXT NOT NULL REFERENCES quarantine_batches(batch_id), item_key TEXT NOT NULL, original_location_json TEXT NOT NULL, original_order INTEGER NOT NULL CHECK (original_order >= 0), source_identity_json TEXT NOT NULL, before_fingerprint BLOB, after_fingerprint BLOB, quarantine_path TEXT NOT NULL, status TEXT NOT NULL, PRIMARY KEY (batch_id, item_key))",
    },
    SchemaObject {
        kind: "table",
        name: "recovery_points",
        sql: "CREATE TABLE recovery_points (recovery_point_id TEXT NOT NULL, object_key TEXT NOT NULL, status TEXT NOT NULL, before_fingerprint BLOB, after_fingerprint BLOB, dpapi_path TEXT NOT NULL, PRIMARY KEY (recovery_point_id, object_key))",
    },
    SchemaObject {
        kind: "table",
        name: "maintenance_requests",
        sql: "CREATE TABLE maintenance_requests (request_id TEXT NOT NULL PRIMARY KEY, kind TEXT NOT NULL, status TEXT NOT NULL, staged_fingerprint BLOB NOT NULL, store_epoch TEXT NOT NULL, staged_path TEXT NOT NULL)",
    },
    SchemaObject {
        kind: "table",
        name: "snapshot_index",
        sql: "CREATE TABLE snapshot_index (document_id TEXT NOT NULL PRIMARY KEY, file_fingerprint BLOB NOT NULL, status TEXT NOT NULL, scan_generation INTEGER NOT NULL CHECK (scan_generation >= 0), snapshot_path TEXT NOT NULL)",
    },
    SchemaObject {
        kind: "index",
        name: "view_states_scope",
        sql: "CREATE INDEX view_states_scope ON view_states(scope)",
    },
    SchemaObject {
        kind: "index",
        name: "runtime_operations_journal",
        sql: "CREATE INDEX runtime_operations_journal ON runtime_operations(journal_id)",
    },
];

static WORKSPACE_SPEC: DatabaseSpec = DatabaseSpec {
    application_id: WORKSPACE_APPLICATION_ID,
    schema_version: WORKSPACE_SCHEMA_VERSION,
    objects: &WORKSPACE_OBJECTS,
    meta_table: "workspace_meta",
};

/// 工作区数据库连接；领域写入 API 由后续存储所有者提供。
pub(crate) struct WorkspaceDatabase {
    database: Database,
}

impl WorkspaceDatabase {
    /// 只从应用数据路径打开工作区库，不读取环境或旧 JSON。
    pub(crate) fn open(paths: &AppPaths) -> Result<Self, DatabaseError> {
        Ok(Self {
            database: Database::open(&paths.workspace_db_path(), &WORKSPACE_SPEC)?,
        })
    }

    pub(crate) const fn access(&self) -> DatabaseAccess {
        self.database.access()
    }

    pub(crate) const fn schema_version(&self) -> u32 {
        self.database.schema_version()
    }

    /// 将一致快照写入新临时库；不覆盖目标，最终替换由统一原子写所有者执行。
    pub(crate) fn backup_to_new(&self, destination: &Path) -> Result<(), DatabaseError> {
        self.database.backup_to_new(destination)
    }
}

#[cfg(test)]
mod workspace_tests {
    use std::fs;
    use std::path::Path;

    use rusqlite::{Connection, TransactionBehavior};
    use tempfile::TempDir;

    use super::*;

    const EXPECTED_TABLES: [&str; 19] = [
        "app_state",
        "close_intents",
        "diff_bindings",
        "documents",
        "journal_steps",
        "maintenance_requests",
        "managed_journals",
        "panel_groups",
        "project_layouts",
        "projects",
        "quarantine_batches",
        "quarantine_items",
        "recovery_points",
        "runtime_operations",
        "snapshot_index",
        "terminals",
        "view_states",
        "window_geometry",
        "workspace_meta",
    ];

    const INSERT_META: &str = "INSERT INTO workspace_meta (singleton, store_epoch, run_id, clean_shutdown, schema_version, process_id, process_created_at_ms) VALUES (1, 'epoch-test', 'run-test', 0, 1, 123, 456)";
    const INSERT_PROJECT: &str = "INSERT INTO projects (project_id, root, display_name, sort_order, root_status, next_terminal_number) VALUES ('project-test', 'D:/repo', 'Repo', 0, 'ready', 0)";

    fn fixture(temp: &TempDir) -> AppPaths {
        AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap()
    }

    fn raw_connection(paths: &AppPaths) -> Connection {
        Connection::open(paths.workspace_db_path()).unwrap()
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

    fn scalar_i64(connection: &Connection, sql: &str) -> i64 {
        connection.query_row(sql, [], |row| row.get(0)).unwrap()
    }

    fn database_bytes(paths: &AppPaths) -> Vec<u8> {
        fs::read(paths.workspace_db_path()).unwrap()
    }

    fn backup_paths(root: &Path) -> AppPaths {
        AppPaths::new(root.join("config"), root).unwrap()
    }

    #[test]
    fn workspace_open_initializes_exact_schema_and_wal_contract() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let store = WorkspaceDatabase::open(&paths).unwrap();
        let connection = &store.database.connection;

        assert_eq!(table_names(connection), EXPECTED_TABLES);
        assert_eq!(store.schema_version(), WORKSPACE_SCHEMA_VERSION);
        assert_eq!(store.access(), DatabaseAccess::ReadWrite);
        assert_eq!(
            scalar_i64(connection, "PRAGMA application_id"),
            i64::from(WORKSPACE_APPLICATION_ID)
        );
        assert_eq!(scalar_i64(connection, "PRAGMA user_version"), 1);
        assert_eq!(scalar_i64(connection, "PRAGMA foreign_keys"), 1);
        assert_eq!(scalar_i64(connection, "PRAGMA query_only"), 0);
        assert!(scalar_i64(connection, "PRAGMA busy_timeout") > 0);
        assert_eq!(
            connection
                .query_row("PRAGMA journal_mode", [], |row| row.get::<_, String>(0))
                .unwrap()
                .to_ascii_lowercase(),
            "wal"
        );
        assert_eq!(
            scalar_i64(connection, "SELECT COUNT(*) FROM workspace_meta"),
            0
        );

        drop(store);
        let reopened = WorkspaceDatabase::open(&paths).unwrap();
        assert_eq!(reopened.access(), DatabaseAccess::ReadWrite);
        assert_eq!(table_names(&reopened.database.connection), EXPECTED_TABLES);
    }

    #[test]
    fn workspace_rejects_wrong_application_without_overwrite() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        drop(WorkspaceDatabase::open(&paths).unwrap());
        let connection = raw_connection(&paths);
        connection
            .pragma_update(None, "application_id", 7_i32)
            .unwrap();
        drop(connection);
        let before = database_bytes(&paths);

        let result = WorkspaceDatabase::open(&paths);

        assert!(matches!(result, Err(DatabaseError::WrongApplication)));
        assert_eq!(database_bytes(&paths), before);
    }

    #[test]
    fn workspace_rejects_lower_version_without_overwrite() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        drop(WorkspaceDatabase::open(&paths).unwrap());
        let connection = raw_connection(&paths);
        connection
            .pragma_update(None, "user_version", 0_u32)
            .unwrap();
        drop(connection);
        let before = database_bytes(&paths);

        let result = WorkspaceDatabase::open(&paths);

        assert!(matches!(result, Err(DatabaseError::IncompatibleSchema)));
        assert_eq!(database_bytes(&paths), before);
    }

    #[test]
    fn workspace_higher_version_is_query_only_and_not_downgraded() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        drop(WorkspaceDatabase::open(&paths).unwrap());
        let connection = raw_connection(&paths);
        connection
            .pragma_update(None, "user_version", 2_u32)
            .unwrap();
        connection
            .execute_batch(
                "CREATE TABLE future_field (value TEXT); INSERT INTO future_field VALUES ('kept')",
            )
            .unwrap();
        drop(connection);
        let before = database_bytes(&paths);

        let store = WorkspaceDatabase::open(&paths).unwrap();

        assert_eq!(store.access(), DatabaseAccess::ReadOnlyHigherVersion);
        assert_eq!(store.schema_version(), 2);
        assert_eq!(
            scalar_i64(&store.database.connection, "PRAGMA query_only"),
            1
        );
        assert!(
            store
                .database
                .connection
                .execute_batch("CREATE TABLE forbidden_write (value TEXT)")
                .is_err()
        );
        assert_eq!(
            store
                .database
                .connection
                .query_row("SELECT value FROM future_field", [], |row| row
                    .get::<_, String>(0))
                .unwrap(),
            "kept"
        );
        drop(store);
        assert_eq!(database_bytes(&paths), before);
        let connection = raw_connection(&paths);
        assert_eq!(scalar_i64(&connection, "PRAGMA user_version"), 2);
    }

    #[test]
    fn workspace_separates_corruption_and_current_schema_damage() {
        let corrupt_temp = tempfile::tempdir().unwrap();
        let corrupt_paths = fixture(&corrupt_temp);
        fs::create_dir_all(corrupt_paths.local_root()).unwrap();
        fs::write(corrupt_paths.workspace_db_path(), b"not-a-sqlite-database").unwrap();
        let before = database_bytes(&corrupt_paths);
        let result = WorkspaceDatabase::open(&corrupt_paths);
        assert!(matches!(result, Err(DatabaseError::Corrupt)));
        assert_eq!(database_bytes(&corrupt_paths), before);

        let damaged_temp = tempfile::tempdir().unwrap();
        let damaged_paths = fixture(&damaged_temp);
        drop(WorkspaceDatabase::open(&damaged_paths).unwrap());
        let connection = raw_connection(&damaged_paths);
        let has_meta = scalar_i64(
            &connection,
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'workspace_meta'",
        ) == 1;
        if has_meta {
            connection
                .execute_batch("DROP TABLE workspace_meta")
                .unwrap();
        } else {
            connection
                .execute_batch("CREATE TABLE unexpected (value TEXT)")
                .unwrap();
        }
        drop(connection);
        let before = database_bytes(&damaged_paths);
        let result = WorkspaceDatabase::open(&damaged_paths);
        assert!(matches!(result, Err(DatabaseError::IncompatibleSchema)));
        assert_eq!(database_bytes(&damaged_paths), before);
    }

    #[test]
    fn workspace_rejects_non_wal_current_database() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        drop(WorkspaceDatabase::open(&paths).unwrap());
        let connection = raw_connection(&paths);
        assert_eq!(
            connection
                .query_row("PRAGMA journal_mode = DELETE", [], |row| row
                    .get::<_, String>(0))
                .unwrap()
                .to_ascii_lowercase(),
            "delete"
        );
        drop(connection);
        let before = database_bytes(&paths);

        let result = WorkspaceDatabase::open(&paths);

        assert!(matches!(result, Err(DatabaseError::StorageContract)));
        assert_eq!(database_bytes(&paths), before);
    }

    #[test]
    fn workspace_transaction_rolls_back_uncommitted_changes() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let mut store = WorkspaceDatabase::open(&paths).unwrap();

        let transaction = store
            .database
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .unwrap();
        assert!(transaction.execute(INSERT_META, []).is_ok());
        assert!(transaction.execute(INSERT_PROJECT, []).is_ok());
        drop(transaction);
        assert_eq!(
            scalar_i64(
                &store.database.connection,
                "SELECT COUNT(*) FROM workspace_meta"
            ),
            0
        );
        assert_eq!(
            scalar_i64(&store.database.connection, "SELECT COUNT(*) FROM projects"),
            0
        );

        let transaction = store
            .database
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .unwrap();
        transaction.execute(INSERT_META, []).unwrap();
        transaction.execute(INSERT_PROJECT, []).unwrap();
        transaction.commit().unwrap();
        assert_eq!(
            scalar_i64(
                &store.database.connection,
                "SELECT COUNT(*) FROM workspace_meta"
            ),
            1
        );
        assert_eq!(
            scalar_i64(&store.database.connection, "SELECT COUNT(*) FROM projects"),
            1
        );
    }

    #[test]
    fn workspace_constraints_cover_singleton_foreign_key_and_mutation_identity() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let store = WorkspaceDatabase::open(&paths).unwrap();
        let connection = &store.database.connection;

        assert!(connection.execute(INSERT_PROJECT, []).is_ok());
        assert!(connection.execute(INSERT_META, []).is_ok());
        assert!(connection.execute(INSERT_META, []).is_err());
        assert!(
            connection
                .execute(
                    "INSERT INTO panel_groups (panel_group_id, project_id) VALUES ('group-test', 'missing-project')",
                    [],
                )
                .is_err()
        );

        let insert_operation = |operation_id: &str| {
            connection.execute(
                "INSERT INTO runtime_operations \
                 (operation_id, store_epoch, client_id, mutation_id, method, params_hash, resource_generations_json, status) \
                 VALUES (?1, 'epoch-test', 'client-test', 'mutation-test', 'document.save', x'00', '{}', 'queued')",
                [operation_id],
            )
        };
        assert!(insert_operation("operation-a").is_ok());
        assert!(insert_operation("operation-b").is_err());
        assert_eq!(
            scalar_i64(connection, "SELECT COUNT(*) FROM runtime_operations"),
            1
        );
    }

    #[test]
    fn workspace_online_backup_validates_new_copy_and_never_overwrites() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let store = WorkspaceDatabase::open(&paths).unwrap();
        let metadata_ready = store.database.connection.execute(INSERT_META, []).is_ok();
        let backup_root = temp.path().join("backup");
        fs::create_dir_all(&backup_root).unwrap();
        let destination = backup_root.join("workspace.db");

        let result = store.backup_to_new(&destination);

        assert!(result.is_ok());
        assert!(destination.exists());
        assert!(metadata_ready);
        let restored_paths = backup_paths(&backup_root);
        let restored = WorkspaceDatabase::open(&restored_paths).unwrap();
        assert_eq!(restored.access(), DatabaseAccess::ReadWrite);
        assert_eq!(
            scalar_i64(
                &restored.database.connection,
                "SELECT COUNT(*) FROM workspace_meta"
            ),
            1
        );
        drop(restored);
        let before = fs::read(&destination).unwrap();
        let result = store.backup_to_new(&destination);
        assert!(matches!(result, Err(DatabaseError::BackupTargetExists)));
        assert_eq!(fs::read(&destination).unwrap(), before);
    }

    #[test]
    fn workspace_recovery_metadata_is_not_implicitly_cascaded() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let store = WorkspaceDatabase::open(&paths).unwrap();
        let connection = &store.database.connection;
        connection
            .execute_batch(
                "INSERT INTO documents (document_id, location_json) VALUES ('document-test', '{}');\
                 INSERT INTO snapshot_index (document_id, file_fingerprint, status, scan_generation, snapshot_path) VALUES ('document-test', x'00', 'ready', 0, 'snapshot-path');\
                 INSERT INTO managed_journals (journal_id, kind, status, store_epoch) VALUES ('journal-test', 'document.save', 'Prepared', 'epoch-test');\
                 INSERT INTO journal_steps (journal_id, step_key, kind, target_json, authorization_generation) VALUES ('journal-test', 'step-test', 'replace', '{}', 0);\
                 INSERT INTO quarantine_batches (batch_id, status, absolute_deadline_ms) VALUES ('batch-test', 'Prepared', 0);\
                 INSERT INTO quarantine_items (batch_id, item_key, original_location_json, original_order, source_identity_json, quarantine_path, status) VALUES ('batch-test', 'item-test', '{}', 0, '{}', 'quarantine-path', 'Prepared');",
            )
            .unwrap();

        let journal_delete = connection.execute("DELETE FROM managed_journals", []);
        let batch_delete = connection.execute("DELETE FROM quarantine_batches", []);
        let document_delete = connection.execute("DELETE FROM documents", []);
        let snapshot_count = scalar_i64(connection, "SELECT COUNT(*) FROM snapshot_index");

        assert!(journal_delete.is_err());
        assert!(batch_delete.is_err());
        assert!(document_delete.is_ok());
        assert_eq!(snapshot_count, 1);
    }

    #[test]
    fn workspace_text_primary_keys_reject_null() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let store = WorkspaceDatabase::open(&paths).unwrap();
        let connection = &store.database.connection;
        connection
            .pragma_update(None, "foreign_keys", false)
            .unwrap();
        let cases = [
            (
                "projects",
                "INSERT INTO projects (project_id, root, display_name, sort_order, root_status, next_terminal_number) VALUES (NULL, 'D:/null', 'Null', 0, 'ready', 0)",
            ),
            (
                "panel_groups",
                "INSERT INTO panel_groups (panel_group_id, project_id) VALUES (NULL, 'project-test')",
            ),
            (
                "project_layouts",
                "INSERT INTO project_layouts (project_id, dto_version, layout_json, revision) VALUES (NULL, 1, '{}', 0)",
            ),
            (
                "documents",
                "INSERT INTO documents (document_id, location_json) VALUES (NULL, '{}')",
            ),
            (
                "terminals",
                "INSERT INTO terminals (terminal_id, project_id, number, cwd, shell, run_intent) VALUES (NULL, 'project-test', 0, 'D:/', 'pwsh', 'reuse')",
            ),
            (
                "view_states",
                "INSERT INTO view_states (view_key, dto_version, state_json, revision, scope) VALUES (NULL, 1, '{}', 0, 'application')",
            ),
            (
                "diff_bindings",
                "INSERT INTO diff_bindings (view_key, repository_binding_json, head_path, revision) VALUES (NULL, '{}', '', 0)",
            ),
            (
                "window_geometry",
                "INSERT INTO window_geometry (machine_key, dto_version, geometry_json) VALUES (NULL, 1, '{}')",
            ),
            (
                "runtime_operations",
                "INSERT INTO runtime_operations (operation_id, store_epoch, client_id, mutation_id, method, params_hash, resource_generations_json, status) VALUES (NULL, 'epoch', 'client', 'mutation', 'method', x'00', '{}', 'queued')",
            ),
            (
                "managed_journals",
                "INSERT INTO managed_journals (journal_id, kind, status, store_epoch) VALUES (NULL, 'document.save', 'Prepared', 'epoch')",
            ),
            (
                "quarantine_batches",
                "INSERT INTO quarantine_batches (batch_id, status, absolute_deadline_ms) VALUES (NULL, 'Prepared', 0)",
            ),
            (
                "maintenance_requests",
                "INSERT INTO maintenance_requests (request_id, kind, status, staged_fingerprint, store_epoch, staged_path) VALUES (NULL, 'restore', 'Prepared', x'00', 'epoch', 'staged')",
            ),
            (
                "snapshot_index",
                "INSERT INTO snapshot_index (document_id, file_fingerprint, status, scan_generation, snapshot_path) VALUES (NULL, x'00', 'ready', 0, 'snapshot')",
            ),
        ];

        let accepted = cases
            .iter()
            .filter_map(|(table, sql)| connection.execute(sql, []).is_ok().then_some(*table))
            .collect::<Vec<_>>();

        assert!(accepted.is_empty(), "接受 NULL 主键：{accepted:?}");
    }

    #[test]
    fn workspace_errors_do_not_echo_storage_details() {
        let connection = Connection::open_in_memory().unwrap();
        let sqlite = connection
            .execute_batch("SELECT * FROM sensitive_marker")
            .unwrap_err();
        let error = DatabaseError::from(sqlite);
        assert_eq!(error.to_string(), "SQLite 操作失败");
        assert!(!error.to_string().contains("sensitive_marker"));
        let io = DatabaseError::from(std::io::Error::other("sensitive-path"));
        assert_eq!(io.to_string(), "持久化 I/O 失败");
        assert!(!io.to_string().contains("sensitive-path"));
    }
}
