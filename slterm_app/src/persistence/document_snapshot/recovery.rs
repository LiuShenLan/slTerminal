//! 启动快照对账与显式重授权。

use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::PathBuf;
use std::sync::Arc;

use ropey::Rope;
use sha2::{Digest as _, Sha256};
use slterm_document::{
    DocumentConflict, DocumentContent, DocumentEncoding, DocumentId, DocumentMetadata,
    DocumentRecovery, DocumentRegistry, EDITABLE_LIMIT_BYTES, PagedSource, TextStateId,
};
use slterm_path::{AppPaths, PathGrant, PathOperation, PathPolicy, PathRequest, ResolvedPath};
use slterm_workspace::ProjectSnapshot;

use super::envelope::DocumentSnapshot;
use super::store::DocumentSnapshotStore;
use crate::app_shell::paged_document::PagedDocument;
use crate::persistence::startup::recovery::{SnapshotRecoverer, SnapshotRecoveryError};
use crate::platform::document_paths::{DocumentPathResolver, SystemDocumentPathResolver};

enum DiskDocument {
    Editable {
        location: ResolvedPath,
        text: String,
        baseline: Arc<[u8]>,
        metadata: DocumentMetadata,
    },
    Paged {
        location: ResolvedPath,
        source: PagedSource,
        metadata: DocumentMetadata,
    },
    Unavailable {
        location: ResolvedPath,
    },
}

impl DiskDocument {
    fn location(&self) -> &ResolvedPath {
        match self {
            Self::Editable { location, .. }
            | Self::Paged { location, .. }
            | Self::Unavailable { location } => location,
        }
    }
}

/// 真实启动恢复器；正文与授权占位都只依附已经归一的布局。
pub(crate) struct DocumentSnapshotRecoverer {
    store: DocumentSnapshotStore,
    resolver: Arc<dyn DocumentPathResolver>,
    registry: DocumentRegistry,
    layout: Option<ProjectSnapshot>,
    orphans: Vec<DocumentSnapshot>,
    quarantined: Vec<PathBuf>,
    requires_authorization: BTreeSet<DocumentId>,
    identity_replaced_documents: BTreeSet<DocumentId>,
    expected_baselines: BTreeMap<DocumentId, [u8; 32]>,
}

impl DocumentSnapshotRecoverer {
    pub(crate) fn new(paths: &AppPaths) -> Self {
        Self {
            store: DocumentSnapshotStore::new(paths),
            resolver: Arc::new(SystemDocumentPathResolver),
            registry: DocumentRegistry::new(),
            layout: None,
            orphans: Vec::new(),
            quarantined: Vec::new(),
            requires_authorization: BTreeSet::new(),
            identity_replaced_documents: BTreeSet::new(),
            expected_baselines: BTreeMap::new(),
        }
    }

    pub(crate) const fn registry(&self) -> &DocumentRegistry {
        &self.registry
    }

    pub(crate) fn take_registry(&mut self) -> DocumentRegistry {
        std::mem::take(&mut self.registry)
    }

    pub(crate) fn layout(&self) -> &ProjectSnapshot {
        self.layout.as_ref().expect("布局必须先于快照对账")
    }

    pub(crate) fn orphans(&self) -> &[DocumentSnapshot] {
        &self.orphans
    }

    pub(crate) fn quarantined(&self) -> &[PathBuf] {
        &self.quarantined
    }

    pub(crate) fn requires_authorization(&self, id: DocumentId) -> bool {
        self.requires_authorization.contains(&id)
    }

    pub(crate) fn identity_was_replaced(&self, id: DocumentId) -> bool {
        self.identity_replaced_documents.contains(&id)
    }

    /// 根外文档只能由本次显式会话授权重新读盘；快照正文与 DocumentId 不变。
    pub(crate) fn reauthorize(
        &mut self,
        registry: &mut DocumentRegistry,
        id: DocumentId,
        grant: PathGrant,
    ) -> io::Result<()> {
        let location = self
            .layout
            .as_ref()
            .and_then(|layout| {
                layout
                    .documents()
                    .find_map(|(document, location)| (document == id).then_some(location))
            })
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "布局中不存在该 Document"))?
            .clone();
        let disk = self.read(&location, &[], Some(grant), None)?;
        if registry.session(id).is_some() {
            let conflict = match &disk {
                DiskDocument::Editable {
                    baseline, location, ..
                } => {
                    let changed = self.expected_baselines.get(&id).is_some_and(|expected| {
                        <[u8; 32]>::from(Sha256::digest(baseline)) != *expected
                    });
                    changed.then(|| DocumentConflict::modified(location.clone(), baseline.clone()))
                }
                DiskDocument::Paged { location, .. } => {
                    Some(DocumentConflict::modified_paged(location.clone()))
                }
                DiskDocument::Unavailable { location } => {
                    Some(DocumentConflict::modified_paged(location.clone()))
                }
            };
            registry
                .reauthorize(id, disk.location().clone(), disk_baseline(&disk), conflict)
                .map_err(io::Error::other)?;
        } else {
            Self::open_clean(registry, id, disk)?;
        }
        self.requires_authorization.remove(&id);
        self.expected_baselines.remove(&id);
        Ok(())
    }

    fn recover_documents(&mut self, layout: &ProjectSnapshot) -> io::Result<()> {
        let scan = self.store.scan_for_recovery().map_err(io::Error::other)?;
        self.quarantined.clear();
        for path in &scan.corrupt {
            let result = self.store.quarantine(path);
            let succeeded = result.is_ok();
            crate::audit::record(
                crate::audit::AuditEvent::new(
                    "snapshot_quarantine",
                    "document_snapshot::recovery",
                    if succeeded {
                        crate::audit::AuditResult::Ok
                    } else {
                        crate::audit::AuditResult::Error
                    },
                )
                .with_path(path),
            );
            let target = result?;
            self.quarantined.push(target);
        }
        let mut snapshots = scan
            .snapshots
            .into_iter()
            .map(|snapshot| (snapshot.document_id(), snapshot))
            .collect::<BTreeMap<_, _>>();
        self.registry = DocumentRegistry::new();
        self.orphans.clear();
        self.requires_authorization.clear();
        self.identity_replaced_documents.clear();
        self.expected_baselines.clear();
        self.layout = Some(layout.clone());

        for (id, location) in layout.documents() {
            let grants = crate::app_shell::documents::document_project_grants(layout, id);
            let snapshot = snapshots.remove(&id);
            if let Some(snapshot) = &snapshot
                && snapshot.location().canonical_path() != location.canonical_path()
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "快照位置与布局记录不一致",
                ));
            }
            if grants.is_empty() || !automatically_authorized(location, &grants) {
                self.requires_authorization.insert(id);
                if let Some(snapshot) = snapshot {
                    self.expected_baselines
                        .insert(id, *snapshot.disk_baseline_hash());
                    self.restore_snapshot(snapshot, None, true)?;
                }
                continue;
            }

            let requested = snapshot
                .as_ref()
                .map(|snapshot| snapshot.encoding().encoding);
            match self.read(location, &grants, None, requested) {
                Ok(disk) => {
                    if snapshot.is_none()
                        && location.file_identity().is_some()
                        && location.file_identity() != disk.location().file_identity()
                    {
                        self.identity_replaced_documents.insert(id);
                        continue;
                    }
                    if snapshot.is_none() && matches!(disk, DiskDocument::Unavailable { .. }) {
                        continue;
                    }
                    if let Some(snapshot) = snapshot {
                        self.expected_baselines
                            .insert(id, *snapshot.disk_baseline_hash());
                        self.restore_authorized_snapshot(snapshot, disk)?;
                    } else {
                        Self::open_clean(&mut self.registry, id, disk)?;
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    if let Some(snapshot) = snapshot {
                        self.restore_snapshot(
                            snapshot,
                            Some(DocumentConflict::deleted(location.clone())),
                            false,
                        )?;
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
                    if let Some(snapshot) = snapshot {
                        self.restore_snapshot(snapshot, None, true)?;
                    }
                    self.requires_authorization.insert(id);
                }
                Err(error) if error.kind() == io::ErrorKind::InvalidData => {
                    if let Some(snapshot) = snapshot {
                        self.restore_snapshot(
                            snapshot,
                            Some(DocumentConflict::modified_paged(location.clone())),
                            false,
                        )?;
                    }
                }
                Err(error) => return Err(error),
            }
        }
        self.orphans = snapshots.into_values().collect();
        Ok(())
    }

    fn restore_authorized_snapshot(
        &mut self,
        snapshot: DocumentSnapshot,
        disk: DiskDocument,
    ) -> io::Result<()> {
        match disk {
            DiskDocument::Editable {
                location,
                text,
                baseline,
                metadata,
            } => {
                if snapshot
                    .encoding()
                    .encode(snapshot.text())
                    .is_ok_and(|encoded| baseline.as_ref() == encoded)
                {
                    let state = snapshot.current_state();
                    let recovered_location = if identity_replaced(&snapshot, &location) {
                        snapshot.location().clone()
                    } else {
                        location
                    };
                    self.registry
                        .restore(
                            snapshot.document_id(),
                            DocumentRecovery {
                                location: recovered_location,
                                body: Rope::from_str(&text).into(),
                                saved_state: state,
                                current_state: state,
                                disk_baseline: baseline,
                                metadata,
                                read_revoked: false,
                                external_conflict: None,
                            },
                        )
                        .map_err(io::Error::other)?;
                    self.store
                        .delete_validated(snapshot.document_id())
                        .map_err(io::Error::other)?;
                    return Ok(());
                }
                if identity_replaced(&snapshot, &location) {
                    return self.restore_snapshot(
                        snapshot.clone(),
                        Some(DocumentConflict::modified_paged(
                            snapshot.location().clone(),
                        )),
                        false,
                    );
                }
                let changed =
                    <[u8; 32]>::from(Sha256::digest(&baseline)) != *snapshot.disk_baseline_hash();
                let conflict =
                    changed.then(|| DocumentConflict::modified(location.clone(), baseline.clone()));
                self.restore_snapshot_at(snapshot, location, baseline, metadata, false, conflict)
            }
            DiskDocument::Paged {
                location, metadata, ..
            } => {
                if identity_replaced(&snapshot, &location) {
                    return self.restore_snapshot(
                        snapshot.clone(),
                        Some(DocumentConflict::modified_paged(
                            snapshot.location().clone(),
                        )),
                        false,
                    );
                }
                self.restore_snapshot_at(
                    snapshot,
                    location.clone(),
                    Arc::default(),
                    metadata,
                    false,
                    Some(DocumentConflict::modified_paged(location)),
                )
            }
            DiskDocument::Unavailable { location } => self.restore_snapshot(
                snapshot,
                Some(DocumentConflict::modified_paged(location)),
                false,
            ),
        }
    }

    fn restore_snapshot(
        &mut self,
        snapshot: DocumentSnapshot,
        conflict: Option<DocumentConflict>,
        revoked: bool,
    ) -> io::Result<()> {
        self.restore_snapshot_at(
            snapshot.clone(),
            snapshot.location().clone(),
            Arc::default(),
            DocumentMetadata {
                encoding: snapshot.encoding().clone(),
                read_only: false,
            },
            revoked,
            conflict,
        )
    }

    fn restore_snapshot_at(
        &mut self,
        snapshot: DocumentSnapshot,
        location: ResolvedPath,
        baseline: Arc<[u8]>,
        metadata: DocumentMetadata,
        revoked: bool,
        conflict: Option<DocumentConflict>,
    ) -> io::Result<()> {
        self.registry
            .restore(
                snapshot.document_id(),
                DocumentRecovery {
                    location,
                    body: Rope::from_str(snapshot.text()).into(),
                    saved_state: snapshot.saved_state(),
                    current_state: snapshot.current_state(),
                    disk_baseline: baseline,
                    metadata,
                    read_revoked: revoked,
                    external_conflict: conflict,
                },
            )
            .map_err(io::Error::other)?;
        Ok(())
    }

    fn open_clean(
        registry: &mut DocumentRegistry,
        id: DocumentId,
        disk: DiskDocument,
    ) -> io::Result<()> {
        let state =
            TextStateId::from_uuid(uuid::Uuid::now_v7()).expect("系统生成的 TextStateId 必须为 v7");
        let recovery = match disk {
            DiskDocument::Editable {
                location,
                text,
                baseline,
                metadata,
            } => DocumentRecovery {
                location,
                body: Rope::from_str(&text).into(),
                saved_state: state,
                current_state: state,
                disk_baseline: baseline,
                metadata,
                read_revoked: false,
                external_conflict: None,
            },
            DiskDocument::Paged {
                location,
                source,
                metadata,
            } => DocumentRecovery {
                location,
                body: source.into(),
                saved_state: state,
                current_state: state,
                disk_baseline: Arc::default(),
                metadata,
                read_revoked: false,
                external_conflict: None,
            },
            DiskDocument::Unavailable { .. } => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "磁盘内容不可用作干净正文",
                ));
            }
        };
        registry.restore(id, recovery).map_err(io::Error::other)?;
        Ok(())
    }

    fn read(
        &self,
        location: &ResolvedPath,
        grants: &[PathGrant],
        session_grant: Option<PathGrant>,
        requested: Option<DocumentEncoding>,
    ) -> io::Result<DiskDocument> {
        let resolved = self.resolver.resolve(location.canonical_path())?;
        let request = PathRequest {
            operation: if session_grant.is_some() {
                PathOperation::Read
            } else {
                PathOperation::AutomaticRead
            },
            lexical_path: location.canonical_path().as_path().to_path_buf(),
            root_hints: Vec::new(),
            session_grant,
            expected_authorization_generation: 0,
        };
        let authorization = PathPolicy::authorize(&request, &resolved, grants, 0)
            .map_err(|_| io::Error::new(io::ErrorKind::PermissionDenied, "文档恢复未获授权"))?;
        let resolved = authorization.resolved_path().clone();
        let mut file = std::fs::File::open(resolved.canonical_path().as_path())?;
        let metadata = file.metadata()?;
        let identity = crate::atomic_file::file_identity(&file)?;
        let location = ResolvedPath::new(resolved.canonical_path().clone(), Some(identity));
        if metadata.len() > EDITABLE_LIMIT_BYTES {
            return match PagedDocument::open(&location, requested) {
                Ok(paged) => Ok(DiskDocument::Paged {
                    location,
                    source: paged.source(),
                    metadata: DocumentMetadata {
                        encoding: paged.encoding().clone(),
                        read_only: true,
                    },
                }),
                Err(error) if error.kind() == io::ErrorKind::InvalidData => {
                    Ok(DiskDocument::Unavailable { location })
                }
                Err(error) => Err(error),
            };
        }
        let mut bytes = Vec::with_capacity(metadata.len() as usize);
        std::io::Read::read_to_end(&mut file, &mut bytes)?;
        if bytes.len() as u64 != metadata.len() {
            return Err(io::Error::other("恢复读取期间文件长度已变化"));
        }
        let content = DocumentContent::decode(bytes, metadata.len(), requested);
        match content {
            DocumentContent::Editable(decoded) => Ok(DiskDocument::Editable {
                location,
                text: decoded.text,
                baseline: decoded.disk_baseline,
                metadata: DocumentMetadata {
                    encoding: decoded.encoding,
                    read_only: metadata.permissions().readonly(),
                },
            }),
            DocumentContent::Paged(_) | DocumentContent::Unavailable(_) => {
                Ok(DiskDocument::Unavailable { location })
            }
        }
    }
}

impl SnapshotRecoverer for DocumentSnapshotRecoverer {
    fn recover(&mut self, layout: &ProjectSnapshot) -> Result<(), SnapshotRecoveryError> {
        self.recover_documents(layout)
            .map_err(|_| SnapshotRecoveryError::Failed)
    }
}

fn identity_replaced(snapshot: &DocumentSnapshot, current: &ResolvedPath) -> bool {
    snapshot.location().file_identity().is_some()
        && snapshot.location().file_identity() != current.file_identity()
}

fn automatically_authorized(location: &ResolvedPath, grants: &[PathGrant]) -> bool {
    let request = PathRequest {
        operation: PathOperation::AutomaticRead,
        lexical_path: location.canonical_path().as_path().to_path_buf(),
        root_hints: Vec::new(),
        session_grant: None,
        expected_authorization_generation: 0,
    };
    PathPolicy::authorize(&request, location, grants, 0).is_ok()
}

fn disk_baseline(disk: &DiskDocument) -> Arc<[u8]> {
    match disk {
        DiskDocument::Editable { baseline, .. } => baseline.clone(),
        DiskDocument::Paged { .. } | DiskDocument::Unavailable { .. } => Arc::default(),
    }
}
