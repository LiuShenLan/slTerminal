#[gpui::test]
fn destroying_editor_cancels_pending_wrap_build(cx: &mut TestAppContext) {
    let host = cx.update(|cx| cx.new(|cx| Host::new("abc", false, cx)));
    let probe = Rc::new(());
    let captured = probe.clone();
    host.update(cx, |host, cx| {
        host.view.update(cx, |view, cx| {
            let task = gpui::App::spawn(cx, async move |_| {
                let _captured = captured;
                std::future::pending::<()>().await;
            });
            *view.wrap_build.borrow_mut() = Some((view.text_state, 1, task));
        });
    });
    assert_eq!(Rc::strong_count(&probe), 2);
    let (view, slot) = host.read_with(cx, |host, app| {
        let view = host.view.clone();
        let slot = view.read(app).wrap_build.clone();
        (view, slot)
    });

    drop(host);
    drop(view);
    cx.update(|_| {});
    cx.run_until_parked();

    assert!(slot.borrow().is_none(), "视图 Drop 必须先清空任务槽");
    assert_eq!(Rc::strong_count(&probe), 1, "Drop 必须打破任务槽自持环");
}

#[gpui::test]
fn soft_wrap_logical_scroll_is_revealed_after_index_build(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        crate::app_shell::theme::apply_dark_theme(cx);
        cx.set_global(crate::app_shell::config::Settings::default());
        super::super::init(cx);
    });
    let states = super::super::view_state::EditorViewStateStore::transient();
    let key = ViewKey::Editor {
        project_id: "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b".parse().unwrap(),
        document_id: "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0f2a".parse().unwrap(),
    };
    states.restore(
        key.clone(),
        super::super::view_state::EditorViewState::new(2, 0).with_wrap(true),
    );
    let handle = cx
        .open_window(gpui::size(px(480.0), px(160.0)), |_, cx| {
            Host::configured_with_store("0\n1\n2\n3\n", false, None, states.clone(), cx)
        });
    let mut visual = gpui::VisualTestContext::from_window(handle.into(), cx);
    let view = handle
        .update(cx, |host, _, _| host.view.clone())
        .unwrap();
    let before = view.read_with(cx, |view, _| {
        (view.scroll_state_for_test().0, view.wrap_reveal_line)
    });
    assert_eq!(before.0, 0, "索引未就绪不得把逻辑行当显示行使用");
    assert_eq!(before.1, Some(2), "逻辑行必须等待索引完成后揭示");

    visual.run_until_parked();
    visual.refresh().unwrap();
    visual.update(|window, cx| {
        window.draw(cx).clear(cx);
    });
    visual.run_until_parked();
    let after = view.read_with(cx, |view, _| view.scroll_state_for_test().0);
    assert_eq!(after, 2, "索引完成后必须换算到目标逻辑行的显示行");
    let stored = states.state(&key);
    assert_eq!(stored.scroll_line(), 2, "持久位置始终是逻辑行");
    assert_eq!(
        stored.scroll_unit(),
        super::super::view_state::ScrollUnit::LogicalLine
    );
}

#[gpui::test]
fn soft_wrap_moves_by_unicode_display_row(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        crate::app_shell::theme::apply_dark_theme(cx);
        cx.set_global(crate::app_shell::config::Settings::default());
        super::super::init(cx);
    });
    let states = super::super::view_state::EditorViewStateStore::transient();
    let key = ViewKey::Editor {
        project_id: "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b".parse().unwrap(),
        document_id: "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0f2a".parse().unwrap(),
    };
    states.restore(
        key,
        super::super::view_state::EditorViewState::new(0, 0).with_wrap(true),
    );
    let window = cx.open_window(gpui::size(px(24.0), px(60.0)), |_, cx| {
        Host::configured_with_store("A中B😀C", false, None, states, cx)
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
    let view = window.update(cx, |host, _, _| host.view.clone()).unwrap();
    let (wrap, rows, width, cell) = view.read_with(cx, |view, _| {
        let metrics = view.metrics().unwrap();
        (
            view.wrap,
            view.display_row_count(),
            metrics.bounds.size.width,
            metrics.cell_width,
        )
    });
    assert!(wrap);
    assert_eq!(rows, Some(3), "视口宽度 {width:?}，单元格 {cell:?}");
    visual.update(|window, cx| {
        let focus = view.read(cx).focus.clone();
        window.focus(&focus, cx);
    });

    visual.simulate_keystrokes("home down down");
    let moved = view.read_with(cx, |view, _| view.scroll_state_for_test());
    assert_eq!(moved.3, 9..9, "两次 Down 应到第二个折行段的 C 起点");

    visual.simulate_keystrokes("home end");
    let boundary = view.read_with(cx, |view, _| view.scroll_state_for_test());
    assert_eq!(boundary.3, 10..10, "Home/End 必须作用于当前显示段");
    visual.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.scroll_line = 1;
            let metrics = view.metrics().unwrap();
            let bounds =
                gpui::EntityInputHandler::bounds_for_range(view, 5..5, metrics.bounds, window, cx)
                    .unwrap();
            assert_eq!(
                bounds.origin.y,
                metrics.bounds.origin.y + metrics.line_height,
                "显示滚动后 IME 不得混算逻辑行"
            );
        });
    });
}

#[gpui::test]
fn soft_wrap_up_from_eof_walks_back_through_shared_boundaries(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        crate::app_shell::theme::apply_dark_theme(cx);
        cx.set_global(crate::app_shell::config::Settings::default());
        super::super::init(cx);
    });
    let states = super::super::view_state::EditorViewStateStore::transient();
    let key = ViewKey::Editor {
        project_id: "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b".parse().unwrap(),
        document_id: "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0f2a".parse().unwrap(),
    };
    states.restore(
        key,
        super::super::view_state::EditorViewState::new(0, 0).with_wrap(true),
    );
    let window = cx.open_window(gpui::size(px(24.0), px(88.0)), |_, cx| {
        Host::configured_with_store("abcdefghi", false, None, states, cx)
    });
    let mut visual = gpui::VisualTestContext::from_window(window.into(), cx);
    visual.run_until_parked();
    visual.refresh().unwrap();
    visual.update(|window, cx| {
        window.draw(cx).clear(cx);
    });
    visual.run_until_parked();
    let view = window.update(cx, |host, _, _| host.view.clone()).unwrap();
    visual.update(|window, cx| {
        let focus = view.read(cx).focus.clone();
        window.focus(&focus, cx);
    });

    visual.simulate_keystrokes("ctrl-a right up");
    let first = view.read_with(cx, |view, _| view.scroll_state_for_test());
    assert_eq!(first.3, 6..6, "第一次 Up 应落在中间显示段末尾");
    visual.simulate_keystrokes("up");
    let second = view.read_with(cx, |view, _| view.scroll_state_for_test());
    assert_eq!(second.3, 3..3, "第二次 Up 必须继续到首段末尾");
}

#[gpui::test]
fn soft_wrap_edit_reveals_cursor_after_index_rebuild(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        crate::app_shell::theme::apply_dark_theme(cx);
        cx.set_global(crate::app_shell::config::Settings::default());
        super::super::init(cx);
    });
    let states = super::super::view_state::EditorViewStateStore::transient();
    let key = ViewKey::Editor {
        project_id: "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b".parse().unwrap(),
        document_id: "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0f2a".parse().unwrap(),
    };
    states.restore(
        key,
        super::super::view_state::EditorViewState::new(0, 0).with_wrap(true),
    );
    let window = cx.open_window(gpui::size(px(24.0), px(48.0)), |_, cx| {
        Host::configured_with_store("abc", false, None, states, cx)
    });
    let mut visual = gpui::VisualTestContext::from_window(window.into(), cx);
    visual.run_until_parked();
    visual.refresh().unwrap();
    visual.update(|window, cx| {
        window.draw(cx).clear(cx);
    });
    visual.run_until_parked();
    let view = window.update(cx, |host, _, _| host.view.clone()).unwrap();
    visual.update(|window, cx| {
        let focus = view.read(cx).focus.clone();
        window.focus(&focus, cx);
    });
    visual.simulate_keystrokes("ctrl-a right");
    visual.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.replace_text_in_range(None, "d", window, cx);
            assert!(view.wrap_reveal_cursor);
        });
    });

    visual.refresh().unwrap();
    visual.update(|window, cx| {
        window.draw(cx).clear(cx);
    });
    visual.run_until_parked();
    visual.refresh().unwrap();
    visual.update(|window, cx| {
        window.draw(cx).clear(cx);
    });
    let restored = view.read_with(cx, |view, _| view.scroll_state_for_test());
    assert_eq!(restored.0, 1, "索引回投后必须显示光标所在第二行");
    assert_eq!(restored.3, 4..4);
}

#[gpui::test]
fn soft_wrap_toggle_paints_segments_and_single_boundary_cursor(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        crate::app_shell::theme::apply_dark_theme(cx);
        cx.set_global(crate::app_shell::config::Settings::default());
        super::super::init(cx);
    });
    let states = super::super::view_state::EditorViewStateStore::transient();
    let key = ViewKey::Editor {
        project_id: "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e2b".parse().unwrap(),
        document_id: "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0f2a".parse().unwrap(),
    };
    let window = cx.open_window(gpui::size(px(240.0), px(88.0)), |_, cx| {
        Host::configured_with_store("A中B😀C", false, None, states.clone(), cx)
    });
    let mut visual = gpui::VisualTestContext::from_window(window.into(), cx);
    let view = window.update(cx, |host, _, _| host.view.clone()).unwrap();
    window
        .update(cx, |_, window, cx| {
            view.update(cx, |view, cx| {
                view.set_selected_text_range(2..2, window, cx);
                view.set_scroll_for_test(2, 0);
            });
        })
        .unwrap();
    visual.run_until_parked();
    visual.refresh().unwrap();
    visual.update(|window, cx| {
        window.draw(cx).clear(cx);
    });
    let selector = Box::leak(format!("document-wrap-toggle-{}", view.entity_id()).into_boxed_str());
    let bounds = visual.debug_bounds(selector).unwrap();
    visual.simulate_click(bounds.center(), gpui::Modifiers::default());
    let immediate = states.state(&key);
    assert!(immediate.wrap());
    assert_eq!(immediate.scroll_line(), 0, "索引未就绪时不得持久化旧逻辑行");
    visual.simulate_resize(gpui::size(px(24.0), px(88.0)));
    visual.refresh().unwrap();
    visual.update(|window, cx| {
        window.draw(cx).clear(cx);
    });
    visual.run_until_parked();
    visual.refresh().unwrap();
    visual.update(|window, cx| {
        window.draw(cx).clear(cx);
    });

    assert!(view.read_with(cx, |view, _| view.wrap));
    assert!(states.state(&key).wrap());
    assert_eq!(
        view.read_with(cx, |view, _| view.painted_lines_for_test()),
        ["A中", "B😀", "C"]
    );
    assert_eq!(
        view.read_with(cx, |view, _| view.painted_cursors_for_test()),
        [(1, 4)],
        "共享折行边界不得同时在前一段末尾与后一段起点绘制光标"
    );
    visual.simulate_resize(gpui::size(px(24.0), px(48.0)));
    visual.refresh().unwrap();
    visual.update(|window, cx| {
        window.draw(cx).clear(cx);
    });
    assert!(
        view.read_with(cx, |view, _| view.painted_cursors_for_test())
            .is_empty(),
        "后段被纵向裁剪时不得把光标回画到前段"
    );
    visual.simulate_resize(gpui::size(px(24.0), px(88.0)));
    visual.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.set_selected_text_range(1..1, window, cx);
            view.move_line_boundary(true, false, cx);
            view.scroll_line = 1;
        });
    });
    visual.refresh().unwrap();
    visual.update(|window, cx| {
        window.draw(cx).clear(cx);
    });
    assert!(
        view.read_with(cx, |view, _| view.painted_cursors_for_test())
            .is_empty(),
        "前段被纵向裁剪时不得把光标回画到后段"
    );
}
