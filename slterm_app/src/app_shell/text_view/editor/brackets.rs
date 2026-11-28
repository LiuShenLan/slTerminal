//! 括号匹配：光标相邻括号的纯文本扫描，不依赖语言服务。
//!
//! 只处理括号栈；不解析字符串或注释语义，类型不匹配即不显示高亮。
//! 扫描按字符数设上限，避免长文档未匹配括号拖慢前台绘制。

use ropey::Rope;

const SCAN_CHAR_LIMIT: usize = 1_000_000;

/// 开括号 → 期望的闭括号。
fn expected_closing(character: char) -> Option<char> {
    match character {
        '(' => Some(')'),
        '[' => Some(']'),
        '{' => Some('}'),
        _ => None,
    }
}

/// 闭括号 → 期望的开括号。
fn expected_opening(character: char) -> Option<char> {
    match character {
        ')' => Some('('),
        ']' => Some('['),
        '}' => Some('{'),
        _ => None,
    }
}

fn char_at(text: &Rope, byte: usize) -> Option<(char, usize)> {
    let character = text.get_char(byte).ok()?;
    Some((character, byte + character.len_utf8()))
}

/// 单步向前找一个 UTF-8 字符起点；只在边界附近至多检查四个字节。
fn char_before(text: &Rope, byte: usize) -> Option<usize> {
    if byte == 0 || byte > text.len() || !text.is_char_boundary(byte) {
        return None;
    }
    for start in (byte.saturating_sub(4)..byte).rev() {
        if !text.is_char_boundary(start) {
            continue;
        }
        let Ok(character) = text.get_char(start) else {
            continue;
        };
        if start + character.len_utf8() == byte {
            return Some(start);
        }
    }
    None
}

/// 光标前后各取一个字符作为候选；返回匹配括号对的字节位置。
pub(super) fn matching_bracket(text: &Rope, cursor: usize) -> Option<(usize, usize)> {
    if cursor > text.len() || !text.is_char_boundary(cursor) {
        return None;
    }
    for start in [char_before(text, cursor), Some(cursor)]
        .into_iter()
        .flatten()
    {
        let Some((character, end)) = char_at(text, start) else {
            continue;
        };
        if expected_closing(character).is_some()
            && let Some(found) = scan_forward(text, end)
        {
            return Some((start, found));
        }
        if expected_opening(character).is_some()
            && let Some(found) = scan_backward(text, start)
        {
            return Some((found, start));
        }
    }
    None
}

fn scan_forward(text: &Rope, from: usize) -> Option<usize> {
    let mut stack: Vec<char> = Vec::new();
    let mut byte = from;
    let mut scanned = 0;
    while byte < text.len() && scanned < SCAN_CHAR_LIMIT {
        let (character, next) = char_at(text, byte)?;
        if let Some(closing) = expected_closing(character) {
            stack.push(closing);
        } else if expected_opening(character).is_some() {
            match stack.pop() {
                None => return Some(byte),
                Some(expected) if expected == character => {}
                Some(_) => return None,
            }
        }
        byte = next;
        scanned += 1;
    }
    None
}

fn scan_backward(text: &Rope, from: usize) -> Option<usize> {
    let mut stack: Vec<char> = Vec::new();
    let mut byte = from;
    let mut scanned = 0;
    while byte > 0 && scanned < SCAN_CHAR_LIMIT {
        let start = char_before(text, byte)?;
        let (character, _) = char_at(text, start)?;
        if expected_opening(character).is_some() {
            stack.push(character);
        } else if let Some(expected) = expected_closing(character) {
            match stack.pop() {
                None => return Some(start),
                Some(closing) if closing == expected => {}
                Some(_) => return None,
            }
        }
        byte = start;
        scanned += 1;
    }
    None
}
