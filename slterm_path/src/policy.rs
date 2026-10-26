use std::fmt;
use std::path::{Component, Path, PathBuf, Prefix};

/// 可分别授予的路径能力位。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Capabilities(u16);

impl Capabilities {
    pub const READ: Self = Self(1 << 0);
    pub const WRITE: Self = Self(1 << 1);
    pub const TERMINAL_CWD: Self = Self(1 << 2);
    pub const STARTUP_RESTORE: Self = Self(1 << 3);
    pub const HOOKS_READ: Self = Self(1 << 4);
    pub const HOOKS_WRITE: Self = Self(1 << 5);
    pub const HISTORY_READ: Self = Self(1 << 6);
    pub const HISTORY_QUARANTINE: Self = Self(1 << 7);
    pub const HISTORY_RESTORE: Self = Self(1 << 8);

    /// 无任何能力。
    pub const fn empty() -> Self {
        Self(0)
    }

    /// 是否没有任何能力。
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// 合并两组能力，不互相否决。
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// 保留两组能力的交集。
    pub const fn intersection(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }

    /// 是否包含另一组能力的全部位。
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

/// 授权有效的持久范围；排序从最短到最长。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum GrantPersistence {
    OneShot,
    Session,
    Persistent,
}

/// 需要独立判定的路径操作。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PathOperation {
    Read,
    AutomaticRead,
    ResourceRead,
    Write,
    TerminalCwd,
    RestoreTerminalCwd,
    HooksRead,
    HooksWrite,
    HistoryRead,
    HistoryQuarantine,
    HistoryRestore,
}

impl PathOperation {
    fn required_capabilities(self) -> Capabilities {
        match self {
            Self::Read | Self::AutomaticRead | Self::ResourceRead => Capabilities::READ,
            Self::Write => Capabilities::WRITE,
            Self::TerminalCwd => Capabilities::TERMINAL_CWD,
            Self::RestoreTerminalCwd => {
                Capabilities::TERMINAL_CWD.union(Capabilities::STARTUP_RESTORE)
            }
            Self::HooksRead => Capabilities::HOOKS_READ,
            Self::HooksWrite => Capabilities::HOOKS_WRITE,
            Self::HistoryRead => Capabilities::HISTORY_READ,
            Self::HistoryQuarantine => Capabilities::HISTORY_QUARANTINE,
            Self::HistoryRestore => Capabilities::HISTORY_RESTORE,
        }
    }

    fn requires_persistent_grant(self) -> bool {
        matches!(
            self,
            Self::AutomaticRead | Self::ResourceRead | Self::RestoreTerminalCwd
        )
    }
}

/// 路径输入错误。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathError {
    NotAbsolute,
    DevicePath,
    NonCanonical,
}

/// application 已解析并规范化的绝对路径；core 不访问文件系统。
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CanonicalPath(PathBuf);

fn validate_absolute(path: &Path) -> Result<(), PathError> {
    if let Some(Component::Prefix(prefix)) = path.components().next()
        && matches!(
            prefix.kind(),
            Prefix::Verbatim(_)
                | Prefix::VerbatimDisk(_)
                | Prefix::VerbatimUNC(_, _)
                | Prefix::DeviceNS(_)
        )
    {
        return Err(PathError::DevicePath);
    }
    if !path.is_absolute() {
        return Err(PathError::NotAbsolute);
    }
    Ok(())
}

impl CanonicalPath {
    /// 接收 PathResolver 的规范化结果；拒绝相对、设备与父目录跳转路径。
    pub fn new(path: impl Into<PathBuf>) -> Result<Self, PathError> {
        let path = path.into();
        validate_absolute(&path)?;
        if path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
        {
            return Err(PathError::NonCanonical);
        }
        Ok(Self(path))
    }

    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

/// 卷与稳定文件标识组成的别名身份。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FileIdentity {
    pub volume: u64,
    pub file: u128,
}

/// application 对同一词法请求本次重新解析得到的结果。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedPath {
    canonical_path: CanonicalPath,
    file_identity: Option<FileIdentity>,
}

impl ResolvedPath {
    pub fn new(canonical_path: CanonicalPath, file_identity: Option<FileIdentity>) -> Self {
        Self {
            canonical_path,
            file_identity,
        }
    }

    pub fn canonical_path(&self) -> &CanonicalPath {
        &self.canonical_path
    }

    pub const fn file_identity(&self) -> Option<FileIdentity> {
        self.file_identity
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GrantScope {
    Subtree,
    ExactFile,
}

/// 一个来源授予的最小路径与能力集合。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PathGrant {
    path: CanonicalPath,
    scope: GrantScope,
    capabilities: Capabilities,
    persistence: GrantPersistence,
}

impl PathGrant {
    fn new(
        path: CanonicalPath,
        scope: GrantScope,
        capabilities: Capabilities,
        persistence: GrantPersistence,
    ) -> Self {
        Self {
            path,
            scope,
            capabilities,
            persistence,
        }
    }

    pub fn project_root(path: CanonicalPath) -> Self {
        let capabilities = Capabilities::READ
            .union(Capabilities::WRITE)
            .union(Capabilities::TERMINAL_CWD)
            .union(Capabilities::STARTUP_RESTORE);
        Self::new(
            path,
            GrantScope::Subtree,
            capabilities,
            GrantPersistence::Persistent,
        )
    }

    pub fn repo_root(path: CanonicalPath) -> Self {
        Self::new(
            path,
            GrantScope::Subtree,
            Capabilities::READ,
            GrantPersistence::Persistent,
        )
    }

    pub fn agent_root(path: CanonicalPath, allow_write: bool, allow_terminal: bool) -> Self {
        let mut capabilities = Capabilities::READ;
        if allow_write {
            capabilities = capabilities.union(Capabilities::WRITE);
        }
        if allow_terminal {
            capabilities = capabilities.union(Capabilities::TERMINAL_CWD);
        }
        Self::new(
            path,
            GrantScope::Subtree,
            capabilities,
            GrantPersistence::Persistent,
        )
    }

    pub fn session_file_read(path: CanonicalPath) -> Self {
        Self::new(
            path,
            GrantScope::ExactFile,
            Capabilities::READ,
            GrantPersistence::Session,
        )
    }

    pub fn save_as_target(path: CanonicalPath) -> Self {
        Self::new(
            path,
            GrantScope::ExactFile,
            Capabilities::WRITE,
            GrantPersistence::Session,
        )
    }

    pub fn output_target(path: CanonicalPath) -> Self {
        Self::new(
            path,
            GrantScope::ExactFile,
            Capabilities::WRITE,
            GrantPersistence::OneShot,
        )
    }

    pub fn hooks_read_target(path: CanonicalPath) -> Self {
        Self::integration_target(path, Capabilities::HOOKS_READ)
    }

    pub fn hooks_write_target(path: CanonicalPath) -> Self {
        Self::integration_target(path, Capabilities::HOOKS_WRITE)
    }

    pub fn history_read_target(path: CanonicalPath) -> Self {
        Self::integration_target(path, Capabilities::HISTORY_READ)
    }

    pub fn history_quarantine_target(path: CanonicalPath) -> Self {
        Self::integration_target(path, Capabilities::HISTORY_QUARANTINE)
    }

    pub fn history_restore_target(path: CanonicalPath) -> Self {
        Self::integration_target(path, Capabilities::HISTORY_RESTORE)
    }

    fn integration_target(path: CanonicalPath, capabilities: Capabilities) -> Self {
        Self::new(
            path,
            GrantScope::ExactFile,
            capabilities,
            GrantPersistence::Persistent,
        )
    }

    pub fn path(&self) -> &CanonicalPath {
        &self.path
    }

    pub const fn capabilities(&self) -> Capabilities {
        self.capabilities
    }

    pub const fn persistence(&self) -> GrantPersistence {
        self.persistence
    }

    fn covers(&self, target: &CanonicalPath) -> bool {
        match self.scope {
            GrantScope::Subtree => target.as_path().starts_with(self.path.as_path()),
            GrantScope::ExactFile => self.path == *target,
        }
    }
}

/// 统一路径请求；词法路径与根提示由 application PathResolver 消费。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PathRequest {
    pub operation: PathOperation,
    pub lexical_path: PathBuf,
    pub root_hints: Vec<PathBuf>,
    pub session_grant: Option<PathGrant>,
    pub expected_authorization_generation: u64,
}

/// 本次操作获准的最小结果，不扩张到其他操作。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PathAuthorization {
    resolved_path: ResolvedPath,
    capabilities: Capabilities,
    persistence: GrantPersistence,
}

impl PathAuthorization {
    pub fn resolved_path(&self) -> &ResolvedPath {
        &self.resolved_path
    }

    pub const fn capabilities(&self) -> Capabilities {
        self.capabilities
    }

    pub const fn persistence(&self) -> GrantPersistence {
        self.persistence
    }
}

/// 默认拒绝的授权失败原因。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PathDenial {
    InvalidPath(PathError),
    GenerationMismatch { expected: u64, actual: u64 },
    Unauthorized(PathOperation),
}

/// 纯路径授权判定，不持有根状态或执行解析 I/O。
pub struct PathPolicy;

impl PathPolicy {
    pub fn authorize(
        request: &PathRequest,
        resolved_path: &ResolvedPath,
        persistent_grants: &[PathGrant],
        current_authorization_generation: u64,
    ) -> Result<PathAuthorization, PathDenial> {
        validate_absolute(&request.lexical_path).map_err(PathDenial::InvalidPath)?;
        if request.expected_authorization_generation != current_authorization_generation {
            return Err(PathDenial::GenerationMismatch {
                expected: request.expected_authorization_generation,
                actual: current_authorization_generation,
            });
        }

        let required = request.operation.required_capabilities();
        let candidates = persistent_grants
            .iter()
            .chain(request.session_grant.as_ref())
            .filter(|grant| {
                grant.covers(&resolved_path.canonical_path)
                    && (!request.operation.requires_persistent_grant()
                        || grant.persistence == GrantPersistence::Persistent)
            })
            .collect::<Vec<_>>();

        // 选择全部所需能力都能达到的最长持久范围，避免把单项持久性误套给整组能力。
        for persistence in [
            GrantPersistence::Persistent,
            GrantPersistence::Session,
            GrantPersistence::OneShot,
        ] {
            let capabilities = candidates
                .iter()
                .filter(|grant| grant.persistence >= persistence)
                .fold(Capabilities::empty(), |capabilities, grant| {
                    capabilities.union(grant.capabilities)
                });
            if capabilities.contains(required) {
                return Ok(PathAuthorization {
                    resolved_path: resolved_path.clone(),
                    capabilities: capabilities.intersection(required),
                    persistence,
                });
            }
        }
        Err(PathDenial::Unauthorized(request.operation))
    }
}

impl fmt::Display for PathError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::NotAbsolute => "路径必须为绝对路径",
            Self::DevicePath => "设备路径不能作为授权输入",
            Self::NonCanonical => "规范路径不能包含父目录跳转",
        })
    }
}

impl std::error::Error for PathError {}

impl fmt::Display for PathDenial {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPath(error) => fmt::Display::fmt(error, formatter),
            Self::GenerationMismatch { .. } => {
                formatter.write_str("授权代次已变化，必须重新解析与判定")
            }
            Self::Unauthorized(operation) => {
                write!(formatter, "路径未授予 {operation:?} 所需能力")
            }
        }
    }
}

impl std::error::Error for PathDenial {}

#[cfg(test)]
mod policy_tests;
