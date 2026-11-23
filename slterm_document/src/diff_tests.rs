use std::{
    str::FromStr as _,
    sync::Arc,
    time::{Duration, Instant},
};

use crate::{
    CommitId, DiffError, DiffInput, DiffPairKind, DiffSideContent, DiffWhitespaceMode, TextStateId,
    compute_diff, compute_diff_with_deadline,
};

fn input(head: &[u8], worktree: &[u8], whitespace: DiffWhitespaceMode) -> DiffInput {
    DiffInput::new(
        Arc::from(head),
        Arc::from(worktree),
        whitespace,
        TextStateId::from_str("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b").unwrap(),
        CommitId::from_str("a".repeat(40).as_str()).unwrap(),
        7,
    )
}

#[test]
fn diff_line_pairs_preserve_raw_text_and_exact_whitespace_rules() {
    let snapshot = compute_diff(&input(
        b"same  \r\nreplace me\nlast",
        b"same\nreplacement\nlast\nextra\n",
        DiffWhitespaceMode::Trailing,
    ))
    .unwrap();
    let pairs = snapshot.pairs.as_ref().unwrap();

    assert_eq!(pairs[0].kind, DiffPairKind::Equal);
    assert_eq!(pairs[0].head.as_ref().unwrap().text, "same  \r\n");
    assert_eq!(pairs[0].worktree.as_ref().unwrap().text, "same\n");
    assert_eq!(pairs[1].kind, DiffPairKind::Replace);
    assert_eq!(pairs[1].head.as_ref().unwrap().text, "replace me\n");
    assert_eq!(pairs[1].worktree.as_ref().unwrap().text, "replacement\n");
    assert_eq!(pairs[2].kind, DiffPairKind::Equal);
    assert_eq!(pairs[3].kind, DiffPairKind::Insert);
    assert!(pairs[3].head.is_none());

    let trailing = compute_diff(&input(
        b"internal space\ntrail  \n",
        b"internal  space\ntrail\t\n",
        DiffWhitespaceMode::Trailing,
    ))
    .unwrap();
    assert_eq!(
        trailing.pairs.as_ref().unwrap()[0].kind,
        DiffPairKind::Replace
    );
    assert_eq!(
        trailing.pairs.as_ref().unwrap()[1].kind,
        DiffPairKind::Equal
    );

    let all = compute_diff(&input(b"a b\tc\n", b"ab c\n", DiffWhitespaceMode::All)).unwrap();
    assert!(
        all.pairs
            .as_ref()
            .unwrap()
            .iter()
            .all(|pair| pair.kind == DiffPairKind::Equal)
    );

    let exact = compute_diff(&input(b"space \n", b"space\n", DiffWhitespaceMode::Exact)).unwrap();
    assert_eq!(exact.pairs.as_ref().unwrap()[0].kind, DiffPairKind::Replace);
    let deleted = compute_diff(&input(
        b"keep\nremove\n",
        b"keep\n",
        DiffWhitespaceMode::Exact,
    ))
    .unwrap();
    assert_eq!(
        deleted.pairs.as_ref().unwrap()[1].kind,
        DiffPairKind::Delete
    );
    assert!(deleted.pairs.as_ref().unwrap()[1].worktree.is_none());
}

#[test]
fn diff_deadline_discards_approximate_results() {
    let input = input(b"old\n", b"new\n", DiffWhitespaceMode::Exact);
    let deadline = Instant::now() - Duration::from_millis(1);

    assert!(matches!(
        compute_diff_with_deadline(&input, deadline),
        Err(DiffError::DeadlineExceeded)
    ));
}

#[test]
fn diff_large_and_binary_side_degradation() {
    let binary = compute_diff(&input(
        b"head\0binary",
        b"worktree\n",
        DiffWhitespaceMode::Exact,
    ))
    .unwrap();
    assert_eq!(binary.head, DiffSideContent::Binary);
    assert!(matches!(binary.worktree, DiffSideContent::Text { .. }));
    assert!(binary.pairs.is_none());

    let huge = vec![b'x'; 50_000_001];
    let large = compute_diff(&DiffInput::new(
        Arc::from("head\n".as_bytes()),
        Arc::from(huge),
        DiffWhitespaceMode::Exact,
        input(b"", b"", DiffWhitespaceMode::Exact).text_state_id(),
        input(b"", b"", DiffWhitespaceMode::Exact)
            .head_identity()
            .clone(),
        8,
    ))
    .unwrap();
    assert!(matches!(large.head, DiffSideContent::Text { .. }));
    assert_eq!(large.worktree, DiffSideContent::TooLarge);
    assert!(large.pairs.is_none());

    let paged = compute_diff(&input(
        b"",
        &vec![b'x'; 10_000_001],
        DiffWhitespaceMode::Exact,
    ))
    .unwrap();
    assert!(matches!(
        paged.worktree,
        DiffSideContent::Text { paged: true, .. }
    ));
    assert!(paged.pairs.is_some());
    let invalid = compute_diff(&input(b"\xff", b"text", DiffWhitespaceMode::Exact)).unwrap();
    assert_eq!(invalid.head, DiffSideContent::InvalidUtf8);
    assert!(invalid.pairs.is_none());
}

#[test]
fn diff_conflict_snapshot_is_immutable() {
    let input = input(
        b"disk conflict\n",
        b"buffer conflict\n",
        DiffWhitespaceMode::Exact,
    );
    let snapshot = compute_diff(&input).unwrap();

    assert_eq!(snapshot.generation, 7);
    assert_eq!(snapshot.text_state_id, input.text_state_id());
    assert_eq!(&snapshot.head_identity, input.head_identity());
    assert!(matches!(snapshot.head, DiffSideContent::Text { .. }));
    assert!(matches!(snapshot.worktree, DiffSideContent::Text { .. }));
    assert!(
        snapshot
            .pairs
            .as_ref()
            .unwrap()
            .iter()
            .any(|pair| pair.kind == DiffPairKind::Replace)
    );
    let updated = compute_diff(&DiffInput::new(
        Arc::from("disk new\n".as_bytes()),
        Arc::from("buffer new\n".as_bytes()),
        DiffWhitespaceMode::Exact,
        input.text_state_id(),
        input.head_identity().clone(),
        8,
    ))
    .unwrap();
    match &snapshot.head {
        DiffSideContent::Text { text, .. } => assert_eq!(text.as_ref(), "disk conflict\n"),
        other => panic!("冲突快照必须保留文本：{other:?}"),
    }
    assert_eq!(updated.generation, 8);
    assert!(
        updated
            .pairs
            .as_ref()
            .unwrap()
            .iter()
            .any(|pair| pair.kind == DiffPairKind::Replace)
    );
    assert_eq!(snapshot.generation, 7);
}

proptest::proptest! {
    #[test]
    fn diff_pairs_cover_all_lines_without_reordering(
        head in proptest::collection::vec("[a-z \\t]{0,12}", 0..8),
        worktree in proptest::collection::vec("[a-z \\t]{0,12}", 0..8),
    ) {
        let head_text = head.join("\n");
        let worktree_text = worktree.join("\n");
        let snapshot = compute_diff(&input(
            head_text.as_bytes(),
            worktree_text.as_bytes(),
            DiffWhitespaceMode::Exact,
        ))
        .unwrap();
        let pairs = snapshot.pairs.as_ref().unwrap();
        let head_indices = pairs.iter().filter_map(|pair| pair.head.as_ref().map(|line| line.index)).collect::<Vec<_>>();
        let worktree_indices = pairs.iter().filter_map(|pair| pair.worktree.as_ref().map(|line| line.index)).collect::<Vec<_>>();
        proptest::prop_assert_eq!(head_indices, (0..head_text.split_inclusive('\n').count()).collect::<Vec<_>>());
        proptest::prop_assert_eq!(worktree_indices, (0..worktree_text.split_inclusive('\n').count()).collect::<Vec<_>>());
    }
}
