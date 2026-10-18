//! `pane.read` 的终端模型只读投影(04 篇锚点,02 篇 `Term` 只消费)。
//!
//! 直接读真实 Grid 尾部:范围锚定 buffer 底部(不是 `display_offset`),
//! 用户上翻历史、选区与光标都不受影响;不足一屏时多扫一段再回退,字节
//! 上限 [`MAX_READ_BYTES`] 超限按整行递减,最后才做 UTF-8 字符边界切片。

// 消费点 = M7.5 壳事件泵的 pane.read 执行器(`RuntimeControl` 臂调用)。
#![allow(dead_code)]

use super::{ApiError, MAX_READ_BYTES, RuntimeTaskState};
use serde::{Deserialize, Serialize};
use slterm_terminal::event::EventListener;
use slterm_terminal::grid::Dimensions;
use slterm_terminal::index::{Column, Line, Point};
use slterm_terminal::term::Term;

/// `pane.read` 响应体(screen v1 不迁,首发只抄文本尾部)。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimePaneRead {
    pub window_id: u64,
    pub pane_id: u64,
    pub text: String,
    pub requested_lines: usize,
    pub returned_lines: usize,
    /// `total_lines - screen_lines`(历史可回滚量)。
    pub history_available: usize,
    pub truncated: bool,
    pub task_state: RuntimeTaskState,
    pub exited: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_reason: Option<String>,
}

/// 读逻辑尾部 `requested_lines` 行。空终端返回空文本而不是错误。
pub(crate) fn capture_terminal_tail<T: EventListener>(
    term: &Term<T>,
    window_id: u64,
    pane_id: u64,
    requested_lines: usize,
    task_state: RuntimeTaskState,
    exited: bool,
    exit_reason: Option<String>,
) -> Result<RuntimePaneRead, ApiError> {
    let columns = term.columns();
    let screen_lines = term.screen_lines();
    let total_lines = term.total_lines();
    let history_available = total_lines.saturating_sub(screen_lines);
    if columns == 0 || screen_lines == 0 || total_lines == 0 {
        return Ok(RuntimePaneRead {
            window_id,
            pane_id,
            text: String::new(),
            requested_lines,
            returned_lines: 0,
            history_available,
            truncated: false,
            task_state,
            exited,
            exit_reason,
        });
    }

    let mut returned_lines = requested_lines.min(total_lines);
    let end = Point::new(Line(screen_lines as i32 - 1), Column(columns - 1));
    let capture = |lines: usize| {
        let start_line = screen_lines as i64 - lines as i64;
        let start = Point::new(
            Line(start_line.max(-(history_available as i64)) as i32),
            Column(0),
        );
        term.bounds_to_string(start, end)
    };
    let mut text = capture(returned_lines);

    // 先按整行递减保留准确的 returned_lines;只有单行仍超限时才退到
    // UTF-8 字节切片。
    while text.len() > MAX_READ_BYTES && returned_lines > 1 {
        let estimated = ((returned_lines as u128 * MAX_READ_BYTES as u128) / text.len() as u128)
            .clamp(1, (returned_lines - 1) as u128) as usize;
        returned_lines = estimated;
        text = capture(returned_lines);
    }
    let mut byte_truncated = false;
    if text.len() > MAX_READ_BYTES {
        let mut start = text.len() - MAX_READ_BYTES;
        while !text.is_char_boundary(start) {
            start += 1;
        }
        text = text[start..].to_owned();
        byte_truncated = true;
    }

    Ok(RuntimePaneRead {
        window_id,
        pane_id,
        text,
        requested_lines,
        returned_lines,
        history_available,
        truncated: returned_lines < total_lines || byte_truncated,
        task_state,
        exited,
        exit_reason,
    })
}

#[cfg(test)]
mod terminal_read_tests {
    use super::*;
    use slterm_terminal::event::VoidListener;
    use slterm_terminal::grid::Scroll;
    use slterm_terminal::term::test::TermSize;
    use vte::ansi::Processor;

    fn terminal(columns: usize, lines: usize, text: &str) -> Term<VoidListener> {
        let mut term = Term::new(
            Default::default(),
            &TermSize::new(columns, lines),
            VoidListener,
        );
        let mut parser: Processor = Processor::new();
        parser.advance(&mut term, text.as_bytes());
        term
    }

    #[test]
    fn tail_is_anchored_at_the_buffer_bottom_not_the_user_viewport() {
        let mut term = terminal(12, 2, "old\r\nsecond\r\nlatest");
        let before = capture_terminal_tail(&term, 1, 2, 8, RuntimeTaskState::Idle, false, None)
            .expect("tail");
        assert!(before.text.contains("latest"));
        assert_eq!(before.window_id, 1);
        assert_eq!(before.pane_id, 2);
        assert!(before.history_available > 0);
        // 用户上翻历史不改变外部读取结果。
        term.scroll_display(Scroll::Top);
        let after = capture_terminal_tail(&term, 1, 2, 8, RuntimeTaskState::Idle, false, None)
            .expect("tail");
        assert_eq!(before.text, after.text);
    }

    #[test]
    fn empty_terminal_returns_empty_text_without_error() {
        let term = terminal(0, 0, "");
        let read = capture_terminal_tail(&term, 3, 4, 120, RuntimeTaskState::Idle, false, None)
            .expect("empty tail");
        assert!(read.text.is_empty());
        assert_eq!(read.returned_lines, 0);
        assert!(!read.truncated);
    }

    #[test]
    fn requested_lines_are_clamped_to_the_buffer() {
        let term = terminal(20, 3, "one\r\ntwo\r\nthree");
        let read = capture_terminal_tail(
            &term,
            1,
            2,
            2_000,
            RuntimeTaskState::Finished,
            true,
            Some("eof".into()),
        )
        .expect("tail");
        assert!(read.returned_lines <= read.requested_lines);
        assert!(read.text.contains("three"));
        assert!(read.exited);
        assert_eq!(read.exit_reason.as_deref(), Some("eof"));
    }
}
