//! 项目根的真实路径解析端口；系统 I/O 只由调用方放到后台执行。

use std::io;

use slterm_path::CanonicalPath;

pub(crate) trait ProjectPathResolver: Send + Sync {
    fn resolve(&self, path: &CanonicalPath) -> io::Result<CanonicalPath>;
}

pub(crate) struct SystemProjectPathResolver;

impl ProjectPathResolver for SystemProjectPathResolver {
    fn resolve(&self, path: &CanonicalPath) -> io::Result<CanonicalPath> {
        let resolved = dunce::canonicalize(path.as_path())?;
        if !resolved.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::NotADirectory,
                "项目根必须是既有目录",
            ));
        }
        CanonicalPath::new(resolved)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "真实项目根不是规范路径"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_resolver_requires_an_existing_directory() {
        let temp = tempfile::tempdir().unwrap();
        let root = CanonicalPath::new(temp.path()).unwrap();
        let resolved = SystemProjectPathResolver.resolve(&root).unwrap();
        assert_eq!(
            resolved,
            CanonicalPath::new(dunce::canonicalize(temp.path()).unwrap()).unwrap()
        );
        let alias = CanonicalPath::new(temp.path().join(".")).unwrap();
        assert_eq!(SystemProjectPathResolver.resolve(&alias).unwrap(), resolved);

        let file = temp.path().join("not-a-directory");
        std::fs::write(&file, "test").unwrap();
        let file = CanonicalPath::new(file).unwrap();
        assert_eq!(
            SystemProjectPathResolver.resolve(&file).unwrap_err().kind(),
            io::ErrorKind::NotADirectory
        );
    }

    #[test]
    fn system_resolver_normalizes_directory_junction() {
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

        let target = SystemProjectPathResolver
            .resolve(&CanonicalPath::new(target).unwrap())
            .unwrap();
        let alias = SystemProjectPathResolver
            .resolve(&CanonicalPath::new(alias).unwrap())
            .unwrap();
        assert_eq!(alias, target);
    }
}
