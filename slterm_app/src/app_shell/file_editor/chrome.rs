//! 文档 chrome(照上游同构):面包屑工具条 + 详情侧栏头/拖拽。
//!
//! chrome 与缓冲区/文档生命周期分离;一切几何取 `reader_presentation`,
//! 颜色取主题 token,组件内不写字面量。

use super::reader_presentation as design;
use super::*;
use gpui::prelude::FluentBuilder as _;
use gpui::{
    InteractiveElement as _, IntoElement, MouseButton, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _,
};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::{Icon, IconName, StyledExt as _};

impl TextFileView {
    pub(super) fn render_toolbar(
        &self,
        editable: bool,
        window: &gpui::Window,
        cx: &mut Context<'_, Self>,
    ) -> gpui::AnyElement {
        let language = cx.global::<Settings>().language;
        let muted = cx.theme().muted_foreground;
        let parent = self
            .path
            .parent()
            .and_then(|path| path.file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        h_flex()
            .id("file-toolbar")
            .debug_selector(|| "file-toolbar".to_owned())
            .h(px(design::TOOLBAR_HEIGHT))
            .flex_shrink_0()
            .px(px(design::PANEL_PADDING))
            .gap(px(9.0))
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                h_flex()
                    .flex_1()
                    .min_w_0()
                    .gap(px(7.0))
                    .child(
                        Icon::new(IconName::File)
                            .size(px(design::ICON_SIZE))
                            .text_color(muted),
                    )
                    .when(!parent.is_empty(), |row| {
                        row.child(
                            div()
                                .max_w(px(140.0))
                                .truncate()
                                .text_size(px(design::SECONDARY_SIZE))
                                .text_color(muted)
                                .child(parent),
                        )
                        .child(
                            div()
                                .text_size(px(design::SECONDARY_SIZE))
                                .text_color(muted)
                                .child("/"),
                        )
                    })
                    .child(
                        div()
                            .debug_selector(|| "file-toolbar-title".to_owned())
                            .min_w_0()
                            .truncate()
                            .text_size(px(design::CHROME_SIZE))
                            .font_semibold()
                            .child(self.title.clone()),
                    )
                    .when(self.dirty, |row| {
                        row.child(
                            div()
                                .debug_selector(|| "file-dirty-dot".to_owned())
                                .size(px(5.0))
                                .flex_shrink_0()
                                .rounded_full()
                                .bg(cx.theme().warning),
                        )
                    }),
            )
            .when(self.markdown, |bar| {
                let label = if self.preview {
                    language.text(Message::EditorSource)
                } else {
                    language.text(Message::EditorPreview)
                };
                bar.child(
                    div()
                        .id("file-toggle-source")
                        .debug_selector(|| "file-toggle-source".to_owned())
                        .h(px(design::CONTROL_HEIGHT))
                        .px(px(10.0))
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .rounded(px(4.0))
                        .cursor_pointer()
                        .text_size(px(design::SECONDARY_SIZE))
                        .text_color(muted)
                        .hover(|control| control.bg(cx.theme().list_hover))
                        .child(label)
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.toggle_preview(window, cx);
                        })),
                )
            })
            .child(div().w(px(1.0)).h(px(18.0)).mx_1().bg(cx.theme().border))
            .child(self.render_reader_focus_button(cx))
            .child(
                div()
                    .id("file-details-toggle")
                    .debug_selector(|| "file-details-toggle".to_owned())
                    .size(px(design::CONTROL_HEIGHT))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(4.0))
                    .cursor_pointer()
                    .when(self.show_details, |button| button.bg(cx.theme().list_hover))
                    .hover(|button| button.bg(cx.theme().list_hover))
                    .child(
                        Icon::new(IconName::PanelRight)
                            .size(px(design::ICON_SIZE))
                            .text_color(if self.show_details {
                                cx.theme().foreground
                            } else {
                                muted
                            }),
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.show_details = !this.show_details;
                        if !this.markdown {
                            this.info = true;
                        }
                        this.details_resize_anchor = None;
                        cx.notify();
                    })),
            )
            .child(self.render_fullscreen_button(window, cx))
            .child(
                div()
                    .id("file-save")
                    .debug_selector(|| "file-save".to_owned())
                    .h(px(design::CONTROL_HEIGHT))
                    .px(px(12.0))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .rounded(px(4.0))
                    .text_size(px(design::CHROME_SIZE))
                    .when(editable && self.dirty && !self.saving, |button| {
                        button
                            .cursor_pointer()
                            .hover(|button| button.bg(cx.theme().list_hover))
                    })
                    .when_some(self.save_button_label(cx), |button, label| {
                        button.child(label)
                    })
                    .on_click(cx.listener(|this, _, _, cx| {
                        if this.is_read_only() || !this.dirty || this.saving {
                            return;
                        }
                        this.save(cx).detach();
                    })),
            )
            .into_any_element()
    }

    /// 聚焦阅读切换按钮:工具条内按钮与退出浮层共用同一 selector,
    /// 保证工具条隐藏后仍留有唯一的恢复入口。
    fn render_reader_focus_button(&self, cx: &mut Context<'_, Self>) -> gpui::AnyElement {
        let language = cx.global::<Settings>().language;
        let (icon, tooltip) = if self.reader_focus {
            (IconName::EyeOff, Message::EditorReaderFocusExit)
        } else {
            (IconName::BookOpen, Message::EditorReaderFocus)
        };
        Button::new("file-reader-focus")
            .debug_selector(|| "file-reader-focus".to_owned())
            .icon(icon)
            .ghost()
            .tooltip(language.text(tooltip))
            .on_click(cx.listener(|this, _, window, cx| this.toggle_reader_focus(window, cx)))
            .into_any_element()
    }

    /// 全屏切换按钮:图标与提示跟随 `Window::is_fullscreen` 当前态。
    fn render_fullscreen_button(
        &self,
        window: &gpui::Window,
        cx: &mut Context<'_, Self>,
    ) -> gpui::AnyElement {
        let language = cx.global::<Settings>().language;
        let (icon, tooltip) = if window.is_fullscreen() {
            (IconName::Minimize, Message::EditorFullscreenExit)
        } else {
            (IconName::Maximize, Message::EditorFullscreenEnter)
        };
        Button::new("file-fullscreen")
            .debug_selector(|| "file-fullscreen".to_owned())
            .icon(icon)
            .ghost()
            .tooltip(language.text(tooltip))
            .on_click(cx.listener(|_, _, window, cx| {
                // 只切窗口全屏态;不新增窗口控制命令、不改标题栏。
                window.toggle_fullscreen();
                cx.notify();
            }))
            .into_any_element()
    }

    /// reader-focus 退出浮层:工具条整体隐藏后,右上角保留恢复入口。
    pub(super) fn render_reader_focus_overlay(
        &self,
        cx: &mut Context<'_, Self>,
    ) -> gpui::AnyElement {
        let tokens = cx.global::<Settings>().editor_theme;
        let background = crate::gpui_terminal::colors::from_rgb8(tokens.background);
        div()
            .id("file-reader-focus-overlay")
            .debug_selector(|| "file-reader-focus-overlay".to_owned())
            .absolute()
            .top(px(10.0))
            .right(px(10.0))
            .occlude()
            .p(px(2.0))
            .rounded(px(6.0))
            .border_1()
            .border_color(cx.theme().border)
            .bg(background.alpha(0.92))
            .child(self.render_reader_focus_button(cx))
            .into_any_element()
    }

    /// 保存按钮文案:保存中显示「保存中」;其余数字文案由 i18n 单点提供。
    fn save_button_label(&self, cx: &App) -> Option<&'static str> {
        let language = cx.global::<Settings>().language;
        if self.saving {
            return Some(language.text(Message::EditorSavingUi));
        }
        Some(language.text(Message::EditorSaveUi))
    }

    pub(super) fn render_details_header(&self, cx: &mut Context<'_, Self>) -> gpui::AnyElement {
        let language = cx.global::<Settings>().language;
        h_flex()
            .id("file-details-header")
            .debug_selector(|| "file-details-header".to_owned())
            .h(px(design::PANEL_HEADER_HEIGHT))
            .flex_shrink_0()
            .px(px(design::PANEL_PADDING))
            .gap(px(20.0))
            .border_b_1()
            .border_color(cx.theme().border)
            .children(
                [
                    (false, "file-toggle-outline", Message::EditorOutline),
                    (true, "file-info", Message::EditorInfo),
                ]
                .into_iter()
                .filter(|(info, _, _)| *info || self.markdown)
                .map(|(info, id, label)| {
                    div()
                        .id(id)
                        .debug_selector(move || id.to_owned())
                        .relative()
                        .h_full()
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .cursor_pointer()
                        .text_size(px(design::SECONDARY_SIZE))
                        .text_color(if self.info == info {
                            cx.theme().foreground
                        } else {
                            cx.theme().muted_foreground
                        })
                        .when(self.info == info, |tab| tab.font_semibold())
                        .child(language.text(label))
                        .when(self.info == info, |tab| {
                            tab.child(
                                div()
                                    .debug_selector(|| "file-details-indicator".to_owned())
                                    .absolute()
                                    .bottom_0()
                                    .left_0()
                                    .right_0()
                                    .h(px(2.0))
                                    .bg(cx.theme().foreground.opacity(0.55)),
                            )
                        })
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.info = info;
                            this.show_details = true;
                            cx.notify();
                        }))
                }),
            )
            .child(div().flex_1())
            .child(
                div()
                    .id("file-details-close")
                    .debug_selector(|| "file-details-close".to_owned())
                    .size(px(design::CONTROL_HEIGHT))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(4.0))
                    .cursor_pointer()
                    .hover(|button| button.bg(cx.theme().list_hover))
                    .child(Icon::new(IconName::Close).size(px(design::ICON_SIZE)))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.show_details = false;
                        cx.notify();
                    })),
            )
            .into_any_element()
    }

    /// 8px 命中区居中对齐 1px 面板分隔线:大纲与信息共用同一宽度。
    pub(super) fn render_details_resize_handle(
        &self,
        cx: &mut Context<'_, Self>,
    ) -> gpui::AnyElement {
        let hover_line = cx.theme().border.opacity(0.45);
        div()
            .id("file-details-resize-handle")
            .debug_selector(|| "file-details-resize-handle".to_owned())
            .absolute()
            .top_0()
            .bottom_0()
            .left(px(-(design::DETAILS_RESIZE_HIT_WIDTH * 0.5)))
            .w(px(design::DETAILS_RESIZE_HIT_WIDTH))
            .cursor_col_resize()
            .hover(move |handle| handle.bg(hover_line))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event, _, cx| {
                    gpui_component::GlobalState::suppress_text_selection(cx);
                    this.begin_details_resize(event, cx);
                    cx.stop_propagation();
                }),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod chrome_tests {
    use super::super::info::read_modified_time;
    use std::time::{Duration, UNIX_EPOCH};

    /// 修改时间格式化:已知纪元秒 → 固定 UTC 字符串(信息栏展示面)。
    #[test]
    fn modified_time_renders_as_a_stable_utc_label() {
        let time = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        assert_eq!(read_modified_time(time), "2023-11-14 22:13 UTC");
    }
}
