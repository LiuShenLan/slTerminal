//! 文件树单项操作：先捕获不可变计划，执行前重新解析、授权并比对真实状态。
//! 本服务含阻塞 I/O，只允许在后台执行体调用。

use std::{
    io::{self, Read as _},
    sync::Arc,
    time::UNIX_EPOCH,
};

use sha2::{Digest as _, Sha256};
use slterm_path::{
    CanonicalPath, FileIdentity, PathGrant, PathOperation, PathPolicy, PathRequest, ResolvedPath,
};
use slterm_sidebar::FileTreeRoot;
use uuid::Uuid;

use crate::platform::{
    document_paths::{DocumentPathResolver, SystemDocumentPathResolver},
    file_operations,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FileOperationKind {
    CreateFile,
    CreateDirectory,
    Copy,
    Rename,
    Delete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RecyclePolicy {
    NotApplicable,
    RecycleBin,
}

#[derive(Clone, Debug)]
struct OperationDestination {
    lexical_path: CanonicalPath,
    canonical_path: CanonicalPath,
    file_identity: Option<FileIdentity>,
    parent: ResolvedPath,
}

/// 已授权的计划只保存预期事实，不保存可沿用到执行时的授权结果。
#[derive(Clone, Debug)]
pub(crate) struct OperationPlan {
    kind: FileOperationKind,
    root_id: FileTreeRoot,
    source_root_id: Option<FileTreeRoot>,
    authorization_generation: u64,
    lexical_path: CanonicalPath,
    canonical_path: CanonicalPath,
    file_identity: Option<FileIdentity>,
    expected_fingerprint: Option<[u8; 32]>,
    parent: Option<ResolvedPath>,
    destination: Option<OperationDestination>,
    recycle_policy: RecyclePolicy,
    document_revision: Option<u64>,
    operation_id: Uuid,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FileOperationOutcome {
    Completed,
    AlreadyAbsent,
}

#[derive(Debug)]
pub(crate) enum FileOperationError {
    Unauthorized,
    Changed,
    Io(io::Error),
}

impl From<io::Error> for FileOperationError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

pub(crate) trait RecycleBin: Send + Sync {
    fn send_to_recycle_bin(&self, path: &std::path::Path) -> io::Result<()>;
}

struct SystemRecycleBin;

impl RecycleBin for SystemRecycleBin {
    fn send_to_recycle_bin(&self, path: &std::path::Path) -> io::Result<()> {
        file_operations::send_to_recycle_bin(path).map_err(io::Error::other)
    }
}

pub(crate) struct FileOperationService {
    resolver: Arc<dyn DocumentPathResolver>,
    recycle_bin: Arc<dyn RecycleBin>,
}

impl Default for FileOperationService {
    fn default() -> Self {
        Self {
            resolver: Arc::new(SystemDocumentPathResolver),
            recycle_bin: Arc::new(SystemRecycleBin),
        }
    }
}

impl FileOperationService {
    #[cfg(test)]
    pub(super) fn with_recycle_bin_for_test(recycle_bin: Arc<dyn RecycleBin>) -> Self {
        Self {
            recycle_bin,
            ..Self::default()
        }
    }

    pub(crate) fn plan_create_file(
        &self,
        root_id: FileTreeRoot,
        target: &CanonicalPath,
        grants: &[PathGrant],
        authorization_generation: u64,
    ) -> Result<OperationPlan, FileOperationError> {
        self.plan_create(
            FileOperationKind::CreateFile,
            root_id,
            target,
            grants,
            authorization_generation,
        )
    }

    pub(crate) fn plan_create_directory(
        &self,
        root_id: FileTreeRoot,
        target: &CanonicalPath,
        grants: &[PathGrant],
        authorization_generation: u64,
    ) -> Result<OperationPlan, FileOperationError> {
        self.plan_create(
            FileOperationKind::CreateDirectory,
            root_id,
            target,
            grants,
            authorization_generation,
        )
    }

    fn plan_create(
        &self,
        kind: FileOperationKind,
        root_id: FileTreeRoot,
        target: &CanonicalPath,
        grants: &[PathGrant],
        authorization_generation: u64,
    ) -> Result<OperationPlan, FileOperationError> {
        validate_entry_path(target)?;
        let resolved = self.resolver.resolve_write_target(target)?;
        if resolved.file_identity().is_some() {
            return Err(io::Error::new(io::ErrorKind::AlreadyExists, "目标已存在").into());
        }
        let resolved = self.authorize(
            target,
            &resolved,
            grants,
            authorization_generation,
            authorization_generation,
        )?;
        let parent = self.resolve_parent(&resolved)?;
        Ok(OperationPlan {
            kind,
            root_id,
            source_root_id: None,
            authorization_generation,
            lexical_path: target.clone(),
            canonical_path: resolved.canonical_path().clone(),
            file_identity: None,
            expected_fingerprint: None,
            parent: Some(parent),
            destination: None,
            recycle_policy: RecyclePolicy::NotApplicable,
            document_revision: None,
            operation_id: Uuid::now_v7(),
        })
    }

    pub(crate) fn check_source(
        &self,
        source: &CanonicalPath,
        operation: PathOperation,
        grants: &[PathGrant],
        authorization_generation: u64,
    ) -> Result<(), FileOperationError> {
        let resolved = self.resolver.resolve(source)?;
        let resolved = self.authorize_with(
            operation,
            source,
            &resolved,
            grants,
            authorization_generation,
            authorization_generation,
        )?;
        fingerprint(&resolved)?;
        Ok(())
    }

    pub(crate) fn plan_copy(
        &self,
        source_root: FileTreeRoot,
        destination_root: FileTreeRoot,
        source: &CanonicalPath,
        destination: &CanonicalPath,
        grants: &[PathGrant],
        authorization_generation: u64,
    ) -> Result<OperationPlan, FileOperationError> {
        validate_entry_path(destination)?;
        let resolved = self.resolver.resolve(source)?;
        let resolved = self.authorize_with(
            PathOperation::Read,
            source,
            &resolved,
            grants,
            authorization_generation,
            authorization_generation,
        )?;
        let expected_fingerprint = fingerprint(&resolved)?;
        let destination_resolved = self.resolver.resolve_write_target(destination)?;
        if destination_resolved.file_identity().is_some() {
            return Err(io::Error::new(io::ErrorKind::AlreadyExists, "目标已存在").into());
        }
        let destination_resolved = self.authorize(
            destination,
            &destination_resolved,
            grants,
            authorization_generation,
            authorization_generation,
        )?;
        let destination_parent = self.resolve_parent(&destination_resolved)?;
        Ok(OperationPlan {
            kind: FileOperationKind::Copy,
            root_id: destination_root,
            source_root_id: Some(source_root),
            authorization_generation,
            lexical_path: source.clone(),
            canonical_path: resolved.canonical_path().clone(),
            file_identity: resolved.file_identity(),
            expected_fingerprint: Some(expected_fingerprint),
            parent: None,
            destination: Some(OperationDestination {
                lexical_path: destination.clone(),
                canonical_path: destination_resolved.canonical_path().clone(),
                file_identity: None,
                parent: destination_parent,
            }),
            recycle_policy: RecyclePolicy::NotApplicable,
            document_revision: None,
            operation_id: Uuid::now_v7(),
        })
    }

    pub(crate) fn plan_rename(
        &self,
        root_id: FileTreeRoot,
        source: &CanonicalPath,
        destination: &CanonicalPath,
        grants: &[PathGrant],
        authorization_generation: u64,
    ) -> Result<OperationPlan, FileOperationError> {
        validate_entry_path(destination)?;
        let resolved = self.resolver.resolve(source)?;
        let resolved = self.authorize(
            source,
            &resolved,
            grants,
            authorization_generation,
            authorization_generation,
        )?;
        let expected_fingerprint = fingerprint(&resolved)?;
        let destination_resolved = self.resolver.resolve_write_target(destination)?;
        if destination_resolved.file_identity().is_some()
            && destination_resolved.file_identity() != resolved.file_identity()
        {
            return Err(io::Error::new(io::ErrorKind::AlreadyExists, "目标已存在").into());
        }
        let destination_resolved = self.authorize(
            destination,
            &destination_resolved,
            grants,
            authorization_generation,
            authorization_generation,
        )?;
        let destination_parent = self.resolve_parent(&destination_resolved)?;
        Ok(OperationPlan {
            kind: FileOperationKind::Rename,
            root_id,
            source_root_id: None,
            authorization_generation,
            lexical_path: source.clone(),
            canonical_path: resolved.canonical_path().clone(),
            file_identity: resolved.file_identity(),
            expected_fingerprint: Some(expected_fingerprint),
            parent: None,
            destination: Some(OperationDestination {
                lexical_path: destination.clone(),
                canonical_path: destination_resolved.canonical_path().clone(),
                file_identity: destination_resolved.file_identity(),
                parent: destination_parent,
            }),
            recycle_policy: RecyclePolicy::NotApplicable,
            document_revision: None,
            operation_id: Uuid::now_v7(),
        })
    }

    pub(crate) fn plan_delete(
        &self,
        root_id: FileTreeRoot,
        target: &CanonicalPath,
        grants: &[PathGrant],
        authorization_generation: u64,
    ) -> Result<OperationPlan, FileOperationError> {
        let resolved = self.resolver.resolve(target)?;
        let resolved = self.authorize(
            target,
            &resolved,
            grants,
            authorization_generation,
            authorization_generation,
        )?;
        let expected_fingerprint = fingerprint(&resolved)?;
        Ok(OperationPlan {
            kind: FileOperationKind::Delete,
            root_id,
            source_root_id: None,
            authorization_generation,
            lexical_path: target.clone(),
            canonical_path: resolved.canonical_path().clone(),
            file_identity: resolved.file_identity(),
            expected_fingerprint: Some(expected_fingerprint),
            parent: None,
            destination: None,
            recycle_policy: RecyclePolicy::RecycleBin,
            document_revision: None,
            operation_id: Uuid::now_v7(),
        })
    }

    pub(crate) fn execute(
        &self,
        plan: &OperationPlan,
        grants: &[PathGrant],
        current_authorization_generation: u64,
    ) -> Result<FileOperationOutcome, FileOperationError> {
        if plan.authorization_generation != current_authorization_generation {
            return Err(FileOperationError::Unauthorized);
        }
        match plan.kind {
            FileOperationKind::CreateFile | FileOperationKind::CreateDirectory => {
                self.execute_create(plan, grants, current_authorization_generation)?;
            }
            FileOperationKind::Rename => {
                self.execute_rename(plan, grants, current_authorization_generation)?;
            }
            FileOperationKind::Copy => {
                self.execute_copy(plan, grants, current_authorization_generation)?;
            }
            FileOperationKind::Delete => {
                return self.execute_delete(plan, grants, current_authorization_generation);
            }
        }
        Ok(FileOperationOutcome::Completed)
    }

    fn execute_create(
        &self,
        plan: &OperationPlan,
        grants: &[PathGrant],
        current_authorization_generation: u64,
    ) -> Result<(), FileOperationError> {
        let resolved = self.resolver.resolve_write_target(&plan.lexical_path)?;
        let resolved = self.authorize(
            &plan.lexical_path,
            &resolved,
            grants,
            plan.authorization_generation,
            current_authorization_generation,
        )?;
        if resolved.canonical_path() != &plan.canonical_path || resolved.file_identity().is_some() {
            return Err(FileOperationError::Changed);
        }
        self.recheck_parent(plan.parent.as_ref().expect("新建计划必须包含父目录"))?;
        match plan.kind {
            FileOperationKind::CreateFile => {
                file_operations::create_file(plan.lexical_path.as_path())?;
            }
            FileOperationKind::CreateDirectory => {
                file_operations::create_directory(plan.lexical_path.as_path())?;
            }
            FileOperationKind::Copy | FileOperationKind::Rename | FileOperationKind::Delete => {
                unreachable!("新建执行体只接受新建计划");
            }
        }
        Ok(())
    }

    fn execute_copy(
        &self,
        plan: &OperationPlan,
        grants: &[PathGrant],
        current_authorization_generation: u64,
    ) -> Result<(), FileOperationError> {
        self.recheck_existing(
            plan,
            PathOperation::Read,
            grants,
            current_authorization_generation,
        )?;
        let destination = plan.destination.as_ref().expect("复制计划必须包含目标");
        let resolved = self
            .resolver
            .resolve_write_target(&destination.lexical_path)?;
        let resolved = self.authorize(
            &destination.lexical_path,
            &resolved,
            grants,
            plan.authorization_generation,
            current_authorization_generation,
        )?;
        if resolved.canonical_path() != &destination.canonical_path
            || resolved.file_identity() != destination.file_identity
        {
            return Err(FileOperationError::Changed);
        }
        self.recheck_parent(&destination.parent)?;
        file_operations::copy_path(
            plan.lexical_path.as_path(),
            destination.lexical_path.as_path(),
        )?;
        Ok(())
    }

    fn execute_rename(
        &self,
        plan: &OperationPlan,
        grants: &[PathGrant],
        current_authorization_generation: u64,
    ) -> Result<(), FileOperationError> {
        self.recheck_existing(
            plan,
            PathOperation::Write,
            grants,
            current_authorization_generation,
        )?;
        let destination = plan.destination.as_ref().expect("重命名计划必须包含目标");
        let resolved = self
            .resolver
            .resolve_write_target(&destination.lexical_path)?;
        let resolved = self.authorize(
            &destination.lexical_path,
            &resolved,
            grants,
            plan.authorization_generation,
            current_authorization_generation,
        )?;
        if resolved.canonical_path() != &destination.canonical_path
            || resolved.file_identity() != destination.file_identity
        {
            return Err(FileOperationError::Changed);
        }
        self.recheck_parent(&destination.parent)?;
        file_operations::rename_path(
            plan.lexical_path.as_path(),
            destination.lexical_path.as_path(),
        )?;
        Ok(())
    }

    fn execute_delete(
        &self,
        plan: &OperationPlan,
        grants: &[PathGrant],
        current_authorization_generation: u64,
    ) -> Result<FileOperationOutcome, FileOperationError> {
        match self.recheck_existing(
            plan,
            PathOperation::Write,
            grants,
            current_authorization_generation,
        ) {
            Ok(()) => {}
            Err(FileOperationError::Io(error)) if error.kind() == io::ErrorKind::NotFound => {
                return match std::fs::symlink_metadata(plan.lexical_path.as_path()) {
                    Ok(_) => Err(FileOperationError::Changed),
                    Err(metadata_error) if metadata_error.kind() == io::ErrorKind::NotFound => {
                        Ok(FileOperationOutcome::AlreadyAbsent)
                    }
                    Err(metadata_error) => Err(FileOperationError::Io(metadata_error)),
                };
            }
            Err(error) => return Err(error),
        }
        debug_assert_eq!(plan.recycle_policy, RecyclePolicy::RecycleBin);
        self.recycle_bin
            .send_to_recycle_bin(plan.lexical_path.as_path())?;
        Ok(FileOperationOutcome::Completed)
    }

    fn recheck_existing(
        &self,
        plan: &OperationPlan,
        operation: PathOperation,
        grants: &[PathGrant],
        current_authorization_generation: u64,
    ) -> Result<(), FileOperationError> {
        let resolved = self.resolver.resolve(&plan.lexical_path)?;
        let resolved = self.authorize_with(
            operation,
            &plan.lexical_path,
            &resolved,
            grants,
            plan.authorization_generation,
            current_authorization_generation,
        )?;
        if resolved.canonical_path() != &plan.canonical_path
            || resolved.file_identity() != plan.file_identity
        {
            return Err(FileOperationError::Changed);
        }
        if Some(fingerprint(&resolved)?) != plan.expected_fingerprint {
            return Err(FileOperationError::Changed);
        }
        Ok(())
    }

    fn resolve_parent(&self, target: &ResolvedPath) -> Result<ResolvedPath, FileOperationError> {
        let parent = target
            .canonical_path()
            .as_path()
            .parent()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "目标缺少父目录"))?;
        let parent = CanonicalPath::new(parent)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
        Ok(self.resolver.resolve(&parent)?)
    }

    fn recheck_parent(&self, expected: &ResolvedPath) -> Result<(), FileOperationError> {
        let current = self.resolver.resolve(expected.canonical_path())?;
        if &current != expected {
            return Err(FileOperationError::Changed);
        }
        Ok(())
    }

    fn authorize(
        &self,
        lexical: &CanonicalPath,
        resolved: &ResolvedPath,
        grants: &[PathGrant],
        expected_generation: u64,
        current_generation: u64,
    ) -> Result<ResolvedPath, FileOperationError> {
        self.authorize_with(
            PathOperation::Write,
            lexical,
            resolved,
            grants,
            expected_generation,
            current_generation,
        )
    }

    fn authorize_with(
        &self,
        operation: PathOperation,
        lexical: &CanonicalPath,
        resolved: &ResolvedPath,
        grants: &[PathGrant],
        expected_generation: u64,
        current_generation: u64,
    ) -> Result<ResolvedPath, FileOperationError> {
        let request = PathRequest {
            operation,
            lexical_path: lexical.as_path().to_path_buf(),
            root_hints: Vec::new(),
            session_grant: None,
            expected_authorization_generation: expected_generation,
        };
        PathPolicy::authorize(&request, resolved, grants, current_generation)
            .map(|authorization| authorization.resolved_path().clone())
            .map_err(|_| FileOperationError::Unauthorized)
    }
}

fn validate_entry_path(path: &CanonicalPath) -> Result<(), FileOperationError> {
    let name = path
        .as_path()
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "目标缺少有效名称"))?;
    file_operations::validate_entry_name(name)?;
    Ok(())
}

/// 文件指纹覆盖元数据与完整字节；目录指纹覆盖其自身元数据，不递归读取后代。
fn fingerprint(resolved: &ResolvedPath) -> io::Result<[u8; 32]> {
    let path = resolved.canonical_path().as_path();
    let metadata = std::fs::metadata(path)?;
    let modified = metadata
        .modified()?
        .duration_since(UNIX_EPOCH)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "文件时间早于 Unix epoch"))?;
    let mut hasher = Sha256::new();
    hasher.update([u8::from(metadata.is_dir())]);
    hasher.update(metadata.len().to_le_bytes());
    hasher.update(modified.as_nanos().to_le_bytes());
    if metadata.is_file() {
        let mut file = std::fs::File::open(path)?;
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            let read = file.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
        }
    }
    Ok(hasher.finalize().into())
}
