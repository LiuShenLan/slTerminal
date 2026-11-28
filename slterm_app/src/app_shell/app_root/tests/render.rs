//! AppRoot 空项目与终端的渲染边界。

use super::*;

#[gpui::test]
fn empty_project_renders_empty_state_without_terminal(cx: &mut TestAppContext) {
    let (_root, mut window, state) = open_root(cx);
    draw_frame(&mut window);
    assert!(window.debug_bounds("app-root-empty").is_some());

    add_project(&_root, &mut window, PROJECT_A, "D:/work/a");
    draw_frame(&mut window);

    assert!(window.debug_bounds("app-root-empty").is_some());
    assert!(window.debug_bounds("slterm-terminal").is_none());
    assert_eq!(state.borrow().attempts, 0);
}
