//! 键位真实分发与 IME 可测面的虚拟窗口集成测试。
//!
//! 纪律(11 篇「键绑定真实分发测试锚」):产品键盘语义(复制/中断/粘贴/
//! 翻页/放行)一律以「注入按键 → 观测动作」端到端锚定——事件经
//! `window.dispatch_event` 真实派发(完整 keymap 匹配 + 焦点链),禁直接
//! 调 `on_key_down`。解析层断言归 keymap.rs 的解析矩阵,不在这里重复。

use super::*;
use gpui::{AppContext as _, Entity, Modifiers, TestAppContext, VisualTestContext};
use gpui_component::Root;
use slterm_terminal::event_loop::Msg;
use std::sync::mpsc::Receiver;

/// 挂载一个不带真 PTY 的终端(同 pointer/tests.rs 形态),额外返回
/// test_session 的 PTY 接收端——键位合同的断言对象就是「哪些字节到了
/// PTY / 剪贴板动没动」。
fn open(cx: &mut TestAppContext) -> (Entity<TerminalView>, VisualTestContext, Receiver<Msg>) {
    cx.update(|cx| {
        gpui_component::init(cx);
        cx.set_global(Settings::default());
        crate::app_shell::theme::apply_dark_theme(cx);
    });
    let mut slot = None;
    let mut rx_slot = None;
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
    // 键盘分发只在焦点链上到达终端;焦点必须在首轮布局前落定。
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

/// 不经 PTY 直接喂字节进 Term(测试注入的标准姿势)。
fn feed(view: &mut TerminalView, bytes: &[u8]) {
    let mut term = view.session.as_ref().unwrap().term.lock();
    let mut parser = vte::ansi::Processor::<vte::ansi::StdSyncHandler>::default();
    parser.advance(&mut *term, bytes);
}

/// 真实派发一次 key-down,返回是否继续冒泡(false = 终端已消费)。
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

/// 手工构造键:`key_char` 恒 None——GPUI Windows 丢弃控制字符文本
/// (Ctrl+C 的 ETX 即其中之一),这正是真实 key-down 到达终端的形态。
fn key(name: &str, modifiers: Modifiers) -> gpui::Keystroke {
    gpui::Keystroke {
        modifiers,
        key: name.to_owned(),
        key_char: None,
    }
}

fn ctrl(name: &str) -> gpui::Keystroke {
    key(
        name,
        Modifiers {
            control: true,
            ..Default::default()
        },
    )
}

/// 排干 test_session 的 PTY 接收端,拼出已下发字节。
fn pty_bytes(rx: &Receiver<Msg>) -> Vec<u8> {
    let mut bytes = Vec::new();
    while let Ok(msg) = rx.try_recv() {
        if let Msg::Input(input) = msg {
            bytes.extend_from_slice(&input);
        }
    }
    bytes
}

fn clipboard_text(window: &mut VisualTestContext) -> Option<String> {
    window.update(|_, cx| cx.read_from_clipboard().and_then(|item| item.text()))
}

/// 程序化构造覆盖首行前 `cols` 列的选区(与拖选同构的 Simple 选区)。
fn select_first_line(terminal: &Entity<TerminalView>, cx: &mut TestAppContext, cols: usize) {
    terminal.update(cx, |view, _| {
        let mut term = view.session.as_ref().unwrap().term.lock();
        let mut selection = Selection::new(
            SelectionType::Simple,
            TermPoint::new(Line(0), Column(0)),
            Side::Left,
        );
        selection.update(TermPoint::new(Line(0), Column(cols - 1)), Side::Right);
        term.selection = Some(selection);
    });
}

/// 定位约束 7 端到端锚:Ctrl+C 不在快捷键层——有/无选区一律经编码器
/// 透传 0x03,剪贴板永不被它触碰。
#[gpui::test]
fn ctrl_c_passes_through_interrupt_with_and_without_selection(cx: &mut TestAppContext) {
    let (terminal, mut window, rx) = open(cx);
    window.update(|_, cx| cx.write_to_clipboard(ClipboardItem::new_string("sentinel".to_owned())));

    // 无选区:透传 0x03,剪贴板不动。
    let propagate = press(&mut window, ctrl("c"));
    assert!(!propagate, "编码键已消费,不再冒泡");
    assert_eq!(pty_bytes(&rx), vec![0x03]);
    assert_eq!(clipboard_text(&mut window).as_deref(), Some("sentinel"));

    // 有选区:同样透传(复制只属于 ctrl+shift+c);write_input 清选区。
    terminal.update(&mut window, |view, _| feed(view, b"hello slterm\r\n"));
    select_first_line(&terminal, cx, 5);
    let propagate = press(&mut window, ctrl("c"));
    assert!(!propagate);
    assert_eq!(pty_bytes(&rx), vec![0x03]);
    assert_eq!(clipboard_text(&mut window).as_deref(), Some("sentinel"));
    let cleared = terminal.read_with(&window, |view, _| {
        view.session
            .as_ref()
            .is_none_or(|s| s.term.lock().selection.is_none())
    });
    assert!(cleared, "输入即清选区(write_input 合同)");
}

/// ctrl+shift+c 有选区:写剪贴板 + CopyFeedback 受控时钟时限(§5.2)+
/// PTY 零字节 + 停止冒泡。
#[gpui::test]
fn ctrl_shift_c_copies_selection_and_feedback_expires_on_controlled_clock(cx: &mut TestAppContext) {
    let (terminal, mut window, rx) = open(cx);
    terminal.update(&mut window, |view, _| feed(view, b"hello slterm\r\n"));
    select_first_line(&terminal, cx, 5);

    let modifiers = Modifiers {
        control: true,
        shift: true,
        ..Default::default()
    };
    let propagate = press(&mut window, key("c", modifiers));
    assert!(!propagate);
    assert_eq!(clipboard_text(&mut window).as_deref(), Some("hello"));
    assert!(pty_bytes(&rx).is_empty(), "复制键永不入 PTY");

    // §5.2 反馈时限:受控时钟推进,到期自动复位。
    assert!(terminal.read_with(&window, |view, _| view.copy_feedback.is_copied()));
    window.executor().advance_clock(
        super::super::copy_feedback::COPY_FEEDBACK_TTL - std::time::Duration::from_millis(1),
    );
    window.run_until_parked();
    assert!(terminal.read_with(&window, |view, _| view.copy_feedback.is_copied()));
    window
        .executor()
        .advance_clock(std::time::Duration::from_millis(1));
    window.run_until_parked();
    assert!(!terminal.read_with(&window, |view, _| view.copy_feedback.is_copied()));
}

/// D13-1 默认语义(13 篇待沉淀,按默认实现):无选区的 ctrl+shift+c
/// 静默吞——剪贴板不动、PTY 零字节、不冒泡、无反馈。
#[gpui::test]
fn ctrl_shift_c_without_selection_is_swallowed_silently(cx: &mut TestAppContext) {
    let (_terminal, mut window, rx) = open(cx);
    window.update(|_, cx| cx.write_to_clipboard(ClipboardItem::new_string("sentinel".to_owned())));

    let modifiers = Modifiers {
        control: true,
        shift: true,
        ..Default::default()
    };
    let propagate = press(&mut window, key("c", modifiers));
    // D13-1:吞语义——stop_propagation 无负反馈(负反馈/透传中断的权衡
    // 与截止口径见 13 篇待沉淀决策,改语义先过那条裁决)。
    assert!(!propagate, "复制键永不冒泡入应用族");
    assert_eq!(clipboard_text(&mut window).as_deref(), Some("sentinel"));
    assert!(pty_bytes(&rx).is_empty());
}

/// ctrl+shift+v:bracketed-paste 包装 + `\x1b[201~` 注入过滤。
#[gpui::test]
fn ctrl_shift_v_pastes_bracketed_and_strips_sentinel(cx: &mut TestAppContext) {
    let (terminal, mut window, rx) = open(cx);
    // 子进程开启 bracketed paste(DECSET 2004)。
    terminal.update(&mut window, |view, _| feed(view, b"\x1b[?2004h"));
    window.update(|_, cx| {
        cx.write_to_clipboard(ClipboardItem::new_string("ab\x1b[201~cd\nef".to_owned()))
    });

    let modifiers = Modifiers {
        control: true,
        shift: true,
        ..Default::default()
    };
    let propagate = press(&mut window, key("v", modifiers));
    assert!(!propagate);
    // 换行归一成 CR;内嵌收尾哨兵被剥掉,防注入截断。
    assert_eq!(pty_bytes(&rx), b"\x1b[200~abcd\ref\x1b[201~".to_vec());
}

/// shift+pageup 仅主屏:本地翻页,零字节下发;备用屏则透传编码序列。
#[gpui::test]
fn shift_pageup_scrolls_main_screen_but_reaches_alt_screen_encoded(cx: &mut TestAppContext) {
    let (terminal, mut window, rx) = open(cx);
    // 40 行输出灌出回滚(屏幕 24 行)。
    terminal.update(&mut window, |view, _| {
        let mut buf = Vec::new();
        for i in 0..40 {
            buf.extend_from_slice(format!("line {i:02}\r\n").as_bytes());
        }
        feed(view, &buf);
    });
    let before = terminal.read_with(&window, |view, _| {
        view.session
            .as_ref()
            .unwrap()
            .term
            .lock()
            .grid()
            .display_offset()
    });

    let modifiers = Modifiers {
        shift: true,
        ..Default::default()
    };
    let propagate = press(&mut window, key("pageup", modifiers));
    assert!(!propagate);
    let after = terminal.read_with(&window, |view, _| {
        view.session
            .as_ref()
            .unwrap()
            .term
            .lock()
            .grid()
            .display_offset()
    });
    assert!(after > before, "主屏 shift+pageup = 本地翻页");
    assert!(pty_bytes(&rx).is_empty(), "翻页不打扰子进程");

    // 备用屏(less/vim 一类):不拦截,修饰翻页序列透传给应用。
    terminal.update(&mut window, |view, _| feed(view, b"\x1b[?1049h"));
    let propagate = press(&mut window, key("pagedown", modifiers));
    assert!(!propagate, "编码键同样消费,只是改走 PTY");
    assert_eq!(pty_bytes(&rx), b"\x1b[6;2~".to_vec());
}

/// alt+f4 是原生窗口快捷键:终端不编码、不拦截,交给 DefWindowProc。
#[gpui::test]
fn alt_f4_propagates_to_the_host_window(cx: &mut TestAppContext) {
    let (_terminal, mut window, rx) = open(cx);
    let propagate = press(
        &mut window,
        key(
            "f4",
            Modifiers {
                alt: true,
                ..Default::default()
            },
        ),
    );
    assert!(propagate, "原生窗口快捷键必须冒泡到系统默认处理");
    assert!(pty_bytes(&rx).is_empty());
}

/// IME 组合中(marked_text 非空)全键放行:不编码、不拦截——GPUI
/// Windows 在 stop_propagation 后跳过 TranslateMessage,组合必须保住。
#[gpui::test]
fn marked_text_keeps_all_keys_off_the_wire(cx: &mut TestAppContext) {
    let (terminal, mut window, rx) = open(cx);
    terminal.update_in(&mut window, |view, window, cx| {
        gpui::EntityInputHandler::replace_and_mark_text_in_range(
            view, None, "ni", None, window, cx,
        );
    });
    assert!(terminal.read_with(&window, |view, _| view.marked_text.is_some()));

    for keystroke in [
        key("enter", Modifiers::default()),
        ctrl("c"),
        key("up", Modifiers::default()),
    ] {
        let propagate = press(&mut window, keystroke);
        assert!(propagate, "组合中按键必须放行,不拦截");
    }
    assert!(pty_bytes(&rx).is_empty(), "组合中任何键都不下发 PTY");
}

/// IME 可测面:marked_text 置/清、提交字节原样(不经编码器)。
#[gpui::test]
fn ime_marks_preedit_and_commits_bytes_verbatim(cx: &mut TestAppContext) {
    let (terminal, mut window, rx) = open(cx);
    // 组合中:marked_text 置位,range = 全串(UTF-16 计);折叠选区
    // 让 IME 把组合文本插在光标处。
    let range = terminal.update_in(&mut window, |view, window, cx| {
        gpui::EntityInputHandler::replace_and_mark_text_in_range(
            view, None, "拼音", None, window, cx,
        );
        let range = gpui::EntityInputHandler::marked_text_range(view, window, cx);
        let collapsed = gpui::EntityInputHandler::selected_text_range(view, false, window, cx);
        (range, collapsed.map(|s| s.range))
    });
    assert_eq!(range.0, Some(0..2), "拼音 = 2 个 UTF-16 单元");
    assert_eq!(range.1, Some(2..2), "折叠选区贴在组合文本尾");
    assert!(
        terminal.read_with(&window, |view, _| view.marked_text.as_deref()
            == Some("拼音"))
    );

    // 提交:marked_text 清除,字节原样(UTF-8)写 PTY。
    terminal.update_in(&mut window, |view, window, cx| {
        gpui::EntityInputHandler::replace_text_in_range(view, None, "中文", window, cx);
    });
    assert!(terminal.read_with(&window, |view, _| view.marked_text.is_none()));
    assert_eq!(pty_bytes(&rx), "中文".as_bytes().to_vec());

    // 组合被 IME 取消:空串标记 = 清除,无字节下发。
    terminal.update_in(&mut window, |view, window, cx| {
        gpui::EntityInputHandler::replace_and_mark_text_in_range(
            view, None, "ni", None, window, cx,
        );
        gpui::EntityInputHandler::unmark_text(view, window, cx);
    });
    assert!(terminal.read_with(&window, |view, _| view.marked_text.is_none()));
    assert!(pty_bytes(&rx).is_empty());
}

/// IME 可测面:ime_bounds 跟光标单元格(paint 帧按光标格算,候选窗定位
/// 合同);`bounds_for_range` 恒回该锚点。
#[gpui::test]
fn ime_bounds_follow_the_cursor_cell(cx: &mut TestAppContext) {
    let (terminal, mut window, _rx) = open(cx);
    terminal.update(&mut window, |view, _| feed(view, b"hi"));
    window.run_until_parked();
    window.refresh().unwrap();
    window.update(|window, cx| {
        window.draw(cx).clear(cx);
    });

    let (origin, cell_width, line_height, ime_bounds) = terminal.read_with(&window, |view, _| {
        (
            view.origin,
            view.cell_width,
            view.line_height,
            view.ime_bounds,
        )
    });
    // "hi" 两列后光标在 (行 0, 列 2)。
    assert_eq!(ime_bounds.origin.x, origin.x + cell_width * 2.0);
    assert_eq!(ime_bounds.origin.y, origin.y);
    assert_eq!(ime_bounds.size.width, cell_width);
    assert_eq!(ime_bounds.size.height, line_height);

    let reported = terminal.update_in(&mut window, |view, window, cx| {
        gpui::EntityInputHandler::bounds_for_range(view, 0..0, ime_bounds, window, cx)
    });
    assert_eq!(reported, Some(ime_bounds));
}
