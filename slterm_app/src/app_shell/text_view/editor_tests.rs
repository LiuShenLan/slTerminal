use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

use gpui::{
    AppContext as _, Bounds, Context, Entity, EntityInputHandler as _, Render, TestAppContext,
    Window, px,
};
use ropey::Rope;
use slterm_document::{
    DocumentId, DocumentMetadata, DocumentRegistry, EditOrigin, EditTransaction, MergePolicy,
    TextEdit, TextStateId,
};
use slterm_path::{CanonicalPath, ResolvedPath};
use uuid::Uuid;

use super::{
    DocumentEditRequest, DocumentEditResponse, DocumentTextSnapshot, DocumentTextView,
    LayoutMetrics, TextSelection, transform_selection,
};

struct Host {
    registry: Rc<RefCell<DocumentRegistry>>,
    id: DocumentId,
    view: Entity<DocumentTextView>,
}

impl Host {
    fn new(text: &str, read_only: bool, cx: &mut Context<'_, Self>) -> Self {
        let id: DocumentId = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0f2a".parse().unwrap();
        let state = TextStateId::from_uuid(Uuid::now_v7()).unwrap();
        let mut registry = DocumentRegistry::new();
        registry
            .open(
                id,
                ResolvedPath::new(CanonicalPath::new("D:/work/editor.txt").unwrap(), None),
                Rope::from_str(text),
                state,
                std::sync::Arc::from(text.as_bytes()),
                DocumentMetadata {
                    read_only,
                    ..DocumentMetadata::default()
                },
            )
            .unwrap();
        let registry = Rc::new(RefCell::new(registry));
        let sink_registry = registry.clone();
        let view = cx.new(|cx| {
            let session = registry.borrow();
            DocumentTextView::new(
                id,
                session.session(id).unwrap(),
                Box::new(move |request, _| {
                    let mut registry = sink_registry.borrow_mut();
                    let outcome = match request {
                        DocumentEditRequest::Edit(transaction, state) => {
                            registry.apply(transaction, state, Instant::now())?
                        }
                        DocumentEditRequest::Travel { redo } => {
                            if redo {
                                registry.redo(id)
                            } else {
                                registry.undo(id)
                            }?
                        }
                    };
                    Ok(DocumentEditResponse {
                        snapshot: DocumentTextSnapshot::from_session(registry.session(id).unwrap()),
                        outcome,
                    })
                }),
                cx,
            )
        });
        Self { registry, id, view }
    }

    fn text(&self) -> String {
        self.registry
            .borrow()
            .session(self.id)
            .unwrap()
            .text()
            .unwrap()
            .to_string()
    }
}

impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut Context<'_, Self>) -> impl gpui::IntoElement {
        self.view.clone()
    }
}

#[test]
fn external_replacement_collapses_a_fully_covered_selection() {
    let selection = TextSelection { anchor: 2, head: 4 };
    let edits = vec![TextEdit::new(1..5, "X")];
    let mapped = transform_selection(selection, &edits);
    assert_eq!(mapped.anchor, 1);
    assert_eq!(mapped.head, 1);
}

#[test]
fn adjacent_edits_rebase_boundaries_through_same_offset_insertions() {
    let selection = TextSelection { anchor: 1, head: 2 };
    let edits = vec![TextEdit::new(0..1, "X"), TextEdit::new(1..1, "Q")];
    let mapped = transform_selection(selection, &edits);
    assert_eq!(mapped.range(), 2..3);
}

#[gpui::test]
fn ime_commits_utf16_edits_through_the_shared_document(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        crate::app_shell::theme::apply_dark_theme(cx);
    });
    let window = cx.add_window(|_, cx| Host::new("A😀B", false, cx));
    window
        .update(cx, |host, window, cx| {
            host.view.update(cx, |view, cx| {
                view.replace_and_mark_text_in_range(None, "中", None, window, cx)
            });
        })
        .unwrap();
    cx.run_until_parked();
    window
        .update(cx, |host, window, cx| {
            assert_eq!(host.text(), "中A😀B");
            host.view.update(cx, |view, cx| {
                assert_eq!(view.marked_text_range(window, cx), Some(0..1));
                view.replace_text_in_range(None, "🙂", window, cx);
                assert_eq!(view.marked_text_range(window, cx), None);
            });
        })
        .unwrap();
    cx.run_until_parked();
    window
        .update(cx, |host, window, cx| {
            assert_eq!(host.text(), "🙂A😀B");
            host.view.update(cx, |view, cx| {
                let mut adjusted = None;
                assert_eq!(
                    view.text_for_range(0..2, &mut adjusted, window, cx),
                    Some("🙂".to_owned())
                );
                assert_eq!(adjusted, Some(0..2));
            });
        })
        .unwrap();
}

#[gpui::test]
fn painted_view_dispatches_text_input_to_document(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        crate::app_shell::theme::apply_dark_theme(cx);
    });
    let window = cx.add_window(|_, cx| Host::new("ab", false, cx));
    window
        .update(cx, |host, window, cx| {
            let focus = host.view.read(cx).focus.clone();
            window.focus(&focus, cx);
        })
        .unwrap();
    let mut visual = gpui::VisualTestContext::from_window(window.into(), cx);
    visual.run_until_parked();
    visual.refresh().unwrap();
    visual.update(|window, cx| {
        window.draw(cx).clear(cx);
    });
    visual.simulate_input("中");

    window
        .update(cx, |host, _, _| assert_eq!(host.text(), "中ab"))
        .unwrap();
}

#[gpui::test]
fn invalid_explicit_input_range_does_not_fall_back_to_selection(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        crate::app_shell::theme::apply_dark_theme(cx);
    });
    let window = cx.add_window(|_, cx| Host::new("abc", false, cx));
    window
        .update(cx, |host, window, cx| {
            host.view.update(cx, |view, cx| {
                view.replace_text_in_range(Some(99..100), "x", window, cx);
            });
        })
        .unwrap();
    cx.run_until_parked();
    window
        .update(cx, |host, _, _| assert_eq!(host.text(), "abc"))
        .unwrap();
}

#[gpui::test]
fn ime_geometry_uses_scrolled_visible_origin(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        crate::app_shell::theme::apply_dark_theme(cx);
    });
    let window = cx.add_window(|_, cx| Host::new("A中\nB", false, cx));
    let mut visual = gpui::VisualTestContext::from_window(window.into(), cx);
    visual.run_until_parked();
    visual.refresh().unwrap();
    visual.update(|window, cx| {
        window.draw(cx).clear(cx);
    });
    window
        .update(cx, |host, window, cx| {
            host.view.update(cx, |view, cx| {
                let metrics = view.metrics().unwrap();
                let hit = view
                    .character_index_for_point(
                        gpui::point(
                            metrics.bounds.origin.x + metrics.cell_width * 2.0,
                            metrics.bounds.origin.y,
                        ),
                        window,
                        cx,
                    )
                    .unwrap();
                assert_eq!(hit, 2);
                view.scroll_line = 1;
                let bounds = view
                    .bounds_for_range(3..4, metrics.bounds, window, cx)
                    .unwrap();
                assert_eq!(bounds.origin.y, metrics.bounds.origin.y);
            });
        })
        .unwrap();
}

#[gpui::test]
fn sequential_input_in_one_update_is_not_lost(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        crate::app_shell::theme::apply_dark_theme(cx);
    });
    let window = cx.add_window(|_, cx| Host::new("", false, cx));
    window
        .update(cx, |host, window, cx| {
            host.view.update(cx, |view, cx| {
                view.replace_text_in_range(None, "a", window, cx);
                view.replace_text_in_range(None, "b", window, cx);
            });
        })
        .unwrap();
    cx.run_until_parked();
    window
        .update(cx, |host, _, _| assert_eq!(host.text(), "ab"))
        .unwrap();
}

#[gpui::test]
fn marked_range_is_queryable_immediately(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        crate::app_shell::theme::apply_dark_theme(cx);
    });
    let window = cx.add_window(|_, cx| Host::new("", false, cx));
    window
        .update(cx, |host, window, cx| {
            host.view.update(cx, |view, cx| {
                view.replace_and_mark_text_in_range(None, "中", None, window, cx);
                assert_eq!(view.marked_text_range(window, cx), Some(0..1));
            });
        })
        .unwrap();
}

#[gpui::test]
fn crlf_input_is_normalized_before_commit(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        crate::app_shell::theme::apply_dark_theme(cx);
    });
    let window = cx.add_window(|_, cx| Host::new("", false, cx));
    window
        .update(cx, |host, window, cx| {
            host.view.update(cx, |view, cx| {
                view.replace_text_in_range(None, "a\r\nb", window, cx);
            });
        })
        .unwrap();
    cx.run_until_parked();
    window
        .update(cx, |host, _, _| assert_eq!(host.text(), "a\nb"))
        .unwrap();
}

#[gpui::test]
fn system_selection_rejects_grapheme_internal_offsets(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        crate::app_shell::theme::apply_dark_theme(cx);
    });
    let window = cx.add_window(|_, cx| Host::new("e\u{301}", false, cx));
    window
        .update(cx, |host, window, cx| {
            host.view.update(cx, |view, cx| {
                view.set_selected_text_range(1..1, window, cx);
                let selection = view.selected_text_range(false, window, cx).unwrap();
                assert_eq!(selection.range, 0..0);
            });
        })
        .unwrap();
}

#[gpui::test]
fn input_after_a_grapheme_merge_continues_after_the_cluster(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        crate::app_shell::theme::apply_dark_theme(cx);
    });
    let window = cx.add_window(|_, cx| Host::new("\u{301}", false, cx));
    window
        .update(cx, |host, window, cx| {
            host.view.update(cx, |view, cx| {
                view.replace_text_in_range(None, "e", window, cx);
                view.replace_text_in_range(None, "x", window, cx);
            });
            assert_eq!(host.text(), "e\u{301}x");
        })
        .unwrap();
}

#[gpui::test]
fn ime_mark_expands_when_input_merges_with_adjacent_grapheme(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        crate::app_shell::theme::apply_dark_theme(cx);
    });
    let window = cx.add_window(|_, cx| Host::new("e", false, cx));
    window
        .update(cx, |host, window, cx| {
            host.view.update(cx, |view, cx| {
                view.set_selected_text_range(1..1, window, cx);
                view.replace_and_mark_text_in_range(None, "\u{301}", None, window, cx);
                assert_eq!(view.marked_text_range(window, cx), Some(0..2));
                view.replace_text_in_range(None, "x", window, cx);
            });
            assert_eq!(host.text(), "x");
        })
        .unwrap();
}

#[gpui::test]
fn explicit_utf16_ranges_reject_surrogate_internal_offsets(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        crate::app_shell::theme::apply_dark_theme(cx);
    });
    let window = cx.add_window(|_, cx| Host::new("😀", false, cx));
    window
        .update(cx, |host, window, cx| {
            host.view.update(cx, |view, cx| {
                view.set_selected_text_range(1..1, window, cx);
                assert_eq!(
                    view.selected_text_range(false, window, cx).unwrap().range,
                    0..0
                );
                view.replace_text_in_range(Some(1..2), "x", window, cx);
            });
            assert_eq!(host.text(), "😀");
        })
        .unwrap();
}

#[gpui::test]
fn text_canvas_clips_horizontal_and_vertical_overflow(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        crate::app_shell::theme::apply_dark_theme(cx);
    });
    let window = cx.add_window(|_, cx| Host::new("long", false, cx));
    let mut visual = gpui::VisualTestContext::from_window(window.into(), cx);
    visual.run_until_parked();
    visual.refresh().unwrap();
    visual.update(|window, cx| {
        window.draw(cx).clear(cx);
    });
    window
        .update(cx, |host, _, cx| {
            let view = host.view.read(cx);
            let bounds = view.metrics().unwrap().bounds;
            assert_eq!(view.painted_content_mask.borrow().unwrap().bounds, bounds);
        })
        .unwrap();
}

#[gpui::test]
fn external_edit_rebases_selection_and_marked_range(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        crate::app_shell::theme::apply_dark_theme(cx);
    });
    let window = cx.add_window(|_, cx| Host::new("", false, cx));
    window
        .update(cx, |host, window, cx| {
            host.view.update(cx, |view, cx| {
                view.replace_and_mark_text_in_range(None, "x", None, window, cx);
            });
            let revision = host
                .registry
                .borrow()
                .session(host.id)
                .unwrap()
                .text_revision();
            let transaction = EditTransaction {
                document_id: host.id,
                expected_text_revision: revision,
                edits: vec![TextEdit::new(0..0, "Q")],
                origin: EditOrigin::UserInput,
                merge_policy: MergePolicy::Disallowed,
            };
            host.registry
                .borrow_mut()
                .apply(
                    transaction.clone(),
                    TextStateId::from_uuid(Uuid::now_v7()).unwrap(),
                    Instant::now(),
                )
                .unwrap();
            {
                let registry = host.registry.borrow();
                let session = registry.session(host.id).unwrap();
                host.view.update(cx, |view, cx| {
                    view.apply_edit_snapshot(session, &transaction, cx);
                    assert_eq!(view.marked_text_range(window, cx), Some(1..2));
                    assert_eq!(
                        view.selected_text_range(false, window, cx).unwrap().range,
                        2..2
                    );
                });
            }
            host.view.update(cx, |view, cx| {
                view.replace_and_mark_text_in_range(None, "y", None, window, cx);
            });
        })
        .unwrap();
    window
        .update(cx, |host, _, _| assert_eq!(host.text(), "Qy"))
        .unwrap();
}

#[gpui::test]
fn rejected_edit_preserves_selection_and_marked_range(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        crate::app_shell::theme::apply_dark_theme(cx);
    });
    let window = cx.add_window(|_, cx| Host::new("", false, cx));
    window
        .update(cx, |host, window, cx| {
            let transaction = EditTransaction {
                document_id: host.id,
                expected_text_revision: 0,
                edits: vec![TextEdit::new(0..0, "external")],
                origin: EditOrigin::UserInput,
                merge_policy: MergePolicy::Disallowed,
            };
            host.registry
                .borrow_mut()
                .apply(
                    transaction,
                    TextStateId::from_uuid(Uuid::now_v7()).unwrap(),
                    Instant::now(),
                )
                .unwrap();
            host.view.update(cx, |view, cx| {
                view.replace_and_mark_text_in_range(None, "stale", None, window, cx);
                assert_eq!(view.marked_text_range(window, cx), None);
                assert_eq!(
                    view.selected_text_range(false, window, cx).unwrap().range,
                    0..0
                );
            });
            assert_eq!(host.text(), "external");
        })
        .unwrap();
}

#[gpui::test]
fn unchanged_edits_do_not_rebase_or_cancel_view_ranges(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        crate::app_shell::theme::apply_dark_theme(cx);
    });
    let window = cx.add_window(|_, cx| Host::new("ab", false, cx));
    window
        .update(cx, |host, window, cx| {
            host.view.update(cx, |view, cx| {
                view.set_selected_text_range(0..1, window, cx);
            });
            let transaction = EditTransaction {
                document_id: host.id,
                expected_text_revision: 0,
                edits: vec![TextEdit::new(0..1, "a"), TextEdit::new(2..2, "X")],
                origin: EditOrigin::UserInput,
                merge_policy: MergePolicy::Disallowed,
            };
            host.registry
                .borrow_mut()
                .apply(
                    transaction.clone(),
                    TextStateId::from_uuid(Uuid::now_v7()).unwrap(),
                    Instant::now(),
                )
                .unwrap();
            let registry = host.registry.borrow();
            let session = registry.session(host.id).unwrap();
            host.view.update(cx, |view, cx| {
                view.apply_edit_snapshot(session, &transaction, cx);
                assert_eq!(
                    view.selected_text_range(false, window, cx).unwrap().range,
                    0..1
                );
            });
            assert_eq!(host.text(), "abX");
        })
        .unwrap();
}

#[gpui::test]
fn crlf_composition_maps_selected_range_to_normalized_text(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        crate::app_shell::theme::apply_dark_theme(cx);
    });
    let window = cx.add_window(|_, cx| Host::new("", false, cx));
    window
        .update(cx, |host, window, cx| {
            host.view.update(cx, |view, cx| {
                view.replace_and_mark_text_in_range(None, "a\r\nb", Some(4..4), window, cx);
                assert_eq!(
                    view.selected_text_range(false, window, cx).unwrap().range,
                    3..3
                );
            });
            assert_eq!(host.text(), "a\nb");
        })
        .unwrap();
}

#[gpui::test]
fn snapshot_clamps_scroll_and_restores_ime_geometry(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        crate::app_shell::theme::apply_dark_theme(cx);
    });
    let window = cx.add_window(|_, cx| Host::new("a\nb\nc", false, cx));
    window
        .update(cx, |host, window, cx| {
            host.view.update(cx, |view, _cx| view.scroll_line = 2);
            let transaction = EditTransaction {
                document_id: host.id,
                expected_text_revision: 0,
                edits: vec![TextEdit::new(0..5, "x")],
                origin: EditOrigin::UserInput,
                merge_policy: MergePolicy::Disallowed,
            };
            host.registry
                .borrow_mut()
                .apply(
                    transaction.clone(),
                    TextStateId::from_uuid(Uuid::now_v7()).unwrap(),
                    Instant::now(),
                )
                .unwrap();
            let registry = host.registry.borrow();
            let session = registry.session(host.id).unwrap();
            host.view.update(cx, |view, cx| {
                view.apply_edit_snapshot(session, &transaction, cx);
                assert_eq!(view.scroll_line, 0);
                assert!(
                    view.bounds_for_range(0..1, Bounds::default(), window, cx)
                        .is_some()
                );
            });
        })
        .unwrap();
}

#[gpui::test]
fn long_line_tail_uses_the_visible_right_edge_for_ime(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        crate::app_shell::theme::apply_dark_theme(cx);
    });
    let window = cx.add_window(|_, cx| Host::new("abcdefgh", false, cx));
    window
        .update(cx, |host, window, cx| {
            host.view.update(cx, |view, cx| {
                let bounds = Bounds {
                    origin: gpui::point(px(10.0), px(10.0)),
                    size: gpui::size(px(32.0), px(20.0)),
                };
                *view.layout.borrow_mut() = Some(LayoutMetrics {
                    bounds,
                    cell_width: px(8.0),
                    line_height: px(20.0),
                });
                let caret = view.bounds_for_range(8..8, bounds, window, cx).unwrap();
                assert_eq!(caret.origin.x, px(42.0));
            });
        })
        .unwrap();
}

#[gpui::test]
fn layout_cache_reuses_lines_and_invalidates_text_and_theme(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        crate::app_shell::theme::apply_dark_theme(cx);
    });
    let window = cx.add_window(|_, cx| Host::new("abc", false, cx));
    let mut visual = gpui::VisualTestContext::from_window(window.into(), cx);
    visual.run_until_parked();
    visual.refresh().unwrap();
    visual.update(|window, cx| {
        window.draw(cx).clear(cx);
    });
    let first = window
        .update(cx, |host, _, cx| {
            host.view.read(cx).layout_cache.borrow().stats()
        })
        .unwrap();
    visual.refresh().unwrap();
    visual.update(|window, cx| {
        window.draw(cx).clear(cx);
    });
    let second = window
        .update(cx, |host, _, cx| {
            host.view.read(cx).layout_cache.borrow().stats()
        })
        .unwrap();
    assert!(second.0 > first.0);
    assert_eq!(second.1, first.1);

    window
        .update(cx, |host, _, cx| {
            host.view.update(cx, |view, _| view.scroll_column = 1)
        })
        .unwrap();
    visual.refresh().unwrap();
    visual.update(|window, cx| {
        window.draw(cx).clear(cx);
    });
    let scrolled = window
        .update(cx, |host, _, cx| {
            host.view.read(cx).layout_cache.borrow().stats()
        })
        .unwrap();
    assert!(scrolled.1 > second.1);

    window
        .update(cx, |host, window, cx| {
            host.view.update(cx, |view, cx| {
                view.replace_text_in_range(None, "x", window, cx);
            });
        })
        .unwrap();
    visual.refresh().unwrap();
    visual.update(|window, cx| {
        window.draw(cx).clear(cx);
    });
    let edited = window
        .update(cx, |host, _, cx| {
            host.view.read(cx).layout_cache.borrow().stats()
        })
        .unwrap();
    assert!(edited.1 > scrolled.1);

    visual.update(|_, cx| {
        let theme = gpui_component::Theme::global_mut(cx);
        let accent = theme.accent;
        theme.accent = theme.foreground;
        theme.foreground = accent;
    });
    visual.refresh().unwrap();
    visual.update(|window, cx| {
        window.draw(cx).clear(cx);
    });
    let themed = window
        .update(cx, |host, _, cx| {
            host.view.read(cx).layout_cache.borrow().stats()
        })
        .unwrap();
    assert!(themed.1 > edited.1);
}

#[gpui::test]
fn keyboard_moves_by_graphemes_and_shift_extends_selection(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        crate::app_shell::theme::apply_dark_theme(cx);
        crate::app_shell::text_view::init(cx);
    });
    let window = cx.add_window(|_, cx| Host::new("A😀e\u{301}\n中", false, cx));
    window
        .update(cx, |host, window, cx| {
            let focus = host.view.read(cx).focus.clone();
            window.focus(&focus, cx);
        })
        .unwrap();
    let mut visual = gpui::VisualTestContext::from_window(window.into(), cx);
    visual.run_until_parked();
    visual.simulate_keystrokes("right");
    visual.simulate_keystrokes("right");
    visual.simulate_keystrokes("right");
    visual.simulate_keystrokes("right");
    window
        .update(cx, |host, window, cx| {
            host.view.update(cx, |view, cx| {
                assert_eq!(
                    view.selected_text_range(false, window, cx).unwrap().range,
                    6..6
                );
            });
        })
        .unwrap();
    visual.simulate_keystrokes("shift-right");
    window
        .update(cx, |host, window, cx| {
            host.view.update(cx, |view, cx| {
                let selection = view.selected_text_range(false, window, cx).unwrap();
                assert_eq!(selection.range, 6..7);
                assert!(!selection.reversed);
            });
        })
        .unwrap();
    visual.simulate_keystrokes("left");
    window
        .update(cx, |host, window, cx| {
            host.view.update(cx, |view, cx| {
                assert_eq!(
                    view.selected_text_range(false, window, cx).unwrap().range,
                    6..6
                );
            });
        })
        .unwrap();
}

#[gpui::test]
fn vertical_navigation_preserves_display_column_and_scrolls(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        crate::app_shell::theme::apply_dark_theme(cx);
        crate::app_shell::text_view::init(cx);
    });
    let window = cx.add_window(|_, cx| Host::new("中A\nx\n中😀", false, cx));
    window
        .update(cx, |host, window, cx| {
            let focus = host.view.read(cx).focus.clone();
            window.focus(&focus, cx);
            host.view.update(cx, |view, cx| {
                view.set_selected_text_range(1..1, window, cx);
            });
        })
        .unwrap();
    let mut visual = gpui::VisualTestContext::from_window(window.into(), cx);
    visual.run_until_parked();
    visual.simulate_keystrokes("down");
    visual.simulate_keystrokes("down");
    window
        .update(cx, |host, window, cx| {
            host.view.update(cx, |view, cx| {
                assert_eq!(
                    view.selected_text_range(false, window, cx).unwrap().range,
                    6..6
                );
                let mut metrics = LayoutMetrics {
                    bounds: Bounds::default(),
                    cell_width: px(8.0),
                    line_height: px(20.0),
                };
                metrics.bounds.size.height = px(20.0);
                *view.layout.borrow_mut() = Some(metrics);
                view.scroll_line = 0;
                view.ensure_cursor_visible();
                assert_eq!(view.scroll_line, 2);
            });
        })
        .unwrap();
}

#[gpui::test]
fn home_end_and_shift_home_use_line_boundaries(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        crate::app_shell::theme::apply_dark_theme(cx);
        crate::app_shell::text_view::init(cx);
    });
    let window = cx.add_window(|_, cx| Host::new("ab\ncd", false, cx));
    window
        .update(cx, |host, window, cx| {
            let focus = host.view.read(cx).focus.clone();
            window.focus(&focus, cx);
        })
        .unwrap();
    let mut visual = gpui::VisualTestContext::from_window(window.into(), cx);
    visual.run_until_parked();
    visual.simulate_keystrokes("end");
    visual.simulate_keystrokes("shift-home");
    window
        .update(cx, |host, window, cx| {
            host.view.update(cx, |view, cx| {
                let selection = view.selected_text_range(false, window, cx).unwrap();
                assert_eq!(selection.range, 0..2);
                assert!(selection.reversed);
            });
        })
        .unwrap();
}

#[gpui::test]
fn system_selection_resets_vertical_preferred_column(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        crate::app_shell::theme::apply_dark_theme(cx);
        crate::app_shell::text_view::init(cx);
    });
    let window = cx.add_window(|_, cx| Host::new("abcdef\nx\nabcdef", false, cx));
    window
        .update(cx, |host, window, cx| {
            let focus = host.view.read(cx).focus.clone();
            window.focus(&focus, cx);
            host.view.update(cx, |view, cx| {
                view.set_selected_text_range(5..5, window, cx);
            });
        })
        .unwrap();
    let mut visual = gpui::VisualTestContext::from_window(window.into(), cx);
    visual.run_until_parked();
    visual.simulate_keystrokes("down");
    window
        .update(cx, |host, window, cx| {
            host.view.update(cx, |view, cx| {
                view.set_selected_text_range(2..2, window, cx);
            });
        })
        .unwrap();
    visual.simulate_keystrokes("down");
    visual.simulate_keystrokes("down");
    window
        .update(cx, |host, window, cx| {
            host.view.update(cx, |view, cx| {
                assert_eq!(
                    view.selected_text_range(false, window, cx).unwrap().range,
                    11..11
                );
            });
        })
        .unwrap();
}

#[gpui::test]
fn horizontal_scroll_keeps_long_line_cursor_visible(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        crate::app_shell::theme::apply_dark_theme(cx);
    });
    let window = cx.add_window(|_, cx| Host::new("abcdef", false, cx));
    window
        .update(cx, |host, window, cx| {
            host.view.update(cx, |view, cx| {
                let bounds = Bounds {
                    origin: gpui::point(px(10.0), px(10.0)),
                    size: gpui::size(px(28.0), px(20.0)),
                };
                *view.layout.borrow_mut() = Some(LayoutMetrics {
                    bounds,
                    cell_width: px(8.0),
                    line_height: px(20.0),
                });
                view.move_line_boundary(true, false, cx);
                assert_eq!(view.scroll_column, 3);
                let caret = view.bounds_for_range(6..6, bounds, window, cx).unwrap();
                assert_eq!(caret.origin.x, px(34.0));
            });
        })
        .unwrap();
}

#[gpui::test]
fn ime_geometry_clamps_wide_grapheme_to_viewport_edges(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        crate::app_shell::theme::apply_dark_theme(cx);
    });
    let window = cx.add_window(|_, cx| Host::new("中", false, cx));
    window
        .update(cx, |host, window, cx| {
            host.view.update(cx, |view, cx| {
                let bounds = Bounds {
                    origin: gpui::point(px(10.0), px(10.0)),
                    size: gpui::size(px(8.0), px(20.0)),
                };
                *view.layout.borrow_mut() = Some(LayoutMetrics {
                    bounds,
                    cell_width: px(8.0),
                    line_height: px(20.0),
                });
                assert_eq!(
                    view.bounds_for_range(1..1, bounds, window, cx)
                        .unwrap()
                        .origin
                        .x,
                    px(18.0)
                );
                view.scroll_column = 1;
                assert_eq!(
                    view.bounds_for_range(0..1, bounds, window, cx)
                        .unwrap()
                        .origin
                        .x,
                    px(10.0)
                );
            });
        })
        .unwrap();
}

#[gpui::test]
fn tab_stops_remain_absolute_after_horizontal_scroll(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        crate::app_shell::theme::apply_dark_theme(cx);
    });
    let window = cx.add_window(|_, cx| Host::new("A\tB", false, cx));
    window
        .update(cx, |host, window, cx| {
            host.view.update(cx, |view, cx| {
                let bounds = Bounds {
                    origin: gpui::point(px(10.0), px(10.0)),
                    size: gpui::size(px(32.0), px(20.0)),
                };
                *view.layout.borrow_mut() = Some(LayoutMetrics {
                    bounds,
                    cell_width: px(8.0),
                    line_height: px(20.0),
                });
                view.scroll_column = 2;
                assert_eq!(
                    view.bounds_for_range(2..3, bounds, window, cx)
                        .unwrap()
                        .origin
                        .x,
                    px(26.0)
                );
                assert_eq!(
                    view.character_index_for_point(gpui::point(px(26.0), px(10.0)), window, cx,),
                    Some(2)
                );
            });
        })
        .unwrap();
}

#[gpui::test]
fn subcell_viewport_still_follows_the_cursor(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        crate::app_shell::theme::apply_dark_theme(cx);
    });
    let window = cx.add_window(|_, cx| Host::new("a", false, cx));
    window
        .update(cx, |host, window, cx| {
            host.view.update(cx, |view, cx| {
                let bounds = Bounds {
                    origin: gpui::point(px(10.0), px(10.0)),
                    size: gpui::size(px(4.0), px(20.0)),
                };
                *view.layout.borrow_mut() = Some(LayoutMetrics {
                    bounds,
                    cell_width: px(8.0),
                    line_height: px(20.0),
                });
                view.move_line_boundary(true, false, cx);
                assert_eq!(view.scroll_column, 1);
                assert_eq!(
                    view.bounds_for_range(1..1, bounds, window, cx)
                        .unwrap()
                        .origin
                        .x,
                    px(10.0)
                );
            });
        })
        .unwrap();
}

#[gpui::test]
fn keyboard_editing_actions_commit_through_document(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        crate::app_shell::theme::apply_dark_theme(cx);
        crate::app_shell::text_view::init(cx);
    });
    let window = cx.add_window(|_, cx| Host::new("ab", false, cx));
    window
        .update(cx, |host, window, cx| {
            let focus = host.view.read(cx).focus.clone();
            window.focus(&focus, cx);
        })
        .unwrap();
    let mut visual = gpui::VisualTestContext::from_window(window.into(), cx);
    visual.run_until_parked();
    visual.simulate_keystrokes("end");
    visual.simulate_keystrokes("backspace");
    visual.simulate_keystrokes("enter");
    visual.simulate_keystrokes("ctrl-a");
    visual.simulate_keystrokes("delete");
    window
        .update(cx, |host, _, _| assert_eq!(host.text(), ""))
        .unwrap();
}

#[gpui::test]
fn undo_redo_travel_the_shared_document_history(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        crate::app_shell::theme::apply_dark_theme(cx);
        crate::app_shell::text_view::init(cx);
    });
    let window = cx.add_window(|_, cx| Host::new("", false, cx));
    window
        .update(cx, |host, window, cx| {
            let focus = host.view.read(cx).focus.clone();
            window.focus(&focus, cx);
        })
        .unwrap();
    let mut visual = gpui::VisualTestContext::from_window(window.into(), cx);
    visual.run_until_parked();
    visual.refresh().unwrap();
    visual.update(|window, cx| {
        window.draw(cx).clear(cx);
    });
    visual.simulate_input("ab");
    visual.simulate_keystrokes("ctrl-z");
    window
        .update(cx, |host, window, cx| {
            assert_eq!(host.text(), "");
            host.view.update(cx, |view, cx| {
                assert_eq!(
                    view.selected_text_range(false, window, cx).unwrap().range,
                    0..0
                );
            });
        })
        .unwrap();
    visual.simulate_keystrokes("ctrl-y");
    window
        .update(cx, |host, _, _| assert_eq!(host.text(), "ab"))
        .unwrap();
}

#[gpui::test]
fn empty_history_does_not_reset_selection(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        crate::app_shell::theme::apply_dark_theme(cx);
        crate::app_shell::text_view::init(cx);
    });
    let window = cx.add_window(|_, cx| Host::new("ab", false, cx));
    window
        .update(cx, |host, window, cx| {
            let focus = host.view.read(cx).focus.clone();
            window.focus(&focus, cx);
            host.view.update(cx, |view, cx| {
                view.set_selected_text_range(1..1, window, cx);
            });
        })
        .unwrap();
    let mut visual = gpui::VisualTestContext::from_window(window.into(), cx);
    visual.run_until_parked();
    visual.simulate_keystrokes("ctrl-z");
    window
        .update(cx, |host, window, cx| {
            host.view.update(cx, |view, cx| {
                assert_eq!(
                    view.selected_text_range(false, window, cx).unwrap().range,
                    1..1
                );
            });
        })
        .unwrap();
}

#[gpui::test]
fn composition_blocks_keyboard_editing_and_history(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        crate::app_shell::theme::apply_dark_theme(cx);
    });
    let window = cx.add_window(|_, cx| Host::new("", false, cx));
    window
        .update(cx, |host, window, cx| {
            host.view.update(cx, |view, cx| {
                view.replace_and_mark_text_in_range(None, "x", None, window, cx);
                view.backspace(cx);
                view.delete(cx);
                view.insert_newline(cx);
                view.select_all(cx);
                view.travel_history(false, cx);
                assert_eq!(view.marked_text_range(window, cx), Some(0..1));
            });
            assert_eq!(host.text(), "x");
        })
        .unwrap();
}

#[gpui::test]
fn clipboard_shortcuts_copy_and_paste_unicode_text(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        crate::app_shell::theme::apply_dark_theme(cx);
        crate::app_shell::text_view::init(cx);
    });
    let window = cx.add_window(|_, cx| Host::new("a😀b", false, cx));
    window
        .update(cx, |host, window, cx| {
            let focus = host.view.read(cx).focus.clone();
            window.focus(&focus, cx);
            host.view.update(cx, |view, cx| {
                view.set_selected_text_range(1..3, window, cx);
            });
        })
        .unwrap();
    let mut visual = gpui::VisualTestContext::from_window(window.into(), cx);
    visual.run_until_parked();
    visual.simulate_keystrokes("ctrl-shift-c");
    window
        .update(cx, |_, _, cx| {
            assert_eq!(
                cx.read_from_clipboard().and_then(|item| item.text()),
                Some("😀".to_owned())
            );
            cx.write_to_clipboard(gpui::ClipboardItem::new_string("中\r\nx".to_owned()));
        })
        .unwrap();
    visual.simulate_keystrokes("ctrl-shift-v");
    window
        .update(cx, |host, _, _| assert_eq!(host.text(), "a中\nxb"))
        .unwrap();
}

#[gpui::test]
fn collapsed_copy_preserves_existing_clipboard(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        crate::app_shell::theme::apply_dark_theme(cx);
        crate::app_shell::text_view::init(cx);
    });
    let window = cx.add_window(|_, cx| Host::new("ab", false, cx));
    window
        .update(cx, |host, window, cx| {
            let focus = host.view.read(cx).focus.clone();
            window.focus(&focus, cx);
            cx.write_to_clipboard(gpui::ClipboardItem::new_string("sentinel".to_owned()));
        })
        .unwrap();
    let mut visual = gpui::VisualTestContext::from_window(window.into(), cx);
    visual.run_until_parked();
    visual.simulate_keystrokes("ctrl-shift-c");
    window
        .update(cx, |_, _, cx| {
            assert_eq!(
                cx.read_from_clipboard().and_then(|item| item.text()),
                Some("sentinel".to_owned())
            );
        })
        .unwrap();
}

#[gpui::test]
fn read_only_view_rejects_ime_without_touching_document(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        crate::app_shell::theme::apply_dark_theme(cx);
    });
    let window = cx.add_window(|_, cx| Host::new("locked", true, cx));
    window
        .update(cx, |host, window, cx| {
            cx.write_to_clipboard(gpui::ClipboardItem::new_string("paste".to_owned()));
            host.view.update(cx, |view, cx| {
                assert!(!view.accepts_text_input(window, cx));
                view.replace_and_mark_text_in_range(None, "x", None, window, cx);
                view.backspace(cx);
                view.delete(cx);
                view.insert_newline(cx);
                view.paste(cx);
                view.travel_history(false, cx);
            });
            assert_eq!(host.text(), "locked");
        })
        .unwrap();
}
