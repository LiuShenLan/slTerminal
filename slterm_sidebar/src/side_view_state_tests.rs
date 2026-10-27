use serde_json::{Value, json};
use slterm_workspace::{SideViewId, ViewKey, ViewState, ViewStateError, ViewStateRegistry};

use super::{SideViewState, side_view_state_schema};

const PROJECT_A: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2a";
const PROJECT_B: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b";
const ROOT_A: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2c";
const ROOT_B: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2d";

fn registry() -> ViewStateRegistry {
    ViewStateRegistry::new(&[side_view_state_schema()]).unwrap()
}

fn key(view: SideViewId) -> ViewKey {
    ViewKey::SideView { side_view_id: view }
}

fn create(view: SideViewId, value: Value) -> Result<ViewState, ViewStateError> {
    ViewState::new(&key(view), value, &registry())
}

fn restore_raw(view: SideViewId, state: &str) -> Result<ViewState, ViewStateError> {
    let envelope = format!(r#"{{"kind":"side_view","state":{state},"version":1}}"#);
    ViewState::from_canonical_json(&envelope, &key(view), &registry())
}

fn nav_state() -> Value {
    json!({
        "projects": [
            {"project_id": PROJECT_B, "expanded": false, "history_expanded": true},
            {"project_id": PROJECT_A, "expanded": true, "history_expanded": false}
        ],
        "unassigned_expanded": true,
        "scroll": 12.5
    })
}

fn explorer_state() -> Value {
    json!({
        "projects": [{
            "project_id": PROJECT_A,
            "expanded": ["src/net", "docs", "src"],
            "current_row": "src/main.rs",
            "scroll": 48.0,
            "show_ignored": true
        }]
    })
}

fn commit_state() -> Value {
    json!({
        "changes_collapsed": false,
        "unversioned_collapsed": true,
        "ignored_collapsed": false,
        "show_ignored": true,
        "scroll": 24.0
    })
}

fn agent_files_state() -> Value {
    json!({
        "roots": [
            {"agent_root_id": ROOT_B, "expanded": false, "directories": []},
            {"agent_root_id": ROOT_A, "expanded": true, "directories": ["skills/a", "skills"]}
        ],
        "current_row": {"agent_root_id": ROOT_A, "path": "skills/a/run.md"},
        "scroll": 36.0
    })
}

fn persistent_state(view: SideViewId) -> Value {
    match view {
        SideViewId::Nav => nav_state(),
        SideViewId::Explorer => explorer_state(),
        SideViewId::Commit => commit_state(),
        SideViewId::AgentFiles => agent_files_state(),
    }
}

fn set_scroll(state: &mut Value, view: SideViewId, scroll: Value) {
    match view {
        SideViewId::Explorer => state["projects"][0]["scroll"] = scroll,
        _ => state["scroll"] = scroll,
    }
}

/// 四种视图只保存各自声明的持久字段，并按稳定身份与路径生成确定顺序。
#[test]
fn side_view_schemas_accept_and_canonicalize_persistent_fields() {
    let nav = create(SideViewId::Nav, nav_state()).unwrap();
    assert_eq!(
        nav.state()["projects"][0]["project_id"],
        Value::String(PROJECT_A.to_owned())
    );
    assert_eq!(nav.state()["scroll"], json!(12.5));

    let explorer = create(SideViewId::Explorer, explorer_state()).unwrap();
    assert_eq!(
        explorer.state()["projects"][0]["expanded"],
        json!(["docs", "src", "src/net"])
    );

    let commit = create(SideViewId::Commit, commit_state()).unwrap();
    assert_eq!(commit.state()["unversioned_collapsed"], json!(true));
    assert_eq!(commit.state()["show_ignored"], json!(true));

    let agent = create(SideViewId::AgentFiles, agent_files_state()).unwrap();
    assert_eq!(
        agent.state()["roots"][0]["agent_root_id"],
        Value::String(ROOT_A.to_owned())
    );
    assert_eq!(
        agent.state()["roots"][0]["directories"],
        json!(["skills", "skills/a"])
    );

    for (view, state) in [
        (SideViewId::Nav, nav),
        (SideViewId::Explorer, explorer),
        (SideViewId::Commit, commit),
        (SideViewId::AgentFiles, agent),
    ] {
        let canonical = state.to_canonical_json().unwrap();
        let restored = ViewState::from_canonical_json(&canonical, &key(view), &registry()).unwrap();
        assert_eq!(restored, state);
        assert_eq!(restored.to_canonical_json().unwrap(), canonical);
    }
}

/// 强类型运行态只经同一 schema 编解码，并拒绝跨视图恢复。
#[test]
fn typed_side_view_state_roundtrips_through_the_registered_schema() {
    for view in SideViewId::ALL {
        let persisted = create(view, persistent_state(view)).unwrap();
        let restored = SideViewState::from_view_state(view, persisted.clone()).unwrap();
        assert_eq!(restored.id(), view);
        assert_eq!(restored.to_view_state(&registry()).unwrap(), persisted);

        let wrong_view = match view {
            SideViewId::Nav => SideViewId::Explorer,
            _ => SideViewId::Nav,
        };
        assert!(SideViewState::from_view_state(wrong_view, persisted).is_err());

        let default = SideViewState::new(view).to_view_state(&registry()).unwrap();
        assert_eq!(
            default.scope(&key(view)).unwrap(),
            slterm_workspace::ViewStateScope::Application
        );
        let restored_default = SideViewState::from_view_state(view, default.clone()).unwrap();
        assert_eq!(
            restored_default.to_view_state(&registry()).unwrap(),
            default
        );
    }
}

/// 分区清理只删除目标身份；共享字段、其他分区与无关视图保持原值。
#[test]
fn side_view_partition_removal_is_precise_and_clears_deleted_current_row() {
    use std::str::FromStr;

    use slterm_settings::AgentRootId;
    use slterm_workspace::ProjectId;

    let project_a = ProjectId::from_str(PROJECT_A).unwrap();
    let project_b = ProjectId::from_str(PROJECT_B).unwrap();
    let root_a = AgentRootId::from_str(ROOT_A).unwrap();

    let mut nav = SideViewState::from_view_state(
        SideViewId::Nav,
        create(SideViewId::Nav, nav_state()).unwrap(),
    )
    .unwrap();
    assert!(nav.remove_project(project_a));
    let nav = nav.to_view_state(&registry()).unwrap();
    assert_eq!(nav.state()["projects"].as_array().unwrap().len(), 1);
    assert_eq!(nav.state()["projects"][0]["project_id"], json!(PROJECT_B));
    assert_eq!(nav.state()["unassigned_expanded"], json!(true));
    assert_eq!(nav.state()["scroll"], json!(12.5));

    let mut explorer_value = explorer_state();
    let mut project_b_state = explorer_value["projects"][0].clone();
    project_b_state["project_id"] = json!(PROJECT_B);
    project_b_state["current_row"] = Value::Null;
    project_b_state["scroll"] = json!(64.0);
    explorer_value["projects"]
        .as_array_mut()
        .unwrap()
        .push(project_b_state);
    let mut explorer = SideViewState::from_view_state(
        SideViewId::Explorer,
        create(SideViewId::Explorer, explorer_value).unwrap(),
    )
    .unwrap();
    assert!(explorer.remove_project(project_a));
    assert!(!explorer.remove_project(project_a));
    let explorer = explorer.to_view_state(&registry()).unwrap();
    assert_eq!(explorer.state()["projects"].as_array().unwrap().len(), 1);
    assert_eq!(
        explorer.state()["projects"][0]["project_id"],
        json!(PROJECT_B)
    );
    assert_eq!(explorer.state()["projects"][0]["scroll"], json!(64.0));

    let mut agent = SideViewState::from_view_state(
        SideViewId::AgentFiles,
        create(SideViewId::AgentFiles, agent_files_state()).unwrap(),
    )
    .unwrap();
    assert!(agent.remove_agent_root(root_a));
    let agent = agent.to_view_state(&registry()).unwrap();
    assert_eq!(agent.state()["roots"].as_array().unwrap().len(), 1);
    assert_eq!(agent.state()["roots"][0]["agent_root_id"], json!(ROOT_B));
    assert_eq!(agent.state()["current_row"], Value::Null);
    assert_eq!(agent.state()["scroll"], json!(36.0));

    let mut commit = SideViewState::from_view_state(
        SideViewId::Commit,
        create(SideViewId::Commit, commit_state()).unwrap(),
    )
    .unwrap();
    let before = commit.to_view_state(&registry()).unwrap();
    assert!(!commit.remove_project(project_b));
    assert!(!commit.remove_agent_root(root_a));
    assert_eq!(commit.to_view_state(&registry()).unwrap(), before);
}

/// fake position、其他视图的字段集合与瞬态字段均不得进入持久状态。
#[test]
fn side_view_schemas_reject_fake_cross_view_and_transient_states() {
    for view in SideViewId::ALL {
        assert_eq!(
            create(view, json!({"position": 7})),
            Err(ViewStateError::InvalidState),
            "{view:?} 不得再接受 fake position"
        );
        let mut transient = persistent_state(view);
        transient["search"] = json!("secret query");
        transient["multi_select"] = json!(["a"]);
        transient["focus"] = json!(true);
        assert_eq!(
            create(view, transient),
            Err(ViewStateError::InvalidState),
            "{view:?} 不得保存瞬态字段"
        );
    }

    assert_eq!(
        create(SideViewId::Nav, explorer_state()),
        Err(ViewStateError::InvalidState)
    );
    assert_eq!(
        create(SideViewId::Explorer, commit_state()),
        Err(ViewStateError::InvalidState)
    );
    assert_eq!(
        create(SideViewId::Commit, agent_files_state()),
        Err(ViewStateError::InvalidState)
    );
    assert_eq!(
        create(SideViewId::AgentFiles, nav_state()),
        Err(ViewStateError::InvalidState)
    );
}

/// 分区身份和路径集合不得因 map/set 去重而静默丢失记录。
#[test]
fn side_view_schemas_reject_duplicate_partitions_and_paths() {
    let mut duplicate_project = nav_state();
    duplicate_project["projects"][1] = duplicate_project["projects"][0].clone();
    assert_eq!(
        create(SideViewId::Nav, duplicate_project),
        Err(ViewStateError::InvalidState)
    );

    let mut duplicate_explorer_project = explorer_state();
    duplicate_explorer_project["projects"] = json!([
        duplicate_explorer_project["projects"][0].clone(),
        duplicate_explorer_project["projects"][0].clone()
    ]);
    assert_eq!(
        create(SideViewId::Explorer, duplicate_explorer_project),
        Err(ViewStateError::InvalidState)
    );

    let mut duplicate_path = explorer_state();
    duplicate_path["projects"][0]["expanded"] = json!(["src", "src"]);
    assert_eq!(
        create(SideViewId::Explorer, duplicate_path),
        Err(ViewStateError::InvalidState)
    );

    let mut duplicate_root = agent_files_state();
    duplicate_root["roots"][1] = duplicate_root["roots"][0].clone();
    assert_eq!(
        create(SideViewId::AgentFiles, duplicate_root),
        Err(ViewStateError::InvalidState)
    );

    let mut duplicate_directory = agent_files_state();
    duplicate_directory["roots"][0]["directories"] = json!(["skills", "skills"]);
    assert_eq!(
        create(SideViewId::AgentFiles, duplicate_directory),
        Err(ViewStateError::InvalidState)
    );

    assert_eq!(
        restore_raw(
            SideViewId::Commit,
            r#"{"changes_collapsed":false,"changes_collapsed":true,"unversioned_collapsed":false,"ignored_collapsed":false,"show_ignored":false,"scroll":0.0}"#,
        ),
        Err(ViewStateError::InvalidState)
    );
    assert_eq!(
        restore_raw(
            SideViewId::Nav,
            &format!(
                r#"{{"projects":[{{"project_id":"{PROJECT_A}","expanded":false,"expanded":true,"history_expanded":false}}],"unassigned_expanded":false,"scroll":0.0}}"#
            ),
        ),
        Err(ViewStateError::InvalidState)
    );
    assert_eq!(
        restore_raw(
            SideViewId::AgentFiles,
            &format!(
                r#"{{"roots":[{{"agent_root_id":"{ROOT_A}","expanded":true,"directories":[],"directories":[]}}],"current_row":null,"scroll":0.0}}"#
            ),
        ),
        Err(ViewStateError::InvalidState)
    );
}

/// 分区只接受规范 UUIDv7，路径只接受非空、正斜杠、无跳转的根相对路径。
#[test]
fn side_view_schemas_validate_stable_ids_and_relative_paths() {
    for invalid in [
        "not-a-uuid",
        "550e8400-e29b-41d4-a716-446655440000",
        &PROJECT_A.to_uppercase(),
    ] {
        let mut state = nav_state();
        state["projects"][0]["project_id"] = json!(invalid);
        assert_eq!(
            create(SideViewId::Nav, state),
            Err(ViewStateError::InvalidState),
            "{invalid}"
        );

        let mut explorer = explorer_state();
        explorer["projects"][0]["project_id"] = json!(invalid);
        assert_eq!(
            create(SideViewId::Explorer, explorer),
            Err(ViewStateError::InvalidState),
            "{invalid}"
        );

        let mut agent_root = agent_files_state();
        agent_root["roots"][0]["agent_root_id"] = json!(invalid);
        assert_eq!(
            create(SideViewId::AgentFiles, agent_root),
            Err(ViewStateError::InvalidState),
            "{invalid}"
        );

        let mut agent_current = agent_files_state();
        agent_current["current_row"]["agent_root_id"] = json!(invalid);
        assert_eq!(
            create(SideViewId::AgentFiles, agent_current),
            Err(ViewStateError::InvalidState),
            "{invalid}"
        );
    }

    for invalid in [
        "",
        "/src",
        "src/",
        "src//net",
        "src\\net",
        "C:/src",
        ".",
        "..",
        "src/../net",
        "src/./net",
        "bad\0path",
    ] {
        let mut state = explorer_state();
        state["projects"][0]["current_row"] = json!(invalid);
        assert_eq!(
            create(SideViewId::Explorer, state),
            Err(ViewStateError::InvalidState),
            "{invalid:?}"
        );

        let mut expanded = explorer_state();
        expanded["projects"][0]["expanded"] = json!([invalid]);
        assert_eq!(
            create(SideViewId::Explorer, expanded),
            Err(ViewStateError::InvalidState),
            "{invalid:?}"
        );

        let mut directories = agent_files_state();
        directories["roots"][0]["directories"] = json!([invalid]);
        assert_eq!(
            create(SideViewId::AgentFiles, directories),
            Err(ViewStateError::InvalidState),
            "{invalid:?}"
        );

        let mut current = agent_files_state();
        current["current_row"]["path"] = json!(invalid);
        assert_eq!(
            create(SideViewId::AgentFiles, current),
            Err(ViewStateError::InvalidState),
            "{invalid:?}"
        );
    }
}

/// 滚动必须是非负有限逻辑像素；Agent 当前行必须归属已声明根分区。
#[test]
fn side_view_schemas_validate_scroll_and_agent_current_row() {
    for view in SideViewId::ALL {
        for invalid in [json!(-1.0), json!("12"), Value::Null] {
            let mut state = persistent_state(view);
            set_scroll(&mut state, view, invalid);
            assert_eq!(
                create(view, state),
                Err(ViewStateError::InvalidState),
                "{view:?}"
            );
        }
    }
    assert_eq!(
        restore_raw(
            SideViewId::Commit,
            r#"{"changes_collapsed":false,"unversioned_collapsed":false,"ignored_collapsed":false,"show_ignored":false,"scroll":1e999}"#,
        ),
        Err(ViewStateError::InvalidState)
    );

    let mut root_row = agent_files_state();
    root_row["current_row"] = json!({"agent_root_id": ROOT_A, "path": null});
    assert!(create(SideViewId::AgentFiles, root_row).is_ok());

    let mut missing_current = agent_files_state();
    missing_current["current_row"] = Value::Null;
    assert!(create(SideViewId::AgentFiles, missing_current).is_ok());

    let mut unknown_root = agent_files_state();
    unknown_root["current_row"] = json!({
        "agent_root_id": "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2e",
        "path": null
    });
    assert_eq!(
        create(SideViewId::AgentFiles, unknown_root),
        Err(ViewStateError::InvalidState)
    );
}

/// 负零滚动统一规范为正零，规范字节可稳定往返。
#[test]
fn side_view_schemas_canonicalize_negative_zero_scroll() {
    for view in SideViewId::ALL {
        let mut state = persistent_state(view);
        set_scroll(&mut state, view, json!(-0.0));
        let state = create(view, state).unwrap();
        let canonical = state.to_canonical_json().unwrap();
        assert!(!canonical.contains("-0.0"), "{view:?}: {canonical}");
        let restored = ViewState::from_canonical_json(&canonical, &key(view), &registry()).unwrap();
        assert_eq!(restored, state);
        assert_eq!(restored.to_canonical_json().unwrap(), canonical);
    }
}

/// 缺字段、null 与额外字段都属于结构非法，不做字段级默认补齐。
#[test]
fn side_view_schemas_require_exact_field_sets() {
    for state in [
        json!({}),
        Value::Null,
        json!({"projects": null, "unassigned_expanded": false, "scroll": 0.0}),
        json!({"projects": [], "unassigned_expanded": false}),
    ] {
        assert_eq!(
            create(SideViewId::Nav, state),
            Err(ViewStateError::InvalidState)
        );
    }

    let mut missing_current = explorer_state();
    missing_current["projects"][0]
        .as_object_mut()
        .unwrap()
        .remove("current_row");
    assert_eq!(
        create(SideViewId::Explorer, missing_current),
        Err(ViewStateError::InvalidState)
    );

    let mut missing_agent_path = agent_files_state();
    missing_agent_path["current_row"]
        .as_object_mut()
        .unwrap()
        .remove("path");
    assert_eq!(
        create(SideViewId::AgentFiles, missing_agent_path),
        Err(ViewStateError::InvalidState)
    );

    let mut extra_nested = explorer_state();
    extra_nested["projects"][0]["menu"] = json!(true);
    assert_eq!(
        create(SideViewId::Explorer, extra_nested),
        Err(ViewStateError::InvalidState)
    );

    let mut extra_agent_row = agent_files_state();
    extra_agent_row["current_row"]["error"] = json!("transient");
    assert_eq!(
        create(SideViewId::AgentFiles, extra_agent_row),
        Err(ViewStateError::InvalidState)
    );

    let mut extra = commit_state();
    extra["position"] = json!(7);
    assert_eq!(
        create(SideViewId::Commit, extra),
        Err(ViewStateError::InvalidState)
    );
}
