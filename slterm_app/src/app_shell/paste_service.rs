//! 粘贴冲突预检与逐项串行执行；计划不跨项沿用，成功项不因后续失败回滚。
//! 本服务含阻塞 I/O 与互斥等待，只允许在后台执行体调用。

use std::{io, sync::Mutex};

use slterm_path::{CanonicalPath, PathGrant, PathOperation};
use slterm_sidebar::{
    FileClipboardMode, FileTreeRoot, PasteItem, PastePlan, PasteRejection, PasteTarget,
};

use super::file_operation_service::{
    FileOperationError, FileOperationOutcome, FileOperationService, OperationPlan,
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
    operation_lock: Mutex<()>,
}

impl PasteService {
    pub(crate) fn new(operations: FileOperationService) -> Self {
        Self {
            operations,
            operation_lock: Mutex::new(()),
        }
    }

    /// 只读预检并聚合全部冲突；返回结果不包含可沿用的单项计划或授权。
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
        if let Err(error) = self.operations.check_source(
            &item.source.path,
            source_operation,
            grants,
            current_authorization_generation,
        ) {
            return PasteItemOutcome::Failed(error);
        }
        if let Err(error) =
            self.operations
                .execute(&delete, grants, current_authorization_generation)
        {
            return PasteItemOutcome::Failed(error);
        }
        // 回收完成后必须基于真实状态重建复制／移动计划。
        self.plan_and_execute(
            mode,
            target_root,
            item,
            grants,
            current_authorization_generation,
        )
    }

    fn plan_and_execute(
        &self,
        mode: FileClipboardMode,
        target_root: &FileTreeRoot,
        item: &PasteItem,
        grants: &[PathGrant],
        current_authorization_generation: u64,
    ) -> PasteItemOutcome {
        let result = self
            .plan_item(
                mode,
                target_root,
                item,
                grants,
                current_authorization_generation,
            )
            .and_then(|plan| {
                self.operations
                    .execute(&plan, grants, current_authorization_generation)
            });
        match result {
            Ok(FileOperationOutcome::Completed | FileOperationOutcome::AlreadyAbsent) => match mode
            {
                FileClipboardMode::Copy => PasteItemOutcome::Copied,
                FileClipboardMode::Cut => PasteItemOutcome::Moved,
            },
            Err(error) => PasteItemOutcome::Failed(error),
        }
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

fn is_already_exists(error: &FileOperationError) -> bool {
    matches!(
        error,
        FileOperationError::Io(error) if error.kind() == io::ErrorKind::AlreadyExists
    )
}
