//! 带原文身份的可复核补全编辑。

use sha2::{Digest as _, Sha256};

use crate::Span;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompletionEdit {
    expected_input_revision: u64,
    span: Span,
    original_fingerprint: [u8; 32],
    insert: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompletionEditError {
    InvalidSpan,
    RevisionMismatch,
    OriginalMismatch,
}

impl CompletionEdit {
    pub fn new(
        original: &str,
        expected_input_revision: u64,
        span: Span,
        insert: String,
    ) -> Result<Self, CompletionEditError> {
        if !valid_span(original, span) {
            return Err(CompletionEditError::InvalidSpan);
        }
        Ok(Self {
            expected_input_revision,
            span,
            original_fingerprint: fingerprint(original),
            insert,
        })
    }

    pub const fn expected_input_revision(&self) -> u64 {
        self.expected_input_revision
    }

    pub const fn span(&self) -> Span {
        self.span
    }

    pub fn insert(&self) -> &str {
        &self.insert
    }

    /// 只有在当前行与创建时的原文及输入代次完全一致时才产生新行。
    pub fn apply(
        &self,
        current: &str,
        current_revision: u64,
    ) -> Result<String, CompletionEditError> {
        if current_revision != self.expected_input_revision {
            return Err(CompletionEditError::RevisionMismatch);
        }
        if fingerprint(current) != self.original_fingerprint {
            return Err(CompletionEditError::OriginalMismatch);
        }
        if !valid_span(current, self.span) {
            return Err(CompletionEditError::InvalidSpan);
        }
        let mut result = String::with_capacity(current.len() + self.insert.len());
        result.push_str(&current[..self.span.start]);
        result.push_str(&self.insert);
        result.push_str(&current[self.span.end..]);
        Ok(result)
    }
}

fn valid_span(text: &str, span: Span) -> bool {
    span.start <= span.end
        && span.end <= text.len()
        && text.is_char_boundary(span.start)
        && text.is_char_boundary(span.end)
}

fn fingerprint(text: &str) -> [u8; 32] {
    Sha256::digest(text.as_bytes()).into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Span;

    #[test]
    fn completion_edit_rejects_stale_revision_and_original_text() {
        let edit = CompletionEdit::new("echo 中", 7, Span::new(5, 8), "界".into()).unwrap();

        assert_eq!(edit.apply("echo 中", 7).unwrap(), "echo 界");
        assert_eq!(
            edit.apply("echo 中", 8),
            Err(CompletionEditError::RevisionMismatch)
        );
        assert_eq!(
            edit.apply("echo 旧", 7),
            Err(CompletionEditError::OriginalMismatch)
        );
        assert!(CompletionEdit::new("echo 中", 7, Span::new(5, 6), "x".into()).is_err());
    }
}
