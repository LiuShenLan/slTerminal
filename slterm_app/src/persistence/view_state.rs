//! `ViewState` 的唯一 SQLite 读写通道与生命周期作用域清理。

use std::fmt;

use rusqlite::{Connection, OptionalExtension, Row, TransactionBehavior, params};
use serde::Deserialize;
use slterm_sidebar::SideViewState;
use slterm_workspace::{
    ProjectId, SideViewId, ViewKey, ViewKeyError, ViewState, ViewStateError, ViewStateRegistry,
    ViewStateScope,
};

use super::database::DatabaseError;
use super::workspace::WorkspaceDatabase;

/// 已从规范 DTO 恢复的视图状态及其单调存储 revision。
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct StoredViewState {
    pub(crate) state: ViewState,
    pub(crate) revision: u64,
}

/// 视图状态读写失败；不向展示层暴露 SQL、路径或状态内容。
#[derive(Debug)]
pub(crate) enum ViewStateStoreError {
    ViewKey(ViewKeyError),
    ViewState(ViewStateError),
    CorruptRecord,
    RevisionExhausted,
    Database(DatabaseError),
}

impl fmt::Display for ViewStateStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::ViewKey(_) => "ViewState 的 ViewKey 编码失败",
            Self::ViewState(error) => return fmt::Display::fmt(error, formatter),
            Self::CorruptRecord => "ViewState 持久记录不一致",
            Self::RevisionExhausted => "ViewState revision 已耗尽",
            Self::Database(_) => "ViewState 数据库操作失败",
        })
    }
}

impl std::error::Error for ViewStateStoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ViewKey(error) => Some(error),
            Self::ViewState(error) => Some(error),
            Self::Database(error) => Some(error),
            Self::CorruptRecord | Self::RevisionExhausted => None,
        }
    }
}

impl From<ViewKeyError> for ViewStateStoreError {
    fn from(error: ViewKeyError) -> Self {
        Self::ViewKey(error)
    }
}

impl From<ViewStateError> for ViewStateStoreError {
    fn from(error: ViewStateError) -> Self {
        Self::ViewState(error)
    }
}

impl From<DatabaseError> for ViewStateStoreError {
    fn from(error: DatabaseError) -> Self {
        Self::Database(error)
    }
}

impl From<rusqlite::Error> for ViewStateStoreError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Database(DatabaseError::from(error))
    }
}

impl WorkspaceDatabase {
    /// 原子写入规范 ViewKey、ViewState 与推导作用域，并返回新的 revision。
    pub(crate) fn save_view_state(
        &mut self,
        key: &ViewKey,
        state: &ViewState,
    ) -> Result<u64, ViewStateStoreError> {
        let transaction = self
            .database
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let revision = save_view_state_in(&transaction, key, state)?;
        transaction.commit()?;
        Ok(revision)
    }

    /// 按精确 ViewKey 恢复单条状态；缺失与读取异常不得混为一谈。
    pub(crate) fn view_state(
        &self,
        key: &ViewKey,
        registry: &ViewStateRegistry,
    ) -> Result<Option<StoredViewState>, ViewStateStoreError> {
        view_state_in(&self.database.connection, key, registry)
    }

    /// 删除一个逻辑视图的持久状态，不影响同作用域的其他视图。
    pub(crate) fn delete_view_state(&mut self, key: &ViewKey) -> Result<bool, ViewStateStoreError> {
        let key_json = key.to_canonical_json()?;
        let changed = self
            .database
            .connection
            .execute("DELETE FROM view_states WHERE view_key = ?1", [&key_json])?;
        Ok(changed == 1)
    }

    /// 项目作用域记录与 Nav／Explorer 对应分区在同一事务内清理。
    pub(crate) fn delete_project_view_states(
        &mut self,
        project_id: ProjectId,
        registry: &ViewStateRegistry,
    ) -> Result<usize, ViewStateStoreError> {
        let scope = scope_text(ViewStateScope::Project(project_id));
        let transaction = self
            .database
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let deleted = transaction.execute("DELETE FROM view_states WHERE scope = ?1", [scope])?;
        for side_view_id in [SideViewId::Nav, SideViewId::Explorer] {
            let key = ViewKey::SideView { side_view_id };
            let Some(stored) = view_state_in(&transaction, &key, registry)? else {
                continue;
            };
            let mut state = SideViewState::from_view_state(side_view_id, stored.state)
                .map_err(|_| ViewStateStoreError::CorruptRecord)?;
            if state.remove_project(project_id) {
                save_view_state_in(&transaction, &key, &state.to_view_state(registry)?)?;
            }
        }
        transaction.commit()?;
        Ok(deleted)
    }

    /// AgentRoot 只清理 AgentFiles 对应根分区，缺失身份不推进 revision。
    pub(crate) fn delete_agent_root_view_states(
        &mut self,
        agent_root_id: slterm_settings::AgentRootId,
        registry: &ViewStateRegistry,
    ) -> Result<bool, ViewStateStoreError> {
        let transaction = self
            .database
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let key = ViewKey::SideView {
            side_view_id: SideViewId::AgentFiles,
        };
        let Some(stored) = view_state_in(&transaction, &key, registry)? else {
            return Ok(false);
        };
        let mut state = SideViewState::from_view_state(SideViewId::AgentFiles, stored.state)
            .map_err(|_| ViewStateStoreError::CorruptRecord)?;
        if !state.remove_agent_root(agent_root_id) {
            return Ok(false);
        }
        save_view_state_in(&transaction, &key, &state.to_view_state(registry)?)?;
        transaction.commit()?;
        Ok(true)
    }
}

/// 在调用方事务内保存；事务所有者统一决定提交或整体回滚。
fn save_view_state_in(
    connection: &Connection,
    key: &ViewKey,
    state: &ViewState,
) -> Result<u64, ViewStateStoreError> {
    let key_json = key.to_canonical_json()?;
    let scope = scope_text(state.scope(key)?);
    let state_json = state.to_canonical_json()?;
    let dto_version = envelope_version(&state_json)
        .ok_or(ViewStateError::InvalidStructure)
        .map_err(ViewStateStoreError::from)?;
    let stored_dto_version = i64::try_from(dto_version)
        .map_err(|_| ViewStateStoreError::ViewState(ViewStateError::UnsupportedVersion))?;
    let existing = connection
        .query_row(
            "SELECT dto_version, state_json, revision FROM view_states WHERE view_key = ?1",
            [&key_json],
            |row| Ok(decode_existing_record(row)),
        )
        .optional()?
        .transpose()?;
    if let Some((existing_version, existing_json, existing_revision)) = existing {
        let existing_version =
            u64::try_from(existing_version).map_err(|_| ViewStateStoreError::CorruptRecord)?;
        if existing_version > dto_version
            || envelope_version(&existing_json).is_some_and(|version| version > dto_version)
        {
            return Err(ViewStateError::UnsupportedVersion.into());
        }
        let current =
            u64::try_from(existing_revision).map_err(|_| ViewStateStoreError::CorruptRecord)?;
        let revision = current
            .checked_add(1)
            .ok_or(ViewStateStoreError::RevisionExhausted)?;
        let stored_revision =
            i64::try_from(revision).map_err(|_| ViewStateStoreError::RevisionExhausted)?;
        let changed = connection.execute(
            "UPDATE view_states \
             SET dto_version = ?2, state_json = ?3, revision = ?4, scope = ?5 \
             WHERE view_key = ?1",
            params![
                key_json,
                stored_dto_version,
                state_json,
                stored_revision,
                scope
            ],
        )?;
        if changed != 1 {
            return Err(ViewStateStoreError::CorruptRecord);
        }
        Ok(revision)
    } else {
        connection.execute(
            "INSERT INTO view_states (view_key, dto_version, state_json, revision, scope) \
             VALUES (?1, ?2, ?3, 0, ?4)",
            params![key_json, stored_dto_version, state_json, scope],
        )?;
        Ok(0)
    }
}

/// 在调用方连接或事务内读取，保持缺失、损坏与版本错误边界一致。
fn view_state_in(
    connection: &Connection,
    key: &ViewKey,
    registry: &ViewStateRegistry,
) -> Result<Option<StoredViewState>, ViewStateStoreError> {
    let key_json = key.to_canonical_json()?;
    let raw = connection
        .query_row(
            "SELECT dto_version, state_json, revision, scope \
             FROM view_states WHERE view_key = ?1",
            [&key_json],
            |row| Ok(decode_record(row)),
        )
        .optional()?
        .transpose()?;
    let Some((stored_version, state_json, stored_revision, stored_scope)) = raw else {
        return Ok(None);
    };
    let stored_version =
        u64::try_from(stored_version).map_err(|_| ViewStateStoreError::CorruptRecord)?;
    let revision =
        u64::try_from(stored_revision).map_err(|_| ViewStateStoreError::CorruptRecord)?;
    let state = ViewState::from_canonical_json(&state_json, key, registry)?;
    if envelope_version(&state_json).ok_or(ViewStateStoreError::CorruptRecord)? != stored_version
        || scope_text(state.scope(key)?) != stored_scope
    {
        return Err(ViewStateStoreError::CorruptRecord);
    }
    Ok(Some(StoredViewState { state, revision }))
}

#[derive(Deserialize)]
struct ViewStateEnvelope {
    version: u64,
}

fn decode_existing_record(row: &Row<'_>) -> Result<(i64, String, i64), ViewStateStoreError> {
    Ok((required(row, 0)?, required(row, 1)?, required(row, 2)?))
}

fn decode_record(row: &Row<'_>) -> Result<(i64, String, i64, String), ViewStateStoreError> {
    Ok((
        required(row, 0)?,
        required(row, 1)?,
        required(row, 2)?,
        required(row, 3)?,
    ))
}

fn required<T: rusqlite::types::FromSql>(
    row: &Row<'_>,
    index: usize,
) -> Result<T, ViewStateStoreError> {
    row.get(index)
        .map_err(|_| ViewStateStoreError::CorruptRecord)
}

/// 存储版本列必须与规范 DTO 自带版本一致，不能形成第二份版本事实。
fn envelope_version(state_json: &str) -> Option<u64> {
    serde_json::from_str::<ViewStateEnvelope>(state_json)
        .ok()
        .map(|envelope| envelope.version)
}

fn scope_text(scope: ViewStateScope) -> String {
    match scope {
        ViewStateScope::Project(project_id) => format!("project:{project_id}"),
        ViewStateScope::Application => "application".to_owned(),
    }
}

#[cfg(test)]
mod view_state_store_tests {
    use std::str::FromStr;

    use rusqlite::params;
    use serde::{Deserialize, Serialize};
    use serde_json::{Value, json};
    use slterm_path::AppPaths;
    use slterm_sidebar::side_view_state_schema;
    use slterm_workspace::{InvalidViewState, SideViewId, ViewKeyKind, ViewStateSchema};
    use tempfile::TempDir;

    use super::*;

    const PROJECT_A: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2a";
    const PROJECT_B: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b";
    const ROOT_A: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2c";
    const ROOT_B: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2d";

    #[derive(Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct TestState {
        persistent: bool,
        position: u64,
    }

    fn decode_test_state(_key: &ViewKey, text: &str) -> Result<Value, InvalidViewState> {
        let state: TestState = serde_json::from_str(text).map_err(|_| InvalidViewState)?;
        serde_json::to_value(state).map_err(|_| InvalidViewState)
    }

    fn registry() -> ViewStateRegistry {
        ViewStateRegistry::new(
            &[
                ViewKeyKind::SideView,
                ViewKeyKind::Settings,
                ViewKeyKind::AppSettingsOverlay,
            ]
            .map(|kind| ViewStateSchema::new(kind, decode_test_state)),
        )
        .unwrap()
    }

    fn cleanup_registry() -> ViewStateRegistry {
        ViewStateRegistry::new(&[
            side_view_state_schema(),
            ViewStateSchema::new(ViewKeyKind::Settings, decode_test_state),
            ViewStateSchema::new(ViewKeyKind::AppSettingsOverlay, decode_test_state),
        ])
        .unwrap()
    }

    fn cleanup_side_state(view: SideViewId, value: Value) -> ViewState {
        ViewState::new(
            &ViewKey::SideView { side_view_id: view },
            value,
            &cleanup_registry(),
        )
        .unwrap()
    }

    fn nav_partitions() -> Value {
        json!({
            "projects": [
                {"project_id": PROJECT_A, "expanded": true, "history_expanded": false},
                {"project_id": PROJECT_B, "expanded": false, "history_expanded": true}
            ],
            "unassigned_expanded": true,
            "scroll": 12.0
        })
    }

    fn explorer_partitions() -> Value {
        json!({
            "projects": [
                {
                    "project_id": PROJECT_A,
                    "expanded": ["src"],
                    "current_row": "src/main.rs",
                    "scroll": 24.0,
                    "show_ignored": true
                },
                {
                    "project_id": PROJECT_B,
                    "expanded": ["docs"],
                    "current_row": null,
                    "scroll": 48.0,
                    "show_ignored": false
                }
            ]
        })
    }

    fn agent_partitions() -> Value {
        json!({
            "roots": [
                {"agent_root_id": ROOT_A, "expanded": true, "directories": ["skills"]},
                {"agent_root_id": ROOT_B, "expanded": false, "directories": []}
            ],
            "current_row": {"agent_root_id": ROOT_A, "path": "skills/run.md"},
            "scroll": 36.0
        })
    }

    fn fixture(temp: &TempDir) -> AppPaths {
        AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap()
    }

    fn project_id(text: &str) -> ProjectId {
        ProjectId::from_str(text).unwrap()
    }

    fn side_view_key() -> ViewKey {
        ViewKey::SideView {
            side_view_id: SideViewId::Nav,
        }
    }

    fn settings_key(project: &str) -> ViewKey {
        ViewKey::Settings {
            project_id: project_id(project),
        }
    }

    fn state(key: &ViewKey, position: u64) -> ViewState {
        ViewState::new(
            key,
            json!({"persistent":true,"position":position}),
            &registry(),
        )
        .unwrap()
    }

    fn stored(state: ViewState, revision: u64) -> Option<StoredViewState> {
        Some(StoredViewState { state, revision })
    }

    fn insert_raw(
        store: &WorkspaceDatabase,
        key: &ViewKey,
        dto_version: i64,
        state_json: &str,
        revision: i64,
        scope: &str,
    ) {
        store
            .database
            .connection
            .execute(
                "INSERT INTO view_states (view_key, dto_version, state_json, revision, scope) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    key.to_canonical_json().unwrap(),
                    dto_version,
                    state_json,
                    revision,
                    scope
                ],
            )
            .unwrap();
    }

    /// 规范字节、作用域与 revision 一同持久化，重开后恢复同一 ViewKey 状态。
    #[test]
    fn view_state_store_round_trips_canonical_record_and_revisions() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let mut store = WorkspaceDatabase::open(&paths).unwrap();
        let key = side_view_key();
        let initial = state(&side_view_key(), 7);

        assert_eq!(store.save_view_state(&key, &initial).unwrap(), 0);
        assert_eq!(
            store.view_state(&key, &registry()).unwrap(),
            stored(initial.clone(), 0)
        );
        let raw = store
            .database
            .connection
            .query_row(
                "SELECT view_key, dto_version, state_json, revision, scope \
             FROM view_states WHERE view_key = ?1",
                [key.to_canonical_json().unwrap()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, String>(4)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(raw.0, key.to_canonical_json().unwrap());
        assert_eq!(raw.1, 1);
        assert_eq!(raw.2, initial.to_canonical_json().unwrap());
        assert_eq!(raw.3, 0);
        assert_eq!(raw.4, "application");

        let updated = state(&side_view_key(), 8);
        assert_eq!(store.save_view_state(&key, &updated).unwrap(), 1);
        drop(store);
        let store = WorkspaceDatabase::open(&paths).unwrap();
        assert_eq!(
            store.view_state(&key, &registry()).unwrap(),
            stored(updated, 1)
        );
    }

    /// ViewState 与 ViewKey 类型不匹配时拒写，不能形成错绑记录。
    #[test]
    fn view_state_store_rejects_key_mismatch_without_writing() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let mut store = WorkspaceDatabase::open(&paths).unwrap();
        let key = side_view_key();
        let mismatched = state(&ViewKey::AppSettingsOverlay, 7);

        assert!(matches!(
            store.save_view_state(&key, &mismatched),
            Err(ViewStateStoreError::ViewState(ViewStateError::KeyMismatch))
        ));
        assert_eq!(store.view_state(&key, &registry()).unwrap(), None);
    }

    /// 版本列、规范 DTO、ViewKey 与作用域必须一致；异常记录不被静默删除。
    #[test]
    fn view_state_store_load_distinguishes_corruption_and_unsupported_version() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let mut store = WorkspaceDatabase::open(&paths).unwrap();
        let key = side_view_key();
        let valid = state(&side_view_key(), 7).to_canonical_json().unwrap();
        let mismatched = state(&ViewKey::AppSettingsOverlay, 7)
            .to_canonical_json()
            .unwrap();
        let future = r#"{"kind":"side_view","state":{"persistent":true,"position":7},"version":2}"#;

        insert_raw(&store, &key, 1, &valid, 0, "project:orphan");
        assert!(matches!(
            store.view_state(&key, &registry()),
            Err(ViewStateStoreError::CorruptRecord)
        ));
        store
            .database
            .connection
            .execute(
                "UPDATE view_states SET scope = 'application', dto_version = 2 WHERE view_key = ?1",
                [key.to_canonical_json().unwrap()],
            )
            .unwrap();
        assert!(matches!(
            store.view_state(&key, &registry()),
            Err(ViewStateStoreError::CorruptRecord)
        ));
        store
            .database
            .connection
            .execute(
                "UPDATE view_states SET dto_version = 1, state_json = ?2 WHERE view_key = ?1",
                params![key.to_canonical_json().unwrap(), mismatched],
            )
            .unwrap();
        assert!(matches!(
            store.view_state(&key, &registry()),
            Err(ViewStateStoreError::ViewState(ViewStateError::KeyMismatch))
        ));
        store
            .database
            .connection
            .execute(
                "UPDATE view_states SET dto_version = 2, state_json = ?2 WHERE view_key = ?1",
                params![key.to_canonical_json().unwrap(), future],
            )
            .unwrap();
        assert!(matches!(
            store.view_state(&key, &registry()),
            Err(ViewStateStoreError::ViewState(
                ViewStateError::UnsupportedVersion
            ))
        ));
        let persisted = store
            .database
            .connection
            .query_row(
                "SELECT dto_version, state_json FROM view_states WHERE view_key = ?1",
                [key.to_canonical_json().unwrap()],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
            )
            .unwrap();
        assert_eq!(persisted, (2, future.to_owned()));
        for (version, state_json) in [(2, future), (1, future), (2, valid.as_str())] {
            store
                .database
                .connection
                .execute(
                    "UPDATE view_states SET dto_version = ?2, state_json = ?3 WHERE view_key = ?1",
                    params![key.to_canonical_json().unwrap(), version, state_json],
                )
                .unwrap();
            assert!(matches!(
                store.save_view_state(&key, &state(&side_view_key(), 9)),
                Err(ViewStateStoreError::ViewState(
                    ViewStateError::UnsupportedVersion
                ))
            ));
            let after_rejected_save = store
                .database
                .connection
                .query_row(
                    "SELECT dto_version, state_json FROM view_states WHERE view_key = ?1",
                    [key.to_canonical_json().unwrap()],
                    |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
                )
                .unwrap();
            assert_eq!(after_rejected_save, (version, state_json.to_owned()));
        }
    }

    /// 项目清理只删除该项目作用域，应用级与其他项目状态保持可恢复。
    #[test]
    fn view_state_store_scope_deletion_preserves_unrelated_records() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let mut store = WorkspaceDatabase::open(&paths).unwrap();
        let project_a = settings_key(PROJECT_A);
        let project_b = settings_key(PROJECT_B);
        let application = side_view_key();
        for (key, position) in [(&project_a, 1), (&project_b, 2)] {
            store.save_view_state(key, &state(key, position)).unwrap();
        }
        store
            .save_view_state(
                &application,
                &cleanup_side_state(SideViewId::Nav, nav_partitions()),
            )
            .unwrap();

        assert_eq!(
            store
                .delete_project_view_states(project_id(PROJECT_A), &cleanup_registry())
                .unwrap(),
            1
        );
        assert_eq!(store.view_state(&project_a, &registry()).unwrap(), None);
        assert!(store.view_state(&project_b, &registry()).unwrap().is_some());
        assert!(
            store
                .view_state(&application, &cleanup_registry())
                .unwrap()
                .is_some()
        );

        assert!(store.delete_view_state(&application).unwrap());
        assert!(!store.delete_view_state(&application).unwrap());
        assert!(store.view_state(&project_b, &registry()).unwrap().is_some());
    }

    /// 项目清理在同一入口删除项目作用域并精确更新 Nav／Explorer 分区。
    #[test]
    fn view_state_project_cleanup_preserves_other_side_view_partitions() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let mut store = WorkspaceDatabase::open(&paths).unwrap();
        let registry = cleanup_registry();
        let project_a = settings_key(PROJECT_A);
        let project_b = settings_key(PROJECT_B);
        store
            .save_view_state(&project_a, &state(&project_a, 1))
            .unwrap();
        store
            .save_view_state(&project_b, &state(&project_b, 2))
            .unwrap();
        let commit_key = ViewKey::SideView {
            side_view_id: SideViewId::Commit,
        };
        let commit = cleanup_side_state(
            SideViewId::Commit,
            json!({
                "changes_collapsed": false,
                "unversioned_collapsed": true,
                "ignored_collapsed": false,
                "show_ignored": true,
                "scroll": 8.0
            }),
        );
        for (key, state) in [
            (
                side_view_key(),
                cleanup_side_state(SideViewId::Nav, nav_partitions()),
            ),
            (
                ViewKey::SideView {
                    side_view_id: SideViewId::Explorer,
                },
                cleanup_side_state(SideViewId::Explorer, explorer_partitions()),
            ),
            (commit_key.clone(), commit.clone()),
        ] {
            store.save_view_state(&key, &state).unwrap();
        }

        assert_eq!(
            store
                .delete_project_view_states(project_id(PROJECT_A), &registry)
                .unwrap(),
            1
        );
        assert_eq!(store.view_state(&project_a, &registry).unwrap(), None);
        assert_eq!(
            store.view_state(&project_b, &registry).unwrap(),
            stored(state(&project_b, 2), 0)
        );
        for (view, partition_field) in [
            (SideViewId::Nav, "projects"),
            (SideViewId::Explorer, "projects"),
        ] {
            let key = ViewKey::SideView { side_view_id: view };
            let stored = store.view_state(&key, &registry).unwrap().unwrap();
            assert_eq!(stored.revision, 1);
            assert_eq!(
                stored.state.state()[partition_field]
                    .as_array()
                    .unwrap()
                    .len(),
                1
            );
            assert_eq!(
                stored.state.state()[partition_field][0]["project_id"],
                json!(PROJECT_B)
            );
        }
        assert_eq!(
            store.view_state(&commit_key, &registry).unwrap(),
            stored(commit, 0)
        );

        let before_repeat = [SideViewId::Nav, SideViewId::Explorer].map(|side_view_id| {
            let key = ViewKey::SideView { side_view_id };
            let state = store.view_state(&key, &registry).unwrap();
            (key, state)
        });
        assert_eq!(
            store
                .delete_project_view_states(project_id(PROJECT_A), &registry)
                .unwrap(),
            0
        );
        for (key, state) in before_repeat {
            assert_eq!(store.view_state(&key, &registry).unwrap(), state);
        }
    }

    /// AgentRoot 清理保留其他根，清除被删根当前行且空操作不推进 revision。
    #[test]
    fn view_state_agent_root_cleanup_is_precise_and_idempotent() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let mut store = WorkspaceDatabase::open(&paths).unwrap();
        let registry = cleanup_registry();
        let key = ViewKey::SideView {
            side_view_id: SideViewId::AgentFiles,
        };
        store
            .save_view_state(
                &key,
                &cleanup_side_state(SideViewId::AgentFiles, agent_partitions()),
            )
            .unwrap();
        let root_a = slterm_settings::AgentRootId::from_str(ROOT_A).unwrap();

        assert!(
            store
                .delete_agent_root_view_states(root_a, &registry)
                .unwrap()
        );
        let cleaned = store.view_state(&key, &registry).unwrap().unwrap();
        assert_eq!(cleaned.revision, 1);
        assert_eq!(cleaned.state.state()["roots"].as_array().unwrap().len(), 1);
        assert_eq!(
            cleaned.state.state()["roots"][0]["agent_root_id"],
            json!(ROOT_B)
        );
        assert_eq!(cleaned.state.state()["current_row"], Value::Null);
        assert_eq!(cleaned.state.state()["scroll"], json!(36.0));

        assert!(
            !store
                .delete_agent_root_view_states(root_a, &registry)
                .unwrap()
        );
        assert_eq!(store.view_state(&key, &registry).unwrap(), Some(cleaned));
    }

    /// 项目作用域删除后的任一分区更新失败，整个清理事务必须回滚且可重试。
    #[test]
    fn view_state_project_cleanup_rolls_back_every_change_on_failure() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let mut store = WorkspaceDatabase::open(&paths).unwrap();
        let registry = cleanup_registry();
        let project_a = settings_key(PROJECT_A);
        let nav_key = side_view_key();
        let nav = cleanup_side_state(SideViewId::Nav, nav_partitions());
        let explorer_key = ViewKey::SideView {
            side_view_id: SideViewId::Explorer,
        };
        let explorer = cleanup_side_state(SideViewId::Explorer, explorer_partitions());
        store
            .save_view_state(&project_a, &state(&project_a, 1))
            .unwrap();
        store.save_view_state(&nav_key, &nav).unwrap();
        store.save_view_state(&explorer_key, &explorer).unwrap();
        store
            .database
            .connection
            .execute_batch(
                "CREATE TRIGGER reject_cleanup_update BEFORE UPDATE ON view_states \
                 WHEN NEW.view_key LIKE '%explorer%' \
                 BEGIN SELECT RAISE(ABORT, 'injected cleanup failure'); END;",
            )
            .unwrap();

        assert!(
            store
                .delete_project_view_states(project_id(PROJECT_A), &registry)
                .is_err()
        );
        assert!(store.view_state(&project_a, &registry).unwrap().is_some());
        assert_eq!(
            store.view_state(&nav_key, &registry).unwrap(),
            stored(nav, 0)
        );
        assert_eq!(
            store.view_state(&explorer_key, &registry).unwrap(),
            stored(explorer, 0)
        );

        store
            .database
            .connection
            .execute_batch("DROP TRIGGER reject_cleanup_update")
            .unwrap();
        assert_eq!(
            store
                .delete_project_view_states(project_id(PROJECT_A), &registry)
                .unwrap(),
            1
        );
        assert_eq!(store.view_state(&project_a, &registry).unwrap(), None);
        let cleaned = store.view_state(&nav_key, &registry).unwrap().unwrap();
        assert_eq!(
            cleaned.state.state()["projects"].as_array().unwrap().len(),
            1
        );
        assert_eq!(
            cleaned.state.state()["projects"][0]["project_id"],
            json!(PROJECT_B)
        );
        let cleaned = store.view_state(&explorer_key, &registry).unwrap().unwrap();
        assert_eq!(
            cleaned.state.state()["projects"].as_array().unwrap().len(),
            1
        );
        assert_eq!(
            cleaned.state.state()["projects"][0]["project_id"],
            json!(PROJECT_B)
        );
    }

    /// revision 耗尽拒绝覆盖，最后一条已提交状态仍可读取。
    #[test]
    fn view_state_store_revision_exhaustion_preserves_last_state() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let mut store = WorkspaceDatabase::open(&paths).unwrap();
        let key = side_view_key();
        let initial = state(&side_view_key(), 7);
        store.save_view_state(&key, &initial).unwrap();
        store
            .database
            .connection
            .execute(
                "UPDATE view_states SET revision = ?2 WHERE view_key = ?1",
                params![key.to_canonical_json().unwrap(), i64::MAX],
            )
            .unwrap();

        assert!(matches!(
            store.save_view_state(&key, &state(&side_view_key(), 8)),
            Err(ViewStateStoreError::RevisionExhausted)
        ));
        assert_eq!(
            store.view_state(&key, &registry()).unwrap(),
            stored(initial, i64::MAX as u64)
        );
    }

    /// SQLite 非严格列中的错误存储类属于记录损坏，不能伪装成普通 I/O 失败。
    #[test]
    fn view_state_store_rejects_invalid_sqlite_storage_classes() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let mut store = WorkspaceDatabase::open(&paths).unwrap();
        let key = side_view_key();
        let initial = state(&side_view_key(), 7);
        let key_json = key.to_canonical_json().unwrap();
        store.save_view_state(&key, &initial).unwrap();

        store
            .database
            .connection
            .execute(
                "UPDATE view_states SET dto_version = 'invalid' WHERE view_key = ?1",
                [&key_json],
            )
            .unwrap();
        assert!(matches!(
            store.view_state(&key, &registry()),
            Err(ViewStateStoreError::CorruptRecord)
        ));

        store
            .database
            .connection
            .execute(
                "UPDATE view_states SET dto_version = 1, revision = 'invalid' WHERE view_key = ?1",
                [&key_json],
            )
            .unwrap();
        assert!(matches!(
            store.view_state(&key, &registry()),
            Err(ViewStateStoreError::CorruptRecord)
        ));
        assert!(matches!(
            store.save_view_state(&key, &initial),
            Err(ViewStateStoreError::CorruptRecord)
        ));

        store
            .database
            .connection
            .execute(
                "UPDATE view_states SET revision = 0, state_json = 7 WHERE view_key = ?1",
                [&key_json],
            )
            .unwrap();
        // TEXT 亲和列已将整数写成文本 "7"，由核心 DTO 判定结构非法。
        assert!(matches!(
            store.view_state(&key, &registry()),
            Err(ViewStateStoreError::ViewState(
                ViewStateError::InvalidStructure
            ))
        ));

        store
            .database
            .connection
            .pragma_update(None, "ignore_check_constraints", true)
            .unwrap();
        store
            .database
            .connection
            .execute(
                "UPDATE view_states SET state_json = ?2, scope = x'00' WHERE view_key = ?1",
                params![key_json, initial.to_canonical_json().unwrap()],
            )
            .unwrap();
        store
            .database
            .connection
            .pragma_update(None, "ignore_check_constraints", false)
            .unwrap();
        assert!(matches!(
            store.view_state(&key, &registry()),
            Err(ViewStateStoreError::CorruptRecord)
        ));
    }

    /// 注入写失败不破坏已提交记录；故障解除后可重试同一内存状态。
    #[test]
    fn view_state_store_failed_save_preserves_record_and_allows_retry() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let mut store = WorkspaceDatabase::open(&paths).unwrap();
        let key = side_view_key();
        let initial = state(&side_view_key(), 7);
        let pending = state(&side_view_key(), 8);
        store.save_view_state(&key, &initial).unwrap();
        store
            .database
            .connection
            .execute_batch(
                "CREATE TRIGGER reject_view_state_write BEFORE UPDATE ON view_states \
             BEGIN SELECT RAISE(ABORT, 'injected write failure'); END;",
            )
            .unwrap();

        let result = store.save_view_state(&key, &pending);
        assert!(matches!(result, Err(ViewStateStoreError::Database(_))));
        let error = result.unwrap_err();
        assert_eq!(error.to_string(), "ViewState 数据库操作失败");
        assert_eq!(
            store.view_state(&key, &registry()).unwrap(),
            stored(initial, 0)
        );

        store
            .database
            .connection
            .execute_batch("DROP TRIGGER reject_view_state_write")
            .unwrap();
        assert_eq!(store.save_view_state(&key, &pending).unwrap(), 1);
        assert_eq!(
            store.view_state(&key, &registry()).unwrap(),
            stored(pending, 1)
        );
    }
}
