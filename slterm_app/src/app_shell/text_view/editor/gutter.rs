//! Editor 左侧槽位：语法 run 构造、Git 行标投影与行号/折叠标记绘制辅助。

use std::ops::Range;

use gpui::{
    Context, Font, Hsla, InteractiveElement as _, IntoElement as _, MouseButton, Styled as _,
    TextRun, div, px,
};
use gpui_component::ActiveTheme as _;
use ropey::LineType;

use super::super::source::VisibleLine;
use super::super::syntax::capture_color;
use super::{DocumentTextEvent, DocumentTextView, GitMarkKind};

/// 按语法 span 与默认前景构造一行 shaping runs。
pub(super) fn line_text_runs(
    line: &VisibleLine,
    spans: Vec<(Range<usize>, String)>,
    font: &Font,
    foreground: Hsla,
    colors: &gpui_component::ThemeColor,
) -> Vec<TextRun> {
    let make_run = |len, color| TextRun {
        len,
        font: font.clone(),
        color,
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let mut runs = Vec::new();
    let mut cursor = 0;
    for (range, capture) in spans {
        if range.start > cursor {
            runs.push(make_run(range.start - cursor, foreground));
        }
        cursor = range.end;
        runs.push(make_run(range.len(), capture_color(colors, &capture)));
    }
    if cursor < line.text().len() || runs.is_empty() {
        runs.push(make_run(line.text().len() - cursor, foreground));
    }
    runs
}

impl DocumentTextView {
    /// Git 行标是 AppRoot 注入的最新差分投影；折叠隐藏的行不绘制。
    pub(super) fn git_markers(&self, cx: &mut Context<'_, Self>) -> Vec<gpui::AnyElement> {
        let scroll_line = self.clamped_scroll_line();
        let hidden = self.hidden_rows();
        let last_line = self.text.len_lines(LineType::LF).saturating_sub(1);
        let line_height = self
            .metrics()
            .map_or(px(20.0), |metrics| metrics.line_height);
        self.git_marks
            .iter()
            .filter(|mark| mark.line <= last_line)
            .filter_map(|mark| {
                let line = mark.line;
                let (color, width) = match mark.kind {
                    GitMarkKind::Added => (cx.theme().success_foreground, 4.0),
                    GitMarkKind::Modified => (cx.theme().warning_foreground, 4.0),
                    GitMarkKind::Deleted => (cx.theme().danger, 2.0),
                };
                let raw = if self.wrap {
                    let byte = self.text.line_to_byte_idx(line, LineType::LF);
                    self.wrap_position(byte)?.row
                } else {
                    line
                };
                let row = super::fold::visible_row(raw, &hidden)?.checked_sub(scroll_line)?;
                let id = format!("document-git-marker-{}-{line}", cx.entity().entity_id());
                let selector = id.clone();
                Some(
                    div()
                        .id(id)
                        .debug_selector(move || selector.clone())
                        .absolute()
                        .left(px(0.0))
                        .top(line_height * row as f32)
                        .w(px(width))
                        .h(line_height)
                        .bg(color)
                        .cursor_pointer()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |_, _, _, cx| {
                                cx.emit(DocumentTextEvent::OpenGitDiff { line });
                            }),
                        )
                        .into_any_element(),
                )
            })
            .collect()
    }
}
