//! Document 编辑事务、差量撤销历史与 TextStateId 清脏语义。

use std::fmt;
use std::ops::Range;
use std::time::{Duration, Instant};

use crate::{DocumentId, DocumentRegistry, TextStateId};

const MERGE_WINDOW: Duration = Duration::from_millis(600);
const MAX_HISTORY_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextEdit {
    utf8_range: Range<usize>,
    replacement: String,
    expected: Option<String>,
}

impl TextEdit {
    pub fn new(utf8_range: Range<usize>, replacement: impl Into<String>) -> Self {
        Self {
            utf8_range,
            replacement: replacement.into(),
            expected: None,
        }
    }

    pub fn background(
        utf8_range: Range<usize>,
        expected: impl Into<String>,
        replacement: impl Into<String>,
    ) -> Self {
        Self {
            utf8_range,
            replacement: replacement.into(),
            expected: Some(expected.into()),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditOrigin {
    UserInput,
    BackgroundReplacement,
    ReplaceAll,
    EncodingReload,
    LineEndingChange,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MergePolicy {
    Adjacent,
    Disallowed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditTransaction {
    pub document_id: DocumentId,
    pub expected_text_revision: u64,
    pub edits: Vec<TextEdit>,
    pub origin: EditOrigin,
    pub merge_policy: MergePolicy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditApplyOutcome {
    Applied,
    Unchanged,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditTransactionError {
    DocumentNotFound(DocumentId),
    RevisionMismatch,
    InvalidRange,
    OverlappingEdits,
    UnorderedEdits,
    ReadOnly,
    MissingOriginalText,
    OriginalTextMismatch,
    RevisionExhausted,
    StateConflict,
}

impl fmt::Display for EditTransactionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::DocumentNotFound(_) => "Document 不存在",
            Self::RevisionMismatch => "编辑基于过期文本 revision",
            Self::InvalidRange => "编辑区间不是有效 UTF-8 边界",
            Self::OverlappingEdits => "编辑区间相互重叠",
            Self::UnorderedEdits => "编辑区间必须按起点递增",
            Self::ReadOnly => "只读 Document 不接受编辑",
            Self::MissingOriginalText => "后台替换缺少原始区间文本",
            Self::OriginalTextMismatch => "后台替换的原始区间文本已变化",
            Self::RevisionExhausted => "文本 revision 已耗尽",
            Self::StateConflict => "新编辑复用了既有 TextStateId",
        })
    }
}

impl std::error::Error for EditTransactionError {}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Change {
    start: usize,
    before: String,
    after: String,
}

impl Change {
    fn bytes(&self) -> usize {
        self.before.len() + self.after.len()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct EditGroup {
    changes: Vec<Change>,
    before_state: TextStateId,
    after_state: TextStateId,
    origin: EditOrigin,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct EditHistory {
    undo: Vec<EditGroup>,
    redo: Vec<EditGroup>,
    bytes: usize,
    last_edit: Option<Instant>,
}

impl EditHistory {
    pub(crate) fn barrier(&mut self) {
        self.last_edit = None;
    }

    fn should_join(&self, transaction: &EditTransaction, changes: &[Change], at: Instant) -> bool {
        if transaction.origin != EditOrigin::UserInput
            || transaction.merge_policy != MergePolicy::Adjacent
            || changes.len() != 1
        {
            return false;
        }
        let Some(group) = self.undo.last() else {
            return false;
        };
        if group.origin != EditOrigin::UserInput || group.changes.len() != 1 {
            return false;
        }
        let change = &changes[0];
        let previous = &group.changes[0];
        self.last_edit
            .is_some_and(|last| at.duration_since(last) < MERGE_WINDOW)
            && change.start <= previous.start + previous.after.len()
            && change.start + change.before.len() >= previous.start
    }

    fn push(&mut self, group: EditGroup, at: Instant, join: bool) {
        self.bytes = self.bytes.saturating_sub(
            self.redo
                .iter()
                .flat_map(|group| &group.changes)
                .map(Change::bytes)
                .sum(),
        );
        self.redo.clear();
        self.bytes += group.changes.iter().map(Change::bytes).sum::<usize>();
        if join {
            let current = self.undo.last_mut().expect("可合并组必须存在");
            current.after_state = group.after_state;
            current.changes.extend(group.changes);
        } else {
            self.undo.push(group);
        }
        while self.bytes > MAX_HISTORY_BYTES && !self.undo.is_empty() {
            let removed = self.undo.remove(0);
            self.bytes = self
                .bytes
                .saturating_sub(removed.changes.iter().map(Change::bytes).sum::<usize>());
        }
        self.last_edit = Some(at);
    }
}

impl crate::DocumentSession {
    fn state_is_known(&self, state: TextStateId) -> bool {
        self.known_states.contains(&state)
    }

    fn validate_transaction(
        &self,
        transaction: &EditTransaction,
        new_state: TextStateId,
    ) -> Result<(), EditTransactionError> {
        if self.metadata.read_only {
            return Err(EditTransactionError::ReadOnly);
        }
        if self.text_revision != transaction.expected_text_revision {
            return Err(EditTransactionError::RevisionMismatch);
        }
        if self.text_revision == u64::MAX {
            return Err(EditTransactionError::RevisionExhausted);
        }
        if self.state_is_known(new_state) {
            return Err(EditTransactionError::StateConflict);
        }
        if transaction.origin == EditOrigin::BackgroundReplacement
            && transaction.edits.iter().any(|edit| edit.expected.is_none())
        {
            return Err(EditTransactionError::MissingOriginalText);
        }
        let mut previous = None;
        for edit in &transaction.edits {
            let range = &edit.utf8_range;
            if range.start > range.end
                || range.end > self.text.len()
                || !self.text.is_char_boundary(range.start)
                || !self.text.is_char_boundary(range.end)
            {
                return Err(EditTransactionError::InvalidRange);
            }
            if let Some((previous_start, previous_end)) = previous {
                if range.start < previous_start {
                    return Err(EditTransactionError::UnorderedEdits);
                }
                if range.start < previous_end {
                    return Err(EditTransactionError::OverlappingEdits);
                }
            }
            if let Some(expected) = &edit.expected
                && self.text.slice(range.clone()) != expected.as_str()
            {
                return Err(EditTransactionError::OriginalTextMismatch);
            }
            previous = Some((range.start, range.end));
        }
        Ok(())
    }

    fn apply(
        &mut self,
        transaction: EditTransaction,
        new_state: TextStateId,
        at: Instant,
    ) -> Result<EditApplyOutcome, EditTransactionError> {
        self.validate_transaction(&transaction, new_state)?;
        let mut text = self.text.clone();
        let mut changes = Vec::new();
        for edit in transaction.edits.iter().rev() {
            let before = text.slice(edit.utf8_range.clone()).to_string();
            if before == edit.replacement {
                continue;
            }
            text.remove(edit.utf8_range.clone());
            text.insert(edit.utf8_range.start, &edit.replacement);
            changes.push(Change {
                start: edit.utf8_range.start,
                before,
                after: edit.replacement.clone(),
            });
        }
        if changes.is_empty() {
            return Ok(EditApplyOutcome::Unchanged);
        }
        let group = EditGroup {
            before_state: self.current_state,
            after_state: new_state,
            origin: transaction.origin,
            changes,
        };
        let join = self.history.should_join(&transaction, &group.changes, at);
        self.history.push(group, at, join);
        self.text = text;
        self.current_state = new_state;
        self.known_states.insert(new_state);
        self.text_revision += 1;
        Ok(EditApplyOutcome::Applied)
    }

    fn travel(&mut self, redo: bool) -> Result<EditApplyOutcome, EditTransactionError> {
        if self.text_revision == u64::MAX {
            return Err(EditTransactionError::RevisionExhausted);
        }
        let stack = if redo {
            &mut self.history.redo
        } else {
            &mut self.history.undo
        };
        let Some(group) = stack.pop() else {
            return Ok(EditApplyOutcome::Unchanged);
        };
        let mut text = self.text.clone();
        if redo {
            for change in &group.changes {
                text.remove(change.start..change.start + change.before.len());
                text.insert(change.start, &change.after);
            }
        } else {
            for change in group.changes.iter().rev() {
                text.remove(change.start..change.start + change.after.len());
                text.insert(change.start, &change.before);
            }
        }
        self.text = text;
        self.current_state = if redo {
            group.after_state
        } else {
            group.before_state
        };
        self.text_revision += 1;
        if redo {
            self.history.undo.push(group);
        } else {
            self.history.redo.push(group);
        }
        self.history.barrier();
        Ok(EditApplyOutcome::Applied)
    }
}

impl DocumentRegistry {
    pub fn apply(
        &mut self,
        transaction: EditTransaction,
        new_state: TextStateId,
        at: Instant,
    ) -> Result<EditApplyOutcome, EditTransactionError> {
        self.session_mut(transaction.document_id)
            .ok_or(EditTransactionError::DocumentNotFound(
                transaction.document_id,
            ))?
            .apply(transaction, new_state, at)
    }

    pub fn undo(&mut self, id: DocumentId) -> Result<EditApplyOutcome, EditTransactionError> {
        self.session_mut(id)
            .ok_or(EditTransactionError::DocumentNotFound(id))?
            .travel(false)
    }

    pub fn redo(&mut self, id: DocumentId) -> Result<EditApplyOutcome, EditTransactionError> {
        self.session_mut(id)
            .ok_or(EditTransactionError::DocumentNotFound(id))?
            .travel(true)
    }
}

#[cfg(test)]
mod tests {
    use proptest::strategy::Strategy;
    use std::time::{Duration, Instant};

    use ropey::Rope;
    use slterm_path::CanonicalPath;

    use crate::{
        DocumentId, DocumentMetadata, DocumentRegistry, DocumentSaveReceipt, EditApplyOutcome,
        EditOrigin, EditTransaction, EditTransactionError, MergePolicy, TextEdit, TextStateId,
    };

    const DOCUMENT: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0f2a";
    const STATE_A: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2a";
    const STATE_B: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b";
    const STATE_C: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2c";
    const STATE_D: &str = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2d";

    fn state(text: &str) -> TextStateId {
        text.parse().unwrap()
    }

    fn document(text: &str, read_only: bool) -> (DocumentRegistry, DocumentId, TextStateId) {
        let id = DOCUMENT.parse().unwrap();
        let state = state(STATE_A);
        let mut registry = DocumentRegistry::new();
        registry
            .open(
                id,
                slterm_path::ResolvedPath::new(
                    CanonicalPath::new("D:/work/file.txt").unwrap(),
                    None,
                ),
                Rope::from_str(text),
                state,
                std::sync::Arc::from(text.as_bytes()),
                DocumentMetadata {
                    read_only,
                    ..DocumentMetadata::default()
                },
            )
            .unwrap();
        (registry, id, state)
    }

    fn transaction(id: DocumentId, revision: u64, edits: Vec<TextEdit>) -> EditTransaction {
        EditTransaction {
            document_id: id,
            expected_text_revision: revision,
            edits,
            origin: EditOrigin::UserInput,
            merge_policy: MergePolicy::Adjacent,
        }
    }

    fn text(registry: &DocumentRegistry, id: DocumentId) -> String {
        registry.session(id).unwrap().text().to_string()
    }

    #[test]
    fn edit_transaction_applies_unicode_ranges_and_undo_restores_state_identity() {
        let (mut registry, id, original) = document("a中😀z", false);
        let edited = state(STATE_B);
        let mut change = transaction(
            id,
            0,
            vec![TextEdit::new(1..4, "文"), TextEdit::new(8..9, "")],
        );
        change.origin = EditOrigin::ReplaceAll;
        change.merge_policy = MergePolicy::Disallowed;

        assert_eq!(
            registry.apply(change, edited, Instant::now()),
            Ok(EditApplyOutcome::Applied)
        );
        assert_eq!(text(&registry, id), "a文😀");
        let session = registry.session(id).unwrap();
        assert_eq!(session.text_revision(), 1);
        assert_eq!(session.current_state(), edited);
        assert!(session.is_dirty());

        assert_eq!(registry.undo(id), Ok(EditApplyOutcome::Applied));
        let session = registry.session(id).unwrap();
        assert_eq!(session.text().to_string(), "a中😀z");
        assert_eq!(session.current_state(), original);
        assert!(!session.is_dirty());
        assert_eq!(session.text_revision(), 2);

        assert_eq!(registry.redo(id), Ok(EditApplyOutcome::Applied));
        let session = registry.session(id).unwrap();
        assert_eq!(session.text().to_string(), "a文😀");
        assert_eq!(session.current_state(), edited);
        assert!(session.is_dirty());
        assert_eq!(session.text_revision(), 3);
    }

    #[test]
    fn invalid_transactions_preserve_text_and_revision() {
        let (mut registry, id, _) = document("a中c", false);
        let cases = [
            (
                transaction(id, 9, vec![TextEdit::new(0..1, "x")]),
                EditTransactionError::RevisionMismatch,
            ),
            (
                transaction(id, 0, vec![TextEdit::new(1..2, "x")]),
                EditTransactionError::InvalidRange,
            ),
            (
                transaction(id, 0, vec![TextEdit::new(0..99, "x")]),
                EditTransactionError::InvalidRange,
            ),
            (
                transaction(
                    id,
                    0,
                    vec![TextEdit::new(0..1, "x"), TextEdit::new(0..1, "y")],
                ),
                EditTransactionError::OverlappingEdits,
            ),
            (
                transaction(
                    id,
                    0,
                    vec![TextEdit::new(4..5, "x"), TextEdit::new(0..1, "y")],
                ),
                EditTransactionError::UnorderedEdits,
            ),
        ];
        for (change, expected) in cases {
            assert_eq!(
                registry.apply(change, state(STATE_B), Instant::now()),
                Err(expected)
            );
            assert_eq!(text(&registry, id), "a中c");
            assert_eq!(registry.session(id).unwrap().text_revision(), 0);
        }
        assert_eq!(
            registry.apply(
                transaction(id, 0, Vec::new()),
                state(STATE_B),
                Instant::now(),
            ),
            Ok(EditApplyOutcome::Unchanged)
        );
        assert_eq!(registry.session(id).unwrap().text_revision(), 0);
    }

    #[test]
    fn background_replacement_requires_and_verifies_original_text() {
        let (mut registry, id, _) = document("actual", false);
        let mut missing = transaction(id, 0, vec![TextEdit::new(0..6, "new")]);
        missing.origin = EditOrigin::BackgroundReplacement;
        assert_eq!(
            registry.apply(missing, state(STATE_B), Instant::now()),
            Err(EditTransactionError::MissingOriginalText)
        );
        let mut mismatch = transaction(id, 0, vec![TextEdit::background(0..6, "stale", "new")]);
        mismatch.origin = EditOrigin::BackgroundReplacement;
        mismatch.merge_policy = MergePolicy::Disallowed;
        assert_eq!(
            registry.apply(mismatch, state(STATE_B), Instant::now()),
            Err(EditTransactionError::OriginalTextMismatch)
        );
        assert_eq!(text(&registry, id), "actual");
        assert_eq!(registry.session(id).unwrap().text_revision(), 0);
    }

    #[test]
    fn adjacent_typing_merges_only_inside_the_time_window() {
        let (mut registry, id, _) = document("", false);
        let start = Instant::now();
        for (range, replacement, next, at) in [
            (0..0, "a", state(STATE_B), Duration::ZERO),
            (1..1, "中", state(STATE_C), Duration::from_millis(100)),
            (4..4, "😀", state(STATE_D), Duration::from_millis(800)),
        ] {
            assert_eq!(
                registry.apply(
                    transaction(
                        id,
                        registry.session(id).unwrap().text_revision(),
                        vec![TextEdit::new(range, replacement)],
                    ),
                    next,
                    start + at,
                ),
                Ok(EditApplyOutcome::Applied)
            );
        }

        // 最后一项超过合并窗口，应独立撤销；前两项仍是一组。
        assert_eq!(registry.undo(id), Ok(EditApplyOutcome::Applied));
        assert_eq!(text(&registry, id), "a中");
        assert_eq!(registry.undo(id), Ok(EditApplyOutcome::Applied));
        assert_eq!(text(&registry, id), "");
        assert_eq!(registry.redo(id), Ok(EditApplyOutcome::Applied));
        assert_eq!(text(&registry, id), "a中");
        assert_eq!(registry.redo(id), Ok(EditApplyOutcome::Applied));
        assert_eq!(text(&registry, id), "a中😀");
    }

    #[test]
    fn new_edit_clears_redo_and_new_state_ids_cannot_be_reused() {
        let (mut registry, id, original) = document("", false);
        let first = state(STATE_B);
        registry
            .apply(
                transaction(id, 0, vec![TextEdit::new(0..0, "a")]),
                first,
                Instant::now(),
            )
            .unwrap();
        registry.undo(id).unwrap();
        registry
            .apply(
                transaction(id, 2, vec![TextEdit::new(0..0, "b")]),
                state(STATE_C),
                Instant::now(),
            )
            .unwrap();
        assert_eq!(registry.redo(id), Ok(EditApplyOutcome::Unchanged));

        for reused in [original, first, state(STATE_C)] {
            assert_eq!(
                registry.apply(
                    transaction(id, 3, vec![TextEdit::new(1..1, "x")]),
                    reused,
                    Instant::now(),
                ),
                Err(EditTransactionError::StateConflict)
            );
            assert_eq!(text(&registry, id), "b");
        }
    }

    #[test]
    fn save_completion_preserves_edits_made_while_write_is_in_flight() {
        let (mut registry, id, _) = document("0", false);
        let saved = state(STATE_B);
        registry
            .apply(
                transaction(id, 0, vec![TextEdit::new(1..1, "1")]),
                saved,
                Instant::now(),
            )
            .unwrap();
        let request = registry.begin_save(id, 17).unwrap();
        assert_eq!(request.text().to_string(), "01");
        assert_eq!(request.text_state(), saved);
        assert_eq!(request.disk_baseline().as_ref(), b"0");
        assert_eq!(request.authorization_generation(), 17);

        let current = state(STATE_C);
        registry
            .apply(
                transaction(id, 1, vec![TextEdit::new(2..2, "2")]),
                current,
                Instant::now(),
            )
            .unwrap();
        registry
            .complete_save(DocumentSaveReceipt::new(
                request,
                std::sync::Arc::from(b"01".as_slice()),
                None,
            ))
            .unwrap();

        let session = registry.session(id).unwrap();
        assert_eq!(session.text().to_string(), "012");
        assert_eq!(session.current_state(), current);
        assert_eq!(session.saved_state(), saved);
        assert_eq!(session.disk_baseline().as_ref(), b"01");
        assert!(session.is_dirty());
        assert!(!session.is_saving());
        registry.undo(id).unwrap();
        assert!(!registry.session(id).unwrap().is_dirty());
    }

    #[test]
    fn stale_save_receipt_cannot_clear_a_newer_attempt() {
        let (mut registry, id, _) = document("0", false);
        registry
            .apply(
                transaction(id, 0, vec![TextEdit::new(1..1, "1")]),
                state(STATE_B),
                Instant::now(),
            )
            .unwrap();
        let first = registry.begin_save(id, 3).unwrap();
        assert_eq!(
            registry.begin_save(id, 3),
            Err(crate::SaveTransactionError::SaveInProgress)
        );
        registry.fail_save(id, first.save_generation()).unwrap();
        registry
            .apply(
                transaction(id, 1, vec![TextEdit::new(2..2, "2")]),
                state(STATE_C),
                Instant::now(),
            )
            .unwrap();
        let second = registry.begin_save(id, 4).unwrap();
        let stale = DocumentSaveReceipt::new(first, std::sync::Arc::from(b"01".as_slice()), None);

        assert_eq!(
            registry.complete_save(stale),
            Err(crate::SaveTransactionError::StaleSave)
        );
        let session = registry.session(id).unwrap();
        assert_eq!(session.text().to_string(), "012");
        assert_eq!(session.disk_baseline().as_ref(), b"0");
        assert!(session.is_dirty());
        assert!(session.is_saving());

        registry
            .complete_save(DocumentSaveReceipt::new(
                second,
                std::sync::Arc::from(b"012".as_slice()),
                None,
            ))
            .unwrap();
        assert!(!registry.session(id).unwrap().is_dirty());
        assert_eq!(
            registry.session(id).unwrap().disk_baseline().as_ref(),
            b"012"
        );
    }

    #[test]
    fn save_rejects_read_only_exhaustion_and_stale_failure() {
        let (mut readonly, id, _) = document("fixed", true);
        assert_eq!(
            readonly.begin_save(id, 0),
            Err(crate::SaveTransactionError::ReadOnly)
        );

        let (mut registry, id, _) = document("0", false);
        registry
            .apply(
                transaction(id, 0, vec![TextEdit::new(1..1, "1")]),
                state(STATE_B),
                Instant::now(),
            )
            .unwrap();
        let request = registry.begin_save(id, 0).unwrap();
        assert_eq!(
            registry.fail_save(id, request.save_generation() + 1),
            Err(crate::SaveTransactionError::StaleSave)
        );
        assert!(registry.session(id).unwrap().is_saving());
        registry.fail_save(id, request.save_generation()).unwrap();
        assert!(registry.session(id).unwrap().is_dirty());
        assert_eq!(registry.session(id).unwrap().disk_baseline().as_ref(), b"0");

        registry.session_mut(id).unwrap().save_generation = u64::MAX;
        assert_eq!(
            registry.begin_save(id, 0),
            Err(crate::SaveTransactionError::SaveGenerationExhausted)
        );
        assert!(!registry.session(id).unwrap().is_saving());
    }

    #[test]
    fn saved_state_controls_dirty_across_undo_and_redo() {
        let (mut registry, id, _) = document("0", false);
        let saved = state(STATE_B);
        registry
            .apply(
                transaction(id, 0, vec![TextEdit::new(1..1, "1")]),
                saved,
                Instant::now(),
            )
            .unwrap();
        let request = registry.begin_save(id, 0).unwrap();
        registry
            .complete_save(DocumentSaveReceipt::new(
                request,
                std::sync::Arc::from(b"01".as_slice()),
                None,
            ))
            .unwrap();
        assert!(!registry.session(id).unwrap().is_dirty());
        registry
            .apply(
                transaction(id, 1, vec![TextEdit::new(2..2, "2")]),
                state(STATE_C),
                Instant::now(),
            )
            .unwrap();
        assert!(registry.session(id).unwrap().is_dirty());

        registry.undo(id).unwrap();
        assert_eq!(registry.session(id).unwrap().current_state(), saved);
        assert!(!registry.session(id).unwrap().is_dirty());
        registry.redo(id).unwrap();
        assert!(registry.session(id).unwrap().is_dirty());
    }

    #[test]
    fn read_only_and_exhausted_revisions_reject_changes() {
        let (mut readonly, id, _) = document("fixed", true);
        assert_eq!(
            readonly.apply(
                transaction(id, 0, vec![TextEdit::new(0..0, "x")]),
                state(STATE_B),
                Instant::now(),
            ),
            Err(EditTransactionError::ReadOnly)
        );

        let (mut registry, id, _) = document("fixed", false);
        registry.session_mut(id).unwrap().text_revision = u64::MAX;
        assert_eq!(
            registry.apply(
                transaction(id, u64::MAX, vec![TextEdit::new(0..0, "x")]),
                state(STATE_B),
                Instant::now(),
            ),
            Err(EditTransactionError::RevisionExhausted)
        );
        assert_eq!(
            registry.undo(id),
            Err(EditTransactionError::RevisionExhausted)
        );
        assert_eq!(
            registry.redo(id),
            Err(EditTransactionError::RevisionExhausted)
        );
        assert_eq!(text(&registry, id), "fixed");
    }

    #[test]
    fn missing_document_rejects_every_edit_entrypoint() {
        let mut registry = DocumentRegistry::new();
        let id = DOCUMENT.parse().unwrap();
        let change = transaction(id, 0, vec![TextEdit::new(0..0, "x")]);
        assert_eq!(
            registry.apply(change, state(STATE_B), Instant::now()),
            Err(EditTransactionError::DocumentNotFound(id))
        );
        assert_eq!(
            registry.undo(id),
            Err(EditTransactionError::DocumentNotFound(id))
        );
        assert_eq!(
            registry.redo(id),
            Err(EditTransactionError::DocumentNotFound(id))
        );
        assert_eq!(
            registry.begin_save(id, 0),
            Err(crate::SaveTransactionError::DocumentNotFound(id))
        );
        assert!(registry.is_empty());
    }

    proptest::proptest! {
        #[test]
        fn random_unicode_edits_round_trip_through_undo_and_redo(
            initial in proptest::collection::vec(
                proptest::sample::select(vec!["a", "中", "😀", "\n"]),
                0..12,
            ).prop_map(|parts| parts.join("")),
            operations in proptest::collection::vec(
                (proptest::prelude::any::<u8>(), proptest::prelude::any::<u8>(), proptest::prelude::any::<u8>()),
                1..24,
            ),
        ) {
            let (mut registry, id, _) = document(&initial, false);
            let mut expected = initial.clone();
            for (index, (start, length, replacement)) in operations.iter().copied().enumerate() {
                let char_count = expected.chars().count();
                let start_char = usize::from(start) % (char_count + 1);
                let end_char = start_char + usize::from(length) % (char_count - start_char + 1);
                let utf8_start = expected.char_indices().nth(start_char).map_or(expected.len(), |(offset, _)| offset);
                let utf8_end = expected.char_indices().nth(end_char).map_or(expected.len(), |(offset, _)| offset);
                let before = &expected[utf8_start..utf8_end];
                let mut replacement = ["a", "中", "😀", "\n"][usize::from(replacement) % 4].to_owned();
                if replacement == before {
                    replacement.push('x');
                }
                expected.replace_range(utf8_start..utf8_end, &replacement);
                let revision = registry.session(id).unwrap().text_revision();
                let mut change = transaction(id, revision, vec![TextEdit::new(utf8_start..utf8_end, replacement)]);
                change.origin = EditOrigin::ReplaceAll;
                change.merge_policy = MergePolicy::Disallowed;
                let mut uuid = (index as u128 + 1).to_be_bytes();
                uuid[6] = 0x70;
                uuid[8] = 0x80;
                let next = TextStateId::from_uuid(uuid::Uuid::from_bytes(uuid)).unwrap();
                assert_eq!(registry.apply(change, next, Instant::now()), Ok(EditApplyOutcome::Applied));
                assert_eq!(text(&registry, id), expected);
            }

            for _ in &operations {
                assert_eq!(registry.undo(id), Ok(EditApplyOutcome::Applied));
            }
            assert_eq!(text(&registry, id), initial);
            assert!(!registry.session(id).unwrap().is_dirty());
            for _ in &operations {
                assert_eq!(registry.redo(id), Ok(EditApplyOutcome::Applied));
            }
            assert_eq!(text(&registry, id), expected);
            assert!(registry.session(id).unwrap().is_dirty());
        }
    }

    #[test]
    fn history_budget_evicts_oldest_undo_groups() {
        let (mut registry, id, _) = document("", false);
        let states = [STATE_A, STATE_B, STATE_C, STATE_D];
        let mut texts: Vec<String> = Vec::new();
        for (index, value) in ["a", "b", "c"].into_iter().enumerate() {
            let next = value.repeat(3 * 1024 * 1024);
            let revision = registry.session(id).unwrap().text_revision();
            let mut change = transaction(id, revision, vec![TextEdit::new(0..0, next.clone())]);
            if !texts.is_empty() {
                let previous = texts[index - 1].clone();
                change.edits[0] = TextEdit::new(0..previous.len(), next.clone());
            }
            change.origin = EditOrigin::ReplaceAll;
            change.merge_policy = MergePolicy::Disallowed;
            registry
                .apply(change, state(states[index + 1]), Instant::now())
                .unwrap();
            texts.push(next);
        }

        registry.undo(id).unwrap();
        assert_eq!(text(&registry, id), texts[1]);
        assert_eq!(registry.undo(id), Ok(EditApplyOutcome::Unchanged));
    }
}
