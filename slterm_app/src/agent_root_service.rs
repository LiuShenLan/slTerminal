//! 自定义 AgentRoot 注册表：真实目录查重、能力查询、改路径与撤权。
//!
//! 词法路径只用于定位；是否重复根按真实目录解析结果判定。删除根只撤销
//! 登记与授权，不触碰磁盘，也不影响既有 Document 与 PTY。
//!
//! 服务含阻塞 I/O，只在后台调用；本模块不启动线程、不持有 GPUI 实体。

use std::path::Path;
use std::sync::Arc;

use slterm_path::CanonicalPath;
use slterm_settings::{AgentRoot, AgentRootId};

use crate::platform::project_paths::ProjectPathResolver;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AgentRootRegistryError {
    /// 同一真实目录已登记为另一个根（返回既有根身份）。
    DuplicateRealPath(AgentRootId),
    /// 身份已被占用。
    DuplicateIdentity,
    UnknownRoot,
    Unresolved,
}

/// 改路径回执：调用方据此清理旧根专属分区并重绑新根。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AgentRootPathChange {
    pub(crate) revoked: AgentRootId,
    pub(crate) registered: AgentRootId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AgentRootLoadError {
    Io,
    /// 段结构或元素非法：不得当作空配置。
    Invalid,
}

/// 自定义根读取（生产设置路径）。
pub(crate) fn load_agent_roots() -> Result<Vec<AgentRoot>, AgentRootLoadError> {
    let loaded = crate::settings_store::load_settings().map_err(|_| AgentRootLoadError::Io)?;
    decode_agent_roots(
        loaded
            .data
            .external_segment(slterm_settings::AGENT_ROOTS_SEGMENT),
    )
}

/// 显式路径读取（测试注入与启动恢复共用）。
pub(crate) fn load_agent_roots_at(path: &Path) -> Result<Vec<AgentRoot>, AgentRootLoadError> {
    let loaded =
        crate::settings_store::load_settings_at(path).map_err(|_| AgentRootLoadError::Io)?;
    decode_agent_roots(
        loaded
            .data
            .external_segment(slterm_settings::AGENT_ROOTS_SEGMENT),
    )
}

fn decode_agent_roots(
    segment: Option<&slterm_settings::json::Value>,
) -> Result<Vec<AgentRoot>, AgentRootLoadError> {
    slterm_settings::agent_roots_from_segment(segment).map_err(|_| AgentRootLoadError::Invalid)
}

/// 自定义根保存（生产设置路径）：经统一设置写通道。
pub(crate) fn save_agent_roots(roots: &[AgentRoot]) -> std::io::Result<()> {
    crate::settings_store::save_settings_blocking(&[(
        slterm_settings::AGENT_ROOTS_SEGMENT,
        slterm_settings::agent_roots_to_segment(roots),
    )])
}

/// 显式路径保存（测试注入）。
pub(crate) fn save_agent_roots_at(path: &Path, roots: &[AgentRoot]) -> std::io::Result<()> {
    crate::settings_store::save_settings_blocking_at(
        path,
        &[(
            slterm_settings::AGENT_ROOTS_SEGMENT,
            slterm_settings::agent_roots_to_segment(roots),
        )],
    )
}

pub(crate) struct AgentRootRegistry {
    resolver: Arc<dyn ProjectPathResolver>,
    builtin: Vec<CanonicalPath>,
    roots: Vec<(AgentRoot, CanonicalPath)>,
}

impl AgentRootRegistry {
    pub(crate) fn new(resolver: Arc<dyn ProjectPathResolver>) -> Self {
        Self {
            resolver,
            builtin: Vec::new(),
            roots: Vec::new(),
        }
    }

    /// 登记内置根的真实目录；同样参与查重，但不可删除或改路径。
    pub(crate) fn add_builtin(
        &mut self,
        lexical: &CanonicalPath,
    ) -> Result<CanonicalPath, AgentRootRegistryError> {
        let real = self.resolve(lexical)?;
        if let Some(existing) = self.roots.iter().find(|(_, root)| *root == real) {
            return Err(AgentRootRegistryError::DuplicateRealPath(existing.0.id()));
        }
        self.builtin.push(real.clone());
        Ok(real)
    }

    /// 登记自定义根：身份重复或真实目录已被占用时拒绝，且不改变现有登记。
    pub(crate) fn register(
        &mut self,
        root: AgentRoot,
        lexical: &CanonicalPath,
    ) -> Result<CanonicalPath, AgentRootRegistryError> {
        if self.contains_identity(root.id()) {
            return Err(AgentRootRegistryError::DuplicateIdentity);
        }
        let real = self.resolve(lexical)?;
        if let Some(existing) = self.find_owner(&real) {
            return Err(AgentRootRegistryError::DuplicateRealPath(existing));
        }
        self.roots.push((root, real.clone()));
        Ok(real)
    }

    /// 改路径 = 撤销旧根 + 登记新根；新旧真实目录冲突或新解析失败时保持原状。
    pub(crate) fn change_path(
        &mut self,
        id: AgentRootId,
        new_root: AgentRoot,
        lexical: &CanonicalPath,
    ) -> Result<AgentRootPathChange, AgentRootRegistryError> {
        let index = self
            .roots
            .iter()
            .position(|(root, _)| root.id() == id)
            .ok_or(AgentRootRegistryError::UnknownRoot)?;
        if new_root.id() == id || self.contains_identity(new_root.id()) {
            return Err(AgentRootRegistryError::DuplicateIdentity);
        }
        let real = self.resolve(lexical)?;
        // 旧根自身的真实目录可被新根沿用；其他根的占用仍拒绝。
        if let Some(existing) = self
            .roots
            .iter()
            .enumerate()
            .find(|(other, (root, root_real))| {
                *other != index && root.id() != id && *root_real == real
            })
            .map(|(_, (root, _))| root.id())
        {
            return Err(AgentRootRegistryError::DuplicateRealPath(existing));
        }
        if self.builtin.contains(&real) {
            return Err(AgentRootRegistryError::DuplicateRealPath(id));
        }
        let registered = new_root.id();
        self.roots[index] = (new_root, real);
        Ok(AgentRootPathChange {
            revoked: id,
            registered,
        })
    }

    /// 删除根：内置根拒绝；成功后该身份不再可用于任何写或终端授权。
    pub(crate) fn remove(&mut self, id: AgentRootId) -> Result<AgentRoot, AgentRootRegistryError> {
        let index = self
            .roots
            .iter()
            .position(|(root, _)| root.id() == id)
            .ok_or(AgentRootRegistryError::UnknownRoot)?;
        Ok(self.roots.remove(index).0)
    }

    pub(crate) fn find(&self, id: AgentRootId) -> Option<&AgentRoot> {
        self.roots
            .iter()
            .find(|(root, _)| root.id() == id)
            .map(|(root, _)| root)
    }

    pub(crate) fn real_path(&self, id: AgentRootId) -> Option<&CanonicalPath> {
        self.roots
            .iter()
            .find(|(root, _)| root.id() == id)
            .map(|(_, real)| real)
    }

    pub(crate) fn custom_roots(&self) -> impl Iterator<Item = &AgentRoot> {
        self.roots.iter().map(|(root, _)| root)
    }

    /// 覆盖该路径的全部根能力并集：读恒可，写与终端逐项取或。
    /// 撤销任一根后并集按剩余根重算；无根覆盖返回 None。
    pub(crate) fn capability_union_at(
        &self,
        path: &CanonicalPath,
    ) -> Option<slterm_settings::AgentRootCapabilities> {
        let mut covered = false;
        let mut union = slterm_settings::AgentRootCapabilities::READ_ONLY;
        for (root, real) in &self.roots {
            if !path.as_path().starts_with(real.as_path()) {
                continue;
            }
            covered = true;
            union.write |= root.capabilities().write;
            union.terminal |= root.capabilities().terminal;
        }
        covered.then_some(union)
    }

    fn resolve(&self, lexical: &CanonicalPath) -> Result<CanonicalPath, AgentRootRegistryError> {
        self.resolver
            .resolve(lexical)
            .map_err(|_| AgentRootRegistryError::Unresolved)
    }

    fn find_owner(&self, real: &CanonicalPath) -> Option<AgentRootId> {
        self.roots
            .iter()
            .find(|(_, root_real)| root_real == real)
            .map(|(root, _)| root.id())
    }

    fn contains_identity(&self, id: AgentRootId) -> bool {
        self.roots.iter().any(|(root, _)| root.id() == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn create_junction(link: &Path, target: &Path) {
        let status = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(link)
            .arg(target)
            .status()
            .unwrap();
        assert!(status.success(), "Windows 必须能创建临时目录 junction");
    }

    fn resolver() -> Arc<dyn ProjectPathResolver> {
        Arc::new(crate::platform::project_paths::SystemProjectPathResolver)
    }

    fn id(seed: u8) -> AgentRootId {
        let mut bytes = [0_u8; 16];
        bytes[0] = 0x01;
        bytes[1] = 0x8f;
        bytes[2] = 0x2f;
        bytes[3] = 0x7c;
        bytes[4] = 0x8f;
        bytes[5] = 0x5a;
        bytes[6] = 0x7c ^ seed;
        bytes[7] = 0x3d;
        bytes[8] = 0x9e;
        bytes[9] = 0x2b;
        bytes[10] = 0x4a;
        bytes[11] = 0x6f;
        bytes[12] = 0x8c;
        bytes[13] = 0x1d;
        bytes[14] = 0x0e;
        bytes[15] = 0x2b ^ seed;
        AgentRootId::from_bytes(bytes).unwrap()
    }

    fn root(seed: u8, path: &Path) -> AgentRoot {
        AgentRoot::custom(
            id(seed),
            format!("根 {seed}"),
            path.to_string_lossy().into_owned(),
        )
        .unwrap()
    }

    /// 自定义根经统一设置写通道往返；清空后读回空列表。
    #[test]
    fn agent_roots_roundtrip_through_settings_channel() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("settings.json");
        let root = root(1, temp.path()).with_capabilities(slterm_settings::AgentRootCapabilities {
            write: true,
            terminal: false,
        });
        save_agent_roots_at(&path, std::slice::from_ref(&root)).unwrap();
        assert_eq!(load_agent_roots_at(&path).unwrap(), vec![root]);
        save_agent_roots_at(&path, &[]).unwrap();
        assert!(load_agent_roots_at(&path).unwrap().is_empty());
    }

    /// 段结构非法时读取报错且不覆盖原文件，不把损坏当空配置。
    #[test]
    fn agent_roots_load_rejects_malformed_segment_without_overwrite() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("settings.json");
        let original = br#"{"agent_roots":{"roots":"broken"}}"#;
        std::fs::write(&path, original).unwrap();
        assert!(load_agent_roots_at(&path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), original);
    }

    /// 新根默认可读但写与终端分别拒绝；开启写不隐含终端能力。
    #[test]
    fn agent_root_default_capabilities() {
        let temp = tempfile::tempdir().unwrap();
        let mut registry = AgentRootRegistry::new(resolver());
        registry
            .register(
                root(1, temp.path()),
                &CanonicalPath::new(temp.path()).unwrap(),
            )
            .unwrap();
        let registered = registry.find(id(1)).unwrap();
        assert!(!registered.capabilities().allows_write());
        assert!(!registered.capabilities().allows_terminal());

        let writable =
            root(1, temp.path()).with_capabilities(slterm_settings::AgentRootCapabilities {
                write: true,
                terminal: false,
            });
        let mut registry = AgentRootRegistry::new(resolver());
        registry
            .register(writable, &CanonicalPath::new(temp.path()).unwrap())
            .unwrap();
        let registered = registry.find(id(1)).unwrap();
        assert!(registered.capabilities().allows_write());
        assert!(!registered.capabilities().allows_terminal());
    }

    /// 写能力不隐含终端：写根授权的 TerminalCwd 请求仍被拒绝。
    #[test]
    fn agent_files_write_does_not_imply_terminal() {
        use slterm_path::{PathGrant, PathOperation, PathPolicy, PathRequest, ResolvedPath};

        let temp = tempfile::tempdir().unwrap();
        let dir = CanonicalPath::new(dunce::canonicalize(temp.path()).unwrap()).unwrap();
        let request = PathRequest {
            operation: PathOperation::TerminalCwd,
            lexical_path: dir.as_path().to_path_buf(),
            root_hints: Vec::new(),
            session_grant: None,
            expected_authorization_generation: 0,
        };
        let resolved = ResolvedPath::new(dir.clone(), None);
        assert!(
            PathPolicy::authorize(
                &request,
                &resolved,
                &[PathGrant::agent_root(dir.clone(), true, false)],
                0,
            )
            .is_err(),
            "写能力不得隐含终端 cwd 授权"
        );
        assert!(
            PathPolicy::authorize(
                &request,
                &resolved,
                &[PathGrant::agent_root(dir.clone(), false, true)],
                0,
            )
            .is_ok(),
            "显式终端能力根必须可作为 cwd"
        );
    }

    /// 多根覆盖同一文件时能力并集取或；撤销可写根后按剩余根重算。
    #[test]
    fn agent_root_capability_union_service_semantics() {
        let temp = tempfile::tempdir().unwrap();
        let outer = temp.path().join("outer");
        let inner = outer.join("inner");
        std::fs::create_dir_all(&inner).unwrap();
        let file = CanonicalPath::new(inner.join("file.txt")).unwrap();
        let mut registry = AgentRootRegistry::new(resolver());
        // 先登记可写内层、后登记只读外层：并集退化为覆盖赋值时首断言即失败。
        let writable = root(2, &inner).with_capabilities(slterm_settings::AgentRootCapabilities {
            write: true,
            terminal: false,
        });
        registry
            .register(writable, &CanonicalPath::new(&inner).unwrap())
            .unwrap();
        registry
            .register(root(1, &outer), &CanonicalPath::new(&outer).unwrap())
            .unwrap();

        let union = registry.capability_union_at(&file).unwrap();
        assert!(union.write, "外层只读与内层可写必须取并集");
        assert!(!union.terminal);

        registry.remove(id(2)).unwrap();
        let union = registry.capability_union_at(&file).unwrap();
        assert!(!union.write, "撤销可写根后并集必须按剩余根重算");

        let outside = CanonicalPath::new(temp.path().join("elsewhere")).unwrap();
        assert!(registry.capability_union_at(&outside).is_none());
    }

    /// 链接别名指向同一真实目录时不形成重复根。
    #[test]
    fn agent_root_real_directory_dedup() {
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("target");
        std::fs::create_dir(&target).unwrap();
        let link = temp.path().join("alias");
        create_junction(&link, &target);

        let mut registry = AgentRootRegistry::new(resolver());
        registry
            .register(root(1, &target), &CanonicalPath::new(&target).unwrap())
            .unwrap();
        let duplicate = registry
            .register(root(2, &link), &CanonicalPath::new(&link).unwrap())
            .unwrap_err();
        assert_eq!(duplicate, AgentRootRegistryError::DuplicateRealPath(id(1)));
        assert!(registry.find(id(2)).is_none());

        let other = temp.path().join("other");
        std::fs::create_dir(&other).unwrap();
        registry
            .register(root(2, &other), &CanonicalPath::new(&other).unwrap())
            .unwrap();
        assert!(registry.find(id(2)).is_some());
    }

    /// 重复身份与删除根：删除后授权查询消失，内置根不可删除。
    #[test]
    fn agent_root_duplicate_and_revocation() {
        let temp = tempfile::tempdir().unwrap();
        let first = temp.path().join("first");
        let second = temp.path().join("second");
        std::fs::create_dir(&first).unwrap();
        std::fs::create_dir(&second).unwrap();

        let mut registry = AgentRootRegistry::new(resolver());
        registry
            .register(root(1, &first), &CanonicalPath::new(&first).unwrap())
            .unwrap();
        assert_eq!(
            registry
                .register(root(1, &second), &CanonicalPath::new(&second).unwrap())
                .unwrap_err(),
            AgentRootRegistryError::DuplicateIdentity
        );

        let removed = registry.remove(id(1)).unwrap();
        assert_eq!(removed.id(), id(1));
        assert!(registry.find(id(1)).is_none());
        assert!(registry.real_path(id(1)).is_none());
        assert_eq!(
            registry.remove(id(1)).unwrap_err(),
            AgentRootRegistryError::UnknownRoot
        );
    }

    /// 改路径换身份：旧 id 撤销、新 id 继承能力，旧 id 不再可授权。
    #[test]
    fn agent_root_path_change_resets_state_and_authorization() {
        let temp = tempfile::tempdir().unwrap();
        let first = temp.path().join("first");
        let second = temp.path().join("second");
        std::fs::create_dir(&first).unwrap();
        std::fs::create_dir(&second).unwrap();

        let mut registry = AgentRootRegistry::new(resolver());
        let original = root(1, &first).with_capabilities(slterm_settings::AgentRootCapabilities {
            write: true,
            terminal: false,
        });
        registry
            .register(original, &CanonicalPath::new(&first).unwrap())
            .unwrap();

        let moved = AgentRoot::custom(
            id(3),
            "根 1".to_owned(),
            second.to_string_lossy().into_owned(),
        )
        .unwrap()
        .with_capabilities(slterm_settings::AgentRootCapabilities {
            write: true,
            terminal: false,
        });
        let change = registry
            .change_path(id(1), moved, &CanonicalPath::new(&second).unwrap())
            .unwrap();
        assert_eq!(change.revoked, id(1));
        assert_eq!(change.registered, id(3));
        assert!(registry.find(id(1)).is_none(), "旧身份必须撤销");
        let registered = registry.find(id(3)).unwrap();
        assert!(registered.capabilities().allows_write());
        assert_eq!(
            registry.real_path(id(3)).unwrap().as_path(),
            dunce::canonicalize(&second).unwrap()
        );
    }
}
