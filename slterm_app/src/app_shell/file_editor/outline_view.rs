//! 大纲侧栏(照上游同构):层级/折叠/跳转与正文同一标题模型。
//!
//! 跳转语义双模态同源:预览态滚动到标题块,源码态把光标放到标题行。

use super::reader_presentation as design;
use super::*;
use gpui::prelude::FluentBuilder as _;
use gpui::{
    InteractiveElement as _, IntoElement, MouseButton, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _,
};
use gpui_component::{Icon, IconName, StyledExt as _};

impl TextFileView {
    pub(super) fn render_outline_content(&self, cx: &mut Context<'_, Self>) -> gpui::AnyElement {
        let language = cx.global::<Settings>().language;
        let scroll = self.outline_scroll.clone();
        let headings = v_flex()
            .id("markdown-headings")
            .debug_selector(|| "markdown-outline-list".to_owned())
            .size_full()
            .min_h_0()
            .overflow_y_scroll()
            .track_scroll(&scroll)
            .px(px(12.0))
            .pr(px(20.0))
            .py(px(12.0))
            .when(self.outline.headings.is_empty(), |list| {
                list.child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(language.text(Message::EditorNoHeadings)),
                )
            })
            .children(self.outline.visible_headings(&self.collapsed_headings).map(
                |(index, heading)| {
                    let collapsed = self.collapsed_headings.contains(&index);
                    let has_children = self.outline.has_children(index);
                    let arrow = if has_children {
                        div()
                            .id(("outline-fold", index))
                            .size(px(20.0))
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .debug_selector(move || format!("outline-fold-{index}"))
                            .child(
                                Icon::new(if collapsed {
                                    IconName::ChevronRight
                                } else {
                                    IconName::ChevronDown
                                })
                                .size(px(12.0)),
                            )
                            .on_click(cx.listener(move |view, _, _, cx| {
                                cx.stop_propagation();
                                if !view.collapsed_headings.remove(&index) {
                                    view.collapsed_headings.insert(index);
                                }
                                cx.notify();
                            }))
                            .into_any_element()
                    } else {
                        div().size(px(20.0)).flex_shrink_0().into_any_element()
                    };
                    h_flex()
                        .id(("markdown-heading", index))
                        .h(px(design::OUTLINE_ROW_HEIGHT))
                        .w_full()
                        .min_w_0()
                        .flex_shrink_0()
                        .items_center()
                        .pl(px(heading.indent as f32 * design::OUTLINE_INDENT))
                        .pr(px(8.0))
                        .rounded(px(5.0))
                        .text_size(px(design::CHROME_SIZE))
                        .cursor_pointer()
                        .debug_selector(move || format!("outline-row-{index}"))
                        .text_color(if heading.indent == 0 {
                            cx.theme().foreground
                        } else {
                            cx.theme().muted_foreground
                        })
                        .when(self.selected_heading == Some(index), |row| {
                            row.bg(cx.theme().list_hover)
                                .text_color(cx.theme().foreground)
                                .font_semibold()
                        })
                        .hover(|row| row.bg(cx.theme().list_hover))
                        .active(|row| row.bg(cx.theme().list_active))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| {
                            gpui_component::GlobalState::suppress_text_selection(cx);
                        })
                        .child(arrow)
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .child(heading.label.clone()),
                        )
                        .on_click(cx.listener(move |view, _, window, cx| {
                            view.jump_to_heading(index, window, cx);
                        }))
                },
            ));
        div()
            .relative()
            .flex_1()
            .min_h_0()
            .overflow_hidden()
            .child(headings)
            .child(
                div()
                    .id("markdown-outline-scrollbar-host")
                    .absolute()
                    .top_0()
                    .right_0()
                    .bottom_0()
                    .w(px(16.0))
                    .debug_selector(|| "markdown-outline-scrollbar".to_owned())
                    .on_hover(cx.listener(|view, hovered: &bool, _, cx| {
                        view.outline_scrollbar_hovered = *hovered;
                        cx.notify();
                    }))
                    .child(
                        gpui_component::scroll::Scrollbar::vertical(&scroll).scrollbar_show(
                            if self.outline_scrollbar_hovered {
                                gpui_component::scroll::ScrollbarShow::Hover
                            } else {
                                gpui_component::scroll::ScrollbarShow::Scrolling
                            },
                        ),
                    ),
            )
            .into_any_element()
    }

    pub(super) fn render_outline(&self, cx: &mut Context<'_, Self>) -> gpui::AnyElement {
        v_flex()
            .id("markdown-outline")
            .debug_selector(|| "markdown-outline".to_owned())
            .relative()
            .w(px(design::clamp_details_width(self.details_width)))
            .min_w(px(design::DETAILS_MIN_WIDTH))
            .max_w(gpui::relative(0.42))
            .h_full()
            .flex_shrink_0()
            .border_l_1()
            .border_color(cx.theme().border)
            .child(self.render_details_header(cx))
            .child(self.render_outline_content(cx))
            .child(self.render_details_resize_handle(cx))
            .into_any_element()
    }

    pub(super) fn jump_to_heading(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(heading) = self.outline.headings.get(index).cloned() else {
            return;
        };
        self.selected_heading = Some(index);
        if self.preview {
            self.scroll.scroll_to(gpui::ListOffset {
                item_ix: heading.block,
                offset_in_item: px(0.0),
            });
        } else {
            let row = heading.row;
            self.input.update(cx, |input, cx| {
                input.set_cursor_position(
                    gpui_component::input::Position {
                        line: row,
                        character: 0,
                    },
                    window,
                    cx,
                )
            });
        }
        cx.notify();
    }
}
