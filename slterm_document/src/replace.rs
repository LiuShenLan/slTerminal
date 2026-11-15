//! 后台替换全部计划与既有编辑事务的衔接。

use std::fmt;

use ropey::Rope;

use crate::{DocumentId, DocumentRegistry, EditOrigin, EditTransaction, MergePolicy, TextEdit};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReplacePlanError {
    DocumentNotFound(DocumentId),
    ReadOnly,
    InvalidRegex,
    Cancelled,
}

impl fmt::Display for ReplacePlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::DocumentNotFound(_) => "Document 不存在",
            Self::ReadOnly => "只读或分页 Document 不支持替换全部",
            Self::InvalidRegex => "替换正则表达式无效",
            Self::Cancelled => "替换全部已取消",
        })
    }
}

impl std::error::Error for ReplacePlanError {}

/// 前台捕获的不可变替换输入；正则编译与捕获展开在后台执行。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocumentReplaceRequest {
    document_id: DocumentId,
    text_revision: u64,
    text: Rope,
    pattern: String,
    replacement: String,
}

impl DocumentReplaceRequest {
    pub fn plan(self) -> Result<DocumentReplacement, ReplacePlanError> {
        let (_cancel, cancellation) = std::sync::mpsc::sync_channel(1);
        self.plan_cancellable(&cancellation)
    }

    pub fn plan_cancellable(
        self,
        cancellation: &std::sync::mpsc::Receiver<()>,
    ) -> Result<DocumentReplacement, ReplacePlanError> {
        let regex = regex::Regex::new(&self.pattern).map_err(|_| ReplacePlanError::InvalidRegex)?;
        let text = self.text.to_string();
        let mut edits = Vec::new();
        for captures in regex.captures_iter(&text) {
            crate::search::check_cancelled(cancellation)
                .map_err(|_| ReplacePlanError::Cancelled)?;
            let Some(matched) = captures.get(0) else {
                continue;
            };
            let mut replacement = String::new();
            captures.expand(&self.replacement, &mut replacement);
            edits.push(TextEdit::background(
                matched.range(),
                matched.as_str(),
                replacement,
            ));
        }
        crate::search::check_cancelled(cancellation).map_err(|_| ReplacePlanError::Cancelled)?;
        Ok(DocumentReplacement {
            document_id: self.document_id,
            text_revision: self.text_revision,
            edits,
        })
    }
}

/// 经用户确认后可一次提交的后台替换结果；应用仍核对捕获时的 revision。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocumentReplacement {
    document_id: DocumentId,
    text_revision: u64,
    edits: Vec<TextEdit>,
}

impl DocumentReplacement {
    pub fn into_transaction(self) -> EditTransaction {
        EditTransaction {
            document_id: self.document_id,
            expected_text_revision: self.text_revision,
            edits: self.edits,
            origin: EditOrigin::BackgroundReplacement,
            merge_policy: MergePolicy::Disallowed,
        }
    }
}

impl DocumentRegistry {
    pub fn replace_request(
        &self,
        id: DocumentId,
        pattern: impl Into<String>,
        replacement: impl Into<String>,
    ) -> Result<DocumentReplaceRequest, ReplacePlanError> {
        let session = self
            .session(id)
            .ok_or(ReplacePlanError::DocumentNotFound(id))?;
        if session.is_read_only() {
            return Err(ReplacePlanError::ReadOnly);
        }
        let text = session.text().cloned().ok_or(ReplacePlanError::ReadOnly)?;
        Ok(DocumentReplaceRequest {
            document_id: id,
            text_revision: session.text_revision(),
            text,
            pattern: pattern.into(),
            replacement: replacement.into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;
    use std::time::Instant;

    use ropey::Rope;
    use slterm_path::{CanonicalPath, ResolvedPath};

    use crate::{
        DocumentId, DocumentMetadata, DocumentRegistry, EditApplyOutcome, EditTransactionError,
        ReplacePlanError, SaveTransactionError, TextStateId,
    };

    fn registry() -> (DocumentRegistry, DocumentId) {
        let id = DocumentId::from_str("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2a").unwrap();
        let state = TextStateId::from_str("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0f2a").unwrap();
        let mut registry = DocumentRegistry::new();
        registry
            .open(
                id,
                ResolvedPath::new(CanonicalPath::new("D:/work/replace.txt").unwrap(), None),
                Rope::from_str("a1\na22"),
                state,
                std::sync::Arc::from(b"a1\na22".as_slice()),
                DocumentMetadata::default(),
            )
            .unwrap();
        (registry, id)
    }

    fn state(text: &str) -> TextStateId {
        TextStateId::from_str(text).unwrap()
    }

    #[test]
    fn editor_replace_all_single_undo() {
        let (mut registry, id) = registry();
        let request = registry.replace_request(id, r"a(\d+)", "x$1").unwrap();
        let replacement = request.plan().unwrap();

        registry
            .apply(
                replacement.into_transaction(),
                state("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0f2b"),
                Instant::now(),
            )
            .unwrap();

        assert_eq!(
            registry.session(id).unwrap().text().unwrap().to_string(),
            "x1\nx22"
        );
        assert_eq!(registry.undo(id), Ok(EditApplyOutcome::Applied));
        assert_eq!(
            registry.session(id).unwrap().text().unwrap().to_string(),
            "a1\na22"
        );
        assert_eq!(registry.redo(id), Ok(EditApplyOutcome::Applied));
        assert_eq!(
            registry.session(id).unwrap().text().unwrap().to_string(),
            "x1\nx22"
        );
    }

    #[test]
    fn invalid_or_stale_replacement_preserves_document() {
        let (mut registry, id) = registry();
        let invalid = registry.replace_request(id, "(", "x").unwrap();
        assert_eq!(invalid.plan(), Err(ReplacePlanError::InvalidRegex));
        assert_eq!(
            registry.session(id).unwrap().text().unwrap().to_string(),
            "a1\na22"
        );

        let request = registry.replace_request(id, "a", "x").unwrap();
        let replacement = request.plan().unwrap();
        registry
            .apply(
                replacement.into_transaction(),
                state("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0f2b"),
                Instant::now(),
            )
            .unwrap();
        let stale = registry.replace_request(id, "x", "stale").unwrap();
        let mut current = stale.plan().unwrap().into_transaction();
        current.expected_text_revision -= 1;
        assert_eq!(
            registry.apply(
                current,
                state("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0f2c"),
                Instant::now(),
            ),
            Err(EditTransactionError::RevisionMismatch)
        );
        assert_eq!(
            registry.session(id).unwrap().text().unwrap().to_string(),
            "x1\nx22"
        );
    }

    #[test]
    fn replacement_plan_observes_cancellation() {
        let (registry, id) = registry();
        let request = registry.replace_request(id, "a", "x").unwrap();
        let (cancel, cancellation) = std::sync::mpsc::sync_channel(1);
        cancel.send(()).unwrap();

        assert_eq!(
            request.plan_cancellable(&cancellation),
            Err(ReplacePlanError::Cancelled)
        );
    }

    #[test]
    fn paged_document_has_no_replace_all_or_save_entrypoint() {
        let id = DocumentId::from_str("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2a").unwrap();
        let state = TextStateId::from_str("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0f2a").unwrap();
        let mut registry = DocumentRegistry::new();
        registry
            .open(
                id,
                ResolvedPath::new(CanonicalPath::new("D:/work/paged.txt").unwrap(), None),
                crate::PagedSource::new(
                    crate::PageIndex::new(0, Vec::new()).unwrap(),
                    [0; 32],
                    crate::PagedLineIndex::from_page_stats(&[]),
                ),
                state,
                std::sync::Arc::default(),
                DocumentMetadata {
                    read_only: true,
                    ..DocumentMetadata::default()
                },
            )
            .unwrap();

        assert_eq!(
            registry.replace_request(id, "a", "x"),
            Err(ReplacePlanError::ReadOnly)
        );
        assert_eq!(
            registry.begin_save(id, 0),
            Err(SaveTransactionError::ReadOnly)
        );
    }
}
