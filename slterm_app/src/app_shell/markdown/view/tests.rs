use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;
use std::str::FromStr;

use gpui::{AppContext as _, EntityInputHandler as _, TestAppContext};
use ropey::Rope;
use slterm_document::{DocumentId, DocumentMetadata, DocumentRegistry, TextStateId};
use slterm_path::{CanonicalPath, ResolvedPath};
use slterm_workspace::{ProjectId, ViewKey};
use uuid::Uuid;

use super::{MarkdownRenderServices, MarkdownView};
use crate::app_shell::text_view::{
    DocumentEditRequest, DocumentEditResponse, DocumentTextSnapshot, DocumentViewStateBinding,
    EditorViewStateStore,
};

#[gpui::test]
fn markdown_source_render_share_document(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        cx.set_global(crate::app_shell::config::Settings::default());
        crate::app_shell::theme::apply_dark_theme(cx);
        crate::app_shell::markdown::init(cx);
    });
    let id = DocumentId::from_str("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2a").unwrap();
    let mut registry = DocumentRegistry::new();
    registry
        .open(
            id,
            ResolvedPath::new(CanonicalPath::new("D:/work/README.md").unwrap(), None),
            Rope::from_str("# Old\n"),
            TextStateId::from_uuid(Uuid::now_v7()).unwrap(),
            std::sync::Arc::from(b"# Old\n".as_slice()),
            DocumentMetadata::default(),
        )
        .unwrap();
    let registry = Rc::new(RefCell::new(registry));
    let session = registry.borrow().session(id).unwrap().clone();
    let sink_registry = registry.clone();
    let (view, visual) = cx.add_window_view(|_, cx| {
        MarkdownView::new(
            &session,
            Box::new(move |request, _, _| {
                let mut registry = sink_registry.borrow_mut();
                let outcome = match request {
                    DocumentEditRequest::Edit(transaction, state) => {
                        registry.apply(transaction, state, std::time::Instant::now())?
                    }
                    DocumentEditRequest::Travel { redo } => {
                        if redo {
                            registry.redo(id)?
                        } else {
                            registry.undo(id)?
                        }
                    }
                };
                let session = registry.session(id).unwrap();
                Ok(DocumentEditResponse {
                    snapshot: DocumentTextSnapshot::from_session(session),
                    outcome,
                })
            }),
            None,
            MarkdownRenderServices::default(),
            Vec::new(),
            DocumentViewStateBinding::new(
                ViewKey::Markdown {
                    project_id: ProjectId::from_str("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b")
                        .unwrap(),
                    document_id: id,
                },
                EditorViewStateStore::transient(),
            ),
            cx,
        )
    });
    visual.update(|window, cx| {
        view.update(cx, |view, cx| view.toggle_mode_for_test(window, cx));
        view.read(cx).source_for_test().update(cx, |source, cx| {
            source.replace_text_in_range(Some(0..0), "# New\n\n", window, cx)
        });
    });
    assert_eq!(
        registry
            .borrow()
            .session(id)
            .unwrap()
            .text()
            .unwrap()
            .to_string(),
        "# New\n\n# Old\n"
    );

    let session = registry.borrow().session(id).unwrap().clone();
    visual.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.apply_snapshot(&session, cx);
            view.toggle_mode_for_test(window, cx);
        });
    });
    visual.run_until_parked();
    visual.update(|_, cx| {
        let view = view.read(cx);
        assert_eq!(view.rendered_text_for_test(), "New\nOld");
        assert_eq!(view.rendered_revision_for_test(), 1);
    });
}

fn resource_view(cx: &mut TestAppContext) -> (gpui::Entity<MarkdownView>, gpui::VisualTestContext) {
    view_for_source(cx, "![pixel](pixel.png)\n")
}

fn view_for_source(
    cx: &mut TestAppContext,
    source: &str,
) -> (gpui::Entity<MarkdownView>, gpui::VisualTestContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        cx.set_global(crate::app_shell::config::Settings::default());
        crate::app_shell::theme::apply_dark_theme(cx);
        crate::app_shell::markdown::init(cx);
    });
    let id = DocumentId::from_str("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2a").unwrap();
    let mut registry = DocumentRegistry::new();
    registry
        .open(
            id,
            ResolvedPath::new(CanonicalPath::new("D:/work/README.md").unwrap(), None),
            Rope::from_str(source),
            TextStateId::from_uuid(Uuid::now_v7()).unwrap(),
            std::sync::Arc::from(source.as_bytes()),
            DocumentMetadata::default(),
        )
        .unwrap();
    let session = registry.session(id).unwrap().clone();
    let mut slot = None;
    let window = cx.add_window(|window, cx| {
        let view = cx.new(|cx| {
            MarkdownView::new(
                &session,
                Box::new(|_, _, _| unreachable!("资源回执测试不编辑正文")),
                None,
                MarkdownRenderServices::default(),
                Vec::new(),
                DocumentViewStateBinding::new(
                    ViewKey::Markdown {
                        project_id: ProjectId::from_str("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b")
                            .unwrap(),
                        document_id: id,
                    },
                    EditorViewStateStore::transient(),
                ),
                cx,
            )
        });
        slot = Some(view.clone());
        gpui_component::Root::new(view, window, cx)
    });
    let visual = gpui::VisualTestContext::from_window(window.into(), cx);
    (slot.unwrap(), visual)
}

fn bitmap(red: u8) -> std::sync::Arc<gpui::RenderImage> {
    std::sync::Arc::new(gpui::RenderImage::new([image::Frame::new(
        image::RgbaImage::from_pixel(1, 1, image::Rgba([red, 0, 0, 255])),
    )]))
}

#[gpui::test]
fn stale_resource_receipts_cannot_overwrite_same_revision_retries(cx: &mut TestAppContext) {
    let (view, _) = resource_view(cx);
    let current_image = bitmap(1);
    let stale_image = bitmap(2);
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            let url = "pixel.png";
            let revision = view.revision;
            view.image_tasks
                .insert(url.to_owned(), (2, gpui::Task::ready(())));
            view.complete_image(url, 2, revision, Ok(current_image.clone()), cx);
            view.complete_image(url, 1, revision, Ok(stale_image.clone()), cx);
            assert!(std::sync::Arc::ptr_eq(
                view.images.get(url).unwrap(),
                &current_image
            ));

            let key = [7; 32];
            view.formula_tasks.insert(key, (4, gpui::Task::ready(())));
            view.complete_formula(key, 4, revision, Ok(current_image.clone()), cx);
            view.complete_formula(key, 3, revision, Ok(stale_image), cx);
            assert!(std::sync::Arc::ptr_eq(
                view.formula_images.get(&key).unwrap(),
                &current_image
            ));
        });
    });
}

#[gpui::test]
fn repeated_image_click_does_not_restart_requests(cx: &mut TestAppContext) {
    let (view, mut visual) = resource_view(cx);
    let owner = view.downgrade();
    visual.update(|_, cx| {
        view.update(cx, |view, cx| {
            let url = "https://images.example/a.png";
            view.load_image_on_click(url, &owner, cx);
            let first = view.remote_image_generations[url];
            assert!(view.image_tasks.contains_key(url));
            view.load_image_on_click(url, &owner, cx);
            assert_eq!(
                view.remote_image_generations[url], first,
                "已发起或已加载的图片重复点击不得重启请求"
            );
        });
    });
}

#[gpui::test]
fn markdown_viewport_releases_offscreen_resource_bitmaps(cx: &mut TestAppContext) {
    let (view, mut visual) = resource_view(cx);
    visual.run_until_parked();
    visual.update(|_, cx| {
        view.update(cx, |view, cx| {
            view.images.insert("current.png".to_owned(), bitmap(1));
            view.images.insert("offscreen.png".to_owned(), bitmap(2));
            view.image_blocks
                .insert("current.png".to_owned(), BTreeSet::from([0]));
            view.image_blocks
                .insert("offscreen.png".to_owned(), BTreeSet::from([20]));
            view.image_failures.insert("offscreen.png".to_owned());
            view.image_tasks
                .insert("offscreen.png".to_owned(), (8, gpui::Task::ready(())));
            view.formula_images.insert([1; 32], bitmap(3));
            view.formula_images.insert([2; 32], bitmap(4));
            view.formula_blocks.insert([1; 32], BTreeSet::from([0]));
            view.formula_blocks.insert([2; 32], BTreeSet::from([20]));

            view.schedule_blocks(0..4, cx);

            assert!(view.images.contains_key("current.png"));
            assert!(!view.images.contains_key("offscreen.png"));
            assert!(!view.image_blocks.contains_key("offscreen.png"));
            assert!(!view.image_failures.contains("offscreen.png"));
            assert!(!view.image_tasks.contains_key("offscreen.png"));
            assert!(view.formula_images.contains_key(&[1; 32]));
            assert!(!view.formula_images.contains_key(&[2; 32]));
            assert!(!view.formula_blocks.contains_key(&[2; 32]));
        });
    });
}

#[gpui::test]
fn markdown_source_mode_releases_render_resource_bitmaps(cx: &mut TestAppContext) {
    let (view, mut visual) = resource_view(cx);
    visual.run_until_parked();
    visual.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.images.insert("pixel.png".to_owned(), bitmap(1));
            view.image_blocks
                .insert("pixel.png".to_owned(), BTreeSet::from([0]));
            view.formula_images.insert([1; 32], bitmap(2));
            view.formula_blocks.insert([1; 32], BTreeSet::from([0]));

            view.toggle_mode_for_test(window, cx);

            assert!(view.images.is_empty());
            assert!(view.formula_images.is_empty());
            assert!(view.image_blocks.is_empty());
            assert!(view.formula_blocks.is_empty());
        });
    });
}

#[gpui::test]
fn markdown_source_rejects_independent_language_override(cx: &mut TestAppContext) {
    let (view, mut visual) = resource_view(cx);
    visual.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.toggle_mode_for_test(window, cx);
            view.source_for_test().update(cx, |source, cx| {
                source.apply_language_override(Some(slterm_document::LanguageId::Rust), None, cx);
                assert_eq!(source.language_override(), None);
            });
        });
    });
    visual.refresh().unwrap();
    visual.update(|window, cx| {
        window.draw(cx).clear(cx);
    });
    let source = visual.update(|_, cx| view.read(cx).source_for_test());
    let selector =
        Box::leak(format!("document-language-selector-{}", source.entity_id()).into_boxed_str());
    assert!(visual.debug_bounds(selector).is_none());
}

#[gpui::test]
fn markdown_ctrl_wheel_zooms_and_ctrl_zero_resets(cx: &mut TestAppContext) {
    let (view, mut visual) = resource_view(cx);
    visual.run_until_parked();
    visual.refresh().unwrap();
    visual.update(|window, cx| {
        window.draw(cx).clear(cx);
    });
    let bounds = visual.debug_bounds("markdown-view").unwrap();
    let modifiers = gpui::Modifiers {
        control: true,
        ..gpui::Modifiers::default()
    };
    visual.simulate_event(gpui::ScrollWheelEvent {
        position: bounds.center(),
        delta: gpui::ScrollDelta::Pixels(gpui::point(gpui::px(0.0), gpui::px(24.0))),
        modifiers,
        ..Default::default()
    });
    assert_eq!(view.read_with(&visual, |view, _| view.zoom), 1_100);

    visual.update(|window, cx| {
        view.read(cx).focus_handle(cx).focus(window, cx);
    });
    visual.simulate_keystrokes("ctrl-0");
    assert_eq!(view.read_with(&visual, |view, _| view.zoom), 1_000);

    visual.simulate_event(gpui::ScrollWheelEvent {
        position: bounds.center(),
        delta: gpui::ScrollDelta::Pixels(gpui::point(gpui::px(0.0), gpui::px(-24.0))),
        modifiers,
        ..Default::default()
    });
    assert_eq!(view.read_with(&visual, |view, _| view.zoom), 909);
}

#[gpui::test]
fn markdown_rendered_search_navigates_visible_matches(cx: &mut TestAppContext) {
    let (view, mut visual) = view_for_source(cx, "alpha\n\nbeta alpha\n");
    visual.run_until_parked();
    visual.refresh().unwrap();
    visual.update(|window, cx| {
        window.draw(cx).clear(cx);
        view.read(cx).focus_handle(cx).focus(window, cx);
    });

    visual.simulate_keystrokes("ctrl-f");
    assert!(visual.debug_bounds("markdown-search-input").is_some());
    visual.simulate_input("alpha");
    visual.run_until_parked();
    let search = view.read_with(&visual, |view, cx| {
        let search = view.search.as_ref().unwrap();
        (
            search.input.read(cx).value().to_string(),
            search.matches.len(),
        )
    });
    assert_eq!(search, ("alpha".to_owned(), 2));
    assert_eq!(
        view.read_with(&visual, |view, _| view.reading_top_for_test()),
        (0, 0)
    );

    visual.simulate_keystrokes("enter");
    assert_eq!(
        view.read_with(&visual, |view, _| view.reading_top_for_test()),
        (1, 0)
    );
    visual.simulate_keystrokes("shift-enter");
    assert_eq!(
        view.read_with(&visual, |view, _| view.reading_top_for_test()),
        (0, 0)
    );
    visual.simulate_keystrokes("escape");

    assert!(visual.debug_bounds("markdown-search-input").is_none());
    visual.update(|window, cx| {
        assert_eq!(window.focused(cx), Some(view.read(cx).focus_handle(cx)));
    });
}

#[gpui::test]
fn markdown_revision_change_releases_view_resource_bitmaps(cx: &mut TestAppContext) {
    let (view, _) = resource_view(cx);
    cx.update(|cx| {
        view.update(cx, |view, _| {
            view.images.insert("old.png".to_owned(), bitmap(1));
            view.formula_images.insert([3; 32], bitmap(2));
            let snapshot = DocumentTextSnapshot {
                text: Rope::from_str("new\n"),
                revision: view.revision + 1,
                text_state: TextStateId::from_uuid(Uuid::now_v7()).unwrap(),
                read_only: false,
            };
            view.adopt_snapshot(snapshot, view.location.clone());
            assert!(view.images.is_empty());
            assert!(view.formula_images.is_empty());
        });
    });
}
