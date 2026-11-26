//! DocumentTextView 的构造配置；固定快照与实时 Document 共用同一初始化链。

use std::cell::RefCell;
use std::rc::Rc;

use gpui::Context;
use ropey::LineType;
use slterm_document::{DocumentId, DocumentSession, EditTransactionError, LanguageId};
use slterm_workspace::ViewKey;

use super::super::source::TextSelection;
use super::super::syntax::DocumentSyntax;
use super::super::view_state::{DocumentViewStateBinding, EditorViewStateStore};
use super::super::wrap::CaretAffinity;
use super::{DocumentEditSink, DocumentTextSnapshot, DocumentTextView};

struct DocumentTextViewConfig {
    snapshot: DocumentTextSnapshot,
    document_id: DocumentId,
    edit_sink: DocumentEditSink,
    automatic_language: LanguageId,
    allow_language_override: bool,
    syntax: Option<Rc<RefCell<DocumentSyntax>>>,
    view_state: Option<DocumentViewStateBinding>,
}

impl DocumentTextView {
    pub(crate) fn new(
        session: &DocumentSession,
        edit_sink: DocumentEditSink,
        automatic_language: LanguageId,
        syntax: Option<Rc<RefCell<DocumentSyntax>>>,
        view_key: ViewKey,
        view_states: EditorViewStateStore,
        cx: &mut Context<'_, Self>,
    ) -> Self {
        Self::new_with_binding(
            session,
            edit_sink,
            automatic_language,
            true,
            syntax,
            DocumentViewStateBinding::new(view_key, view_states),
            cx,
        )
    }

    /// 复合视图与内部源码子视图共享同一 ViewKey 租约，避免互相判旧。
    pub(crate) fn new_with_binding(
        session: &DocumentSession,
        edit_sink: DocumentEditSink,
        automatic_language: LanguageId,
        allow_language_override: bool,
        syntax: Option<Rc<RefCell<DocumentSyntax>>>,
        view_state: DocumentViewStateBinding,
        cx: &mut Context<'_, Self>,
    ) -> Self {
        Self::from_parts(
            DocumentTextViewConfig {
                snapshot: DocumentTextSnapshot::from_session(session),
                document_id: session.id(),
                edit_sink,
                automatic_language,
                allow_language_override,
                syntax,
                view_state: Some(view_state),
            },
            cx,
        )
    }

    /// 固定只读快照不注册 Document，也不持有 Editor ViewState 租约。
    pub(crate) fn fixed(
        snapshot: DocumentTextSnapshot,
        document_id: DocumentId,
        automatic_language: LanguageId,
        cx: &mut Context<'_, Self>,
    ) -> Self {
        Self::from_parts(
            DocumentTextViewConfig {
                snapshot: DocumentTextSnapshot {
                    read_only: true,
                    ..snapshot
                },
                document_id,
                edit_sink: Box::new(|_, _, _| Err(EditTransactionError::ReadOnly)),
                automatic_language,
                allow_language_override: false,
                syntax: None,
                view_state: None,
            },
            cx,
        )
    }

    fn from_parts(config: DocumentTextViewConfig, cx: &mut Context<'_, Self>) -> Self {
        let DocumentTextViewConfig {
            snapshot,
            document_id,
            edit_sink,
            automatic_language,
            allow_language_override,
            syntax,
            view_state,
        } = config;
        let restored = view_state
            .as_ref()
            .map(|state| state.view_states().state(state.view_key()))
            .unwrap_or_default();
        let (view_lease, view_states) = view_state.map_or((None, None), |state| {
            let (_key, states, lease) = state.into_parts();
            (Some(lease), Some(states))
        });
        let wrap = restored.wrap();
        let scroll_line = if wrap {
            // 显示行总数依赖首个视口宽度，首帧索引建立后再夹紧。
            restored.scroll_line()
        } else {
            restored
                .scroll_line()
                .min(snapshot.text.len_lines(LineType::LF) - 1)
        };
        let mut view = Self {
            document_id,
            view_lease,
            view_states,
            view_state_dirty: false,
            text: snapshot.text,
            revision: snapshot.revision,
            text_state: snapshot.text_state,
            read_only: snapshot.read_only,
            edit_sink,
            automatic_language,
            allow_language_override,
            language_override: if allow_language_override {
                restored.language_override()
            } else {
                None
            },
            git_marks: Vec::new(),
            language_revision: 0,
            theme_revision: 0,
            theme_fingerprint: None,
            layout_cache: Rc::default(),
            syntax,
            applied_syntax: None,
            syntax_poll_task: None,
            selection: TextSelection { anchor: 0, head: 0 },
            preferred_column: None,
            marked_range: None,
            scroll_line,
            scroll_column: if wrap { 0 } else { restored.scroll_column() },
            wrap,
            wrap_index: Rc::default(),
            wrap_build: Rc::default(),
            caret_affinity: CaretAffinity::AfterWrap,
            wrap_reveal_cursor: false,
            git_gutter: true,
            wrap_reveal_line: None,
            wheel_remainder: 0.0,
            mouse_anchor: None,
            focus: cx.focus_handle().tab_stop(true),
            layout: Rc::default(),
            #[cfg(all(test, feature = "gpui-test-support"))]
            painted_syntax: Rc::default(),
            #[cfg(all(test, feature = "gpui-test-support"))]
            painted_lines: Rc::default(),
            #[cfg(all(test, feature = "gpui-test-support"))]
            painted_cursors: Rc::default(),
            #[cfg(test)]
            painted_content_mask: Rc::default(),
        };
        view.schedule_syntax_poll(cx);
        view
    }
}
