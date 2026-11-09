//! ProjectSnapshot 的 Document 元数据编解码与项目关闭清理。

use std::collections::{BTreeMap, BTreeSet};
use std::str::FromStr;

use serde::Deserialize;
use slterm_document::DocumentId;
use slterm_path::{CanonicalPath, FileIdentity, ResolvedPath};
use slterm_workspace::{ProjectId, ViewKey};

use super::{ProjectStoreError, load_project_snapshot_from};

pub(super) struct EncodedDocument {
    pub(super) id: String,
    pub(super) location_json: String,
    pub(super) identity_json: Option<String>,
}

impl EncodedDocument {
    pub(super) fn new(id: DocumentId, location: &ResolvedPath) -> Result<Self, ProjectStoreError> {
        let path = location
            .canonical_path()
            .as_path()
            .to_str()
            .ok_or(ProjectStoreError::CorruptRecord)?;
        let location_json = serde_json::to_string(&BTreeMap::from([
            ("path", serde_json::Value::String(path.to_owned())),
            ("version", serde_json::Value::from(1)),
        ]))
        .map_err(|_| ProjectStoreError::CorruptRecord)?;
        let identity_json = location
            .file_identity()
            .map(|identity| {
                serde_json::to_string(&BTreeMap::from([
                    ("file", serde_json::Value::String(identity.file.to_string())),
                    ("version", serde_json::Value::from(1)),
                    ("volume", serde_json::Value::from(identity.volume)),
                ]))
                .map_err(|_| ProjectStoreError::CorruptRecord)
            })
            .transpose()?;
        Ok(Self {
            id: id.to_string(),
            location_json,
            identity_json,
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DocumentLocationDto {
    version: u64,
    path: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DocumentIdentityDto {
    version: u64,
    volume: u64,
    file: String,
}

pub(super) fn load_document_records(
    connection: &rusqlite::Connection,
) -> Result<Vec<(DocumentId, ResolvedPath)>, ProjectStoreError> {
    let mut statement = connection.prepare(
        "SELECT document_id, location_json, file_identity_json FROM documents ORDER BY document_id",
    )?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    rows.into_iter()
        .map(|(id, location, identity)| {
            let id = DocumentId::from_str(&id).map_err(|_| ProjectStoreError::CorruptRecord)?;
            let location: DocumentLocationDto =
                serde_json::from_str(&location).map_err(|_| ProjectStoreError::CorruptRecord)?;
            if location.version != 1 {
                return Err(ProjectStoreError::CorruptRecord);
            }
            let path =
                CanonicalPath::new(location.path).map_err(|_| ProjectStoreError::CorruptRecord)?;
            let identity = identity
                .map(|text| {
                    let identity: DocumentIdentityDto = serde_json::from_str(&text)
                        .map_err(|_| ProjectStoreError::CorruptRecord)?;
                    if identity.version != 1 {
                        return Err(ProjectStoreError::CorruptRecord);
                    }
                    let file = identity
                        .file
                        .parse::<u128>()
                        .map_err(|_| ProjectStoreError::CorruptRecord)?;
                    if identity.file != file.to_string() {
                        return Err(ProjectStoreError::CorruptRecord);
                    }
                    Ok(FileIdentity {
                        volume: identity.volume,
                        file,
                    })
                })
                .transpose()?;
            Ok((id, ResolvedPath::new(path, identity)))
        })
        .collect()
}

/// 项目删除前精确移除其独占 Document；共享记录由其他项目的布局引用保留。
pub(in crate::persistence) fn delete_project_documents_in_transaction(
    connection: &rusqlite::Connection,
    project_id: ProjectId,
) -> Result<(), ProjectStoreError> {
    let snapshot = load_project_snapshot_from(connection)?;
    let mut target_documents = BTreeSet::new();
    let mut retained_documents = BTreeSet::new();
    for project in snapshot.registry().projects() {
        let destination = if project.id() == project_id {
            &mut target_documents
        } else {
            &mut retained_documents
        };
        destination.extend(project.layout().view_keys().into_iter().filter_map(
            |view| match view {
                ViewKey::Editor { document_id, .. }
                | ViewKey::Markdown { document_id, .. }
                | ViewKey::Diff { document_id, .. } => Some(document_id),
                _ => None,
            },
        ));
    }
    for document_id in target_documents.difference(&retained_documents) {
        let deleted = connection.execute(
            "DELETE FROM documents WHERE document_id = ?1",
            [document_id.to_string()],
        )?;
        if deleted != 1 {
            return Err(ProjectStoreError::CorruptRecord);
        }
    }
    Ok(())
}

#[cfg(test)]
mod document_tests {
    use rusqlite::params;
    use slterm_path::AppPaths;
    use slterm_workspace::{PanelGroupId, ProjectRegistry, ProjectSnapshot};
    use tempfile::TempDir;

    use super::*;
    use crate::persistence::workspace::WorkspaceDatabase;

    const PROJECT_A: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2a";
    const PROJECT_B: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b";
    const GROUP_A: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0a2a";
    const GROUP_B: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0a2b";
    const DOCUMENT_A: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0f2a";

    fn fixture(temp: &TempDir) -> AppPaths {
        AppPaths::new(temp.path().join("config"), temp.path().join("local")).unwrap()
    }

    fn empty_projects() -> ProjectSnapshot {
        let mut registry = ProjectRegistry::new();
        for (project, root) in [(PROJECT_A, "D:/work/a"), (PROJECT_B, "D:/work/b")] {
            registry
                .add_project(
                    ProjectId::from_str(project).unwrap(),
                    CanonicalPath::new(root).unwrap(),
                )
                .unwrap();
        }
        ProjectSnapshot::restore(registry, Vec::new()).unwrap()
    }

    #[test]
    fn project_snapshot_store_round_trips_shared_document_metadata() {
        let temp = tempfile::tempdir().unwrap();
        let paths = fixture(&temp);
        let mut store = WorkspaceDatabase::open(&paths).unwrap();
        let document_id = DocumentId::from_str(DOCUMENT_A).unwrap();
        let location = ResolvedPath::new(
            CanonicalPath::new("D:/work/shared.txt").unwrap(),
            Some(FileIdentity {
                volume: 7,
                file: u128::MAX,
            }),
        );
        let (with_editor, _) = empty_projects()
            .with_opened_document(
                ProjectId::from_str(PROJECT_A).unwrap(),
                PanelGroupId::from_str(GROUP_A).unwrap(),
                document_id,
                location.clone(),
            )
            .unwrap();
        let (shared, _) = with_editor
            .with_opened_document(
                ProjectId::from_str(PROJECT_B).unwrap(),
                PanelGroupId::from_str(GROUP_B).unwrap(),
                document_id,
                location,
            )
            .unwrap();

        store.save_project_snapshot(&shared).unwrap();
        drop(store);

        let mut reopened = WorkspaceDatabase::open(&paths).unwrap();
        assert_eq!(reopened.load_project_snapshot().unwrap(), shared);
        assert_eq!(
            reopened
                .load_project_snapshot()
                .unwrap()
                .documents()
                .count(),
            1
        );
    }

    #[test]
    fn project_snapshot_store_document_write_failure_rolls_back_layout() {
        let temp = tempfile::tempdir().unwrap();
        let mut store = WorkspaceDatabase::open(&fixture(&temp)).unwrap();
        let original = empty_projects();
        store.save_project_snapshot(&original).unwrap();
        let (opened, _) = original
            .with_opened_document(
                ProjectId::from_str(PROJECT_B).unwrap(),
                PanelGroupId::from_str(GROUP_B).unwrap(),
                DocumentId::from_str(DOCUMENT_A).unwrap(),
                ResolvedPath::new(CanonicalPath::new("D:/work/shared.txt").unwrap(), None),
            )
            .unwrap();
        store
            .database
            .connection
            .execute_batch(
                "CREATE TRIGGER reject_document_write BEFORE INSERT ON documents \
                 BEGIN SELECT RAISE(ABORT, 'injected document write failure'); END;",
            )
            .unwrap();

        assert!(matches!(
            store.save_project_snapshot(&opened),
            Err(ProjectStoreError::Database(_))
        ));
        assert_eq!(store.load_project_snapshot().unwrap(), original);
    }

    #[test]
    fn project_snapshot_store_rejects_corrupt_document_dtos() {
        let temp = tempfile::tempdir().unwrap();
        let mut store = WorkspaceDatabase::open(&fixture(&temp)).unwrap();
        let valid_location = r#"{"path":"D:/work/shared.txt","version":1}"#;
        let cases = [
            ("{", None),
            (r#"{"path":"D:/work/shared.txt","version":2}"#, None),
            (
                r#"{"path":"D:/work/shared.txt","version":1,"extra":true}"#,
                None,
            ),
            (r#"{"path":"relative.txt","version":1}"#, None),
            (valid_location, Some("{")),
            (
                valid_location,
                Some(r#"{"file":"1","version":2,"volume":7}"#),
            ),
            (
                valid_location,
                Some(r#"{"file":"1","version":1,"volume":7,"extra":true}"#),
            ),
            (
                valid_location,
                Some(r#"{"file":"01","version":1,"volume":7}"#),
            ),
        ];
        for (location, identity) in cases {
            store
                .database
                .connection
                .execute("DELETE FROM documents", [])
                .unwrap();
            store
                .database
                .connection
                .execute(
                    "INSERT INTO documents (document_id, location_json, file_identity_json) \
                     VALUES (?1, ?2, ?3)",
                    params![DOCUMENT_A, location, identity],
                )
                .unwrap();

            assert!(matches!(
                store.load_project_snapshot(),
                Err(ProjectStoreError::CorruptRecord)
            ));
        }
    }

    /// 非 Unicode Document 路径不得经替换字符进入持久库。
    #[test]
    fn project_snapshot_store_rejects_non_unicode_document_path_without_writing() {
        use std::ffi::OsString;
        use std::os::windows::ffi::OsStringExt;
        use std::path::PathBuf;

        let temp = tempfile::tempdir().unwrap();
        let mut store = WorkspaceDatabase::open(&fixture(&temp)).unwrap();
        let mut path = OsString::from("D:/");
        path.push(OsString::from_wide(&[0xd800]));
        let location = ResolvedPath::new(CanonicalPath::new(PathBuf::from(path)).unwrap(), None);
        let (opened, _) = empty_projects()
            .with_opened_document(
                ProjectId::from_str(PROJECT_B).unwrap(),
                PanelGroupId::from_str(GROUP_B).unwrap(),
                DocumentId::from_str(DOCUMENT_A).unwrap(),
                location,
            )
            .unwrap();

        assert!(matches!(
            store.save_project_snapshot(&opened),
            Err(ProjectStoreError::CorruptRecord)
        ));
        assert_eq!(
            store.load_project_snapshot().unwrap(),
            ProjectSnapshot::default()
        );
    }
}
