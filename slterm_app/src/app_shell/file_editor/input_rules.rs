//! 输入规则(07 篇 M8.2 判定 + M8.3 接线):已提交打字/粘贴可引入结构。
//!
//! 纯判定函数=规则清单的单源元数据;IME 组合更新不触发(组合期由
//! live_edit 的 `marked_text_range` 守卫)。
//!
//! 消费点:预览态 live_edit 的输入订阅(`apply_input_rule`/`insert_block_on_enter`)。

use super::inline_edit::{Marks, Projection};
use super::*;
use gpui::EntityInputHandler as _;
use markdown::mdast::Node;

/// 块前缀判定:列表/引用/标题/有序列表必须完整标记(含尾随空格)。
pub(super) fn block_prefix(text: &str) -> bool {
    if matches!(text, "- " | "* " | "+ " | "> ") {
        return true;
    }
    if let Some(hashes) = text.strip_suffix(' ') {
        if (1..=6).contains(&hashes.len()) && hashes.bytes().all(|ch| ch == b'#') {
            return true;
        }
        let digits = hashes.trim_end_matches(['.', ')']);
        return digits.len() < hashes.len()
            && !digits.is_empty()
            && digits.len() <= 9
            && digits.bytes().all(|ch| ch.is_ascii_digit());
    }
    false
}

/// 回车转换:围栏/数学块/分割线前缀 → 块源码与光标位。
pub(super) fn enter_block(text: &str) -> Option<(String, usize)> {
    let trimmed = text.trim();
    if trimmed == "$$" {
        return Some(("$$\n\n$$".to_owned(), 3));
    }
    let marker = trimmed.chars().next()?;
    if matches!(marker, '`' | '~') {
        let length = trimmed.chars().take_while(|ch| *ch == marker).count();
        if length >= 3 && !trimmed.contains('\n') {
            let fence = marker.to_string().repeat(length);
            return Some((format!("{trimmed}\n\n{fence}"), trimmed.len() + 1));
        }
    }
    if matches!(trimmed, "---" | "***" | "___") {
        return Some((format!("{trimmed}\n\n"), trimmed.len() + 2));
    }
    None
}

impl TextFileView {
    /// 已提交的 Rich 输入引入完整前缀/片段结构时,整块交给结构事务。
    pub(super) fn apply_input_rule(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let Some(edit) = &self.live_edit else {
            return;
        };
        if !matches!(edit.kind, super::block_structure::PartKind::Rich)
            || edit.projection.revealed().is_some()
            || edit.input.update(cx, |input, cx| {
                input.marked_text_range(window, cx).is_some()
            })
        {
            return;
        }
        let text = edit.input.read(cx).value().to_string();
        let no_marks = edit
            .projection
            .marks()
            .all(|(_, marks)| marks == Marks::default());
        if !no_marks {
            return;
        }
        let mut convert = edit.part.is_none()
            && self.outline.heading_level(edit.block).is_none()
            && block_prefix(&text);
        if let Some(part) = edit.part {
            if let Some(structure) = self.outline.structures[edit.block].as_ref() {
                convert |= structure
                    .root
                    .list_item(part)
                    .is_some_and(|item| item.check.is_none())
                    && matches!(text.as_str(), "[ ] " | "[x] " | "[X] ");
            }
        } else if text.contains('\n')
            && let Ok(Node::Root(root)) = markdown::to_mdast(&text, &markdown::ParseOptions::gfm())
        {
            convert |= root.children.len() > 1
                || root.children.first().is_some_and(|node| {
                    matches!(
                        node,
                        Node::Table(_)
                            | Node::List(_)
                            | Node::Code(_)
                            | Node::Blockquote(_)
                            | Node::Heading(_)
                    )
                });
        }
        let parsed = Projection::new(&text);
        convert |= parsed.rich
            && parsed.text != text
            && parsed.marks().any(|(_, marks)| marks != Marks::default());
        if convert {
            let cursor = edit.range.start + text.len();
            self.commit_structure_edit(edit.range.clone(), &text, Some(cursor), window, cx);
        }
    }

    /// 回车把围栏/数学块/分割线前缀转换为完整块源码。
    pub(super) fn insert_block_on_enter(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> bool {
        let Some(edit) = &self.live_edit else {
            return false;
        };
        if edit.part.is_some() {
            return false;
        }
        let Some((source, offset)) = enter_block(&edit.input.read(cx).value()) else {
            return false;
        };
        let cursor = edit.range.start + offset;
        self.commit_structure_edit(edit.range.clone(), &source, Some(cursor), window, cx);
        true
    }
}

#[cfg(test)]
mod input_rules_tests {
    use super::{block_prefix, enter_block};

    /// 完整标记才命中;围栏保留语言标记。
    #[test]
    fn markdown_prefixes_require_complete_markers_and_keep_fence_language() {
        for prefix in ["# ", "###### ", "- ", "* ", "> ", "12. ", "1) "] {
            assert!(block_prefix(prefix));
        }
        for plain in ["#word", "####### ", "-word", "12 ", "ordinary text"] {
            assert!(!block_prefix(plain));
        }
        assert_eq!(
            enter_block("```rust"),
            Some(("```rust\n\n```".to_owned(), 8))
        );
        assert!(enter_block("ordinary text").is_none());
    }

    /// 数学块/分割线/波浪线围栏的源码与光标位。
    #[test]
    fn math_rules_and_horizontal_rules_convert_with_caret_inside_the_block() {
        assert_eq!(enter_block("$$"), Some(("$$\n\n$$".to_owned(), 3)));
        assert_eq!(enter_block("---"), Some(("---\n\n".to_owned(), 5)));
        assert_eq!(enter_block("~~~"), Some(("~~~\n\n~~~".to_owned(), 4)));
        assert!(enter_block("``").is_none(), "不足三个反引号不是围栏");
        assert!(
            enter_block("```rust\nfn main() {}").is_none(),
            "已含换行不转换"
        );
    }
}
