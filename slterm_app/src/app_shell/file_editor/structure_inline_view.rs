//! 混合段落内联 run 渲染(07 篇 M8.3 照抄点 7/10)。
//!
//! 只有聚焦 run 拥有原生输入;其余 run 保持普通文本/图片/公式呈现。
//! 段落内空白属于外层段落,按源码前后空白宽度显式补齐(TextView 片段
//! 只解析自身内容,不替外层保留空格)。

use super::block_structure::{BlockStructure, InlineRun, PartKind};
use super::*;
use gpui::prelude::FluentBuilder as _;
use gpui::{
    IntoElement, ObjectFit, StatefulInteractiveElement as _, Styled as _, StyledImage as _, img, px,
};
use gpui_component::StyledExt as _;
use gpui_component::text::{TextView, TextViewState, TextViewStyle};

impl TextFileView {
    pub(super) fn render_inline_parts(
        &self,
        runs: &[InlineRun],
        structure: &BlockStructure,
        source: &str,
        block: usize,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> gpui::AnyElement {
        let mut line = h_flex().w_full().min_w_0().flex_wrap().items_end();
        for run in runs {
            let part = run.part;
            let spec = &structure.parts[part];
            let active = self
                .live_edit
                .as_ref()
                .filter(|edit| edit.block == block && edit.part == Some(part));
            let mut frame = div()
                .id(("inline-edit-part", part))
                .debug_selector(move || format!("markdown-inline-part-{block}-{part}"))
                .min_w(px(2.0))
                .max_w_full()
                .flex_shrink_0()
                .cursor_text()
                .text_size(px(reader_presentation::heading_size(spec.heading)))
                .when(spec.heading.is_some(), |frame| {
                    frame.font_weight(reader_presentation::heading_weight(spec.heading))
                })
                .when(run.marks.bold, |frame| frame.font_bold())
                .when(run.marks.italic, |frame| frame.italic())
                .when(run.marks.link, |frame| frame.text_color(cx.theme().link))
                .on_click(cx.listener(move |view, event, window, cx| {
                    view.begin_live_part_at(block, Some(part), Some(event), window, cx);
                    cx.stop_propagation();
                }));
            if let Some(edit) = active {
                self.inline_views.borrow_mut().remove(&(block, part));
                // 只有活跃 run 需要输入实体;按不换行字形宽度取宽,短公式保持
                // 内联,长内容由父级封顶后原生输入自动换行。
                let mut style = window.text_style();
                style.font_family = if matches!(spec.kind, PartKind::Source) {
                    cx.theme().mono_font_family.clone()
                } else {
                    cx.theme().font_family.clone()
                };
                if spec.heading.is_some() {
                    style.font_weight = reader_presentation::heading_weight(spec.heading);
                }
                if run.marks.bold {
                    style.font_weight = gpui::FontWeight::BOLD;
                }
                if run.marks.italic {
                    style.font_style = gpui::FontStyle::Italic;
                }
                let font_size = px(if matches!(spec.kind, PartKind::Source) {
                    13.0
                } else {
                    reader_presentation::heading_size(spec.heading)
                });
                let width = edit
                    .projection
                    .text
                    .lines()
                    .map(|text| {
                        window
                            .text_system()
                            .shape_line(
                                text.to_owned().into(),
                                font_size,
                                &[style.to_run(text.len())],
                                None,
                            )
                            .width
                    })
                    .fold(px(0.0), |width, next| width.max(next));
                frame = frame
                    .w((width + px(12.0)).max(px(40.0)))
                    .child(self.render_live_input(cx));
            } else {
                let raw = &source[spec.range.clone()];
                let preview = spec.preview.as_deref().unwrap_or(raw);
                if is_image_markdown(preview) {
                    frame = frame.child(self.render_inline_image(preview, cx));
                } else {
                    let text = self.outline.part_source(block, preview);
                    let owner = cx.weak_entity();
                    let state = window.use_keyed_state(
                        gpui::SharedString::from(format!("inline-state-{block}-{part}")),
                        cx,
                        |_, cx| TextViewState::markdown(&text, cx),
                    );
                    state.update(cx, |state, cx| state.set_text(&text, cx));
                    self.inline_views
                        .borrow_mut()
                        .insert((block, part), state.downgrade());
                    // 片段解析器只负责自身内容:run 前后空格属于外层段落。
                    let leading = raw.len() - raw.trim_start_matches([' ', '\t', '\n', '\r']).len();
                    let trailing = raw.trim_start_matches([' ', '\t', '\n', '\r']).len()
                        - raw.trim_matches([' ', '\t', '\n', '\r']).len();
                    let style = window.text_style();
                    let space = window
                        .text_system()
                        .shape_line(
                            " ".into(),
                            px(reader_presentation::heading_size(spec.heading)),
                            &[style.to_run(1)],
                            None,
                        )
                        .width;
                    frame = frame.pl(space * leading as f32).pr(space * trailing as f32);
                    frame = frame.child(
                        TextView::new(&state)
                            .min_w_0()
                            .max_w_full()
                            .selectable(true)
                            .scrollable(false)
                            .style(TextViewStyle {
                                paragraph_gap: gpui::rems(0.0),
                                image_base: self.path.parent().map(std::sync::Arc::from),
                                highlight_theme: cx.theme().highlight_theme.clone(),
                                is_dark: cx.theme().is_dark(),
                                inline_code: gpui::HighlightStyle {
                                    background_color: Some(cx.theme().tokens.muted.into()),
                                    ..Default::default()
                                },
                                ..Default::default()
                            })
                            .on_link_click(move |url, event, window, cx| {
                                if event.modifiers().control || event.modifiers().platform {
                                    cx.open_url(url);
                                } else {
                                    let _ = owner.update(cx, |view, cx| {
                                        view.begin_live_part_at(
                                            block,
                                            Some(part),
                                            Some(event),
                                            window,
                                            cx,
                                        );
                                    });
                                }
                            })
                            .markdown_extensions(self.preview_extensions.clone()),
                    );
                }
            }
            line = line.child(frame);
        }
        line.into_any_element()
    }

    /// 段落内联图片:本地文件或远端可解析时交给 gpui;缺失时画可感知占位。
    fn render_inline_image(&self, raw: &str, cx: &mut Context<'_, Self>) -> gpui::AnyElement {
        let (alt, url) = image_markdown_parts(raw);
        let remote =
            url.starts_with("http://") || url.starts_with("https://") || url.starts_with("data:");
        let resolved = self.path.parent().map(|base| base.join(&url));
        let available = remote || resolved.as_ref().is_some_and(|path| path.is_file());
        if !available {
            return div()
                .debug_selector(|| "markdown-image-placeholder".to_owned())
                .min_w(px(120.0))
                .min_h(px(32.0))
                .px_2()
                .py_1()
                .rounded(px(4.0))
                .border_1()
                .border_color(cx.theme().border)
                .bg(cx.theme().muted)
                .text_size(px(reader_presentation::SECONDARY_SIZE))
                .text_color(cx.theme().muted_foreground)
                .child(if alt.is_empty() { url } else { alt })
                .into_any_element();
        }
        let source = if remote {
            url
        } else {
            resolved.unwrap().to_string_lossy().into_owned()
        };
        img(source)
            .debug_selector(|| "markdown-image".to_owned())
            .max_w_full()
            .object_fit(ObjectFit::Contain)
            .into_any_element()
    }
}

/// `![alt](url)` 的最小解析(结构层只把单图 run 标成 Source)。
fn is_image_markdown(raw: &str) -> bool {
    let trimmed = raw.trim_start();
    trimmed.starts_with("![") && trimmed.contains("](")
}

fn image_markdown_parts(raw: &str) -> (String, String) {
    let trimmed = raw.trim();
    let Some(rest) = trimmed.strip_prefix("![") else {
        return (String::new(), trimmed.to_owned());
    };
    let Some(close) = rest.find("](") else {
        return (String::new(), trimmed.to_owned());
    };
    let alt = rest[..close].to_owned();
    let url = rest[close + 2..].trim_end_matches(')').to_owned();
    (alt, url)
}

#[cfg(test)]
mod structure_inline_view_tests {
    use super::*;

    /// 图片 Markdown 解析:alt 与 URL 分离,普通文本不误判。
    #[test]
    fn image_markdown_parts_split_alt_and_url() {
        assert!(is_image_markdown("![图](images/a.png)"));
        assert!(!is_image_markdown("普通文本"));
        assert_eq!(
            image_markdown_parts("![图](images/a.png)"),
            ("图".to_owned(), "images/a.png".to_owned())
        );
        assert_eq!(
            image_markdown_parts("![](b.png)"),
            (String::new(), "b.png".to_owned())
        );
    }
}
