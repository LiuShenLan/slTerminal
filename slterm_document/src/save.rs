//! Document 保存请求、代次与回执对账。

use std::fmt;
use std::sync::Arc;

use ropey::Rope;
use slterm_path::{FileIdentity, ResolvedPath};

use crate::{DocumentId, DocumentRegistry, TextEncoding, TextStateId};

/// 前台一次性捕获的后台保存输入；后续编辑不改变此快照。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocumentSaveRequest {
    document_id: DocumentId,
    text_revision: u64,
    text_state: TextStateId,
    text: Rope,
    disk_baseline: Arc<[u8]>,
    encoding: TextEncoding,
    location: ResolvedPath,
    authorization_generation: u64,
    save_generation: u64,
}

impl DocumentSaveRequest {
    pub const fn document_id(&self) -> DocumentId {
        self.document_id
    }

    pub const fn text_revision(&self) -> u64 {
        self.text_revision
    }

    pub const fn text_state(&self) -> TextStateId {
        self.text_state
    }

    pub fn text(&self) -> &Rope {
        &self.text
    }

    pub fn disk_baseline(&self) -> &Arc<[u8]> {
        &self.disk_baseline
    }

    pub const fn encoding(&self) -> &TextEncoding {
        &self.encoding
    }

    pub fn location(&self) -> &ResolvedPath {
        &self.location
    }

    pub const fn authorization_generation(&self) -> u64 {
        self.authorization_generation
    }

    pub const fn save_generation(&self) -> u64 {
        self.save_generation
    }
}

/// 后台写盘完成后回前台的不可变事实；文件身份来自替换后的真实句柄。
#[derive(Clone, Debug)]
pub struct DocumentSaveReceipt {
    request: DocumentSaveRequest,
    disk_baseline: Arc<[u8]>,
    location: ResolvedPath,
}

impl DocumentSaveReceipt {
    pub fn new(
        request: DocumentSaveRequest,
        disk_baseline: Arc<[u8]>,
        file_identity: Option<FileIdentity>,
    ) -> Self {
        let location = ResolvedPath::new(request.location.canonical_path().clone(), file_identity);
        Self {
            request,
            disk_baseline,
            location,
        }
    }

    /// 另存为回执携带用户选定并已实际写入的新位置。
    pub fn for_save_as(
        request: DocumentSaveRequest,
        disk_baseline: Arc<[u8]>,
        location: ResolvedPath,
    ) -> Self {
        Self {
            request,
            disk_baseline,
            location,
        }
    }

    pub const fn document_id(&self) -> DocumentId {
        self.request.document_id
    }

    pub const fn save_generation(&self) -> u64 {
        self.request.save_generation
    }

    pub const fn text_state(&self) -> TextStateId {
        self.request.text_state
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveTransactionError {
    DocumentNotFound(DocumentId),
    ReadOnly,
    SaveInProgress,
    SaveGenerationExhausted,
    StaleSave,
    LocationMismatch,
    TargetOccupied,
}

impl fmt::Display for SaveTransactionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::DocumentNotFound(_) => "Document 不存在",
            Self::ReadOnly => "只读 Document 不能保存",
            Self::SaveInProgress => "Document 已有保存任务在途",
            Self::SaveGenerationExhausted => "保存代次已耗尽",
            Self::StaleSave => "保存回执不属于当前保存代次",
            Self::LocationMismatch => "保存回执的位置与保存类型不匹配",
            Self::TargetOccupied => "另存为目标已关联其他 Document",
        })
    }
}

impl std::error::Error for SaveTransactionError {}

impl DocumentRegistry {
    /// 捕获保存快照并占用本 Document 的唯一在途保存代次。
    pub fn begin_save(
        &mut self,
        id: DocumentId,
        authorization_generation: u64,
    ) -> Result<DocumentSaveRequest, SaveTransactionError> {
        let session = self
            .session_mut(id)
            .ok_or(SaveTransactionError::DocumentNotFound(id))?;
        if session.is_read_only() {
            return Err(SaveTransactionError::ReadOnly);
        }
        let text = session
            .text()
            .cloned()
            .ok_or(SaveTransactionError::ReadOnly)?;
        if session.active_save.is_some() {
            return Err(SaveTransactionError::SaveInProgress);
        }
        let save_generation = session
            .save_generation
            .checked_add(1)
            .ok_or(SaveTransactionError::SaveGenerationExhausted)?;
        session.save_generation = save_generation;
        session.active_save = Some(save_generation);
        // 保存快照是撤销合并边界；在途编辑必须可单独撤回到被保存状态。
        session.history.barrier();
        Ok(DocumentSaveRequest {
            document_id: session.id,
            text_revision: session.text_revision,
            text_state: session.current_state,
            text,
            disk_baseline: session.disk_baseline.clone(),
            encoding: session.metadata.encoding.clone(),
            location: session.location.clone(),
            authorization_generation,
            save_generation,
        })
    }

    /// 失败只释放在途闸门；正文、保存状态与磁盘基线保持原值。
    pub fn fail_save(
        &mut self,
        id: DocumentId,
        save_generation: u64,
    ) -> Result<(), SaveTransactionError> {
        let session = self
            .session_mut(id)
            .ok_or(SaveTransactionError::DocumentNotFound(id))?;
        if session.active_save != Some(save_generation) {
            return Err(SaveTransactionError::StaleSave);
        }
        session.active_save = None;
        Ok(())
    }

    /// 只提交请求捕获的状态；在请求之后产生的编辑继续保持脏。
    pub fn complete_save(
        &mut self,
        receipt: DocumentSaveReceipt,
    ) -> Result<(), SaveTransactionError> {
        self.complete_save_at(receipt, false)
    }

    /// 另存为成功保持 DocumentId，只重绑位置、身份与被保存状态。
    pub fn complete_save_as(
        &mut self,
        receipt: DocumentSaveReceipt,
    ) -> Result<(), SaveTransactionError> {
        self.complete_save_at(receipt, true)
    }

    fn complete_save_at(
        &mut self,
        receipt: DocumentSaveReceipt,
        save_as: bool,
    ) -> Result<(), SaveTransactionError> {
        let id = receipt.request.document_id;
        if save_as
            && self
                .existing(&receipt.location)
                .is_some_and(|existing| existing != id)
        {
            return Err(SaveTransactionError::TargetOccupied);
        }
        // 普通保存替换文件后也可能取得另一聚合已占用的身份；提交前拒绝，
        // 不得改写身份唯一索引。
        if let Some(identity) = receipt.location.file_identity()
            && self
                .identity_owner(identity)
                .is_some_and(|owner| owner != id)
        {
            return Err(SaveTransactionError::TargetOccupied);
        }
        let session = self
            .session_mut(id)
            .ok_or(SaveTransactionError::DocumentNotFound(id))?;
        if session.active_save != Some(receipt.request.save_generation) {
            return Err(SaveTransactionError::StaleSave);
        }
        let original = receipt.request.location.canonical_path();
        let target = receipt.location.canonical_path();
        if (!save_as && (target != original || session.location.canonical_path() != original))
            || (save_as && target == original)
        {
            return Err(SaveTransactionError::LocationMismatch);
        }
        let previous_identity = session.location.file_identity();
        let location_changed = save_as && session.location.canonical_path() != target;
        session.saved_state = receipt.request.text_state;
        session.disk_baseline = receipt.disk_baseline;
        if save_as {
            session.location = receipt.location;
        } else {
            // 普通保存已核对当前位置未偏离，只更新替换后的文件身份。
            session.location = ResolvedPath::new(
                session.location.canonical_path().clone(),
                receipt.location.file_identity(),
            );
        }
        session.active_save = None;
        session.history.barrier();
        let current_identity = session.location.file_identity();
        if location_changed {
            self.rebind_path(id);
        }
        if previous_identity != current_identity {
            self.refresh_identity(id, current_identity);
        }
        Ok(())
    }
}
