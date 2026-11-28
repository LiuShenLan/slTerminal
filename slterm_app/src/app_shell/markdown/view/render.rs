//! Comrak 投影的只读 GPUI 呈现；不重新解析 Markdown 或 HTML。

use std::cell::RefCell;
use std::collections::HashMap;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, Context, InteractiveElement as _, InteractiveText, IntoElement,
    ParentElement as _, ScrollHandle, SharedString, StatefulInteractiveElement as _, Styled as _,
    StyledImage as _, StyledText, WeakEntity, Window, div, px, relative,
};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::input::Input;
use gpui_component::{ActiveTheme as _, IconName, StyledExt as _, h_flex, v_flex};
use slterm_document::{
    MarkdownAnchorKind, MarkdownBlock, MarkdownBlockKind, MarkdownDocument, MarkdownHtmlAttribute,
    MarkdownHtmlElement, MarkdownHtmlFragment, MarkdownHtmlNode, MarkdownHtmlTag, MarkdownInline,
    MarkdownInlineKind, MarkdownListItem, MarkdownNestedBlock, MarkdownStructure, MarkdownTable,
};

use super::super::formula::logical_formula_size;
use super::super::progressive::{MarkdownBlockOutcome, MarkdownRenderDecision};
use super::super::{
    CloseMarkdownSearch, FindMarkdown, FindNextMarkdown, FindPreviousMarkdown,
    MarkdownClearLinkFocus, MarkdownNextLink, MarkdownOpenLink, MarkdownPreviousLink,
    MarkdownZoomIn, MarkdownZoomOut, MarkdownZoomReset, RenderLargeMarkdown, ScrollMarkdownBottom,
    ScrollMarkdownDown, ScrollMarkdownPageDown, ScrollMarkdownPageUp, ScrollMarkdownTop,
    ScrollMarkdownUp, ToggleMarkdownMode, ToggleMarkdownOutline,
};
use super::selection::{LayoutSegment, highlight_selected_runs};
use super::{KeyboardScroll, MarkdownView, MarkdownViewEvent};
use crate::app_shell::config::Settings;
use crate::app_shell::text_view::MarkdownMode;
use crate::i18n::Message;

#[cfg(all(test, feature = "gpui-test-support"))]
mod selection_tests {
    include!("render_selection_tests.rs");
}

pub(super) fn render_markdown(
    view: &mut MarkdownView,
    _window: &mut Window,
    cx: &mut Context<'_, MarkdownView>,
) -> AnyElement {
    let decision = view.effective_mode();
    let content = match decision {
        MarkdownRenderDecision::Source => view.source.clone().into_any_element(),
        MarkdownRenderDecision::Forbidden => div()
            .p_4()
            .text_color(cx.theme().muted_foreground)
            .child(
                cx.global::<Settings>()
                    .language
                    .text(Message::EditorRenderForbidden),
            )
            .into_any_element(),
        MarkdownRenderDecision::Progressive => render_document(view, cx),
    };
    let mut root = div()
        .id("markdown-view")
        .debug_selector(|| "markdown-view".to_string())
        .key_context("MarkdownView")
        .track_focus(&view.focus)
        .size_full()
        .min_w_0()
        .min_h_0()
        .flex()
        .flex_col()
        .bg(cx.theme().background)
        .on_action(cx.listener(|view, _: &ToggleMarkdownMode, window, cx| {
            view.toggle_mode(window, cx);
        }))
        .on_action(cx.listener(|view, _: &ToggleMarkdownOutline, _, cx| {
            view.toggle_outline(cx);
        }))
        .on_action(cx.listener(|view, _: &ScrollMarkdownUp, _, cx| {
            view.scroll_keyboard(KeyboardScroll::LineUp, cx);
        }))
        .on_action(cx.listener(|view, _: &ScrollMarkdownDown, _, cx| {
            view.scroll_keyboard(KeyboardScroll::LineDown, cx);
        }))
        .on_action(cx.listener(|view, _: &ScrollMarkdownPageUp, _, cx| {
            view.scroll_keyboard(KeyboardScroll::PageUp, cx);
        }))
        .on_action(cx.listener(|view, _: &ScrollMarkdownPageDown, _, cx| {
            view.scroll_keyboard(KeyboardScroll::PageDown, cx);
        }))
        .on_action(cx.listener(|view, _: &ScrollMarkdownTop, _, cx| {
            view.scroll_keyboard(KeyboardScroll::Top, cx);
        }))
        .on_action(cx.listener(|view, _: &ScrollMarkdownBottom, _, cx| {
            view.scroll_keyboard(KeyboardScroll::Bottom, cx);
        }))
        .on_action(cx.listener(|view, _: &MarkdownNextLink, _, cx| {
            view.move_link_focus(true, cx);
        }))
        .on_action(cx.listener(|view, _: &MarkdownPreviousLink, _, cx| {
            view.move_link_focus(false, cx);
        }))
        .on_action(cx.listener(|view, _: &MarkdownOpenLink, _, cx| {
            view.open_focused_link(cx);
        }))
        .on_action(cx.listener(|view, _: &MarkdownClearLinkFocus, _, cx| {
            view.clear_link_focus(cx);
        }))
        .on_action(cx.listener(|view, _: &FindMarkdown, window, cx| {
            view.open_search(window, cx);
        }))
        .on_action(cx.listener(|view, _: &FindNextMarkdown, _, cx| {
            view.move_search(true, cx);
        }))
        .on_action(cx.listener(|view, _: &FindPreviousMarkdown, _, cx| {
            view.move_search(false, cx);
        }))
        .on_action(cx.listener(|view, _: &CloseMarkdownSearch, window, cx| {
            view.close_search(window, cx);
        }))
        .on_action(cx.listener(|view, _: &MarkdownZoomIn, _, cx| view.zoom_in(cx)))
        .on_action(cx.listener(|view, _: &MarkdownZoomOut, _, cx| view.zoom_out(cx)))
        .on_action(cx.listener(|view, _: &MarkdownZoomReset, _, cx| view.zoom_reset(cx)))
        .on_scroll_wheel(cx.listener(|view, event, _, cx| view.zoom_wheel(event, cx)))
        .on_action(cx.listener(|view, _: &RenderLargeMarkdown, _, cx| {
            view.allow_large_document(cx);
        }))
        .on_action(
            cx.listener(|_, _: &crate::app_shell::text_view::SaveFile, _, cx| {
                cx.emit(MarkdownViewEvent::Save);
            }),
        )
        .on_action(
            cx.listener(|_, _: &crate::app_shell::text_view::SaveAsFile, _, cx| {
                cx.emit(MarkdownViewEvent::SaveAs);
            }),
        )
        .on_action(cx.listener(|view, _: &gpui_component::input::Copy, _, cx| {
            if view.effective_mode() == MarkdownRenderDecision::Progressive {
                view.copy_rendered_selection(cx);
            } else {
                cx.propagate();
            }
        }))
        .on_action(
            cx.listener(|view, _: &gpui_component::input::SelectAll, _, cx| {
                if view.effective_mode() == MarkdownRenderDecision::Progressive {
                    view.select_all_rendered();
                    cx.notify();
                } else {
                    cx.propagate();
                }
            }),
        )
        .child(render_toolbar(view, decision, cx))
        .child(
            div()
                .relative()
                .flex_1()
                .min_h_0()
                .min_w_0()
                .child(
                    div()
                        .id("markdown-content")
                        .debug_selector(|| "markdown-content".to_owned())
                        .size_full()
                        .min_w_0()
                        .child(content),
                )
                .when(
                    decision == MarkdownRenderDecision::Progressive && view.outline,
                    |host| host.child(render_outline(view, cx)),
                ),
        );
    if decision == MarkdownRenderDecision::Forbidden {
        root = root.text_color(cx.theme().muted_foreground);
    }
    root.into_any_element()
}

fn render_search(view: &MarkdownView, cx: &mut Context<'_, MarkdownView>) -> Option<AnyElement> {
    let search = view.search.as_ref()?;
    let position = search.active.map_or(0, |active| active + 1);
    let input = search.input.clone();
    Some(
        h_flex()
            .key_context("MarkdownSearch")
            .gap_1()
            .child(
                div()
                    .id("markdown-search-input")
                    .debug_selector(|| "markdown-search-input".to_string())
                    .w(px(220.0))
                    .child(Input::new(&input)),
            )
            .child(
                div()
                    .min_w(px(40.0))
                    .text_center()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!("{position}/{}", search.matches.len())),
            )
            .child(
                Button::new("markdown-search-previous")
                    .label("↑")
                    .ghost()
                    .on_click(cx.listener(|view, _, _, cx| view.move_search(false, cx))),
            )
            .child(
                Button::new("markdown-search-next")
                    .label("↓")
                    .ghost()
                    .on_click(cx.listener(|view, _, _, cx| view.move_search(true, cx))),
            )
            .child(
                Button::new("markdown-search-close")
                    .label("×")
                    .ghost()
                    .on_click(cx.listener(|view, _, window, cx| view.close_search(window, cx))),
            )
            .into_any_element(),
    )
}

fn render_toolbar(
    view: &MarkdownView,
    decision: MarkdownRenderDecision,
    cx: &mut Context<'_, MarkdownView>,
) -> AnyElement {
    let language = cx.global::<Settings>().language;
    let mode_label = if view.mode == MarkdownMode::Render {
        language.text(Message::EditorSource)
    } else {
        language.text(Message::EditorPreview)
    };
    h_flex()
        .id("markdown-toolbar")
        .h(px(36.0))
        .flex_shrink_0()
        .px_2()
        .gap_1()
        .border_b_1()
        .border_color(cx.theme().border)
        .child(
            Button::new("markdown-toggle-mode")
                .label(mode_label)
                .ghost()
                .on_click(cx.listener(|view, _, window, cx| view.toggle_mode(window, cx))),
        )
        .child(
            Button::new("markdown-toggle-outline")
                .icon(IconName::PanelRight)
                .tooltip_with_action(
                    language.text(Message::EditorOutline),
                    &ToggleMarkdownOutline,
                    Some("MarkdownView"),
                )
                .ghost()
                .on_click(cx.listener(|view, _, _, cx| view.toggle_outline(cx))),
        )
        .when_some(render_search(view, cx), |bar, search| bar.child(search))
        .child(div().flex_1())
        .child(
            Button::new("markdown-zoom-out")
                .label("−")
                .ghost()
                .on_click(cx.listener(|view, _, _, cx| view.zoom_out(cx))),
        )
        .child(
            div()
                .min_w(px(48.0))
                .text_center()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(format!("{}%", (view.zoom + 5) / 10)),
        )
        .child(
            Button::new("markdown-zoom-in")
                .label("+")
                .ghost()
                .on_click(cx.listener(|view, _, _, cx| view.zoom_in(cx))),
        )
        .when(
            view.mode == MarkdownMode::Render && decision == MarkdownRenderDecision::Source,
            |bar| {
                bar.child(
                    Button::new("markdown-render-large")
                        .label(language.text(Message::EditorRenderLarge))
                        .ghost()
                        .on_click(cx.listener(|view, _, _, cx| view.allow_large_document(cx))),
                )
            },
        )
        .into_any_element()
}

fn render_document(view: &MarkdownView, cx: &mut Context<'_, MarkdownView>) -> AnyElement {
    let Some(document) = view.document.clone() else {
        return div().size_full().into_any_element();
    };
    let owner = cx.entity().downgrade();
    view.text_layouts.borrow_mut().clear();
    view.selection_cursors.borrow_mut().clear();
    let scroll = view.scroll.clone();
    let block_scrolls = view.block_scrolls.clone();
    let zoom = view.zoom as f32 / 1_000.0;
    div()
        .id("markdown-render")
        .size_full()
        .px_4()
        .child(gpui::list(scroll, move |index, window, cx| {
            let Some(block) = document.blocks().get(index) else {
                return div().into_any_element();
            };
            let outcome = owner
                .update(cx, |view, _| view.renderer.outcome(block.id()).cloned())
                .ok()
                .flatten();
            match outcome {
                Some(MarkdownBlockOutcome::Rendered(prepared)) => render_block(
                    &prepared,
                    &document,
                    index,
                    zoom,
                    &block_scrolls,
                    &owner,
                    window,
                    cx,
                ),
                Some(MarkdownBlockOutcome::Fallback { source, .. }) => div()
                    .id(("markdown-block-fallback", index))
                    .font_family(cx.theme().mono_font_family.clone())
                    .whitespace_nowrap()
                    .child(source)
                    .into_any_element(),
                None => div()
                    .id(("markdown-block-pending", index))
                    .min_h(px(24.0 * zoom))
                    .into_any_element(),
            }
        }))
        .into_any_element()
}

#[allow(clippy::too_many_arguments)]
fn render_block(
    block: &MarkdownBlock,
    document: &MarkdownDocument,
    index: usize,
    zoom: f32,
    scrolls: &RefCell<HashMap<usize, ScrollHandle>>,
    owner: &WeakEntity<MarkdownView>,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let theme = cx.theme().clone();
    let block_selector = format!("markdown-block-{index}");
    let base = div()
        .id(("markdown-block", index))
        .debug_selector(move || block_selector.clone())
        .w_full()
        .max_w(px(900.0 * zoom))
        .mx_auto()
        .py(px(6.0 * zoom))
        .text_size(px(16.0 * zoom))
        .line_height(relative(1.55))
        .text_color(theme.foreground);
    match block.kind() {
        MarkdownBlockKind::FrontMatter => div().into_any_element(),
        MarkdownBlockKind::ThematicBreak => base
            .child(div().h(px(1.0)).w_full().bg(theme.border))
            .into_any_element(),
        MarkdownBlockKind::Code(code) => {
            let scroll = scrolls.borrow_mut().entry(index).or_default().clone();
            let id = SharedString::from(format!("markdown-code-{index}"));
            let source = code.source().to_owned();
            let segments = vec![LayoutSegment {
                local: 0..source.len(),
                visible: block.visible_range(),
            }];
            let selection = owner.update(cx, |view, _| view.selection).ok().flatten();
            let mut run = window.text_style().to_run(source.len());
            run.font.family = theme.mono_font_family.clone();
            let runs = highlight_selected_runs(vec![run], &segments, selection, theme.selection);
            let styled = StyledText::new(SharedString::from(source)).with_runs(runs);
            let _ = owner.update(cx, |view, _| {
                view.register_text_layout(id.to_string(), styled.layout().clone(), segments);
            });
            let content = div()
                .id(("markdown-code-scroll", index))
                .overflow_x_scroll()
                .track_scroll(&scroll)
                .child(div().whitespace_nowrap().child(styled))
                .into_any_element();
            let content = selection_container(id, content, owner);
            base.child(
                div()
                    .w_full()
                    .rounded(px(5.0))
                    .bg(theme.muted)
                    .p_3()
                    .font_family(theme.mono_font_family.clone())
                    .child(content),
            )
            .into_any_element()
        }
        MarkdownBlockKind::Math => {
            let source = document.visible_text()[block.visible_range()].to_owned();
            let settings = cx.global::<Settings>();
            let color = settings.editor_theme.foreground;
            let scale = window.scale_factor();
            let image = owner
                .update(cx, |view, cx| {
                    let key =
                        view.ensure_formula(index, &source, true, 18.0, scale, color, owner, cx);
                    view.formula(key)
                })
                .ok()
                .flatten();
            base.child(if let Some(image) = image {
                // 位图按设备像素生成，显示宽度除回窗口 scale，避免高 DPI 放大。
                let pixels = image.size(0);
                let (width, _) = logical_formula_size(
                    i32::from(pixels.width) as f32,
                    i32::from(pixels.height) as f32,
                    scale,
                );
                gpui::img(image)
                    .w(px(width))
                    .max_w_full()
                    .object_fit(gpui::ObjectFit::Contain)
                    .into_any_element()
            } else {
                div()
                    .font_family(theme.mono_font_family.clone())
                    .child(source)
                    .into_any_element()
            })
            .into_any_element()
        }
        MarkdownBlockKind::Html(fragment) => {
            let content = render_html_fragment(fragment, block, document, owner, window, cx);
            base.child(content).into_any_element()
        }
        _ => {
            let font_size = match block.kind() {
                MarkdownBlockKind::Heading(depth) => 28.0 - (*depth as f32 - 1.0).min(5.0) * 2.0,
                _ => 16.0,
            };
            let content = match block.structure() {
                Some(MarkdownStructure::List(list)) => {
                    let mut rows = v_flex().w_full().gap_1();
                    for (item_index, item) in list.items().iter().enumerate() {
                        rows = rows.child(render_list_item(
                            item, item_index, block, document, owner, window, cx,
                        ));
                    }
                    rows.into_any_element()
                }
                Some(MarkdownStructure::Table(table)) => {
                    let scroll = scrolls.borrow_mut().entry(index).or_default().clone();
                    let mut rows = v_flex();
                    for (row_index, row) in table.rows().iter().enumerate() {
                        let mut cells = h_flex();
                        for (column, cell) in row.cells().iter().enumerate() {
                            let alignment = table.alignments().get(column).copied();
                            cells = cells.child(
                                div()
                                    .w(px(160.0 * zoom))
                                    .flex_shrink_0()
                                    .p_2()
                                    .border_1()
                                    .border_color(theme.border)
                                    .text_align(match alignment {
                                        Some(slterm_document::MarkdownAlignment::Right) => {
                                            gpui::TextAlign::Right
                                        }
                                        Some(slterm_document::MarkdownAlignment::Center) => {
                                            gpui::TextAlign::Center
                                        }
                                        _ => gpui::TextAlign::Left,
                                    })
                                    .when(row.header(), |cell| cell.font_semibold().bg(theme.muted))
                                    .child(rich_text(
                                        SharedString::from(format!(
                                            "markdown-table-cell-{index}-{row_index}-{column}"
                                        )),
                                        cell.inlines(),
                                        block,
                                        document,
                                        owner,
                                        window,
                                        cx,
                                    )),
                            );
                        }
                        rows = rows.child(cells);
                    }
                    div()
                        .id(("markdown-table-scroll", index))
                        .overflow_x_scroll()
                        .track_scroll(&scroll)
                        .child(rows)
                        .into_any_element()
                }
                Some(MarkdownStructure::Quote(nested)) => render_nested_blocks(
                    nested,
                    &format!("markdown-quote-{index}"),
                    block,
                    document,
                    owner,
                    window,
                    cx,
                ),
                None => rich_text(
                    SharedString::from(format!("markdown-inline-{index}")),
                    block.inlines(),
                    block,
                    document,
                    owner,
                    window,
                    cx,
                ),
            };
            let framed = match block.kind() {
                MarkdownBlockKind::Quote | MarkdownBlockKind::Alert(_) => {
                    base.border_l_2().border_color(theme.border).pl_3()
                }
                _ => base,
            };
            // 脚注定义块带名称标记，与正文中的上标引用共享同一锚点。
            let footnote = if matches!(block.kind(), MarkdownBlockKind::Footnote) {
                document
                    .anchors()
                    .iter()
                    .find(|anchor| {
                        anchor.block() == block.id()
                            && anchor.kind() == MarkdownAnchorKind::Footnote
                    })
                    .map(|anchor| {
                        format!(
                            "[^{}]",
                            anchor.id().strip_prefix("fn-").unwrap_or(anchor.id())
                        )
                    })
            } else {
                None
            };
            framed
                .when_some(footnote, |element, marker| {
                    element.child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(marker),
                    )
                })
                .text_size(px(font_size * zoom))
                .when(
                    matches!(block.kind(), MarkdownBlockKind::Heading(_)),
                    |block| block.font_semibold(),
                )
                .child(content)
                .into_any_element()
        }
    }
}

/// 引用与列表项内的块按真实类型渲染；行内文本仍共享同一块选区映射。
#[allow(clippy::too_many_arguments)]
fn render_nested_blocks(
    blocks: &[MarkdownNestedBlock],
    id_prefix: &str,
    block: &MarkdownBlock,
    document: &MarkdownDocument,
    owner: &WeakEntity<MarkdownView>,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let mut container = v_flex().w_full().gap_1();
    for (index, nested) in blocks.iter().enumerate() {
        let selector = format!("{id_prefix}-{index}");
        container = container.child(
            div()
                .debug_selector({
                    let selector = selector.clone();
                    move || selector.clone()
                })
                .child(render_nested_block(
                    nested, &selector, block, document, owner, window, cx,
                )),
        );
    }
    container.into_any_element()
}

#[allow(clippy::too_many_arguments)]
fn render_nested_block(
    nested: &MarkdownNestedBlock,
    id_prefix: &str,
    block: &MarkdownBlock,
    document: &MarkdownDocument,
    owner: &WeakEntity<MarkdownView>,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let theme = cx.theme().clone();
    match nested {
        MarkdownNestedBlock::Paragraph { inlines } => rich_text(
            SharedString::from(id_prefix.to_owned()),
            inlines,
            block,
            document,
            owner,
            window,
            cx,
        ),
        MarkdownNestedBlock::Heading { inlines, depth } => {
            let content = rich_text(
                SharedString::from(id_prefix.to_owned()),
                inlines,
                block,
                document,
                owner,
                window,
                cx,
            );
            div()
                .font_semibold()
                .text_size(px(24.0 - f32::from((*depth).saturating_sub(1).min(4)) * 2.0))
                .child(content)
                .into_any_element()
        }
        MarkdownNestedBlock::Code(code) => {
            let selector = format!("{id_prefix}-code");
            div()
                .debug_selector(move || selector.clone())
                .w_full()
                .rounded(px(5.0))
                .bg(theme.muted)
                .p_2()
                .font_family(theme.mono_font_family.clone())
                .child(code.source().to_owned())
                .into_any_element()
        }
        MarkdownNestedBlock::Html(fragment) => {
            render_html_fragment(fragment, block, document, owner, window, cx)
        }
        MarkdownNestedBlock::Table(table) => {
            render_nested_table(table, id_prefix, block, document, owner, window, cx)
        }
        MarkdownNestedBlock::Math { source } => {
            let settings = cx.global::<Settings>();
            let color = settings.editor_theme.foreground;
            let scale = window.scale_factor();
            let image = owner
                .update(cx, |view, cx| {
                    let key = view.ensure_formula(
                        block.id().index(),
                        source,
                        true,
                        18.0,
                        scale,
                        color,
                        owner,
                        cx,
                    );
                    view.formula(key)
                })
                .ok()
                .flatten();
            if let Some(image) = image {
                let pixels = image.size(0);
                let (width, _) = logical_formula_size(
                    i32::from(pixels.width) as f32,
                    i32::from(pixels.height) as f32,
                    scale,
                );
                gpui::img(image)
                    .w(px(width))
                    .max_w_full()
                    .object_fit(gpui::ObjectFit::Contain)
                    .into_any_element()
            } else {
                div()
                    .font_family(theme.mono_font_family.clone())
                    .child(source.clone())
                    .into_any_element()
            }
        }
        MarkdownNestedBlock::List(list) => {
            let mut rows = v_flex().w_full().gap_1();
            for (index, item) in list.items().iter().enumerate() {
                rows = rows.child(render_list_item(
                    item, index, block, document, owner, window, cx,
                ));
            }
            rows.into_any_element()
        }
        MarkdownNestedBlock::Quote(blocks) => div()
            .w_full()
            .border_l_2()
            .border_color(theme.border)
            .pl_3()
            .child(render_nested_blocks(
                blocks, id_prefix, block, document, owner, window, cx,
            ))
            .into_any_element(),
        MarkdownNestedBlock::ThematicBreak => div()
            .my_1()
            .h(px(1.0))
            .w_full()
            .bg(theme.border)
            .into_any_element(),
    }
}

#[allow(clippy::too_many_arguments)]
fn render_nested_table(
    table: &MarkdownTable,
    id_prefix: &str,
    block: &MarkdownBlock,
    document: &MarkdownDocument,
    owner: &WeakEntity<MarkdownView>,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let theme = cx.theme().clone();
    let mut rows = v_flex();
    for (row_index, row) in table.rows().iter().enumerate() {
        let mut cells = h_flex();
        for (column, cell) in row.cells().iter().enumerate() {
            let alignment = table.alignments().get(column).copied();
            cells = cells.child(
                div()
                    .w(px(140.0))
                    .flex_shrink_0()
                    .p_1()
                    .border_1()
                    .border_color(theme.border)
                    .text_align(match alignment {
                        Some(slterm_document::MarkdownAlignment::Right) => gpui::TextAlign::Right,
                        Some(slterm_document::MarkdownAlignment::Center) => gpui::TextAlign::Center,
                        _ => gpui::TextAlign::Left,
                    })
                    .when(row.header(), |cell| cell.font_semibold().bg(theme.muted))
                    .child(rich_text(
                        SharedString::from(format!("{id_prefix}-table-{row_index}-{column}")),
                        cell.inlines(),
                        block,
                        document,
                        owner,
                        window,
                        cx,
                    )),
            );
        }
        rows = rows.child(cells);
    }
    rows.into_any_element()
}

#[allow(clippy::too_many_arguments)]
fn render_list_item(
    item: &MarkdownListItem,
    item_index: usize,
    block: &MarkdownBlock,
    document: &MarkdownDocument,
    owner: &WeakEntity<MarkdownView>,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let marker = match (item.checked(), item.ordinal()) {
        (Some(true), _) => "[x]".to_owned(),
        (Some(false), _) => "[ ]".to_owned(),
        (None, Some(ordinal)) => format!("{ordinal}."),
        (None, None) => "•".to_owned(),
    };
    let mut children = v_flex().flex_1().min_w_0();
    if !item.blocks().is_empty() {
        children = children.child(render_nested_blocks(
            item.blocks(),
            &format!(
                "markdown-list-item-{}-{}-{item_index}",
                block.id().index(),
                item.depth()
            ),
            block,
            document,
            owner,
            window,
            cx,
        ));
    }
    for (child_index, child) in item.children().iter().enumerate() {
        children = children.child(div().pl_4().child(render_list_item(
            child,
            child_index,
            block,
            document,
            owner,
            window,
            cx,
        )));
    }
    h_flex()
        .w_full()
        .items_start()
        .gap_2()
        .child(div().min_w(px(28.0)).child(marker))
        .child(children)
        .into_any_element()
}

fn selection_container(
    id: SharedString,
    content: AnyElement,
    owner: &WeakEntity<MarkdownView>,
) -> AnyElement {
    let selection_id = id.to_string();
    let selection_selector = format!("markdown-selection-{selection_id}");
    let down_owner = owner.clone();
    let down_id = selection_id.clone();
    let move_owner = owner.clone();
    let move_id = selection_id.clone();
    let up_owner = owner.clone();
    div()
        .id(SharedString::from(selection_selector.clone()))
        .debug_selector(move || selection_selector.clone())
        .on_mouse_down(gpui::MouseButton::Left, move |event, window, cx| {
            let _ = down_owner.update(cx, |view, cx| {
                view.begin_text_selection(&down_id, event.position, window, cx);
            });
        })
        .on_mouse_move(move |event, _, cx| {
            let _ = move_owner.update(cx, |view, _| {
                if event.pressed_button == Some(gpui::MouseButton::Left) {
                    view.extend_text_selection(&move_id, event.position);
                } else {
                    view.end_text_selection();
                }
            });
        })
        .on_mouse_up(gpui::MouseButton::Left, move |_, _, cx| {
            let _ = up_owner.update(cx, |view, _| view.end_text_selection());
        })
        .child(content)
        .into_any_element()
}

/// 行内文本片段：同一块在行内公式或图片处分段，分段按文档顺序排布。
struct InlineTextPiece {
    id: SharedString,
    text: String,
    runs: Vec<gpui::TextRun>,
    segments: Vec<LayoutSegment>,
    links: Vec<slterm_document::MarkdownLink>,
    link_ranges: Vec<std::ops::Range<usize>>,
}

impl InlineTextPiece {
    fn new(id: SharedString) -> Self {
        Self {
            id,
            text: String::new(),
            runs: Vec::new(),
            segments: Vec::new(),
            links: Vec::new(),
            link_ranges: Vec::new(),
        }
    }

    fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    fn into_element(self, owner: &WeakEntity<MarkdownView>, cx: &mut App) -> AnyElement {
        let selection = owner.update(cx, |view, _| view.selection).ok().flatten();
        let runs =
            highlight_selected_runs(self.runs, &self.segments, selection, cx.theme().selection);
        let styled = StyledText::new(SharedString::from(self.text)).with_runs(runs);
        // TextLayout 克隆共享测量状态；注册实际绘制实例，鼠标命中才有 prepaint 边界。
        let _ = owner.update(cx, |view, _| {
            view.register_text_layout(self.id.to_string(), styled.layout().clone(), self.segments);
        });
        let content = if self.links.is_empty() {
            styled.into_any_element()
        } else {
            let link_owner = owner.clone();
            let links = self.links;
            InteractiveText::new(self.id.clone(), styled)
                .on_click(self.link_ranges, move |index, _, cx| {
                    let Some(link) = links.get(index).cloned() else {
                        return;
                    };
                    let _ =
                        link_owner.update(cx, |_, cx| cx.emit(MarkdownViewEvent::OpenLink(link)));
                })
                .into_any_element()
        };
        selection_container(self.id, content, owner)
    }
}

/// 当前片段入列，并为后续文本准备新片段身份。
fn flush_inline_piece(
    parts: &mut Vec<AnyElement>,
    piece: &mut InlineTextPiece,
    base: &str,
    owner: &WeakEntity<MarkdownView>,
    cx: &mut App,
) {
    let current = std::mem::replace(
        piece,
        InlineTextPiece::new(SharedString::from(format!("{base}-{}", parts.len()))),
    );
    if !current.is_empty() {
        parts.push(current.into_element(owner, cx));
    }
}

/// 行内图片只占原位盒子：未加载时显示替代文本，点击才请求资源。
fn inline_image_element(
    alt: &str,
    url: &str,
    block: &MarkdownBlock,
    owner: &WeakEntity<MarkdownView>,
    cx: &mut App,
) -> AnyElement {
    let image = owner
        .update(cx, |view, cx| {
            view.ensure_image(block.id().index(), url, owner, cx);
            view.image(url)
        })
        .ok()
        .flatten();
    let placeholder = div()
        .p_1()
        .rounded(px(4.0))
        .border_1()
        .border_color(cx.theme().border)
        .text_xs()
        .text_color(cx.theme().muted_foreground)
        .child(if alt.is_empty() {
            url.to_owned()
        } else {
            alt.to_owned()
        });
    let image_owner = owner.clone();
    let image_url = url.to_owned();
    let selector = format!("markdown-image-{url}");
    div()
        .id(SharedString::from(selector.clone()))
        .debug_selector(move || selector.clone())
        .flex_shrink_0()
        .cursor_pointer()
        .on_click(move |_, _, cx| {
            let _ = image_owner.update(cx, |view, cx| {
                view.load_image_on_click(&image_url, &image_owner, cx);
            });
        })
        .child(if let Some(image) = image {
            gpui::img(image)
                .max_h(px(160.0))
                .max_w_full()
                .object_fit(gpui::ObjectFit::Contain)
                .into_any_element()
        } else {
            placeholder.into_any_element()
        })
        .into_any_element()
}

/// 行内公式盒子与文字同序排布，宽度按窗口 scale 校正。
fn inline_formula_element(
    image: std::sync::Arc<gpui::RenderImage>,
    scale_factor: f32,
) -> AnyElement {
    let pixels = image.size(0);
    let (width, _) = logical_formula_size(
        i32::from(pixels.width) as f32,
        i32::from(pixels.height) as f32,
        scale_factor,
    );
    div()
        .debug_selector(|| "markdown-inline-formula".to_owned())
        .flex_shrink_0()
        .mb(px(2.0))
        .child(
            gpui::img(image)
                .w(px(width))
                .object_fit(gpui::ObjectFit::Contain),
        )
        .into_any_element()
}

/// 安全 HTML 的样式继承：链接、上下标与 kbd 沿子树向下传递。
#[derive(Clone, Default)]
struct HtmlInlineStyle {
    link: Option<String>,
    sub: bool,
    sup: bool,
    kbd: bool,
    bold: bool,
}

/// 安全 HTML 流中的一项：文本、行内盒子或独立块。
enum HtmlFlowItem {
    Text {
        text: String,
        style: HtmlInlineStyle,
    },
    Break,
    Inline(AnyElement),
    Block(AnyElement),
}

fn html_attribute(
    element: &MarkdownHtmlElement,
    select: impl Fn(&MarkdownHtmlAttribute) -> bool,
) -> Option<&MarkdownHtmlAttribute> {
    element
        .attributes()
        .iter()
        .find(|attribute| select(attribute))
}

fn html_image_url(element: &MarkdownHtmlElement) -> Option<String> {
    // srcset 优先；只消费选中的候选，其他候选保持未授权
    if let Some(MarkdownHtmlAttribute::SrcSet(candidates)) = html_attribute(element, |attribute| {
        matches!(attribute, MarkdownHtmlAttribute::SrcSet(_))
    }) && let Some(candidate) = candidates.first()
    {
        return Some(candidate.url().to_owned());
    }
    if let Some(MarkdownHtmlAttribute::Src(url)) = html_attribute(element, |attribute| {
        matches!(attribute, MarkdownHtmlAttribute::Src(_))
    }) {
        return Some(url.clone());
    }
    None
}

/// 图片盒子：选中候选经逐 URL 授权；点击才加载，尺寸来自净化属性。
fn html_image_element(
    element: &MarkdownHtmlElement,
    block: &MarkdownBlock,
    owner: &WeakEntity<MarkdownView>,
    index: usize,
    cx: &mut App,
) -> Option<AnyElement> {
    let url = html_image_url(element)?;
    let alt = html_attribute(element, |attribute| {
        matches!(attribute, MarkdownHtmlAttribute::Alt(_))
    })
    .map(|attribute| match attribute {
        MarkdownHtmlAttribute::Alt(value) => value.clone(),
        _ => String::new(),
    })
    .unwrap_or_default();
    let width = html_attribute(element, |attribute| {
        matches!(attribute, MarkdownHtmlAttribute::Width(_))
    })
    .map(|attribute| match attribute {
        MarkdownHtmlAttribute::Width(value) => *value,
        _ => 0,
    });
    let height = html_attribute(element, |attribute| {
        matches!(attribute, MarkdownHtmlAttribute::Height(_))
    })
    .map(|attribute| match attribute {
        MarkdownHtmlAttribute::Height(value) => *value,
        _ => 0,
    });
    let image = owner
        .update(cx, |view, cx| {
            view.ensure_image(block.id().index(), &url, owner, cx);
            view.image(&url)
        })
        .ok()
        .flatten();
    let mut placeholder = div()
        .p_1()
        .rounded(px(4.0))
        .border_1()
        .border_color(cx.theme().border)
        .text_xs()
        .text_color(cx.theme().muted_foreground)
        .child(if alt.is_empty() {
            url.clone()
        } else {
            alt.clone()
        });
    if let Some(width) = width {
        placeholder = placeholder.w(px(width as f32));
    }
    if let Some(height) = height {
        placeholder = placeholder.h(px(height as f32));
    }
    let image_owner = owner.clone();
    let image_url = url.clone();
    let selector = format!("markdown-html-image-{index}");
    Some(
        div()
            .id(SharedString::from(selector.clone()))
            .debug_selector(move || selector.clone())
            .flex_shrink_0()
            .cursor_pointer()
            .on_click(move |_, _, cx| {
                let _ = image_owner.update(cx, |view, cx| {
                    view.load_image_on_click(&image_url, &image_owner, cx);
                });
            })
            .child(if let Some(image) = image {
                let mut element = gpui::img(image)
                    .max_h(px(160.0))
                    .max_w_full()
                    .object_fit(gpui::ObjectFit::Contain);
                if let Some(width) = width {
                    element = element.w(px(width as f32));
                }
                if let Some(height) = height {
                    element = element.h(px(height as f32));
                }
                element.into_any_element()
            } else {
                placeholder.into_any_element()
            })
            .into_any_element(),
    )
}

fn html_picture_element(
    element: &MarkdownHtmlElement,
    block: &MarkdownBlock,
    owner: &WeakEntity<MarkdownView>,
    index: usize,
    cx: &mut App,
) -> Option<AnyElement> {
    for child in element.children() {
        let MarkdownHtmlNode::Element(source) = child else {
            continue;
        };
        if source.tag() == MarkdownHtmlTag::Source && html_image_url(source).is_some() {
            return html_image_element(source, block, owner, index, cx);
        }
    }
    element.children().iter().find_map(|child| {
        let MarkdownHtmlNode::Element(image) = child else {
            return None;
        };
        (image.tag() == MarkdownHtmlTag::Img)
            .then(|| html_image_element(image, block, owner, index, cx))
            .flatten()
    })
}

#[allow(clippy::too_many_arguments)]
fn html_flow_text(
    text: String,
    style: HtmlInlineStyle,
    id: SharedString,
    document: &MarkdownDocument,
    block: &MarkdownBlock,
    owner: &WeakEntity<MarkdownView>,
    window: &Window,
    cx: &mut App,
) -> Option<AnyElement> {
    if text.is_empty() {
        return None;
    }
    let mut run = window.text_style().to_run(text.len());
    if style.bold {
        run.font.weight = gpui::FontWeight::SEMIBOLD;
    }
    if style.kbd {
        run.font.family = cx.theme().mono_font_family.clone();
        run.background_color = Some(cx.theme().muted);
    }
    let mut link = None;
    if let Some(url) = style.link.as_deref() {
        run.color = cx.theme().primary;
        run.underline = Some(gpui::UnderlineStyle {
            thickness: px(1.0),
            color: Some(cx.theme().primary),
            wavy: false,
        });
        link = document
            .links()
            .iter()
            .find(|link| link.block() == block.id() && link.url() == url)
            .cloned();
    }
    let styled = StyledText::new(SharedString::from(text.clone())).with_runs(vec![run]);
    let link_selector = link.as_ref().and_then(|link| {
        document
            .links()
            .iter()
            .position(|candidate| candidate == link)
            .map(|position| format!("markdown-html-link-{position}"))
    });
    let content = if let Some(link) = link {
        let link_owner = owner.clone();
        let links = [link];
        InteractiveText::new(id, styled)
            .on_click(
                std::iter::once(0..text.len()).collect::<Vec<_>>(),
                move |index, _, cx| {
                    let Some(link) = links.get(index).cloned() else {
                        return;
                    };
                    let _ =
                        link_owner.update(cx, |_, cx| cx.emit(MarkdownViewEvent::OpenLink(link)));
                },
            )
            .into_any_element()
    } else {
        styled.into_any_element()
    };
    let mut wrapper = div();
    if style.sub {
        // 位图文本运行不支持基线偏移，用相对定位与较小字号近似上下标。
        wrapper = wrapper.relative().top(px(3.0)).text_size(px(12.0));
    } else if style.sup {
        wrapper = wrapper.relative().top(px(-5.0)).text_size(px(12.0));
    }
    if let Some(selector) = link_selector {
        wrapper = wrapper.debug_selector(move || selector.clone());
    }
    Some(wrapper.child(content).into_any_element())
}

/// 按文档顺序收集安全 HTML 节点；内联样式随子树继承。
#[allow(clippy::too_many_arguments)]
fn collect_html_flow(
    nodes: &[MarkdownHtmlNode],
    style: &HtmlInlineStyle,
    block: &MarkdownBlock,
    document: &MarkdownDocument,
    owner: &WeakEntity<MarkdownView>,
    window: &Window,
    cx: &mut App,
    next_id: &mut usize,
    output: &mut Vec<HtmlFlowItem>,
) {
    for node in nodes {
        match node {
            MarkdownHtmlNode::Text(text) => {
                if !text.is_empty() {
                    output.push(HtmlFlowItem::Text {
                        text: text.clone(),
                        style: style.clone(),
                    });
                }
            }
            MarkdownHtmlNode::Placeholder => {}
            MarkdownHtmlNode::Element(element) => match element.tag() {
                MarkdownHtmlTag::Br => output.push(HtmlFlowItem::Break),
                MarkdownHtmlTag::Hr => output.push(HtmlFlowItem::Block(
                    div()
                        .my_2()
                        .h(px(1.0))
                        .w_full()
                        .bg(cx.theme().border)
                        .into_any_element(),
                )),
                MarkdownHtmlTag::Sub
                | MarkdownHtmlTag::Sup
                | MarkdownHtmlTag::Kbd
                | MarkdownHtmlTag::Summary => {
                    let mut style = style.clone();
                    match element.tag() {
                        MarkdownHtmlTag::Sub => style.sub = true,
                        MarkdownHtmlTag::Sup => style.sup = true,
                        MarkdownHtmlTag::Kbd => style.kbd = true,
                        MarkdownHtmlTag::Summary => style.bold = true,
                        _ => {}
                    }
                    collect_html_flow(
                        element.children(),
                        &style,
                        block,
                        document,
                        owner,
                        window,
                        cx,
                        next_id,
                        output,
                    );
                }
                MarkdownHtmlTag::A => {
                    let mut style = style.clone();
                    if let Some(MarkdownHtmlAttribute::Href(url)) =
                        html_attribute(element, |attribute| {
                            matches!(attribute, MarkdownHtmlAttribute::Href(_))
                        })
                    {
                        style.link = Some(url.clone());
                    }
                    collect_html_flow(
                        element.children(),
                        &style,
                        block,
                        document,
                        owner,
                        window,
                        cx,
                        next_id,
                        output,
                    );
                }
                MarkdownHtmlTag::Img => {
                    *next_id += 1;
                    if let Some(element) = html_image_element(element, block, owner, *next_id, cx) {
                        output.push(HtmlFlowItem::Inline(element));
                    }
                }
                MarkdownHtmlTag::Picture => {
                    *next_id += 1;
                    if let Some(element) = html_picture_element(element, block, owner, *next_id, cx)
                    {
                        output.push(HtmlFlowItem::Inline(element));
                    }
                }
                MarkdownHtmlTag::Source => {}
                MarkdownHtmlTag::Details => {
                    output.push(HtmlFlowItem::Block(html_details_element(
                        element, block, document, owner, window, cx, next_id,
                    )));
                }
            },
        }
    }
}

/// details 保留摘要与内容结构；closed 状态只显示摘要。
fn html_details_element(
    element: &MarkdownHtmlElement,
    block: &MarkdownBlock,
    document: &MarkdownDocument,
    owner: &WeakEntity<MarkdownView>,
    window: &Window,
    cx: &mut App,
    next_id: &mut usize,
) -> AnyElement {
    let open = element
        .attributes()
        .iter()
        .any(|attribute| matches!(attribute, MarkdownHtmlAttribute::Open));
    let mut summary_items = Vec::new();
    let mut content_items = Vec::new();
    let style = HtmlInlineStyle::default();
    for child in element.children() {
        match child {
            MarkdownHtmlNode::Element(summary) if summary.tag() == MarkdownHtmlTag::Summary => {
                collect_html_flow(
                    summary.children(),
                    &style,
                    block,
                    document,
                    owner,
                    window,
                    cx,
                    next_id,
                    &mut summary_items,
                );
            }
            other => collect_html_flow(
                std::slice::from_ref(other),
                &style,
                block,
                document,
                owner,
                window,
                cx,
                next_id,
                &mut content_items,
            ),
        }
    }
    let mut body = v_flex().w_full().gap_1().child(html_flow_element(
        summary_items,
        block,
        document,
        owner,
        window,
        cx,
    ));
    if open {
        body = body.child(html_flow_element(
            content_items,
            block,
            document,
            owner,
            window,
            cx,
        ));
    }
    let selector = format!("markdown-html-details-{}", block.id().index());
    div()
        .debug_selector(move || selector.clone())
        .rounded(px(5.0))
        .border_1()
        .border_color(cx.theme().border)
        .p_2()
        .text_sm()
        .child(body)
        .into_any_element()
}

/// 行内项按可换行行序排布，块项独占一行。
fn html_flow_element(
    items: Vec<HtmlFlowItem>,
    block: &MarkdownBlock,
    document: &MarkdownDocument,
    owner: &WeakEntity<MarkdownView>,
    window: &Window,
    cx: &mut App,
) -> AnyElement {
    let mut parts: Vec<AnyElement> = Vec::new();
    let mut inline: Vec<AnyElement> = Vec::new();
    let mut inline_index = 0usize;
    let flush = |inline: &mut Vec<AnyElement>, parts: &mut Vec<AnyElement>| {
        if inline.is_empty() {
            return;
        }
        parts.push(
            h_flex()
                .w_full()
                .flex_wrap()
                .items_center()
                .children(std::mem::take(inline))
                .into_any_element(),
        );
    };
    for item in items {
        match item {
            HtmlFlowItem::Block(element) => {
                flush(&mut inline, &mut parts);
                parts.push(element);
            }
            HtmlFlowItem::Break => {
                // 满宽零高项把后续行内内容推到下一行
                inline.push(div().w_full().h(px(0.0)).flex_shrink_0().into_any_element());
            }
            HtmlFlowItem::Inline(element) => inline.push(element),
            HtmlFlowItem::Text { text, style } => {
                inline_index += 1;
                let id = SharedString::from(format!(
                    "markdown-html-text-{}-{inline_index}",
                    block.id().index()
                ));
                if let Some(element) =
                    html_flow_text(text, style, id, document, block, owner, window, cx)
                {
                    inline.push(element);
                }
            }
        }
    }
    flush(&mut inline, &mut parts);
    if parts.len() == 1 {
        return parts.pop().expect("HTML 流至少有一项");
    }
    v_flex().w_full().gap_1().children(parts).into_any_element()
}

fn render_html_fragment(
    fragment: &MarkdownHtmlFragment,
    block: &MarkdownBlock,
    document: &MarkdownDocument,
    owner: &WeakEntity<MarkdownView>,
    window: &Window,
    cx: &mut App,
) -> AnyElement {
    let mut items = Vec::new();
    let mut next_id = 0usize;
    collect_html_flow(
        fragment.nodes(),
        &HtmlInlineStyle::default(),
        block,
        document,
        owner,
        window,
        cx,
        &mut next_id,
        &mut items,
    );
    let content = html_flow_element(items, block, document, owner, window, cx);
    if fragment.is_degraded() {
        // 超限降级必须对用户可见，而不是静默丢结构
        let language = cx.global::<Settings>().language;
        return v_flex()
            .w_full()
            .gap_1()
            .child(content)
            .child(
                div()
                    .debug_selector(|| "markdown-html-degraded".to_owned())
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(language.text(Message::EditorHtmlLimited)),
            )
            .into_any_element();
    }
    content
}

#[allow(clippy::too_many_arguments)]
fn rich_text(
    id: SharedString,
    inlines: &[MarkdownInline],
    block: &MarkdownBlock,
    document: &MarkdownDocument,
    owner: &WeakEntity<MarkdownView>,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let sequence = inlines
        .iter()
        .filter(|inline| !matches!(inline.kind(), MarkdownInlineKind::Image))
        .map(|inline| inline.text())
        .collect::<String>();
    let sequence_base = if block.structure().is_some() && !sequence.is_empty() {
        owner
            .update(cx, |view, _| {
                let block_range = block.visible_range();
                let visible = &document.visible_text()[block_range.clone()];
                let mut cursors = view.selection_cursors.borrow_mut();
                let cursor = cursors.entry(block.id().index()).or_insert(0);
                let relative = visible[*cursor..].find(&sequence).unwrap_or(0);
                *cursor = relative + sequence.len();
                block_range.start + relative
            })
            .unwrap_or_else(|_| block.visible_range().start)
    } else {
        block.visible_range().start
    };
    let math_color = cx.global::<Settings>().editor_theme.foreground;
    let focused_link = owner
        .update(cx, |view, _| view.focused_link.clone())
        .ok()
        .flatten();
    let scale_factor = window.scale_factor();
    let base_id = id.to_string();
    let mut parts: Vec<AnyElement> = Vec::new();
    let mut piece = InlineTextPiece::new(id.clone());
    for inline in inlines {
        if inline.kind() == MarkdownInlineKind::Image {
            flush_inline_piece(&mut parts, &mut piece, &base_id, owner, cx);
            parts.push(inline_image_element(
                inline.text(),
                inline.url().unwrap_or_default(),
                block,
                owner,
                cx,
            ));
            continue;
        }
        if inline.kind() == MarkdownInlineKind::Math {
            let image = owner
                .update(cx, |view, cx| {
                    let key = view.ensure_formula(
                        block.id().index(),
                        inline.text(),
                        false,
                        16.0,
                        scale_factor,
                        math_color,
                        owner,
                        cx,
                    );
                    view.formula(key)
                })
                .ok()
                .flatten();
            if let Some(image) = image {
                flush_inline_piece(&mut parts, &mut piece, &base_id, owner, cx);
                parts.push(inline_formula_element(image, scale_factor));
                continue;
            }
        }
        let value = inline.text();
        if value.is_empty() {
            continue;
        }
        let start = piece.text.len();
        piece.text.push_str(value);
        let mut run = window.text_style().to_run(value.len());
        let style = inline.style();
        if style.strong {
            run.font.weight = gpui::FontWeight::SEMIBOLD;
        }
        if style.emphasis {
            run.font.style = gpui::FontStyle::Italic;
        }
        if style.strikethrough {
            run.strikethrough = Some(gpui::StrikethroughStyle {
                thickness: px(1.0),
                color: None,
            });
        }
        if inline.kind() == MarkdownInlineKind::Code || style.kbd {
            run.font.family = cx.theme().mono_font_family.clone();
            run.background_color = Some(cx.theme().muted);
        }
        if let Some(url) = inline.url() {
            run.color = cx.theme().primary;
            run.underline = Some(gpui::UnderlineStyle {
                thickness: px(1.0),
                color: Some(cx.theme().primary),
                wavy: false,
            });
            if let Some(link) = document
                .links()
                .iter()
                .find(|link| link.block() == block.id() && link.url() == url)
            {
                if focused_link.as_ref() == Some(link) {
                    run.background_color = Some(cx.theme().selection);
                }
                piece.link_ranges.push(start..piece.text.len());
                piece.links.push(link.clone());
            }
        }
        piece.segments.push(LayoutSegment {
            local: start..piece.text.len(),
            visible: sequence_base + inline.visible_range().start
                ..sequence_base + inline.visible_range().end,
        });
        piece.runs.push(run);
    }
    if parts.is_empty() {
        return piece.into_element(owner, cx);
    }
    flush_inline_piece(&mut parts, &mut piece, &base_id, owner, cx);
    if parts.len() == 1 {
        return parts.pop().expect("已有片段");
    }
    // 行内盒子与相邻文本同序排布，允许在盒子边界换行。
    h_flex()
        .w_full()
        .flex_wrap()
        .items_center()
        .children(parts)
        .into_any_element()
}

fn render_outline(view: &MarkdownView, cx: &mut Context<'_, MarkdownView>) -> AnyElement {
    let language = cx.global::<Settings>().language;
    let Some(document) = view.document.clone() else {
        return div().into_any_element();
    };
    let mut headings = v_flex()
        .id("markdown-outline-list")
        .size_full()
        .overflow_y_scroll()
        .p_2()
        .gap_1();
    if document.outline().is_empty() {
        headings = headings.child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(language.text(Message::EditorNoHeadings)),
        );
    }
    for (index, heading) in document.outline().iter().enumerate() {
        let block = heading.block();
        // 按标题深度缩进，让大纲保留文档层级。
        let indent = f32::from(heading.depth().saturating_sub(1)) * 12.0;
        headings = headings.child(
            div().pl(px(indent)).child(
                Button::new(("markdown-heading", index))
                    .debug_selector(move || format!("markdown-heading-{index}"))
                    .label(heading.text().to_owned())
                    .ghost()
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.scroll_to_block(block, cx);
                    })),
            ),
        );
    }
    // 大纲是覆盖在正文右侧的浮层，不参与内容行宽分配。
    v_flex()
        .id("markdown-outline")
        .debug_selector(|| "markdown-outline".to_owned())
        .absolute()
        .top(px(0.0))
        .right(px(0.0))
        .bottom(px(0.0))
        .w(px(240.0))
        .flex_shrink_0()
        .bg(cx.theme().background)
        .border_l_1()
        .border_color(cx.theme().border)
        .child(
            div()
                .h(px(32.0))
                .px_3()
                .flex()
                .items_center()
                .text_xs()
                .font_semibold()
                .child(language.text(Message::EditorOutline)),
        )
        .child(headings)
        .into_any_element()
}
