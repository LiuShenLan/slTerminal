//! 呈现接线豁免层:真实按键分发 → 补全状态 → PTY 字节。
//!
//! 与 key_tests 同纪律:经 `window.dispatch_event` 走完整焦点链与动作解析,
//! 不直接调 `on_key_down`;断言对象是「哪些字节到了 PTY / 状态是否落位」。

use super::*;
use gpui::{AppContext as _, Entity, Modifiers, TestAppContext, VisualTestContext};
use gpui_component::Root;
use slterm_terminal::event_loop::Msg;
use slterm_terminal::index::{Column, Line, Side};
use std::cell::RefCell;
use std::rc::Rc;
use std::str::FromStr as _;
use std::sync::mpsc::Receiver;

use crate::completion::{self, Session};
use crate::suggest::{CompletionStyle, SltermCompletionKind};

/// 挂载一个可注入补全会话的终端;返回视图、虚拟窗口、PTY 接收端与
/// 壳侧会话句柄(所有权锚点)。
fn open(
    cx: &mut TestAppContext,
    style: CompletionStyle,
    ghost: bool,
) -> (
    Entity<TerminalView>,
    VisualTestContext,
    Receiver<Msg>,
    Rc<RefCell<crate::gpui_terminal::registry::TerminalRegistry>>,
) {
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
    let registry = Rc::new(RefCell::new(
        crate::gpui_terminal::registry::TerminalRegistry::default(),
    ));
    let (_root, window) = cx.add_window_view(|window, cx| {
        let terminal = cx.new(|cx| {
            let mut view = TerminalView::restore_error(
                crate::gpui_terminal::PaneId(1),
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
                "测试禁用真实 PTY".to_owned(),
                window,
                cx,
            );
            view.attach_completion_session(Rc::new(Session::default()));
            view
        });
        terminal.update(cx, |view, _| {
            let (session, receiver) = session::test_session();
            view.install_test_worker(
                &registry,
                slterm_workspace::TerminalId::from_str("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e09")
                    .unwrap(),
                session,
            );
            view.establish_test_prompt();
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
    (terminal, window.clone(), rx_slot.unwrap(), registry)
}

fn dispatch(window: &mut VisualTestContext, keystroke: gpui::Keystroke) -> bool {
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

fn press(
    terminal: &Entity<TerminalView>,
    window: &mut VisualTestContext,
    keystroke: gpui::Keystroke,
) -> bool {
    let before = terminal.read_with(window, |view, _| view.suggest_line().to_owned());
    let key = keystroke.key.clone();
    let result = dispatch(window, keystroke);
    let after = terminal.read_with(window, |view, _| view.suggest_line().to_owned());
    terminal.update_in(window, |view, _, cx| {
        if key == "enter" && after.is_empty() {
            view.feed_test_bytes(b"\r\n\x1b]133;A\x07\x1b]133;B\x07");
            view.refresh_suggestion(cx);
            return;
        }
        let old: Vec<char> = before.chars().collect();
        let new: Vec<char> = after.chars().collect();
        let common = old
            .iter()
            .zip(&new)
            .take_while(|(left, right)| left == right)
            .count();
        let mut echo = vec![0x7f; old.len().saturating_sub(common)];
        echo.extend(new[common..].iter().collect::<String>().as_bytes());
        if !echo.is_empty() {
            view.feed_test_bytes(&echo);
            view.refresh_suggestion(cx);
        }
    });
    window.run_until_parked();
    result
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
            terminal,
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
            // 内存 worker 不产生 PTY 回显；保留输入接收端供字节断言，单独把
            // 同等字节喂给真实网格，使可信行只能经正常帧与回显对账开放。
            view.feed_test_bytes(text.as_bytes());
            view.refresh_suggestion(cx);
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
    let (terminal, mut window, rx, _registry) = open(cx, CompletionStyle::Inline, true);

    type_text(&terminal, &mut window, &format!("{unique}b"));
    let _ = pty_bytes(&rx);
    let suggestion = terminal.read_with(&window, |view, _| view.suggest.suggestion.clone());
    assert_eq!(suggestion, "uild --release", "ghost 余量来自历史提示");

    let propagate = press(&terminal, &mut window, key("tab"));
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

    let propagate = press(&terminal, &mut window, key("enter"));
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
    let (terminal, mut window, rx, _registry) = open(cx, CompletionStyle::Popup, true);

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
    let propagate = press(&terminal, &mut window, key("enter"));
    assert!(!propagate);
    assert_eq!(pty_bytes(&rx), b"\r".to_vec());

    // 重新键入并导航。
    type_text(&terminal, &mut window, &format!("{unique}alpha"));
    let _ = pty_bytes(&rx);
    window.run_until_parked();
    press(&terminal, &mut window, key("down"));
    let selected = terminal.read_with(&window, |view, _| {
        view.suggest
            .completion_selected
            .and_then(|index| view.suggest.completion_items.get(index).cloned())
    });
    let selected = selected.expect("Down 应选中首项");
    assert_eq!(selected.kind, SltermCompletionKind::History);
    let propagate = press(&terminal, &mut window, key("enter"));
    assert!(!propagate);
    let bytes = pty_bytes(&rx);
    assert!(
        !bytes.contains(&b'\r'),
        "接受候选的 Enter 不得提交命令: {bytes:?}"
    );
    assert_eq!(bytes, selected.insert.as_bytes(), "只写候选余量");
}

/// 可信门控：回显之外的编辑关闭补全；无 Start 或被取消的执行不采集；
/// 接受候选只编辑当前行，不产生 Enter 或执行记录。
#[gpui::test]
fn terminal_trusted_line_completion_gate(cx: &mut TestAppContext) {
    let command = "slterm-trusted-gate-command";
    completion::insert_history_for_test(command);
    let (terminal, mut window, rx, _registry) = open(cx, CompletionStyle::Inline, true);
    let submitted = Rc::new(RefCell::new(Vec::new()));
    let observed = Rc::clone(&submitted);
    let _subscription = window.update(|_, cx| {
        cx.subscribe(&terminal, move |_, event, _| {
            if let TerminalViewEvent::CommandSubmitted(line) = event {
                observed.borrow_mut().push(line.clone());
            }
        })
    });

    type_text(&terminal, &mut window, "slterm-trusted-gate-c");
    let _ = pty_bytes(&rx);
    assert!(!terminal.read_with(&window, |view, _| view.suggest.suggestion.is_empty()));
    press(&terminal, &mut window, key("left"));
    assert!(terminal.read_with(&window, |view, _| {
        view.suggest.suggestion.is_empty() && view.suggest.completion_items.is_empty()
    }));
    let _ = pty_bytes(&rx);
    terminal.update_in(&mut window, |view, _, cx| {
        view.feed_event_for_test(TermEvent::CommandStart, cx);
        view.feed_event_for_test(TermEvent::CommandDone { exit_code: Some(0) }, cx);
    });
    window.run_until_parked();
    assert!(submitted.borrow().is_empty(), "不可信行不得采集");

    terminal.update(&mut window, |view, _| view.establish_test_prompt());
    type_text(&terminal, &mut window, command);
    let _ = pty_bytes(&rx);
    dispatch(&mut window, key("enter"));
    assert_eq!(pty_bytes(&rx), b"\r".to_vec());
    terminal.update_in(&mut window, |view, _, cx| {
        view.feed_event_for_test(TermEvent::CommandDone { exit_code: Some(0) }, cx);
    });
    window.run_until_parked();
    assert!(submitted.borrow().is_empty(), "没有 Start 不得采集");

    terminal.update(&mut window, |view, _| view.establish_test_prompt());
    type_text(&terminal, &mut window, command);
    let _ = pty_bytes(&rx);
    dispatch(&mut window, key("enter"));
    let _ = pty_bytes(&rx);
    terminal.update_in(&mut window, |view, _, cx| {
        view.feed_event_for_test(TermEvent::CommandStart, cx);
    });
    press(
        &terminal,
        &mut window,
        gpui::Keystroke {
            modifiers: Modifiers {
                control: true,
                ..Modifiers::default()
            },
            key: "c".to_owned(),
            key_char: None,
        },
    );
    terminal.update_in(&mut window, |view, _, cx| {
        view.feed_event_for_test(TermEvent::CommandDone { exit_code: Some(0) }, cx);
    });
    window.run_until_parked();
    assert!(submitted.borrow().is_empty(), "取消的执行不得采集");
    let _ = pty_bytes(&rx);

    terminal.update(&mut window, |view, _| view.establish_test_prompt());
    type_text(&terminal, &mut window, "slterm-trusted-gate-c");
    let _ = pty_bytes(&rx);
    press(&terminal, &mut window, key("tab"));
    let bytes = pty_bytes(&rx);
    assert_eq!(bytes, b"ommand".to_vec());
    assert!(!bytes.contains(&b'\r'), "接受候选不得自动执行");
    assert!(submitted.borrow().is_empty());
}

/// 进程退出也是未闭合执行的取消边界，迟到的 Done 不得采集。
#[gpui::test]
fn terminal_exit_cancels_unclosed_execution(cx: &mut TestAppContext) {
    let (terminal, mut window, rx, _registry) = open(cx, CompletionStyle::Inline, true);
    type_text(&terminal, &mut window, "echo slterm-unclosed-exit");
    let _ = pty_bytes(&rx);
    dispatch(&mut window, key("enter"));
    let _ = pty_bytes(&rx);
    terminal.update_in(&mut window, |view, _, cx| {
        view.feed_event_for_test(TermEvent::CommandStart, cx);
        view.feed_event_for_test(TermEvent::Exit, cx);
        assert_eq!(view.shell_line.command_done(), None);
    });
}

/// 工作区面板循环优先于活动补全弹窗，且不得写入 PTY。
#[gpui::test]
fn ctrl_tab_is_not_consumed_by_active_completion_popup(cx: &mut TestAppContext) {
    let unique = "slterm-panel-cycle-popup-";
    completion::insert_history_for_test(&format!("{unique}alpha one"));
    completion::insert_history_for_test(&format!("{unique}alpha two"));
    let (terminal, mut window, rx, _registry) = open(cx, CompletionStyle::Popup, true);
    type_text(&terminal, &mut window, &format!("{unique}alpha"));
    let _ = pty_bytes(&rx);
    assert!(
        terminal.read_with(&window, |view, _| !view.suggest.completion_items.is_empty()),
        "前置条件必须是活动补全弹窗"
    );
    let before = terminal.read_with(&window, |view, _| {
        (
            view.suggest_line().to_owned(),
            view.suggest.completion_selected,
        )
    });

    for (control, shift) in [(true, false), (true, true)] {
        let propagate = press(
            &terminal,
            &mut window,
            gpui::Keystroke {
                modifiers: Modifiers {
                    control,
                    shift,
                    ..Modifiers::default()
                },
                key: "tab".to_owned(),
                key_char: None,
            },
        );
        assert!(propagate, "Ctrl+Tab 必须冒泡到工作区循环");
        assert!(pty_bytes(&rx).is_empty(), "面板循环不得写入 PTY");
        assert_eq!(
            terminal.read_with(&window, |view, _| {
                (
                    view.suggest_line().to_owned(),
                    view.suggest.completion_selected,
                )
            }),
            before,
            "全局循环不得接受或移动补全候选"
        );
    }
}

/// Hybrid:Tab 请求列表;Esc 关闭且行不变不自动复开。
#[gpui::test]
fn hybrid_tab_requests_popup_and_escape_suppresses_the_same_line(cx: &mut TestAppContext) {
    let unique = "slterm-m95-hybrid-";
    completion::insert_history_for_test(&format!("{unique}launch now"));
    let (terminal, mut window, rx, _registry) = open(cx, CompletionStyle::Hybrid, true);

    type_text(&terminal, &mut window, &format!("{unique}la"));
    let _ = pty_bytes(&rx);
    window.run_until_parked();
    assert!(!terminal.read_with(&window, |view, _| view.suggest.suggestion.is_empty()));

    let propagate = press(&terminal, &mut window, key("tab"));
    assert!(!propagate);
    window.run_until_parked();
    assert!(
        terminal.read_with(&window, |view, _| view.suggest.popup_requested),
        "Hybrid Tab 只请求弹窗"
    );
    assert!(pty_bytes(&rx).is_empty(), "请求弹窗不写 PTY");

    let propagate = press(&terminal, &mut window, key("escape"));
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
    let (terminal, mut window, _rx, _registry) = open(cx, CompletionStyle::Inline, false);
    terminal.update(&mut window, |view, _| {
        view.feed_test_bytes(b"$x^2$ suffix\r\n");
    });
    let row_text = |terminal: &Entity<TerminalView>, window: &VisualTestContext| {
        terminal.read_with(window, |view, _| {
            let frame = view.frame.as_ref().unwrap();
            let mut cells = vec![" ".to_owned(); frame.render().cols as usize];
            for segment in &frame.render().segments {
                if segment.row != 0 {
                    continue;
                }
                for cell in &segment.cells {
                    cells[cell.col as usize] = cell.text.clone();
                }
            }
            cells.concat()
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
        let mut selection = Selection::new(
            SelectionType::Simple,
            TermPoint::new(Line(0), Column(0)),
            Side::Left,
        );
        selection.update(TermPoint::new(Line(0), Column(11)), Side::Right);
        view.set_test_selection(selection);
        assert_eq!(
            view.selected_text(),
            Some("$x^2$ suffix"),
            "复制源必须保留公式样式文本的原始字节"
        );
    });
}
