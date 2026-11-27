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
    MarkdownAnchorKind, MarkdownBlock, MarkdownBlockKind, MarkdownDocument, MarkdownInline,
    MarkdownInlineKind, MarkdownListItem, MarkdownStructure,
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
    let base = div()
        .id(("markdown-block", index))
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
        MarkdownBlockKind::Html(_) => base
            .child(document.visible_text()[block.visible_range()].to_owned())
            .into_any_element(),
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
    if !item.inlines().is_empty() {
        children = children.child(rich_text(
            SharedString::from(format!(
                "markdown-list-item-{}-{}-{item_index}",
                block.id().index(),
                item.depth()
            )),
            item.inlines(),
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
    let mut text = String::new();
    let mut runs = Vec::new();
    let mut links = Vec::new();
    let mut link_ranges = Vec::new();
    let mut images = Vec::new();
    let mut formulas = Vec::new();
    let mut segments = Vec::new();
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
    for inline in inlines {
        if inline.kind() == MarkdownInlineKind::Image {
            images.push((
                inline.text().to_owned(),
                inline.url().unwrap_or_default().to_owned(),
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
                formulas.push(image);
                continue;
            }
        }
        let value = inline.text();
        if value.is_empty() {
            continue;
        }
        let start = text.len();
        text.push_str(value);
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
                link_ranges.push(start..text.len());
                links.push(link.clone());
            }
        }
        segments.push(LayoutSegment {
            local: start..text.len(),
            visible: sequence_base + inline.visible_range().start
                ..sequence_base + inline.visible_range().end,
        });
        runs.push(run);
    }
    let has_text = !runs.is_empty();
    let selection = owner.update(cx, |view, _| view.selection).ok().flatten();
    let runs = highlight_selected_runs(runs, &segments, selection, cx.theme().selection);
    let styled = StyledText::new(SharedString::from(text)).with_runs(runs);
    // TextLayout 克隆共享测量状态；注册实际绘制实例，鼠标命中才有 prepaint 边界。
    let _ = owner.update(cx, |view, _| {
        view.register_text_layout(id.to_string(), styled.layout().clone(), segments);
    });
    let content = if links.is_empty() {
        styled.into_any_element()
    } else {
        let link_owner = owner.clone();
        InteractiveText::new(id.clone(), styled)
            .on_click(link_ranges, move |index, _, cx| {
                let Some(link) = links.get(index).cloned() else {
                    return;
                };
                let _ = link_owner.update(cx, |_, cx| cx.emit(MarkdownViewEvent::OpenLink(link)));
            })
            .into_any_element()
    };
    let content = selection_container(id, content, owner);
    if images.is_empty() && formulas.is_empty() {
        return content;
    }
    let mut container = v_flex().w_full().gap_2();
    if has_text {
        container = container.child(content);
    }
    for image in formulas {
        // 行内公式同样按窗口 scale 校正逻辑宽度。
        let pixels = image.size(0);
        let (width, _) = logical_formula_size(
            i32::from(pixels.width) as f32,
            i32::from(pixels.height) as f32,
            scale_factor,
        );
        container = container.child(
            gpui::img(image)
                .w(px(width))
                .object_fit(gpui::ObjectFit::Contain)
                .into_any_element(),
        );
    }
    for (alt, url) in images {
        let image = owner
            .update(cx, |view, cx| {
                view.ensure_image(block.id().index(), &url, owner, cx);
                view.image(&url)
            })
            .ok()
            .flatten();
        let child = div()
            .min_h(px(48.0))
            .p_2()
            .rounded(px(4.0))
            .border_1()
            .border_color(cx.theme().border)
            .text_color(cx.theme().muted_foreground)
            .child(if alt.is_empty() { url.clone() } else { alt });
        let image_owner = owner.clone();
        let image_url = url.clone();
        container = container.child(
            div()
                .id(SharedString::from(format!("markdown-image-{url}")))
                .cursor_pointer()
                .on_click(move |_, _, cx| {
                    let _ = image_owner.update(cx, |view, cx| {
                        view.load_image_on_click(&image_url, &image_owner, cx);
                    });
                })
                .child(if let Some(image) = image {
                    gpui::img(image)
                        .max_w_full()
                        .object_fit(gpui::ObjectFit::Contain)
                        .into_any_element()
                } else {
                    child.into_any_element()
                }),
        );
    }
    container.into_any_element()
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
