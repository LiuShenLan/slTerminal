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
pub(crate) use view_state::ScrollUnit;
pub(crate) use view_state::{
    DiffCollapsedSide, DiffViewMode, DiffViewState, DocumentViewStateBinding, EditorViewState,
    EditorViewStateLease, EditorViewStateStore, MarkdownMode, MarkdownReadingPosition,
    MarkdownViewState, document_view_state_schemas,
};

gpui::actions!(
    document_text_view,
    [
        SelectLeft,
        SelectRight,
        SelectUp,
        SelectDown,
        SaveFile,
        SaveAsFile,
        FindEditor,
        FindNextEditor,
        FindPreviousEditor,
        CloseEditorSearch,
        GoToLineEditor,
        ToggleFoldEditor,
        CompleteEditor,
        CloseEditorCompletion,
        CloseEditorGoto,
        IndentEditor,
        OutdentEditor
    ]
);

/// 专用文本视图的键盘上下文；复制键与终端中断按上下文独立。
pub(crate) const TEXT_VIEW_CONTEXT: &str = "DocumentText";

/// 查找栏独立上下文：搜索输入获得 Enter／Esc 等导航键，不与正文编辑混用。
pub(crate) const TEXT_VIEW_SEARCH_CONTEXT: &str = "DocumentSearch";

/// 跳转行栏独立上下文：Esc 只关闭本栏，不影响正文。
pub(crate) const TEXT_VIEW_GOTO_CONTEXT: &str = "DocumentGoto";

/// 全局编辑器字号：所有 Document 视图共享，Ctrl+滚轮逐像素调整。
#[cfg(all(test, feature = "gpui-test-support"))]
const DEFAULT_EDITOR_FONT_SIZE_PX: f32 = 14.0;
pub(crate) const MIN_EDITOR_FONT_SIZE_PX: f32 = 8.0;
pub(crate) const MAX_EDITOR_FONT_SIZE_PX: f32 = 32.0;
// 14.0_f32.to_bits() 的稳定常量；避免依赖 const float 转换。
static EDITOR_FONT_SIZE_BITS: std::sync::atomic::AtomicU32 =
    std::sync::atomic::AtomicU32::new(0x4160_0000);

pub(crate) fn editor_font_size_px() -> f32 {
    f32::from_bits(EDITOR_FONT_SIZE_BITS.load(std::sync::atomic::Ordering::Relaxed))
}

/// 字号越界夹取；返回实际生效值。
pub(crate) fn set_editor_font_size_px(size: f32) -> f32 {
    let size = size.clamp(MIN_EDITOR_FONT_SIZE_PX, MAX_EDITOR_FONT_SIZE_PX);
    EDITOR_FONT_SIZE_BITS.store(size.to_bits(), std::sync::atomic::Ordering::Relaxed);
    size
}

#[cfg(all(test, feature = "gpui-test-support"))]
pub(crate) fn reset_editor_font_size_for_test() {
    set_editor_font_size_px(DEFAULT_EDITOR_FONT_SIZE_PX);
}

/// Ctrl+滚轮按整像素步进缩放；方向与滚动一致（向上放大）。
pub(crate) fn zoom_editor_font_size(steps: i32) -> f32 {
    let current = editor_font_size_px();
    set_editor_font_size_px(current + steps as f32)
}

/// 专用文本视图的键盘合同；复制键不与终端中断争用。
pub(crate) fn key_bindings() -> Vec<gpui::KeyBinding> {
    use self::{
        CloseEditorCompletion, CloseEditorGoto, CloseEditorSearch, CompleteEditor, FindEditor,
        FindNextEditor, FindPreviousEditor, GoToLineEditor, IndentEditor, OutdentEditor,
        SaveAsFile, SaveFile, SelectDown, SelectLeft, SelectRight, SelectUp, ToggleFoldEditor,
    };
    use gpui::KeyBinding;
    use gpui_component::input::{
        Backspace, Copy, Delete, Enter, MoveDown, MoveLeft, MoveRight, MoveToEndOfLine,
        MoveToStartOfLine, MoveUp, Paste, Redo, SelectAll, SelectToEndOfLine, SelectToStartOfLine,
        Undo,
    };

    const CONTEXT: &str = TEXT_VIEW_CONTEXT;
    const SEARCH: &str = TEXT_VIEW_SEARCH_CONTEXT;
    const GOTO: &str = TEXT_VIEW_GOTO_CONTEXT;
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
        KeyBinding::new("ctrl-f", FindEditor, Some(CONTEXT)),
        KeyBinding::new("f3", FindNextEditor, Some(CONTEXT)),
        KeyBinding::new("shift-f3", FindPreviousEditor, Some(CONTEXT)),
        // 跳转行、折叠、文档内补全与缩进均属于正文编辑键。
        KeyBinding::new("ctrl-alt-g", GoToLineEditor, Some(CONTEXT)),
        KeyBinding::new("ctrl-shift-[", ToggleFoldEditor, Some(CONTEXT)),
        KeyBinding::new("ctrl-space", CompleteEditor, Some(CONTEXT)),
        KeyBinding::new("tab", IndentEditor, Some(CONTEXT)),
        KeyBinding::new("shift-tab", OutdentEditor, Some(CONTEXT)),
        KeyBinding::new("escape", CloseEditorCompletion, Some(CONTEXT)),
        KeyBinding::new("escape", CloseEditorGoto, Some(GOTO)),
        // 查找栏获得焦点时由独立上下文接管导航与关闭；Ctrl+F 只再次还焦。
        KeyBinding::new("ctrl-f", FindEditor, Some(SEARCH)),
        KeyBinding::new("f3", FindNextEditor, Some(SEARCH)),
        KeyBinding::new("shift-f3", FindPreviousEditor, Some(SEARCH)),
        KeyBinding::new("enter", FindNextEditor, Some(SEARCH)),
        KeyBinding::new("shift-enter", FindPreviousEditor, Some(SEARCH)),
        KeyBinding::new("escape", CloseEditorSearch, Some(SEARCH)),
    ]
}

pub(crate) fn init(cx: &mut gpui::App) {
    cx.bind_keys(key_bindings());
}

#[cfg(test)]
mod keyboard_context_tests {
    use super::{
        CompleteEditor, GoToLineEditor, IndentEditor, OutdentEditor, SaveAsFile, TEXT_VIEW_CONTEXT,
        ToggleFoldEditor, key_bindings,
    };
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

        for (keystroke, matches) in [
            ("ctrl-alt-g", KeymapCase::GoToLine),
            ("ctrl-shift-[", KeymapCase::ToggleFold),
            ("ctrl-space", KeymapCase::Complete),
            ("tab", KeymapCase::Indent),
            ("shift-tab", KeymapCase::Outdent),
        ] {
            let keys = [Keystroke::parse(keystroke).unwrap()];
            let (bindings, _) = keymap.bindings_for_input(&keys, &document);
            assert_eq!(bindings.len(), 1, "{keystroke} 必须唯一绑定");
            let action = bindings[0].action();
            let hit = match matches {
                KeymapCase::GoToLine => action.as_any().is::<GoToLineEditor>(),
                KeymapCase::ToggleFold => action.as_any().is::<ToggleFoldEditor>(),
                KeymapCase::Complete => action.as_any().is::<CompleteEditor>(),
                KeymapCase::Indent => action.as_any().is::<IndentEditor>(),
                KeymapCase::Outdent => action.as_any().is::<OutdentEditor>(),
            };
            assert!(hit, "{keystroke} 命中了错误动作");
        }
    }

    enum KeymapCase {
        GoToLine,
        ToggleFold,
        Complete,
        Indent,
        Outdent,
    }
}
