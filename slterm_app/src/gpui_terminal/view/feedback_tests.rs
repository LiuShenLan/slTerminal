//! 呈现接线豁免层:真实按键分发 → 补全状态 → PTY 字节。
//!
//! 与 key_tests 同纪律:经 `window.dispatch_event` 走完整焦点链与动作解析,
//! 不直接调 `on_key_down`;断言对象是「哪些字节到了 PTY / 状态是否落位」。

use super::*;
use gpui::{AppContext as _, Entity, Modifiers, TestAppContext, VisualTestContext};
use gpui_component::Root;
use slterm_terminal::event_loop::Msg;
use std::rc::Rc;
use std::sync::mpsc::Receiver;

use crate::completion::{self, Session};
use crate::suggest::{CompletionStyle, SltermCompletionKind};

/// 挂载一个可注入补全会话的终端;返回视图、虚拟窗口、PTY 接收端与
/// 壳侧会话句柄(所有权锚点)。
fn open(
    cx: &mut TestAppContext,
    style: CompletionStyle,
    ghost: bool,
) -> (Entity<TerminalView>, VisualTestContext, Receiver<Msg>) {
    cx.update(|cx| {
        gpui_component::init(cx);
        // Tab 必须先命中终端动作(否则 Root 焦点遍历会吞掉它)。
        crate::gpui_terminal::init(cx);
        let settings = crate::app_shell::config::Settings {
            completion_style: style,
            ghost,
            ..crate::app_shell::config::Settings::default()
        };
        cx.set_global(settings);
        crate::app_shell::theme::apply_dark_theme(cx);
    });
    let mut slot = None;
    let mut rx_slot = None;
    let (_root, window) = cx.add_window_view(|window, cx| {
        let terminal = cx.new(|cx| {
            let mut view = TerminalView::new(
                slterm_split::PaneId(1),
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
            );
            view.attach_completion_session(Rc::new(Session::default()));
            view
        });
        terminal.update(cx, |view, _| {
            let (session, receiver) = session::test_session();
            view.session = Some(session);
            view.error = None;
            view.exited = None;
            rx_slot = Some(receiver);
        });
        slot = Some(terminal.clone());
        Root::new(terminal, window, cx)
    });
    let terminal = slot.unwrap();
    window.update(|window, cx| {
        terminal.update(cx, |view, cx| {
            view.focus_handle.focus(window, cx);
        });
    });
    window.run_until_parked();
    window.refresh().unwrap();
    window.update(|window, cx| {
        window.draw(cx).clear(cx);
    });
    (terminal, window.clone(), rx_slot.unwrap())
}

fn press(window: &mut VisualTestContext, keystroke: gpui::Keystroke) -> bool {
    window
        .update(|window, cx| {
            window.dispatch_event(
                gpui::PlatformInput::KeyDown(KeyDownEvent {
                    keystroke,
                    is_held: false,
                    prefer_character_input: false,
                }),
                cx,
            )
        })
        .propagate
}

fn key(name: &str) -> gpui::Keystroke {
    gpui::Keystroke {
        modifiers: Modifiers::default(),
        key: name.to_owned(),
        key_char: None,
    }
}

/// 真实键入形态:key-down 先到(可能被补全/AI 条消费),文本再经 IME
/// 提交路径落进终端——与产品键盘管线一致。
fn type_text(terminal: &Entity<TerminalView>, window: &mut VisualTestContext, text: &str) {
    for ch in text.chars() {
        press(
            window,
            gpui::Keystroke {
                modifiers: Modifiers::default(),
                key: ch.to_string(),
                key_char: Some(ch.to_string()),
            },
        );
        let text = ch.to_string();
        terminal.update_in(window, |view, window, cx| {
            gpui::EntityInputHandler::replace_text_in_range(view, None, &text, window, cx);
        });
    }
    window.run_until_parked();
}

fn pty_bytes(rx: &Receiver<Msg>) -> Vec<u8> {
    let mut bytes = Vec::new();
    while let Ok(msg) = rx.try_recv() {
        if let Msg::Input(input) = msg {
            bytes.extend_from_slice(&input);
        }
    }
    bytes
}

/// Inline:键入 ghost → Tab 接受(只写余量,不写 Tab 字节)→ Enter 提交。
#[gpui::test]
fn inline_ghost_accepts_on_tab_and_enter_commits_history(cx: &mut TestAppContext) {
    let unique = "slterm-m95-ghost-";
    completion::insert_history_for_test(&format!("{unique}build --release"));
    let (terminal, mut window, rx) = open(cx, CompletionStyle::Inline, true);

    type_text(&terminal, &mut window, &format!("{unique}b"));
    let _ = pty_bytes(&rx);
    let suggestion = terminal.read_with(&window, |view, _| view.suggest.suggestion.clone());
    assert_eq!(suggestion, "uild --release", "ghost 余量来自历史提示");

    let propagate = press(&mut window, key("tab"));
    assert!(!propagate, "接受键必须被终端消费");
    assert_eq!(
        pty_bytes(&rx),
        b"uild --release".to_vec(),
        "Tab 只写 ghost 余量,不写 Tab 字节"
    );
    assert_eq!(
        terminal.read_with(&window, |view, _| view.suggest_line().to_owned()),
        format!("{unique}build --release"),
        "接受后行镜像完成整条命令"
    );

    let propagate = press(&mut window, key("enter"));
    assert!(!propagate);
    assert_eq!(pty_bytes(&rx), b"\r".to_vec(), "Enter 照常提交换行");
    assert!(
        terminal.read_with(&window, |view, _| view.suggest_line().is_empty()),
        "提交后行镜像清空"
    );
}

/// Popup:自动列表不预选;Down/Tab 导航、Enter 接受选中项(不提交换行)。
#[gpui::test]
fn popup_navigation_and_accept_edit_the_line_without_submitting(cx: &mut TestAppContext) {
    let unique = "slterm-m95-popup-";
    completion::insert_history_for_test(&format!("{unique}alpha one"));
    completion::insert_history_for_test(&format!("{unique}alpha two"));
    let (terminal, mut window, rx) = open(cx, CompletionStyle::Popup, true);

    type_text(&terminal, &mut window, &format!("{unique}alpha"));
    let _ = pty_bytes(&rx);
    let items = terminal.read_with(&window, |view, _| {
        view.suggest
            .completion_items
            .iter()
            .map(|item| item.insert.clone())
            .collect::<Vec<_>>()
    });
    assert!(!items.is_empty(), "Popup 形态自动计算候选: {items:?}");
    assert_eq!(
        terminal.read_with(&window, |view, _| view.suggest.completion_selected),
        None,
        "未导航不高亮"
    );

    // Enter 在没有选中项时不接受,走正常提交换行。
    let propagate = press(&mut window, key("enter"));
    assert!(!propagate);
    assert_eq!(pty_bytes(&rx), b"\r".to_vec());

    // 重新键入并导航。
    type_text(&terminal, &mut window, &format!("{unique}alpha"));
    let _ = pty_bytes(&rx);
    window.run_until_parked();
    press(&mut window, key("down"));
    let selected = terminal.read_with(&window, |view, _| {
        view.suggest
            .completion_selected
            .and_then(|index| view.suggest.completion_items.get(index).cloned())
    });
    let selected = selected.expect("Down 应选中首项");
    assert_eq!(selected.kind, SltermCompletionKind::History);
    let propagate = press(&mut window, key("enter"));
    assert!(!propagate);
    let bytes = pty_bytes(&rx);
    assert!(
        !bytes.contains(&b'\r'),
        "接受候选的 Enter 不得提交命令: {bytes:?}"
    );
    assert_eq!(bytes, selected.insert.as_bytes(), "只写候选余量");
}

/// Hybrid:Tab 请求列表;Esc 关闭且行不变不自动复开。
#[gpui::test]
fn hybrid_tab_requests_popup_and_escape_suppresses_the_same_line(cx: &mut TestAppContext) {
    let unique = "slterm-m95-hybrid-";
    completion::insert_history_for_test(&format!("{unique}launch now"));
    let (terminal, mut window, rx) = open(cx, CompletionStyle::Hybrid, true);

    type_text(&terminal, &mut window, &format!("{unique}la"));
    let _ = pty_bytes(&rx);
    window.run_until_parked();
    assert!(!terminal.read_with(&window, |view, _| view.suggest.suggestion.is_empty()));

    let propagate = press(&mut window, key("tab"));
    assert!(!propagate);
    window.run_until_parked();
    assert!(
        terminal.read_with(&window, |view, _| view.suggest.popup_requested),
        "Hybrid Tab 只请求弹窗"
    );
    assert!(pty_bytes(&rx).is_empty(), "请求弹窗不写 PTY");

    let propagate = press(&mut window, key("escape"));
    assert!(!propagate);
    let state = terminal.read_with(&window, |view, _| {
        (
            view.suggest.popup_requested,
            view.suggest.completion_items.len(),
        )
    });
    assert!(!state.0, "Esc 关闭 Hybrid 请求位");
    assert_eq!(state.1, 0, "Esc 清空弹窗列表");
    assert!(pty_bytes(&rx).is_empty());
}

/// 公式样式文本只走普通网格:绘制不改写源文本,指针列不做公式反投影。
#[gpui::test]
fn formula_source_paint_keeps_plain_grid_text_and_pointer_mapping(cx: &mut TestAppContext) {
    let (terminal, mut window, _rx) = open(cx, CompletionStyle::Inline, false);
    terminal.update(&mut window, |view, _| {
        let mut term = view.session.as_ref().unwrap().term.lock();
        let mut parser = vte::ansi::Processor::<vte::ansi::StdSyncHandler>::default();
        parser.advance(&mut *term, b"$x^2$ suffix\r\n");
    });
    let row_text = |terminal: &Entity<TerminalView>, window: &VisualTestContext| {
        terminal.read_with(window, |view, _| {
            let term = view.session.as_ref().unwrap().term.lock();
            (0..term.columns())
                .map(|col| {
                    term.grid()[slterm_terminal::index::Line(0)]
                        [slterm_terminal::index::Column(col)]
                    .c
                })
                .collect::<String>()
        })
    };
    let before = row_text(&terminal, &window);
    window.refresh().unwrap();
    window.update(|window, cx| {
        window.draw(cx).clear(cx);
    });
    let after = row_text(&terminal, &window);
    assert_eq!(before, after, "普通文本绘制不得改写终端 cell");
    assert_eq!(before.trim_end(), "$x^2$ suffix");

    let source_column = terminal.read_with(&window, |view, _| {
        let position = point(
            view.origin.x + view.cell_width * 6.25,
            view.origin.y + view.line_height * 0.25,
        );
        view.grid_point(position).0.column.0
    });
    assert_eq!(source_column, 6, "suffix 的 s 必须仍命中源网格第 6 列");

    terminal.update(&mut window, |view, _| {
        let mut term = view.session.as_ref().unwrap().term.lock();
        term.selection = Some(Selection::new(
            SelectionType::Simple,
            TermPoint::new(Line(0), Column(0)),
            Side::Left,
        ));
        term.selection
            .as_mut()
            .unwrap()
            .update(TermPoint::new(Line(0), Column(11)), Side::Right);
        assert_eq!(
            term.selection_to_string().as_deref(),
            Some("$x^2$ suffix"),
            "复制源必须保留公式样式文本的原始字节"
        );
    });
}
