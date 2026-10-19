//! 指针交互的虚拟窗口集成测试:真实命中派发 → 选区 → 剪贴板。
//!
//! 走 builder 模板:draw 后读回写好的布局字段取真实坐标,事件经
//! `simulate_*` 命中测试派发,不直接调 handler。

use super::*;
use gpui::{AppContext as _, Entity, Modifiers, TestAppContext, VisualTestContext};
use gpui_component::Root;

/// 挂载一个不带真 PTY 的终端(不存在的可执行文件拉不起会话,测试用
/// `session::test_session` 顶替),返回视图实体与虚拟窗口。
fn open(cx: &mut TestAppContext) -> (Entity<TerminalView>, VisualTestContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        cx.set_global(Settings::default());
        crate::app_shell::theme::apply_dark_theme(cx);
    });
    let mut slot = None;
    let (_root, window) = cx.add_window_view(|window, cx| {
        let terminal = cx.new(|cx| {
            TerminalView::new(
                PaneId(1),
                (80, 24),
                TerminalLaunch::Local {
                    cwd: None,
                    shell: Some(slterm_terminal::tty::Shell::new(
                        "slterm-test-missing-shell-executable".into(),
                        Vec::new(),
                    )),
                    shell_name: None,
                    explicit_command: false,
                },
                window,
                cx,
            )
        });
        terminal.update(cx, |view, _| {
            let (session, _receiver) = session::test_session();
            view.session = Some(session);
            view.error = None;
            view.exited = None;
        });
        slot = Some(terminal.clone());
        Root::new(terminal, window, cx)
    });
    window.run_until_parked();
    window.refresh().unwrap();
    window.update(|window, cx| {
        window.draw(cx).clear(cx);
    });
    (slot.unwrap(), window.clone())
}

/// 不经 PTY 直接喂字节进 Term(测试注入的标准姿势)。
fn feed(view: &mut TerminalView, bytes: &[u8]) {
    let mut term = view.session.as_ref().unwrap().term.lock();
    let mut parser = vte::ansi::Processor::<vte::ansi::StdSyncHandler>::default();
    parser.advance(&mut *term, bytes);
}

/// 指定网格行列的单元格内横向 x_frac 处(窗口坐标)。
/// 选区 side 语义:x 落格左半 = 该格 Left(不含),右半 = Right(含)——
/// 断言「含端点格」的拖选终点必须给 > 0.5 的 x_frac。
fn cell_point(view: &TerminalView, col: usize, row: usize, x_frac: f32) -> Point<Pixels> {
    point(
        view.origin.x + view.cell_width * (col as f32 + x_frac),
        view.origin.y + view.line_height * (row as f32 + 0.5),
    )
}

/// 拖选渲染出的文本:按下 → 移动 → 松手,选区内容命中注入的文本;
/// `copy_selection` 落剪贴板(缝合点 8 的形态)。
#[gpui::test]
fn drag_selects_rendered_text_and_copies(cx: &mut TestAppContext) {
    let (terminal, mut window) = open(cx);
    terminal.update(&mut window, |view, _| {
        feed(view, b"hello slterm\r\n");
    });
    window.update(|window, cx| {
        window.draw(cx).clear(cx);
    });

    let (from, to) = terminal.read_with(&window, |view, _| {
        (cell_point(view, 0, 0, 0.5), cell_point(view, 4, 0, 0.75))
    });
    window.simulate_mouse_move(from, None, Modifiers::default());
    window.simulate_mouse_down(from, MouseButton::Left, Modifiers::default());
    window.simulate_mouse_move(to, MouseButton::Left, Modifiers::default());
    window.simulate_mouse_up(to, MouseButton::Left, Modifiers::default());

    let selected = terminal.read_with(&window, |view, _| {
        view.session
            .as_ref()
            .and_then(|session| session.term.lock().selection_to_string())
    });
    assert_eq!(selected.as_deref(), Some("hello"));

    let copied = terminal.update_in(&mut window, |view, window, cx| {
        view.copy_selection(true, window, cx)
    });
    assert!(copied);
    let clipboard = window.update(|_, cx| cx.read_from_clipboard().and_then(|item| item.text()));
    assert_eq!(clipboard.as_deref(), Some("hello"));
    // 复制后选区即清(缝合点 8 合同)。
    let cleared = terminal.read_with(&window, |view, _| {
        view.session
            .as_ref()
            .is_none_or(|session| session.term.lock().selection.is_none())
    });
    assert!(cleared);
}
