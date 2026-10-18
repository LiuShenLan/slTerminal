//! 07 篇 M8.4 文件树 L4 关键路径:渲染 / 双击路由 / 路径栏 / 拖放落点 /
//! OS 拖出。全程 tempdir 假文件;系统处理器与回收站调用走注入断言端。

use std::time::{Duration, Instant};

use gpui::{Modifiers, MouseButton, MouseDownEvent, MouseUpEvent, point, px};

use super::*;
use crate::app_shell::workspace::SltermWorkspace;
use crate::side_panel::drop_text_for_path;

/// 把聚焦 pane 的 cwd 设成测试目录(文件树根跟随源)。
fn set_pane_cwd(
    cx: &mut TestAppContext,
    workspace: &gpui::Entity<SltermWorkspace>,
    directory: &std::path::Path,
) {
    let view = workspace
        .read_with(cx, |ws, _| ws.first_pane_view())
        .expect("终端 pane 必须存在");
    view.update(cx, |view, _| {
        view.cwd = directory.to_string_lossy().into_owned();
    });
}

/// 打开 explorer 槽并画一帧。
fn open_explorer(window: &mut VisualTestContext) {
    draw_frame(window);
    click_selector(window, "sidebar-view-btn-explorer");
    draw_frame(window);
}

/// 反复推进时钟/绘制,直到文件树模型满足条件(后台快照是真实线程)。
fn wait_file_tree(
    cx: &mut TestAppContext,
    workspace: &gpui::Entity<SltermWorkspace>,
    window: &mut VisualTestContext,
    matches: impl Fn(&SltermWorkspace) -> bool,
) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        window.executor().advance_clock(Duration::from_millis(50));
        window.run_until_parked();
        window.refresh().unwrap();
        draw_frame(window);
        if workspace.read_with(cx, |ws, _| matches(ws)) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "file tree did not reach the expected state"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn row_index(
    cx: &mut TestAppContext,
    workspace: &gpui::Entity<SltermWorkspace>,
    name: &str,
) -> usize {
    let rows = workspace.read_with(cx, |ws, _| ws.file_tree_row_names_for_test());
    rows.iter()
        .position(|row| row == name)
        .unwrap_or_else(|| panic!("missing file-tree row {name}: {rows:?}"))
}

fn row_selector(index: usize) -> &'static str {
    // debug selector 是 `&'static str` 查找键;行号有限,测试用固定小集合。
    match index {
        0 => "file-tree-row-0",
        1 => "file-tree-row-1",
        2 => "file-tree-row-2",
        3 => "file-tree-row-3",
        4 => "file-tree-row-4",
        5 => "file-tree-row-5",
        6 => "file-tree-row-6",
        7 => "file-tree-row-7",
        _ => panic!("test expects a small directory; row {index} has no selector"),
    }
}

fn double_click_selector(window: &mut VisualTestContext, selector: &'static str) {
    let center = window
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} 必须绘制"))
        .center();
    window.simulate_event(MouseDownEvent {
        position: center,
        button: MouseButton::Left,
        modifiers: Modifiers::default(),
        click_count: 2,
        first_mouse: false,
    });
    window.simulate_event(MouseUpEvent {
        position: center,
        button: MouseButton::Left,
        modifiers: Modifiers::default(),
        click_count: 2,
    });
    draw_frame(window);
}

fn fixture_directory() -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(directory.path().join("sub")).unwrap();
    std::fs::write(directory.path().join("sub/inner.rs"), b"fn inner() {}\n").unwrap();
    std::fs::write(directory.path().join("main.rs"), b"fn main() {}\n").unwrap();
    std::fs::write(directory.path().join("data.bin"), b"binary").unwrap();
    directory
}

/// 文件树渲染:explorer 槽挂真实树行,行选择器有非零矩形。
#[gpui::test]
fn file_tree_renders_rows_for_the_active_pane_cwd(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    let directory = fixture_directory();
    set_pane_cwd(cx, &workspace, directory.path());
    open_explorer(&mut window);
    wait_file_tree(cx, &workspace, &mut window, |ws| {
        ws.file_tree_row_names_for_test()
            .iter()
            .any(|name| name == "main.rs")
    });

    let names = workspace.read_with(cx, |ws, _| ws.file_tree_row_names_for_test());
    assert_eq!(names.first().map(String::as_str), Some(".."));
    assert!(names.iter().any(|name| name == "sub"));
    assert_eq!(
        workspace.read_with(cx, |ws, _| ws.file_tree_root_for_test()),
        Some(directory.path().to_path_buf())
    );
    let tree = window.debug_bounds("file-tree").expect("文件树必须绘制");
    assert!(tree.size.width > px(0.0) && tree.size.height > px(0.0));
    let index = row_index(cx, &workspace, "main.rs");
    let row = window
        .debug_bounds(row_selector(index))
        .expect("树行必须绘制");
    assert!(row.size.width > px(0.0) && row.size.height > px(0.0));
}

/// 双击命中:应用内可读文件 → 对应 tab(Code);单击只更新选中行。
#[gpui::test]
fn double_clicking_an_openable_file_opens_the_matching_tab(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    let directory = fixture_directory();
    set_pane_cwd(cx, &workspace, directory.path());
    open_explorer(&mut window);
    wait_file_tree(cx, &workspace, &mut window, |ws| {
        ws.file_tree_row_names_for_test()
            .iter()
            .any(|name| name == "main.rs")
    });

    let index = row_index(cx, &workspace, "main.rs");
    let selector = row_selector(index);
    click_selector(&mut window, selector);
    assert_eq!(
        workspace.read_with(cx, |ws, _| ws.file_tree_selected_for_test()),
        Some(directory.path().join("main.rs"))
    );

    double_click_selector(&mut window, selector);
    window.run_until_parked();
    assert_eq!(
        workspace.read_with(cx, |ws, _| ws.active_tab_kind_for_test()),
        "code",
        "双击 main.rs 必须打开 Code tab"
    );
    assert_eq!(
        workspace.read_with(cx, |ws, cx| ws.active_text_for_test(cx)),
        Some("fn main() {}\n".to_owned())
    );
    assert!(
        workspace
            .read_with(cx, |ws, _| ws.file_tree_system_opened_for_test())
            .is_empty(),
        "命中应用内路由时不得触发系统处理器"
    );
}

/// 双击未命中:交系统处理器(注入记录),不产生 tab。
#[gpui::test]
fn double_clicking_an_unroutable_file_uses_the_system_handler_stub(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    let directory = fixture_directory();
    set_pane_cwd(cx, &workspace, directory.path());
    open_explorer(&mut window);
    wait_file_tree(cx, &workspace, &mut window, |ws| {
        ws.file_tree_row_names_for_test()
            .iter()
            .any(|name| name == "data.bin")
    });

    let tab_count = workspace.read_with(cx, |ws, _| ws.tab_count());
    let index = row_index(cx, &workspace, "data.bin");
    double_click_selector(&mut window, row_selector(index));
    window.run_until_parked();
    assert_eq!(
        workspace.read_with(cx, |ws, _| ws.file_tree_system_opened_for_test()),
        vec![directory.path().join("data.bin")],
        "未命中应用内路由的路径必须交给系统处理器"
    );
    assert_eq!(
        workspace.read_with(cx, |ws, _| ws.tab_count()),
        tab_count,
        "系统处理器路径不得新建 tab"
    );
}

/// 搜索框:非空 query 换平铺结果集;清空恢复树快照(不走盘)。
#[gpui::test]
fn file_tree_search_filters_rows_and_clears_back_to_the_tree(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    let directory = fixture_directory();
    set_pane_cwd(cx, &workspace, directory.path());
    open_explorer(&mut window);
    wait_file_tree(cx, &workspace, &mut window, |ws| {
        ws.file_tree_row_names_for_test()
            .iter()
            .any(|name| name == "data.bin")
    });

    click_selector(&mut window, "file-tree-search");
    window.simulate_input("main");
    wait_file_tree(cx, &workspace, &mut window, |ws| {
        let names = ws.file_tree_row_names_for_test();
        ws.file_tree_search_query_for_test() == "main"
            && names.iter().any(|name| name == "main.rs")
            && !names.iter().any(|name| name == "data.bin")
    });

    window.simulate_keystrokes("ctrl-a backspace");
    wait_file_tree(cx, &workspace, &mut window, |ws| {
        ws.file_tree_search_query_for_test().is_empty()
            && ws
                .file_tree_row_names_for_test()
                .iter()
                .any(|name| name == "data.bin")
    });
}

/// 路径栏:草稿不导航,Enter 提交后切换树根;非法路径保留编辑器并给出错误。
#[gpui::test]
fn path_bar_commits_a_typed_directory_and_reports_invalid_paths(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    let first = fixture_directory();
    let second = fixture_directory();
    set_pane_cwd(cx, &workspace, first.path());
    open_explorer(&mut window);
    wait_file_tree(cx, &workspace, &mut window, |ws| {
        ws.file_tree_root_for_test().as_deref() == Some(first.path())
    });

    click_selector(&mut window, "file-tree-path");
    draw_frame(&mut window);
    assert!(
        workspace.read_with(cx, |ws, _| ws.file_tree_path_editor_active_for_test()),
        "点击路径按钮必须进入编辑态"
    );
    window.simulate_input(&second.path().to_string_lossy());
    // 草稿不导航。
    assert_eq!(
        workspace.read_with(cx, |ws, _| ws.file_tree_root_for_test()),
        Some(first.path().to_path_buf())
    );
    window.simulate_keystrokes("enter");
    window.run_until_parked();
    draw_frame(&mut window);
    wait_file_tree(cx, &workspace, &mut window, |ws| {
        ws.file_tree_root_for_test().as_deref() == Some(second.path())
    });
    assert!(
        !workspace.read_with(cx, |ws, _| ws.file_tree_path_editor_active_for_test()),
        "提交成功后编辑器必须关闭"
    );

    // 非法路径:编辑器保留 + 错误可见。
    click_selector(&mut window, "file-tree-path");
    draw_frame(&mut window);
    window.simulate_input(r"D:\slterm\definitely\missing\path");
    window.simulate_keystrokes("enter");
    window.run_until_parked();
    draw_frame(&mut window);
    assert!(
        workspace.read_with(cx, |ws, _| ws.file_tree_path_editor_active_for_test()),
        "无效路径不得关闭编辑器"
    );
    assert!(
        workspace
            .read_with(cx, |ws, _| ws.file_tree_path_error_for_test())
            .is_some(),
        "无效路径必须给出错误"
    );
    assert_eq!(
        workspace.read_with(cx, |ws, _| ws.file_tree_root_for_test()),
        Some(second.path().to_path_buf()),
        "失败提交不得改变树根"
    );
}

/// 拖到终端 = 路径原文进 PTY(不含换行;尾随空格)。
#[gpui::test]
fn dragging_a_tree_row_onto_a_pane_writes_the_path_to_the_pty(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    let directory = fixture_directory();
    set_pane_cwd(cx, &workspace, directory.path());
    open_explorer(&mut window);
    wait_file_tree(cx, &workspace, &mut window, |ws| {
        ws.file_tree_row_names_for_test()
            .iter()
            .any(|name| name == "main.rs")
    });

    // 用 test_session 截获 PTY 输入。
    let view = workspace.read_with(cx, |ws, _| ws.first_pane_view().unwrap());
    let (session, receiver) = crate::gpui_terminal::session::test_session();
    view.update(cx, |view, _| view.session = Some(session));

    let index = row_index(cx, &workspace, "main.rs");
    let row = window
        .debug_bounds(row_selector(index))
        .expect("树行必须绘制")
        .center();
    let pane = window
        .debug_bounds("slterm-pane-1")
        .expect("终端 pane 必须绘制")
        .center();
    window.simulate_mouse_down(row, MouseButton::Left, Modifiers::default());
    window.simulate_mouse_move(
        point(row.x + px(24.0), row.y),
        MouseButton::Left,
        Modifiers::default(),
    );
    window.simulate_mouse_move(pane, MouseButton::Left, Modifiers::default());
    window.simulate_mouse_up(pane, MouseButton::Left, Modifiers::default());
    window.run_until_parked();

    let expected = drop_text_for_path(&directory.path().join("main.rs").display().to_string())
        .expect("路径必须可安全写入");
    let mut written = Vec::new();
    while let Ok(message) = receiver.try_recv() {
        if let slterm_terminal::event_loop::Msg::Input(bytes) = message {
            written.extend_from_slice(bytes.as_ref());
        }
    }
    assert_eq!(written, expected, "拖放落点必须把路径原文写进 PTY");
}

/// OS 拖出:行拖出窗口 → 平台收到真实文件拖放载荷(路径 + 目录位)。
#[gpui::test]
fn dragging_a_tree_row_out_of_the_window_promotes_an_os_file_drag(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    let directory = fixture_directory();
    set_pane_cwd(cx, &workspace, directory.path());
    open_explorer(&mut window);
    wait_file_tree(cx, &workspace, &mut window, |ws| {
        ws.file_tree_row_names_for_test()
            .iter()
            .any(|name| name == "sub")
    });

    let index = row_index(cx, &workspace, "sub");
    let center = window
        .debug_bounds(row_selector(index))
        .expect("目录行必须绘制")
        .center();
    window.simulate_mouse_down(center, MouseButton::Left, Modifiers::default());
    window.simulate_mouse_move(
        point(center.x + px(24.0), center.y),
        MouseButton::Left,
        Modifiers::default(),
    );
    // 移出视口左缘 = 由 GPUI 提升为平台原生拖放。
    window.simulate_mouse_move(
        point(px(-1.0), center.y),
        MouseButton::Left,
        Modifiers::default(),
    );
    window.run_until_parked();

    // gpui 平台的 `external_drag_files` 断言口是 crate 私有;这里验证
    // 解析器确实被平台调用,且载荷 = 真实路径 + 目录标记(载荷形态的
    // 单元用例在 `file_drop_tests`)。
    assert_eq!(
        workspace.read_with(cx, |ws, _| ws.file_tree_external_payloads_for_test()),
        [(directory.path().join("sub"), true)],
        "OS 拖出必须为平台准备真实文件载荷"
    );
}

// ── M8.5 搜索选项三钮 + 右键删除菜单 ──

/// 三钮切换只改模型选项,并触发一次结果重排(按钮有真实矩形)。
#[gpui::test]
fn search_option_buttons_toggle_case_word_and_regex(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    let directory = fixture_directory();
    set_pane_cwd(cx, &workspace, directory.path());
    open_explorer(&mut window);
    wait_file_tree(cx, &workspace, &mut window, |ws| {
        !ws.file_tree_row_names_for_test().is_empty()
    });
    draw_frame(&mut window);

    click_selector(&mut window, "file-tree-search-case");
    draw_frame(&mut window);
    assert!(
        workspace
            .read_with(cx, |ws, _| ws.file_tree_search_options_for_test())
            .match_case,
        "大小写钮必须点亮"
    );
    click_selector(&mut window, "file-tree-search-word");
    draw_frame(&mut window);
    assert!(
        workspace
            .read_with(cx, |ws, _| ws.file_tree_search_options_for_test())
            .whole_word,
        "全字钮必须点亮"
    );
    click_selector(&mut window, "file-tree-search-regex");
    draw_frame(&mut window);
    let options = workspace.read_with(cx, |ws, _| ws.file_tree_search_options_for_test());
    assert!(options.match_case && options.whole_word && options.regex);
}

/// 右键行 → 菜单挂在 workspace 根;删除项经回收站注入通道执行并关菜单。
#[gpui::test]
fn context_menu_delete_routes_through_the_recycle_bin(cx: &mut TestAppContext) {
    let (workspace, mut window) = open_workspace(cx);
    let directory = fixture_directory();
    set_pane_cwd(cx, &workspace, directory.path());
    open_explorer(&mut window);
    wait_file_tree(cx, &workspace, &mut window, |ws| {
        ws.file_tree_row_names_for_test()
            .iter()
            .any(|name| name == "main.rs")
    });
    draw_frame(&mut window);
    let index = row_index(cx, &workspace, "main.rs");
    let center = window
        .debug_bounds(row_selector(index))
        .expect("行必须绘制")
        .center();
    window.simulate_event(MouseDownEvent {
        position: center,
        button: MouseButton::Right,
        modifiers: Modifiers::default(),
        click_count: 1,
        first_mouse: false,
    });
    window.run_until_parked();
    draw_frame(&mut window);

    let expected = directory.path().join("main.rs");
    assert_eq!(
        workspace.read_with(cx, |ws, _| ws.file_tree_context_menu_for_test()),
        Some(expected.clone()),
        "右键必须打开该行菜单"
    );
    click_selector(&mut window, "file-tree-context-delete");
    window.run_until_parked();
    assert_eq!(
        workspace.read_with(cx, |ws, _| ws.file_tree_recycled_for_test()),
        vec![expected],
        "删除必须走回收站单点"
    );
    assert_eq!(
        workspace.read_with(cx, |ws, _| ws.file_tree_context_menu_for_test()),
        None,
        "执行后菜单关闭"
    );
}
