//! view 层纯函数单测(光标闪烁相位、选区自动回滚分档、字体特性位)。

use gpui::{FontStyle, FontWeight};

use super::{cursor_blink_allowed, restart_cursor_blink_phase, selection_scroll_lines, typography};

#[test]
fn ime_preedit_stops_blinking_and_restores_visible_cursor() {
    assert!(!cursor_blink_allowed(true, false, true, true, true, true));

    let (mut visible, mut epoch) = (false, 7);
    assert_eq!(restart_cursor_blink_phase(&mut visible, &mut epoch), 8);
    assert!(visible);
}

#[test]
fn unfocused_cursor_does_not_blink_and_stays_visible() {
    assert!(!cursor_blink_allowed(true, false, true, false, false, true));
    assert!(!cursor_blink_allowed(true, false, true, false, true, false));

    let (mut visible, mut epoch) = (false, 2);
    restart_cursor_blink_phase(&mut visible, &mut epoch);
    assert!(visible);
}

#[test]
fn restoring_focus_restarts_blinking_immediately() {
    assert!(!cursor_blink_allowed(true, false, true, false, true, false));
    assert!(cursor_blink_allowed(true, false, true, false, true, true));

    let (mut visible, mut epoch) = (false, 11);
    assert_eq!(restart_cursor_blink_phase(&mut visible, &mut epoch), 12);
    assert!(
        visible,
        "restart must restore the visible phase before the next tick"
    );
}

/// 拖选自动回滚的判据:网格内不滚,越过上边界往回滚历史、越过下边界
/// 往前滚,贴边 1 行、每远离 20px 加一档,并且封顶在一屏——指针甩出
/// 窗口不该一抖到底。
#[test]
fn selection_scroll_speed_scales_with_distance_past_the_grid() {
    let (top, bottom, rows) = (100.0, 500.0, 30);
    assert_eq!(
        selection_scroll_lines(300.0, top, bottom, rows),
        0,
        "网格内不滚"
    );
    assert_eq!(
        selection_scroll_lines(top, top, bottom, rows),
        0,
        "上边界本身仍在网格内"
    );
    assert_eq!(
        selection_scroll_lines(top - 1.0, top, bottom, rows),
        1,
        "刚越界=最慢一档"
    );
    assert_eq!(selection_scroll_lines(top - 20.0, top, bottom, rows), 2);
    assert_eq!(selection_scroll_lines(top - 40.0, top, bottom, rows), 3);
    assert_eq!(
        selection_scroll_lines(bottom, top, bottom, rows),
        -1,
        "底边像素属于下方"
    );
    assert_eq!(selection_scroll_lines(bottom + 20.0, top, bottom, rows), -2);
    assert_eq!(
        selection_scroll_lines(-10_000.0, top, bottom, rows),
        rows,
        "甩到屏幕外也每 tick 最多一屏"
    );
    assert_eq!(selection_scroll_lines(10_000.0, top, bottom, rows), -rows);
}

/// 四族字体都必须显式写死连字特性位(含关闭态),且不启用 kern:
/// 平台缺省各异,Windows 后端忽略空特性列表。
#[test]
fn all_font_faces_explicitly_control_ligatures_without_enabling_kerning() {
    for enabled in [true, false] {
        for weight in [FontWeight::NORMAL, FontWeight::BOLD] {
            for style in [FontStyle::Normal, FontStyle::Italic] {
                let font = typography::mono_font(
                    crate::app_shell::assets::REQUIRED_FONT_FAMILY,
                    weight,
                    style,
                    enabled,
                );
                assert_eq!(
                    font.features.tag_value_list(),
                    &[
                        ("calt".to_owned(), u32::from(enabled)),
                        ("liga".to_owned(), u32::from(enabled)),
                        ("clig".to_owned(), u32::from(enabled)),
                        ("kern".to_owned(), 0),
                    ]
                );
            }
        }
    }
}
