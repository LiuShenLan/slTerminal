//! 文档载入与条件保存。
//!
//! 保存链 = 写前重解析与授权 → 真实句柄身份／权限／磁盘基线复核 → core 编码回写
//! (BOM／首个行尾／原始编码往返，不可表示字符明确失败)→ 原子替换。任一步失败
//! 都不破坏磁盘:Changed 拒绝覆盖外部修改,ReadOnly 拒绝写只读/截断文档,
//! Unauthorized 拒绝失权写入,Io 原样上抛给提示横幅。
//!
//! 与上游实现的差异仅在原子写载体:本仓锁定的持久化原语是
//! `crate::atomic_file`(临时文件 + `MoveFileExW`),不再另起 `tempfile` 臂。
//! 平台为 Windows 单态,Unix 符号链接/权限用例无对象。

use std::fs;
use std::io::Read as _;
use std::path::Path;
use std::sync::Arc;
use std::time::SystemTime;

use slterm_document::{
    DocumentContent, DocumentSaveReceipt, DocumentSaveRequest, EDITABLE_LIMIT_BYTES,
    NORMAL_EDIT_LIMIT_BYTES, SYNTAX_LIMIT_BYTES,
};
use slterm_path::{CanonicalPath, PathGrant, PathOperation, PathPolicy, PathRequest, ResolvedPath};

use crate::document_io;
use crate::platform::document_paths::DocumentPathResolver;
use crate::text_document::{MAX_BYTES, SaveError, TextSnapshot};

const PAGED_PREFIX_BYTES: usize = 2 * 1024 * 1024;

/// 保存时使用的最小授权集合与代次；解析器必须与打开服务同源。
#[derive(Clone)]
pub(crate) struct DocumentWriteContext {
    grants: Vec<PathGrant>,
    expected_generation: u64,
    current_generation: u64,
    resolver: Arc<dyn DocumentPathResolver>,
}

impl DocumentWriteContext {
    pub(crate) fn new(
        grants: Vec<PathGrant>,
        expected_generation: u64,
        current_generation: u64,
        resolver: Arc<dyn DocumentPathResolver>,
    ) -> Self {
        Self {
            grants,
            expected_generation,
            current_generation,
            resolver,
        }
    }
}

impl std::fmt::Debug for DocumentWriteContext {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DocumentWriteContext")
            .field("grants", &self.grants)
            .field("expected_generation", &self.expected_generation)
            .field("current_generation", &self.current_generation)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Document {
    snapshot: TextSnapshot,
    /// 打开时的规范位置与真实文件身份；保存重定向复核共用同一事实。
    location: ResolvedPath,
    /// 载入时的完整文件大小(截断判定是 `snapshot.truncated`,本字段供信息栏)。
    pub(crate) total_bytes: u64,
    /// 载入/保存后的磁盘修改时间(信息栏展示)。
    modified: Option<SystemTime>,
    /// 未挂接写授权的加载入口必须 fail-closed。
    write: Option<DocumentWriteContext>,
}

impl std::ops::Deref for Document {
    type Target = TextSnapshot;

    fn deref(&self) -> &Self::Target {
        &self.snapshot
    }
}

impl Document {
    /// 磁盘修改时间(信息侧栏展示)。
    pub(crate) fn modified(&self) -> Option<SystemTime> {
        self.modified
    }

    pub(crate) fn location(&self) -> &ResolvedPath {
        &self.location
    }

    /// 纯外部改名只替换保存与展示位置，正文、基线和历史保持不变。
    pub(crate) fn relocate(&mut self, location: ResolvedPath) {
        self.location = location;
    }

    /// 有界载入:上限检查先于解码;只读属性/截断/非法编码进入快照合同。
    pub(crate) fn load(path: &Path) -> std::io::Result<Self> {
        Self::load_with_encoding(path, None)
    }

    /// 显式编码重开：BOM 仍优先，未知编码以可重试类别返回。
    pub(crate) fn load_with_encoding(
        path: &Path,
        encoding: Option<slterm_document::DocumentEncoding>,
    ) -> std::io::Result<Self> {
        Self::load_with_read(path, encoding, document_io::read_prefix)
    }

    /// 读取端口只用于故障注入；正文、身份与长度仍来自同一真实文件句柄。
    fn load_with_read(
        path: &Path,
        encoding: Option<slterm_document::DocumentEncoding>,
        read: impl FnOnce(&mut fs::File, u64, usize) -> std::io::Result<(Vec<u8>, bool)>,
    ) -> std::io::Result<Self> {
        let target = dunce::canonicalize(path)?;
        let canonical = CanonicalPath::new(target.clone())
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidInput, error))?;
        let mut file = fs::File::open(&target)?;
        let metadata = file.metadata()?;
        let identity = crate::atomic_file::file_identity(&file)?;
        let location = ResolvedPath::new(canonical, Some(identity));
        let limit = if metadata.len() > EDITABLE_LIMIT_BYTES {
            PAGED_PREFIX_BYTES
        } else {
            MAX_BYTES
        };
        let (bytes, truncated) = read(&mut file, metadata.len(), limit)?;
        if file.metadata()?.len() != metadata.len() {
            return Err(std::io::Error::other("文件在读取期间改变了长度"));
        }
        let content = DocumentContent::decode(bytes, metadata.len(), encoding);
        if let DocumentContent::Unavailable(reason) = content {
            // 编码歧义可由用户显式选择编码后重试；二进制内容没有这种恢复路径。
            let kind = match reason {
                slterm_document::ContentUnavailable::Binary => std::io::ErrorKind::InvalidData,
                slterm_document::ContentUnavailable::UnknownEncoding
                | slterm_document::ContentUnavailable::InvalidEncoding => {
                    std::io::ErrorKind::Unsupported
                }
            };
            return Err(std::io::Error::new(kind, "内容不是可建立的文本编辑基线"));
        }
        Ok(Self {
            snapshot: TextSnapshot::from_content(
                content,
                metadata.permissions().readonly(),
                truncated,
            ),
            location,
            total_bytes: metadata.len(),
            modified: metadata.modified().ok(),
            write: None,
        })
    }

    /// 启动恢复只复制到既有编辑器视图；core Session 仍是保存与关闭的权威。
    pub(crate) fn from_recovered(
        session: &slterm_document::DocumentSession,
        write: Option<DocumentWriteContext>,
    ) -> Option<Self> {
        let text = session.text()?.to_string();
        let text_bytes = text.len() as u64;
        Some(Self {
            snapshot: TextSnapshot {
                text,
                bytes: session.disk_baseline().clone(),
                encoding: session.encoding().clone(),
                completion_enabled: text_bytes <= NORMAL_EDIT_LIMIT_BYTES,
                syntax_enabled: text_bytes <= SYNTAX_LIMIT_BYTES,
                truncated: false,
                invalid_encoding: false,
                read_only: session.is_read_only(),
            },
            location: session.location().clone(),
            total_bytes: session.disk_baseline().len() as u64,
            modified: None,
            write,
        })
    }

    pub(crate) fn with_write_context(mut self, write: DocumentWriteContext) -> Self {
        self.write = Some(write);
        self
    }

    /// 写前重新解析并授权，再比较真实句柄身份、权限与完整磁盘基线。
    fn authorize_current(&self) -> Result<ResolvedPath, SaveError> {
        let write = self.write.as_ref().ok_or(SaveError::Unauthorized)?;
        let lexical = self.location.canonical_path();
        let resolved = write.resolver.resolve(lexical)?;
        let request = PathRequest {
            operation: PathOperation::Write,
            lexical_path: lexical.as_path().to_path_buf(),
            root_hints: Vec::new(),
            session_grant: None,
            expected_authorization_generation: write.expected_generation,
        };
        let authorization =
            PathPolicy::authorize(&request, &resolved, &write.grants, write.current_generation)
                .map_err(|_| SaveError::Unauthorized)?;
        let resolved = authorization.resolved_path();
        if resolved != &self.location {
            return Err(SaveError::Changed);
        }
        Ok(resolved.clone())
    }

    /// 覆盖保存只证明目标仍是原文件且仍可写；内容差异由用户决定。
    fn check_writable_identity(&self, resolved: &ResolvedPath) -> Result<(), SaveError> {
        let file = fs::File::open(resolved.canonical_path().as_path())?;
        if Some(crate::atomic_file::file_identity(&file)?) != self.location.file_identity() {
            return Err(SaveError::Changed);
        }
        if file.metadata()?.permissions().readonly() {
            return Err(SaveError::ReadOnly);
        }
        Ok(())
    }

    /// 保存前比对:真实句柄身份未变、磁盘仍可写、字节与载入快照逐字节一致。
    fn check_current(&self, resolved: &ResolvedPath) -> Result<(), SaveError> {
        let file = fs::File::open(resolved.canonical_path().as_path())?;
        if Some(crate::atomic_file::file_identity(&file)?) != self.location.file_identity() {
            return Err(SaveError::Changed);
        }
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
    pub(crate) fn save(&self, text: &str) -> Result<Self, SaveError> {
        self.save_with(text, false)
    }

    /// 用户在冲突决定面明确选择覆盖：跳过磁盘字节基线比较，仍复核真实身份与权限。
    pub(crate) fn save_overwrite(&self, text: &str) -> Result<Self, SaveError> {
        self.save_with(text, true)
    }

    fn save_with(&self, text: &str, overwrite: bool) -> Result<Self, SaveError> {
        if self.read_only {
            return Err(SaveError::ReadOnly);
        }
        let resolved = self.authorize_current()?;
        if overwrite {
            self.check_writable_identity(&resolved)?;
        } else {
            self.check_current(&resolved)?;
            if text == self.text {
                return Ok(self.clone());
            }
        }
        let bytes = self.snapshot.encode(text)?;
        // 失败不触碰目标文件:临时文件与替换都在原子原语内部收口。
        let identity =
            crate::atomic_file::write_with_identity(resolved.canonical_path().as_path(), &bytes)?;
        let modified = fs::metadata(resolved.canonical_path().as_path())
            .and_then(|metadata| metadata.modified())
            .ok();
        let location = ResolvedPath::new(resolved.canonical_path().clone(), Some(identity));
        let total_bytes = bytes.len() as u64;
        let content = DocumentContent::decode(bytes, total_bytes, Some(self.encoding.encoding));
        Ok(Self {
            total_bytes,
            snapshot: TextSnapshot::from_content(content, false, false),
            location,
            modified,
            write: self.write.clone(),
        })
    }

    fn from_save_snapshot(
        request: &DocumentSaveRequest,
        write: DocumentWriteContext,
    ) -> Result<Self, SaveError> {
        let baseline = request.disk_baseline().to_vec();
        let content = DocumentContent::decode(
            baseline,
            request.disk_baseline().len() as u64,
            Some(request.encoding().encoding),
        );
        let snapshot = match content {
            DocumentContent::Editable(decoded) | DocumentContent::Paged(decoded) => {
                TextSnapshot::from_content(DocumentContent::Editable(decoded), false, false)
            }
            DocumentContent::Unavailable(_) => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "保存请求的磁盘基线不可解码",
                )
                .into());
            }
        };
        Ok(Self {
            snapshot,
            location: request.location().clone(),
            total_bytes: request.disk_baseline().len() as u64,
            modified: None,
            write: Some(write),
        })
    }

    /// 冲突决定面的覆盖保存：同一原子写链，跳过磁盘字节基线比较。
    pub(crate) fn save_request_overwrite(
        request: DocumentSaveRequest,
        write: DocumentWriteContext,
    ) -> Result<DocumentSaveReceipt, SaveError> {
        let document = Self::from_save_snapshot(&request, write)?;
        let saved = document.save_overwrite(&request.text().to_string())?;
        Ok(DocumentSaveReceipt::new(
            request,
            saved.bytes.clone(),
            saved.location.file_identity(),
        ))
    }

    /// 从 core 捕获快照执行同一条件保存链，并返回可按代次对账的回执。
    pub(crate) fn save_request(
        request: DocumentSaveRequest,
        write: DocumentWriteContext,
    ) -> Result<DocumentSaveReceipt, SaveError> {
        let document = Self::from_save_snapshot(&request, write)?;
        let saved = document.save(&request.text().to_string())?;
        let disk_baseline = saved.bytes.clone();
        let file_identity = saved.location.file_identity();
        Ok(DocumentSaveReceipt::new(
            request,
            disk_baseline,
            file_identity,
        ))
    }

    /// 已授权且未占用的另存为目标；源位置不执行任何写入。
    pub(crate) fn save_as_request(
        request: DocumentSaveRequest,
        target: ResolvedPath,
        write: DocumentWriteContext,
    ) -> Result<DocumentSaveReceipt, SaveError> {
        let document = Self::from_save_snapshot(&request, write)?;
        let bytes = document.snapshot.encode(&request.text().to_string())?;
        let rechecked = document
            .write
            .as_ref()
            .expect("另存为快照必须携带解析器")
            .resolver
            .resolve_write_target(target.canonical_path())?;
        if rechecked != target {
            return Err(SaveError::Changed);
        }
        let identity =
            crate::atomic_file::write_with_identity(target.canonical_path().as_path(), &bytes)?;
        let location = ResolvedPath::new(target.canonical_path().clone(), Some(identity));
        Ok(DocumentSaveReceipt::for_save_as(
            request,
            Arc::from(bytes),
            location,
        ))
    }
}

#[cfg(test)]
mod document_tests {
    use super::*;

    fn load_writable(path: &Path) -> Document {
        let root =
            CanonicalPath::new(dunce::canonicalize(path.parent().unwrap()).unwrap()).unwrap();
        Document::load(path)
            .unwrap()
            .with_write_context(DocumentWriteContext::new(
                vec![PathGrant::project_root(root)],
                0,
                0,
                Arc::new(crate::platform::document_paths::SystemDocumentPathResolver),
            ))
    }

    #[test]
    fn load_rejects_growth_before_committing_a_snapshot() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("growing.txt");
        fs::write(&path, vec![b'x'; EDITABLE_LIMIT_BYTES as usize]).unwrap();

        let result = Document::load_with_read(&path, None, |file, declared, limit| {
            let read = document_io::read_prefix(file, declared, limit)?;
            let mut append = fs::OpenOptions::new().append(true).open(&path)?;
            std::io::Write::write_all(&mut append, b"y")?;
            Ok(read)
        });

        assert!(result.is_err(), "读取期间增长不得提交旧长度快照");
    }

    /// 保存保留 BOM/CRLF/Unicode;空串保存为仅 BOM 文件。
    #[test]
    fn saving_preserves_bom_crlf_and_unicode() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("中文.md");
        fs::write(&path, b"\xef\xbb\xbf# title\r\nold\r\n").unwrap();
        let doc = load_writable(&path);
        assert_eq!(doc.text, "# title\nold\n");
        let saved = doc.save("# 标题\n新内容\n").unwrap();
        assert_eq!(
            fs::read(&path).unwrap(),
            "\u{feff}# 标题\r\n新内容\r\n".as_bytes()
        );
        assert_eq!(saved.text, "# 标题\n新内容\n");
        saved.save("").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"\xef\xbb\xbf");
    }

    /// 冲突决定面的覆盖写入接受外部内容变化；替换后的新文件身份仍拒绝。
    #[test]
    fn explicit_overwrite_accepts_content_change_but_not_identity_replacement() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("draft.txt");
        fs::write(&path, "first").unwrap();
        let doc = load_writable(&path);
        fs::write(&path, "external").unwrap();
        assert!(matches!(doc.save("local"), Err(SaveError::Changed)));
        let saved = doc.save_overwrite("local").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "local");
        assert_eq!(saved.text, "local");

        fs::remove_file(&path).unwrap();
        fs::write(&path, "replaced").unwrap();
        assert!(
            matches!(saved.save_overwrite("again"), Err(SaveError::Changed)),
            "文件身份已被替换时不得覆盖新对象"
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), "replaced");
    }

    /// 等长外部修改同样是冲突:拒绝覆盖且磁盘保持外部内容。
    #[test]
    fn concurrent_edit_is_not_overwritten_even_if_length_matches() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.rs");
        fs::write(&path, "first").unwrap();
        let doc = load_writable(&path);
        fs::write(&path, "other").unwrap();
        assert!(matches!(doc.save("draft"), Err(SaveError::Changed)));
        assert_eq!(fs::read_to_string(&path).unwrap(), "other");
    }

    /// 文件被删除后保存是 IO 失败,不得重建目标文件。
    #[test]
    fn saving_a_deleted_file_does_not_resurrect_it() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("gone.rs");
        fs::write(&path, "first").unwrap();
        let doc = load_writable(&path);
        fs::remove_file(&path).unwrap();
        assert!(doc.save("draft").is_err());
        assert!(!path.exists());
    }

    /// 截断前缀永不可写；未知编码与 NUL 不得建立可保存正文基线。
    #[test]
    fn partial_or_unavailable_documents_cannot_be_saved() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("large.txt");
        fs::write(&path, vec![b'x'; MAX_BYTES + 1]).unwrap();
        let doc = load_writable(&path);
        assert!(doc.truncated && doc.read_only);
        assert!(matches!(doc.save("short"), Err(SaveError::ReadOnly)));
        for bytes in [b"\xff".as_slice(), b"text\0binary".as_slice()] {
            fs::write(&path, bytes).unwrap();
            assert!(Document::load(&path).is_err());
        }
    }

    #[test]
    fn utf16_bom_files_round_trip_through_real_document_io() {
        for little_endian in [true, false] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("utf16.txt");
            let mut bytes = if little_endian {
                b"\xff\xfe".to_vec()
            } else {
                b"\xfe\xff".to_vec()
            };
            for unit in "标题😀\r\n".encode_utf16() {
                let encoded = if little_endian {
                    unit.to_le_bytes()
                } else {
                    unit.to_be_bytes()
                };
                bytes.extend_from_slice(&encoded);
            }
            fs::write(&path, bytes).unwrap();

            let doc = load_writable(&path);
            assert_eq!(doc.text, "标题😀\n");
            assert!(!doc.read_only);
            let saved = doc.save("新😀\n").unwrap();
            assert_eq!(saved.text, "新😀\n");
            let reread = fs::read(&path).unwrap();
            let mut expected = if little_endian {
                b"\xff\xfe".to_vec()
            } else {
                b"\xfe\xff".to_vec()
            };
            for unit in "新😀\r\n".encode_utf16() {
                let encoded = if little_endian {
                    unit.to_le_bytes()
                } else {
                    unit.to_be_bytes()
                };
                expected.extend_from_slice(&encoded);
            }
            assert_eq!(reread, expected);
        }
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
        let doc = load_writable(&path);
        assert!(doc.read_only);
        assert!(matches!(doc.save("unlocked"), Err(SaveError::ReadOnly)));
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
