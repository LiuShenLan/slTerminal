//! Document 共享的语法 worker、最新树与可见行高亮计算。

use std::ops::Range;
use std::sync::mpsc::TryRecvError;

use gpui::Hsla;
use gpui_component::ThemeColor;
use ropey::Rope;
use slterm_document::{
    DocumentSession, EditTransaction, LanguageId, SyntaxHighlighter, SyntaxTree, SyntaxWorker,
    SyntaxWorkerError, syntax_transaction_input_edits,
};

/// 一个 Document + LanguageId 的语法共享体；同语言视图只读同一最新树。
pub(crate) struct DocumentSyntax {
    worker: SyntaxWorker,
    highlighter: SyntaxHighlighter,
    tree: Option<SyntaxTree>,
    revision: u64,
    closed: bool,
}

impl DocumentSyntax {
    pub(crate) fn new(session: &DocumentSession, language: LanguageId) -> Option<Self> {
        if language == LanguageId::PlainText {
            return None;
        }
        let text = session.text()?.clone();
        let worker =
            SyntaxWorker::new(session.id(), language, text, session.text_revision()).ok()?;
        let highlighter = SyntaxHighlighter::new(language).ok()?;
        Some(Self {
            worker,
            highlighter,
            tree: None,
            revision: session.text_revision(),
            closed: false,
        })
    }

    /// 非阻塞接收当前代次；只有新树被采纳才触发布局失效。
    pub(crate) fn poll(&mut self) -> bool {
        if self.closed {
            return false;
        }
        if let Err(SyntaxWorkerError::Closed) = self.worker.flush() {
            self.closed = true;
            self.tree = None;
            return true;
        }
        let mut changed = false;
        loop {
            match self.worker.try_recv() {
                Ok(Some(tree)) => {
                    self.tree = Some(tree);
                    changed = true;
                }
                Ok(None) => return changed,
                Err(TryRecvError::Empty) => return changed,
                Err(TryRecvError::Disconnected) => {
                    self.closed = true;
                    self.tree = None;
                    return true;
                }
            }
        }
    }

    /// 当前共享树的稳定标识；每个视图据此独立失效自己的布局。
    pub(crate) fn current_marker(&self) -> Option<(u64, u64)> {
        self.tree
            .as_ref()
            .map(|tree| (tree.revision(), tree.generation()))
    }

    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn revision_for_test(&self) -> u64 {
        self.revision
    }

    /// 当前 revision 已有树，或 worker 已关闭时停止轮询。
    pub(crate) fn is_resolved(&self) -> bool {
        self.closed
            || self
                .tree
                .as_ref()
                .is_some_and(|tree| tree.revision() == self.revision)
    }

    /// 只高亮与视图当前 revision 一致的可见字节范围，返回行内坐标。
    pub(crate) fn highlight_line(
        &self,
        text: &Rope,
        line: Range<usize>,
        revision: u64,
    ) -> Vec<(Range<usize>, String)> {
        let Some(tree) = self
            .tree
            .as_ref()
            .filter(|tree| tree.revision() == revision)
        else {
            return Vec::new();
        };
        let start = line.start;
        self.highlighter
            .highlight_range(text, tree.tree(), line)
            .unwrap_or_default()
            .into_iter()
            .map(|span| {
                let range = span.range();
                (
                    range.start - start..range.end - start,
                    span.capture().to_owned(),
                )
            })
            .collect()
    }

    /// 只对与当前 revision 一致的共享树执行当前语言的 fold query。
    pub(crate) fn fold_ranges(
        &self,
        text: &Rope,
        revision: u64,
    ) -> Vec<slterm_document::FoldRange> {
        let Some(tree) = self
            .tree
            .as_ref()
            .filter(|tree| tree.revision() == revision)
        else {
            return Vec::new();
        };
        self.highlighter
            .fold_ranges(text, tree.tree())
            .unwrap_or_default()
    }

    /// 同一 Document 的多视图可能收到同一回执；只提交一次新 revision。
    pub(crate) fn edit(
        &mut self,
        before: &Rope,
        after: Rope,
        revision: u64,
        transaction: &EditTransaction,
    ) {
        if revision <= self.revision || self.closed {
            return;
        }
        if transaction.expected_text_revision != self.revision {
            self.rebuild(after, revision);
            return;
        }
        let edits = syntax_transaction_input_edits(before, transaction);
        self.revision = revision;
        let result = self.worker.edit(after, revision, edits);
        self.handle(result);
    }

    pub(crate) fn rebuild(&mut self, text: Rope, revision: u64) {
        if revision <= self.revision || self.closed {
            return;
        }
        self.revision = revision;
        let result = self.worker.rebuild(text, revision);
        self.handle(result);
    }

    fn handle(&mut self, result: Result<(), SyntaxWorkerError>) {
        if matches!(result, Err(SyntaxWorkerError::Closed)) {
            self.closed = true;
            self.tree = None;
        }
    }
}

/// 查询 capture 到既有主题语义 token 的唯一映射，不引入第二套色值。
pub(crate) fn capture_color(theme: &ThemeColor, capture: &str) -> Hsla {
    if capture.starts_with("comment") || capture.starts_with("documentation") {
        theme.muted_foreground
    } else if capture.starts_with("string")
        || capture.starts_with("character")
        || capture.starts_with("escape")
    {
        theme.success_foreground
    } else if capture.starts_with("number")
        || capture.starts_with("float")
        || capture.starts_with("boolean")
        || capture.starts_with("constant")
    {
        theme.warning_foreground
    } else if capture.starts_with("function")
        || capture.starts_with("constructor")
        || capture.starts_with("macro")
        || capture.starts_with("type")
        || capture.starts_with("class")
        || capture.starts_with("namespace")
    {
        theme.info_foreground
    } else if capture.starts_with("keyword")
        || capture.starts_with("storage")
        || capture.starts_with("modifier")
    {
        theme.accent
    } else if capture.starts_with("operator")
        || capture.starts_with("punctuation")
        || capture.starts_with("bracket")
    {
        theme.muted_foreground
    } else {
        theme.foreground
    }
}

#[cfg(test)]
mod tests {
    use gpui::hsla;
    use gpui_component::ThemeColor;
    use ropey::Rope;
    use slterm_document::{
        EditOrigin, EditTransaction, MergePolicy, TextEdit, syntax_transaction_input_edits,
    };

    use super::capture_color;

    #[test]
    fn capture_names_use_existing_semantic_theme_tokens() {
        let theme = ThemeColor {
            foreground: hsla(0.01, 0.2, 0.3, 1.0),
            accent: hsla(0.11, 0.2, 0.3, 1.0),
            muted_foreground: hsla(0.21, 0.2, 0.3, 1.0),
            success_foreground: hsla(0.31, 0.2, 0.3, 1.0),
            warning_foreground: hsla(0.41, 0.2, 0.3, 1.0),
            info_foreground: hsla(0.51, 0.2, 0.3, 1.0),
            ..ThemeColor::default()
        };
        assert_eq!(capture_color(&theme, "keyword"), theme.accent);
        assert_eq!(
            capture_color(&theme, "comment.line"),
            theme.muted_foreground
        );
        assert_eq!(capture_color(&theme, "string"), theme.success_foreground);
        assert_eq!(capture_color(&theme, "number"), theme.warning_foreground);
        assert_eq!(capture_color(&theme, "function"), theme.info_foreground);
        assert_eq!(capture_color(&theme, "variable"), theme.foreground);
    }

    #[test]
    fn transaction_edits_follow_descending_application_order() {
        let before = Rope::from_str("abcd");
        let transaction = EditTransaction {
            document_id: "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0f2a".parse().unwrap(),
            expected_text_revision: 0,
            edits: vec![TextEdit::new(0..1, "A"), TextEdit::new(3..4, "D")],
            origin: EditOrigin::UserInput,
            merge_policy: MergePolicy::Disallowed,
        };
        let edits = syntax_transaction_input_edits(&before, &transaction);
        assert_eq!(edits.len(), 2);
        assert_eq!(edits[0].start_byte, 3);
        assert_eq!(edits[0].new_end_byte, 4);
        assert_eq!(edits[1].start_byte, 0);
        assert_eq!(edits[1].new_end_byte, 1);
    }
}
