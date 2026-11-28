// Rope Editor 查找／替换栏：真实键盘与按钮路径、匹配计数、导航回绕、
// 替换确认与单次撤销、非法正则、只读、关闭清理与迟到回执丢弃。

fn draw_search_frame(visual: &mut gpui::VisualTestContext) {
    visual.refresh().unwrap();
    visual.update(|window, cx| {
        window.draw(cx).clear(cx);
    });
}

/// 窗口根必须是 gpui_component::Root，Input 组件才能解析焦点与文本选择。
fn search_window(
    cx: &mut TestAppContext,
    text: &str,
    read_only: bool,
) -> (Entity<Host>, gpui::VisualTestContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        crate::app_shell::theme::apply_dark_theme(cx);
        crate::app_shell::text_view::init(cx);
    });
    let mut slot = None;
    let window = cx.add_window(|window, cx| {
        let host = cx.new(|cx| Host::new(text, read_only, cx));
        slot = Some(host.clone());
        gpui_component::Root::new(host, window, cx)
    });
    let host = slot.unwrap();
    let visual = gpui::VisualTestContext::from_window(window.into(), cx);
    (host, visual)
}

fn editor_view(host: &Entity<Host>, cx: &TestAppContext) -> Entity<DocumentTextView> {
    host.read_with(cx, |host, _| host.view.clone())
}

fn editor_text(host: &Entity<Host>, cx: &TestAppContext) -> String {
    host.read_with(cx, |host, _| host.text())
}

fn focus_editor(host: &Entity<Host>, visual: &mut gpui::VisualTestContext) {
    let focus = visual.update(|_, cx| host.read(cx).view.read(cx).focus.clone());
    visual.update(|window, cx| window.focus(&focus, cx));
}

fn open_find(host: &Entity<Host>, visual: &mut gpui::VisualTestContext) {
    focus_editor(host, visual);
    draw_search_frame(visual);
    visual.simulate_keystrokes("ctrl-f");
    visual.run_until_parked();
    draw_search_frame(visual);
}

fn click_search_selector(visual: &mut gpui::VisualTestContext, selector: &'static str) {
    let bounds = visual
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("缺少 debug selector: {selector}"));
    visual.simulate_click(bounds.center(), gpui::Modifiers::default());
    visual.run_until_parked();
    draw_search_frame(visual);
}

/// 输入框从 Editor 找到 Input；测试只走真实键盘与鼠标路径。
#[gpui::test]
#[allow(clippy::single_range_in_vec_init)] // 单元素匹配列表就是被测语义。
fn ctrl_f_opens_incremental_find_with_options_and_counts(cx: &mut TestAppContext) {
    let (host, mut visual) = search_window(cx, "Alpha alpha alphabet\nbeta ALPHA", false);
    visual.run_until_parked();
    open_find(&host, &mut visual);
    assert!(
        visual.debug_bounds("editor-find-bar").is_some(),
        "Ctrl+F 必须打开面板内查找栏"
    );

    visual.simulate_input("alpha");
    visual.run_until_parked();
    draw_search_frame(&mut visual);
    let view = editor_view(&host, cx);
    assert_eq!(
        view.read_with(cx, |view, _| view.search_matches_for_test()),
        Some(vec![0..5, 6..11, 12..17, 26..31]),
        "默认不区分大小写且按字面量查找"
    );
    assert_eq!(
        view.read_with(cx, |view, _| view.search_active_for_test()),
        Some(0)
    );
    assert_eq!(
        view.read_with(cx, |view, _| view.scroll_state_for_test().3),
        0..5
    );

    click_search_selector(&mut visual, "editor-find-case");
    assert_eq!(
        view.read_with(cx, |view, _| view.search_matches_for_test()),
        Some(vec![6..11, 12..17]),
        "区分大小写后只保留同形匹配"
    );

    click_search_selector(&mut visual, "editor-find-word");
    assert_eq!(
        view.read_with(cx, |view, _| view.search_matches_for_test()),
        Some(vec![6..11]),
        "整词选项排除词内匹配"
    );

    click_search_selector(&mut visual, "editor-find-regex");
    assert_eq!(
        view.read_with(cx, |view, _| view.search_matches_for_test()),
        Some(vec![6..11]),
        "正则选项按正则解释同一模式"
    );
    assert!(visual.debug_bounds("editor-find-count").is_some());
}

/// Enter／Shift+Enter 与 F3／Shift+F3 共享导航并双向回绕。
#[gpui::test]
fn find_navigation_wraps_in_both_directions(cx: &mut TestAppContext) {
    let (host, mut visual) = search_window(cx, "x x x", false);
    visual.run_until_parked();
    open_find(&host, &mut visual);
    visual.simulate_input("x");
    visual.run_until_parked();
    let view = editor_view(&host, cx);
    assert_eq!(
        view.read_with(cx, |view, _| view.search_active_for_test()),
        Some(0)
    );

    visual.simulate_keystrokes("enter");
    assert_eq!(
        view.read_with(cx, |view, _| view.search_active_for_test()),
        Some(1)
    );
    assert_eq!(
        view.read_with(cx, |view, _| view.scroll_state_for_test().3),
        2..3
    );
    visual.simulate_keystrokes("enter");
    assert_eq!(
        view.read_with(cx, |view, _| view.search_active_for_test()),
        Some(2)
    );
    visual.simulate_keystrokes("enter");
    assert_eq!(
        view.read_with(cx, |view, _| view.search_active_for_test()),
        Some(0),
        "正向导航到末尾后回绕"
    );
    assert_eq!(
        view.read_with(cx, |view, _| view.scroll_state_for_test().3),
        0..1
    );

    visual.simulate_keystrokes("shift-enter");
    assert_eq!(
        view.read_with(cx, |view, _| view.search_active_for_test()),
        Some(2),
        "反向导航到开头前回绕"
    );
    assert_eq!(
        view.read_with(cx, |view, _| view.scroll_state_for_test().3),
        4..5
    );

    visual.simulate_keystrokes("f3");
    assert_eq!(
        view.read_with(cx, |view, _| view.search_active_for_test()),
        Some(0)
    );
    visual.simulate_keystrokes("shift-f3");
    assert_eq!(
        view.read_with(cx, |view, _| view.search_active_for_test()),
        Some(2)
    );
}

/// 替换当前匹配只改当前项，并经同一编辑事务提交。
#[gpui::test]
#[allow(clippy::single_range_in_vec_init)] // 单元素匹配列表就是被测语义。
fn replace_current_match_updates_only_the_active_match(cx: &mut TestAppContext) {
    let (host, mut visual) = search_window(cx, "cat cat", false);
    visual.run_until_parked();
    open_find(&host, &mut visual);
    visual.simulate_input("cat");
    visual.run_until_parked();
    click_search_selector(&mut visual, "editor-replace-input");
    visual.simulate_input("dog");
    visual.run_until_parked();
    click_search_selector(&mut visual, "editor-replace-one");
    visual.run_until_parked();

    assert_eq!(editor_text(&host, cx), "dog cat");
    let view = editor_view(&host, cx);
    assert_eq!(
        view.read_with(cx, |view, _| view.search_matches_for_test()),
        Some(vec![4..7]),
        "替换后按新正文重新匹配"
    );
    focus_editor(&host, &mut visual);
    visual.simulate_keystrokes("ctrl-z");
    assert_eq!(editor_text(&host, cx), "cat cat");
}

/// 全部替换需显式确认，确认后一次撤销回滚整次替换。
#[gpui::test]
fn replace_all_confirms_and_undoes_as_one_step(cx: &mut TestAppContext) {
    let (host, mut visual) = search_window(cx, "a a a", false);
    visual.run_until_parked();
    open_find(&host, &mut visual);
    visual.simulate_input("a");
    visual.run_until_parked();
    click_search_selector(&mut visual, "editor-replace-input");
    visual.simulate_input("b");
    visual.run_until_parked();

    click_search_selector(&mut visual, "editor-replace-all");
    let view = editor_view(&host, cx);
    assert!(view.read_with(cx, |view, _| view.search_confirm_for_test()));
    assert_eq!(editor_text(&host, cx), "a a a");

    click_search_selector(&mut visual, "editor-replace-all");
    visual.run_until_parked();
    assert_eq!(editor_text(&host, cx), "b b b");

    focus_editor(&host, &mut visual);
    visual.simulate_keystrokes("ctrl-z");
    assert_eq!(editor_text(&host, cx), "a a a");
    visual.simulate_keystrokes("ctrl-y");
    assert_eq!(editor_text(&host, cx), "b b b");
}

/// 非法正则只显示行内错误，匹配与正文均不变。
#[gpui::test]
fn invalid_regex_shows_inline_error_without_touching_text(cx: &mut TestAppContext) {
    let (host, mut visual) = search_window(cx, "abc", false);
    visual.run_until_parked();
    open_find(&host, &mut visual);
    click_search_selector(&mut visual, "editor-find-regex");
    visual.simulate_keystrokes("ctrl-f");
    visual.simulate_input("(");
    visual.run_until_parked();
    draw_search_frame(&mut visual);

    let view = editor_view(&host, cx);
    assert_eq!(
        view.read_with(cx, |view, _| view.search_error_for_test()),
        Some(true)
    );
    assert_eq!(
        view.read_with(cx, |view, _| view.search_matches_for_test()),
        Some(Vec::new())
    );
    assert!(
        visual.debug_bounds("editor-find-error").is_some(),
        "非法正则必须显示行内错误"
    );
    assert_eq!(editor_text(&host, cx), "abc");
}

/// 只读可查找但不可替换：替换控件不渲染，替换命令零副作用。
#[gpui::test]
fn read_only_document_finds_but_cannot_replace(cx: &mut TestAppContext) {
    let (host, mut visual) = search_window(cx, "abc abc", true);
    visual.run_until_parked();
    open_find(&host, &mut visual);
    visual.simulate_input("abc");
    visual.run_until_parked();
    draw_search_frame(&mut visual);

    let view = editor_view(&host, cx);
    assert_eq!(
        view.read_with(cx, |view, _| view.search_matches_for_test()),
        Some(vec![0..3, 4..7])
    );
    assert!(visual.debug_bounds("editor-replace-input").is_none());
    assert!(visual.debug_bounds("editor-replace-one").is_none());
    assert!(visual.debug_bounds("editor-replace-all").is_none());

    host.update(cx, |host, cx| {
        host.view.update(cx, |view, cx| {
            view.replace_search_current(cx);
            view.replace_search_all(cx);
            view.replace_search_all(cx);
        });
    });
    visual.run_until_parked();
    assert_eq!(editor_text(&host, cx), "abc abc");
}

/// 大文件扫描在途显示进度并允许显式取消，迟到回执不复活结果。
#[gpui::test]
fn pending_search_shows_progress_and_can_be_cancelled(cx: &mut TestAppContext) {
    let (host, mut visual) = search_window(cx, "abc", false);
    visual.run_until_parked();
    open_find(&host, &mut visual);
    visual.simulate_input("abc");
    let view = editor_view(&host, cx);
    view.update(cx, |view, _| view.mark_search_pending_for_test());
    assert!(
        view.read_with(cx, |view, _| view.search_pending_for_test()),
        "后台扫描必须处于在途态"
    );
    draw_search_frame(&mut visual);
    assert!(
        visual.debug_bounds("editor-find-cancel").is_some(),
        "在途扫描必须提供取消入口"
    );

    click_search_selector(&mut visual, "editor-find-cancel");
    assert!(
        !view.read_with(cx, |view, _| view.search_pending_for_test()),
        "取消后不得继续显示在途态"
    );
    assert_eq!(
        view.read_with(cx, |view, _| view.search_matches_for_test()),
        Some(Vec::new()),
        "取消必须清除未完成匹配"
    );
    visual.run_until_parked();
    assert_eq!(
        view.read_with(cx, |view, _| view.search_matches_for_test()),
        Some(Vec::new()),
        "取消后的迟到回执不得复活结果"
    );
}

/// 关闭查找栏清除全部瞬态，不留下高亮或匹配。
#[gpui::test]
#[allow(clippy::single_range_in_vec_init)] // 单元素匹配列表就是被测语义。
fn closing_find_clears_transient_state(cx: &mut TestAppContext) {
    let (host, mut visual) = search_window(cx, "abc", false);
    visual.run_until_parked();
    open_find(&host, &mut visual);
    visual.simulate_input("abc");
    visual.run_until_parked();
    let view = editor_view(&host, cx);
    assert_eq!(
        view.read_with(cx, |view, _| view.search_matches_for_test()),
        Some(vec![0..3])
    );

    visual.simulate_keystrokes("escape");
    visual.run_until_parked();
    draw_search_frame(&mut visual);
    assert!(visual.debug_bounds("editor-find-bar").is_none());
    view.update(cx, |view, _| {
        assert!(view.search.is_none(), "关闭后查找状态必须整体释放");
        assert_eq!(view.search_paint_ranges(), (Vec::new(), None));
    });
    assert_eq!(editor_text(&host, cx), "abc");
}

/// 正文 revision 变化后旧匹配立即失效，旧代次回执不得覆盖新结果。
#[gpui::test]
fn revision_change_discards_stale_search_replies(cx: &mut TestAppContext) {
    let (host, mut visual) = search_window(cx, "aa", false);
    visual.run_until_parked();
    open_find(&host, &mut visual);
    visual.simulate_input("a");
    visual.run_until_parked();
    let view = editor_view(&host, cx);
    let generation = view.read_with(cx, |view, _| view.search_generation_for_test().unwrap());
    let stale_revision =
        host.read_with(cx, |host, _| {
            host.registry
                .borrow()
                .session(host.id)
                .unwrap()
                .text_revision()
        });
    assert_eq!(
        view.read_with(cx, |view, _| view.search_matches_for_test()),
        Some(vec![0..1, 1..2])
    );

    host.update(cx, |host, cx| {
        let transaction = EditTransaction {
            document_id: host.id,
            expected_text_revision: stale_revision,
            edits: vec![TextEdit::new(0..2, "b")],
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
        let session = host.registry.borrow().session(host.id).unwrap().clone();
        host.view.update(cx, |view, cx| {
            view.apply_snapshot(&session, cx);
            assert_eq!(
                view.search_matches_for_test(),
                Some(Vec::new()),
                "正文变化必须同步使旧结果失效"
            );
            let rejected = view.apply_stale_search_reply_for_test(
                generation,
                stale_revision,
                vec![0..1, 1..2],
                cx,
            );
            assert!(!rejected, "旧代次回执必须被丢弃");
            assert_eq!(view.search_matches_for_test(), Some(Vec::new()));
        });
    });
    visual.run_until_parked();
    assert_eq!(
        view.read_with(cx, |view, _| view.search_matches_for_test()),
        Some(Vec::new()),
        "新正文没有匹配，迟到结果不得复活旧匹配"
    );
}
