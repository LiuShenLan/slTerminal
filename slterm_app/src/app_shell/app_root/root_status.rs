//! 启动恢复期的项目根真实状态探测。
//!
//! 持久根只是词法事实；真实目标缺失、不可访问或经 junction 归一后落到
//! 另一位置时，必须在任何 PTY／正文恢复前更新根状态或根关联。探测整体
//! 在候选快照上完成，任一真实重复根都不留下部分改态。

use std::io;

use slterm_workspace::{ProjectRegistryError, ProjectRootStatus, ProjectSnapshot};

use crate::platform::project_paths::ProjectPathResolver;

/// 按真实目标更新每个项目的根关联或根状态。
///
/// - 真实目标与持久根一致：就绪。
/// - 真实目标经别名归一为另一规范路径：改绑该规范根并恢复就绪；
///   与其他项目真实根重复时整体失败。
/// - 根不存在（`NotFound`／`NotADirectory`）：需要重定位。
/// - 其余解析失败（权限等）：不可访问；仍可重命名、重定位与移除。
pub(super) fn probe_project_roots(
    snapshot: &mut ProjectSnapshot,
    resolver: &dyn ProjectPathResolver,
) -> Result<(), ProjectRegistryError> {
    let mut candidate = snapshot.clone();
    let projects = candidate
        .registry()
        .projects()
        .iter()
        .map(|project| (project.id(), project.root().clone()))
        .collect::<Vec<_>>();
    for (id, root) in projects {
        let status = match resolver.resolve(&root) {
            Ok(resolved) if resolved == root => ProjectRootStatus::Ready,
            Ok(resolved) => {
                // 别名归一后可能与其他项目真实根重复；core 统一拒绝。
                candidate.registry_mut().relocate_project(id, resolved)?;
                continue;
            }
            Err(error) if root_missing(&error) => ProjectRootStatus::RelocationRequired,
            Err(_) => ProjectRootStatus::Inaccessible,
        };
        candidate.registry_mut().set_root_status(id, status)?;
    }
    *snapshot = candidate;
    Ok(())
}

fn root_missing(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
    )
}

/// 探测接口只接受真实路径解析端口；规范根用于归一后的词法查重。
pub(super) fn probe_with_system_resolver(
    snapshot: &mut ProjectSnapshot,
) -> Result<(), ProjectRegistryError> {
    probe_project_roots(
        snapshot,
        &crate::platform::project_paths::SystemProjectPathResolver,
    )
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::io;
    use std::path::PathBuf;
    use std::str::FromStr;

    use slterm_path::CanonicalPath;
    use slterm_workspace::{ProjectId, ProjectRootStatus, ProjectSnapshot};

    use super::probe_project_roots;
    use crate::platform::project_paths::ProjectPathResolver;

    #[derive(Clone)]
    enum FakeOutcome {
        Ok(CanonicalPath),
        Kind(io::ErrorKind),
    }

    struct FakeResolver {
        results: BTreeMap<PathBuf, FakeOutcome>,
    }

    impl ProjectPathResolver for FakeResolver {
        fn resolve(&self, path: &CanonicalPath) -> io::Result<CanonicalPath> {
            match self.results.get(path.as_path()).cloned() {
                Some(FakeOutcome::Ok(resolved)) => Ok(resolved),
                Some(FakeOutcome::Kind(kind)) => Err(io::Error::from(kind)),
                None => Err(io::Error::from(io::ErrorKind::NotFound)),
            }
        }
    }

    fn project(text: &str) -> ProjectId {
        ProjectId::from_str(text).unwrap()
    }

    fn path(text: &str) -> CanonicalPath {
        CanonicalPath::new(text).unwrap()
    }

    fn snapshot(roots: &[(&str, &str)]) -> ProjectSnapshot {
        let mut snapshot = ProjectSnapshot::default();
        for (id, root) in roots {
            snapshot
                .registry_mut()
                .add_project(project(id), path(root))
                .unwrap();
        }
        snapshot
    }

    fn resolver(entries: impl IntoIterator<Item = (&'static str, FakeOutcome)>) -> FakeResolver {
        FakeResolver {
            results: entries
                .into_iter()
                .map(|(root, result)| (PathBuf::from(root), result))
                .collect(),
        }
    }

    #[test]
    fn missing_root_requires_relocation() {
        let mut snapshot = snapshot(&[("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b", "D:/gone")]);
        let resolver = resolver([("D:/gone", FakeOutcome::Kind(io::ErrorKind::NotFound))]);

        probe_project_roots(&mut snapshot, &resolver).unwrap();

        assert_eq!(
            snapshot.registry().projects()[0].root_status(),
            ProjectRootStatus::RelocationRequired
        );
    }

    #[test]
    fn permission_failure_marks_inaccessible() {
        let mut snapshot = snapshot(&[("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b", "D:/locked")]);
        let resolver = resolver([(
            "D:/locked",
            FakeOutcome::Kind(io::ErrorKind::PermissionDenied),
        )]);

        probe_project_roots(&mut snapshot, &resolver).unwrap();

        assert_eq!(
            snapshot.registry().projects()[0].root_status(),
            ProjectRootStatus::Inaccessible
        );
    }

    #[test]
    fn alias_rebinds_to_the_real_root() {
        let mut snapshot = snapshot(&[("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b", "D:/alias")]);
        let resolver = resolver([("D:/alias", FakeOutcome::Ok(path("D:/real")))]);

        probe_project_roots(&mut snapshot, &resolver).unwrap();

        assert_eq!(snapshot.registry().projects()[0].root(), &path("D:/real"));
        assert_eq!(
            snapshot.registry().projects()[0].root_status(),
            ProjectRootStatus::Ready
        );
    }

    #[test]
    fn duplicate_real_root_aborts_without_partial_mutation() {
        let mut snapshot = snapshot(&[
            ("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b", "D:/a"),
            ("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2c", "D:/b"),
        ]);
        let original = snapshot.clone();
        let resolver = resolver([
            ("D:/a", FakeOutcome::Ok(path("D:/real"))),
            ("D:/b", FakeOutcome::Ok(path("D:/real"))),
        ]);

        assert!(probe_project_roots(&mut snapshot, &resolver).is_err());

        assert_eq!(snapshot, original, "真实重复根必须整体失败且不改态");
    }

    #[cfg(windows)]
    #[test]
    fn junction_alias_is_rebound_by_the_system_resolver() {
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("target");
        let alias = temp.path().join("alias");
        std::fs::create_dir(&target).unwrap();
        let status = std::process::Command::new("cmd.exe")
            .arg("/C")
            .arg("mklink")
            .arg("/J")
            .arg(&alias)
            .arg(&target)
            .status()
            .unwrap();
        assert!(status.success(), "Windows 必须能创建临时目录 junction");
        let mut snapshot = ProjectSnapshot::default();
        snapshot
            .registry_mut()
            .add_project(
                project("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b"),
                path(alias.to_str().unwrap()),
            )
            .unwrap();

        super::probe_with_system_resolver(&mut snapshot).unwrap();

        let expected = path(dunce::canonicalize(&target).unwrap().to_str().unwrap());
        assert_eq!(snapshot.registry().projects()[0].root(), &expected);
        assert_eq!(
            snapshot.registry().projects()[0].root_status(),
            ProjectRootStatus::Ready
        );
    }
}
