//! 编辑增量到视图偏移的纯映射；不持有状态。

use slterm_document::TextEdit;

use super::super::source::TextSelection;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum BoundaryBias {
    Left,
    Right,
}

/// 将旧正文字节位置按一组有序、不重叠编辑映射到新正文。
pub(super) fn transform_offset(offset: usize, edits: &[TextEdit], bias: BoundaryBias) -> usize {
    let mut result = offset as i128;
    let mut delta = 0_i128;
    for edit in edits {
        let range = &edit.utf8_range();
        if offset < range.start {
            break;
        }
        let start = range.start as i128 + delta;
        let old_len = (range.end - range.start) as i128;
        let new_len = edit.replacement().len() as i128;
        if range.is_empty() {
            if offset == range.start {
                if bias == BoundaryBias::Right {
                    result = start + new_len;
                }
            } else {
                result = offset as i128 + delta + new_len;
            }
            delta += new_len;
            continue;
        }
        if offset > range.end {
            delta += new_len - old_len;
            result = offset as i128 + delta;
            continue;
        }
        result = if offset == range.start {
            if bias == BoundaryBias::Right {
                start + new_len
            } else {
                start
            }
        } else if offset == range.end || bias == BoundaryBias::Right {
            start + new_len
        } else {
            start
        };
        if offset == range.end {
            delta += new_len - old_len;
            continue;
        }
        break;
    }
    result as usize
}

pub(super) fn transform_selection(selection: TextSelection, edits: &[TextEdit]) -> TextSelection {
    if selection.anchor == selection.head {
        let cursor = transform_offset(selection.head, edits, BoundaryBias::Left);
        return TextSelection {
            anchor: cursor,
            head: cursor,
        };
    }
    let range = selection.range();
    let start = transform_offset(range.start, edits, BoundaryBias::Right);
    let end = transform_offset(range.end, edits, BoundaryBias::Left);
    if start > end {
        return TextSelection {
            anchor: end,
            head: end,
        };
    }
    if selection.anchor > selection.head {
        TextSelection {
            anchor: end,
            head: start,
        }
    } else {
        TextSelection {
            anchor: start,
            head: end,
        }
    }
}
