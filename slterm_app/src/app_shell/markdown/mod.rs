//! Markdown 安全资源的解码与本地授权边界。

mod decoder;
mod formula;
pub(crate) mod link;
mod local_resource;
mod progressive;
mod remote_resource;
mod view;

pub(crate) use view::{MarkdownRenderServices, MarkdownView, MarkdownViewEvent};

gpui::actions!(
    markdown_view,
    [
        ToggleMarkdownMode,
        ToggleMarkdownOutline,
        FindMarkdown,
        FindNextMarkdown,
        FindPreviousMarkdown,
        CloseMarkdownSearch,
        MarkdownZoomIn,
        MarkdownZoomOut,
        MarkdownZoomReset,
        RenderLargeMarkdown
    ]
);

pub(crate) fn init(cx: &mut gpui::App) {
    use gpui::KeyBinding;
    cx.bind_keys([
        KeyBinding::new("ctrl-/", ToggleMarkdownMode, Some("MarkdownView")),
        KeyBinding::new("ctrl-f", FindMarkdown, Some("MarkdownView")),
        KeyBinding::new("enter", FindNextMarkdown, Some("MarkdownSearch")),
        KeyBinding::new("shift-enter", FindPreviousMarkdown, Some("MarkdownSearch")),
        KeyBinding::new("escape", CloseMarkdownSearch, Some("MarkdownSearch")),
        KeyBinding::new("ctrl-=", MarkdownZoomIn, Some("MarkdownView")),
        KeyBinding::new("ctrl-shift-=", MarkdownZoomIn, Some("MarkdownView")),
        KeyBinding::new("ctrl--", MarkdownZoomOut, Some("MarkdownView")),
        KeyBinding::new("ctrl-0", MarkdownZoomReset, Some("MarkdownView")),
        KeyBinding::new(
            "ctrl-s",
            crate::app_shell::text_view::SaveFile,
            Some("MarkdownView"),
        ),
        KeyBinding::new(
            "ctrl-shift-c",
            gpui_component::input::Copy,
            Some("MarkdownView"),
        ),
        KeyBinding::new(
            "ctrl-a",
            gpui_component::input::SelectAll,
            Some("MarkdownView"),
        ),
    ]);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MarkdownResourceError {
    Unauthorized,
    Read,
    Changed,
    TooLarge,
    Unsafe,
    Decode,
}
