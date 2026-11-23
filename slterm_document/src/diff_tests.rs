use std::{
    str::FromStr as _,
    sync::Arc,
    time::{Duration, Instant},
};

use crate::{
    CommitId, DiffBaseline, DiffError, DiffInput, DiffPairKind, DiffSideContent,
    DiffWhitespaceMode, TextStateId, compute_diff, compute_diff_with_deadline,
};

fn text_state_id() -> TextStateId {
    TextStateId::from_str("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b").unwrap()
}

fn commit_id() -> CommitId {
    CommitId::from_str("a".repeat(40).as_str()).unwrap()
}

fn input(head: &[u8], worktree: &[u8], whitespace: DiffWhitespaceMode) -> DiffInput {
    DiffInput::git(
        Arc::from(head),
        Arc::from(worktree),
        whitespace,
        text_state_id(),
        commit_id(),
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
    let pairs = snapshot.pairs().unwrap();

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
    assert_eq!(trailing.pairs().unwrap()[0].kind, DiffPairKind::Replace);
    assert_eq!(trailing.pairs().unwrap()[1].kind, DiffPairKind::Equal);

    let all = compute_diff(&input(b"a b\tc\n", b"ab c\n", DiffWhitespaceMode::All)).unwrap();
    assert!(
        all.pairs()
            .unwrap()
            .iter()
            .all(|pair| pair.kind == DiffPairKind::Equal)
    );

    let exact = compute_diff(&input(b"space \n", b"space\n", DiffWhitespaceMode::Exact)).unwrap();
    assert_eq!(exact.pairs().unwrap()[0].kind, DiffPairKind::Replace);
    let deleted = compute_diff(&input(
        b"keep\nremove\n",
        b"keep\n",
        DiffWhitespaceMode::Exact,
    ))
    .unwrap();
    assert_eq!(deleted.pairs().unwrap()[1].kind, DiffPairKind::Delete);
    assert!(deleted.pairs().unwrap()[1].worktree.is_none());
}

#[test]
fn diff_exact_preserves_bare_carriage_return() {
    let snapshot = compute_diff(&input(b"a\r", b"a", DiffWhitespaceMode::Exact)).unwrap();

    assert_eq!(snapshot.pairs().unwrap()[0].kind, DiffPairKind::Replace);
}

#[test]
fn diff_deadline_discards_approximate_results() {
    let request = input(b"old\n", b"new\n", DiffWhitespaceMode::Exact);
    let deadline = Instant::now() - Duration::from_millis(1);

    assert!(matches!(
        compute_diff_with_deadline(&request, deadline),
        Err(DiffError::DeadlineExceeded)
    ));
    assert!(matches!(
        compute_diff_with_deadline(
            &input(b"head\0binary", b"worktree\n", DiffWhitespaceMode::Exact),
            deadline,
        ),
        Err(DiffError::DeadlineExceeded)
    ));
}

#[test]
fn diff_hunk_hook_rechecks_deadline() {
    let head = ["same\n"];
    let worktree = ["same\n"];
    let mut builder =
        crate::diff::PairBuilder::new(&head, &worktree, Instant::now() - Duration::from_millis(1));

    assert!(matches!(
        similar::algorithms::DiffHook::equal(&mut builder, 0, 0, 1),
        Err(DiffError::DeadlineExceeded)
    ));
}

#[test]
fn diff_side_analysis_rechecks_deadline() {
    assert!(matches!(
        crate::diff::analyze_side(b"text\n", Instant::now() - Duration::from_millis(1),),
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
    assert_eq!(binary.head(), &DiffSideContent::Binary);
    assert!(matches!(binary.worktree(), DiffSideContent::Text { .. }));
    assert!(binary.pairs().is_none());

    let huge = vec![b'x'; 50_000_001];
    let large = compute_diff(&DiffInput::git(
        Arc::from("head\n".as_bytes()),
        Arc::from(huge),
        DiffWhitespaceMode::Exact,
        text_state_id(),
        commit_id(),
        8,
    ))
    .unwrap();
    assert!(matches!(large.head(), DiffSideContent::Text { .. }));
    assert_eq!(large.worktree(), &DiffSideContent::TooLarge);
    assert!(large.pairs().is_none());

    let paged = compute_diff_with_deadline(
        &input(b"", &vec![b'x'; 10_000_001], DiffWhitespaceMode::Exact),
        Instant::now() + Duration::from_secs(60),
    )
    .unwrap();
    assert!(paged.worktree().is_paged());
    assert!(paged.pairs().is_some());
    let invalid = compute_diff(&input(b"\xff", b"text", DiffWhitespaceMode::Exact)).unwrap();
    assert_eq!(invalid.head(), &DiffSideContent::InvalidUtf8);
    assert!(invalid.pairs().is_none());
}

#[test]
fn diff_excessive_line_count_degrades_only_that_side() {
    let snapshot = compute_diff(&input(
        &vec![b'\n'; 200_001],
        b"worktree\n",
        DiffWhitespaceMode::Exact,
    ))
    .unwrap();

    assert_eq!(snapshot.head(), &DiffSideContent::TooManyLines);
    assert!(matches!(snapshot.worktree(), DiffSideContent::Text { .. }));
    assert!(snapshot.pairs().is_none());
}

#[test]
fn diff_conflict_snapshot_is_immutable() {
    let input = DiffInput::conflict(
        Arc::from("disk conflict\n".as_bytes()),
        Arc::from("buffer conflict\n".as_bytes()),
        DiffWhitespaceMode::Exact,
        text_state_id(),
        7,
    );
    let snapshot = compute_diff(&input).unwrap();

    assert_eq!(snapshot.generation(), 7);
    assert_eq!(snapshot.text_state_id(), input.text_state_id());
    assert_eq!(snapshot.baseline(), &DiffBaseline::Conflict);
    assert!(matches!(snapshot.head(), DiffSideContent::Text { .. }));
    assert!(matches!(snapshot.worktree(), DiffSideContent::Text { .. }));
    assert!(
        snapshot
            .pairs()
            .unwrap()
            .iter()
            .any(|pair| pair.kind == DiffPairKind::Replace)
    );
    let updated = compute_diff(&DiffInput::conflict(
        Arc::from("disk new\n".as_bytes()),
        Arc::from("buffer new\n".as_bytes()),
        DiffWhitespaceMode::Exact,
        input.text_state_id(),
        8,
    ))
    .unwrap();
    match snapshot.head() {
        DiffSideContent::Text { text } => assert_eq!(text.as_ref(), "disk conflict\n"),
        other => panic!("冲突快照必须保留文本：{other:?}"),
    }
    assert_eq!(updated.generation(), 8);
    assert!(
        updated
            .pairs()
            .unwrap()
            .iter()
            .any(|pair| pair.kind == DiffPairKind::Replace)
    );
    assert_eq!(snapshot.generation(), 7);
}

fn reference_line_key(line: &str, whitespace: DiffWhitespaceMode) -> String {
    let line = match line.strip_suffix('\n') {
        Some(line) => line.strip_suffix('\r').unwrap_or(line),
        None => line,
    };
    match whitespace {
        DiffWhitespaceMode::Exact => line.to_owned(),
        DiffWhitespaceMode::Trailing => line.trim_end_matches([' ', '\t']).to_owned(),
        DiffWhitespaceMode::All => line
            .chars()
            .filter(|character| !matches!(character, ' ' | '\t'))
            .collect(),
    }
}

fn reference_pairs(
    head: &str,
    worktree: &str,
    whitespace: DiffWhitespaceMode,
) -> Vec<(DiffPairKind, Option<usize>, Option<usize>)> {
    let head = head.split_inclusive('\n').collect::<Vec<_>>();
    let worktree = worktree.split_inclusive('\n').collect::<Vec<_>>();
    let head_keys = head
        .iter()
        .map(|line| reference_line_key(line, whitespace))
        .collect::<Vec<_>>();
    let worktree_keys = worktree
        .iter()
        .map(|line| reference_line_key(line, whitespace))
        .collect::<Vec<_>>();
    let mut hook = similar::algorithms::Replace::new(similar::algorithms::Capture::new());
    similar::algorithms::diff_slices(
        similar::Algorithm::Myers,
        &mut hook,
        &head_keys,
        &worktree_keys,
    )
    .unwrap();
    let ops = hook.into_inner().into_ops();
    let mut pairs = Vec::new();
    for op in ops {
        let old = op.old_range();
        let new = op.new_range();
        match op.tag() {
            similar::DiffTag::Equal => pairs.extend(
                old.zip(new)
                    .map(|(old, new)| (DiffPairKind::Equal, Some(old), Some(new))),
            ),
            similar::DiffTag::Delete => {
                pairs.extend(old.map(|old| (DiffPairKind::Delete, Some(old), None)));
            }
            similar::DiffTag::Insert => {
                pairs.extend(new.map(|new| (DiffPairKind::Insert, None, Some(new))));
            }
            similar::DiffTag::Replace => {
                let shared = old.len().min(new.len());
                pairs.extend((0..shared).map(|offset| {
                    (
                        DiffPairKind::Replace,
                        Some(old.start + offset),
                        Some(new.start + offset),
                    )
                }));
                pairs.extend(
                    (old.start + shared..old.end)
                        .map(|old| (DiffPairKind::Delete, Some(old), None)),
                );
                pairs.extend(
                    (new.start + shared..new.end)
                        .map(|new| (DiffPairKind::Insert, None, Some(new))),
                );
            }
        }
    }
    pairs
}

fn render_property_document(lines: &[(String, bool)], trailing_newline: bool) -> String {
    let mut text = String::new();
    for (index, (line, crlf)) in lines.iter().enumerate() {
        text.push_str(line);
        if index + 1 < lines.len() || trailing_newline {
            text.push_str(if *crlf { "\r\n" } else { "\n" });
        }
    }
    text
}

proptest::proptest! {
    #[test]
    fn diff_pairs_match_myers_hunks_across_whitespace_and_endings(
        head in proptest::collection::vec(("[a-z \\t\\r]{0,12}", proptest::bool::ANY), 0..8),
        worktree in proptest::collection::vec(("[a-z \\t\\r]{0,12}", proptest::bool::ANY), 0..8),
        head_trailing_newline in proptest::bool::ANY,
        worktree_trailing_newline in proptest::bool::ANY,
        whitespace in proptest::prop_oneof![
            proptest::strategy::Just(DiffWhitespaceMode::Exact),
            proptest::strategy::Just(DiffWhitespaceMode::Trailing),
            proptest::strategy::Just(DiffWhitespaceMode::All),
        ],
    ) {
        let head_text = render_property_document(&head, head_trailing_newline);
        let worktree_text = render_property_document(&worktree, worktree_trailing_newline);
        let snapshot = compute_diff(&input(
            head_text.as_bytes(),
            worktree_text.as_bytes(),
            whitespace,
        ))
        .unwrap();
        let pairs = snapshot.pairs().unwrap();
        let head_lines = head_text.split_inclusive('\n').collect::<Vec<_>>();
        let worktree_lines = worktree_text.split_inclusive('\n').collect::<Vec<_>>();
        let actual = pairs.iter().map(|pair| {
            if let Some(line) = &pair.head {
                proptest::prop_assert_eq!(line.text.as_str(), head_lines[line.index]);
            }
            if let Some(line) = &pair.worktree {
                proptest::prop_assert_eq!(line.text.as_str(), worktree_lines[line.index]);
            }
            Ok((
                pair.kind,
                pair.head.as_ref().map(|line| line.index),
                pair.worktree.as_ref().map(|line| line.index),
            ))
        }).collect::<Result<Vec<_>, _>>()?;
        proptest::prop_assert_eq!(actual, reference_pairs(&head_text, &worktree_text, whitespace));
    }
}
