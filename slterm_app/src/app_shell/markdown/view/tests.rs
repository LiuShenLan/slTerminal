use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use gpui::{AppContext as _, EntityInputHandler as _, TestAppContext};
use ropey::Rope;
use slterm_document::{DocumentId, DocumentMetadata, DocumentRegistry, TextStateId};
use slterm_path::{CanonicalPath, ResolvedPath};
use slterm_workspace::{ProjectId, ViewKey};
use uuid::Uuid;

use super::{MarkdownRenderServices, MarkdownView, MarkdownViewEvent};
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

/// 等待后台解析完成并经持续绘制把文档投影交给视图。
fn draw_until_document_ready(
    view: &gpui::Entity<MarkdownView>,
    visual: &mut gpui::VisualTestContext,
) {
    visual.run_until_parked();
    visual.refresh().unwrap();
    for _ in 0..20 {
        visual.update(|window, cx| {
            window.draw(cx).clear(cx);
        });
        visual.run_until_parked();
        if view.read_with(visual, |view, _| view.document.is_some()) {
            return;
        }
    }
    panic!("文档投影必须就绪");
}

fn resource_view(cx: &mut TestAppContext) -> (gpui::Entity<MarkdownView>, gpui::VisualTestContext) {
    view_for_source(cx, "![pixel](pixel.png)\n")
}

fn view_for_source(
    cx: &mut TestAppContext,
    source: &str,
) -> (gpui::Entity<MarkdownView>, gpui::VisualTestContext) {
    view_for_source_with_services(cx, source, MarkdownRenderServices::default())
}

fn view_for_source_with_services(
    cx: &mut TestAppContext,
    source: &str,
    services: MarkdownRenderServices,
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
                services,
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
fn remote_image_render_never_fetches_and_click_fetches_once(cx: &mut TestAppContext) {
    use crate::app_shell::markdown::MarkdownResourceError;
    use crate::app_shell::markdown::local_resource::MarkdownImageService;
    use crate::app_shell::markdown::remote_resource::{RemoteImageFetch, RemoteImageResponse};

    struct CountingRemote(Arc<AtomicUsize>);
    impl RemoteImageFetch for CountingRemote {
        fn fetch(&self, _url: &str) -> Result<RemoteImageResponse, MarkdownResourceError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Err(MarkdownResourceError::Read)
        }
    }

    let calls = Arc::new(AtomicUsize::new(0));
    let images =
        MarkdownImageService::default().with_remote_fetch(Arc::new(CountingRemote(calls.clone())));
    let services = MarkdownRenderServices {
        images: Arc::new(std::sync::Mutex::new(images)),
        formulas: None,
    };
    let (view, mut visual) = view_for_source_with_services(
        cx,
        "![remote](https://images.example/a.png)
",
        services,
    );
    visual.run_until_parked();
    assert_eq!(
        calls.load(Ordering::SeqCst),
        0,
        "渲染可见远程图片不得触发网络请求"
    );

    let owner = view.downgrade();
    visual.update(|_, cx| {
        view.update(cx, |view, cx| {
            view.load_image_on_click("https://images.example/a.png", &owner, cx);
        });
    });
    visual.run_until_parked();
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "显式点击必须只发起一次请求"
    );
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

/// 大纲必须是按标题层级缩进的浮层，且不改变正文可用宽度。
#[gpui::test]
fn markdown_outline_is_hierarchical_overlay_without_shrinking_content(cx: &mut TestAppContext) {
    let (view, mut visual) =
        view_for_source(cx, "# Top\n\n## Child\n\n### Grand\n\n## Sibling\n\ntext\n");
    visual.run_until_parked();
    visual.refresh().unwrap();
    // 解析在后台完成；持续绘制直到文档投影就绪。
    for _ in 0..20 {
        visual.update(|window, cx| {
            window.draw(cx).clear(cx);
        });
        visual.run_until_parked();
        if view.read_with(&visual, |view, _| view.document.is_some()) {
            break;
        }
    }
    assert!(
        view.read_with(&visual, |view, _| view.document.is_some()),
        "文档投影必须就绪"
    );

    let content_with_outline = visual
        .debug_bounds("markdown-content")
        .expect("正文区域必须可定位");
    let outline = visual
        .debug_bounds("markdown-outline")
        .expect("大纲必须渲染为浮层");
    assert_eq!(
        outline.origin.x + outline.size.width,
        content_with_outline.origin.x + content_with_outline.size.width,
        "大纲浮层应贴靠正文右侧"
    );

    visual.update(|window, cx| {
        view.update(cx, |view, cx| view.toggle_outline_for_test(cx));
        window.draw(cx).clear(cx);
    });
    let content_without_outline = visual
        .debug_bounds("markdown-content")
        .expect("关闭大纲后正文仍须可定位");
    assert!(
        (f32::from(content_without_outline.size.width)
            - f32::from(content_with_outline.size.width))
        .abs()
            <= 1.0,
        "大纲浮层不得挤占正文宽度"
    );

    visual.update(|window, cx| {
        view.update(cx, |view, cx| view.toggle_outline_for_test(cx));
        window.draw(cx).clear(cx);
    });
    let top = visual
        .debug_bounds("markdown-heading-0")
        .expect("一级标题必须可定位");
    let child = visual
        .debug_bounds("markdown-heading-1")
        .expect("二级标题必须可定位");
    let grand = visual
        .debug_bounds("markdown-heading-2")
        .expect("三级标题必须可定位");
    let sibling = visual
        .debug_bounds("markdown-heading-3")
        .expect("同级标题必须可定位");
    assert!(
        child.origin.x > top.origin.x,
        "二级标题必须比一级标题缩进更深"
    );
    assert!(
        grand.origin.x > child.origin.x,
        "三级标题必须比二级标题缩进更深"
    );
    assert_eq!(sibling.origin.x, child.origin.x, "同级标题必须保持同一缩进");
}

/// 渲染态必须能用键盘切换大纲、滚动和激活链接。
#[gpui::test]
fn markdown_render_mode_is_keyboard_operable(cx: &mut TestAppContext) {
    let source = format!(
        "# Top

[first](https://example.com/a)

## Child

[second](https://example.com/b)

{}",
        (0..80)
            .map(|index| format!(
                "paragraph {index}

"
            ))
            .collect::<String>()
    );
    let (view, mut visual) = view_for_source(cx, &source);
    draw_until_document_ready(&view, &mut visual);
    visual.update(|window, cx| {
        view.read(cx).focus_handle(cx).focus(window, cx);
    });

    visual.update(|window, cx| {
        window.draw(cx).clear(cx);
    });
    assert!(visual.debug_bounds("markdown-outline").is_some());
    visual.simulate_keystrokes("ctrl-shift-o");
    visual.update(|window, cx| {
        window.draw(cx).clear(cx);
    });
    assert!(
        visual.debug_bounds("markdown-outline").is_none(),
        "Ctrl+Shift+O 必须切换大纲"
    );
    visual.simulate_keystrokes("ctrl-shift-o");
    visual.update(|window, cx| {
        window.draw(cx).clear(cx);
    });
    assert!(visual.debug_bounds("markdown-outline").is_some());

    let start = view.read_with(&visual, |view, _| view.reading_top_for_test());
    visual.simulate_keystrokes("down");
    let after_line = view.read_with(&visual, |view, _| view.reading_top_for_test());
    assert!(after_line > start, "方向键必须滚动渲染正文");
    visual.simulate_keystrokes("pagedown");
    let after_page = view.read_with(&visual, |view, _| view.reading_top_for_test());
    assert!(after_page > after_line, "PageDown 必须按页滚动");
    visual.simulate_keystrokes("end");
    let after_end = view.read_with(&visual, |view, _| view.reading_top_for_test());
    assert!(after_end > after_page, "End 必须滚动到底部");
    visual.simulate_keystrokes("home");
    assert_eq!(
        view.read_with(&visual, |view, _| view.reading_top_for_test()),
        (0, 0),
        "Home 必须回到顶部"
    );

    let opened = Rc::new(RefCell::new(Vec::new()));
    let sink = opened.clone();
    let _subscription = cx.update(|cx| {
        cx.subscribe(&view, move |_, event: &MarkdownViewEvent, _| {
            if let MarkdownViewEvent::OpenLink(link) = event {
                sink.borrow_mut().push(link.url().to_owned());
            }
        })
    });
    visual.simulate_keystrokes("alt-down");
    visual.simulate_keystrokes("alt-enter");
    assert_eq!(
        opened.borrow().as_slice(),
        ["https://example.com/a".to_owned()],
        "Alt+↓/Alt+Enter 必须聚焦并激活首个链接"
    );
    visual.simulate_keystrokes("alt-down");
    visual.simulate_keystrokes("alt-enter");
    assert_eq!(
        opened.borrow().as_slice(),
        [
            "https://example.com/a".to_owned(),
            "https://example.com/b".to_owned()
        ],
        "再次 Alt+↓ 必须前进到下一个链接"
    );
    visual.simulate_keystrokes("alt-up");
    visual.simulate_keystrokes("alt-enter");
    assert_eq!(
        opened.borrow().last().map(String::as_str),
        Some("https://example.com/a"),
        "Alt+↑ 必须回到上一个链接"
    );
}

/// 程序化跳转与阅读位置恢复必须把目标块带入渲染调度。
#[gpui::test]
fn markdown_programmatic_jump_and_restore_schedule_target_blocks(cx: &mut TestAppContext) {
    let source = format!(
        "{}[tail](https://example.com/tail)\n\n{}",
        (0..40)
            .map(|index| format!("paragraph {index}\n\n"))
            .collect::<String>(),
        (0..40)
            .map(|index| format!("after {index}\n\n"))
            .collect::<String>()
    );
    let (view, mut visual) = view_for_source(cx, &source);
    draw_until_document_ready(&view, &mut visual);
    visual.update(|window, cx| {
        view.read(cx).focus_handle(cx).focus(window, cx);
    });
    visual.simulate_keystrokes("alt-down");
    let mut jumped = false;
    for _ in 0..20 {
        visual.run_until_parked();
        visual.update(|window, cx| {
            window.draw(cx).clear(cx);
        });
        jumped = view.read_with(&visual, |view, _| {
            let Some(document) = view.document.as_ref() else {
                return false;
            };
            let Some(link) = document.links().first() else {
                return false;
            };
            view.renderer.outcome(link.block()).is_some()
        });
        if jumped {
            break;
        }
    }
    assert!(jumped, "链接跳转必须调度目标块渲染");

    // 恢复阅读位置到尚未渲染的块，重新解析后同样必须调度该块。
    view.update(cx, |view, _| view.set_reading_for_test(40, 0));
    visual.update(|window, cx| {
        view.update(cx, |view, cx| view.toggle_mode_for_test(window, cx));
        view.update(cx, |view, cx| view.toggle_mode_for_test(window, cx));
    });
    let mut restored = false;
    for _ in 0..20 {
        visual.run_until_parked();
        visual.update(|window, cx| {
            window.draw(cx).clear(cx);
        });
        restored = view.read_with(&visual, |view, _| {
            let Some(document) = view.document.as_ref() else {
                return false;
            };
            document
                .blocks()
                .get(40)
                .is_some_and(|block| view.renderer.outcome(block.id()).is_some())
        });
        if restored {
            break;
        }
    }
    assert!(restored, "阅读位置恢复必须调度目标块渲染");
    assert_eq!(
        view.read_with(&visual, |view, _| view.reading_top_for_test()),
        (40, 0),
        "恢复位置必须停在持久化的块"
    );
}
