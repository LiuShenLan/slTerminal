//! 跨表示统一撤销历史。
//!
//! 源码态键入、渲染态 live_edit 提交与结构化命令共享同一份历史——输入实体
//! 的寿命不决定可撤销性。条目按差量存储(rope 差量提取,不物化全文),
//! 连续输入按 600ms 窗口与相邻性合并为一个撤销组;总字节上限 8MB,超限
//! 从最旧组开始丢弃。
//!
//! 消费点:M8.2 源码态 `TextFileView` 的输入订阅(record_rope);`travel` 的
//! UI 接线归 M8.3(渲染态撤销与源码态同栈)。

use std::ops::Range;
use std::time::{Duration, Instant};

use gpui_component::Rope;

/// 撤销历史字节上限(契约值;差量 before+after 之和)。
const MAX_HISTORY_BYTES: usize = 8 * 1024 * 1024;

struct Change {
    start: usize,
    before: String,
    after: String,
}

impl Change {
    fn bytes(&self) -> usize {
        self.before.len() + self.after.len()
    }
}

/// 不物化全文地定位 rope 的变更区间。
///
/// `InputState::value()` 会把底层 Rope 转成新 String;普通键入只需要光标处
/// 的小替换,因此逐字符比较两端,只把差值复制进历史状态。
fn changed_rope_span(before: &str, after: &Rope) -> (Range<usize>, Range<usize>) {
    let mut prefix = 0;
    let mut after_chars = after.chars();
    for (offset, before_char) in before.char_indices() {
        if after_chars.next() == Some(before_char) {
            prefix = offset + before_char.len_utf8();
        } else {
            break;
        }
    }

    let mut before_suffix = before[prefix..].chars().rev();
    let after_tail = after.slice(prefix..);
    let mut after_suffix = after_tail.chars_at(after_tail.len());
    let mut suffix = 0;
    while let (Some(before_char), Some(after_char)) = (before_suffix.next(), after_suffix.prev()) {
        if before_char != after_char {
            break;
        }
        suffix += before_char.len_utf8();
    }

    (prefix..before.len() - suffix, prefix..after.len() - suffix)
}

#[derive(Default)]
pub(crate) struct EditHistory {
    current: String,
    undo: Vec<Vec<Change>>,
    redo: Vec<Vec<Change>>,
    bytes: usize,
    last_edit: Option<Instant>,
}

impl EditHistory {
    /// 载入/重载后重置:当前文本成为新的基线,历史与合并窗口清空。
    pub(crate) fn reset(&mut self, text: &str) {
        *self = Self {
            current: text.to_owned(),
            ..Self::default()
        };
    }

    /// 撤销组边界:保存、失焦、结构提交等显式断点后不再与之前键入合并。
    pub(crate) fn barrier(&mut self) {
        self.last_edit = None;
    }

    /// 从输入实体的 Rope 直接记录局部编辑(源码态键入热路径)。
    pub(crate) fn record_rope(&mut self, text: &Rope) {
        let (before, after) = changed_rope_span(&self.current, text);
        if before.is_empty() && after.is_empty() {
            return;
        }

        let change = Change {
            start: before.start,
            before: self.current[before.clone()].to_owned(),
            after: text.slice(after).to_string(),
        };
        self.current.replace_range(before, &change.after);
        self.push_change(change);
    }

    /// 记录一次全文替换(结构化命令/渲染态提交的统一入口)。
    #[allow(dead_code)] // 消费点 = M8.3 live_edit 提交与结构命令。
    pub(crate) fn record(&mut self, text: &str) {
        if text == self.current {
            return;
        }
        let (before, after) = super::inline_edit::changed_span(&self.current, text);
        let change = Change {
            start: before.start,
            before: self.current[before.clone()].to_owned(),
            after: text[after].to_owned(),
        };
        self.current.replace_range(before, &change.after);
        self.push_change(change);
    }

    fn push_change(&mut self, change: Change) {
        self.bytes -= self.redo.iter().flatten().map(Change::bytes).sum::<usize>();
        self.redo.clear();
        let now = Instant::now();
        let join = self
            .last_edit
            .is_some_and(|last| now.duration_since(last) < Duration::from_millis(600))
            && self
                .undo
                .last()
                .and_then(|group| group.last())
                .is_some_and(|last| {
                    change.start <= last.start + last.after.len()
                        && change.start + change.before.len() >= last.start
                });
        self.bytes += change.bytes();
        if join {
            self.undo.last_mut().unwrap().push(change);
        } else {
            self.undo.push(vec![change]);
        }
        while self.bytes > MAX_HISTORY_BYTES && !self.undo.is_empty() {
            self.bytes -= self.undo.remove(0).iter().map(Change::bytes).sum::<usize>();
        }
        self.last_edit = Some(now);
    }

    /// 测试专用:撤销栈组数(撤销统一断言端)。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(crate) fn undo_depth_for_test(&self) -> usize {
        self.undo.len()
    }

    /// 撤销/重做一步,返回(全文,光标 span)。`redo == true` 表示重做。
    #[allow(dead_code)] // 消费点 = M8.3 撤销动作接线(与源码态输入共栈)。
    pub(crate) fn travel(&mut self, redo: bool) -> Option<(String, Range<usize>)> {
        self.barrier();
        let changes = if redo {
            self.redo.pop()?
        } else {
            self.undo.pop()?
        };
        let mut cursor = 0;
        if redo {
            for change in &changes {
                self.current.replace_range(
                    change.start..change.start + change.before.len(),
                    &change.after,
                );
                cursor = change.start + change.after.len();
            }
            self.undo.push(changes);
        } else {
            for change in changes.iter().rev() {
                self.current.replace_range(
                    change.start..change.start + change.after.len(),
                    &change.before,
                );
                cursor = change.start + change.before.len();
            }
            self.redo.push(changes);
        }
        Some((self.current.clone(), cursor..cursor))
    }
}

#[cfg(test)]
mod edit_history_tests {
    use super::*;

    /// 删除重复后缀不得吞掉公共前缀(中文多字节边界)。
    #[test]
    fn deleting_a_repeated_suffix_does_not_overlap_the_common_prefix() {
        let mut history = EditHistory::default();
        history.reset("中文abc中文abc");
        history.record_rope(&Rope::from("中文abc"));
        assert_eq!(history.travel(false).unwrap().0, "中文abc中文abc");
        assert_eq!(history.travel(true).unwrap().0, "中文abc");
    }

    /// 跨输入实体寿命的撤销;新编辑丢弃 redo 栈。
    #[test]
    fn history_spans_input_lifetimes_and_discards_redo_after_a_new_edit() {
        let mut history = EditHistory::default();
        history.reset("# 中文\n\nbody");
        history.record("# 中文😀\n\nbody");
        history.barrier();
        history.record("# 中文😀\n\nchanged");
        assert_eq!(history.travel(false).unwrap().0, "# 中文😀\n\nbody");
        assert_eq!(history.travel(false).unwrap().0, "# 中文\n\nbody");
        assert_eq!(history.travel(true).unwrap().0, "# 中文😀\n\nbody");
        history.record("# 中文🌿\n\nbody");
        assert!(history.travel(true).is_none());
        assert_eq!(history.travel(false).unwrap().0, "# 中文😀\n\nbody");
    }

    /// rope 记录只保存局部 Unicode 变更,不物化全文。
    #[test]
    fn rope_records_only_the_local_unicode_change() {
        let mut history = EditHistory::default();
        history.reset("prefix 中文 suffix");

        history.record_rope(&Rope::from("prefix 🌿 suffix"));
        assert_eq!(history.current, "prefix 🌿 suffix");
        assert_eq!(history.travel(false).unwrap().0, "prefix 中文 suffix");
        assert_eq!(history.travel(true).unwrap().0, "prefix 🌿 suffix");
    }

    /// 600ms 内的相邻键入合并为一组;barrier 之后的编辑另起一组。
    #[test]
    fn adjacent_edits_join_one_undo_group_until_a_barrier() {
        let mut history = EditHistory::default();
        history.reset("ab");
        history.record_rope(&Rope::from("abc"));
        history.record_rope(&Rope::from("abcd"));
        assert_eq!(history.travel(false).unwrap().0, "ab");
        assert_eq!(history.travel(true).unwrap().0, "abcd");

        history.barrier();
        history.record_rope(&Rope::from("abcde"));
        history.barrier();
        history.record_rope(&Rope::from("abcdef"));
        assert_eq!(history.travel(false).unwrap().0, "abcde");
        assert_eq!(history.travel(false).unwrap().0, "abcd");
        // 第一组在之前的 undo/redo 往返后仍留在撤销栈中。
        assert_eq!(history.travel(false).unwrap().0, "ab");
        assert!(history.travel(false).is_none());
    }

    /// 8MB 上限从最旧组丢弃:被丢组不可再撤销,较新组不受影响。
    #[test]
    fn history_bytes_cap_discards_the_oldest_groups() {
        let mut history = EditHistory::default();
        history.reset("");
        let first = "a".repeat(3 * 1024 * 1024);
        let second = "b".repeat(3 * 1024 * 1024);
        history.record(&first);
        history.barrier();
        history.record(&second);
        history.barrier();
        history.record(&first);
        // 三组各约 3–6MB:超上限后最旧两组被丢弃,只剩最后一组可回退。
        assert_eq!(history.travel(false).unwrap().0, second);
        assert!(history.travel(false).is_none());
    }
}
