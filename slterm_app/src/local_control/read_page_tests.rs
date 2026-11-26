//! Runtime 有界读取：页面预算、游标续读、字符边界与代次失效。

use super::reads::{PAGE_BUDGET_BYTES, ReadError, ReadGeneration, read_page, resource_token};

fn generation() -> ReadGeneration {
    ReadGeneration {
        resource: resource_token("terminal:a"),
        generation: 3,
        revision: 7,
    }
}

#[test]
fn reads_are_budget_bounded_and_never_split_utf8() {
    // 每行 4 个字节的中文，预算恰好落在字符中间时不得拆字符。
    let text = "中文输出\n第二行中文\n第三行中文\n";
    let budget = 5;
    let first = read_page(text.as_bytes(), generation(), None, budget).unwrap();
    assert!(first.bytes.len() <= budget);
    assert!(
        std::str::from_utf8(&first.bytes).is_ok(),
        "页面必须是完整 UTF-8"
    );
    assert!(first.next_cursor.is_some());

    let mut collected = first.bytes;
    let mut cursor = first.next_cursor;
    while let Some(next) = cursor {
        let page = read_page(text.as_bytes(), generation(), Some(&next), budget).unwrap();
        assert!(page.bytes.len() <= budget);
        collected.extend_from_slice(&page.bytes);
        cursor = page.next_cursor;
    }
    assert_eq!(collected, text.as_bytes(), "续读必须无损拼接全文");
}

#[test]
fn reads_prefer_line_boundaries_without_starving() {
    let text = "alpha\nbeta\ngamma";
    let page = read_page(text.as_bytes(), generation(), None, 8).unwrap();
    assert_eq!(page.bytes, b"alpha\n", "预算内优先在行尾收束");
    assert!(page.next_cursor.is_some());

    // 超长单行仍必须前进且不拆字符。
    let long = "超长行".repeat(8);
    let page = read_page(long.as_bytes(), generation(), None, 7).unwrap();
    assert!(!page.bytes.is_empty());
    assert!(std::str::from_utf8(&page.bytes).is_ok());
    assert!(page.next_cursor.is_some());
}

#[test]
fn stale_generation_or_revision_invalidates_the_cursor() {
    let text = "0123456789";
    let first = read_page(text.as_bytes(), generation(), None, 4).unwrap();
    let cursor = first.next_cursor.unwrap();

    for changed in [
        ReadGeneration {
            resource: resource_token("terminal:a"),
            generation: 4,
            revision: 7,
        },
        ReadGeneration {
            resource: resource_token("terminal:a"),
            generation: 3,
            revision: 8,
        },
        ReadGeneration {
            resource: resource_token("terminal:b"),
            generation: 3,
            revision: 7,
        },
    ] {
        assert_eq!(
            read_page(text.as_bytes(), changed, Some(&cursor), 4),
            Err(ReadError::StaleCursor)
        );
    }
    for invalid in [
        "",
        "v2",
        "v2:0:3:7",
        "v2:0:3:7:x",
        "v1:0:3:7:0",
        "v2:0:3:7:999",
        "v2:1:3:7:1",
    ] {
        assert_eq!(
            read_page(text.as_bytes(), generation(), Some(invalid), 4),
            Err(ReadError::StaleCursor),
            "非法游标必须显式失效: {invalid}"
        );
    }
}

#[test]
fn truncation_is_explicit_and_never_claims_complete_output() {
    let text = "0123456789";
    let complete = read_page(text.as_bytes(), generation(), None, PAGE_BUDGET_BYTES).unwrap();
    assert_eq!(complete.bytes, text.as_bytes());
    assert!(complete.next_cursor.is_none(), "完整页面不得给出续读游标");

    let partial = read_page(text.as_bytes(), generation(), None, 3).unwrap();
    assert_eq!(partial.bytes, b"012");
    assert!(partial.next_cursor.is_some(), "截断必须显式标记");

    let empty = read_page(b"", generation(), None, PAGE_BUDGET_BYTES).unwrap();
    assert!(empty.bytes.is_empty());
    assert!(empty.next_cursor.is_none());
}

/// 资源身份不同的游标不得复用；字符中段偏移必须失效。
#[test]
fn cursors_are_bound_to_resource_identity_and_character_boundaries() {
    let text = "中文";
    let token = resource_token("terminal:a");
    let mid = format!("v2:{token}:0:0:1");
    assert_eq!(
        read_page(
            text.as_bytes(),
            ReadGeneration {
                resource: token,
                generation: 0,
                revision: 0,
            },
            Some(&mid),
            1,
        ),
        Err(ReadError::StaleCursor),
        "字符中段偏移必须失效"
    );
    let first = read_page(
        text.as_bytes(),
        ReadGeneration {
            resource: token,
            generation: 0,
            revision: 0,
        },
        None,
        4,
    )
    .unwrap();
    let cursor = first.next_cursor.unwrap();
    assert_eq!(
        read_page(
            text.as_bytes(),
            ReadGeneration {
                resource: resource_token("terminal:b"),
                generation: 0,
                revision: 0,
            },
            Some(&cursor),
            4,
        ),
        Err(ReadError::StaleCursor),
        "不同资源不得复用游标"
    );
}
