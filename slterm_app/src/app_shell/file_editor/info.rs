//! 文件信息侧栏(照上游同构):字符/行数/编码/行尾/大小/修改时间。
//!
//! 统计以工作副本为准(草稿态也如实反映);属性取载入快照。宽度与面板
//! 头共享 `reader_presentation` 常量族。

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::reader_presentation as design;
use super::*;
use gpui::{
    ClipboardItem, InteractiveElement as _, IntoElement, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _,
};
use gpui_component::{Icon, IconName, StyledExt as _};

/// 系统时间 → 固定 UTC 标签(不引入时区依赖;信息栏只要求可读与稳定)。
pub(super) fn read_modified_time(time: SystemTime) -> String {
    let seconds = time
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration: Duration| duration.as_secs()) as i64;
    let days = seconds.div_euclid(86_400);
    let remainder = seconds.rem_euclid(86_400);
    // Howard Hinnant civil_from_days:把 Unix 日序换算为公历年月日。
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = if month_prime < 10 {
        month_prime + 3
    } else {
        month_prime - 9
    };
    let year = if month <= 2 { year + 1 } else { year };
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02} UTC",
        remainder / 3_600,
        remainder % 3_600 / 60
    )
}

impl TextFileView {
    pub(super) fn render_info_content(&self, cx: &mut Context<'_, Self>) -> gpui::AnyElement {
        let language = cx.global::<Settings>().language;
        let muted = cx.theme().muted_foreground;
        let text = self.input.read(cx).value();
        let kind = self
            .path
            .extension()
            .unwrap_or_default()
            .to_string_lossy()
            .to_uppercase();
        let statistics = vec![
            (Message::EditorCharacters, text.chars().count().to_string()),
            (Message::EditorLineCount, text.lines().count().to_string()),
            (
                Message::EditorStatus,
                language
                    .text(if self.dirty {
                        Message::EditorUnsaved
                    } else {
                        Message::EditorSaved
                    })
                    .to_owned(),
            ),
        ];
        let mut properties = Vec::new();
        if let Some(document) = &self.document {
            properties.push((Message::EditorSize, format!("{} B", document.total_bytes)));
            properties.push((
                Message::EditorEncodingLabel,
                if document.invalid_encoding {
                    "—"
                } else if document.bom {
                    "UTF-8 BOM"
                } else {
                    "UTF-8"
                }
                .to_owned(),
            ));
            properties.push((
                Message::EditorLineEnding,
                if document.crlf { "CRLF" } else { "LF" }.to_owned(),
            ));
            if let Some(modified) = document.modified() {
                properties.push((Message::EditorModified, read_modified_time(modified)));
            }
        }
        let section = |title, rows: Vec<(Message, String)>| {
            v_flex()
                .gap(px(4.0))
                .mt(px(24.0))
                .child(
                    div()
                        .mb(px(10.0))
                        .text_size(px(design::SECONDARY_SIZE))
                        .text_color(muted)
                        .child(language.text(title)),
                )
                .children(rows.into_iter().map(|(label, value)| {
                    h_flex()
                        .w_full()
                        .items_start()
                        .gap_2()
                        .py(px(5.0))
                        .text_size(px(design::SECONDARY_SIZE))
                        .child(
                            div()
                                .flex_shrink_0()
                                .text_color(muted)
                                .child(language.text(label)),
                        )
                        .child(div().flex_1().min_w_0().text_right().child(value))
                }))
        };
        let path = self.path.display().to_string();
        let name = self
            .path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let action = |id: &'static str, icon: IconName, label: Message| {
            div()
                .id(id)
                .debug_selector(move || id.to_owned())
                .w_full()
                .h(px(design::ACTION_HEIGHT))
                .px(px(6.0))
                .flex()
                .items_center()
                .gap(px(10.0))
                .rounded(px(4.0))
                .cursor_pointer()
                .text_size(px(design::SECONDARY_SIZE))
                .text_color(muted)
                .hover(|row| row.bg(cx.theme().list_hover))
                .child(Icon::new(icon).size(px(design::ICON_SIZE)))
                .child(language.text(label))
        };
        let actions = v_flex()
            .gap(px(3.0))
            .mt(px(24.0))
            .pt(px(14.0))
            .border_t_1()
            .border_color(cx.theme().border)
            .child(
                action("info-copy-path", IconName::Copy, Message::EditorCopyPath).on_click(
                    move |_, _, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string(path.clone()));
                    },
                ),
            )
            .child(
                action("info-copy-name", IconName::File, Message::EditorCopyName).on_click(
                    move |_, _, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string(name.clone()));
                    },
                ),
            );
        v_flex()
            .id("file-info-body")
            .debug_selector(|| "file-info-body".to_owned())
            .flex_1()
            .min_h_0()
            .px(px(design::PANEL_PADDING))
            .py(px(22.0))
            .overflow_y_scroll()
            .child(
                h_flex()
                    .gap(px(10.0))
                    .items_center()
                    .child(
                        div()
                            .w(px(32.0))
                            .h(px(38.0))
                            .flex_shrink_0()
                            .rounded(px(5.0))
                            .border_1()
                            .border_color(cx.theme().border)
                            .bg(cx.theme().muted)
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(px(10.0))
                            .font_semibold()
                            .text_color(muted)
                            .child(kind.clone()),
                    )
                    .child(
                        v_flex()
                            .min_w_0()
                            .gap(px(3.0))
                            .child(
                                div()
                                    .text_size(px(design::CHROME_SIZE))
                                    .font_semibold()
                                    .child(self.title.clone()),
                            )
                            .child(
                                div()
                                    .text_size(px(design::SECONDARY_SIZE))
                                    .text_color(muted)
                                    .child(if self.markdown {
                                        language.text(Message::EditorMarkdownDocument).to_owned()
                                    } else {
                                        kind
                                    }),
                            ),
                    ),
            )
            .child(
                div()
                    .mt(px(12.0))
                    .text_size(px(design::SECONDARY_SIZE))
                    .line_height(gpui::relative(1.7))
                    .text_color(muted)
                    .child(self.path.display().to_string()),
            )
            .child(section(Message::EditorStatistics, statistics))
            .child(section(Message::EditorProperties, properties))
            .child(actions)
            .into_any_element()
    }

    pub(super) fn render_info(&self, cx: &mut Context<'_, Self>) -> gpui::AnyElement {
        v_flex()
            .id("file-info-panel")
            .debug_selector(|| "file-info-panel".to_owned())
            .relative()
            .w(px(design::clamp_details_width(self.details_width)))
            .min_w(px(design::DETAILS_MIN_WIDTH))
            .max_w(gpui::relative(0.42))
            .h_full()
            .flex_shrink_0()
            .border_l_1()
            .border_color(cx.theme().border)
            .child(self.render_details_header(cx))
            .child(self.render_info_content(cx))
            .child(self.render_details_resize_handle(cx))
            .into_any_element()
    }
}
