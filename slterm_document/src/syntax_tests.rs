use std::sync::mpsc::sync_channel;
use std::thread;
use std::time::Duration;

use ropey::Rope;
use tree_sitter::{Parser, Point};

use super::{
    COMMAND_CAPACITY, RESULT_CAPACITY, SyntaxHighlighter, SyntaxWorker, SyntaxWorkerError,
    WorkerObservation, syntax_input_edit,
};
use crate::{DocumentId, LanguageId, LanguageRegistry};

fn document_id() -> DocumentId {
    "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0f2a".parse().unwrap()
}

fn rust_tree(text: &Rope) -> tree_sitter::Tree {
    let mut parser = Parser::new();
    parser
        .set_language(
            &LanguageRegistry::new()
                .descriptor(LanguageId::Rust)
                .grammar()
                .unwrap(),
        )
        .unwrap();
    parser.parse(text.to_string(), None).unwrap()
}

fn large_rust_source() -> Rope {
    let mut source = "fn main() {\n    let 甲 = 1;\n    let 乙 = 2;\n}\n".to_owned();
    for index in 0..4000 {
        source.push_str(&format!("fn filler_{index}() {{ let value = {index}; }}\n"));
    }
    Rope::from_str(&source)
}

#[test]
fn highlighter_returns_nonoverlapping_semantic_spans_in_visible_ranges() {
    let text = Rope::from_str("fn main() {\n    // 注释\n}\n");
    let tree = rust_tree(&text);
    let highlighter = SyntaxHighlighter::new(LanguageId::Rust).unwrap();
    let spans = highlighter
        .highlight_range(&text, &tree, 0..text.len())
        .unwrap();
    assert!(
        spans
            .iter()
            .any(|span| { span.range() == (0..2) && span.capture().starts_with("keyword") })
    );
    assert!(spans.iter().any(|span| {
        text.slice(span.range()) == "// 注释" && span.capture().starts_with("comment")
    }));
    assert!(
        spans
            .windows(2)
            .all(|pair| pair[0].range().end <= pair[1].range().start)
    );

    let visible = highlighter.highlight_range(&text, &tree, 0..2).unwrap();
    assert_eq!(visible.len(), 1);
    assert_eq!(visible[0].range(), 0..2);
    assert!(SyntaxHighlighter::new(LanguageId::PlainText).is_err());
}

#[test]
fn input_edit_tracks_unicode_and_multiline_replacement() {
    let before = Rope::from_str("let 数值 = 1;\nfn main() {}\n");
    let after = Rope::from_str("let 数值 = 你好\n2;\nfn main() {}\n");
    let start = before.to_string().find('1').unwrap();
    let edit = syntax_input_edit(&before, &after, start..start + 1);

    assert_eq!(edit.start_byte, start);
    assert_eq!(edit.old_end_byte, start + 1);
    assert_eq!(edit.new_end_byte, start + "你好\n2".len());
    assert_eq!(edit.start_position, Point::new(0, start));
    assert_eq!(edit.old_end_position, Point::new(0, start + 1));
    assert_eq!(edit.new_end_position, Point::new(1, 1));
}

#[test]
fn worker_rebuilds_an_external_snapshot_without_reusing_old_edits() {
    let before = Rope::from_str("fn main() { let value = 1; }\n");
    let after = Rope::from_str("fn other() { let changed = 2; }\n");
    let mut worker = SyntaxWorker::new(document_id(), LanguageId::Rust, before, 0).unwrap();
    worker.recv_timeout(Duration::from_secs(5)).unwrap();
    worker.rebuild(after.clone(), 1).unwrap();
    let parsed = worker.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(parsed.revision(), 1);
    assert_eq!(
        parsed.tree().root_node().to_sexp(),
        rust_tree(&after).root_node().to_sexp()
    );
    worker.shutdown().unwrap();
}

#[test]
fn worker_applies_every_edit_in_a_transaction() {
    let before = Rope::from_str("fn main() {\n    let 甲 = 1;\n    let 乙 = 2;\n}\n");
    let mut intermediate = before.clone();
    let second_start = intermediate.to_string().find('2').unwrap();
    intermediate.remove(second_start..second_start + 1);
    intermediate.insert(second_start, "some_fn(22)");
    let second = syntax_input_edit(&before, &intermediate, second_start..second_start + 1);
    let first_start = intermediate.to_string().find('1').unwrap();
    let mut after = intermediate.clone();
    after.remove(first_start..first_start + 1);
    after.insert(first_start, "{ 11 }");
    let first = syntax_input_edit(&intermediate, &after, first_start..first_start + 1);

    let mut worker = SyntaxWorker::new(document_id(), LanguageId::Rust, before, 0).unwrap();
    let initial = worker.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(initial.revision(), 0);
    worker.edit(after.clone(), 1, vec![second, first]).unwrap();
    let parsed = worker.recv_timeout(Duration::from_secs(5)).unwrap();

    assert_eq!(parsed.revision(), 1);
    assert_eq!(
        parsed.tree().root_node().to_sexp(),
        rust_tree(&after).root_node().to_sexp()
    );
    assert!(!parsed.tree().root_node().has_error());
    worker.shutdown().unwrap();
}

#[test]
fn superseded_edits_still_advance_the_old_tree() {
    let before = large_rust_source();
    let (entered_tx, entered_rx) = sync_channel(0);
    let (release_tx, release_rx) = sync_channel(0);
    let mut first_progress = true;
    let mut worker = SyntaxWorker::with_parse_observer(
        document_id(),
        LanguageId::Rust,
        before.clone(),
        0,
        move |event| {
            if first_progress && matches!(event, WorkerObservation::Progress) {
                first_progress = false;
                entered_tx.send(()).unwrap();
                release_rx.recv_timeout(Duration::from_secs(5)).unwrap();
            }
        },
    )
    .unwrap();
    entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();

    let first_start = before.to_string().find('1').unwrap();
    let mut middle = before.clone();
    middle.remove(first_start..first_start + 1);
    middle.insert(first_start, "{ 11 }");
    let first = syntax_input_edit(&before, &middle, first_start..first_start + 1);
    worker.edit(middle.clone(), 1, vec![first]).unwrap();

    let second_start = middle.to_string().find('2').unwrap();
    let mut after = middle.clone();
    after.remove(second_start..second_start + 1);
    after.insert(second_start, "other_fn(22)");
    let second = syntax_input_edit(&middle, &after, second_start..second_start + 1);
    worker.edit(after.clone(), 2, vec![second]).unwrap();
    release_tx.send(()).unwrap();

    let publication = worker.results.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(publication.revision, 2);
    let parsed = worker.adopt(publication).unwrap();
    let parsed_main = parsed.tree().root_node().child(0).unwrap();
    let expected = rust_tree(&after);
    let expected_main = expected.root_node().child(0).unwrap();
    assert_eq!(parsed_main.to_sexp(), expected_main.to_sexp());
    assert!(!parsed.tree().root_node().has_error());
    worker.shutdown().unwrap();
}

#[test]
fn hidden_worker_releases_tree_and_show_rebuilds_latest_text() {
    let before = Rope::from_str("fn main() { let value = 1; }\n");
    let (starts_tx, starts_rx) = sync_channel(8);
    let mut worker = SyntaxWorker::with_parse_observer(
        document_id(),
        LanguageId::Rust,
        before.clone(),
        0,
        move |event| {
            if let WorkerObservation::ParseStart {
                revision,
                has_old_tree,
            } = event
            {
                starts_tx.send((revision, has_old_tree)).unwrap();
            }
        },
    )
    .unwrap();
    let initial = worker.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(initial.revision(), 0);
    assert_eq!(
        starts_rx.recv_timeout(Duration::from_secs(5)).unwrap(),
        (0, false)
    );

    let start = before.to_string().find('1').unwrap();
    let mut after = before.clone();
    after.remove(start..start + 1);
    after.insert(start, "call(2)");
    let edit = syntax_input_edit(&before, &after, start..start + 1);
    worker.set_visible(false).unwrap();
    worker.edit(after.clone(), 1, vec![edit]).unwrap();
    worker.set_visible(true).unwrap();

    let parsed = worker.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(parsed.revision(), 1);
    assert_eq!(
        starts_rx.recv_timeout(Duration::from_secs(5)).unwrap(),
        (1, false)
    );
    assert_eq!(
        parsed.tree().root_node().to_sexp(),
        rust_tree(&after).root_node().to_sexp()
    );
    worker.shutdown().unwrap();
}

#[test]
fn publish_in_progress_cannot_cross_a_hide_show_boundary() {
    let (entered_tx, entered_rx) = sync_channel(0);
    let (release_tx, release_rx) = sync_channel(0);
    let mut first_publish = true;
    let mut worker = SyntaxWorker::with_parse_observer(
        document_id(),
        LanguageId::Rust,
        Rope::from_str("fn main() {}\n"),
        0,
        move |event| {
            if first_publish && matches!(event, WorkerObservation::Publish { revision: 0 }) {
                first_publish = false;
                entered_tx.send(()).unwrap();
                release_rx.recv_timeout(Duration::from_secs(5)).unwrap();
            }
        },
    )
    .unwrap();
    entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();

    worker.set_visible(false).unwrap();
    worker.set_visible(true).unwrap();
    release_tx.send(()).unwrap();

    let parsed = worker.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(parsed.revision(), 0);
    assert_eq!(parsed.generation(), 2);
    worker.shutdown().unwrap();
}

#[test]
fn hide_releases_a_tree_whose_publish_was_already_in_progress() {
    let (entered_tx, entered_rx) = sync_channel(0);
    let (release_tx, release_rx) = sync_channel(0);
    let (hidden_tx, hidden_rx) = sync_channel(0);
    let mut first_publish = true;
    let mut worker = SyntaxWorker::with_parse_observer(
        document_id(),
        LanguageId::Rust,
        Rope::from_str("fn main() {}\n"),
        0,
        move |event| match event {
            WorkerObservation::Publish { revision: 0 } if first_publish => {
                first_publish = false;
                entered_tx.send(()).unwrap();
                release_rx.recv_timeout(Duration::from_secs(5)).unwrap();
            }
            WorkerObservation::Hidden => hidden_tx.send(()).unwrap(),
            _ => {}
        },
    )
    .unwrap();
    entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();

    worker.set_visible(false).unwrap();
    release_tx.send(()).unwrap();
    hidden_rx.recv_timeout(Duration::from_secs(5)).unwrap();

    let publication = worker.results.try_recv().unwrap();
    assert!(publication.tree.upgrade().is_none());
    worker.shutdown().unwrap();
}

#[test]
fn command_queue_full_coalesces_rebuild_before_later_revisions() {
    let before = large_rust_source();
    let (entered_tx, entered_rx) = sync_channel(0);
    let (release_tx, release_rx) = sync_channel(0);
    let (latest_started_tx, latest_started_rx) = sync_channel(0);
    let mut first_progress = true;
    let mut worker = SyntaxWorker::with_parse_observer(
        document_id(),
        LanguageId::Rust,
        before.clone(),
        0,
        move |event| match event {
            WorkerObservation::Progress if first_progress => {
                first_progress = false;
                entered_tx.send(()).unwrap();
                release_rx.recv_timeout(Duration::from_secs(5)).unwrap();
            }
            WorkerObservation::ParseStart { revision, .. }
                if revision == COMMAND_CAPACITY as u64 =>
            {
                latest_started_tx.send(()).unwrap();
            }
            _ => {}
        },
    )
    .unwrap();
    entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();

    let mut text = before;
    for revision in 1..=COMMAND_CAPACITY as u64 {
        let previous = text.clone();
        text.insert(previous.len(), &format!("\n// revision {revision}"));
        let edit = syntax_input_edit(&previous, &text, previous.len()..previous.len());
        worker.edit(text.clone(), revision, vec![edit]).unwrap();
    }
    let previous = text.clone();
    let mut rejected = previous.clone();
    rejected.insert(previous.len(), "\n// rejected");
    let edit = syntax_input_edit(&previous, &rejected, previous.len()..previous.len());
    assert!(matches!(
        worker.edit(rejected.clone(), COMMAND_CAPACITY as u64 + 1, vec![edit]),
        Err(SyntaxWorkerError::QueueFull)
    ));
    release_tx.send(()).unwrap();
    latest_started_rx
        .recv_timeout(Duration::from_secs(5))
        .unwrap();
    worker.flush().unwrap();

    let parsed = worker.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(parsed.revision(), COMMAND_CAPACITY as u64 + 1);
    assert_eq!(
        parsed.tree().root_node().to_sexp(),
        rust_tree(&rejected).root_node().to_sexp()
    );

    let next_revision = COMMAND_CAPACITY as u64 + 2;
    let mut after = rejected.clone();
    after.insert(rejected.len(), "\n// next");
    let edit = syntax_input_edit(&rejected, &after, rejected.len()..rejected.len());
    worker
        .edit(after.clone(), next_revision, vec![edit])
        .unwrap();
    let parsed = worker.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(parsed.revision(), next_revision);
    assert_eq!(
        parsed.tree().root_node().to_sexp(),
        rust_tree(&after).root_node().to_sexp()
    );
    worker.shutdown().unwrap();
}

#[test]
fn full_result_queue_backpressures_without_losing_latest_tree() {
    let (publish_tx, publish_rx) = sync_channel(RESULT_CAPACITY + 2);
    let mut worker = SyntaxWorker::with_parse_observer(
        document_id(),
        LanguageId::Rust,
        Rope::from_str("fn main() {}\n"),
        0,
        move |event| {
            if let WorkerObservation::Publish { revision } = event {
                publish_tx.send(revision).unwrap();
            }
        },
    )
    .unwrap();
    assert_eq!(publish_rx.recv_timeout(Duration::from_secs(5)).unwrap(), 0);
    let mut text = Rope::from_str("fn main() {}\n");
    for revision in 1..=RESULT_CAPACITY as u64 {
        let previous = text.clone();
        text.insert(previous.len(), &format!("// revision {revision}\n"));
        let edit = syntax_input_edit(&previous, &text, previous.len()..previous.len());
        worker.edit(text.clone(), revision, vec![edit]).unwrap();
        assert_eq!(
            publish_rx.recv_timeout(Duration::from_secs(5)).unwrap(),
            revision
        );
    }

    let parsed = worker.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(parsed.revision(), RESULT_CAPACITY as u64);
    assert_eq!(
        parsed.tree().root_node().to_sexp(),
        rust_tree(&text).root_node().to_sexp()
    );
    worker.shutdown().unwrap();
}

#[test]
fn close_cancels_an_active_parse_and_rejoins_the_thread() {
    let (entered_tx, entered_rx) = sync_channel(0);
    let (release_tx, release_rx) = sync_channel(0);
    let mut first_progress = true;
    let worker = SyntaxWorker::with_parse_observer(
        document_id(),
        LanguageId::Rust,
        large_rust_source(),
        0,
        move |event| {
            if first_progress && matches!(event, WorkerObservation::Progress) {
                first_progress = false;
                entered_tx.send(()).unwrap();
                release_rx.recv_timeout(Duration::from_secs(5)).unwrap();
            }
        },
    )
    .unwrap();
    entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();

    let closing = thread::spawn(move || worker.shutdown());
    release_tx.send(()).unwrap();
    closing.join().unwrap().unwrap();
}

#[test]
fn empty_result_queue_is_not_a_disconnect() {
    let mut worker = SyntaxWorker::new(
        document_id(),
        LanguageId::Rust,
        Rope::from_str("fn main() {}\n"),
        0,
    )
    .unwrap();
    worker.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(matches!(worker.try_recv(), Ok(None)));
    worker.shutdown().unwrap();
}

#[test]
fn worker_rejects_stale_revisions_and_plain_text() {
    let text = Rope::from_str("fn main() {}\n");
    assert!(matches!(
        SyntaxWorker::new(document_id(), LanguageId::PlainText, text.clone(), 0),
        Err(SyntaxWorkerError::MissingGrammar)
    ));
    let mut worker = SyntaxWorker::new(document_id(), LanguageId::Rust, text.clone(), 1).unwrap();
    assert!(matches!(
        worker.edit(text, 1, Vec::new()),
        Err(SyntaxWorkerError::StaleRevision)
    ));
    worker.shutdown().unwrap();
}
