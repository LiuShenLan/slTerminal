use std::str::FromStr;
use std::sync::Arc;

use gpui::{
    AppContext as _, Entity, Modifiers, MouseButton, Render, TestAppContext, VisualTestContext,
};
use slterm_workspace::ViewKey;

use super::*;
use crate::app_shell::markdown::MarkdownRenderServices;

fn bitmap(red: u8) -> std::sync::Arc<gpui::RenderImage> {
    std::sync::Arc::new(gpui::RenderImage::new([image::Frame::new(
        image::RgbaImage::from_pixel(1, 1, image::Rgba([red, 0, 0, 255])),
    )]))
}

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
        // 全部块走生产 render_block 路径，纯文本块内部仍是 rich_text
        let content = render_block(
            block,
            &self.document,
            0,
            1.0,
            &RefCell::default(),
            &owner,
            window,
            cx,
        );
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

/// 行内图片必须留在文本流中：前后文本独立成段，图片盒子位于两者之间同一行。
#[gpui::test]
fn markdown_inline_image_stays_between_surrounding_text(cx: &mut TestAppContext) {
    let source = "前 ![图](https://example.com/a.png) 后\n";
    let view = selection_view(cx, source);
    let mut window = open_selection_window(cx, view.clone(), source);
    let ids = view.read_with(&window, |view, _| view.text_layout_ids_for_test());
    assert!(ids.contains(&"markdown-inline-0".to_owned()), "{ids:?}");
    assert!(ids.contains(&"markdown-inline-0-0".to_owned()), "{ids:?}");
    let before = window
        .debug_bounds("markdown-selection-markdown-inline-0")
        .expect("前段文本必须绘制");
    let image = window
        .debug_bounds("markdown-image-https://example.com/a.png")
        .expect("行内图片盒子必须绘制");
    let after = window
        .debug_bounds("markdown-selection-markdown-inline-0-0")
        .expect("后段文本必须绘制");
    assert!(
        before.origin.x < image.origin.x && image.origin.x < after.origin.x,
        "顺序必须为 文本→图片→文本：{before:?} {image:?} {after:?}"
    );
    let before_bottom = before.origin.y + before.size.height;
    let image_bottom = image.origin.y + image.size.height;
    assert!(
        before.origin.y < image_bottom && image.origin.y < before_bottom,
        "图片必须与文本同一行：{before:?} {image:?}"
    );
    let after_bottom = after.origin.y + after.size.height;
    assert!(
        image.origin.y < after_bottom && after.origin.y < image_bottom,
        "后段文本必须与图片同一行：{image:?} {after:?}"
    );
}

/// 已缓存的公式位图同样按原位参与文本流，而不是掉到段落末尾。
#[gpui::test]
fn markdown_inline_formula_stays_between_surrounding_text(cx: &mut TestAppContext) {
    let source = "前 $x$ 后\n";
    let view = selection_view(cx, source);
    let owner = view.downgrade();
    let key = view.update(cx, |view, cx| {
        let color = cx.global::<Settings>().editor_theme.foreground;
        view.ensure_formula(0, "x", false, 16.0, 1.0, color, &owner, cx)
    });
    view.update(cx, |view, _| {
        view.formula_failures.remove(&key);
        view.formula_tasks.remove(&key);
        view.formula_images.insert(key, bitmap(3));
    });
    let mut window = open_selection_window(cx, view.clone(), source);
    let ids = view.read_with(&window, |view, _| view.text_layout_ids_for_test());
    assert!(ids.contains(&"markdown-inline-0".to_owned()), "{ids:?}");
    assert!(ids.contains(&"markdown-inline-0-0".to_owned()), "{ids:?}");
    let before = window
        .debug_bounds("markdown-selection-markdown-inline-0")
        .expect("前段文本必须绘制");
    let formula = window
        .debug_bounds("markdown-inline-formula")
        .expect("行内公式位图必须绘制");
    let after = window
        .debug_bounds("markdown-selection-markdown-inline-0-0")
        .expect("后段文本必须绘制");
    assert!(
        before.origin.x < formula.origin.x && formula.origin.x < after.origin.x,
        "公式必须位于两段文本之间：{before:?} {formula:?} {after:?}"
    );
    let before_bottom = before.origin.y + before.size.height;
    let formula_bottom = formula.origin.y + formula.size.height;
    assert!(
        before.origin.y < formula_bottom && formula.origin.y < before_bottom,
        "公式必须与文本同一行：{before:?} {formula:?}"
    );
}

/// 安全 HTML 块：结构、srcset 首个候选与链接点击都必须在真实渲染路径上生效。
#[gpui::test]
fn safe_html_block_renders_details_image_and_clickable_link(cx: &mut TestAppContext) {
    let source = r#"<details open><summary>More</summary><img src="https://example.invalid/fallback.png" srcset="https://example.invalid/a.png 1x, https://example.invalid/b.png 2x" alt="图" width="10" height="20"><a href="https://example.com/html-link">go</a></details>
"#;
    let view = selection_view(cx, source);
    let mut window = open_selection_window(cx, view.clone(), source);
    assert!(
        window.debug_bounds("markdown-html-details-0").is_some(),
        "details 必须渲染为独立结构块"
    );
    assert!(
        window.debug_bounds("markdown-html-image-1").is_some(),
        "图片必须渲染为行内盒子"
    );
    let consumed = view.read_with(&window, |view, _| {
        view.image_blocks.keys().cloned().collect::<Vec<_>>()
    });
    assert!(
        consumed.contains(&"https://example.invalid/a.png".to_owned()),
        "必须消费 srcset 的首个候选: {consumed:?}"
    );
    assert!(
        !consumed.contains(&"https://example.invalid/fallback.png".to_owned())
            && !consumed.contains(&"https://example.invalid/b.png".to_owned()),
        "未选中的候选不得被请求: {consumed:?}"
    );
    let opened = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let sink = opened.clone();
    let _subscription = cx.update(|cx| {
        cx.subscribe(&view, move |_, event: &MarkdownViewEvent, _| {
            if let MarkdownViewEvent::OpenLink(link) = event {
                sink.borrow_mut().push(link.url().to_owned());
            }
        })
    });
    let link = window
        .debug_bounds("markdown-html-link-0")
        .expect("HTML 链接必须可定位");
    window.simulate_click(link.center(), Modifiers::default());
    assert_eq!(
        opened.borrow().as_slice(),
        ["https://example.com/html-link".to_owned()],
        "HTML 块内链接必须可点击"
    );
}

/// 超出安全上限的 HTML 片段必须显示降级提示，而不是静默截断。
#[gpui::test]
fn degraded_html_fragment_shows_a_visible_notice(cx: &mut TestAppContext) {
    let source = format!(
        "{}x{}\n",
        "<div>".repeat(70),
        "</div>".repeat(70)
    );
    let view = selection_view(cx, &source);
    let mut window = open_selection_window(cx, view.clone(), &source);
    assert!(
        window.debug_bounds("markdown-html-degraded").is_some(),
        "降级提示必须绘制"
    );
}

/// 引用与列表项内的代码、HTML 块按真实结构渲染，不再是行内文本。
#[gpui::test]
fn nested_blocks_render_code_and_html_as_structures(cx: &mut TestAppContext) {
    let quote = "> para one
>
> ```rust
> fn main() {}
> ```
>
> <details><summary>html block</summary></details>
";
    let view = selection_view(cx, quote);
    let mut window = open_selection_window(cx, view.clone(), quote);
    for selector in ["markdown-quote-0-0", "markdown-quote-0-1-code", "markdown-quote-0-2"] {
        assert!(
            window.debug_bounds(selector).is_some(),
            "引用内块必须按结构绘制: {selector}"
        );
    }
    assert!(
        window.debug_bounds("markdown-html-details-0").is_some(),
        "引用内 HTML 必须按 details 结构绘制"
    );

    let list = "- item one

  ```js
  code();
  ```
";
    let list_view = selection_view(cx, list);
    let mut list_window = open_selection_window(cx, list_view.clone(), list);
    assert!(
        list_window
            .debug_bounds("markdown-list-item-0-0-0-1-code")
            .is_some(),
        "列表项内代码必须按代码块绘制"
    );
}
