//! 后台替换全部计划与既有编辑事务的衔接。

use std::fmt;
use std::ops::Range;

use ropey::Rope;

use crate::{
    DocumentId, DocumentRegistry, EditOrigin, EditTransaction, MergePolicy, SearchOptions, TextEdit,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReplacePlanError {
    DocumentNotFound(DocumentId),
    ReadOnly,
    InvalidRegex,
    Cancelled,
    StaleMatch,
}

impl fmt::Display for ReplacePlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::DocumentNotFound(_) => "Document 不存在",
            Self::ReadOnly => "只读或分页 Document 不支持替换",
            Self::InvalidRegex => "替换正则表达式无效",
            Self::Cancelled => "替换已取消",
            Self::StaleMatch => "当前匹配已失效",
        })
    }
}

impl std::error::Error for ReplacePlanError {}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ReplaceTarget {
    All,
    Current(Range<usize>),
}

/// 前台捕获的不可变替换输入；正则编译与捕获展开在后台执行。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocumentReplaceRequest {
    document_id: DocumentId,
    text_revision: u64,
    text: Rope,
    pattern: String,
    replacement: String,
    options: SearchOptions,
    target: ReplaceTarget,
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
        let mut source = if self.options.regex {
            self.pattern.clone()
        } else {
            regex::escape(&self.pattern)
        };
        if self.options.whole_word {
            source = format!(r"\b(?:{source})\b");
        }
        let regex = regex::RegexBuilder::new(&source)
            .case_insensitive(!self.options.case_sensitive)
            .build()
            .map_err(|_| ReplacePlanError::InvalidRegex)?;
        let text = self.text.to_string();
        let mut edits = Vec::new();
        for captures in regex.captures_iter(&text) {
            crate::search::check_cancelled(cancellation)
                .map_err(|_| ReplacePlanError::Cancelled)?;
            let Some(matched) = captures.get(0) else {
                continue;
            };
            if let ReplaceTarget::Current(range) = &self.target
                && matched.range() != *range
            {
                continue;
            }
            let replacement = if self.options.regex {
                let mut expanded = String::new();
                captures.expand(&self.replacement, &mut expanded);
                expanded
            } else {
                self.replacement.clone()
            };
            edits.push(TextEdit::background(
                matched.range(),
                matched.as_str(),
                replacement,
            ));
            if matches!(self.target, ReplaceTarget::Current(_)) {
                break;
            }
        }
        if matches!(self.target, ReplaceTarget::Current(_)) && edits.is_empty() {
            return Err(ReplacePlanError::StaleMatch);
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
    /// 兼容入口：区分大小写的正则全量替换。
    pub fn replace_request(
        &self,
        id: DocumentId,
        pattern: impl Into<String>,
        replacement: impl Into<String>,
    ) -> Result<DocumentReplaceRequest, ReplacePlanError> {
        self.replace_request_with_options(
            id,
            pattern,
            replacement,
            SearchOptions {
                case_sensitive: true,
                regex: true,
                ..SearchOptions::default()
            },
        )
    }

    /// 按查找选项替换全部；字面量替换不展开捕获组。
    pub fn replace_request_with_options(
        &self,
        id: DocumentId,
        pattern: impl Into<String>,
        replacement: impl Into<String>,
        options: SearchOptions,
    ) -> Result<DocumentReplaceRequest, ReplacePlanError> {
        self.replace_request_for(id, pattern, replacement, options, ReplaceTarget::All)
    }

    /// 只替换当前匹配；后台仍按选项与原文重新验证该范围。
    pub fn replace_current_request(
        &self,
        id: DocumentId,
        pattern: impl Into<String>,
        replacement: impl Into<String>,
        options: SearchOptions,
        range: Range<usize>,
    ) -> Result<DocumentReplaceRequest, ReplacePlanError> {
        self.replace_request_for(
            id,
            pattern,
            replacement,
            options,
            ReplaceTarget::Current(range),
        )
    }

    fn replace_request_for(
        &self,
        id: DocumentId,
        pattern: impl Into<String>,
        replacement: impl Into<String>,
        options: SearchOptions,
        target: ReplaceTarget,
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
            options,
            target,
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
        ReplacePlanError, SaveTransactionError, SearchOptions, TextStateId,
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
    fn literal_and_current_replace_honor_options_and_stale_ranges() {
        let (mut registry, id) = registry();
        let options = SearchOptions {
            case_sensitive: false,
            regex: false,
            whole_word: true,
        };
        let replacement = registry
            .replace_request_with_options(id, "A1", "$1", options)
            .unwrap()
            .plan()
            .unwrap();
        registry
            .apply(
                replacement.into_transaction(),
                state("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0f2d"),
                Instant::now(),
            )
            .unwrap();
        assert_eq!(
            registry.session(id).unwrap().text().unwrap().to_string(),
            "$1\na22",
            "字面量替换不展开捕获组，整词与大小写选项生效"
        );

        let current = registry
            .replace_current_request(id, "A22", "tail", SearchOptions::default(), 3..6)
            .unwrap()
            .plan()
            .unwrap();
        registry
            .apply(
                current.into_transaction(),
                state("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0f2e"),
                Instant::now(),
            )
            .unwrap();
        assert_eq!(
            registry.session(id).unwrap().text().unwrap().to_string(),
            "$1\ntail"
        );

        let stale = registry
            .replace_current_request(id, "tail", "x", SearchOptions::default(), 0..3)
            .unwrap();
        assert_eq!(stale.plan(), Err(ReplacePlanError::StaleMatch));
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
