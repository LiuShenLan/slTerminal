//! Windows 文件打开解析：规范位置与读取句柄身份来自同一真实目标。

use std::io;

use slterm_path::{CanonicalPath, ResolvedPath};

pub(crate) trait DocumentPathResolver: Send + Sync {
    fn resolve(&self, lexical: &CanonicalPath) -> io::Result<ResolvedPath>;

    fn resolve_write_target(&self, lexical: &CanonicalPath) -> io::Result<ResolvedPath> {
        self.resolve(lexical)
    }
}

pub(crate) struct SystemDocumentPathResolver;

impl DocumentPathResolver for SystemDocumentPathResolver {
    fn resolve(&self, lexical: &CanonicalPath) -> io::Result<ResolvedPath> {
        let canonical = dunce::canonicalize(lexical.as_path())?;
        let file = std::fs::File::open(&canonical)?;
        let identity = crate::atomic_file::file_identity(&file)?;
        let canonical = CanonicalPath::new(canonical)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
        Ok(ResolvedPath::new(canonical, Some(identity)))
    }

    fn resolve_write_target(&self, lexical: &CanonicalPath) -> io::Result<ResolvedPath> {
        match dunce::canonicalize(lexical.as_path()) {
            Ok(_) => self.resolve(lexical),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                // 已存在但无法 canonicalize 的最终组件可能是悬空链接；不得替链接落盘。
                match std::fs::symlink_metadata(lexical.as_path()) {
                    Ok(_) => {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "另存为目标是不可解析的既有对象",
                        ));
                    }
                    Err(metadata_error) if metadata_error.kind() == io::ErrorKind::NotFound => {}
                    Err(metadata_error) => return Err(metadata_error),
                }
                let name = lexical.as_path().file_name().ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "另存为目标缺少文件名")
                })?;
                let parent = lexical.as_path().parent().ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "另存为目标缺少父目录")
                })?;
                let parent = dunce::canonicalize(parent)?;
                let target = CanonicalPath::new(parent.join(name))
                    .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
                Ok(ResolvedPath::new(target, None))
            }
            Err(error) => Err(error),
        }
    }
}
