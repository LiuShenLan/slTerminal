mod editor;
pub(crate) use editor::{
    DocumentEditRequest, DocumentEditResponse, DocumentEditSink, DocumentTextEvent,
    DocumentTextSnapshot, DocumentTextView, GitMark, GitMarkKind,
};
#[allow(dead_code)] // 消费点 = 专用视图布局缓存。
mod layout;
#[allow(dead_code)] // 消费点 = 专用视图选区、命中测试与 IME。
mod metrics;
mod paged;
pub(crate) use paged::PagedTextView;
#[allow(dead_code)] // 消费点 = 专用视图可见行渲染。
mod source;
mod syntax;
pub(crate) use syntax::DocumentSyntax;
mod view_state;
mod wrap;
#[cfg(all(test, feature = "gpui-test-support"))]
pub(crate) use view_state::{DiffCollapsedSide, DiffViewMode};
pub(crate) use view_state::{
    DiffViewState, DocumentViewStateBinding, EditorViewState, EditorViewStateLease,
    EditorViewStateStore, MarkdownMode, MarkdownReadingPosition, MarkdownViewState,
    document_view_state_schemas,
};

gpui::actions!(
    document_text_view,
    [
        SelectLeft,
        SelectRight,
        SelectUp,
        SelectDown,
        SaveFile,
        SaveAsFile
    ]
);

/// 专用文本视图的键盘上下文；复制键与终端中断按上下文独立。
pub(crate) const TEXT_VIEW_CONTEXT: &str = "DocumentText";

/// 专用文本视图的键盘合同；复制键不与终端中断争用。
pub(crate) fn key_bindings() -> Vec<gpui::KeyBinding> {
    use self::{SaveAsFile, SaveFile, SelectDown, SelectLeft, SelectRight, SelectUp};
    use gpui::KeyBinding;
    use gpui_component::input::{
        Backspace, Copy, Delete, Enter, MoveDown, MoveLeft, MoveRight, MoveToEndOfLine,
        MoveToStartOfLine, MoveUp, Paste, Redo, SelectAll, SelectToEndOfLine, SelectToStartOfLine,
        Undo,
    };

    const CONTEXT: &str = TEXT_VIEW_CONTEXT;
    vec![
        KeyBinding::new("left", MoveLeft, Some(CONTEXT)),
        KeyBinding::new("right", MoveRight, Some(CONTEXT)),
        KeyBinding::new("up", MoveUp, Some(CONTEXT)),
        KeyBinding::new("down", MoveDown, Some(CONTEXT)),
        KeyBinding::new("shift-left", SelectLeft, Some(CONTEXT)),
        KeyBinding::new("shift-right", SelectRight, Some(CONTEXT)),
        KeyBinding::new("shift-up", SelectUp, Some(CONTEXT)),
        KeyBinding::new("shift-down", SelectDown, Some(CONTEXT)),
        KeyBinding::new("home", MoveToStartOfLine, Some(CONTEXT)),
        KeyBinding::new("end", MoveToEndOfLine, Some(CONTEXT)),
        KeyBinding::new("shift-home", SelectToStartOfLine, Some(CONTEXT)),
        KeyBinding::new("shift-end", SelectToEndOfLine, Some(CONTEXT)),
        KeyBinding::new("backspace", Backspace, Some(CONTEXT)),
        KeyBinding::new("delete", Delete, Some(CONTEXT)),
        KeyBinding::new(
            "enter",
            Enter {
                secondary: false,
                shift: false,
            },
            Some(CONTEXT),
        ),
        KeyBinding::new(
            "shift-enter",
            Enter {
                secondary: false,
                shift: true,
            },
            Some(CONTEXT),
        ),
        KeyBinding::new("ctrl-a", SelectAll, Some(CONTEXT)),
        KeyBinding::new("ctrl-z", Undo, Some(CONTEXT)),
        KeyBinding::new("ctrl-y", Redo, Some(CONTEXT)),
        KeyBinding::new("ctrl-shift-z", Redo, Some(CONTEXT)),
        KeyBinding::new("ctrl-shift-c", Copy, Some(CONTEXT)),
        KeyBinding::new("ctrl-shift-v", Paste, Some(CONTEXT)),
        KeyBinding::new("ctrl-s", SaveFile, Some(CONTEXT)),
        KeyBinding::new("ctrl-shift-s", SaveAsFile, Some(CONTEXT)),
    ]
}

pub(crate) fn init(cx: &mut gpui::App) {
    cx.bind_keys(key_bindings());
}

#[cfg(test)]
mod keyboard_context_tests {
    use super::{SaveAsFile, TEXT_VIEW_CONTEXT, key_bindings};
    use gpui::{KeyContext, Keymap, Keystroke};
    use gpui_component::input::Copy;

    /// 复制键按上下文独立绑定：文本视图命中 Copy，终端上下文不命中；
    /// Ctrl+C 在任何文本视图绑定中都不出现，留给终端中断。
    #[test]
    fn keybinding_reserved_contexts() {
        let keymap = Keymap::new(key_bindings());
        let document = [KeyContext::parse(TEXT_VIEW_CONTEXT).unwrap()];
        let terminal = [KeyContext::parse("SltermTerminal").unwrap()];
        let copy = [Keystroke::parse("ctrl-shift-c").unwrap()];

        let (bindings, pending) = keymap.bindings_for_input(&copy, &document);
        assert!(!pending);
        assert_eq!(bindings.len(), 1);
        assert!(
            bindings[0].action().as_any().is::<Copy>(),
            "文本视图必须命中复制动作"
        );
        let (bindings, _) = keymap.bindings_for_input(&copy, &terminal);
        assert!(bindings.is_empty(), "文本复制绑定不得在终端上下文命中");

        let interrupt = [Keystroke::parse("ctrl-c").unwrap()];
        let (bindings, _) = keymap.bindings_for_input(&interrupt, &document);
        assert!(
            bindings.is_empty(),
            "Ctrl+C 不得进入文本视图绑定，必须保留终端中断语义"
        );

        let save_as = [Keystroke::parse("ctrl-shift-s").unwrap()];
        let (bindings, _) = keymap.bindings_for_input(&save_as, &document);
        assert_eq!(bindings.len(), 1);
        assert!(
            bindings[0].action().as_any().is::<SaveAsFile>(),
            "Ctrl+Shift+S 必须命中另存为动作"
        );
    }
}
