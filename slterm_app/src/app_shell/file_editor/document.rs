//! 文档载入与条件保存(07 篇 M8.2)。
//!
//! 保存链 = 磁盘快照逐字节比对(`TextSnapshot::verify`)→ 编码回写
//! (`encode`,BOM/CRLF 往返)→ 原子替换(`atomic_file::write`)。任一步失败
//! 都不破坏磁盘:Changed 拒绝覆盖外部修改,ReadOnly 拒绝写只读/截断文档,
//! Io 原样上抛给提示横幅。
//!
//! 与上游实现的差异仅在原子写载体:本仓锁定的持久化原语是
//! `crate::atomic_file`(临时文件 + `MoveFileExW`),不再另起 `tempfile` 臂。
//! 平台为 Windows 单态,Unix 符号链接/权限用例无对象。

use std::fs;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::document_io;
use crate::text_document::{MAX_BYTES, SaveError, TextSnapshot};

#[derive(Clone, Debug)]
pub(crate) struct Document {
    snapshot: TextSnapshot,
    /// 规范化目标路径:保存时与再次解析的路径比对,防符号链接/重定向替换。
    target: PathBuf,
    /// 载入时的完整文件大小(截断判定是 `snapshot.truncated`,本字段供信息栏)。
    pub(crate) total_bytes: u64,
    /// 载入/保存后的磁盘修改时间(信息栏展示)。
    modified: Option<SystemTime>,
}

impl std::ops::Deref for Document {
    type Target = TextSnapshot;

    fn deref(&self) -> &Self::Target {
        &self.snapshot
    }
}

impl Document {
    /// 磁盘修改时间(M8.3 信息侧栏展示)。
    pub(crate) fn modified(&self) -> Option<SystemTime> {
        self.modified
    }

    /// 有界载入:上限检查先于解码;只读属性/截断/非法编码进入快照合同。
    pub(crate) fn load(path: &Path) -> std::io::Result<Self> {
        let target = path.canonicalize()?;
        let mut file = fs::File::open(&target)?;
        let metadata = file.metadata()?;
        let (bytes, truncated) = document_io::read_prefix(&mut file, metadata.len(), MAX_BYTES)?;
        Ok(Self {
            snapshot: TextSnapshot::decode_prefix(
                bytes,
                metadata.permissions().readonly(),
                truncated,
            ),
            target,
            total_bytes: metadata.len(),
            modified: metadata.modified().ok(),
        })
    }

    /// 保存前比对:路径未被替换、磁盘仍可写、字节与载入快照逐字节一致。
    fn check_current(&self, path: &Path) -> Result<(), SaveError> {
        if path.canonicalize()? != self.target {
            return Err(SaveError::Changed);
        }
        let file = fs::File::open(&self.target)?;
        let metadata = file.metadata()?;
        if metadata.permissions().readonly() {
            return Err(SaveError::ReadOnly);
        }
        if metadata.len() != self.bytes.len() as u64 {
            return Err(SaveError::Changed);
        }
        let mut current = Vec::new();
        file.take(MAX_BYTES as u64 + 1).read_to_end(&mut current)?;
        self.snapshot.verify(&current)
    }

    /// 条件保存:文本未变 = 无写盘;成功后返回以新字节为准的快照。
    pub(crate) fn save(&self, path: &Path, text: &str) -> Result<Self, SaveError> {
        if self.read_only {
            return Err(SaveError::ReadOnly);
        }
        self.check_current(path)?;
        if text == self.text {
            return Ok(self.clone());
        }
        let bytes = self.snapshot.encode(text)?;
        // 失败不触碰目标文件:临时文件与替换都在原子原语内部收口。
        crate::atomic_file::write(&self.target, &bytes)?;
        Ok(Self {
            total_bytes: bytes.len() as u64,
            snapshot: TextSnapshot::decode(bytes, false),
            modified: fs::metadata(&self.target)
                .and_then(|metadata| metadata.modified())
                .ok(),
            ..self.clone()
        })
    }
}

#[cfg(test)]
mod document_tests {
    use super::*;

    /// 保存保留 BOM/CRLF/Unicode;空串保存为仅 BOM 文件。
    #[test]
    fn saving_preserves_bom_crlf_and_unicode() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("中文.md");
        fs::write(&path, b"\xef\xbb\xbf# title\r\nold\r\n").unwrap();
        let doc = Document::load(&path).unwrap();
        assert_eq!(doc.text, "# title\nold\n");
        let saved = doc.save(&path, "# 标题\n新内容\n").unwrap();
        assert_eq!(
            fs::read(&path).unwrap(),
            "\u{feff}# 标题\r\n新内容\r\n".as_bytes()
        );
        assert_eq!(saved.text, "# 标题\n新内容\n");
        saved.save(&path, "").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"\xef\xbb\xbf");
    }

    /// 等长外部修改同样是冲突:拒绝覆盖且磁盘保持外部内容。
    #[test]
    fn concurrent_edit_is_not_overwritten_even_if_length_matches() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.rs");
        fs::write(&path, "first").unwrap();
        let doc = Document::load(&path).unwrap();
        fs::write(&path, "other").unwrap();
        assert!(matches!(doc.save(&path, "draft"), Err(SaveError::Changed)));
        assert_eq!(fs::read_to_string(&path).unwrap(), "other");
    }

    /// 文件被删除后保存是 IO 失败,不得重建目标文件。
    #[test]
    fn saving_a_deleted_file_does_not_resurrect_it() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("gone.rs");
        fs::write(&path, "first").unwrap();
        let doc = Document::load(&path).unwrap();
        fs::remove_file(&path).unwrap();
        assert!(doc.save(&path, "draft").is_err());
        assert!(!path.exists());
    }

    /// 截断/非法编码快照永不可写(只读合同在 encode 层)。
    #[test]
    fn partial_or_lossy_previews_can_never_be_saved() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("large.txt");
        fs::write(&path, vec![b'x'; MAX_BYTES + 1]).unwrap();
        let doc = Document::load(&path).unwrap();
        assert!(doc.truncated && doc.read_only);
        assert!(matches!(doc.save(&path, "short"), Err(SaveError::ReadOnly)));
        fs::write(&path, [0xff, 0xfe, 0]).unwrap();
        let doc = Document::load(&path).unwrap();
        assert!(doc.invalid_encoding && doc.read_only);
        assert!(matches!(doc.save(&path, "text"), Err(SaveError::ReadOnly)));
    }

    /// 只读属性文件载入即只读,保存被类型层拒绝。
    #[test]
    fn read_only_files_stay_read_only_after_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("locked.rs");
        fs::write(&path, "locked").unwrap();
        let mut permissions = fs::metadata(&path).unwrap().permissions();
        permissions.set_readonly(true);
        fs::set_permissions(&path, permissions).unwrap();
        let doc = Document::load(&path).unwrap();
        assert!(doc.read_only);
        assert!(matches!(
            doc.save(&path, "unlocked"),
            Err(SaveError::ReadOnly)
        ));
        // 测试夹具清理:项目仅 Windows,无 Unix 世界可写语义;clippy 的
        // Unix 告警不适用。
        #[allow(clippy::permissions_set_readonly_false)]
        {
            let mut permissions = fs::metadata(&path).unwrap().permissions();
            permissions.set_readonly(false);
            fs::set_permissions(&path, permissions).unwrap();
        }
    }
}
