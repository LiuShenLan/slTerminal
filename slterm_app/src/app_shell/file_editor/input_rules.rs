//! 输入规则(07 篇 M8.2 照抄):已提交打字/粘贴可引入结构。
//!
//! 纯判定函数=规则清单的单源元数据;IME 组合更新不触发(组合期由
//! live_edit 的 `marked_text_range` 守卫,接线归 M8.3 预览态)。
//!
//! 消费点:M8.3 live_edit 的 `apply_input_rule`/`insert_block_on_enter`。

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
