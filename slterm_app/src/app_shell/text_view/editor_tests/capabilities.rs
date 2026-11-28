// 折叠、行号/当前行、跳转行、括号匹配、文档内补全、状态条、字号与缩进。
// 全部用例走真实视图绘制、键盘动作与共享 Document 编辑入口。

fn draw_capabilities_frame(visual: &mut gpui::VisualTestContext) {
    visual.refresh().unwrap();
    visual.update(|window, cx| {
        window.draw(cx).clear(cx);
    });
}

fn capabilities_window(
    cx: &mut TestAppContext,
    text: &str,
    read_only: bool,
    language: Option<LanguageId>,
) -> (Entity<Host>, gpui::VisualTestContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        crate::app_shell::theme::apply_dark_theme(cx);
        crate::app_shell::text_view::init(cx);
    });
    let mut slot = None;
    let window = cx.add_window(|window, cx| {
        let host = cx.new(|cx| Host::configured(text, read_only, language, cx));
        slot = Some(host.clone());
        gpui_component::Root::new(host, window, cx)
    });
    let host = slot.unwrap();
    let visual = gpui::VisualTestContext::from_window(window.into(), cx);
    (host, visual)
}

fn capabilities_view(host: &Entity<Host>, cx: &TestAppContext) -> Entity<DocumentTextView> {
    host.read_with(cx, |host, _| host.view.clone())
}

fn focus_capabilities_editor(host: &Entity<Host>, visual: &mut gpui::VisualTestContext) {
    let focus = visual.update(|_, cx| host.read(cx).view.read(cx).focus.clone());
    visual.update(|window, cx| window.focus(&focus, cx));
}

/// 等待真实语法树并把当前语言的 fold query 区域送进视图。
fn sync_fold_regions(
    host: &Entity<Host>,
    cx: &mut TestAppContext,
    visual: &mut gpui::VisualTestContext,
) {
    for _ in 0..200 {
        let ready = host.update(cx, |host, cx| {
            if let Some(syntax) = &host.syntax {
                syntax.borrow_mut().poll();
            }
            host.view.update(cx, |view, _| {
                view.refresh_folds_for_test();
                !view.fold_state_for_test().0.is_empty()
            })
        });
        if ready {
            return;
        }
        std::thread::sleep(Duration::from_millis(1));
        visual.run_until_parked();
        visual.executor().advance_clock(Duration::from_millis(30));
    }
    panic!("等待 Rust fold query 区域超时");
}

/// 折叠只改显示：正文不变、隐藏行不绘制，编辑后区域立即展开。
#[gpui::test]
fn rust_fold_query_hides_the_body_and_editing_expands_the_region(cx: &mut TestAppContext) {
    let source = "fn main() {\n    let value = 1;\n    println!(value);\n}\n";
    let (host, mut visual) = capabilities_window(cx, source, false, Some(LanguageId::Rust));
    let view = capabilities_view(&host, cx);
    visual.run_until_parked();
    sync_fold_regions(&host, cx, &mut visual);
    draw_capabilities_frame(&mut visual);

    let (regions, folded) = view.read_with(cx, |view, _| view.fold_state_for_test());
    assert_eq!(regions, vec![(0, 3)], "fold query 必须给出函数块区域");
    assert!(folded.is_empty());
    view.update(cx, |view, cx| view.toggle_fold_for_test(0, cx));
    assert_eq!(
        view.read_with(cx, |view, _| view.fold_state_for_test().1),
        vec![0]
    );
    assert_eq!(
        view.read_with(cx, |view, _| view.hidden_row_count_for_test()),
        3,
        "折叠隐藏区域内的三个逻辑行"
    );
    draw_capabilities_frame(&mut visual);
    let painted = view.read_with(cx, |view, _| view.painted_lines_for_test());
    assert_eq!(painted.first().map(String::as_str), Some("fn main() {"));
    assert!(
        !painted.iter().any(|line| line.contains("let value") || line == "}"),
        "折叠区域不得绘制: {painted:?}"
    );
    assert_eq!(host.read_with(cx, |host, _| host.text()), source);

    // 正文变化立即展开旧区域，不等待新语法树。
    visual.update(|window, cx| {
        host.update(cx, |host, cx| {
            host.view.update(cx, |view, cx| {
                gpui::EntityInputHandler::replace_text_in_range(
                    view,
                    Some(0..0),
                    "x",
                    window,
                    cx,
                );
            });
        });
    });
    cx.run_until_parked();
    assert!(host.read_with(cx, |host, _| host.text()).starts_with('x'));
    assert!(view.read_with(cx, |view, _| view.fold_state_for_test().1.is_empty()));
    draw_capabilities_frame(&mut visual);
    let painted = view.read_with(cx, |view, _| view.painted_lines_for_test());
    assert!(
        painted.iter().any(|line| line.contains("let value = 1;")),
        "正文变化后折叠区域必须自动展开: {painted:?}"
    );
}

/// 行号按可见逻辑行输出；当前行高亮跟随光标。
#[gpui::test]
fn line_numbers_and_current_line_highlight_follow_the_cursor(cx: &mut TestAppContext) {
    let (host, mut visual) = capabilities_window(cx, "alpha\nbeta\ngamma", false, None);
    let view = capabilities_view(&host, cx);
    view.update(cx, |view, _| {
        view.set_line_numbers_for_test(true);
        view.selection = TextSelection { anchor: 6, head: 6 };
    });
    draw_capabilities_frame(&mut visual);
    assert_eq!(
        view.read_with(cx, |view, _| view.painted_line_numbers_for_test()),
        vec![(0, 1), (1, 2), (2, 3)],
        "行号从 1 起算并与可见行对齐"
    );
    assert_eq!(
        view.read_with(cx, |view, _| view.painted_current_line_for_test()),
        Some(1),
        "当前行必须来自光标所在逻辑行"
    );

    view.update(cx, |view, _| {
        view.selection = TextSelection { anchor: 11, head: 11 };
    });
    draw_capabilities_frame(&mut visual);
    assert_eq!(
        view.read_with(cx, |view, _| view.painted_current_line_for_test()),
        Some(2)
    );
    assert_eq!(
        view.read_with(cx, |view, _| view.painted_line_numbers_for_test()),
        vec![(0, 1), (1, 2), (2, 3)]
    );
}

/// Ctrl+Alt+G：越界夹取并说明 1..=max；非法输入保持原位置。
#[gpui::test]
fn go_to_line_clamps_out_of_range_and_reports_the_actual_range(cx: &mut TestAppContext) {
    let (host, mut visual) = capabilities_window(cx, "0\n1\n2\n3\n4", false, None);
    let view = capabilities_view(&host, cx);
    visual.run_until_parked();
    focus_capabilities_editor(&host, &mut visual);
    draw_capabilities_frame(&mut visual);
    visual.simulate_keystrokes("ctrl-alt-g");
    visual.run_until_parked();
    draw_capabilities_frame(&mut visual);
    assert!(
        visual.debug_bounds("editor-goto-bar").is_some(),
        "跳转行命令必须打开面板内输入栏"
    );

    let input = view.read_with(cx, |view, _| view.goto_input_for_test()).unwrap();
    visual.update(|window, cx| {
        input.update(cx, |state, cx| state.set_value("99", window, cx));
    });
    visual.simulate_keystrokes("enter");
    visual.run_until_parked();
    assert_eq!(
        view.read_with(cx, |view, _| view.scroll_state_for_test().3),
        8..8,
        "越界行号必须夹到最后一行"
    );
    let message = view
        .read_with(cx, |view, _| view.goto_message_for_test())
        .expect("越界必须说明实际范围");
    assert!(
        message.contains('1') && message.contains('5'),
        "提示必须包含实际 1–5 范围: {message}"
    );

    visual.update(|window, cx| {
        input.update(cx, |state, cx| state.set_value("2", window, cx));
    });
    visual.simulate_keystrokes("enter");
    visual.run_until_parked();
    assert_eq!(
        view.read_with(cx, |view, _| view.scroll_state_for_test().3),
        2..2
    );
    assert_eq!(
        view.read_with(cx, |view, _| view.goto_message_for_test()),
        None
    );

    visual.update(|window, cx| {
        input.update(cx, |state, cx| state.set_value("oops", window, cx));
    });
    visual.simulate_keystrokes("enter");
    visual.run_until_parked();
    assert!(
        view.read_with(cx, |view, _| view.goto_message_for_test())
            .is_some(),
        "非法输入必须显示封闭提示"
    );
    assert_eq!(
        view.read_with(cx, |view, _| view.scroll_state_for_test().3),
        2..2,
        "非法输入不得移动光标"
    );
}

/// 括号匹配由纯文本栈扫描给出，并进入绘制高亮；不匹配时不产生标记。
#[gpui::test]
fn bracket_matching_highlights_the_pair_under_the_cursor(cx: &mut TestAppContext) {
    let (host, mut visual) = capabilities_window(cx, "a(b[c]d)e", false, None);
    let view = capabilities_view(&host, cx);
    view.update(cx, |view, _| {
        view.selection = TextSelection { anchor: 4, head: 4 };
    });
    assert_eq!(
        view.read_with(cx, |view, _| view.bracket_pair_for_test()),
        Some((3, 5))
    );
    draw_capabilities_frame(&mut visual);
    assert_eq!(
        view.read_with(cx, |view, _| view.painted_brackets_for_test()),
        vec![3, 5],
        "匹配括号必须进入绘制投影"
    );

    view.update(cx, |view, _| {
        view.selection = TextSelection { anchor: 2, head: 2 };
    });
    assert_eq!(
        view.read_with(cx, |view, _| view.bracket_pair_for_test()),
        Some((1, 7))
    );

    let (unmatched, _visual) = capabilities_window(cx, "a(b", false, None);
    let unmatched_view = capabilities_view(&unmatched, cx);
    unmatched_view.update(cx, |view, _| {
        view.selection = TextSelection { anchor: 2, head: 2 };
    });
    assert_eq!(
        unmatched_view.read_with(cx, |view, _| view.bracket_pair_for_test()),
        None,
        "未闭合括号不得伪造匹配"
    );
}

/// 文档内补全扫描真实正文标识符，接受候选取唯一编辑事务并写入 Document。
#[gpui::test]
fn document_identifier_completion_is_applied_through_the_edit_transaction(
    cx: &mut TestAppContext,
) {
    let source = "alph = 1\nalpha = 2\nalphabet = 3\n";
    let (host, mut visual) = capabilities_window(cx, source, false, None);
    let view = capabilities_view(&host, cx);
    view.update(cx, |view, cx| {
        view.selection = TextSelection { anchor: 4, head: 4 };
        view.trigger_completion(cx);
    });
    let candidates = view
        .read_with(cx, |view, _| view.completion_candidates_for_test())
        .expect("必须给出文档内候选");
    assert!(candidates.contains(&"alpha".to_owned()), "{candidates:?}");
    assert!(
        candidates.contains(&"alphabet".to_owned()),
        "{candidates:?}"
    );
    draw_capabilities_frame(&mut visual);
    assert!(
        visual.debug_bounds("editor-completion-popup").is_some(),
        "补全候选必须真实渲染"
    );

    view.update(cx, |view, _| {
        assert!(view.move_completion(true));
    });
    assert_eq!(
        view.read_with(cx, |view, _| view.completion_selected_for_test()),
        Some(1),
        "Down 必须导航候选"
    );
    view.update(cx, |view, cx| {
        assert!(view.accept_completion(cx));
    });
    cx.run_until_parked();
    assert!(
        host.read_with(cx, |host, _| host.text()).starts_with("alphabet = 1"),
        "接受候选必须替换当前词前缀并提交 Document"
    );
}

/// 超过 10,000,000 字节关闭文档内补全；只读也不产生候选。
#[gpui::test]
fn completion_is_disabled_over_ten_megabytes_and_in_read_only_views(cx: &mut TestAppContext) {
    let large = "a".repeat(slterm_document::NORMAL_EDIT_LIMIT_BYTES as usize + 1);
    let (host, _visual) = capabilities_window(cx, &large, false, None);
    let view = capabilities_view(&host, cx);
    view.update(cx, |view, cx| {
        view.selection = TextSelection { anchor: 1, head: 1 };
        view.trigger_completion(cx);
    });
    assert!(
        view.read_with(cx, |view, _| view.completion_candidates_for_test())
            .is_none(),
        "超限文档必须关闭文档内补全"
    );

    let (read_only, _visual) = capabilities_window(cx, "alpha beta\n", true, None);
    let read_only_view = capabilities_view(&read_only, cx);
    read_only_view.update(cx, |view, cx| {
        view.selection = TextSelection { anchor: 3, head: 3 };
        view.trigger_completion(cx);
    });
    assert!(
        read_only_view
            .read_with(cx, |view, _| view.completion_candidates_for_test())
            .is_none(),
        "只读视图不得提供编辑补全"
    );
}

/// 状态条显示语言、编码/BOM、行尾与只读原因。
#[gpui::test]
fn status_bar_reports_language_encoding_bom_line_ending_and_read_only_reason(
    cx: &mut TestAppContext,
) {
    let (host, mut visual) = capabilities_window(cx, "x", true, None);
    let view = capabilities_view(&host, cx);
    view.update(cx, |view, _| {
        view.encoding.bom = true;
        view.manual_read_only = false;
        view.inherently_read_only = true;
    });
    draw_capabilities_frame(&mut visual);
    let entity_id = view.entity_id();
    for name in ["language", "encoding", "line-ending", "read-only"] {
        let selector: &'static str =
            Box::leak(format!("editor-status-{name}-{entity_id}").into_boxed_str());
        assert!(
            visual.debug_bounds(selector).is_some(),
            "状态条必须渲染 {name}"
        );
    }
    let texts = view.read_with(cx, |view, cx| view.status_texts_for_test(cx));
    assert!(texts.iter().any(|text| text.contains("UTF-8 BOM")), "{texts:?}");
    assert!(texts.iter().any(|text| text.contains("CRLF")), "{texts:?}");
    assert!(texts.iter().any(|text| text.contains("text")), "{texts:?}");
    let language = cx.update(|cx| cx.global::<crate::app_shell::config::Settings>().language);
    assert!(
        texts
            .iter()
            .any(|text| text == language.text(crate::i18n::Message::EditorStatusReadOnlyInherent)),
        "{texts:?}"
    );

    view.update(cx, |view, _| {
        view.manual_read_only = true;
    });
    draw_capabilities_frame(&mut visual);
    let texts = view.read_with(cx, |view, cx| view.status_texts_for_test(cx));
    assert!(
        texts
            .iter()
            .any(|text| text == language.text(crate::i18n::Message::EditorStatusReadOnlyManual)),
        "手动只读必须给出独立原因: {texts:?}"
    );
}

/// Ctrl+滚轮缩放全局编辑器字号，范围 8–32px，且不滚动正文。
#[gpui::test]
fn ctrl_wheel_zooms_shared_editor_font_between_eight_and_thirty_two(cx: &mut TestAppContext) {
    super::super::reset_editor_font_size_for_test();
    let (host, mut visual) = capabilities_window(cx, "alpha\nbeta\n", false, None);
    let view = capabilities_view(&host, cx);
    draw_capabilities_frame(&mut visual);
    let metrics = view.read_with(cx, |view, _| view.metrics().unwrap());
    let scroll_before = view.read_with(cx, |view, _| view.scroll_state_for_test().0);

    visual.simulate_event(gpui::ScrollWheelEvent {
        position: metrics.bounds.center(),
        delta: gpui::ScrollDelta::Pixels(gpui::point(px(0.0), px(20.0))),
        modifiers: gpui::Modifiers {
            control: true,
            ..gpui::Modifiers::default()
        },
        ..Default::default()
    });
    assert_eq!(super::super::editor_font_size_px(), 15.0);
    assert_eq!(
        view.read_with(cx, |view, _| view.scroll_state_for_test().0),
        scroll_before,
        "Ctrl+滚轮不得滚动正文"
    );
    draw_capabilities_frame(&mut visual);
    let zoomed = view.read_with(cx, |view, _| view.metrics().unwrap());
    assert!(
        zoomed.line_height > metrics.line_height,
        "字号变化必须进入真实行高"
    );

    for _ in 0..40 {
        visual.simulate_event(gpui::ScrollWheelEvent {
            position: metrics.bounds.center(),
            delta: gpui::ScrollDelta::Pixels(gpui::point(px(0.0), px(-20.0))),
            modifiers: gpui::Modifiers {
                control: true,
                ..gpui::Modifiers::default()
            },
            ..Default::default()
        });
    }
    assert_eq!(super::super::editor_font_size_px(), 8.0);
    for _ in 0..60 {
        visual.simulate_event(gpui::ScrollWheelEvent {
            position: metrics.bounds.center(),
            delta: gpui::ScrollDelta::Pixels(gpui::point(px(0.0), px(20.0))),
            modifiers: gpui::Modifiers {
                control: true,
                ..gpui::Modifiers::default()
            },
            ..Default::default()
        });
    }
    assert_eq!(super::super::editor_font_size_px(), 32.0);
    super::super::reset_editor_font_size_for_test();
}

/// Tab 缩进光标或选中行，Shift+Tab 反缩进；只读拒绝。
#[gpui::test]
fn tab_indents_and_shift_tab_outdents_selected_lines(cx: &mut TestAppContext) {
    let (host, mut visual) = capabilities_window(cx, "one\ntwo\nthree\n", false, None);
    visual.run_until_parked();
    focus_capabilities_editor(&host, &mut visual);
    draw_capabilities_frame(&mut visual);
    visual.simulate_keystrokes("ctrl-a tab");
    visual.run_until_parked();
    assert_eq!(
        host.read_with(cx, |host, _| host.text()),
        "    one\n    two\n    three\n",
        "Tab 必须缩进全部选中行"
    );
    visual.simulate_keystrokes("ctrl-a shift-tab");
    visual.run_until_parked();
    assert_eq!(
        host.read_with(cx, |host, _| host.text()),
        "one\ntwo\nthree\n",
        "Shift+Tab 必须反缩进"
    );

    let (read_only, _visual) = capabilities_window(cx, "x", true, None);
    let read_only_view = capabilities_view(&read_only, cx);
    read_only_view.update(cx, |view, cx| view.indent(false, cx));
    assert_eq!(read_only.read_with(cx, |host, _| host.text()), "x");
}

/// 只读视图保留选择、复制、查找、换行与折叠，但不允许编辑。
#[gpui::test]
fn read_only_view_keeps_selection_copy_find_wrap_and_folding(cx: &mut TestAppContext) {
    let source = "fn main() {\n    let value = 1;\n    let other = 2;\n}\n";
    let (host, mut visual) = capabilities_window(cx, source, true, Some(LanguageId::Rust));
    let view = capabilities_view(&host, cx);
    visual.run_until_parked();
    sync_fold_regions(&host, cx, &mut visual);
    draw_capabilities_frame(&mut visual);

    view.update(cx, |view, cx| view.toggle_fold_for_test(0, cx));
    assert!(
        view.read_with(cx, |view, _| view.hidden_row_count_for_test()) > 0,
        "只读视图必须支持折叠"
    );

    focus_capabilities_editor(&host, &mut visual);
    visual.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.set_selected_text_range(0..2, window, cx);
        });
    });
    draw_capabilities_frame(&mut visual);
    visual.simulate_keystrokes("ctrl-shift-c");
    visual.update(|_, cx| {
        assert_eq!(
            cx.read_from_clipboard().and_then(|item| item.text()),
            Some("fn".to_owned()),
            "只读选区必须可复制"
        );
    });

    visual.simulate_keystrokes("ctrl-f");
    visual.run_until_parked();
    visual.simulate_input("let");
    visual.run_until_parked();
    assert!(
        view.read_with(cx, |view, _| view.search_matches_for_test())
            .is_some_and(|matches| !matches.is_empty()),
        "只读视图必须支持查找"
    );

    view.update(cx, |view, cx| view.set_wrap(true, cx));
    assert!(view.read_with(cx, |view, _| view.wrap_enabled_for_test()));

    visual.update(|window, cx| {
        view.update(cx, |view, cx| {
            gpui::EntityInputHandler::replace_text_in_range(view, Some(0..0), "x", window, cx);
        });
    });
    cx.run_until_parked();
    assert_eq!(host.read_with(cx, |host, _| host.text()), source);
}
