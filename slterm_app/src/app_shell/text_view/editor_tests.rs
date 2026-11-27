use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui::{
    AppContext as _, Bounds, Context, Entity, EntityInputHandler as _, ParentElement as _, Render,
    Styled as _, TestAppContext, Window, px,
};
use gpui_component::ActiveTheme;
use ropey::Rope;
use slterm_document::{
    DocumentId, DocumentMetadata, DocumentRegistry, EditOrigin, EditTransaction, LanguageId,
    MergePolicy, TextEdit, TextStateId,
};
use slterm_path::{CanonicalPath, ResolvedPath};
use slterm_workspace::ViewKey;
use uuid::Uuid;

include!("editor_tests/wrap.rs");

use super::{
    DocumentEditRequest, DocumentEditResponse, DocumentTextSnapshot, DocumentTextView,
    LayoutMetrics, TextSelection, transform_selection,
};

struct Host {
    registry: Rc<RefCell<DocumentRegistry>>,
    id: DocumentId,
    view: Entity<DocumentTextView>,
    syntax: Option<Rc<RefCell<super::super::syntax::DocumentSyntax>>>,
}

impl Host {
    fn new(text: &str, read_only: bool, cx: &mut Context<'_, Self>) -> Self {
        Self::configured(text, read_only, None, cx)
    }

    fn new_rust(text: &str, cx: &mut Context<'_, Self>) -> Self {
        Self::configured(text, false, Some(LanguageId::Rust), cx)
    }

    fn configured(
        text: &str,
        read_only: bool,
        language: Option<LanguageId>,
        cx: &mut Context<'_, Self>,
    ) -> Self {
        Self::configured_with_store(
            text,
            read_only,
            language,
            super::super::view_state::EditorViewStateStore::transient(),
            cx,
        )
    }

    fn configured_with_store(
        text: &str,
        read_only: bool,
        language: Option<LanguageId>,
        view_states: super::super::view_state::EditorViewStateStore,
        cx: &mut Context<'_, Self>,
    ) -> Self {
        cx.set_global(crate::app_shell::config::Settings::default());
        let id: DocumentId = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0f2a".parse().unwrap();
        let state = TextStateId::from_uuid(Uuid::now_v7()).unwrap();
        let mut registry = DocumentRegistry::new();
        registry
            .open(
                id,
                ResolvedPath::new(
                    CanonicalPath::new(if language == Some(LanguageId::Rust) {
                        "D:/work/editor.rs"
                    } else {
                        "D:/work/editor.txt"
                    })
                    .unwrap(),
                    None,
                ),
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
        let syntax = language.and_then(|language| {
            super::super::syntax::DocumentSyntax::new(
                registry.borrow().session(id).unwrap(),
                language,
            )
            .map(|syntax| Rc::new(RefCell::new(syntax)))
        });
        let view = cx.new(|cx| {
            let session = registry.borrow();
            let mut view = DocumentTextView::new(
                session.session(id).unwrap(),
                Box::new(move |request, _, _| {
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
                language.unwrap_or(LanguageId::PlainText),
                syntax.clone(),
                ViewKey::Editor {
                    project_id: "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b".parse().unwrap(),
                    document_id: id,
                },
                view_states,
                cx,
            );
            // 测试夹具按满宽测量软换行；生产编辑域固定预留行标槽位。
            view.git_gutter = false;
            view
        });
        Self {
            registry,
            id,
            view,
            syntax,
        }
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

/// 外部快照与撤销/重做不因选区复位把滚动位置拉回顶部。
/// 软换行下 reveal_line 必须换算为显示行，不能把逻辑行号当滚动行。
#[gpui::test]
fn reveal_line_targets_the_display_row_under_soft_wrap(cx: &mut TestAppContext) {
    let host = cx.update(|cx| {
        cx.new(|cx| {
            Host::new(
                "abcdefgh
xyz
",
                false,
                cx,
            )
        })
    });
    let view = host.read_with(cx, |host, _| host.view.clone());
    view.update(cx, |view, cx| {
        view.wrap = true;
        view.invalidate_wrap_index();
        let index = super::super::wrap::SoftWrapIndex::new(&view.text, view.text_state, 4);
        let byte = view.text.line_to_byte_idx(1, ropey::LineType::LF);
        let expected = index.position(byte).expect("逻辑行必须有显示位置").row;
        assert!(expected > 1, "前置：目标行必须已被前一行换行下推");
        *view.wrap_index.borrow_mut() = Some(index);
        view.reveal_line(1, cx);
        assert_eq!(
            view.scroll_state_for_test().0,
            expected,
            "滚动行必须是显示行"
        );
    });
}

/// 索引未就绪时 reveal_line 只登记待揭示逻辑行，索引完成后换算。
#[gpui::test]
fn reveal_line_waits_for_the_wrap_index_before_scrolling(cx: &mut TestAppContext) {
    let host = cx.update(|cx| {
        cx.new(|cx| {
            Host::new(
                "abcdefgh
xyz
",
                false,
                cx,
            )
        })
    });
    let view = host.read_with(cx, |host, _| host.view.clone());
    view.update(cx, |view, cx| {
        view.wrap = true;
        view.invalidate_wrap_index();
        view.reveal_line(1, cx);
        assert_eq!(
            view.scroll_state_for_test().0,
            0,
            "索引未就绪不得按逻辑行滚动"
        );
        assert_eq!(view.wrap_reveal_line, Some(1), "必须保留待揭示行");
    });
}

#[gpui::test]
fn external_snapshot_keeps_the_current_scroll_position(cx: &mut TestAppContext) {
    let text = (0..50)
        .map(|index| {
            format!(
                "line {index}
"
            )
        })
        .collect::<String>();
    let host = cx.update(|cx| cx.new(|cx| Host::new(&text, false, cx)));
    let view = host.read_with(cx, |host, _| host.view.clone());
    view.update(cx, |view, _| {
        view.set_scroll_for_test(20, 3);
        *view.layout.borrow_mut() = Some(LayoutMetrics {
            bounds: Bounds {
                origin: gpui::point(px(0.0), px(0.0)),
                size: gpui::size(px(120.0), px(100.0)),
            },
            cell_width: px(8.0),
            line_height: px(20.0),
        });
    });

    cx.update(|cx| {
        host.update(cx, |host, cx| {
            let registry = host.registry.borrow();
            let session = registry.session(host.id).unwrap();
            host.view
                .update(cx, |view, cx| view.apply_snapshot(session, cx));
        });
    });

    let (line, column, _, _) = view.read_with(cx, |view, _| view.scroll_state_for_test());
    assert_eq!(line, 20, "外部快照不得把滚动行重置到顶部");
    assert_eq!(column, 3, "外部快照不得丢弃滚动列");
}

#[gpui::test]
fn extreme_restored_scroll_does_not_overflow_editor_painting_or_hit_testing(
    cx: &mut TestAppContext,
) {
    cx.update(|cx| {
        gpui_component::init(cx);
        crate::app_shell::theme::apply_dark_theme(cx);
    });
    let states = super::super::view_state::EditorViewStateStore::transient();
    let key = ViewKey::Editor {
        project_id: "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b".parse().unwrap(),
        document_id: "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0f2a".parse().unwrap(),
    };
    states.restore(
        key,
        super::super::view_state::EditorViewState::new(0, usize::MAX),
    );
    let window = cx.add_window(|_, cx| Host::configured_with_store("abc", false, None, states, cx));
    let mut visual = gpui::VisualTestContext::from_window(window.into(), cx);
    visual.run_until_parked();
    visual.refresh().unwrap();
    visual.update(|window, cx| {
        window.draw(cx).clear(cx);
    });
    let view = window.update(cx, |host, _, _| host.view.clone()).unwrap();
    view.update(cx, |view, _| {
        let (line, column, _, _) = view.scroll_state_for_test();
        assert_eq!(line, 0, "恢复行必须夹到真实行范围");
        assert_eq!(column, 3 * 4, "恢复列必须夹到显示列上界，而不是 usize::MAX");
        *view.layout.borrow_mut() = Some(LayoutMetrics {
            bounds: Bounds::default(),
            cell_width: px(8.0),
            line_height: px(20.0),
        });
        let _ = view.byte_for_point(gpui::point(px(0.0), px(0.0)));
    });
}

struct SharedHost {
    first: Entity<Host>,
    second: Entity<DocumentTextView>,
}

impl Render for SharedHost {
    fn render(&mut self, _: &mut Window, _: &mut Context<'_, Self>) -> impl gpui::IntoElement {
        gpui::div()
            .flex()
            .flex_col()
            .child(gpui::div().h(px(100.0)).child(self.first.clone()))
            .child(gpui::div().h(px(100.0)).child(self.second.clone()))
    }
}

fn has_keyword(
    painted: &super::PaintedSyntax,
    range: std::ops::Range<usize>,
    color: gpui::Hsla,
) -> bool {
    painted.borrow().iter().any(|(line, runs)| {
        *line == 0
            && runs
                .iter()
                .scan(0, |start, run| {
                    let actual = *start..*start + run.len;
                    *start += run.len;
                    Some((actual, run.color))
                })
                .any(|(actual, actual_color)| actual == range && actual_color == color)
    })
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
fn syntax_worker_publishes_semantic_runs_to_the_painted_view(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        crate::app_shell::theme::apply_dark_theme(cx);
    });
    let keyword_color = cx.update(|cx| cx.theme().colors.accent);
    let window = cx.add_window(|_, cx| Host::new_rust("fn main() {}\n", cx));
    let mut visual = gpui::VisualTestContext::from_window(window.into(), cx);
    let painted = window
        .update(cx, |host, _, cx| host.view.read(cx).painted_syntax.clone())
        .unwrap();
    for _ in 0..100 {
        std::thread::sleep(Duration::from_millis(1));
        visual.run_until_parked();
        visual.executor().advance_clock(Duration::from_millis(30));
        visual.run_until_parked();
        visual.refresh().unwrap();
        visual.update(|window, cx| {
            window.draw(cx).clear(cx);
        });
        if has_keyword(&painted, 0..2, keyword_color) {
            break;
        }
    }
    assert!(
        has_keyword(&painted, 0..2, keyword_color),
        "实际 shaping runs: {:?}",
        painted.borrow()
    );

    window
        .update(cx, |host, window, cx| {
            host.view.update(cx, |view, cx| {
                view.replace_text_in_range(None, "pub ", window, cx);
            });
        })
        .unwrap();
    window
        .update(cx, |host, _, _| {
            assert_eq!(host.text(), "pub fn main() {}\n")
        })
        .unwrap();
    for _ in 0..100 {
        std::thread::sleep(Duration::from_millis(1));
        visual.run_until_parked();
        visual.executor().advance_clock(Duration::from_millis(30));
        visual.run_until_parked();
        visual.refresh().unwrap();
        visual.update(|window, cx| {
            window.draw(cx).clear(cx);
        });
        if has_keyword(&painted, 4..6, keyword_color) {
            break;
        }
    }
    assert!(
        has_keyword(&painted, 4..6, keyword_color),
        "编辑后 shaping runs: {:?}",
        painted.borrow()
    );
}

#[gpui::test]
fn two_painted_views_independently_adopt_a_shared_syntax_tree(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        crate::app_shell::theme::apply_dark_theme(cx);
    });
    let keyword_color = cx.update(|cx| cx.theme().colors.accent);
    let window = cx.add_window(|_, cx| -> SharedHost {
        let first = cx.new(|cx| Host::new_rust("fn main() {}\n", cx));
        let id = first.read(cx).id;
        let registry = first.read(cx).registry.clone();
        let syntax = first.read(cx).syntax.clone();
        let sink_registry = registry.clone();
        let second = cx.new(|cx| {
            let registry_read = registry.borrow();
            let session = registry_read.session(id).unwrap();
            DocumentTextView::new(
                session,
                Box::new(move |request, _, _| {
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
                LanguageId::Rust,
                syntax,
                ViewKey::Editor {
                    project_id: "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b".parse().unwrap(),
                    document_id: id,
                },
                super::super::view_state::EditorViewStateStore::transient(),
                cx,
            )
        });
        SharedHost { first, second }
    });
    let mut visual = gpui::VisualTestContext::from_window(window.into(), cx);
    let (first, second) = window
        .update(cx, |host, _, cx| {
            (
                host.first.read(cx).view.read(cx).painted_syntax.clone(),
                host.second.read(cx).painted_syntax.clone(),
            )
        })
        .unwrap();
    for _ in 0..100 {
        std::thread::sleep(Duration::from_millis(1));
        visual.run_until_parked();
        visual.executor().advance_clock(Duration::from_millis(30));
        visual.run_until_parked();
        visual.refresh().unwrap();
        visual.update(|window, cx| {
            window.draw(cx).clear(cx);
        });
        if has_keyword(&first, 0..2, keyword_color) && has_keyword(&second, 0..2, keyword_color) {
            break;
        }
    }
    assert!(has_keyword(&first, 0..2, keyword_color));
    assert!(
        has_keyword(&second, 0..2, keyword_color),
        "第二视图未采纳共享首树"
    );

    window
        .update(cx, |host, _, cx| {
            let id = host.first.read(cx).id;
            let registry = host.first.read(cx).registry.clone();
            let transaction = EditTransaction {
                document_id: id,
                expected_text_revision: 0,
                edits: vec![TextEdit::new(0..0, "pub ")],
                origin: EditOrigin::UserInput,
                merge_policy: MergePolicy::Disallowed,
            };
            let state = TextStateId::from_uuid(Uuid::now_v7()).unwrap();
            registry
                .borrow_mut()
                .apply(transaction.clone(), state, Instant::now())
                .unwrap();
            let registry = registry.borrow();
            let session = registry.session(id).unwrap();
            let first_view = host.first.read(cx).view.clone();
            first_view.update(cx, |view, cx| {
                view.apply_edit_snapshot(session, &transaction, cx)
            });
            host.second.update(cx, |view, cx| {
                view.apply_edit_snapshot(session, &transaction, cx)
            });
        })
        .unwrap();
    for _ in 0..100 {
        std::thread::sleep(Duration::from_millis(1));
        visual.run_until_parked();
        visual.executor().advance_clock(Duration::from_millis(30));
        visual.run_until_parked();
        visual.refresh().unwrap();
        visual.update(|window, cx| {
            window.draw(cx).clear(cx);
        });
        if has_keyword(&first, 4..6, keyword_color) && has_keyword(&second, 4..6, keyword_color) {
            break;
        }
    }
    assert!(has_keyword(&first, 4..6, keyword_color));
    assert!(
        has_keyword(&second, 4..6, keyword_color),
        "第二视图未采纳编辑后的共享树"
    );
}

#[gpui::test]
fn unchanged_replacement_preserves_selection_and_view_transients(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        crate::app_shell::theme::apply_dark_theme(cx);
    });
    let window = cx.add_window(|_, cx| Host::new("ab", false, cx));
    window
        .update(cx, |host, window, cx| {
            host.view.update(cx, |view, cx| {
                view.set_selected_text_range(1..2, window, cx);
                view.replace_text_in_range(None, "b", window, cx);
                assert_eq!(
                    view.selected_text_range(false, window, cx).unwrap().range,
                    1..2
                );
            });
        })
        .unwrap();
}

#[gpui::test]
fn unchanged_ime_commit_clears_composition_and_updates_selection(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        crate::app_shell::theme::apply_dark_theme(cx);
    });
    let window = cx.add_window(|_, cx| Host::new("", false, cx));
    window
        .update(cx, |host, window, cx| {
            host.view.update(cx, |view, cx| {
                view.replace_and_mark_text_in_range(None, "中", None, window, cx);
                view.replace_text_in_range(None, "中", window, cx);
                assert_eq!(view.marked_text_range(window, cx), None);
                assert_eq!(
                    view.selected_text_range(false, window, cx).unwrap().range,
                    1..1
                );
                view.backspace(cx);
            });
            assert_eq!(host.text(), "");
        })
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
fn mouse_click_drag_and_shift_click_control_selection(cx: &mut TestAppContext) {
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
    let metrics = window
        .update(cx, |host, _, cx| host.view.read(cx).metrics().unwrap())
        .unwrap();
    let start = gpui::point(
        metrics.bounds.origin.x + metrics.cell_width * 2.0,
        metrics.bounds.origin.y,
    );
    visual.simulate_mouse_down(start, gpui::MouseButton::Left, gpui::Modifiers::default());
    visual.simulate_mouse_move(
        gpui::point(
            metrics.bounds.origin.x,
            metrics.bounds.origin.y + metrics.line_height,
        ),
        gpui::MouseButton::Left,
        gpui::Modifiers::default(),
    );
    visual.simulate_mouse_up(
        gpui::point(
            metrics.bounds.origin.x,
            metrics.bounds.origin.y + metrics.line_height,
        ),
        gpui::MouseButton::Left,
        gpui::Modifiers::default(),
    );
    window
        .update(cx, |host, window, cx| {
            host.view.update(cx, |view, cx| {
                assert_eq!(
                    view.selected_text_range(false, window, cx).unwrap().range,
                    2..3
                );
            });
        })
        .unwrap();
    visual.simulate_mouse_move(
        gpui::point(metrics.bounds.origin.x, metrics.bounds.origin.y),
        None,
        gpui::Modifiers::default(),
    );
    window
        .update(cx, |host, window, cx| {
            host.view.update(cx, |view, cx| {
                assert_eq!(
                    view.selected_text_range(false, window, cx).unwrap().range,
                    2..3
                );
            });
        })
        .unwrap();
    visual.simulate_click(
        gpui::point(metrics.bounds.origin.x, metrics.bounds.origin.y),
        gpui::Modifiers {
            shift: true,
            ..gpui::Modifiers::default()
        },
    );
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
fn mouse_wheel_scrolls_lines_and_preserves_fractional_pixels(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        crate::app_shell::theme::apply_dark_theme(cx);
    });
    let window = cx.add_window(|_, cx| Host::new("0\n1\n2\n3\n4", false, cx));
    let mut visual = gpui::VisualTestContext::from_window(window.into(), cx);
    visual.run_until_parked();
    visual.refresh().unwrap();
    visual.update(|window, cx| {
        window.draw(cx).clear(cx);
    });
    let metrics = window
        .update(cx, |host, _, cx| host.view.read(cx).metrics().unwrap())
        .unwrap();
    visual.simulate_event(gpui::ScrollWheelEvent {
        position: metrics.bounds.center(),
        delta: gpui::ScrollDelta::Pixels(gpui::point(
            px(0.0),
            px(-metrics.line_height.as_f32() / 2.0),
        )),
        modifiers: gpui::Modifiers::default(),
        ..Default::default()
    });
    window
        .update(cx, |host, _, cx| {
            assert_eq!(host.view.read(cx).scroll_line, 0)
        })
        .unwrap();
    visual.simulate_event(gpui::ScrollWheelEvent {
        position: metrics.bounds.center(),
        delta: gpui::ScrollDelta::Pixels(gpui::point(
            px(0.0),
            px(-metrics.line_height.as_f32() / 2.0),
        )),
        modifiers: gpui::Modifiers::default(),
        ..Default::default()
    });
    window
        .update(cx, |host, _, cx| {
            assert_eq!(host.view.read(cx).scroll_line, 1)
        })
        .unwrap();
    visual.simulate_event(gpui::ScrollWheelEvent {
        position: metrics.bounds.center(),
        delta: gpui::ScrollDelta::Lines(gpui::point(0.0, 1.0)),
        modifiers: gpui::Modifiers::default(),
        ..Default::default()
    });
    window
        .update(cx, |host, _, cx| {
            assert_eq!(host.view.read(cx).scroll_line, 0)
        })
        .unwrap();
}

#[gpui::test]
fn snapshot_change_cancels_an_inflight_mouse_selection(cx: &mut TestAppContext) {
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
    let metrics = window
        .update(cx, |host, _, cx| host.view.read(cx).metrics().unwrap())
        .unwrap();
    visual.simulate_mouse_down(
        gpui::point(
            metrics.bounds.origin.x + metrics.cell_width * 3.0,
            metrics.bounds.origin.y,
        ),
        gpui::MouseButton::Left,
        gpui::Modifiers::default(),
    );
    window
        .update(cx, |host, _window, cx| {
            let transaction = EditTransaction {
                document_id: host.id,
                expected_text_revision: 0,
                edits: vec![TextEdit::new(0..3, "")],
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
            });
        })
        .unwrap();
    visual.simulate_mouse_move(
        metrics.bounds.origin,
        gpui::MouseButton::Left,
        gpui::Modifiers::default(),
    );
    window
        .update(cx, |host, window, cx| {
            host.view.update(cx, |view, cx| {
                assert_eq!(
                    view.selected_text_range(false, window, cx).unwrap().range,
                    0..0
                );
            });
        })
        .unwrap();
}

#[gpui::test]
fn ime_start_cancels_an_inflight_mouse_selection(cx: &mut TestAppContext) {
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
    let metrics = window
        .update(cx, |host, _, cx| host.view.read(cx).metrics().unwrap())
        .unwrap();
    visual.simulate_mouse_down(
        gpui::point(
            metrics.bounds.origin.x + metrics.cell_width * 3.0,
            metrics.bounds.origin.y,
        ),
        gpui::MouseButton::Left,
        gpui::Modifiers::default(),
    );
    window
        .update(cx, |host, window, cx| {
            host.view.update(cx, |view, cx| {
                view.replace_and_mark_text_in_range(None, "x", None, window, cx);
            });
        })
        .unwrap();
    visual.simulate_mouse_move(
        metrics.bounds.origin,
        gpui::MouseButton::Left,
        gpui::Modifiers::default(),
    );
    window
        .update(cx, |host, window, cx| {
            host.view.update(cx, |view, cx| {
                assert_eq!(
                    view.selected_text_range(false, window, cx).unwrap().range,
                    4..4
                );
                assert_eq!(view.marked_text_range(window, cx), Some(3..4));
            });
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
