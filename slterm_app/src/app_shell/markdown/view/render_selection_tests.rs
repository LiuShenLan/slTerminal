use std::str::FromStr;
use std::sync::Arc;

use gpui::{
    AppContext as _, Entity, Modifiers, MouseButton, Render, TestAppContext, VisualTestContext,
};
use slterm_workspace::ViewKey;

use super::*;
use crate::app_shell::markdown::MarkdownRenderServices;

struct SelectionHost {
    view: Entity<MarkdownView>,
    document: MarkdownDocument,
}

impl Render for SelectionHost {
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let owner = self.view.downgrade();
        let copy_owner = owner.clone();
        let focus = self.view.read(cx).focus_handle(cx);
        let block = self.document.blocks().first().unwrap();
        let content = if matches!(block.kind(), MarkdownBlockKind::Code(_)) {
            render_block(
                block,
                &self.document,
                0,
                1.0,
                &RefCell::default(),
                &owner,
                window,
                cx,
            )
        } else {
            rich_text(
                SharedString::from("markdown-inline-0"),
                block.inlines(),
                block,
                &self.document,
                &owner,
                window,
                cx,
            )
        };
        div()
            .size_full()
            .key_context("MarkdownView")
            .track_focus(&focus)
            .on_action(
                move |_: &gpui_component::input::Copy, _, cx: &mut App| {
                    let _ = copy_owner.update(cx, |view, cx| view.copy_rendered_selection(cx));
                },
            )
            .child(content)
    }
}

fn selection_view(cx: &mut TestAppContext, source: &str) -> Entity<MarkdownView> {
    cx.update(|cx| {
        gpui_component::init(cx);
        cx.set_global(Settings::default());
        crate::app_shell::theme::apply_dark_theme(cx);
        crate::app_shell::markdown::init(cx);
        cx.write_to_clipboard(gpui::ClipboardItem::new_string("sentinel".to_owned()));
    });
    let document_id =
        slterm_document::DocumentId::from_str("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2a").unwrap();
    let mut registry = slterm_document::DocumentRegistry::new();
    registry
        .open(
            document_id,
            slterm_path::ResolvedPath::new(
                slterm_path::CanonicalPath::new("D:/work/selection.md").unwrap(),
                None,
            ),
            ropey::Rope::from_str(source),
            slterm_document::TextStateId::from_uuid(uuid::Uuid::now_v7()).unwrap(),
            Arc::from(source.as_bytes()),
            slterm_document::DocumentMetadata::default(),
        )
        .unwrap();
    let session = registry.session(document_id).unwrap().clone();
    cx.new(|cx| {
        MarkdownView::new(
            &session,
            Box::new(|_, _, _| unreachable!("只读选择不得编辑正文")),
            None,
            MarkdownRenderServices::default(),
            Vec::new(),
            crate::app_shell::text_view::DocumentViewStateBinding::new(
                ViewKey::Markdown {
                    project_id: slterm_workspace::ProjectId::from_str(
                        "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b",
                    )
                    .unwrap(),
                    document_id,
                },
                crate::app_shell::text_view::EditorViewStateStore::transient(),
            ),
            cx,
        )
    })
}

fn open_selection_window(
    cx: &mut TestAppContext,
    view: Entity<MarkdownView>,
    source: &str,
) -> VisualTestContext {
    cx.run_until_parked();
    let document = MarkdownDocument::parse(source);
    let window = cx.open_window(gpui::size(px(800.0), px(600.0)), |_, _| SelectionHost {
        view,
        document,
    });
    let mut window = VisualTestContext::from_window(window.into(), cx);
    window.run_until_parked();
    window.refresh().unwrap();
    window.update(|window, cx| {
        window.draw(cx).clear(cx);
    });
    window
}

fn drag_copy_first_two_characters(
    window: &mut VisualTestContext,
    view: &Entity<MarkdownView>,
    selector: &'static str,
) {
    let bounds = window.debug_bounds(selector).unwrap();
    assert!(bounds.size.width > px(0.0));
    let layout_id = selector.strip_prefix("markdown-selection-").unwrap();
    let start = view
        .read_with(window, |view, _| view.selection_position_for_test(layout_id, 0))
        .unwrap();
    let end = view
        .read_with(window, |view, _| view.selection_position_for_test(layout_id, 2))
        .unwrap();

    assert_eq!(
        view.read_with(window, |view, _| view.selection_hit_for_test(layout_id, start)),
        Some(0)
    );
    let local_end = view.read_with(window, |view, _| {
        view.selection_local_hit_for_test(layout_id, end)
    });
    assert!(
        matches!(local_end, Ok(2) | Err(2)),
        "实际局部命中={local_end:?}, start={start:?}, end={end:?}"
    );
    assert_eq!(
        view.read_with(window, |view, _| view.selection_hit_for_test(layout_id, end)),
        Some(2)
    );
    window.simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
    window.simulate_mouse_move(end, MouseButton::Left, Modifiers::default());
    window.simulate_mouse_up(end, MouseButton::Left, Modifiers::default());
    assert_eq!(
        view.read_with(window, |view, _| view.selection_range_for_test()),
        Some(0..2),
        "鼠标拖选必须覆盖实际可见文本"
    );
    window.refresh().unwrap();
    window.update(|window, cx| {
        window.draw(cx).clear(cx);
    });
    window.simulate_keystrokes("ctrl-shift-c");
}

#[gpui::test]
fn markdown_rendered_text_drag_selection_copies_visible_text(cx: &mut TestAppContext) {
    let view = selection_view(cx, "old\n");
    let mut window = open_selection_window(cx, view.clone(), "old\n");
    drag_copy_first_two_characters(&mut window, &view, "markdown-selection-markdown-inline-0");
    assert_eq!(
        cx.update(|cx| cx.read_from_clipboard().and_then(|item| item.text())),
        Some("ol".to_owned()),
        "拖选后必须复制实际绘制的可见文本"
    );
}

#[gpui::test]
fn markdown_code_block_drag_selection_copies_visible_source(cx: &mut TestAppContext) {
    let source = "```rust\nold\n```\n";
    let view = selection_view(cx, source);
    let mut window = open_selection_window(cx, view.clone(), source);
    drag_copy_first_two_characters(&mut window, &view, "markdown-selection-markdown-code-0");
    assert_eq!(
        cx.update(|cx| cx.read_from_clipboard().and_then(|item| item.text())),
        Some("ol".to_owned()),
        "代码块拖选必须复制真实可见源码"
    );
}
