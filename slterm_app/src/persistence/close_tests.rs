use std::time::{Duration, Instant};

use rusqlite::{Connection, params};
use serde_json::{Value, json};
use slterm_document::DocumentId;
use slterm_path::{AppPaths, CanonicalPath, ResolvedPath};
use slterm_sidebar::side_view_state_schema;
use slterm_workspace::{
    CloseDecision, CloseObject, CloseScope, InvalidViewState, PanelGroup, PanelGroupId, ProjectId,
    ProjectRegistry, ProjectSnapshot, ProjectSnapshotError, RunIntent, SideViewId, TerminalId,
    TerminalRecord, ViewKey, ViewKeyKind, ViewState, ViewStateRegistry, ViewStateSchema,
};
use tempfile::TempDir;

use super::*;
use crate::persistence::metadata::RunMarker;
use crate::persistence::project::ProjectStoreError;

const EPOCH: &str = "epoch-close";
const OTHER_EPOCH: &str = "epoch-other";
const CLOSE_ID: &str = "close-a";
const PROJECT_A: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2a";
const PROJECT_B: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b";
const GROUP_A: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0a2a";
const GROUP_B: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0a2b";
const TERMINAL_A: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0b2a";
const TERMINAL_A_SECOND: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0b2b";
const TERMINAL_B: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0b2c";
const TERMINAL_B_SECOND: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0b2d";
const DOCUMENT_SHARED: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0f2a";
const DOCUMENT_EXCLUSIVE: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0f2b";

fn fixture(temp: &TempDir) -> AppPaths {
    AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap()
}

fn store(temp: &TempDir) -> WorkspaceDatabase {
    let mut store = WorkspaceDatabase::open(&fixture(temp)).unwrap();
    store
        .begin_run(
            EPOCH,
            RunMarker {
                run_id: "run-close".to_owned(),
                process_id: 17,
                process_created_at_ms: 456,
            },
        )
        .unwrap();
    store
}

fn project_id(text: &str) -> ProjectId {
    text.parse().unwrap()
}

fn object(key: &str, generation: u64, decision: CloseDecision) -> CloseObject<String> {
    CloseObject {
        id: key.to_owned(),
        generation,
        decision,
    }
}

fn registration(
    close_id: &str,
    scope: CloseScope,
    objects: Vec<CloseObject<String>>,
) -> PersistedCloseIntent {
    PersistedCloseIntent {
        close_id: close_id.to_owned(),
        scope,
        store_epoch: EPOCH.to_owned(),
        intent: CloseIntent::new(objects, [7; 32]).unwrap(),
        commit_status: CloseCommitStatus::Collected,
    }
}

fn empty_registration(close_id: &str) -> PersistedCloseIntent {
    registration(close_id, CloseScope::ApplicationExit, Vec::new())
}

fn object_registration(close_id: &str) -> PersistedCloseIntent {
    registration(
        close_id,
        CloseScope::Panels {
            project_id: project_id(PROJECT_A),
        },
        vec![object("terminal:a", 3, CloseDecision::Save)],
    )
}

fn close_snapshot(name: &str) -> ProjectSnapshot {
    let project_id = project_id(PROJECT_A);
    let mut registry = ProjectRegistry::new();
    registry
        .add_project(project_id, CanonicalPath::new("D:/work/close").unwrap())
        .unwrap();
    registry.rename_project(project_id, name).unwrap();
    ProjectSnapshot::restore(registry, Vec::new()).unwrap()
}

fn terminal_id(text: &str) -> TerminalId {
    text.parse().unwrap()
}

fn multi_project_snapshot(active: ProjectId) -> ProjectSnapshot {
    let mut registry = ProjectRegistry::new();
    let mut records = Vec::new();
    for (project, group, terminals) in [
        (PROJECT_A, GROUP_A, [TERMINAL_A, TERMINAL_A_SECOND]),
        (PROJECT_B, GROUP_B, [TERMINAL_B, TERMINAL_B_SECOND]),
    ] {
        let project_id = project_id(project);
        let root = CanonicalPath::new(format!("D:/work/{project}")).unwrap();
        registry.add_project(project_id, root.clone()).unwrap();
        registry.rename_project(project_id, project).unwrap();
        let views = terminals
            .map(|terminal| ViewKey::Terminal {
                project_id,
                terminal_id: terminal_id(terminal),
            })
            .to_vec();
        let active_view = views.first().cloned().unwrap();
        registry
            .layout_mut(project_id)
            .unwrap()
            .open_first_group(
                group.parse().unwrap(),
                PanelGroup::new(views, active_view).unwrap(),
            )
            .unwrap();
        for terminal in terminals {
            let number = registry.allocate_terminal_number(project_id).unwrap();
            records.push(
                TerminalRecord::new(
                    terminal_id(terminal),
                    project_id,
                    number,
                    root.clone(),
                    CanonicalPath::new("C:/shell/pwsh.exe").unwrap(),
                    Some(format!("终端 {terminal}")),
                    RunIntent::Exited,
                )
                .unwrap(),
            );
        }
    }
    registry.activate_project(active).unwrap();
    ProjectSnapshot::restore(registry, records).unwrap()
}

fn project_registration(
    close_id: &str,
    project_id: ProjectId,
    snapshot: &ProjectSnapshot,
) -> PersistedCloseIntent {
    registration(
        close_id,
        CloseScope::Project { project_id },
        snapshot
            .terminals()
            .filter(|terminal| terminal.project_id() == project_id)
            .map(|terminal| {
                object(
                    &format!("terminal:{}", terminal.id()),
                    terminal.number(),
                    CloseDecision::NotRequired,
                )
            })
            .collect(),
    )
}

fn document_project_snapshot() -> ProjectSnapshot {
    let original = multi_project_snapshot(project_id(PROJECT_A));
    let shared = DOCUMENT_SHARED.parse::<DocumentId>().unwrap();
    let exclusive = DOCUMENT_EXCLUSIVE.parse::<DocumentId>().unwrap();
    let shared_location =
        ResolvedPath::new(CanonicalPath::new("D:/work/shared.txt").unwrap(), None);
    let exclusive_location =
        ResolvedPath::new(CanonicalPath::new("D:/work/exclusive.txt").unwrap(), None);
    let (with_shared, _) = original
        .with_opened_document(
            project_id(PROJECT_A),
            GROUP_A.parse().unwrap(),
            shared,
            shared_location.clone(),
        )
        .unwrap();
    let (shared_across_projects, _) = with_shared
        .with_opened_document(
            project_id(PROJECT_B),
            GROUP_B.parse().unwrap(),
            shared,
            shared_location,
        )
        .unwrap();
    shared_across_projects
        .with_opened_document(
            project_id(PROJECT_A),
            GROUP_A.parse().unwrap(),
            exclusive,
            exclusive_location,
        )
        .unwrap()
        .0
}

fn decode_test_state(_key: &ViewKey, text: &str) -> Result<Value, InvalidViewState> {
    let state: Value = serde_json::from_str(text).map_err(|_| InvalidViewState)?;
    if state["position"].as_u64().is_some() {
        Ok(state)
    } else {
        Err(InvalidViewState)
    }
}

fn cleanup_view_registry() -> ViewStateRegistry {
    ViewStateRegistry::new(&[
        side_view_state_schema(),
        ViewStateSchema::new(ViewKeyKind::Settings, decode_test_state),
    ])
    .unwrap()
}

fn settings_key(project: &str) -> ViewKey {
    ViewKey::Settings {
        project_id: project_id(project),
    }
}

fn settings_state(project: &str, position: u64) -> ViewState {
    let key = settings_key(project);
    ViewState::new(
        &key,
        json!({"position": position}),
        &cleanup_view_registry(),
    )
    .unwrap()
}

fn side_view_state(side_view_id: SideViewId, value: Value) -> ViewState {
    ViewState::new(
        &ViewKey::SideView { side_view_id },
        value,
        &cleanup_view_registry(),
    )
    .unwrap()
}

fn nav_state() -> ViewState {
    side_view_state(
        SideViewId::Nav,
        json!({
            "projects": [
                {"project_id": PROJECT_A, "expanded": true, "history_expanded": false},
                {"project_id": PROJECT_B, "expanded": false, "history_expanded": true}
            ],
            "unassigned_expanded": true,
            "scroll": 12.0
        }),
    )
}

fn explorer_state() -> ViewState {
    side_view_state(
        SideViewId::Explorer,
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
        }),
    )
}

fn commit_state() -> ViewState {
    side_view_state(
        SideViewId::Commit,
        json!({
            "changes_collapsed": false,
            "unversioned_collapsed": true,
            "ignored_collapsed": false,
            "show_ignored": true,
            "scroll": 8.0
        }),
    )
}

fn project_rows(store: &WorkspaceDatabase, table: &str, project: &str) -> i64 {
    store
        .database
        .connection
        .query_row(
            &format!("SELECT COUNT(*) FROM {table} WHERE project_id = ?1"),
            [project],
            |row| row.get(0),
        )
        .unwrap()
}

fn active_project(store: &WorkspaceDatabase) -> Option<String> {
    store
        .database
        .connection
        .query_row(
            "SELECT active_project_id FROM app_state WHERE singleton = 1",
            [],
            |row| row.get(0),
        )
        .unwrap()
}

struct ProjectCleanupFixture {
    store: WorkspaceDatabase,
    original: ProjectSnapshot,
    current: PersistedCloseIntent,
    registry: ViewStateRegistry,
}

fn project_cleanup_fixture(temp: &TempDir, close_id: &str) -> ProjectCleanupFixture {
    let mut store = store(temp);
    let original = multi_project_snapshot(project_id(PROJECT_A));
    store.save_project_snapshot(&original).unwrap();
    let (candidate, _) = original.without_project(project_id(PROJECT_A)).unwrap();
    let collected = project_registration(close_id, project_id(PROJECT_A), &original);
    let committed = store
        .commit_project_close_intent(&candidate, &collected)
        .unwrap();
    let current = advance(&mut store, &committed, CloseCommitStatus::PtyStopped);
    let registry = cleanup_view_registry();
    for (project, position) in [(PROJECT_A, 1), (PROJECT_B, 2)] {
        let key = settings_key(project);
        store
            .save_view_state(&key, &settings_state(project, position))
            .unwrap();
    }
    for (side_view_id, state) in [
        (SideViewId::Nav, nav_state()),
        (SideViewId::Explorer, explorer_state()),
        (SideViewId::Commit, commit_state()),
    ] {
        let key = ViewKey::SideView { side_view_id };
        store.save_view_state(&key, &state).unwrap();
    }
    ProjectCleanupFixture {
        store,
        original,
        current,
        registry,
    }
}

fn invalid_close_snapshot() -> ProjectSnapshot {
    let mut snapshot = close_snapshot("invalid");
    let project_id = project_id(PROJECT_A);
    let view = ViewKey::Terminal {
        project_id,
        terminal_id: "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0b2a"
            .parse::<TerminalId>()
            .unwrap(),
    };
    snapshot
        .registry_mut()
        .layout_mut(project_id)
        .unwrap()
        .open_first_group(
            "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0a2a"
                .parse::<PanelGroupId>()
                .unwrap(),
            PanelGroup::new(vec![view.clone()], view).unwrap(),
        )
        .unwrap();
    snapshot
}

fn advance(
    store: &mut WorkspaceDatabase,
    current: &PersistedCloseIntent,
    next: CloseCommitStatus,
) -> PersistedCloseIntent {
    store
        .advance_close_intent(
            &current.close_id,
            current.commit_status,
            next,
            current.intent.fingerprint(),
            EPOCH,
        )
        .unwrap()
}

fn complete(store: &mut WorkspaceDatabase, current: &PersistedCloseIntent) {
    let mut current = current.clone();
    for next in [
        CloseCommitStatus::StateCommitted,
        CloseCommitStatus::PtyStopped,
        CloseCommitStatus::CleanupCommitted,
        CloseCommitStatus::Completed,
    ] {
        current = advance(store, &current, next);
    }
}

fn count(store: &WorkspaceDatabase, table: &str) -> i64 {
    store
        .database
        .connection
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .unwrap()
}

fn assert_preflight_did_not_wait(elapsed: Duration) {
    assert!(
        elapsed < Duration::from_secs(1),
        "事务前纯校验不得等待 busy_timeout"
    );
}

#[test]
fn close_intent_empty_and_multi_object_records_round_trip() {
    let empty_temp = tempfile::tempdir().unwrap();
    let paths = fixture(&empty_temp);
    let mut empty_store = store(&empty_temp);
    let empty = empty_registration(CLOSE_ID);
    empty_store.register_close_intent(&empty).unwrap();
    assert_eq!(count(&empty_store, "close_intents"), 1);
    assert_eq!(count(&empty_store, "close_objects"), 0);
    drop(empty_store);
    let mut reopened = WorkspaceDatabase::open(&paths).unwrap();
    assert_eq!(reopened.load_close_intent(CLOSE_ID).unwrap(), Some(empty));
    assert_eq!(reopened.load_close_intent("close-missing").unwrap(), None);

    let object_temp = tempfile::tempdir().unwrap();
    let mut store = store(&object_temp);
    let expected = registration(
        CLOSE_ID,
        CloseScope::Project {
            project_id: project_id(PROJECT_A),
        },
        vec![
            object("terminal:z", 9, CloseDecision::NotRequired),
            object("document:a", 2, CloseDecision::Discard),
            object("terminal:a", 3, CloseDecision::Save),
        ],
    );
    store.register_close_intent(&expected).unwrap();
    let loaded = store.load_close_intent(CLOSE_ID).unwrap().unwrap();
    assert_eq!(loaded, expected);
    assert_eq!(
        loaded
            .intent
            .objects()
            .map(|object| object.id.as_str())
            .collect::<Vec<_>>(),
        ["document:a", "terminal:a", "terminal:z"]
    );
}

#[test]
fn close_id_conflict_preserves_original_header_and_objects() {
    let temp = tempfile::tempdir().unwrap();
    let mut store = store(&temp);
    let original = object_registration(CLOSE_ID);
    store.register_close_intent(&original).unwrap();
    let replacement = registration(
        CLOSE_ID,
        CloseScope::UpdateHandoff,
        vec![object("document:replacement", 99, CloseDecision::Discard)],
    );

    assert!(matches!(
        store.register_close_intent(&replacement),
        Err(CloseStoreError::CloseIdConflict)
    ));
    assert_eq!(store.load_close_intent(CLOSE_ID).unwrap(), Some(original));
    assert_eq!(count(&store, "close_intents"), 1);
    assert_eq!(count(&store, "close_objects"), 1);
}

#[test]
fn unfinished_close_intents_are_ordered_and_exclude_only_completed() {
    let temp = tempfile::tempdir().unwrap();
    let mut store = store(&temp);
    for close_id in ["close-c", "close-a", "close-b"] {
        let current = registration(
            close_id,
            CloseScope::ApplicationExit,
            vec![
                object("terminal:z", 1, CloseDecision::Discard),
                object("document:a", 2, CloseDecision::Save),
            ],
        );
        store.register_close_intent(&current).unwrap();
        if close_id == "close-c" {
            complete(&mut store, &current);
        }
    }

    let unfinished = store.unfinished_close_intents().unwrap();
    assert_eq!(
        unfinished
            .iter()
            .map(|intent| intent.close_id.as_str())
            .collect::<Vec<_>>(),
        ["close-a", "close-b"]
    );
    for intent in &unfinished {
        assert_eq!(
            intent
                .intent
                .objects()
                .map(|object| object.id.as_str())
                .collect::<Vec<_>>(),
            ["document:a", "terminal:z"]
        );
    }
    assert_eq!(
        store
            .load_close_intent("close-c")
            .unwrap()
            .unwrap()
            .commit_status,
        CloseCommitStatus::Completed
    );
    assert_eq!(count(&store, "close_intents"), 3);
}

#[test]
fn close_registration_preflight_precedes_write_lock() {
    let temp = tempfile::tempdir().unwrap();
    let paths = fixture(&temp);
    let mut store = store(&temp);
    let lock = WorkspaceDatabase::open(&paths).unwrap();
    lock.database
        .connection
        .execute_batch("BEGIN EXCLUSIVE")
        .unwrap();

    let started = Instant::now();
    let result = store.register_close_intent(&empty_registration(""));
    let elapsed = started.elapsed();

    lock.database.connection.execute_batch("ROLLBACK").unwrap();
    assert!(matches!(result, Err(CloseStoreError::InvalidRegistration)));
    assert_preflight_did_not_wait(elapsed);
}

#[test]
fn invalid_registrations_write_no_partial_records() {
    let temp = tempfile::tempdir().unwrap();
    let mut store = store(&temp);
    let mut invalid = Vec::new();
    invalid.push(empty_registration(""));
    let mut empty_epoch = empty_registration(CLOSE_ID);
    empty_epoch.store_epoch.clear();
    invalid.push(empty_epoch);
    invalid.push(registration(
        CLOSE_ID,
        CloseScope::ApplicationExit,
        vec![object("", 0, CloseDecision::Save)],
    ));
    invalid.push(registration(
        CLOSE_ID,
        CloseScope::ApplicationExit,
        vec![object("terminal:a", u64::MAX, CloseDecision::Save)],
    ));
    let mut non_initial = empty_registration(CLOSE_ID);
    non_initial.commit_status = CloseCommitStatus::StateCommitted;
    invalid.push(non_initial);

    for candidate in invalid {
        assert!(matches!(
            store.register_close_intent(&candidate),
            Err(CloseStoreError::InvalidRegistration)
        ));
    }
    assert_eq!(count(&store, "close_intents"), 0);
    assert_eq!(count(&store, "close_objects"), 0);

    let stale = PersistedCloseIntent {
        store_epoch: OTHER_EPOCH.to_owned(),
        ..empty_registration(CLOSE_ID)
    };
    assert!(matches!(
        store.register_close_intent(&stale),
        Err(CloseStoreError::StoreEpochConflict)
    ));
    assert_eq!(count(&store, "close_intents"), 0);
}

#[test]
fn close_object_failure_rolls_back_header_and_prior_objects() {
    let temp = tempfile::tempdir().unwrap();
    let mut store = store(&temp);
    store
        .database
        .connection
        .execute_batch(
            "CREATE TRIGGER reject_close_object BEFORE INSERT ON close_objects \
             WHEN NEW.object_key = 'terminal:b' \
             BEGIN SELECT RAISE(ABORT, 'injected close object failure'); END;",
        )
        .unwrap();
    let candidate = registration(
        CLOSE_ID,
        CloseScope::ApplicationExit,
        vec![
            object("terminal:a", 1, CloseDecision::Save),
            object("terminal:b", 2, CloseDecision::Discard),
        ],
    );

    assert!(matches!(
        store.register_close_intent(&candidate),
        Err(CloseStoreError::Database(_))
    ));
    assert_eq!(count(&store, "close_intents"), 0);
    assert_eq!(count(&store, "close_objects"), 0);
}

#[test]
fn close_commit_saves_snapshot_and_advances_intent_atomically() {
    let temp = tempfile::tempdir().unwrap();
    let mut store = store(&temp);
    let snapshot = close_snapshot("committed");
    let collected = object_registration(CLOSE_ID);

    let committed = store.commit_close_intent(&snapshot, &collected).unwrap();

    assert_eq!(committed.commit_status, CloseCommitStatus::StateCommitted);
    assert_eq!(store.load_project_snapshot().unwrap(), snapshot);
    assert_eq!(store.load_close_intent(CLOSE_ID).unwrap(), Some(committed));
}

#[test]
fn close_commit_rejects_invalid_snapshot_without_registration() {
    let temp = tempfile::tempdir().unwrap();
    let mut store = store(&temp);

    assert!(matches!(
        store.commit_close_intent(&invalid_close_snapshot(), &object_registration(CLOSE_ID)),
        Err(CloseCommitError::Project(_))
    ));
    assert_eq!(
        store.load_project_snapshot().unwrap(),
        ProjectSnapshot::default()
    );
    assert_eq!(count(&store, "close_intents"), 0);
    assert_eq!(count(&store, "close_objects"), 0);
}

#[test]
fn close_commit_snapshot_preflight_precedes_write_lock() {
    let temp = tempfile::tempdir().unwrap();
    let paths = fixture(&temp);
    let mut store = store(&temp);
    let lock = WorkspaceDatabase::open(&paths).unwrap();
    lock.database
        .connection
        .execute_batch("BEGIN EXCLUSIVE")
        .unwrap();

    let started = Instant::now();
    let result =
        store.commit_close_intent(&invalid_close_snapshot(), &object_registration(CLOSE_ID));
    let elapsed = started.elapsed();

    lock.database.connection.execute_batch("ROLLBACK").unwrap();
    let terminal_id = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0b2a"
        .parse::<TerminalId>()
        .unwrap();
    assert!(matches!(
        result,
        Err(CloseCommitError::Project(ProjectStoreError::Snapshot(
            ProjectSnapshotError::MissingTerminal(missing)
        ))) if missing == terminal_id
    ));
    assert_preflight_did_not_wait(elapsed);
}

#[test]
fn close_commit_registration_preflight_precedes_write_lock() {
    let temp = tempfile::tempdir().unwrap();
    let paths = fixture(&temp);
    let mut store = store(&temp);
    let lock = WorkspaceDatabase::open(&paths).unwrap();
    lock.database
        .connection
        .execute_batch("BEGIN EXCLUSIVE")
        .unwrap();

    let started = Instant::now();
    let result = store.commit_close_intent(&close_snapshot("valid"), &empty_registration(""));
    let elapsed = started.elapsed();

    lock.database.connection.execute_batch("ROLLBACK").unwrap();
    assert!(matches!(
        result,
        Err(CloseCommitError::Close(
            CloseStoreError::InvalidRegistration
        ))
    ));
    assert_preflight_did_not_wait(elapsed);
}

#[test]
fn close_commit_close_id_conflict_rolls_back_snapshot() {
    let temp = tempfile::tempdir().unwrap();
    let mut store = store(&temp);
    let original_snapshot = close_snapshot("original");
    let original_intent = object_registration(CLOSE_ID);
    store.save_project_snapshot(&original_snapshot).unwrap();
    store.register_close_intent(&original_intent).unwrap();
    let replacement = registration(
        CLOSE_ID,
        CloseScope::ApplicationExit,
        vec![object("terminal:replacement", 8, CloseDecision::Discard)],
    );

    assert!(matches!(
        store.commit_close_intent(&close_snapshot("replacement"), &replacement),
        Err(CloseCommitError::Close(CloseStoreError::CloseIdConflict))
    ));
    assert_eq!(store.load_project_snapshot().unwrap(), original_snapshot);
    assert_eq!(
        store.load_close_intent(CLOSE_ID).unwrap(),
        Some(original_intent)
    );
}

#[test]
fn close_commit_object_failure_rolls_back_snapshot_and_records() {
    let temp = tempfile::tempdir().unwrap();
    let mut store = store(&temp);
    store
        .database
        .connection
        .execute_batch(
            "CREATE TRIGGER reject_commit_close_object BEFORE INSERT ON close_objects \
             WHEN NEW.object_key = 'terminal:b' \
             BEGIN SELECT RAISE(ABORT, 'injected close commit failure'); END;",
        )
        .unwrap();
    let collected = registration(
        CLOSE_ID,
        CloseScope::ApplicationExit,
        vec![
            object("terminal:a", 1, CloseDecision::Save),
            object("terminal:b", 2, CloseDecision::Discard),
        ],
    );

    assert!(matches!(
        store.commit_close_intent(&close_snapshot("rolled-back"), &collected),
        Err(CloseCommitError::Close(CloseStoreError::Database(_)))
    ));
    assert_eq!(
        store.load_project_snapshot().unwrap(),
        ProjectSnapshot::default()
    );
    assert_eq!(count(&store, "close_intents"), 0);
    assert_eq!(count(&store, "close_objects"), 0);
}

#[test]
fn close_commit_state_failure_rolls_back_snapshot_header_and_objects() {
    let temp = tempfile::tempdir().unwrap();
    let mut store = store(&temp);
    let original = close_snapshot("original");
    store.save_project_snapshot(&original).unwrap();
    store
        .database
        .connection
        .execute_batch(
            "CREATE TRIGGER reject_state_commit BEFORE UPDATE ON close_intents \
             WHEN NEW.commit_status = 'state_committed' \
             BEGIN SELECT RAISE(ABORT, 'injected state commit failure'); END;",
        )
        .unwrap();
    let collected = registration(
        CLOSE_ID,
        CloseScope::ApplicationExit,
        vec![
            object("terminal:a", 1, CloseDecision::Save),
            object("terminal:b", 2, CloseDecision::Discard),
        ],
    );

    assert!(matches!(
        store.commit_close_intent(&close_snapshot("rolled-back"), &collected),
        Err(CloseCommitError::Close(CloseStoreError::Database(_)))
    ));
    assert_eq!(store.load_project_snapshot().unwrap(), original);
    assert_eq!(count(&store, "close_intents"), 0);
    assert_eq!(count(&store, "close_objects"), 0);
}

#[test]
fn project_close_commit_retains_target_until_cleanup_and_persists_next_active() {
    for (target, other) in [(PROJECT_A, PROJECT_B), (PROJECT_B, PROJECT_A)] {
        let temp = tempfile::tempdir().unwrap();
        let mut store = store(&temp);
        let target_id = project_id(target);
        let other_id = project_id(other);
        let original = multi_project_snapshot(target_id);
        store.save_project_snapshot(&original).unwrap();
        let (mut candidate, _) = original.without_project(target_id).unwrap();
        candidate
            .registry_mut()
            .rename_project(other_id, "其他项目已更新")
            .unwrap();
        let collected = project_registration(CLOSE_ID, target_id, &original);

        let committed = store
            .commit_project_close_intent(&candidate, &collected)
            .unwrap();

        assert_eq!(committed.commit_status, CloseCommitStatus::StateCommitted);
        assert_eq!(committed.intent.objects().count(), 2);
        assert_eq!(count(&store, "close_objects"), 2);
        assert_eq!(
            store.load_close_intent(CLOSE_ID).unwrap(),
            Some(committed.clone())
        );
        for table in ["projects", "project_layouts", "panel_groups"] {
            assert_eq!(project_rows(&store, table, target), 1, "{table} 必须保留");
        }
        assert_eq!(project_rows(&store, "terminals", target), 2);
        assert_eq!(project_rows(&store, "terminals", other), 2);
        assert_eq!(active_project(&store).as_deref(), Some(other));
        let restored = store.load_project_snapshot().unwrap();
        assert!(restored.registry().project(target_id).is_some());
        assert_eq!(
            restored
                .registry()
                .project(other_id)
                .unwrap()
                .display_name(),
            "其他项目已更新"
        );

        let stopped = advance(&mut store, &committed, CloseCommitStatus::PtyStopped);
        let cleaned = store
            .cleanup_project_close(&stopped, &cleanup_view_registry())
            .unwrap();
        assert_eq!(cleaned.commit_status, CloseCommitStatus::CleanupCommitted);
        for table in ["projects", "project_layouts", "panel_groups", "terminals"] {
            assert_eq!(
                project_rows(&store, table, target),
                0,
                "{table} 必须级联删除"
            );
        }
        assert_eq!(project_rows(&store, "projects", other), 1);
        assert_eq!(project_rows(&store, "terminals", other), 2);
        let completed = advance(&mut store, &cleaned, CloseCommitStatus::Completed);
        assert_eq!(completed.commit_status, CloseCommitStatus::Completed);
    }
}

#[test]
fn project_cleanup_deletes_exclusive_documents_and_retains_shared_documents() {
    let temp = tempfile::tempdir().unwrap();
    let mut store = store(&temp);
    let original = document_project_snapshot();
    store.save_project_snapshot(&original).unwrap();
    let (candidate, _) = original.without_project(project_id(PROJECT_A)).unwrap();
    let collected = project_registration(CLOSE_ID, project_id(PROJECT_A), &original);
    let committed = store
        .commit_project_close_intent(&candidate, &collected)
        .unwrap();
    let current = advance(&mut store, &committed, CloseCommitStatus::PtyStopped);
    assert_eq!(count(&store, "documents"), 2);

    let cleaned = store
        .cleanup_project_close(&current, &cleanup_view_registry())
        .unwrap();

    assert_eq!(cleaned.commit_status, CloseCommitStatus::CleanupCommitted);
    assert_eq!(store.load_project_snapshot().unwrap(), candidate);
    assert_eq!(count(&store, "documents"), 1);
    let remaining = store.load_project_snapshot().unwrap();
    assert!(
        remaining
            .document(DOCUMENT_SHARED.parse().unwrap())
            .is_some()
    );
    assert!(
        remaining
            .document(DOCUMENT_EXCLUSIVE.parse().unwrap())
            .is_none()
    );
}

#[test]
fn project_cleanup_document_delete_failure_rolls_back_and_allows_retry() {
    let temp = tempfile::tempdir().unwrap();
    let mut store = store(&temp);
    let original = document_project_snapshot();
    store.save_project_snapshot(&original).unwrap();
    let (candidate, _) = original.without_project(project_id(PROJECT_A)).unwrap();
    let collected = project_registration(CLOSE_ID, project_id(PROJECT_A), &original);
    let committed = store
        .commit_project_close_intent(&candidate, &collected)
        .unwrap();
    let current = advance(&mut store, &committed, CloseCommitStatus::PtyStopped);
    let before_cleanup = store.load_project_snapshot().unwrap();
    store
        .database
        .connection
        .execute_batch(&format!(
            "CREATE TRIGGER reject_project_cleanup BEFORE DELETE ON documents \
             WHEN OLD.document_id = '{DOCUMENT_EXCLUSIVE}' \
             BEGIN SELECT RAISE(ABORT, 'injected document delete failure'); END;"
        ))
        .unwrap();

    assert!(
        store
            .cleanup_project_close(&current, &cleanup_view_registry())
            .is_err()
    );
    assert_eq!(store.load_project_snapshot().unwrap(), before_cleanup);
    assert_eq!(count(&store, "documents"), 2);
    assert_eq!(
        store.load_close_intent(CLOSE_ID).unwrap(),
        Some(current.clone())
    );

    store
        .database
        .connection
        .execute_batch("DROP TRIGGER reject_project_cleanup")
        .unwrap();
    store
        .cleanup_project_close(&current, &cleanup_view_registry())
        .unwrap();
    assert_eq!(store.load_project_snapshot().unwrap(), candidate);
}

#[test]
fn project_cleanup_removes_only_target_view_states_and_side_view_partitions() {
    let temp = tempfile::tempdir().unwrap();
    let fixture = &mut project_cleanup_fixture(&temp, CLOSE_ID);
    let original = fixture.original.clone();
    let current = fixture.current.clone();

    let cleaned = fixture
        .store
        .cleanup_project_close(&current, &fixture.registry)
        .unwrap();

    assert_eq!(cleaned.commit_status, CloseCommitStatus::CleanupCommitted);
    assert_eq!(
        fixture
            .store
            .view_state(&settings_key(PROJECT_A), &fixture.registry)
            .unwrap(),
        None
    );
    assert_eq!(
        fixture
            .store
            .view_state(&settings_key(PROJECT_B), &fixture.registry)
            .unwrap()
            .unwrap()
            .state
            .state()["position"],
        json!(2)
    );
    for side_view_id in [SideViewId::Nav, SideViewId::Explorer] {
        let key = ViewKey::SideView { side_view_id };
        let stored = fixture
            .store
            .view_state(&key, &fixture.registry)
            .unwrap()
            .unwrap();
        assert_eq!(stored.revision, 1);
        assert_eq!(
            stored.state.state()["projects"].as_array().unwrap().len(),
            1
        );
        assert_eq!(
            stored.state.state()["projects"][0]["project_id"],
            json!(PROJECT_B)
        );
    }
    let commit_key = ViewKey::SideView {
        side_view_id: SideViewId::Commit,
    };
    assert_eq!(
        fixture
            .store
            .view_state(&commit_key, &fixture.registry)
            .unwrap()
            .unwrap()
            .revision,
        0
    );
    assert_eq!(project_rows(&fixture.store, "terminals", PROJECT_B), 2);
    assert_eq!(
        fixture.store.load_project_snapshot().unwrap(),
        original.without_project(project_id(PROJECT_A)).unwrap().0
    );
    let completed = advance(&mut fixture.store, &cleaned, CloseCommitStatus::Completed);
    assert_eq!(completed.commit_status, CloseCommitStatus::Completed);
}

#[test]
fn empty_project_close_commits_and_cleans_up_without_close_objects() {
    let temp = tempfile::tempdir().unwrap();
    let mut store = store(&temp);
    let project_id = project_id(PROJECT_A);
    let mut registry = ProjectRegistry::new();
    registry
        .add_project(
            project_id,
            CanonicalPath::new("D:/work/empty-project").unwrap(),
        )
        .unwrap();
    let original = ProjectSnapshot::restore(registry, Vec::new()).unwrap();
    store.save_project_snapshot(&original).unwrap();
    let (candidate, _) = original.without_project(project_id).unwrap();
    let collected = registration(CLOSE_ID, CloseScope::Project { project_id }, Vec::new());

    let committed = store
        .commit_project_close_intent(&candidate, &collected)
        .unwrap();
    assert_eq!(count(&store, "close_objects"), 0);
    assert_eq!(project_rows(&store, "projects", PROJECT_A), 1);
    let stopped = advance(&mut store, &committed, CloseCommitStatus::PtyStopped);
    let cleaned = store
        .cleanup_project_close(&stopped, &cleanup_view_registry())
        .unwrap();
    assert_eq!(project_rows(&store, "projects", PROJECT_A), 0);
    let completed = advance(&mut store, &cleaned, CloseCommitStatus::Completed);
    assert_eq!(completed.commit_status, CloseCommitStatus::Completed);
    assert!(store.unfinished_close_intents().unwrap().is_empty());
}

#[test]
fn project_close_rejects_invalid_scope_candidate_and_cleanup_preconditions() {
    let temp = tempfile::tempdir().unwrap();
    let mut store = store(&temp);
    let target = project_id(PROJECT_A);
    let original = multi_project_snapshot(target);
    store.save_project_snapshot(&original).unwrap();
    let (candidate, _) = original.without_project(target).unwrap();

    assert!(matches!(
        store.commit_project_close_intent(
            &candidate,
            &registration(CLOSE_ID, CloseScope::ApplicationExit, Vec::new()),
        ),
        Err(CloseCommitError::Close(
            CloseStoreError::InvalidRegistration
        ))
    ));
    assert!(matches!(
        store.commit_project_close_intent(
            &original,
            &project_registration(CLOSE_ID, target, &original),
        ),
        Err(CloseCommitError::Close(CloseStoreError::CloseConflict))
    ));
    assert!(
        matches!(
            store.commit_close_intent(
                &candidate,
                &project_registration("close-generic-project", target, &original),
            ),
            Err(CloseCommitError::Close(
                CloseStoreError::InvalidRegistration
            ))
        ),
        "项目关闭不得经通用组合提交提前删除记录"
    );
    assert_eq!(count(&store, "close_intents"), 0);
    assert_eq!(store.load_project_snapshot().unwrap(), original);

    let collected = project_registration(CLOSE_ID, target, &original);
    let committed = store
        .commit_project_close_intent(&candidate, &collected)
        .unwrap();
    let stopped = advance(&mut store, &committed, CloseCommitStatus::PtyStopped);
    let registry = cleanup_view_registry();
    assert!(
        matches!(
            store.advance_close_intent(
                CLOSE_ID,
                CloseCommitStatus::PtyStopped,
                CloseCommitStatus::CleanupCommitted,
                stopped.intent.fingerprint(),
                &stopped.store_epoch,
            ),
            Err(CloseStoreError::InvalidTransition)
        ),
        "通用状态推进不得跳过项目清理"
    );
    let before = store.load_project_snapshot().unwrap();

    let wrong_status = store.cleanup_project_close(&committed, &registry);
    assert!(
        matches!(
            &wrong_status,
            Err(ProjectCleanupError::Close(CloseStoreError::CloseConflict))
        ),
        "错前序必须拒绝且不改写：{wrong_status:?}"
    );
    let mut wrong_id = stopped.clone();
    wrong_id.close_id = "close-missing".to_owned();
    assert!(matches!(
        store.cleanup_project_close(&wrong_id, &registry),
        Err(ProjectCleanupError::Close(
            CloseStoreError::UnknownCloseIntent
        ))
    ));
    let mut wrong_fingerprint = stopped.clone();
    wrong_fingerprint.intent =
        CloseIntent::new(stopped.intent.objects().cloned().collect(), [8; 32]).unwrap();
    assert!(matches!(
        store.cleanup_project_close(&wrong_fingerprint, &registry),
        Err(ProjectCleanupError::Close(CloseStoreError::CloseConflict))
    ));
    let mut wrong_epoch = stopped.clone();
    wrong_epoch.store_epoch = OTHER_EPOCH.to_owned();
    assert!(matches!(
        store.cleanup_project_close(&wrong_epoch, &registry),
        Err(ProjectCleanupError::Close(
            CloseStoreError::StoreEpochConflict
        ))
    ));
    let mut wrong_scope = stopped.clone();
    wrong_scope.scope = CloseScope::ApplicationExit;
    assert!(matches!(
        store.cleanup_project_close(&wrong_scope, &registry),
        Err(ProjectCleanupError::Close(
            CloseStoreError::InvalidRegistration
        ))
    ));
    assert_eq!(store.load_project_snapshot().unwrap(), before);
    assert_eq!(store.load_close_intent(CLOSE_ID).unwrap(), Some(stopped));
    assert_eq!(project_rows(&store, "terminals", PROJECT_A), 2);
}

#[test]
fn ordinary_snapshot_and_close_commit_keep_eager_project_deletion() {
    let temp = tempfile::tempdir().unwrap();
    let mut store = store(&temp);
    let original = multi_project_snapshot(project_id(PROJECT_A));
    let (candidate, _) = original.without_project(project_id(PROJECT_A)).unwrap();
    store.save_project_snapshot(&original).unwrap();
    store.save_project_snapshot(&candidate).unwrap();
    assert_eq!(project_rows(&store, "projects", PROJECT_A), 0);
    assert_eq!(project_rows(&store, "terminals", PROJECT_A), 0);

    store.save_project_snapshot(&original).unwrap();
    store
        .commit_close_intent(&candidate, &empty_registration(CLOSE_ID))
        .unwrap();
    assert_eq!(project_rows(&store, "projects", PROJECT_A), 0);
    assert_eq!(project_rows(&store, "terminals", PROJECT_A), 0);
}

#[test]
fn project_cleanup_faults_roll_back_every_fact_and_allow_same_current_retry() {
    let faults = [
        format!(
            "CREATE TRIGGER reject_project_cleanup BEFORE DELETE ON projects \
             WHEN OLD.project_id = '{PROJECT_A}' \
             BEGIN SELECT RAISE(ABORT, 'injected project delete failure'); END;"
        ),
        format!(
            "CREATE TRIGGER reject_project_cleanup BEFORE DELETE ON view_states \
             WHEN OLD.scope = 'project:{PROJECT_A}' \
             BEGIN SELECT RAISE(ABORT, 'injected view delete failure'); END;"
        ),
        "CREATE TRIGGER reject_project_cleanup BEFORE UPDATE ON view_states \
         WHEN OLD.view_key LIKE '%nav%' \
         BEGIN SELECT RAISE(ABORT, 'injected nav update failure'); END;"
            .to_owned(),
        "CREATE TRIGGER reject_project_cleanup BEFORE UPDATE ON close_intents \
         WHEN NEW.commit_status = 'cleanup_committed' \
         BEGIN SELECT RAISE(ABORT, 'injected state advance failure'); END;"
            .to_owned(),
    ];
    for fault in faults {
        let temp = tempfile::tempdir().unwrap();
        let fixture = &mut project_cleanup_fixture(&temp, CLOSE_ID);
        let current = fixture.current.clone();
        let before = fixture.store.load_project_snapshot().unwrap();
        let nav_key = ViewKey::SideView {
            side_view_id: SideViewId::Nav,
        };
        let before_nav = fixture
            .store
            .view_state(&nav_key, &fixture.registry)
            .unwrap();
        fixture
            .store
            .database
            .connection
            .execute_batch(&fault)
            .unwrap();

        assert!(
            fixture
                .store
                .cleanup_project_close(&current, &fixture.registry)
                .is_err()
        );
        assert_eq!(fixture.store.load_project_snapshot().unwrap(), before);
        assert_eq!(project_rows(&fixture.store, "terminals", PROJECT_A), 2);
        assert!(
            fixture
                .store
                .view_state(&settings_key(PROJECT_A), &fixture.registry)
                .unwrap()
                .is_some()
        );
        assert_eq!(
            fixture
                .store
                .view_state(&nav_key, &fixture.registry)
                .unwrap(),
            before_nav
        );
        assert_eq!(
            fixture.store.load_close_intent(CLOSE_ID).unwrap(),
            Some(current.clone())
        );

        fixture
            .store
            .database
            .connection
            .execute_batch("DROP TRIGGER reject_project_cleanup")
            .unwrap();
        let cleaned = fixture
            .store
            .cleanup_project_close(&current, &fixture.registry)
            .unwrap();
        assert_eq!(cleaned.commit_status, CloseCommitStatus::CleanupCommitted);
        assert_eq!(project_rows(&fixture.store, "projects", PROJECT_A), 0);
        assert!(
            fixture
                .store
                .view_state(&settings_key(PROJECT_A), &fixture.registry)
                .unwrap()
                .is_none()
        );
    }
}

#[test]
fn project_cleanup_errors_do_not_echo_persisted_details() {
    let marker = "sensitive-project-cleanup-path-sql-state";
    let connection = Connection::open_in_memory().unwrap();
    let sqlite = connection
        .execute_batch(&format!("SELECT * FROM {marker}"))
        .unwrap_err();
    let errors = [
        ProjectCleanupError::ViewState(
            crate::persistence::view_state::ViewStateStoreError::CorruptRecord,
        ),
        ProjectCleanupError::Project(ProjectStoreError::CorruptRecord),
        ProjectCleanupError::Close(CloseStoreError::CloseConflict),
        ProjectCleanupError::Database(DatabaseError::from(sqlite)),
    ];
    for error in errors {
        assert!(!error.to_string().contains(marker));
    }
}

#[test]
fn close_commit_errors_do_not_echo_persisted_details() {
    let marker = "sensitive-close-commit-path-sql-fingerprint";
    let connection = Connection::open_in_memory().unwrap();
    let sqlite = connection
        .execute_batch(&format!("SELECT * FROM {marker}"))
        .unwrap_err();
    let database = DatabaseError::from(sqlite);
    let errors = [
        CloseCommitError::Project(ProjectStoreError::Database(database)),
        CloseCommitError::Close(CloseStoreError::CloseIdConflict),
    ];

    for error in errors {
        assert!(!error.to_string().contains(marker));
    }
}

#[test]
fn corrupt_close_headers_are_reported_without_repair() {
    let cases = [
        (
            "unknown scope",
            "UPDATE close_intents SET scope = 'unknown'",
        ),
        (
            "unknown status",
            "UPDATE close_intents SET commit_status = 'unknown'",
        ),
        (
            "short fingerprint",
            "UPDATE close_intents SET commit_fingerprint = zeroblob(31)",
        ),
        ("empty epoch", "UPDATE close_intents SET store_epoch = ''"),
        (
            "global project mismatch",
            "UPDATE close_intents SET project_id = 'project-unexpected'",
        ),
        ("non-text scope", "UPDATE close_intents SET scope = x'ff'"),
    ];
    for (name, corruption) in cases {
        let temp = tempfile::tempdir().unwrap();
        let mut store = store(&temp);
        store
            .register_close_intent(&empty_registration(CLOSE_ID))
            .unwrap();
        store
            .database
            .connection
            .pragma_update(None, "ignore_check_constraints", true)
            .unwrap();
        store.database.connection.execute(corruption, []).unwrap();

        assert!(
            matches!(
                store.load_close_intent(CLOSE_ID),
                Err(CloseStoreError::CorruptRecord)
            ),
            "{name} 必须报告损坏"
        );
        assert!(
            matches!(
                store.unfinished_close_intents(),
                Err(CloseStoreError::CorruptRecord)
            ),
            "{name} 不得在未完成枚举中被跳过"
        );
        assert_eq!(count(&store, "close_intents"), 1);
    }
}

#[test]
fn corrupt_close_objects_and_project_scopes_are_reported() {
    let object_cases = [
        (
            "unknown decision",
            "UPDATE close_objects SET decision = 'unknown'",
        ),
        (
            "negative generation",
            "UPDATE close_objects SET object_generation = -1",
        ),
        (
            "empty object key",
            "UPDATE close_objects SET object_key = ''",
        ),
        (
            "non-text decision",
            "UPDATE close_objects SET decision = x'ff'",
        ),
    ];
    for (name, corruption) in object_cases {
        let temp = tempfile::tempdir().unwrap();
        let mut store = store(&temp);
        store
            .register_close_intent(&object_registration(CLOSE_ID))
            .unwrap();
        store
            .database
            .connection
            .pragma_update(None, "ignore_check_constraints", true)
            .unwrap();
        store.database.connection.execute(corruption, []).unwrap();
        assert!(
            matches!(
                store.load_close_intent(CLOSE_ID),
                Err(CloseStoreError::CorruptRecord)
            ),
            "{name} 必须报告损坏"
        );
    }

    let project_cases = [
        ("missing project", None),
        ("invalid project", Some("not-a-project")),
    ];
    for (name, project) in project_cases {
        let temp = tempfile::tempdir().unwrap();
        let mut store = store(&temp);
        store
            .register_close_intent(&registration(
                CLOSE_ID,
                CloseScope::Project {
                    project_id: project_id(PROJECT_A),
                },
                Vec::new(),
            ))
            .unwrap();
        store
            .database
            .connection
            .execute("UPDATE close_intents SET project_id = ?1", [project])
            .unwrap();
        assert!(
            matches!(
                store.load_close_intent(CLOSE_ID),
                Err(CloseStoreError::CorruptRecord)
            ),
            "{name} 必须报告损坏"
        );
    }
}

#[test]
fn empty_persisted_close_id_is_corrupt_and_input_id_is_validated() {
    let temp = tempfile::tempdir().unwrap();
    let mut store = store(&temp);
    store
        .register_close_intent(&empty_registration(CLOSE_ID))
        .unwrap();
    store
        .database
        .connection
        .execute("UPDATE close_intents SET close_id = ''", [])
        .unwrap();

    assert!(matches!(
        store.unfinished_close_intents(),
        Err(CloseStoreError::CorruptRecord)
    ));
    assert!(matches!(
        store.load_close_intent(""),
        Err(CloseStoreError::InvalidRegistration)
    ));
}

#[test]
fn close_status_advances_in_fixed_order_and_retains_history() {
    let temp = tempfile::tempdir().unwrap();
    let mut store = store(&temp);
    assert_eq!(
        CloseCommitStatus::ALL,
        [
            CloseCommitStatus::Collected,
            CloseCommitStatus::StateCommitted,
            CloseCommitStatus::PtyStopped,
            CloseCommitStatus::CleanupCommitted,
            CloseCommitStatus::Completed,
        ]
    );
    let registered = object_registration(CLOSE_ID);
    store.register_close_intent(&registered).unwrap();
    let mut current = registered;
    for next in CloseCommitStatus::ALL.into_iter().skip(1) {
        let expected = current.commit_status;
        current = advance(&mut store, &current, next);
        assert_eq!(current.commit_status, next);
        assert_ne!(current.commit_status, expected);
        assert_eq!(
            store.load_close_intent(CLOSE_ID).unwrap(),
            Some(current.clone())
        );
    }
    assert!(store.unfinished_close_intents().unwrap().is_empty());
    assert_eq!(count(&store, "close_intents"), 1);
    assert_eq!(count(&store, "close_objects"), 1);
}

#[test]
fn rejected_close_transitions_never_modify_records() {
    let temp = tempfile::tempdir().unwrap();
    let mut store = store(&temp);
    let original = object_registration(CLOSE_ID);
    store.register_close_intent(&original).unwrap();
    let fingerprint = original.intent.fingerprint();

    assert!(matches!(
        store.advance_close_intent(
            CLOSE_ID,
            CloseCommitStatus::Collected,
            CloseCommitStatus::PtyStopped,
            fingerprint,
            EPOCH,
        ),
        Err(CloseStoreError::InvalidTransition)
    ));
    assert!(matches!(
        store.advance_close_intent(
            CLOSE_ID,
            CloseCommitStatus::Collected,
            CloseCommitStatus::Collected,
            fingerprint,
            EPOCH,
        ),
        Err(CloseStoreError::InvalidTransition)
    ));
    assert!(matches!(
        store.advance_close_intent(
            CLOSE_ID,
            CloseCommitStatus::StateCommitted,
            CloseCommitStatus::PtyStopped,
            fingerprint,
            EPOCH,
        ),
        Err(CloseStoreError::CloseConflict)
    ));
    assert!(matches!(
        store.advance_close_intent(
            CLOSE_ID,
            CloseCommitStatus::Collected,
            CloseCommitStatus::StateCommitted,
            &[8; 32],
            EPOCH,
        ),
        Err(CloseStoreError::CloseConflict)
    ));
    store
        .rotate_store_epoch("run-close", EPOCH, OTHER_EPOCH)
        .unwrap();
    assert!(matches!(
        store.advance_close_intent(
            CLOSE_ID,
            CloseCommitStatus::Collected,
            CloseCommitStatus::StateCommitted,
            fingerprint,
            EPOCH,
        ),
        Err(CloseStoreError::StoreEpochConflict)
    ));
    store
        .rotate_store_epoch("run-close", OTHER_EPOCH, EPOCH)
        .unwrap();
    assert!(matches!(
        store.advance_close_intent(
            "close-missing",
            CloseCommitStatus::Collected,
            CloseCommitStatus::StateCommitted,
            fingerprint,
            EPOCH,
        ),
        Err(CloseStoreError::UnknownCloseIntent)
    ));
    assert_eq!(
        store.load_close_intent(CLOSE_ID).unwrap(),
        Some(original.clone())
    );

    let committed = advance(&mut store, &original, CloseCommitStatus::StateCommitted);
    assert!(matches!(
        store.advance_close_intent(
            CLOSE_ID,
            CloseCommitStatus::StateCommitted,
            CloseCommitStatus::Collected,
            fingerprint,
            EPOCH,
        ),
        Err(CloseStoreError::InvalidTransition)
    ));
    assert_eq!(store.load_close_intent(CLOSE_ID).unwrap(), Some(committed));
}

#[test]
fn close_schema_enforces_foreign_keys_nulls_and_value_constraints() {
    let temp = tempfile::tempdir().unwrap();
    let store = store(&temp);
    let connection = &store.database.connection;
    assert!(
        connection
            .execute(
                "INSERT INTO close_objects \
                 (close_id, object_key, object_generation, decision) \
                 VALUES ('close-missing', 'terminal:a', 0, 'save')",
                [],
            )
            .is_err()
    );
    connection
        .execute(
            "INSERT INTO close_intents \
             (close_id, scope, project_id, store_epoch, commit_fingerprint, commit_status) \
             VALUES (?1, 'application_exit', NULL, ?2, zeroblob(32), 'collected')",
            params![CLOSE_ID, EPOCH],
        )
        .unwrap();
    assert!(
        connection
            .execute(
                "INSERT INTO close_objects \
                 (close_id, object_key, object_generation, decision) \
                 VALUES (?1, NULL, 0, 'save')",
                [CLOSE_ID],
            )
            .is_err()
    );
    assert!(
        connection
            .execute(
                "INSERT INTO close_objects \
                 (close_id, object_key, object_generation, decision) \
                 VALUES (NULL, 'terminal:a', 0, 'save')",
                [],
            )
            .is_err()
    );
    assert!(
        connection
            .execute(
                "INSERT INTO close_objects \
                 (close_id, object_key, object_generation, decision) \
                 VALUES (?1, 'terminal:a', -1, 'save')",
                [CLOSE_ID],
            )
            .is_err()
    );
    assert!(
        connection
            .execute(
                "INSERT INTO close_intents \
                 (close_id, scope, store_epoch, commit_fingerprint, commit_status) \
                 VALUES ('close-short', 'application_exit', ?1, zeroblob(31), 'collected')",
                [EPOCH],
            )
            .is_err()
    );
    assert!(
        connection
            .execute(
                "INSERT INTO close_intents \
                 (close_id, scope, store_epoch, commit_fingerprint, commit_status) \
                 VALUES (NULL, 'application_exit', ?1, zeroblob(32), 'collected')",
                [EPOCH],
            )
            .is_err()
    );
    connection
        .execute(
            "INSERT INTO close_objects \
             (close_id, object_key, object_generation, decision) \
             VALUES (?1, 'terminal:a', 0, 'save')",
            [CLOSE_ID],
        )
        .unwrap();
    assert!(
        connection
            .execute(
                "INSERT INTO close_objects \
                 (close_id, object_key, object_generation, decision) \
                 VALUES (?1, 'terminal:a', 1, 'discard')",
                [CLOSE_ID],
            )
            .is_err()
    );
}

#[test]
fn close_store_errors_do_not_echo_persisted_details() {
    let marker = "sensitive-object-path-sql-fingerprint";
    let connection = Connection::open_in_memory().unwrap();
    let sqlite = connection
        .execute_batch(&format!("SELECT * FROM {marker}"))
        .unwrap_err();
    let errors = [
        CloseStoreError::InvalidRegistration,
        CloseStoreError::CloseIdConflict,
        CloseStoreError::UnknownCloseIntent,
        CloseStoreError::CloseConflict,
        CloseStoreError::InvalidTransition,
        CloseStoreError::StoreEpochConflict,
        CloseStoreError::CorruptRecord,
        CloseStoreError::Database(DatabaseError::from(sqlite)),
    ];
    for error in errors {
        assert!(!error.to_string().contains(marker));
    }
}

#[test]
fn close_scope_round_trips_update_handoff_without_project() {
    let temp = tempfile::tempdir().unwrap();
    let mut store = store(&temp);
    let expected = registration(CLOSE_ID, CloseScope::UpdateHandoff, Vec::new());
    store.register_close_intent(&expected).unwrap();
    assert_eq!(store.load_close_intent(CLOSE_ID).unwrap(), Some(expected));
    assert_eq!(
        store
            .database
            .connection
            .query_row(
                "SELECT project_id FROM close_intents WHERE close_id = ?1",
                [CLOSE_ID],
                |row| row.get::<_, Option<String>>(0),
            )
            .unwrap(),
        None
    );
}
