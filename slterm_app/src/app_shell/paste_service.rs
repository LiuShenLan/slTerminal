//! 粘贴冲突预检、逐项串行执行与 Journal 结算；成功项不因后续失败回滚。
//! 本服务含阻塞 I/O 与互斥等待，只允许在后台执行体调用。

use std::{
    io,
    sync::{Arc, Mutex},
};

use slterm_path::{CanonicalPath, PathGrant, PathOperation};
use slterm_sidebar::{
    FileClipboardMode, FileTreeRoot, PasteItem, PastePlan, PasteRejection, PasteTarget,
};
use uuid::Uuid;

use super::{
    file_operation_service::{FileOperationError, FileOperationService, OperationPlan},
    paste_journal::{
        PasteJournal, PasteJournalRecord, PasteOperationKind, PastePath, outcome_fingerprint,
    },
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum PasteConflictDecision {
    Cancel,
    #[default]
    Skip,
    Replace,
}

pub(crate) struct PreparedPaste {
    mode: FileClipboardMode,
    target: PasteTarget,
    rejections: Vec<PasteRejection>,
    items: Vec<PreparedPasteItem>,
}

impl PreparedPaste {
    pub(crate) fn conflicts(&self) -> Vec<&CanonicalPath> {
        self.items
            .iter()
            .filter(|item| matches!(item.state, PreparedItemState::Conflict(_)))
            .map(|item| &item.item.target)
            .collect()
    }
}

struct PreparedPasteItem {
    item: PasteItem,
    state: PreparedItemState,
}

enum PreparedItemState {
    Ready,
    Conflict(Box<OperationPlan>),
    Failed(FileOperationError),
}

#[derive(Debug)]
pub(crate) enum PasteItemOutcome {
    Copied,
    Moved,
    Skipped,
    Failed(FileOperationError),
    NeedsAttention,
}

#[derive(Debug)]
pub(crate) struct PasteItemResult {
    pub(crate) item: PasteItem,
    pub(crate) outcome: PasteItemOutcome,
}

#[derive(Debug)]
pub(crate) struct PasteReport {
    pub(crate) cancelled: bool,
    pub(crate) items: Vec<PasteItemResult>,
    pub(crate) rejections: Vec<PasteRejection>,
}

pub(crate) struct PasteService {
    operations: FileOperationService,
    journal: Option<Arc<dyn PasteJournal>>,
    store_epoch: String,
    operation_lock: Mutex<()>,
}

impl PasteService {
    pub(crate) fn new(
        operations: FileOperationService,
        journal: Arc<dyn PasteJournal>,
        store_epoch: String,
    ) -> Self {
        Self {
            operations,
            journal: Some(journal),
            store_epoch,
            operation_lock: Mutex::new(()),
        }
    }

    /// 无 writer 的测试根走同一执行语义；生产无 Journal 时调用方拒绝粘贴。
    #[cfg(test)]
    pub(crate) fn without_journal(operations: FileOperationService) -> Self {
        Self {
            operations,
            journal: None,
            store_epoch: String::new(),
            operation_lock: Mutex::new(()),
        }
    }

    /// 只读预检并聚合全部冲突；冲突删除计划只保存预期事实，不保存授权。
    pub(crate) fn prepare(
        &self,
        plan: &PastePlan,
        grants: &[PathGrant],
        authorization_generation: u64,
    ) -> PreparedPaste {
        let items = plan
            .items
            .iter()
            .cloned()
            .map(|item| {
                let state = match self.plan_item(
                    plan.mode,
                    &plan.target.root,
                    &item,
                    grants,
                    authorization_generation,
                ) {
                    Ok(_) => PreparedItemState::Ready,
                    Err(error) if is_already_exists(&error) => match self.operations.plan_delete(
                        plan.target.root.clone(),
                        &item.target,
                        grants,
                        authorization_generation,
                    ) {
                        Ok(delete) => PreparedItemState::Conflict(Box::new(delete)),
                        Err(error) => PreparedItemState::Failed(error),
                    },
                    Err(error) => PreparedItemState::Failed(error),
                };
                PreparedPasteItem { item, state }
            })
            .collect();
        PreparedPaste {
            mode: plan.mode,
            target: plan.target.clone(),
            rejections: plan.rejections.clone(),
            items,
        }
    }

    pub(crate) fn execute(
        &self,
        prepared: PreparedPaste,
        decision: PasteConflictDecision,
        grants: &[PathGrant],
        current_authorization_generation: u64,
    ) -> PasteReport {
        if decision == PasteConflictDecision::Cancel {
            return PasteReport {
                cancelled: true,
                items: Vec::new(),
                rejections: prepared.rejections,
            };
        }
        let _guard = self
            .operation_lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut results = Vec::with_capacity(prepared.items.len());
        for prepared_item in prepared.items {
            let item = prepared_item.item;
            let outcome = match prepared_item.state {
                PreparedItemState::Failed(error) => PasteItemOutcome::Failed(error),
                PreparedItemState::Conflict(_) if decision == PasteConflictDecision::Skip => {
                    PasteItemOutcome::Skipped
                }
                PreparedItemState::Conflict(delete) => self.execute_replacement(
                    prepared.mode,
                    &prepared.target.root,
                    &item,
                    *delete,
                    grants,
                    current_authorization_generation,
                ),
                PreparedItemState::Ready => self.plan_and_execute(
                    prepared.mode,
                    &prepared.target.root,
                    &item,
                    grants,
                    current_authorization_generation,
                ),
            };
            results.push(PasteItemResult { item, outcome });
        }
        PasteReport {
            cancelled: false,
            items: results,
            rejections: prepared.rejections,
        }
    }

    fn execute_replacement(
        &self,
        mode: FileClipboardMode,
        target_root: &FileTreeRoot,
        item: &PasteItem,
        delete: OperationPlan,
        grants: &[PathGrant],
        current_authorization_generation: u64,
    ) -> PasteItemOutcome {
        let source_operation = match mode {
            FileClipboardMode::Copy => PathOperation::Read,
            FileClipboardMode::Cut => PathOperation::Write,
        };
        let source = match self.operations.check_source(
            &item.source.path,
            source_operation,
            grants,
            current_authorization_generation,
        ) {
            Ok(source) => source,
            Err(error) => return PasteItemOutcome::Failed(error),
        };
        let replacement = delete.paste_snapshot();
        let destination = PastePath {
            lexical_path: item.target.clone(),
            canonical_path: replacement.resolved.canonical_path().clone(),
        };
        let kind = match mode {
            FileClipboardMode::Copy => PasteOperationKind::ReplaceCopy,
            FileClipboardMode::Cut => PasteOperationKind::ReplaceCut,
        };
        self.execute_recorded(kind, mode, source, destination, Some(replacement), || {
            self.operations
                .execute(&delete, grants, current_authorization_generation)?;
            // 回收完成后基于真实状态重建复制／移动计划。
            let plan = self.plan_item(
                mode,
                target_root,
                item,
                grants,
                current_authorization_generation,
            )?;
            self.operations
                .execute(&plan, grants, current_authorization_generation)?;
            Ok(())
        })
    }

    fn plan_and_execute(
        &self,
        mode: FileClipboardMode,
        target_root: &FileTreeRoot,
        item: &PasteItem,
        grants: &[PathGrant],
        current_authorization_generation: u64,
    ) -> PasteItemOutcome {
        let plan = match self.plan_item(
            mode,
            target_root,
            item,
            grants,
            current_authorization_generation,
        ) {
            Ok(plan) => plan,
            Err(error) => return PasteItemOutcome::Failed(error),
        };
        let kind = match mode {
            FileClipboardMode::Copy => PasteOperationKind::Copy,
            FileClipboardMode::Cut => PasteOperationKind::Cut,
        };
        let source = plan.paste_snapshot();
        let destination = plan.paste_destination();
        self.execute_recorded(kind, mode, source, destination, None, || {
            self.operations
                .execute(&plan, grants, current_authorization_generation)?;
            Ok(())
        })
    }

    fn execute_recorded(
        &self,
        kind: PasteOperationKind,
        mode: FileClipboardMode,
        source: super::paste_journal::PasteSnapshot,
        destination: PastePath,
        replacement: Option<super::paste_journal::PasteSnapshot>,
        operation: impl FnOnce() -> Result<(), FileOperationError>,
    ) -> PasteItemOutcome {
        let Some(journal) = &self.journal else {
            return paste_outcome(mode, operation());
        };
        let journal_id = Uuid::now_v7().to_string();
        let registration = match PasteJournalRecord::registration(
            &journal_id,
            kind,
            &source,
            &destination,
            replacement.as_ref(),
            &self.store_epoch,
        ) {
            Ok(registration) => registration,
            Err(_) => return PasteItemOutcome::Failed(FileOperationError::Journal),
        };
        if journal.begin(registration).is_err() || journal.mark_applying(&journal_id).is_err() {
            return PasteItemOutcome::Failed(FileOperationError::Journal);
        }

        let result = operation();
        let Ok(after) = outcome_fingerprint(destination.lexical_path.as_path()) else {
            return PasteItemOutcome::NeedsAttention;
        };
        if journal
            .record_after(&journal_id, &journal_id, after)
            .is_err()
            || journal.commit(&journal_id, after).is_err()
        {
            return PasteItemOutcome::NeedsAttention;
        }
        paste_outcome(mode, result)
    }

    fn plan_item(
        &self,
        mode: FileClipboardMode,
        target_root: &FileTreeRoot,
        item: &PasteItem,
        grants: &[PathGrant],
        authorization_generation: u64,
    ) -> Result<OperationPlan, FileOperationError> {
        match mode {
            FileClipboardMode::Copy => self.operations.plan_copy(
                item.source.root.clone(),
                target_root.clone(),
                &item.source.path,
                &item.target,
                grants,
                authorization_generation,
            ),
            FileClipboardMode::Cut => self.operations.plan_rename(
                target_root.clone(),
                &item.source.path,
                &item.target,
                grants,
                authorization_generation,
            ),
        }
    }
}

fn paste_outcome(
    mode: FileClipboardMode,
    result: Result<(), FileOperationError>,
) -> PasteItemOutcome {
    match result {
        Ok(()) => match mode {
            FileClipboardMode::Copy => PasteItemOutcome::Copied,
            FileClipboardMode::Cut => PasteItemOutcome::Moved,
        },
        Err(error) => PasteItemOutcome::Failed(error),
    }
}

fn is_already_exists(error: &FileOperationError) -> bool {
    matches!(
        error,
        FileOperationError::Io(error) if error.kind() == io::ErrorKind::AlreadyExists
    )
}
