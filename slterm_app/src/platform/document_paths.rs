//! Windows 文件打开解析：规范位置与读取句柄身份来自同一真实目标。

use std::io;

use slterm_path::{CanonicalPath, ResolvedPath};

pub(crate) trait DocumentPathResolver: Send + Sync {
    fn resolve(&self, lexical: &CanonicalPath) -> io::Result<ResolvedPath>;
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
}
