mod editor;
pub(crate) use editor::{
    DocumentEditRequest, DocumentEditResponse, DocumentTextEvent, DocumentTextSnapshot,
    DocumentTextView,
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
pub(crate) use view_state::{EditorViewState, EditorViewStateStore, editor_view_state_schema};

gpui::actions!(
    document_text_view,
    [SelectLeft, SelectRight, SelectUp, SelectDown, SaveFile]
);

/// 专用文本视图的键盘合同；复制键不与终端中断争用。
pub(crate) fn init(cx: &mut gpui::App) {
    use self::{SaveFile, SelectDown, SelectLeft, SelectRight, SelectUp};
    use gpui::KeyBinding;
    use gpui_component::input::{
        Backspace, Copy, Delete, Enter, MoveDown, MoveLeft, MoveRight, MoveToEndOfLine,
        MoveToStartOfLine, MoveUp, Paste, Redo, SelectAll, SelectToEndOfLine, SelectToStartOfLine,
        Undo,
    };

    const CONTEXT: &str = "DocumentText";
    cx.bind_keys([
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
    ]);
}
