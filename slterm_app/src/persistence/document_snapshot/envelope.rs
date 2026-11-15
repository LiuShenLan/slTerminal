//! 自描述快照信封：文件头、规范元数据与 UTF-8 正文的严格校验。

use std::fmt;
use std::io;
use std::str::FromStr as _;

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use slterm_document::{
    DocumentEncoding, DocumentId, DocumentSession, LineEnding, TextEncoding, TextStateId,
};
use slterm_path::{CanonicalPath, FileIdentity, ResolvedPath};

pub(crate) const SNAPSHOT_BODY_LIMIT_BYTES: u64 = 64 * 1024 * 1024;
const METADATA_LIMIT_BYTES: usize = 1024 * 1024;
const MAGIC: &[u8; 8] = b"SLTSNAP1";
const FORMAT_VERSION: u32 = 1;
const HEADER_LEN: usize = 60;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SnapshotCaptureError {
    NotDirty,
    ReadOnly,
    BodyTooLarge,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SnapshotStoreError {
    Io(io::ErrorKind),
    Corrupt,
    QuotaExceeded,
    CloseNotCommitted,
}

impl fmt::Display for SnapshotStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Io(_) => "快照文件读写失败",
            Self::Corrupt => "快照文件损坏或身份不一致",
            Self::QuotaExceeded => "快照配额不足，编辑现场仍保留在内存中",
            Self::CloseNotCommitted => "关闭意图尚未提交，快照必须保留",
        })
    }
}

impl std::error::Error for SnapshotStoreError {}

impl From<io::Error> for SnapshotStoreError {
    fn from(error: io::Error) -> Self {
        Self::Io(error.kind())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SnapshotMetadata {
    document_id: DocumentId,
    location: ResolvedPath,
    encoding: TextEncoding,
    disk_baseline_hash: [u8; 32],
    saved_state: TextStateId,
    current_state: TextStateId,
    body_hash: [u8; 32],
    run_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DocumentSnapshot {
    metadata: SnapshotMetadata,
    text: String,
}

impl DocumentSnapshot {
    /// 只捕获脏的可编辑正文；Paged、只读与超限内容没有快照入口。
    pub(crate) fn capture(
        session: &DocumentSession,
        run_id: impl Into<String>,
    ) -> Result<Self, SnapshotCaptureError> {
        if session.is_read_only() {
            return Err(SnapshotCaptureError::ReadOnly);
        }
        if !session.is_dirty() {
            return Err(SnapshotCaptureError::NotDirty);
        }
        let text = session
            .text()
            .ok_or(SnapshotCaptureError::ReadOnly)?
            .to_string();
        if text.len() as u64 > SNAPSHOT_BODY_LIMIT_BYTES {
            return Err(SnapshotCaptureError::BodyTooLarge);
        }
        let run_id = run_id.into();
        Ok(Self {
            metadata: SnapshotMetadata {
                document_id: session.id(),
                location: session.location().clone(),
                encoding: session.encoding().clone(),
                disk_baseline_hash: Sha256::digest(session.disk_baseline()).into(),
                saved_state: session.saved_state(),
                current_state: session.current_state(),
                body_hash: Sha256::digest(text.as_bytes()).into(),
                run_id,
            },
            text,
        })
    }

    pub(crate) const fn document_id(&self) -> DocumentId {
        self.metadata.document_id
    }

    pub(crate) fn text(&self) -> &str {
        &self.text
    }

    pub(crate) fn body_len(&self) -> usize {
        self.text.len()
    }

    pub(crate) fn encode(&self) -> Result<Vec<u8>, SnapshotStoreError> {
        let metadata = encode_metadata(&self.metadata)?;
        if metadata.len() > METADATA_LIMIT_BYTES
            || self.text.len() as u64 > SNAPSHOT_BODY_LIMIT_BYTES
        {
            return Err(SnapshotStoreError::Corrupt);
        }
        let metadata_len =
            u64::try_from(metadata.len()).map_err(|_| SnapshotStoreError::Corrupt)?;
        let body_len = u64::try_from(self.text.len()).map_err(|_| SnapshotStoreError::Corrupt)?;
        let mut encoded = Vec::with_capacity(HEADER_LEN + metadata.len() + self.text.len());
        encoded.extend_from_slice(MAGIC);
        encoded.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
        encoded.extend_from_slice(&metadata_len.to_le_bytes());
        encoded.extend_from_slice(&body_len.to_le_bytes());
        let mut hasher = Sha256::new();
        hasher.update(&metadata);
        hasher.update(self.text.as_bytes());
        encoded.extend_from_slice(&hasher.finalize());
        encoded.extend_from_slice(&metadata);
        encoded.extend_from_slice(self.text.as_bytes());
        Ok(encoded)
    }

    pub(crate) fn decode(encoded: &[u8]) -> Result<Self, SnapshotStoreError> {
        if encoded.len() < HEADER_LEN || &encoded[..8] != MAGIC {
            return Err(SnapshotStoreError::Corrupt);
        }
        if u32::from_le_bytes(encoded[8..12].try_into().unwrap()) != FORMAT_VERSION {
            return Err(SnapshotStoreError::Corrupt);
        }
        let metadata_len = usize::try_from(u64::from_le_bytes(encoded[12..20].try_into().unwrap()))
            .map_err(|_| SnapshotStoreError::Corrupt)?;
        let body_len = usize::try_from(u64::from_le_bytes(encoded[20..28].try_into().unwrap()))
            .map_err(|_| SnapshotStoreError::Corrupt)?;
        if metadata_len > METADATA_LIMIT_BYTES || body_len as u64 > SNAPSHOT_BODY_LIMIT_BYTES {
            return Err(SnapshotStoreError::Corrupt);
        }
        let body_start = HEADER_LEN
            .checked_add(metadata_len)
            .ok_or(SnapshotStoreError::Corrupt)?;
        let end = body_start
            .checked_add(body_len)
            .ok_or(SnapshotStoreError::Corrupt)?;
        if end != encoded.len() {
            return Err(SnapshotStoreError::Corrupt);
        }
        let metadata_bytes = &encoded[HEADER_LEN..body_start];
        let body = &encoded[body_start..end];
        let actual_hash = Sha256::new()
            .chain_update(metadata_bytes)
            .chain_update(body)
            .finalize();
        if actual_hash.as_slice() != &encoded[28..HEADER_LEN] {
            return Err(SnapshotStoreError::Corrupt);
        }
        let metadata = decode_metadata(metadata_bytes)?;
        let text = String::from_utf8(body.to_vec()).map_err(|_| SnapshotStoreError::Corrupt)?;
        if metadata.saved_state == metadata.current_state
            || metadata.body_hash != <[u8; 32]>::from(Sha256::digest(body))
        {
            return Err(SnapshotStoreError::Corrupt);
        }
        Ok(Self { metadata, text })
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct MetadataDto {
    version: u32,
    document_id: String,
    path: String,
    identity: Option<IdentityDto>,
    encoding: EncodingDto,
    bom: bool,
    line_ending: LineEndingDto,
    trailing_newline: bool,
    disk_baseline_hash: String,
    saved_state: String,
    current_state: String,
    body_hash: String,
    run_id: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct IdentityDto {
    volume: u64,
    file: String,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum EncodingDto {
    Utf8,
    Utf16Le,
    Utf16Be,
    Gbk,
    Gb18030,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum LineEndingDto {
    Lf,
    Crlf,
}

fn encode_metadata(metadata: &SnapshotMetadata) -> Result<Vec<u8>, SnapshotStoreError> {
    if metadata.run_id.is_empty() || metadata.saved_state == metadata.current_state {
        return Err(SnapshotStoreError::Corrupt);
    }
    let path = metadata
        .location
        .canonical_path()
        .as_path()
        .to_str()
        .ok_or(SnapshotStoreError::Corrupt)?;
    let dto = MetadataDto {
        version: FORMAT_VERSION,
        document_id: metadata.document_id.to_string(),
        path: path.to_owned(),
        identity: metadata
            .location
            .file_identity()
            .map(|identity| IdentityDto {
                volume: identity.volume,
                file: identity.file.to_string(),
            }),
        encoding: match metadata.encoding.encoding {
            DocumentEncoding::Utf8 => EncodingDto::Utf8,
            DocumentEncoding::Utf16Le => EncodingDto::Utf16Le,
            DocumentEncoding::Utf16Be => EncodingDto::Utf16Be,
            DocumentEncoding::Gbk => EncodingDto::Gbk,
            DocumentEncoding::Gb18030 => EncodingDto::Gb18030,
        },
        bom: metadata.encoding.bom,
        line_ending: match metadata.encoding.line_ending {
            LineEnding::Lf => LineEndingDto::Lf,
            LineEnding::Crlf => LineEndingDto::Crlf,
        },
        trailing_newline: metadata.encoding.trailing_newline,
        disk_baseline_hash: hex(&metadata.disk_baseline_hash),
        saved_state: metadata.saved_state.to_string(),
        current_state: metadata.current_state.to_string(),
        body_hash: hex(&metadata.body_hash),
        run_id: metadata.run_id.clone(),
    };
    serde_json::to_vec(&dto).map_err(|_| SnapshotStoreError::Corrupt)
}

fn decode_metadata(bytes: &[u8]) -> Result<SnapshotMetadata, SnapshotStoreError> {
    let dto: MetadataDto =
        serde_json::from_slice(bytes).map_err(|_| SnapshotStoreError::Corrupt)?;
    if dto.version != FORMAT_VERSION || dto.run_id.is_empty() {
        return Err(SnapshotStoreError::Corrupt);
    }
    let identity = dto
        .identity
        .map(|identity| {
            let file = identity
                .file
                .parse::<u128>()
                .map_err(|_| SnapshotStoreError::Corrupt)?;
            if identity.file != file.to_string() {
                return Err(SnapshotStoreError::Corrupt);
            }
            Ok(FileIdentity {
                volume: identity.volume,
                file,
            })
        })
        .transpose()?;
    Ok(SnapshotMetadata {
        document_id: DocumentId::from_str(&dto.document_id)
            .map_err(|_| SnapshotStoreError::Corrupt)?,
        location: ResolvedPath::new(
            CanonicalPath::new(dto.path).map_err(|_| SnapshotStoreError::Corrupt)?,
            identity,
        ),
        encoding: TextEncoding {
            encoding: match dto.encoding {
                EncodingDto::Utf8 => DocumentEncoding::Utf8,
                EncodingDto::Utf16Le => DocumentEncoding::Utf16Le,
                EncodingDto::Utf16Be => DocumentEncoding::Utf16Be,
                EncodingDto::Gbk => DocumentEncoding::Gbk,
                EncodingDto::Gb18030 => DocumentEncoding::Gb18030,
            },
            bom: dto.bom,
            line_ending: match dto.line_ending {
                LineEndingDto::Lf => LineEnding::Lf,
                LineEndingDto::Crlf => LineEnding::Crlf,
            },
            trailing_newline: dto.trailing_newline,
        },
        disk_baseline_hash: unhex(&dto.disk_baseline_hash)?,
        saved_state: TextStateId::from_str(&dto.saved_state)
            .map_err(|_| SnapshotStoreError::Corrupt)?,
        current_state: TextStateId::from_str(&dto.current_state)
            .map_err(|_| SnapshotStoreError::Corrupt)?,
        body_hash: unhex(&dto.body_hash)?,
        run_id: dto.run_id,
    })
}

fn hex(bytes: &[u8; 32]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn unhex(text: &str) -> Result<[u8; 32], SnapshotStoreError> {
    if text.len() != 64 || !text.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(SnapshotStoreError::Corrupt);
    }
    let mut bytes = [0; 32];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[index * 2..index * 2 + 2], 16)
            .map_err(|_| SnapshotStoreError::Corrupt)?;
    }
    if hex(&bytes) != text {
        return Err(SnapshotStoreError::Corrupt);
    }
    Ok(bytes)
}
