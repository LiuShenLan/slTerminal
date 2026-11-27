//! 键位真实分发与 IME 可测面的虚拟窗口集成测试。
//!
//! 纪律:产品键盘语义(复制/中断/粘贴/
//! 翻页/放行)一律以「注入按键 → 观测动作」端到端锚定——事件经
//! `window.dispatch_event` 真实派发(完整 keymap 匹配 + 焦点链),禁直接
//! 调 `on_key_down`。解析层断言归 keymap.rs 的解析矩阵,不在这里重复。

use super::*;
use gpui::{AppContext as _, Entity, Modifiers, TestAppContext, VisualTestContext};
use gpui_component::Root;
use slterm_terminal::event_loop::Msg;
use slterm_terminal::index::{Column, Line, Side};
use std::cell::RefCell;
use std::rc::Rc;
use std::str::FromStr as _;
use std::sync::mpsc::Receiver;

/// 挂载一个不带真 PTY 的终端(同 pointer/tests.rs 形态),额外返回
/// test_session 的 PTY 接收端——键位合同的断言对象就是「哪些字节到了
/// PTY / 剪贴板动没动」。
fn open(
    cx: &mut TestAppContext,
) -> (
    Entity<TerminalView>,
    VisualTestContext,
    Receiver<Msg>,
    Rc<RefCell<crate::gpui_terminal::registry::TerminalRegistry>>,
) {
    cx.update(|cx| {
        gpui_component::init(cx);
        cx.set_global(Settings::default());
        crate::app_shell::theme::apply_dark_theme(cx);
    });
    let mut slot = None;
    let mut rx_slot = None;
    let registry = Rc::new(RefCell::new(
        crate::gpui_terminal::registry::TerminalRegistry::default(),
    ));
    let (_root, window) = cx.add_window_view(|window, cx| {
        let terminal = cx.new(|cx| {
            TerminalView::restore_error(
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
                "测试禁用真实 PTY".to_owned(),
                window,
                cx,
            )
        });
        terminal.update(cx, |view, _| {
            let (session, receiver) = session::test_session();
            view.install_test_worker(
                &registry,
                slterm_workspace::TerminalId::from_str("018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0e07")
                    .unwrap(),
                session,
            );
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
    (terminal, window.clone(), rx_slot.unwrap(), registry)
}

/// 不经 PTY 直接喂字节进 Term(测试注入的标准姿势)。
fn feed(view: &mut TerminalView, bytes: &[u8]) {
    view.feed_test_bytes(bytes);
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
        let mut selection = Selection::new(
            SelectionType::Simple,
            TermPoint::new(Line(0), Column(0)),
            Side::Left,
        );
        selection.update(TermPoint::new(Line(0), Column(cols - 1)), Side::Right);
        view.set_test_selection(selection);
    });
}

/// 定位约束 7 端到端锚:Ctrl+C 不在快捷键层——有/无选区一律经编码器
/// 透传 0x03,剪贴板永不被它触碰。
#[gpui::test]
fn ctrl_c_passes_through_interrupt_with_and_without_selection(cx: &mut TestAppContext) {
    let (terminal, mut window, rx, _registry) = open(cx);
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
    let cleared = terminal.read_with(&window, |view, _| view.selection_cleared);
    assert!(cleared, "输入即清选区(write_input 合同)");
}

/// ctrl+shift+c 有选区:写剪贴板 + CopyFeedback 受控时钟时限(§5.2)+
/// PTY 零字节 + 停止冒泡。
#[gpui::test]
fn ctrl_shift_c_copies_selection_and_feedback_expires_on_controlled_clock(cx: &mut TestAppContext) {
    let (terminal, mut window, rx, _registry) = open(cx);
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

/// ctrl+shift+a 全选回滚与屏幕:只发选择命令、零 PTY 字节，复制得到完整文本。
#[gpui::test]
fn ctrl_shift_a_selects_all_terminal_text_without_pty_write(cx: &mut TestAppContext) {
    let (terminal, mut window, rx, _registry) = open(cx);
    terminal.update(&mut window, |view, _| {
        let mut buf = Vec::new();
        for i in 0..40 {
            buf.extend_from_slice(format!("line {i:02}\r\n").as_bytes());
        }
        feed(view, &buf);
    });

    let modifiers = Modifiers {
        control: true,
        shift: true,
        ..Default::default()
    };
    let propagate = press(&mut window, key("a", modifiers));
    assert!(!propagate, "全选键必须先于编码器消费");

    let mut commands = Vec::new();
    while let Ok(command) = rx.try_recv() {
        commands.push(command);
    }
    assert!(
        matches!(
            commands.as_slice(),
            [Msg::Select(
                slterm_terminal::selection::SelectionCommand::SelectAll
            )]
        ),
        "全选路径只允许一条选择命令，不得向 PTY 写字节: {commands:?}"
    );
    terminal.update(&mut window, |view, _| {
        for command in &commands {
            view.apply_test_worker_command(command);
        }
    });

    let text = terminal
        .read_with(&window, |view, _| view.selected_text().map(str::to_owned))
        .expect("全选必须形成可复制文本");
    assert!(text.starts_with("line 00"), "{text:?}");
    assert!(text.ends_with("line 39"), "{text:?}");
    assert!(!text.ends_with('\n'), "全选不得凭空追加尾随换行");
}

/// Shift+方向键从光标扩选：只发选择命令，不把修饰键序列写进 PTY。
#[gpui::test]
fn shift_arrow_extends_selection_without_pty_write(cx: &mut TestAppContext) {
    let (terminal, mut window, rx, _registry) = open(cx);
    terminal.update(&mut window, |view, _| feed(view, b"abc"));
    let modifiers = Modifiers {
        shift: true,
        ..Default::default()
    };

    let propagate = press(&mut window, key("left", modifiers));
    assert!(!propagate, "扩选键必须先于编码器消费");
    let mut commands = Vec::new();
    while let Ok(command) = rx.try_recv() {
        commands.push(command);
    }
    assert!(
        matches!(
            commands.as_slice(),
            [Msg::Select(
                slterm_terminal::selection::SelectionCommand::Extend(
                    slterm_terminal::selection::SelectionMotion::Left
                )
            )]
        ),
        "扩选路径只允许一条选择命令: {commands:?}"
    );
    terminal.update(&mut window, |view, _| {
        for command in &commands {
            view.apply_test_worker_command(command);
        }
    });
    assert_eq!(
        terminal.read_with(&window, |view, _| view.selected_text().map(str::to_owned)),
        Some("c".to_owned())
    );
}

/// Shift+PageUp 扩选并滚动到较早内容，仍不写 PTY。
#[gpui::test]
fn shift_pageup_extends_selection_into_scrollback(cx: &mut TestAppContext) {
    let (terminal, mut window, rx, _registry) = open(cx);
    terminal.update(&mut window, |view, _| {
        let mut buf = Vec::new();
        for i in 0..40 {
            buf.extend_from_slice(format!("line {i:02}\r\n").as_bytes());
        }
        feed(view, &buf);
    });
    let modifiers = Modifiers {
        shift: true,
        ..Default::default()
    };

    let propagate = press(&mut window, key("pageup", modifiers));
    assert!(!propagate);
    let mut commands = Vec::new();
    while let Ok(command) = rx.try_recv() {
        commands.push(command);
    }
    assert!(
        matches!(
            commands.as_slice(),
            [Msg::Select(
                slterm_terminal::selection::SelectionCommand::Extend(
                    slterm_terminal::selection::SelectionMotion::PageUp
                )
            )]
        ),
        "翻页扩选只允许一条选择命令: {commands:?}"
    );
    terminal.update(&mut window, |view, _| {
        for command in &commands {
            view.apply_test_worker_command(command);
        }
    });
    let text = terminal
        .read_with(&window, |view, _| view.selected_text().map(str::to_owned))
        .expect("翻页扩选必须形成选区");
    assert!(text.contains("line 16"), "{text:?}");
    assert!(!text.contains('\u{1b}'), "不得把编码序列当作正文: {text:?}");
}

/// 默认语义(待沉淀,按默认实现):无选区的 ctrl+shift+c
/// 静默吞——剪贴板不动、PTY 零字节、不冒泡、无反馈。
#[gpui::test]
fn ctrl_shift_c_without_selection_is_swallowed_silently(cx: &mut TestAppContext) {
    let (_terminal, mut window, rx, _registry) = open(cx);
    window.update(|_, cx| cx.write_to_clipboard(ClipboardItem::new_string("sentinel".to_owned())));

    let modifiers = Modifiers {
        control: true,
        shift: true,
        ..Default::default()
    };
    let propagate = press(&mut window, key("c", modifiers));
    // 吞语义——stop_propagation 无负反馈(负反馈/透传中断的权衡
    // 与截止口径见待沉淀决策,改语义先过那条裁决)。
    assert!(!propagate, "复制键永不冒泡入应用族");
    assert_eq!(clipboard_text(&mut window).as_deref(), Some("sentinel"));
    assert!(pty_bytes(&rx).is_empty());
}

/// ctrl+shift+v:bracketed-paste 包装 + `\x1b[201~` 注入过滤。
#[gpui::test]
fn ctrl_shift_v_pastes_bracketed_and_strips_sentinel(cx: &mut TestAppContext) {
    let (terminal, mut window, rx, _registry) = open(cx);
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
    assert!(cx.has_pending_prompt(), "内嵌 ESC 属于危险控制字符");
    cx.simulate_prompt_answer("粘贴");
    window.run_until_parked();
    // 换行归一成 CR;内嵌收尾哨兵被剥掉,防注入截断。
    assert_eq!(pty_bytes(&rx), b"\x1b[200~abcd\ref\x1b[201~".to_vec());
}

/// 危险粘贴必须等待明确决定；提示只含风险类别，不复制剪贴板正文。
#[gpui::test]
fn dangerous_paste_confirmation_does_not_leak_or_write_early(cx: &mut TestAppContext) {
    let (terminal, mut window, rx, _registry) = open(cx);
    terminal.update(&mut window, |view, _| feed(view, b"\x1b[?2004h"));
    let text = "secret\x1b[201~value";
    window.update(|_, cx| cx.write_to_clipboard(ClipboardItem::new_string(text.to_owned())));
    let modifiers = Modifiers {
        control: true,
        shift: true,
        ..Default::default()
    };

    press(&mut window, key("v", modifiers));
    assert!(pty_bytes(&rx).is_empty(), "确认前不得写 PTY");
    assert!(cx.has_pending_prompt());
    let (message, detail) = cx.pending_prompt().unwrap();
    assert!(!message.contains("secret") && !detail.contains("secret"));
    cx.simulate_prompt_answer("取消");
    window.run_until_parked();
    assert!(pty_bytes(&rx).is_empty(), "取消不得写 PTY");

    press(&mut window, key("v", modifiers));
    cx.simulate_prompt_answer("粘贴");
    window.run_until_parked();
    assert_eq!(pty_bytes(&rx), b"\x1b[200~secretvalue\x1b[201~");

    window.update(|_, cx| {
        cx.write_to_clipboard(ClipboardItem::new_string("x".repeat(10 * 1024 * 1024 + 1)))
    });
    press(&mut window, key("v", modifiers));
    assert!(!cx.has_pending_prompt());
    assert!(pty_bytes(&rx).is_empty(), "超过 10 MiB 必须拒绝");
    assert!(
        terminal.read_with(&window, |view, _| view.notice.is_some()),
        "超限粘贴只产生瞬态提示"
    );
}

/// 超限粘贴只产生可关闭的瞬态提示，不得替换或遮挡终端画面。
#[gpui::test]
fn oversized_paste_keeps_terminal_visible_with_dismissible_notice(cx: &mut TestAppContext) {
    let (terminal, mut window, rx, _registry) = open(cx);
    terminal.update(&mut window, |view, _| {
        feed(
            view,
            b"still visible
",
        )
    });
    window.run_until_parked();
    window.update(|_, cx| {
        cx.write_to_clipboard(ClipboardItem::new_string("x".repeat(10 * 1024 * 1024 + 1)));
    });
    terminal.update_in(&mut window, |view, window, cx| view.paste(window, cx));
    window.run_until_parked();
    assert!(
        window.update(|_, cx| terminal.read(cx).notice_for_test().is_some()),
        "超限粘贴必须给出瞬态提示"
    );
    assert!(
        !window.update(|_, cx| terminal.read(cx).has_error()),
        "瞬态失败不得升级为致命错误"
    );
    let notice = window
        .debug_bounds("slterm-terminal-notice")
        .expect("提示必须可见");
    assert!(
        window.debug_bounds("slterm-terminal-error").is_none(),
        "终端画面不得被错误文本替换"
    );
    assert!(
        window.debug_bounds("slterm-terminal").is_some(),
        "终端元素必须继续渲染"
    );
    window.simulate_click(notice.center(), gpui::Modifiers::default());
    window.run_until_parked();
    assert!(
        window.debug_bounds("slterm-terminal-notice").is_none(),
        "点击后提示必须关闭"
    );

    // 超限拒绝不写 PTY。
    terminal.update_in(&mut window, |_, _, _| {
        while let Ok(message) = rx.try_recv() {
            assert!(
                !matches!(message, Msg::Input(_)),
                "超限粘贴不得写入 PTY: {message:?}"
            );
        }
    });
}

#[test]
fn paste_confirmation_thresholds_cover_controls_and_line_count() {
    assert!(!paste_requires_confirmation("plain\ttext"));
    assert!(!paste_requires_confirmation(&"line\n".repeat(50)));
    assert!(paste_requires_confirmation(&"line\n".repeat(51)));
    assert!(paste_requires_confirmation("bad\0text"));
    assert!(paste_requires_confirmation("bad\x1btext"));
}

#[gpui::test]
fn shift_insert_pastes_plain_text(cx: &mut TestAppContext) {
    let (_terminal, mut window, rx, _registry) = open(cx);
    window
        .update(|_, cx| cx.write_to_clipboard(ClipboardItem::new_string("plain text".to_owned())));
    press(
        &mut window,
        key(
            "insert",
            Modifiers {
                shift: true,
                ..Default::default()
            },
        ),
    );
    assert_eq!(pty_bytes(&rx), b"plain text");
    assert!(!cx.has_pending_prompt());
}

#[gpui::test]
fn paste_confirmation_can_be_disabled_explicitly(cx: &mut TestAppContext) {
    let (_terminal, mut window, rx, _registry) = open(cx);
    window.update(|_, cx| {
        cx.set_global(Settings {
            multiline_paste_confirm: false,
            ..Settings::default()
        });
        cx.write_to_clipboard(ClipboardItem::new_string("bad\0text".to_owned()));
    });
    press(
        &mut window,
        key(
            "v",
            Modifiers {
                control: true,
                shift: true,
                ..Default::default()
            },
        ),
    );
    assert!(!cx.has_pending_prompt());
    assert_eq!(pty_bytes(&rx), b"bad\0text");
}

/// 退出后只读：复制与回滚仍可用，普通输入不再写入 PTY。
#[gpui::test]
fn exited_terminal_keeps_copy_and_scroll_but_rejects_input(cx: &mut TestAppContext) {
    let (terminal, mut window, rx, _registry) = open(cx);
    terminal.update(&mut window, |view, _| {
        feed(
            view,
            b"line one
line two
",
        )
    });
    select_first_line(&terminal, cx, 4);
    terminal.update(&mut window, |view, _| {
        view.exited = Some("进程已退出".to_owned());
    });

    window.update(|_, cx| cx.write_to_clipboard(ClipboardItem::new_string("sentinel".to_owned())));
    let modifiers = Modifiers {
        control: true,
        shift: true,
        ..Default::default()
    };
    let _ = press(&mut window, key("c", modifiers));
    assert_eq!(
        clipboard_text(&mut window).as_deref(),
        Some("line"),
        "退出后复制必须可用"
    );

    let _ = press(
        &mut window,
        key(
            "pageup",
            Modifiers {
                shift: true,
                ..Default::default()
            },
        ),
    );
    let mut saw_extend = false;
    while let Ok(msg) = rx.try_recv() {
        if matches!(
            msg,
            Msg::Select(slterm_terminal::selection::SelectionCommand::Extend(
                slterm_terminal::selection::SelectionMotion::PageUp
            ))
        ) {
            saw_extend = true;
        }
    }
    assert!(saw_extend, "退出后键盘扩选与回滚必须可用");

    let _ = press(&mut window, key("a", Modifiers::default()));
    assert!(pty_bytes(&rx).is_empty(), "退出后不得向 PTY 写输入");
}

/// Shift+PageUp/PageDown 扩选：主屏可进入回滚，备用屏被限制在当前画面，
/// 两个路径都不写 PTY。
#[gpui::test]
fn shift_paging_extends_selection_and_alt_screen_stays_local(cx: &mut TestAppContext) {
    let (terminal, mut window, rx, _registry) = open(cx);
    // 40 行输出灌出回滚(屏幕 24 行)。
    terminal.update(&mut window, |view, _| {
        let mut buf = Vec::new();
        for i in 0..40 {
            buf.extend_from_slice(format!("line {i:02}\r\n").as_bytes());
        }
        feed(view, &buf);
    });
    let modifiers = Modifiers {
        shift: true,
        ..Default::default()
    };
    let propagate = press(&mut window, key("pageup", modifiers));
    assert!(!propagate);
    assert!(matches!(
        rx.try_recv(),
        Ok(Msg::Select(
            slterm_terminal::selection::SelectionCommand::Extend(
                slterm_terminal::selection::SelectionMotion::PageUp
            )
        ))
    ));
    assert!(pty_bytes(&rx).is_empty(), "扩选不打扰子进程");

    // 备用屏(less/vim 一类):扩选仍归本地，但只能落在当前画面。
    terminal.update(&mut window, |view, _| feed(view, b"\x1b[?1049h"));
    let propagate = press(&mut window, key("pagedown", modifiers));
    assert!(!propagate, "扩选键必须由本地消费");
    assert!(matches!(
        rx.try_recv(),
        Ok(Msg::Select(
            slterm_terminal::selection::SelectionCommand::Extend(
                slterm_terminal::selection::SelectionMotion::PageDown
            )
        ))
    ));
    assert!(pty_bytes(&rx).is_empty(), "备用屏扩选同样不得写 PTY");
}

/// alt+f4 是原生窗口快捷键:终端不编码、不拦截,交给 DefWindowProc。
#[gpui::test]
fn alt_f4_propagates_to_the_host_window(cx: &mut TestAppContext) {
    let (_terminal, mut window, rx, _registry) = open(cx);
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
/// 组合态必须同步到全局命令闸门：命令派发前即可拦截。
#[gpui::test]
fn marked_text_toggles_the_ime_command_gate(cx: &mut TestAppContext) {
    let (terminal, mut window, _rx, _registry) = open(cx);
    assert!(!cx.update(|cx| {
        cx.try_global::<crate::gpui_terminal::ImeComposition>()
            .is_some_and(|state| state.active)
    }));
    terminal.update_in(&mut window, |view, window, cx| {
        gpui::EntityInputHandler::replace_and_mark_text_in_range(
            view, None, "ni", None, window, cx,
        );
    });
    assert!(
        cx.update(|cx| {
            cx.try_global::<crate::gpui_terminal::ImeComposition>()
                .is_some_and(|state| state.active)
        }),
        "组合开始必须打开命令闸门"
    );
    terminal.update_in(&mut window, |view, window, cx| {
        gpui::EntityInputHandler::replace_and_mark_text_in_range(view, None, "", None, window, cx);
    });
    assert!(
        !cx.update(|cx| {
            cx.try_global::<crate::gpui_terminal::ImeComposition>()
                .is_some_and(|state| state.active)
        }),
        "组合结束必须关闭命令闸门"
    );
}

#[gpui::test]
fn marked_text_keeps_all_keys_off_the_wire(cx: &mut TestAppContext) {
    let (terminal, mut window, rx, _registry) = open(cx);
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
    let (terminal, mut window, rx, _registry) = open(cx);
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
    let (terminal, mut window, _rx, _registry) = open(cx);
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

/// 回滚搜索经真实按键与 worker 命令运行；任何阶段都不得写 PTY。
#[gpui::test]
fn terminal_search_is_read_only(cx: &mut TestAppContext) {
    let (terminal, mut window, rx, _registry) = open(cx);
    terminal.update(&mut window, |view, _| {
        for index in 0..30 {
            let text = if index % 10 == 0 {
                format!("Needle row {index}\r\n")
            } else {
                format!("plain row {index}\r\n")
            };
            feed(view, text.as_bytes());
        }
    });
    let _ = pty_bytes(&rx);
    let modifiers = Modifiers {
        control: true,
        shift: true,
        ..Default::default()
    };
    press(&mut window, key("f", modifiers));
    assert!(terminal.read_with(&window, |view, _| view.terminal_search.is_some()));

    terminal.update_in(&mut window, |view, window, cx| {
        gpui::EntityInputHandler::replace_text_in_range(view, None, "needle", window, cx);
    });
    terminal.update_in(&mut window, |view, _, _| view.pump_test_worker(&rx));
    window.run_until_parked();
    let (count, selected) = terminal.read_with(&window, |view, _| {
        let search = view.terminal_search.as_ref().unwrap();
        (search.matches.len(), search.selected)
    });
    assert_eq!(count, 3);
    assert_eq!(selected, Some(0));
    assert!(pty_bytes(&rx).is_empty(), "打开与输入不得写 PTY");

    press(&mut window, key("enter", Modifiers::default()));
    assert_eq!(
        terminal.read_with(&window, |view, _| view
            .terminal_search
            .as_ref()
            .unwrap()
            .selected),
        Some(1)
    );
    press(
        &mut window,
        key(
            "enter",
            Modifiers {
                shift: true,
                ..Default::default()
            },
        ),
    );
    assert_eq!(
        terminal.read_with(&window, |view, _| view
            .terminal_search
            .as_ref()
            .unwrap()
            .selected),
        Some(0)
    );
    assert!(pty_bytes(&rx).is_empty(), "前后导航不得写 PTY");

    terminal.update(&mut window, |view, _| feed(view, b"Needle new output\r\n"));
    assert!(
        terminal.read_with(&window, |view, _| {
            view.terminal_search.as_ref().unwrap().matches.is_empty()
        }),
        "新输出到达时旧位置必须立即失效"
    );
    terminal.update_in(&mut window, |view, _, _| view.pump_test_worker(&rx));
    window.run_until_parked();
    let frame = terminal.read_with(&window, |view, _| view.frame.clone().unwrap());
    terminal.update_in(&mut window, |view, _, cx| {
        view.feed_event_for_test(slterm_terminal::event::Event::Frame(frame), cx);
    });
    terminal.update_in(&mut window, |view, _, _| view.pump_test_worker(&rx));
    window.run_until_parked();
    assert_eq!(
        terminal.read_with(&window, |view, _| {
            view.terminal_search.as_ref().unwrap().matches.len()
        }),
        4
    );
    assert!(pty_bytes(&rx).is_empty());

    press(&mut window, key("escape", Modifiers::default()));
    assert!(terminal.read_with(&window, |view, _| view.terminal_search.is_none()));
    press(&mut window, key("f", modifiers));
    assert!(
        terminal.read_with(&window, |view, _| {
            let search = view.terminal_search.as_ref().unwrap();
            search.query.is_empty() && search.matches.is_empty()
        }),
        "关闭重开不得恢复搜索词或位置"
    );
}

/// 新输出帧触发的搜索刷新必须保留当前导航位置，且不得形成滚动回环。
#[gpui::test]
fn frame_driven_search_refresh_preserves_selection(cx: &mut TestAppContext) {
    let (terminal, mut window, rx, _registry) = open(cx);
    terminal.update(&mut window, |view, _| {
        feed(
            view,
            b"needle one
needle two
",
        );
        for index in 0..30 {
            feed(
                view,
                format!(
                    "plain {index}
"
                )
                .as_bytes(),
            );
        }
    });
    press(
        &mut window,
        key(
            "f",
            Modifiers {
                control: true,
                shift: true,
                ..Default::default()
            },
        ),
    );
    terminal.update_in(&mut window, |view, window, cx| {
        gpui::EntityInputHandler::replace_text_in_range(view, None, "needle", window, cx);
    });
    terminal.update_in(&mut window, |view, _, _| view.pump_test_worker(&rx));
    window.run_until_parked();
    terminal.update_in(&mut window, |view, _, _| view.pump_test_worker(&rx));
    // 导航到第二处匹配。
    press(&mut window, key("down", Modifiers::default()));
    let selected_before = terminal.read_with(&window, |view, _| {
        view.terminal_search.as_ref().unwrap().selected
    });
    assert_eq!(selected_before, Some(1));

    // 模拟新输出帧到达：刷新必须保留当前位置。
    let frame = terminal.read_with(&window, |view, _| view.frame.clone().unwrap());
    terminal.update_in(&mut window, |view, _, cx| {
        view.feed_event_for_test(slterm_terminal::event::Event::Frame(frame), cx);
    });
    terminal.update_in(&mut window, |view, _, _| view.pump_test_worker(&rx));
    window.run_until_parked();
    terminal.update_in(&mut window, |view, _, _| view.pump_test_worker(&rx));
    let selected_after = terminal.read_with(&window, |view, _| {
        view.terminal_search.as_ref().unwrap().selected
    });
    assert_eq!(selected_after, Some(1), "新帧刷新不得重置导航位置");
}

/// 集成面：当前匹配滚入视口；切备用屏后不得继续搜索主屏回滚。
#[gpui::test]
fn integration_terminal_search(cx: &mut TestAppContext) {
    let (terminal, mut window, rx, _registry) = open(cx);
    terminal.update(&mut window, |view, _| {
        feed(view, b"Needle oldest\r\n");
        for index in 0..40 {
            feed(view, format!("plain {index}\r\n").as_bytes());
        }
    });
    press(
        &mut window,
        key(
            "f",
            Modifiers {
                control: true,
                shift: true,
                ..Default::default()
            },
        ),
    );
    terminal.update_in(&mut window, |view, window, cx| {
        gpui::EntityInputHandler::replace_text_in_range(view, None, "needle", window, cx);
    });
    terminal.update_in(&mut window, |view, _, _| view.pump_test_worker(&rx));
    window.run_until_parked();
    terminal.update_in(&mut window, |view, _, _| view.pump_test_worker(&rx));
    let visible = terminal.read_with(&window, |view, _| {
        let search = view.terminal_search.as_ref().unwrap();
        let line = search.matches[search.selected.unwrap()].start().line.0;
        let top = -(view.frame.as_ref().unwrap().render().display_offset as i32);
        let bottom = top + view.frame.as_ref().unwrap().render().rows as i32 - 1;
        (top..=bottom).contains(&line)
    });
    assert!(visible, "当前匹配必须滚入可见区域");
    assert!(pty_bytes(&rx).is_empty());

    terminal.update(&mut window, |view, _| {
        feed(view, b"\x1b[?1049hAlt Needle\r\n");
    });
    terminal.update_in(&mut window, |view, _, cx| view.refresh_terminal_search(cx));
    terminal.update_in(&mut window, |view, _, _| view.pump_test_worker(&rx));
    window.run_until_parked();
    let count = terminal.read_with(&window, |view, _| {
        view.terminal_search.as_ref().unwrap().matches.len()
    });
    assert_eq!(count, 1, "备用屏只搜索当前可提供内容");
    assert!(pty_bytes(&rx).is_empty());
}

/// OSC 52 只写不读；聚焦、活跃项目、显式非聚焦授权与单条上限共同门控。
#[gpui::test]
fn terminal_osc52_authorization(cx: &mut TestAppContext) {
    let (terminal, mut window, rx, _registry) = open(cx);
    let store = |terminal: &Entity<TerminalView>, window: &mut VisualTestContext, text: &str| {
        terminal.update_in(window, |view, _, cx| {
            view.feed_event_for_test(
                slterm_terminal::event::Event::ClipboardStore(
                    slterm_terminal::term::ClipboardType::Clipboard,
                    text.to_owned(),
                ),
                cx,
            );
        });
    };
    window.update(|_, cx| cx.write_to_clipboard(ClipboardItem::new_string("sentinel".to_owned())));
    terminal.update_in(&mut window, |view, _, _| {
        view.cursor_window_active = true;
        view.cursor_pane_focused = true;
    });
    store(&terminal, &mut window, "focused");
    assert_eq!(clipboard_text(&mut window).as_deref(), Some("focused"));

    terminal.update_in(&mut window, |view, _, _| view.cursor_pane_focused = false);
    store(&terminal, &mut window, "unfocused-denied");
    assert_eq!(clipboard_text(&mut window).as_deref(), Some("focused"));

    window.update(|_, cx| {
        cx.set_global(Settings {
            osc52_unfocused: true,
            ..Settings::default()
        });
    });
    store(&terminal, &mut window, "unfocused-allowed");
    assert_eq!(
        clipboard_text(&mut window).as_deref(),
        Some("unfocused-allowed")
    );

    terminal.update_in(&mut window, |view, _, cx| {
        view.set_output_visible(false, cx)
    });
    store(&terminal, &mut window, "hidden-denied");
    assert_eq!(
        clipboard_text(&mut window).as_deref(),
        Some("unfocused-allowed")
    );

    terminal.update_in(&mut window, |view, _, cx| {
        view.set_output_visible(true, cx);
        view.cursor_pane_focused = true;
    });
    store(&terminal, &mut window, &"x".repeat(1024 * 1024 + 1));
    assert_eq!(
        clipboard_text(&mut window).as_deref(),
        Some("unfocused-allowed"),
        "超过 1 MiB 不得改剪贴板"
    );

    terminal.update_in(&mut window, |view, _, cx| {
        view.feed_event_for_test(
            slterm_terminal::event::Event::ClipboardLoad(
                slterm_terminal::term::ClipboardType::Clipboard,
                std::sync::Arc::new(|_| panic!("OSC 52 读取不得访问剪贴板")),
            ),
            cx,
        );
    });
    assert!(pty_bytes(&rx).is_empty(), "读取请求不得产生 PTY 应答");
}
