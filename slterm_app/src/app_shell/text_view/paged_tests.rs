
    use std::cell::RefCell;
    use std::io;
    use std::rc::Rc;
    use std::sync::{Arc, Mutex};

    use gpui::{AppContext as _, Entity, Render, TestAppContext, Window, px};
    use slterm_document::{
        DocumentId, DocumentMetadata, DocumentPageCache, DocumentRegistry, PageIndex,
        PagedLineIndex, PagedSource, SourceByteRange, TextStateId,
    };
    use slterm_path::{CanonicalPath, ResolvedPath};
    use uuid::Uuid;

    use super::{PagedMotion, PagedTextView};
    use crate::app_shell::paged_document::PageReader;

    fn source_for(pages: &[&str]) -> PagedSource {
        let mut offset = 0_u64;
        let mut ranges = Vec::with_capacity(pages.len());
        let mut stats = Vec::with_capacity(pages.len());
        for page in pages {
            let end = offset + page.len() as u64;
            ranges.push(SourceByteRange::new(offset, end).unwrap());
            stats.push((
                page.bytes().filter(|byte| *byte == b'\n').count(),
                page.len(),
                page.ends_with('\n'),
            ));
            offset = end;
        }
        PagedSource::new(
            PageIndex::new(offset, ranges).unwrap(),
            [21; 32],
            PagedLineIndex::from_page_stats(&stats),
        )
    }

    fn make_view(
        cache: Rc<RefCell<DocumentPageCache>>,
        reader: PageReader,
        source: PagedSource,
        cx: &mut gpui::App,
    ) -> Entity<PagedTextView> {
        make_view_with_store(
            cache,
            reader,
            source,
            super::super::view_state::EditorViewStateStore::transient(),
            cx,
        )
    }

    fn make_view_with_store(
        cache: Rc<RefCell<DocumentPageCache>>,
        reader: PageReader,
        source: PagedSource,
        view_states: super::super::view_state::EditorViewStateStore,
        cx: &mut gpui::App,
    ) -> Entity<PagedTextView> {
        let id: DocumentId = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0f2a".parse().unwrap();
        let mut registry = DocumentRegistry::new();
        registry
            .open(
                id,
                ResolvedPath::new(CanonicalPath::new("D:/work/paged.txt").unwrap(), None),
                source,
                TextStateId::from_uuid(Uuid::now_v7()).unwrap(),
                Arc::<[u8]>::default(),
                DocumentMetadata {
                    read_only: true,
                    ..DocumentMetadata::default()
                },
            )
            .unwrap();
        let request = registry.text_request(id, 0).unwrap();
        cx.new(|cx| {
            PagedTextView::new(
                registry.session(id).unwrap(),
                cache,
                reader,
                request,
                slterm_workspace::ViewKey::Editor {
                    project_id: "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b".parse().unwrap(),
                    document_id: id,
                },
                view_states,
                cx,
            )
        })
    }

    struct Host {
        view: Entity<PagedTextView>,
    }

    impl Host {
        fn new(
            cache: Rc<RefCell<DocumentPageCache>>,
            reader: PageReader,
            cx: &mut gpui::Context<'_, Self>,
        ) -> Self {
            Self::with_source(cache, reader, source_for(&["hello\n", "world\n"]), cx)
        }

        fn with_source(
            cache: Rc<RefCell<DocumentPageCache>>,
            reader: PageReader,
            source: PagedSource,
            cx: &mut gpui::Context<'_, Self>,
        ) -> Self {
            let view = make_view(cache, reader, source, cx);
            Self { view }
        }
    }

    impl Render for Host {
        fn render(
            &mut self,
            _: &mut Window,
            _: &mut gpui::Context<'_, Self>,
        ) -> impl gpui::IntoElement {
            self.view.clone()
        }
    }

    #[gpui::test]
    fn paged_view_restores_scroll_state_without_transients(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::app_shell::theme::apply_dark_theme(cx);
        });
        let id: DocumentId = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0f2a".parse().unwrap();
        let key = slterm_workspace::ViewKey::Editor {
            project_id: "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b".parse().unwrap(),
            document_id: id,
        };
        let states = super::super::view_state::EditorViewStateStore::transient();
        states.restore(
            key.clone(),
            super::super::view_state::EditorViewState::new(3, 4).with_wrap(true),
        );
        let cache = Rc::new(RefCell::new(DocumentPageCache::new()));
        let reader: PageReader = Arc::new(|_| Ok("0\n1\n2\n3\n4\n".to_owned()));
        let view = cx.update(|cx| {
            make_view_with_store(
                cache,
                reader,
                source_for(&["0\n1\n2\n3\n4\n"]),
                states.clone(),
                cx,
            )
        });
        view.update(cx, |view, _| {
            assert_eq!(view.scroll_state_for_test(), (3, 4, 0..0, false));
            view.persist_view_state();
        });
        assert!(states.state(&key).wrap());
        assert_eq!(
            states.state(&key).scroll_unit(),
            super::super::view_state::ScrollUnit::LogicalLine
        );
        assert_eq!(states.state(&key).scroll_line(), 3);
        let persisted = serde_json::to_value(states.state(&key)).unwrap();
        let mut persisted_keys = persisted
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        persisted_keys.sort_unstable();
        assert_eq!(
            persisted_keys,
            ["scroll_column", "scroll_line", "scroll_unit", "wrap"],
            "持久 DTO 不得携带选区、光标或待续动作"
        );

        // 渲染必须真正采用恢复行，而不是只把字段存回内存。
        let window = cx.add_window(|_, _| Host { view: view.clone() });
        let mut visual = gpui::VisualTestContext::from_window(window.into(), cx);
        visual.run_until_parked();
        for _ in 0..2 {
            visual.refresh().unwrap();
            visual.update(|window, cx| {
                window.draw(cx).clear(cx);
            });
            visual.run_until_parked();
        }
        let painted = view.read_with(cx, |view, _| view.painted_lines_for_test());
        assert_eq!(
            painted.first().map(|row| row.0),
            Some(3),
            "绘制首行必须来自恢复位置: {painted:?}"
        );
    }

    /// Paged 没有全文换行索引：显示行记录退回顶部，不把显示行当逻辑行。
    #[gpui::test]
    fn paged_rejects_display_row_scroll_units(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::app_shell::theme::apply_dark_theme(cx);
        });
        let id: DocumentId = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0f2a".parse().unwrap();
        let key = slterm_workspace::ViewKey::Editor {
            project_id: "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b".parse().unwrap(),
            document_id: id,
        };
        let states = super::super::view_state::EditorViewStateStore::transient();
        states.restore(
            key,
            super::super::view_state::EditorViewState::new(7, 0)
                .with_wrap(true)
                .with_scroll_unit(super::super::view_state::ScrollUnit::DisplayRow),
        );
        let cache = Rc::new(RefCell::new(DocumentPageCache::new()));
        let reader: PageReader = Arc::new(|_| Ok("unused
".to_owned()));
        let view = cx.update(|cx| {
            make_view_with_store(
                cache,
                reader,
                source_for(&["0
1
2
3
4
"]),
                states,
                cx,
            )
        });
        view.update(cx, |view, _| {
            assert_eq!(view.scroll_state_for_test().0, 0);
        });
    }

    #[gpui::test]
    fn extreme_restored_scroll_does_not_overflow_paged_hit_testing(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::app_shell::theme::apply_dark_theme(cx);
        });
        let id: DocumentId = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0f2a".parse().unwrap();
        let states = super::super::view_state::EditorViewStateStore::transient();
        states.restore(
            slterm_workspace::ViewKey::Editor {
                project_id: "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b".parse().unwrap(),
                document_id: id,
            },
            super::super::view_state::EditorViewState::new(0, usize::MAX),
        );
        let cache = Rc::new(RefCell::new(DocumentPageCache::new()));
        cache
            .borrow_mut()
            .insert(slterm_document::PageKey::new(id, 0, 0), "abc".to_owned());
        let reader: PageReader = Arc::new(|_| Ok("abc".to_owned()));
        let view =
            cx.update(|cx| make_view_with_store(cache, reader, source_for(&["abc"]), states, cx));
        view.update(cx, |view, _| {
            // 恢复列必须先夹到源字节换算的显示列上界；否则绘制与命中测试
            // 拿到 usize::MAX 只是碰巧不溢出。
            assert_eq!(view.scroll_state_for_test().0, 0);
            assert_eq!(
                view.scroll_state_for_test().1,
                3 * 4,
                "恢复列必须夹到显示列上界"
            );
            *view.layout.borrow_mut() = Some(super::super::layout::LayoutMetrics {
                bounds: gpui::Bounds::default(),
                cell_width: px(8.0),
                line_height: px(20.0),
            });
            let _ = view.byte_for_point(gpui::point(px(0.0), px(0.0)));
        });
    }

    #[gpui::test]
    fn paged_view_loads_missing_pages_into_shared_cache(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::app_shell::theme::apply_dark_theme(cx);
        });
        let cache = Rc::new(RefCell::new(DocumentPageCache::new()));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let recorded = requests.clone();
        let reader: PageReader = Arc::new(move |page| {
            recorded.lock().unwrap().push(page);
            Ok(match page {
                0 => "hello\n".to_owned(),
                1 => "world\n".to_owned(),
                _ => return Err(io::Error::new(io::ErrorKind::InvalidInput, "页号越界")),
            })
        });
        let window = cx.add_window(|_, cx| Host::new(cache.clone(), reader, cx));
        let mut visual = gpui::VisualTestContext::from_window(window.into(), cx);
        visual.run_until_parked();
        visual.refresh().unwrap();
        visual.update(|window, cx| {
            window.draw(cx).clear(cx);
        });

        let painted = window
            .update(cx, |host, _, cx| host.view.read(cx).painted_lines.clone())
            .unwrap();
        assert_eq!(painted.borrow().len(), 3);
        visual.run_until_parked();
        assert_eq!(cache.borrow().len(), 2);

        visual.refresh().unwrap();
        visual.update(|window, cx| {
            window.draw(cx).clear(cx);
        });
        assert_eq!(
            painted.borrow().as_slice(),
            &[
                (0, Some("hello".to_owned())),
                (1, Some("world".to_owned())),
                (2, Some(String::new())),
            ]
        );
        let mut requests = requests.lock().unwrap().clone();
        requests.sort_unstable();
        assert_eq!(requests, vec![0, 1]);
        let tasks = window
            .update(cx, |host, _, cx| host.view.read(cx).tasks.len())
            .unwrap();
        assert_eq!(tasks, 0);
    }

    #[gpui::test]
    fn paged_view_does_not_retry_failed_pages_on_every_frame(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::app_shell::theme::apply_dark_theme(cx);
        });
        let cache = Rc::new(RefCell::new(DocumentPageCache::new()));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let recorded = requests.clone();
        let reader: PageReader = Arc::new(move |page| {
            recorded.lock().unwrap().push(page);
            Err(io::Error::new(io::ErrorKind::PermissionDenied, "读取失败"))
        });
        let window = cx.add_window(|_, cx| Host::new(cache.clone(), reader, cx));
        let mut visual = gpui::VisualTestContext::from_window(window.into(), cx);
        visual.run_until_parked();
        visual.refresh().unwrap();
        visual.update(|window, cx| {
            window.draw(cx).clear(cx);
        });
        visual.run_until_parked();
        for _ in 0..2 {
            visual.refresh().unwrap();
            visual.update(|window, cx| {
                window.draw(cx).clear(cx);
            });
        }

        let mut requests = requests.lock().unwrap().clone();
        requests.sort_unstable();
        assert_eq!(requests, vec![0, 1]);
        assert!(cache.borrow().is_empty());
        let failed = window
            .update(cx, |host, _, cx| host.view.read(cx).failed_pages.len())
            .unwrap();
        assert_eq!(failed, 2);
    }

    /// 失败页不得逐帧自动重试，但必须给出可见提示与显式重试入口。
    #[gpui::test]
    fn paged_view_explicit_retry_reruns_failed_pages(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::app_shell::theme::apply_dark_theme(cx);
        });
        let cache = Rc::new(RefCell::new(DocumentPageCache::new()));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let recorded = requests.clone();
        let reader: PageReader = Arc::new(move |page| {
            recorded.lock().unwrap().push(page);
            Err(io::Error::new(io::ErrorKind::PermissionDenied, "读取失败"))
        });
        let window = cx.add_window(|_, cx| Host::new(cache.clone(), reader, cx));
        let mut visual = gpui::VisualTestContext::from_window(window.into(), cx);
        visual.run_until_parked();
        visual.refresh().unwrap();
        visual.update(|window, cx| {
            window.draw(cx).clear(cx);
        });
        visual.run_until_parked();
        visual.update(|window, cx| {
            window.draw(cx).clear(cx);
        });
        let banner = visual
            .debug_bounds("paged-page-retry")
            .expect("失败页必须显示重试入口");
        visual.simulate_click(banner.center(), gpui::Modifiers::default());
        visual.run_until_parked();
        visual.update(|window, cx| {
            window.draw(cx).clear(cx);
        });
        visual.run_until_parked();

        let mut requests = requests.lock().unwrap().clone();
        requests.sort_unstable();
        assert_eq!(requests, vec![0, 0, 1, 1], "显式重试必须重新读取失败页");
        let failed = window
            .update(cx, |host, _, cx| host.view.read(cx).failed_pages.len())
            .unwrap();
        assert_eq!(failed, 2, "再次失败后仍保留可见失败态");
    }

    #[gpui::test]
    fn paged_view_scrolls_to_request_a_later_page(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::app_shell::theme::apply_dark_theme(cx);
        });
        let first = (0..100).map(|line| format!("{line}\n")).collect::<String>();
        let pages = Arc::new(vec![first, "last\n".to_owned()]);
        let page_refs = pages.iter().map(String::as_str).collect::<Vec<_>>();
        let source = source_for(&page_refs);
        let cache = Rc::new(RefCell::new(DocumentPageCache::new()));
        let reader_pages = pages.clone();
        let reader: PageReader = Arc::new(move |page| {
            reader_pages
                .get(page)
                .cloned()
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "页号越界"))
        });
        let window = cx.add_window(|_, cx| Host::with_source(cache.clone(), reader, source, cx));
        window
            .update(cx, |_, window, _| {
                window.resize(gpui::size(px(160.0), px(40.0)))
            })
            .unwrap();
        let mut visual = gpui::VisualTestContext::from_window(window.into(), cx);
        visual.run_until_parked();
        visual.refresh().unwrap();
        visual.update(|window, cx| {
            window.draw(cx).clear(cx);
        });
        visual.run_until_parked();
        let id: DocumentId = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0f2a".parse().unwrap();
        assert!(
            cache
                .borrow_mut()
                .get(&slterm_document::PageKey::new(id, 0, 0))
                .is_some()
        );
        assert!(
            cache
                .borrow_mut()
                .get(&slterm_document::PageKey::new(id, 0, 1))
                .is_none()
        );

        let metrics = window
            .update(cx, |host, _, cx| *host.view.read(cx).layout.borrow())
            .unwrap()
            .unwrap();
        visual.simulate_event(gpui::ScrollWheelEvent {
            position: metrics.bounds.center(),
            delta: gpui::ScrollDelta::Lines(gpui::point(0.0, -100.0)),
            ..Default::default()
        });
        visual.refresh().unwrap();
        visual.update(|window, cx| {
            window.draw(cx).clear(cx);
        });
        visual.run_until_parked();
        assert!(
            cache
                .borrow_mut()
                .get(&slterm_document::PageKey::new(id, 0, 1))
                .is_some()
        );
    }

    #[gpui::test]
    fn paged_view_deduplicates_inflight_page_requests(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::app_shell::theme::apply_dark_theme(cx);
        });
        let cache = Rc::new(RefCell::new(DocumentPageCache::new()));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let recorded = requests.clone();
        let reader: PageReader = Arc::new(move |page| {
            recorded.lock().unwrap().push(page);
            Ok("only\n".to_owned())
        });
        let window = cx.add_window(|_, cx| {
            Host::with_source(cache.clone(), reader, source_for(&["only\n"]), cx)
        });
        window
            .update(cx, |host, _, cx| {
                PagedTextView::request_pages(&host.view, vec![0], cx);
                PagedTextView::request_pages(&host.view, vec![0], cx);
            })
            .unwrap();
        let visual = gpui::VisualTestContext::from_window(window.into(), cx);
        visual.run_until_parked();
        assert_eq!(requests.lock().unwrap().as_slice(), &[0]);
    }

    #[gpui::test]
    fn destroying_paged_view_cancels_requests_and_ignores_late_results(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::app_shell::theme::apply_dark_theme(cx);
        });
        let cache = Rc::new(RefCell::new(DocumentPageCache::new()));
        let reader: PageReader = Arc::new(|_| Ok("late\n".to_owned()));
        let window = cx.add_window(|_, cx| {
            Host::with_source(cache.clone(), reader, source_for(&["only\n"]), cx)
        });
        let weak = window
            .update(cx, |host, _, cx| {
                let weak = host.view.downgrade();
                PagedTextView::request_pages(&host.view, vec![0], cx);
                weak
            })
            .unwrap();
        window
            .update(cx, |_, window, _| window.remove_window())
            .unwrap();
        let visual = gpui::VisualTestContext::from_window(window.into(), cx);
        visual.run_until_parked();

        assert!(weak.upgrade().is_none(), "在途任务不得强持已销毁视图");
        assert!(cache.borrow().is_empty(), "迟到回执不得进入共享缓存");
    }

    #[gpui::test]
    fn closing_paged_view_cancels_requests_and_ignores_late_results(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::app_shell::theme::apply_dark_theme(cx);
        });
        let cache = Rc::new(RefCell::new(DocumentPageCache::new()));
        let reader: PageReader = Arc::new(|_| Ok("late\n".to_owned()));
        let view = cx.update(|cx| make_view(cache.clone(), reader, source_for(&["only\n"]), cx));
        cx.update(|cx| PagedTextView::request_pages(&view, vec![0], cx));
        view.update(cx, |view, _| view.close());
        cx.run_until_parked();

        assert!(cache.borrow().is_empty());
        cx.update(|cx| {
            assert!(view.read(cx).tasks.is_empty());
            assert!(view.read(cx).pending_pages.is_empty());
        });
    }

    #[gpui::test]
    fn paged_copy_reads_selected_uncached_pages_without_filling_view_cache(
        cx: &mut TestAppContext,
    ) {
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::app_shell::theme::apply_dark_theme(cx);
        });
        let cache = Rc::new(RefCell::new(DocumentPageCache::new()));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let recorded = requests.clone();
        let reader: PageReader = Arc::new(move |page| {
            recorded.lock().unwrap().push(page);
            Ok(match page {
                0 => "ab".to_owned(),
                1 => "cd".to_owned(),
                2 => "ef\n".to_owned(),
                _ => return Err(io::Error::new(io::ErrorKind::InvalidInput, "页号越界")),
            })
        });
        let view =
            cx.update(|cx| make_view(cache.clone(), reader, source_for(&["ab", "cd", "ef\n"]), cx));
        view.update(cx, |view, cx| {
            view.selection = super::super::source::TextSelection { anchor: 1, head: 5 };
            view.copy(cx);
        });
        cx.run_until_parked();
        cx.update(|cx| {
            assert_eq!(
                cx.read_from_clipboard().and_then(|item| item.text()),
                Some("bcde".to_owned())
            );
        });
        assert!(cache.borrow().is_empty());
        assert_eq!(requests.lock().unwrap().as_slice(), &[0, 1, 2]);

        view.update(cx, |view, cx| {
            view.selection = super::super::source::TextSelection { anchor: 2, head: 2 };
            view.copy(cx);
            cx.write_to_clipboard(gpui::ClipboardItem::new_string("sentinel".to_owned()));
        });
        cx.run_until_parked();
        cx.update(|cx| {
            assert_eq!(
                cx.read_from_clipboard().and_then(|item| item.text()),
                Some("sentinel".to_owned())
            );
        });
        view.update(cx, |view, cx| {
            view.select_all(cx);
            view.copy(cx);
        });
        cx.run_until_parked();
        cx.update(|cx| {
            assert_eq!(
                cx.read_from_clipboard().and_then(|item| item.text()),
                Some("abcdef\n".to_owned())
            );
        });
    }

    #[gpui::test]
    fn paged_copy_rejects_stale_results_and_read_failures(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::app_shell::theme::apply_dark_theme(cx);
        });
        let cache = Rc::new(RefCell::new(DocumentPageCache::new()));
        let reader: PageReader = Arc::new(|_| Ok("ab".to_owned()));
        let view = cx.update(|cx| make_view(cache, reader, source_for(&["ab"]), cx));
        view.update(cx, |view, cx| {
            cx.write_to_clipboard(gpui::ClipboardItem::new_string("sentinel".to_owned()));
            view.copy_generation = 7;
            view.apply_copy_result(6, Ok("old".to_owned()), cx);
            view.apply_copy_result(7, Err(slterm_document::SearchError::ReadFailed), cx);
        });
        cx.update(|cx| {
            assert_eq!(
                cx.read_from_clipboard().and_then(|item| item.text()),
                Some("sentinel".to_owned())
            );
        });
        view.update(cx, |view, cx| {
            view.apply_copy_result(7, Ok("new".to_owned()), cx);
        });
        cx.update(|cx| {
            assert_eq!(
                cx.read_from_clipboard().and_then(|item| item.text()),
                Some("new".to_owned())
            );
        });
    }

    #[gpui::test]
    fn paged_mouse_drag_selects_across_cached_pages(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::app_shell::theme::apply_dark_theme(cx);
        });
        let cache = Rc::new(RefCell::new(DocumentPageCache::new()));
        let reader: PageReader = Arc::new(|page| {
            Ok(match page {
                0 => "ab\n".to_owned(),
                1 => "cd\n".to_owned(),
                _ => return Err(io::Error::new(io::ErrorKind::InvalidInput, "页号越界")),
            })
        });
        let window = cx.add_window(|_, cx| {
            Host::with_source(cache, reader, source_for(&["ab\n", "cd\n"]), cx)
        });
        let mut visual = gpui::VisualTestContext::from_window(window.into(), cx);
        visual.run_until_parked();
        visual.refresh().unwrap();
        visual.update(|window, cx| {
            window.draw(cx).clear(cx);
        });
        visual.run_until_parked();
        visual.refresh().unwrap();
        visual.update(|window, cx| {
            window.draw(cx).clear(cx);
        });
        let metrics = window
            .update(cx, |host, _, cx| *host.view.read(cx).layout.borrow())
            .unwrap()
            .unwrap();
        visual.simulate_mouse_down(
            gpui::point(
                metrics.bounds.origin.x + metrics.cell_width,
                metrics.bounds.origin.y,
            ),
            gpui::MouseButton::Left,
            gpui::Modifiers::default(),
        );
        visual.simulate_mouse_move(
            gpui::point(
                metrics.bounds.origin.x + metrics.cell_width * 2.0,
                metrics.bounds.origin.y + metrics.line_height,
            ),
            gpui::MouseButton::Left,
            gpui::Modifiers::default(),
        );
        visual.simulate_mouse_up(
            gpui::point(
                metrics.bounds.origin.x + metrics.cell_width * 2.0,
                metrics.bounds.origin.y + metrics.line_height,
            ),
            gpui::MouseButton::Left,
            gpui::Modifiers::default(),
        );
        window
            .update(cx, |host, _, cx| {
                assert_eq!(host.view.read(cx).selection.range(), 1..5);
            })
            .unwrap();
    }

    #[gpui::test]
    fn paged_keyboard_navigation_preserves_unicode_columns_and_expands_selection(
        cx: &mut TestAppContext,
    ) {
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::app_shell::theme::apply_dark_theme(cx);
            crate::app_shell::text_view::init(cx);
        });
        let cache = Rc::new(RefCell::new(DocumentPageCache::new()));
        let reader: PageReader = Arc::new(|page| {
            Ok(match page {
                0 => "中A\nx\n中😀".to_owned(),
                _ => return Err(io::Error::new(io::ErrorKind::InvalidInput, "页号越界")),
            })
        });
        let window = cx.add_window(|_, cx| {
            Host::with_source(cache, reader, source_for(&["中A\nx\n中😀"]), cx)
        });
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
        visual.run_until_parked();

        visual.simulate_keystrokes("right");
        visual.simulate_keystrokes("down");
        visual.simulate_keystrokes("down");
        window
            .update(cx, |host, _, cx| {
                let view = host.view.read(cx);
                assert_eq!(view.selection.range(), 10..10);
            })
            .unwrap();

        visual.simulate_keystrokes("shift-home");
        window
            .update(cx, |host, _, cx| {
                let view = host.view.read(cx);
                assert_eq!(view.selection.range(), 7..10);
                assert!(view.selection.anchor > view.selection.head);
            })
            .unwrap();
        visual.simulate_keystrokes("end");
        visual.simulate_keystrokes("left");
        window
            .update(cx, |host, _, cx| {
                assert_eq!(host.view.read(cx).selection.range(), 10..10);
            })
            .unwrap();
    }

    #[gpui::test]
    fn paged_keyboard_navigation_resumes_after_loading_the_target_page(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::app_shell::theme::apply_dark_theme(cx);
            crate::app_shell::text_view::init(cx);
        });
        let cache = Rc::new(RefCell::new(DocumentPageCache::new()));
        let id: DocumentId = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0f2a".parse().unwrap();
        cache
            .borrow_mut()
            .insert(slterm_document::PageKey::new(id, 0, 0), "a\n".to_owned());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let recorded = requests.clone();
        let reader: PageReader = Arc::new(move |page| {
            recorded.lock().unwrap().push(page);
            Ok(match page {
                0 => "a\n".to_owned(),
                1 => "b\n".to_owned(),
                _ => return Err(io::Error::new(io::ErrorKind::InvalidInput, "页号越界")),
            })
        });
        let view =
            cx.update(|cx| make_view(cache.clone(), reader, source_for(&["a\n", "b\n"]), cx));
        view.update(cx, |view, cx| {
            let mut metrics = super::super::layout::LayoutMetrics {
                bounds: gpui::Bounds::default(),
                cell_width: px(8.0),
                line_height: px(20.0),
            };
            metrics.bounds.size = gpui::size(px(80.0), px(20.0));
            *view.layout.borrow_mut() = Some(metrics);
            view.start_motion(
                PagedMotion::Vertical {
                    down: true,
                    select: true,
                },
                cx,
            );
        });
        assert!(
            cache
                .borrow_mut()
                .get(&slterm_document::PageKey::new(id, 0, 1))
                .is_none()
        );

        cx.run_until_parked();

        assert!(requests.lock().unwrap().contains(&1));
        view.update(cx, |view, _| {
            assert_eq!(view.selection.range(), 0..2);
            assert_eq!(view.scroll_line, 1);
        });
    }

    #[gpui::test]
    fn paged_reader_relocation_restarts_pending_motion_on_the_new_reader(
        cx: &mut TestAppContext,
    ) {
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::app_shell::theme::apply_dark_theme(cx);
            crate::app_shell::text_view::init(cx);
        });
        let cache = Rc::new(RefCell::new(DocumentPageCache::new()));
        let id: DocumentId = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0f2a".parse().unwrap();
        cache
            .borrow_mut()
            .insert(slterm_document::PageKey::new(id, 0, 0), "a\n".to_owned());
        let old_reader: PageReader = Arc::new(|_| Ok("a\n".to_owned()));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let recorded = requests.clone();
        let new_reader: PageReader = Arc::new(move |page| {
            recorded.lock().unwrap().push(page);
            Ok("b\n".to_owned())
        });
        let view = cx.update(|cx| {
            make_view(cache.clone(), old_reader, source_for(&["a\n", "b\n"]), cx)
        });
        view.update(cx, |view, cx| {
            view.start_motion(
                PagedMotion::Vertical {
                    down: true,
                    select: true,
                },
                cx,
            );
            assert!(view.pending_motion.is_some());
            view.relocate_reader(new_reader, cx);
        });

        cx.run_until_parked();

        assert!(requests.lock().unwrap().contains(&1));
        view.update(cx, |view, _| {
            assert_eq!(view.selection.range(), 0..2);
            assert!(view.pending_motion.is_none());
        });
    }

    #[gpui::test]
    fn horizontal_boundary_movement_resets_paged_vertical_column(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::app_shell::theme::apply_dark_theme(cx);
        });
        let cache = Rc::new(RefCell::new(DocumentPageCache::new()));
        let id: DocumentId = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0f2a".parse().unwrap();
        cache.borrow_mut().insert(
            slterm_document::PageKey::new(id, 0, 0),
            "abcdef\nx".to_owned(),
        );
        let reader: PageReader = Arc::new(|_| Ok("abcdef\nx".to_owned()));
        let view = cx.update(|cx| make_view(cache, reader, source_for(&["abcdef\nx"]), cx));
        view.update(cx, |view, cx| {
            for motion in [
                PagedMotion::LineBoundary {
                    end: true,
                    select: false,
                },
                PagedMotion::Vertical {
                    down: true,
                    select: false,
                },
                PagedMotion::Horizontal {
                    left: false,
                    select: false,
                },
                PagedMotion::Vertical {
                    down: false,
                    select: false,
                },
            ] {
                view.start_motion(motion, cx);
            }
            assert_eq!(view.selection.range(), 1..1);
            assert_eq!(view.preferred_column, Some(1));
        });
    }

    #[gpui::test]
    fn paged_horizontal_navigation_resumes_after_loading_the_next_page(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::app_shell::theme::apply_dark_theme(cx);
        });
        let cache = Rc::new(RefCell::new(DocumentPageCache::new()));
        let id: DocumentId = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0f2a".parse().unwrap();
        cache
            .borrow_mut()
            .insert(slterm_document::PageKey::new(id, 0, 0), "a".to_owned());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let recorded = requests.clone();
        let reader: PageReader = Arc::new(move |page| {
            recorded.lock().unwrap().push(page);
            Ok(match page {
                0 => "a".to_owned(),
                1 => "b".to_owned(),
                _ => return Err(io::Error::new(io::ErrorKind::InvalidInput, "页号越界")),
            })
        });
        let view = cx.update(|cx| make_view(cache, reader, source_for(&["a", "b"]), cx));
        view.update(cx, |view, cx| {
            view.start_motion(
                PagedMotion::Horizontal {
                    left: false,
                    select: true,
                },
                cx,
            );
        });
        cx.run_until_parked();

        assert!(requests.lock().unwrap().contains(&1));
        view.update(cx, |view, _| {
            assert_eq!(view.selection.range(), 0..1);
        });

        let reverse_cache = Rc::new(RefCell::new(DocumentPageCache::new()));
        reverse_cache
            .borrow_mut()
            .insert(slterm_document::PageKey::new(id, 0, 1), "b".to_owned());
        let reverse_reader: PageReader = Arc::new(|page| {
            Ok(match page {
                0 => "a".to_owned(),
                1 => "b".to_owned(),
                _ => return Err(io::Error::new(io::ErrorKind::InvalidInput, "页号越界")),
            })
        });
        let reverse =
            cx.update(|cx| make_view(reverse_cache, reverse_reader, source_for(&["a", "b"]), cx));
        reverse.update(cx, |view, cx| {
            view.selection = super::super::source::TextSelection { anchor: 1, head: 1 };
            view.anchor_line = 0;
            view.head_line = 0;
            view.anchor_column = Some(1);
            view.head_column = Some(1);
            view.start_motion(
                PagedMotion::Horizontal {
                    left: true,
                    select: true,
                },
                cx,
            );
        });
        cx.run_until_parked();
        reverse.update(cx, |view, _| {
            assert_eq!(view.selection.anchor, 1);
            assert_eq!(view.selection.head, 0);
            assert_eq!(view.selection.range(), 0..1);
        });
    }

    #[gpui::test]
    fn failed_paged_navigation_does_not_resume_after_later_cache_fill(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::app_shell::theme::apply_dark_theme(cx);
        });
        let cache = Rc::new(RefCell::new(DocumentPageCache::new()));
        let id: DocumentId = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0f2a".parse().unwrap();
        cache
            .borrow_mut()
            .insert(slterm_document::PageKey::new(id, 0, 0), "a\n".to_owned());
        let reader: PageReader = Arc::new(|_| Err(io::Error::other("注入失败")));
        let view =
            cx.update(|cx| make_view(cache.clone(), reader, source_for(&["a\n", "b\n"]), cx));
        view.update(cx, |view, cx| {
            view.start_motion(
                PagedMotion::Vertical {
                    down: true,
                    select: true,
                },
                cx,
            );
        });
        cx.run_until_parked();
        view.update(cx, |view, _| {
            assert!(view.failed_pages.contains(&1));
            assert!(view.pending_motion.is_none());
            assert_eq!(view.selection.range(), 0..0);
        });

        cache
            .borrow_mut()
            .insert(slterm_document::PageKey::new(id, 0, 1), "b\n".to_owned());
        view.update(cx, |view, cx| {
            view.apply_page_result(0, 0, [21; 32], Ok("a\n".to_owned()), cx);
            assert_eq!(view.selection.range(), 0..0);
        });
    }

    #[gpui::test]
    fn collapsing_select_all_to_the_end_reveals_the_real_column(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::app_shell::theme::apply_dark_theme(cx);
        });
        let cache = Rc::new(RefCell::new(DocumentPageCache::new()));
        let id: DocumentId = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0f2a".parse().unwrap();
        cache
            .borrow_mut()
            .insert(slterm_document::PageKey::new(id, 0, 0), "abcdef".to_owned());
        let reader: PageReader = Arc::new(|_| Ok("abcdef".to_owned()));
        let view = cx.update(|cx| make_view(cache, reader, source_for(&["abcdef"]), cx));
        view.update(cx, |view, cx| {
            let mut metrics = super::super::layout::LayoutMetrics {
                bounds: gpui::Bounds::default(),
                cell_width: px(8.0),
                line_height: px(20.0),
            };
            metrics.bounds.size = gpui::size(px(24.0), px(20.0));
            *view.layout.borrow_mut() = Some(metrics);
            view.select_all(cx);
            view.start_motion(
                PagedMotion::Horizontal {
                    left: false,
                    select: false,
                },
                cx,
            );

            assert_eq!(view.selection.range(), 6..6);
            assert_eq!(view.head_column, Some(6));
            assert_eq!(view.scroll_column, 3);
        });
    }

    #[gpui::test]
    fn paged_view_rejects_a_mismatched_source_generation(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::app_shell::theme::apply_dark_theme(cx);
        });
        let cache = Rc::new(RefCell::new(DocumentPageCache::new()));
        let reader: PageReader = Arc::new(|_| Ok("unused\n".to_owned()));
        let window = cx.add_window(|_, cx| Host::new(cache.clone(), reader, cx));
        window
            .update(cx, |host, _, cx| {
                host.view.update(cx, |view, cx| {
                    view.pending_pages.insert(0);
                    view.apply_page_result(0, 1, [21; 32], Ok("stale\n".to_owned()), cx);
                    assert!(view.pending_pages.contains(&0));
                });
            })
            .unwrap();
        assert!(cache.borrow().is_empty());
    }
