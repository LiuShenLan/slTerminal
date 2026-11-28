// Diff 展示参数的真实控件、渲染消费与按侧降级。

use crate::app_shell::text_view::DiffViewMode;

fn click_control(window: &mut VisualTestContext, selector: &'static str) {
    draw_frame(window);
    let bounds = window
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("控件必须渲染: {selector}"));
    window.simulate_click(bounds.center(), Modifiers::default());
    window.run_until_parked();
    draw_frame(window);
}

fn wait_for_diff(view: &Entity<DiffView>, cx: &mut TestAppContext) {
    for _ in 0..40 {
        cx.run_until_parked();
        if view.read_with(cx, |view, _| view.change_state_for_test().0 > 0) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

fn open_modified_diff(
    cx: &mut TestAppContext,
    repository: &std::path::Path,
    binding: &slterm_document::RepositoryBinding,
    file: &CanonicalPath,
) -> (Entity<AppRoot>, VisualTestContext, Entity<DiffView>) {
    let (root, mut window, _) = open_root(cx);
    add_project(
        &root,
        &mut window,
        PROJECT_A,
        repository.to_str().unwrap(),
    );
    super::documents::open_document(&root, &mut window, PROJECT_A, GROUP_A, file.clone());
    draw_frame(&mut window);
    let document = root.read_with(cx, |root, _| {
        root.snapshot()
            .documents()
            .find_map(|(id, location)| (location.canonical_path() == file).then_some(id))
            .unwrap()
    });
    open_diff(&root, &mut window, document, binding, "first.txt");
    draw_frame(&mut window);
    let panel = root.read_with(cx, |root, cx| {
        root.projects[&project_id(PROJECT_A)]
            .read(cx)
            .panels
            .values()
            .find_map(|panel| match &panel.content {
                PanelContent::Diff(view) => Some(view.clone()),
                _ => None,
            })
            .unwrap()
    });
    wait_for_diff(&panel, cx);
    draw_frame(&mut window);
    (root, window, panel)
}

/// 模式、交换、折叠、列宽、同步滚动与未变更折叠都由真实控件消费并落盘。
#[gpui::test]
fn diff_display_controls_change_rendering_and_persist(cx: &mut TestAppContext) {
    let (_temp, repository, binding) = super::gitshow::repository();
    let mut content = String::from("changed first\n");
    for index in 1..30 {
        content.push_str(&format!("line {index}\n"));
    }
    std::fs::write(repository.join("first.txt"), &content).unwrap();
    let file =
        CanonicalPath::new(dunce::canonicalize(repository.join("first.txt")).unwrap()).unwrap();
    let (_root, mut window, diff) = open_modified_diff(cx, &repository, &binding, &file);
    draw_frame(&mut window);
    let head = window.debug_bounds("diff-pane-head").expect("HEAD 侧必须渲染");
    let worktree = window
        .debug_bounds("diff-pane-worktree")
        .expect("工作区侧必须渲染");
    assert!(head.origin.x < worktree.origin.x);
    let head_width = f32::from(head.size.width);

    click_control(&mut window, "diff-swap");
    draw_frame(&mut window);
    let head = window.debug_bounds("diff-pane-head").unwrap();
    let worktree = window.debug_bounds("diff-pane-worktree").unwrap();
    assert!(head.origin.x > worktree.origin.x, "交换必须改变显示顺序");
    diff.read_with(cx, |view, _| {
        assert!(view.view_state_for_test().swapped());
    });

    click_control(&mut window, "diff-collapse-head");
    draw_frame(&mut window);
    assert!(
        window.debug_bounds("diff-pane-head").is_none(),
        "折叠 HEAD 后不得渲染该侧"
    );
    assert!(window.debug_bounds("diff-pane-worktree").is_some());

    click_control(&mut window, "diff-collapse-worktree");
    draw_frame(&mut window);
    assert!(
        window.debug_bounds("diff-pane-worktree").is_none(),
        "改折叠工作区后只保留 HEAD"
    );
    assert!(window.debug_bounds("diff-pane-head").is_some());
    click_control(&mut window, "diff-collapse-worktree");
    draw_frame(&mut window);
    // 恢复未交换的并排顺序，列宽断言针对首栏。
    click_control(&mut window, "diff-swap");
    draw_frame(&mut window);

    click_control(&mut window, "diff-width-wider");
    draw_frame(&mut window);
    let widened = f32::from(
        window
            .debug_bounds("diff-pane-head")
            .expect("并排两侧必须可见")
            .size
            .width,
    );
    assert!(
        (widened - head_width - 40.0).abs() < 2.0,
        "列宽控件必须真实改变首栏宽度: {head_width} -> {widened}"
    );
    diff.read_with(cx, |view, _| {
        assert_eq!(view.view_state_for_test().column_width(), 520);
    });

    click_control(&mut window, "diff-mode-toggle");
    draw_frame(&mut window);
    assert!(
        window.debug_bounds("diff-inline").is_some(),
        "行内模式必须渲染统一列"
    );
    assert!(window.debug_bounds("diff-pane-head").is_none());
    diff.read_with(cx, |view, _| {
        assert_eq!(view.view_state_for_test().mode(), DiffViewMode::Inline);
        assert!(view.inline_row_count_for_test() > 0);
    });

    // 行内模式保留空白与未变更折叠控件，但隐藏交换／列宽。
    assert!(window.debug_bounds("diff-swap").is_none());
    assert!(window.debug_bounds("diff-collapse-unchanged").is_some());
    click_control(&mut window, "diff-whitespace");
    draw_frame(&mut window);
    diff.read_with(cx, |view, _| {
        assert_eq!(
            view.view_state_for_test().whitespace(),
            slterm_document::DiffWhitespaceMode::Trailing
        );
    });

    let before = diff.read_with(cx, |view, _| view.inline_row_count_for_test());
    click_control(&mut window, "diff-collapse-unchanged");
    draw_frame(&mut window);
    assert!(
        window.debug_bounds("diff-inline-collapsed").is_some(),
        "未变更折叠必须出现占位行"
    );
    let after = diff.read_with(cx, |view, _| view.inline_row_count_for_test());
    assert!(after < before, "折叠必须减少可见行: {before} -> {after}");

    // 参数已写入 DiffViewState；关闭重开由同一 Store 恢复并再次消费。
    diff.read_with(cx, |view, _| {
        let state = view.view_state_for_test();
        assert_eq!(state.mode(), DiffViewMode::Inline);
        assert!(!state.swapped());
        assert_eq!(state.column_width(), 520);
        assert!(state.collapse_unchanged());
    });
}

/// 同步滚动开启时两侧共享逻辑滚动行；关闭后互不影响。
#[gpui::test]
fn diff_sync_scroll_moves_both_sides(cx: &mut TestAppContext) {
    let (_temp, repository, binding) = super::gitshow::repository();
    let mut content = String::from("changed first\n");
    for index in 1..40 {
        content.push_str(&format!("line {index}\n"));
    }
    std::fs::write(repository.join("first.txt"), &content).unwrap();
    let file =
        CanonicalPath::new(dunce::canonicalize(repository.join("first.txt")).unwrap()).unwrap();
    let (_root, mut window, diff) = open_modified_diff(cx, &repository, &binding, &file);
    for _ in 0..20 {
        window.run_until_parked();
        draw_frame(&mut window);
        if diff.read_with(cx, |view, _| view.head_loaded_for_test()) {
            break;
        }
    }
    assert!(
        diff.read_with(cx, |view, _| view.head_loaded_for_test()),
        "HEAD 必须加载后才能同步滚动"
    );
    window.update(|_, cx| {
        diff.update(cx, |view, cx| {
            assert!(view.view_state_for_test().sync_scroll());
            view.scroll_right_for_test(7, cx);
        });
    });
    window.run_until_parked();
    assert_eq!(
        diff.read_with(cx, |view, app| view.head_scroll_line_for_test(app)),
        7,
        "同步开启时工作区滚动必须带动 HEAD"
    );

    click_control(&mut window, "diff-sync-scroll");
    window.update(|_, cx| {
        diff.update(cx, |view, cx| view.scroll_right_for_test(15, cx));
    });
    window.run_until_parked();
    assert_eq!(
        diff.read_with(cx, |view, app| view.head_scroll_line_for_test(app)),
        7,
        "同步关闭后 HEAD 不得跟随"
    );
    assert_eq!(
        diff.read_with(cx, |view, app| view.worktree_scroll_line_for_test(app)),
        Some(15)
    );
}

/// 工作区侧超限分页时 Diff 仍打开并显示可见可重试的降级错误。
#[gpui::test]
fn diff_paged_worktree_keeps_body_with_retryable_notice(cx: &mut TestAppContext) {
    let (_temp, repository, binding) = super::gitshow::repository();
    let content = "a\n".repeat(5_100_000);
    std::fs::write(repository.join("first.txt"), &content).unwrap();
    let file =
        CanonicalPath::new(dunce::canonicalize(repository.join("first.txt")).unwrap()).unwrap();
    let (root, mut window, _) = open_root(cx);
    add_project(
        &root,
        &mut window,
        PROJECT_A,
        repository.to_str().unwrap(),
    );
    super::documents::open_document(&root, &mut window, PROJECT_A, GROUP_A, file.clone());
    draw_frame(&mut window);
    let document = root.read_with(cx, |root, _| {
        root.snapshot()
            .documents()
            .find_map(|(id, location)| (location.canonical_path() == &file).then_some(id))
            .unwrap()
    });
    assert!(
        root.read_with(cx, |root, _| root.paged_documents.contains_key(&document)),
        "超限文件必须注册分页正文"
    );
    open_diff(&root, &mut window, document, &binding, "first.txt");
    draw_frame(&mut window);
    window.run_until_parked();
    draw_frame(&mut window);

    assert!(
        window.debug_bounds("diff-pane-worktree").is_some(),
        "Diff 必须保留分页工作区正文"
    );
    assert!(
        window.debug_bounds("diff-error-banner").is_some(),
        "差异不可用必须有可见降级说明"
    );
    assert!(
        window.debug_bounds("diff-retry").is_some(),
        "降级说明必须提供重试"
    );
}

/// HEAD 侧含 NUL 时按二进制降级，另一侧正文保持可见。
#[gpui::test]
fn diff_binary_head_degrades_that_side_and_keeps_worktree(cx: &mut TestAppContext) {
    let (_temp, repository, binding) = super::gitshow::repository();
    std::fs::write(
        repository.join("binary.dat"),
        [b'a', b'b', 0, b'c', b'd', b'\n'],
    )
    .unwrap();
    super::gitshow::git(&repository, &["add", "--", "binary.dat"]);
    super::gitshow::git(&repository, &["commit", "--quiet", "-m", "binary"]);
    std::fs::write(repository.join("binary.dat"), "text now\n").unwrap();
    let file =
        CanonicalPath::new(dunce::canonicalize(repository.join("binary.dat")).unwrap()).unwrap();
    let (root, mut window, _) = open_root(cx);
    add_project(
        &root,
        &mut window,
        PROJECT_A,
        repository.to_str().unwrap(),
    );
    super::documents::open_document(&root, &mut window, PROJECT_A, GROUP_A, file.clone());
    draw_frame(&mut window);
    let document = root.read_with(cx, |root, _| {
        root.snapshot()
            .documents()
            .find_map(|(id, location)| (location.canonical_path() == &file).then_some(id))
            .unwrap()
    });
    open_diff(&root, &mut window, document, &binding, "binary.dat");
    draw_frame(&mut window);
    window.run_until_parked();
    draw_frame(&mut window);

    assert!(
        window.debug_bounds("gitshow-open-system").is_some(),
        "HEAD 二进制侧必须显示自身降级内容"
    );
    assert!(
        window.debug_bounds("diff-pane-worktree").is_some(),
        "工作区文本侧必须保留"
    );
    assert!(
        window.debug_bounds("diff-error-banner").is_some(),
        "二进制侧差异不可用必须可见"
    );
}
