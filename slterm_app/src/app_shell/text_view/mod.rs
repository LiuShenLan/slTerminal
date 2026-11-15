#[allow(dead_code)] // 消费点 = Document 编辑器生产接线。
mod editor;
#[allow(dead_code)] // 消费点 = 专用视图布局缓存。
mod layout;
#[allow(dead_code)] // 消费点 = 专用视图选区、命中测试与 IME。
mod metrics;
#[allow(dead_code)] // 消费点 = 专用视图可见行渲染。
mod source;

gpui::actions!(
    document_text_view,
    [SelectLeft, SelectRight, SelectUp, SelectDown]
);

/// 专用文本视图的键盘合同；复制键不与终端中断争用。
pub(crate) fn init(cx: &mut gpui::App) {
    use self::{SelectDown, SelectLeft, SelectRight, SelectUp};
    use gpui::KeyBinding;
    use gpui_component::input::{
        Backspace, Delete, Enter, MoveDown, MoveLeft, MoveRight, MoveToEndOfLine,
        MoveToStartOfLine, MoveUp, Redo, SelectAll, SelectToEndOfLine, SelectToStartOfLine, Undo,
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
    ]);
}
