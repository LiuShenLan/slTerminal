//! 工作区备份类别：项目、布局、终端元数据、Document 位置与侧栏／几何。
//!
//! 只消费已登记规范 DTO（布局与侧栏 canonical JSON、ViewState 持久 JSON、
//! 几何 DTO），不复制 SQLite 物理文件；恢复在维护期经同一快照写通道落库。

use serde::{Deserialize, Serialize};
use slterm_agents::AgentSession;
use slterm_document::DocumentId;
use slterm_path::{CanonicalPath, ResolvedPath};
use slterm_sidebar::SidebarState;
use slterm_workspace::{
    Project, ProjectId, ProjectLayout, ProjectRegistry, ProjectRootStatus, ProjectSnapshot,
    RunIntent, TerminalId, TerminalMetadata, TerminalRecord, ViewKey, ViewState, ViewStateRegistry,
};

/// 工作区条目在归档内的固定名。
pub(crate) const WORKSPACE_ENTRY_NAME: &str = "workspace.json";
/// 工作区投影 DTO 版本；超前或落后一律拒绝。
pub(crate) const WORKSPACE_DTO_VERSION: u32 = 1;

/// 可移植工作区投影；物理库、WAL、日志与凭据永不进入。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WorkspaceBackup {
    pub version: u32,
    /// 来源机器标识；目标机不直接应用该机几何。
    pub source_machine: String,
    pub active_project: Option<String>,
    pub projects: Vec<WorkspaceProjectBackup>,
    pub terminals: Vec<WorkspaceTerminalBackup>,
    pub documents: Vec<WorkspaceDocumentBackup>,
    pub view_states: Vec<WorkspaceViewStateBackup>,
    pub sidebar: Option<String>,
    pub window_geometry: Option<WorkspaceGeometryBackup>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WorkspaceProjectBackup {
    pub id: String,
    pub name: String,
    pub root: String,
    /// `ProjectLayout::to_canonical_json` 原样文本。
    pub layout: String,
    pub next_terminal_number: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WorkspaceTerminalBackup {
    pub id: String,
    pub project_id: String,
    pub number: u64,
    pub cwd: String,
    pub shell: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<AgentSession>,
    /// `launch` 或 `exited`；恢复后仍按可见性懒启动。
    pub run_intent: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WorkspaceDocumentBackup {
    pub id: String,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WorkspaceViewStateBackup {
    /// 完整 ViewKey 规范 JSON。
    pub view_key: String,
    /// `persistence::view_state` 存储的规范状态 JSON。
    pub state: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WorkspaceGeometryBackup {
    pub machine_key: String,
    pub geometry: crate::windowing::geometry::WindowGeometry,
}

/// 运行态收集输入：项目快照与其它已登记 DTO 的唯一来源。
pub(crate) struct WorkspaceCapture<'a> {
    pub snapshot: &'a ProjectSnapshot,
    pub view_states: Vec<WorkspaceViewStateBackup>,
    pub sidebar: Option<String>,
    pub geometry: Option<WorkspaceGeometryBackup>,
    pub source_machine: &'a str,
}

fn unicode_path(path: &CanonicalPath) -> Result<String, String> {
    path.as_path()
        .to_str()
        .map(str::to_owned)
        .ok_or_else(|| "工作区路径无法无损编码".to_owned())
}

/// 从运行态事实生成逻辑投影；编码失败保留领域错误，不产生半份归档。
pub(crate) fn capture_workspace(input: WorkspaceCapture<'_>) -> Result<WorkspaceBackup, String> {
    let snapshot = input.snapshot;
    let mut projects = Vec::new();
    for project in snapshot.registry().projects() {
        projects.push(WorkspaceProjectBackup {
            id: project.id().to_string(),
            name: project.display_name().to_owned(),
            root: unicode_path(project.root())?,
            layout: project
                .layout()
                .to_canonical_json()
                .map_err(|error| format!("布局编码失败: {error}"))?,
            next_terminal_number: project.next_terminal_number(),
        });
    }
    let terminals = snapshot
        .terminals()
        .map(|record| {
            Ok(WorkspaceTerminalBackup {
                id: record.id().to_string(),
                project_id: record.project_id().to_string(),
                number: record.number(),
                cwd: unicode_path(record.cwd())?,
                shell: unicode_path(record.shell())?,
                custom_title: record.custom_title().map(str::to_owned),
                agent: record.agent().cloned(),
                run_intent: match record.run_intent() {
                    RunIntent::Launch => "launch".to_owned(),
                    RunIntent::Exited => "exited".to_owned(),
                },
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let documents = snapshot
        .documents()
        .map(|(id, location)| {
            Ok(WorkspaceDocumentBackup {
                id: id.to_string(),
                path: unicode_path(location.canonical_path())?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(WorkspaceBackup {
        version: WORKSPACE_DTO_VERSION,
        source_machine: input.source_machine.to_owned(),
        active_project: snapshot.registry().active().map(|id| id.to_string()),
        projects,
        terminals,
        documents,
        view_states: input.view_states,
        sidebar: input.sidebar,
        window_geometry: input.geometry,
    })
}

/// 解析并全量校验工作区投影；版本、身份与跨引用任一不成立即整单拒绝。
pub(crate) fn decode_workspace(bytes: &[u8]) -> Result<WorkspaceBackup, String> {
    let payload: WorkspaceBackup =
        serde_json::from_slice(bytes).map_err(|error| format!("工作区投影结构非法: {error}"))?;
    if payload.version != WORKSPACE_DTO_VERSION {
        return Err("工作区投影版本不受支持".to_owned());
    }
    rebuild_snapshot(&payload)?;
    Ok(payload)
}

fn rebuild_snapshot(payload: &WorkspaceBackup) -> Result<ProjectSnapshot, String> {
    let mut projects = Vec::with_capacity(payload.projects.len());
    for backup in &payload.projects {
        let id = backup
            .id
            .parse::<ProjectId>()
            .map_err(|_| "工作区项目身份非法".to_owned())?;
        let root = CanonicalPath::new(&backup.root).map_err(|_| "项目根路径非法".to_owned())?;
        let layout = ProjectLayout::from_canonical_json(&backup.layout)
            .map_err(|error| format!("布局解码失败: {error}"))?;
        projects.push(
            Project::restore(
                id,
                root,
                backup.name.clone(),
                layout,
                ProjectRootStatus::Ready,
                backup.next_terminal_number,
            )
            .map_err(|error| format!("项目记录非法: {error:?}"))?,
        );
    }
    let active = payload
        .active_project
        .as_deref()
        .map(|id| id.parse::<ProjectId>())
        .transpose()
        .map_err(|_| "活跃项目身份非法".to_owned())?;
    let registry = ProjectRegistry::restore(projects, active)
        .map_err(|error| format!("项目集合非法: {error:?}"))?;
    let mut terminals = Vec::with_capacity(payload.terminals.len());
    for backup in &payload.terminals {
        let id = backup
            .id
            .parse::<TerminalId>()
            .map_err(|_| "终端身份非法".to_owned())?;
        let project_id = backup
            .project_id
            .parse::<ProjectId>()
            .map_err(|_| "终端项目归属非法".to_owned())?;
        let cwd = CanonicalPath::new(&backup.cwd).map_err(|_| "终端 cwd 非法".to_owned())?;
        let shell = CanonicalPath::new(&backup.shell).map_err(|_| "终端 Shell 非法".to_owned())?;
        let run_intent = match backup.run_intent.as_str() {
            "launch" => RunIntent::Launch,
            "exited" => RunIntent::Exited,
            _ => return Err("终端运行意图非法".to_owned()),
        };
        terminals.push(
            TerminalRecord::new(
                id,
                project_id,
                backup.number,
                cwd,
                shell,
                TerminalMetadata::new(backup.custom_title.clone(), backup.agent.clone()),
                run_intent,
            )
            .map_err(|error| format!("终端记录非法: {error}"))?,
        );
    }
    let documents = payload
        .documents
        .iter()
        .map(|backup| {
            let id = backup
                .id
                .parse::<DocumentId>()
                .map_err(|_| "Document 身份非法".to_owned())?;
            let path =
                CanonicalPath::new(&backup.path).map_err(|_| "Document 位置非法".to_owned())?;
            Ok::<_, String>((id, ResolvedPath::new(path, None)))
        })
        .collect::<Result<Vec<_>, String>>()?;
    ProjectSnapshot::restore_with_documents(registry, terminals, documents)
        .map_err(|error| format!("工作区快照非法: {error:?}"))
}

/// 恢复工作区：项目与终端写唯一快照通道，侧栏写规范 JSON，
/// 几何只在本机匹配时应用；ViewState 经注册表逐条复验后写库。
pub(crate) fn restore_workspace(
    store: &mut crate::persistence::workspace::WorkspaceDatabase,
    payload: &WorkspaceBackup,
    local_machine: &str,
    registry: &ViewStateRegistry,
) -> Result<(), String> {
    let snapshot = rebuild_snapshot(payload)?;
    let sidebar = payload
        .sidebar
        .as_deref()
        .map(SidebarState::from_canonical_json)
        .transpose()
        .map_err(|error| format!("侧栏状态非法: {error}"))?;
    // 先全量复验，任何一项不成立都不得产生部分写入。
    let mut view_states = Vec::with_capacity(payload.view_states.len());
    for backup in &payload.view_states {
        let key = ViewKey::from_canonical_json(&backup.view_key)
            .map_err(|error| format!("ViewKey 非法: {error}"))?;
        let state = ViewState::from_canonical_json(&backup.state, &key, registry)
            .map_err(|error| format!("ViewState 非法: {error}"))?;
        view_states.push((key, state));
    }
    store
        .save_project_snapshot(&snapshot)
        .map_err(|error| format!("保存项目快照失败: {error}"))?;
    if let Some(sidebar) = sidebar {
        store
            .save_sidebar_state(&sidebar)
            .map_err(|error| format!("保存侧栏状态失败: {error}"))?;
    }
    for (key, state) in view_states {
        store
            .save_view_state(&key, &state)
            .map_err(|error| format!("保存 ViewState 失败: {error}"))?;
    }
    if let Some(geometry) = &payload.window_geometry
        && geometry.machine_key == local_machine
    {
        store
            .save_window_geometry(local_machine, &geometry.geometry)
            .map_err(|error| format!("保存窗口几何失败: {error}"))?;
    }
    Ok(())
}

#[cfg(test)]
mod workspace_backup_tests {
    use std::str::FromStr as _;

    use slterm_path::AppPaths;
    use slterm_workspace::{PanelGroup, PanelGroupId};

    use super::*;
    use crate::persistence::workspace::WorkspaceDatabase;

    const PROJECT: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2a";
    const GROUP: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0a2a";
    const TERMINAL: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0b2a";
    const DOCUMENT: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0c2a";

    fn snapshot() -> ProjectSnapshot {
        let project_id = ProjectId::from_str(PROJECT).unwrap();
        let terminal_id = TerminalId::from_str(TERMINAL).unwrap();
        let document_id = DocumentId::from_str(DOCUMENT).unwrap();
        let terminal_view = ViewKey::Terminal {
            project_id,
            terminal_id,
        };
        let editor_view = ViewKey::Editor {
            project_id,
            document_id,
        };
        let layout = ProjectLayout::new(
            project_id,
            PanelGroupId::from_str(GROUP).unwrap(),
            PanelGroup::new(vec![terminal_view.clone(), editor_view], terminal_view).unwrap(),
        )
        .unwrap();
        let project = Project::restore(
            project_id,
            CanonicalPath::new("D:/work/a").unwrap(),
            "a".to_owned(),
            layout,
            ProjectRootStatus::Ready,
            4,
        )
        .unwrap();
        let registry = ProjectRegistry::restore(vec![project], Some(project_id)).unwrap();
        let record = TerminalRecord::new(
            terminal_id,
            project_id,
            0,
            CanonicalPath::new("D:/work/a").unwrap(),
            CanonicalPath::new("C:/shell/pwsh.exe").unwrap(),
            TerminalMetadata::default(),
            RunIntent::Launch,
        )
        .unwrap();
        ProjectSnapshot::restore_with_documents(
            registry,
            vec![record],
            vec![(
                document_id,
                ResolvedPath::new(CanonicalPath::new("D:/work/a/README.md").unwrap(), None),
            )],
        )
        .unwrap()
    }

    fn geometry() -> WorkspaceGeometryBackup {
        WorkspaceGeometryBackup {
            machine_key: "machine-a".to_owned(),
            geometry: crate::windowing::geometry::WindowGeometry {
                x: 10.0,
                y: 20.0,
                width: 1000.0,
                height: 700.0,
                maximized: false,
                display: None,
            },
        }
    }

    fn captured() -> WorkspaceBackup {
        capture_workspace(WorkspaceCapture {
            snapshot: &snapshot(),
            view_states: Vec::new(),
            sidebar: Some(SidebarState::default().to_canonical_json().unwrap()),
            geometry: Some(geometry()),
            source_machine: "machine-a",
        })
        .unwrap()
    }

    /// 投影只含规范逻辑对象，不含物理库或正文。
    #[test]
    fn projection_uses_canonical_dtos_without_physical_files() {
        let payload = captured();
        assert_eq!(payload.projects.len(), 1);
        assert_eq!(payload.terminals.len(), 1);
        assert_eq!(payload.documents.len(), 1);
        assert_eq!(payload.projects[0].next_terminal_number, 4);
        let layout = ProjectLayout::from_canonical_json(&payload.projects[0].layout).unwrap();
        assert_eq!(layout.view_keys().len(), 2);
        let json = serde_json::to_string(&payload).unwrap();
        assert!(!json.contains("workspace.db"));
        assert!(!json.contains("-wal"));
        assert!(json.contains("README.md"));
    }

    /// 恢复重建同一快照，侧栏与同机几何写库；跨机几何不应用。
    #[test]
    fn restore_rebuilds_snapshot_and_applies_geometry_only_same_machine() {
        let payload = captured();
        let temp = tempfile::tempdir().unwrap();
        let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
        let mut store = WorkspaceDatabase::open(&paths).unwrap();
        let registry = ViewStateRegistry::new(&[]).unwrap();
        restore_workspace(&mut store, &payload, "machine-a", &registry).unwrap();
        assert_eq!(store.load_project_snapshot().unwrap(), snapshot());
        assert!(store.window_geometry("machine-a").unwrap().is_some());
        assert!(store.sidebar_state().unwrap().is_some());

        let temp_b = tempfile::tempdir().unwrap();
        let paths_b =
            AppPaths::new(temp_b.path().join("config"), temp_b.path().join("local")).unwrap();
        let mut store_b = WorkspaceDatabase::open(&paths_b).unwrap();
        restore_workspace(&mut store_b, &payload, "machine-b", &registry).unwrap();
        assert_eq!(store_b.load_project_snapshot().unwrap(), snapshot());
        assert!(
            store_b.window_geometry("machine-b").unwrap().is_none(),
            "跨机几何不得直接应用"
        );
    }

    /// 版本超前、未知字段与损坏运行意图整单拒绝。
    #[test]
    fn decode_rejects_version_unknown_fields_and_bad_run_intent() {
        let mut payload = captured();
        payload.version = 2;
        let bytes = serde_json::to_vec(&payload).unwrap();
        assert!(decode_workspace(&bytes).is_err());

        let mut payload = captured();
        payload.version = WORKSPACE_DTO_VERSION;
        let mut value: serde_json::Value = serde_json::to_value(&payload).unwrap();
        value["unexpected"] = serde_json::Value::Bool(true);
        assert!(decode_workspace(&serde_json::to_vec(&value).unwrap()).is_err());

        payload.terminals[0].run_intent = "never".to_owned();
        assert!(decode_workspace(&serde_json::to_vec(&payload).unwrap()).is_err());
    }

    /// 仅选工作区类别：设置、凭据与物理库材料不得进入归档。
    #[test]
    fn workspace_only_selection_excludes_settings_credentials_and_physical_files() {
        let temp = tempfile::tempdir().unwrap();
        let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
        std::fs::create_dir_all(paths.config_root()).unwrap();
        std::fs::create_dir_all(paths.local_root()).unwrap();
        std::fs::write(
            paths.config_root().join("settings.json"),
            br#"{"notify":{"credential":"sk-test-value"}}"#,
        )
        .unwrap();
        std::fs::write(paths.local_root().join("workspace.db"), b"sqlite-bytes").unwrap();

        let payload = captured();
        let archive = crate::encrypted_backup::collect_from(
            &paths,
            crate::encrypted_backup::BackupSelection::from_categories([
                crate::encrypted_backup::BackupCategory::Workspace,
            ]),
            None,
            Some(&payload),
        )
        .unwrap();
        assert_eq!(
            archive.manifest.categories,
            vec![crate::encrypted_backup::BackupCategory::Workspace]
        );
        assert_eq!(archive.entries.len(), 1);
        assert_eq!(archive.entries[0].name, WORKSPACE_ENTRY_NAME);
        let text = std::str::from_utf8(&archive.entries[0].bytes).unwrap();
        assert!(!text.contains("sk-test"));
        assert!(!text.contains("settings.json"));
        assert!(!text.contains("workspace.db"));
        assert!(!text.contains("sqlite-bytes"));
        assert!(text.contains("README.md"), "逻辑对象必须导出");

        let packet = crate::encrypted_backup::seal(&archive, "correct horse").unwrap();
        let opened = crate::encrypted_backup::open(&packet, "correct horse").unwrap();
        assert_eq!(opened, archive);
    }

    /// 未注册 ViewState 在恢复前拒绝，且不产生部分写入。
    #[test]
    fn restore_rejects_unregistered_view_state_before_writing() {
        let mut payload = captured();
        let project_id = ProjectId::from_str(PROJECT).unwrap();
        let terminal_id = TerminalId::from_str(TERMINAL).unwrap();
        let key = ViewKey::Terminal {
            project_id,
            terminal_id,
        };
        payload.view_states.push(WorkspaceViewStateBackup {
            view_key: key.to_canonical_json().unwrap(),
            state: r#"{"version":1}"#.to_owned(),
        });
        let temp = tempfile::tempdir().unwrap();
        let paths = AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap();
        let mut store = WorkspaceDatabase::open(&paths).unwrap();
        let registry = ViewStateRegistry::new(&[]).unwrap();
        assert!(restore_workspace(&mut store, &payload, "machine-a", &registry).is_err());
        assert!(
            store
                .load_project_snapshot()
                .unwrap()
                .registry()
                .projects()
                .is_empty(),
            "预检失败不得留下部分写入"
        );
    }
}
