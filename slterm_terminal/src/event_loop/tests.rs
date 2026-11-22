use super::*;

use crate::event::VoidListener;
use crate::term::Config;
use crate::term::test::TermSize;

#[test]
fn animation_snapshots_cannot_observe_a_partial_synchronized_update() {
    use crate::render::{RenderSnapshot, SnapshotConfig};
    let mut term = Term::new(Config::default(), &TermSize::new(20, 2), VoidListener);
    let mut stream = StreamProcessor::default();
    let cfg = SnapshotConfig { rows: 2, cols: 20 };
    stream.feed(&mut term, &VoidListener, b"old");
    let before = RenderSnapshot::capture(&term, &cfg);
    for bytes in [
        b"\x1b[?20".as_slice(),
        b"26h\r",
        b"new",
        b"\x1b[10G",
        b"\x1b[?2026",
    ] {
        stream.feed(&mut term, &VoidListener, bytes);
        // Animation frames read the grid even without a Wakeup.
        let frame = RenderSnapshot::capture(&term, &cfg);
        assert_eq!(
            frame.cursor.as_ref().unwrap().col,
            before.cursor.as_ref().unwrap().col
        );
        assert_eq!(term.grid()[Line(0)][Column(0)].c, 'o');
    }
    stream.feed(&mut term, &VoidListener, b"l");
    assert_eq!(RenderSnapshot::capture(&term, &cfg).cursor.unwrap().col, 9);
    assert_eq!(term.grid()[Line(0)][Column(0)].c, 'n');
    stream.feed(&mut term, &VoidListener, b"\x1b[?2026h\rtimeout");
    assert_eq!(term.grid()[Line(0)][Column(0)].c, 'n');
    stream.stop_sync(&mut term);
    assert_eq!(term.grid()[Line(0)][Column(0)].c, 't');
}

#[test]
fn shell_identity_cwd_and_title_keep_wire_order_across_chunk_boundaries() {
    #[derive(Clone, Default)]
    struct Listener(std::sync::Arc<std::sync::Mutex<Vec<String>>>);
    impl EventListener for Listener {
        fn send_event(&self, event: Event) {
            let label = match event {
                Event::UserVar { value, .. } => format!("shell:{value}"),
                Event::CwdReport(cwd) => format!("cwd:{cwd}"),
                Event::Title(title) => format!("title:{title}"),
                _ => return,
            };
            self.0.lock().unwrap().push(label);
        }
    }
    let bytes = b"\x1b]1337;SetUserVar=slterm_shell=cmVtb3Rl\x07\x1b]7;file://box/remote\x07\x1b]2;remote\x07\x1b]1337;SetUserVar=slterm_shell=bG9jYWw=\x07\x1b]7;file://localhost/local\x07";
    for split in 0..=bytes.len() {
        let listener = Listener::default();
        let size = TermSize::new(80, 24);
        let mut terminal = Term::new(Config::default(), &size, listener.clone());
        let mut stream = StreamProcessor::default();
        stream.feed(&mut terminal, &listener, &bytes[..split]);
        stream.feed(&mut terminal, &listener, &bytes[split..]);
        assert_eq!(
            *listener.0.lock().unwrap(),
            [
                "shell:remote",
                "cwd:/remote",
                "title:remote",
                "shell:local",
                "cwd:/local"
            ],
            "split at {split}"
        );
    }
}

#[test]
fn semantic_input_boundary_survives_chunking_and_scrolling() {
    use crate::index::{Column, Line, Point};

    let bytes = b"\x1b]133;A\x07[first]\r\n>\x1b]133;B\x07pause";
    for split in 0..=bytes.len() {
        let mut terminal = Term::new(Config::default(), &TermSize::new(20, 2), VoidListener);
        let mut stream = StreamProcessor::default();
        stream.feed(&mut terminal, &VoidListener, &bytes[..split]);
        stream.feed(&mut terminal, &VoidListener, &bytes[split..]);
        assert_eq!(
            terminal.slterm_prompt_input_point(),
            Some(Point::new(Line(1), Column(1)))
        );
        stream.feed(&mut terminal, &VoidListener, b"\r\n");
        assert_eq!(
            terminal.slterm_prompt_input_point(),
            Some(Point::new(Line(0), Column(1)))
        );
        stream.feed(&mut terminal, &VoidListener, b"\x1b]133;C\x07");
        assert_eq!(terminal.slterm_prompt_input_point(), None);
    }
}

#[test]
fn input_boundaries_require_a_prompt_and_do_not_survive_reflow_or_reset() {
    let mut terminal = Term::new(Config::default(), &TermSize::new(20, 2), VoidListener);
    let mut stream = StreamProcessor::default();
    stream.feed(&mut terminal, &VoidListener, b"\x1b]133;B\x07");
    assert_eq!(terminal.slterm_prompt_input_point(), None);
    for ending in [b"\x1bc".as_slice(), b"\x1b]133;D;0\x07", b"\x1b]133;A\x07"] {
        stream.feed(
            &mut terminal,
            &VoidListener,
            b"\x1b]133;A\x07\x1b]133;B\x07",
        );
        assert!(terminal.slterm_prompt_input_point().is_some());
        stream.feed(&mut terminal, &VoidListener, ending);
        assert_eq!(terminal.slterm_prompt_input_point(), None);
    }
    stream.feed(
        &mut terminal,
        &VoidListener,
        b"\x1b]133;A\x07\x1b]133;B\x07",
    );
    terminal.resize(TermSize::new(10, 2));
    assert_eq!(terminal.slterm_prompt_input_point(), None);
}

#[test]
fn shell_semantic_events_track_the_active_prompt() {
    let size = TermSize::new(80, 24);
    let mut terminal = Term::new(Config::default(), &size, VoidListener);
    let listener = VoidListener;
    let mut stream = StreamProcessor::default();

    stream.feed(&mut terminal, &listener, b"\x1b]133;A\x07custom prompt :: ");
    assert!(terminal.slterm_prompt_active());

    stream.feed(&mut terminal, &listener, b"\x1b]133;C\x07");
    assert!(!terminal.slterm_prompt_active());

    stream.feed(&mut terminal, &listener, b"\x1b]133;A\x07next prompt :: ");
    assert!(terminal.slterm_prompt_active());

    stream.feed(&mut terminal, &listener, b"\x1b]133;D;0\x07");
    assert!(!terminal.slterm_prompt_active());
}

// ─── DA1/DSR 接管族（旧栈 reader_tests 语义迁移 + 状态机新增）───

#[test]
fn strip_osc_title_bel() {
    assert_eq!(strip_conpty_startup(b"\x1b]0;pwsh\x07"), b"");
}

#[test]
fn strip_clear_screen() {
    assert_eq!(strip_conpty_startup(b"\x1b[2J"), b"");
}

#[test]
fn strip_clear_screen_3j() {
    // CSI 3J（清屏含滚动缓冲）与 2J 一并剥离。
    assert_eq!(strip_conpty_startup(b"\x1b[3J"), b"");
}

#[test]
fn strip_cursor_home() {
    assert_eq!(strip_conpty_startup(b"\x1b[H"), b"");
}

#[test]
fn strip_dsr() {
    assert_eq!(strip_conpty_startup(b"\x1b[6n"), b"");
}

#[test]
fn strip_cursor_visibility() {
    assert_eq!(strip_conpty_startup(b"\x1b[?25h"), b"");
    assert_eq!(strip_conpty_startup(b"\x1b[?25l"), b"");
}

#[test]
fn strip_da1_query_in_startup() {
    // DA1 全量接管：启动窗口内的 DA1 查询一并剥离，不透传。
    assert_eq!(strip_conpty_startup(b"\x1b[c"), b"");
    assert_eq!(strip_conpty_startup(b"\x1b[0c"), b"");
}

#[test]
fn strip_preserves_da1_lookalikes() {
    // DA2（ESC[>c）/ 带参变体（ESC[1c）不是 DA1 查询——不动。
    assert_eq!(strip_conpty_startup(b"\x1b[>c"), b"\x1b[>c");
    assert_eq!(strip_conpty_startup(b"\x1b[1c"), b"\x1b[1c");
}

#[test]
fn preserve_normal_text() {
    let input = b"PS C:\\Users\\test> ";
    assert_eq!(strip_conpty_startup(input), input);
}

#[test]
fn strip_startup_preserve_shell_output() {
    let input = b"\x1b]0;pwsh\x07\x1b[2J\x1b[HPS C:\\> ";
    assert_eq!(strip_conpty_startup(input), b"PS C:\\> ");
}

#[test]
fn preserve_osc7_cwd() {
    let input = b"\x1b]7;file:///C:/Users\x1b\\";
    assert_eq!(strip_conpty_startup(input), input);
}

#[test]
fn strip_preserves_non_title_osc() {
    // OSC 1/3/4/9（非窗口标题类）不被剥离——剥离仅针对 OSC 0/2。
    let cases: [&[u8]; 4] = [
        b"\x1b]1;icon-title\x07",
        b"\x1b]3;prop\x07",
        b"\x1b]4;0;#000000\x07",
        b"\x1b]9;notify\x07",
    ];
    for case in cases {
        assert_eq!(strip_conpty_startup(case), case);
    }
}

#[test]
fn strip_startup_with_large_payload() {
    // >4KB 连续非启动数据完整保留（验证大数据块不被截断）。
    let payload = vec![b'X'; 10000];
    let prefix = b"\x1b]0;pwsh\x07\x1b[2J\x1b[H";
    let mut input = prefix.to_vec();
    input.extend_from_slice(&payload);
    input.extend_from_slice(b"END_MARKER");

    let result = strip_conpty_startup(&input);
    assert_eq!(result.len(), payload.len() + 10);
    assert!(result.ends_with(b"END_MARKER"));
    assert!(!result.starts_with(b"\x1b]"));
}

#[test]
fn strip_preserves_save_cursor() {
    // ESC[s（标准 VT100 保存光标）不被剥离。
    let input = b"\x1b[s hello";
    assert_eq!(strip_conpty_startup(input), input);
}

#[test]
fn strip_preserves_restore_cursor() {
    // ESC[u（标准 VT100 恢复光标）不被剥离。
    let input = b"\x1b[u world";
    assert_eq!(strip_conpty_startup(input), input);
}

#[test]
fn strip_startup_with_read_buffer_boundary() {
    // 读取块契约 64 KiB；接近缓冲上限的剥离不得截断。
    assert_eq!(READ_BUFFER_SIZE, 64 * 1024);
    let payload = vec![b'Y'; READ_BUFFER_SIZE];
    let prefix = b"\x1b[2J\x1b[H";
    let mut input = prefix.to_vec();
    input.extend_from_slice(&payload);
    input.extend_from_slice(b"TAIL");

    let result = strip_conpty_startup(&input);
    assert_eq!(result.len(), READ_BUFFER_SIZE + 4);
    assert!(result.starts_with(b"Y"));
    assert!(result.ends_with(b"TAIL"));
}

// ─── count_da1_queries 纯函数族 ───

#[test]
fn da1_standard_query_detected() {
    assert_eq!(count_da1_queries(b"\x1b[c"), 1);
}

#[test]
fn da1_with_leading_zero_detected() {
    assert_eq!(count_da1_queries(b"\x1b[0c"), 1);
}

#[test]
fn da2_not_detected() {
    assert_eq!(count_da1_queries(b"\x1b[>c"), 0);
}

#[test]
fn plain_text_not_falsely_detected() {
    assert_eq!(count_da1_queries(b"hello [c world"), 0);
}

#[test]
fn xtversion_not_detected() {
    assert_eq!(count_da1_queries(b"\x1b[>0q"), 0);
}

#[test]
fn da1_embedded_in_output_detected() {
    let input = b"prompt> \x1b[c\x1b[0c more output";
    assert_eq!(count_da1_queries(input), 2);
}

// ─── mirror_dsr_query / should_answer_dsr 纯函数族 ───

#[test]
fn dsr_standard_query_detected() {
    // DSR 光标位置查询 ESC[6n（ConPTY VtIo 握手形态）。
    assert!(mirror_dsr_query(b"\x1b[6n"));
}

#[test]
fn dsr_embedded_in_startup_burst_detected() {
    // 启动突发内嵌 DSR（OSC 标题 + 清屏 + DSR 同块）。
    let input = b"\x1b]0;pwsh\x07\x1b[2J\x1b[6n";
    assert!(mirror_dsr_query(input));
}

#[test]
fn dsr_lookalikes_not_detected() {
    // ESC[5n（设备状态）/ ESC[?6n（私有模式）/ ESC[65n 均非握手 DSR。
    assert!(!mirror_dsr_query(b"\x1b[5n"));
    assert!(!mirror_dsr_query(b"\x1b[?6n"));
    assert!(!mirror_dsr_query(b"\x1b[65n"));
    // 不完整前缀（跨块残片形态）不检测——由扣留层接管。
    assert!(!mirror_dsr_query(b"\x1b[6"));
    // 普通文本不误触发。
    assert!(!mirror_dsr_query(b"hello [6n world"));
}

#[test]
fn dsr_answered_only_in_startup_window() {
    // 启动窗口内（drained=false）DSR → 代答 CPR。
    assert!(should_answer_dsr(false, b"\x1b[6n"));
    // 启动窗口外（drained=true）DSR → 不代答：Win11+ 透传 Term 实答；
    // Win10 家族经 apply_output_strip 门控剥离（CPR 应答即 F3 毒键）。
    assert!(!should_answer_dsr(true, b"\x1b[6n"));
    // 窗口内无 DSR → 不代答（绝不盲注——Win10 捆绑 conhost 握手发 DA1
    // 不发 DSR，盲注 CPR 会被解析为 F3 吞键，防复发锚点）。
    assert!(!should_answer_dsr(false, b"\x1b[2J\x1b[H"));
}

// ─── apply_output_strip 纯函数族 ───

#[test]
fn output_strip_drained_passthrough() {
    // 非首轮：无 DA1 的输出原样返回。
    let data = b"normal output";
    let result = apply_output_strip(true, false, data);
    assert_eq!(result, Some(data.to_vec()));
}

#[test]
fn output_strip_drained_strips_da1() {
    // 非首轮：DA1 查询剥离不透传（Ink 启动哨兵场景）。
    assert_eq!(
        apply_output_strip(true, false, b"prompt> \x1b[c more"),
        Some(b"prompt>  more".to_vec())
    );
    assert_eq!(
        apply_output_strip(true, false, b"\x1b[0c rest"),
        Some(b" rest".to_vec())
    );
}

#[test]
fn output_strip_drained_all_da1_returns_none() {
    // 非首轮：整块全是 DA1 → None 无内容可发。
    assert_eq!(apply_output_strip(true, false, b"\x1b[c"), None);
    assert_eq!(apply_output_strip(true, false, b"\x1b[0c"), None);
}

#[test]
fn output_strip_drained_dsr_gated_stripped() {
    // 毒链防复发回归（核心场景）：窗口外 DSR + 门控开启（Win10 家族）→
    // 剥离不透传（旧代码透传 → Term 自答 ESC[1;1R 回灌 stdin →
    // conhost 键事件解析为 F3 → PSReadLine CharacterSearch 吞键）。
    assert_eq!(
        apply_output_strip(true, true, b"prompt> \x1b[6nrest"),
        Some(b"prompt> rest".to_vec())
    );
    // 整块全是 DSR → None 无内容可发。
    assert_eq!(apply_output_strip(true, true, b"\x1b[6n"), None);
    // DA1 与 DSR 同块 → 一并剥离。
    assert_eq!(
        apply_output_strip(true, true, b"\x1b[c\x1b[6nx"),
        Some(b"x".to_vec())
    );
}

#[test]
fn output_strip_drained_dsr_ungated_passthrough() {
    // 门控关闭（Win11+）→ 窗口外 DSR 维持透传（Term 实答真实光标位置，
    // 零回归锁——Win11 主用例行为不变）。
    assert_eq!(
        apply_output_strip(true, false, b"prompt> \x1b[6nrest"),
        Some(b"prompt> \x1b[6nrest".to_vec())
    );
}

#[test]
fn output_strip_first_round_unaffected_by_gate() {
    // 窗口内剥离（strip_conpty_startup 恒含 DSR）在两门控值下行为一致。
    let input = b"\x1b[2J\x1b[6nPS> ";
    assert_eq!(
        apply_output_strip(false, true, input),
        Some(b"PS> ".to_vec())
    );
    assert_eq!(
        apply_output_strip(false, false, input),
        Some(b"PS> ".to_vec())
    );
}

#[test]
fn output_strip_first_round_all_stripped() {
    // 首轮全部为启动序列 → 返回 None（跳过本轮）。
    let result = apply_output_strip(false, false, b"\x1b]0;pwsh\x07\x1b[2J\x1b[H");
    assert_eq!(result, None);
}

#[test]
fn output_strip_first_round_startup_plus_da1_all_stripped() {
    // 防复发回归：启动窗口内「启动序列 + DA1」整块 → 零字节（旧代码 DA1
    // 不在剥离清单 → 透传触发自答回灌）。调用方须在剥离前的原始字节上
    // 做 count_da1_queries 检测；除 bundled 首条宿主 DA1 外，None 分支
    // 不漏答。
    let result = apply_output_strip(false, false, b"\x1b[2J\x1b[c\x1b[H");
    assert_eq!(result, None);
    assert_eq!(count_da1_queries(b"\x1b[2J\x1b[c\x1b[H"), 1);
}

#[test]
fn output_strip_first_round_partial_strip() {
    // 首轮启动序列后跟正常输出 → 剥离前缀。
    let result = apply_output_strip(false, false, b"\x1b[2J\x1b[HPS C:\\> ");
    assert_eq!(result, Some(b"PS C:\\> ".to_vec()));
}

#[test]
fn output_strip_across_buffer_boundary() {
    // 跨缓冲区边界的启动序列剥离——第一轮全为启动序列（None）
    // 不置 drained；第二轮仍有启动序列 + 真实输出，应继续剥离。
    let r1 = apply_output_strip(false, false, b"\x1b]0;pwsh\x07");
    assert_eq!(r1, None);
    let r2 = apply_output_strip(false, false, b"\x1b[2J\x1b[HPS C:\\> ");
    assert_eq!(r2, Some(b"PS C:\\> ".to_vec()));
}

#[test]
fn output_strip_multi_round_all_startup_then_real() {
    // 多轮纯启动序列后出现真实输出——drained 仅在 Some 时才置。
    assert_eq!(apply_output_strip(false, false, b"\x1b]0;pwsh\x07"), None);
    assert_eq!(apply_output_strip(false, false, b"\x1b[2J"), None);
    let r3 = apply_output_strip(false, false, b"\x1b[?25h\x1b[HHello World");
    assert_eq!(r3, Some(b"Hello World".to_vec()));
}

#[test]
fn output_strip_first_round_no_startup_seq() {
    // 首轮无启动序列（如 cmd.exe 场景）→ 原样返回。
    let data = b"Microsoft Windows [Version 10.0]\r\n";
    let result = apply_output_strip(false, false, data);
    assert_eq!(result, Some(data.to_vec()));
}

// ─── strip_da1_queries 纯函数族 ───

#[test]
fn strip_da1_removes_both_forms() {
    assert_eq!(strip_da1_queries(b"\x1b[c"), b"");
    assert_eq!(strip_da1_queries(b"\x1b[0c"), b"");
}

#[test]
fn strip_da1_preserves_surrounding_bytes() {
    assert_eq!(strip_da1_queries(b"ab\x1b[ccd\x1b[0cef"), b"abcdef");
}

#[test]
fn strip_da1_preserves_non_da1_sequences() {
    // DA2 / XTVERSION / 带参变体 / 其它 CSI 均不动。
    assert_eq!(strip_da1_queries(b"\x1b[>c"), b"\x1b[>c");
    assert_eq!(strip_da1_queries(b"\x1b[>0q"), b"\x1b[>0q");
    assert_eq!(strip_da1_queries(b"\x1b[1c"), b"\x1b[1c");
    assert_eq!(strip_da1_queries(b"\x1b[2J"), b"\x1b[2J");
}

#[test]
fn strip_da1_trailing_partial_forwarded_as_is() {
    // 纯函数本身不扣留残片（扣留是 split_trailing_query_prefix 职责）
    // ——尾部落单的 ESC/ESC[ 原样通过，记录两层的职责边界。
    assert_eq!(strip_da1_queries(b"abc\x1b"), b"abc\x1b");
    assert_eq!(strip_da1_queries(b"abc\x1b["), b"abc\x1b[");
}

// ─── strip_dsr_queries 纯函数族（Win10 门控剥离）───

#[test]
fn strip_dsr_removes_exact_form() {
    assert_eq!(strip_dsr_queries(b"\x1b[6n"), b"");
}

#[test]
fn strip_dsr_preserves_surrounding_bytes() {
    assert_eq!(strip_dsr_queries(b"ab\x1b[6ncd"), b"abcd");
}

#[test]
fn strip_dsr_removes_multiple_occurrences() {
    assert_eq!(strip_dsr_queries(b"\x1b[6nx\x1b[6ny"), b"xy");
}

#[test]
fn strip_dsr_preserves_lookalikes() {
    // ESC[5n（设备状态）/ ESC[?6n（私有模式）/ ESC[65n 同族不动——
    // 判别集与 mirror_dsr_query 一致。
    assert_eq!(strip_dsr_queries(b"\x1b[5n"), b"\x1b[5n");
    assert_eq!(strip_dsr_queries(b"\x1b[?6n"), b"\x1b[?6n");
    assert_eq!(strip_dsr_queries(b"\x1b[65n"), b"\x1b[65n");
    assert_eq!(strip_dsr_queries(b"\x1b[2J"), b"\x1b[2J");
}

#[test]
fn strip_dsr_trailing_partial_forwarded_as_is() {
    // 纯函数本身不扣留残片——尾部落单 ESC/ESC[/ESC[6 原样通过。
    assert_eq!(strip_dsr_queries(b"abc\x1b"), b"abc\x1b");
    assert_eq!(strip_dsr_queries(b"abc\x1b[6"), b"abc\x1b[6");
}

// ─── split_trailing_query_prefix 纯函数族 ───

#[test]
fn split_holds_trailing_esc() {
    let (body, pending) = split_trailing_query_prefix(b"abc\x1b".to_vec());
    assert_eq!(body, b"abc");
    assert_eq!(pending, b"\x1b");
}

#[test]
fn split_holds_trailing_esc_bracket() {
    let (body, pending) = split_trailing_query_prefix(b"abc\x1b[".to_vec());
    assert_eq!(body, b"abc");
    assert_eq!(pending, b"\x1b[");
}

#[test]
fn split_holds_trailing_esc_bracket_zero() {
    let (body, pending) = split_trailing_query_prefix(b"abc\x1b[0".to_vec());
    assert_eq!(body, b"abc");
    assert_eq!(pending, b"\x1b[0");
}

#[test]
fn split_holds_trailing_esc_bracket_six() {
    // ESC[6 是 DSR 握手查询前缀——扣留防跨块裂（扣留只延迟不丢字节，
    // ESC[6;..H 等 CUP 前缀同形不误伤，随下块原样拼回）。
    let (body, pending) = split_trailing_query_prefix(b"abc\x1b[6".to_vec());
    assert_eq!(body, b"abc");
    assert_eq!(pending, b"\x1b[6");
}

#[test]
fn split_complete_da1_not_held() {
    // 完整 DA1 查询不是前缀残片——不扣留（交检测/剥离处理）。
    let (body, pending) = split_trailing_query_prefix(b"\x1b[c".to_vec());
    assert_eq!(body, b"\x1b[c");
    assert!(pending.is_empty());
    let (body, pending) = split_trailing_query_prefix(b"\x1b[0c".to_vec());
    assert_eq!(body, b"\x1b[0c");
    assert!(pending.is_empty());
}

#[test]
fn split_non_da1_prefix_not_held() {
    // ESC[1 / ESC[? 等不是 DA1 前缀——不扣留（ESC[1c 非 DA1，不误伤）。
    let (body, pending) = split_trailing_query_prefix(b"abc\x1b[1".to_vec());
    assert_eq!(body, b"abc\x1b[1");
    assert!(pending.is_empty());
}

#[test]
fn split_plain_and_empty_input() {
    let (body, pending) = split_trailing_query_prefix(b"plain".to_vec());
    assert_eq!(body, b"plain");
    assert!(pending.is_empty());
    let (body, pending) = split_trailing_query_prefix(Vec::new());
    assert!(body.is_empty());
    assert!(pending.is_empty());
}

#[test]
fn split_reassembly_closes_cross_chunk_da1() {
    // 防复发回归（跨块形态）：chunk1 = "abc ESC["，chunk2 = "c def"——
    // 拼接后检测命中 + 剥离无泄漏（旧代码半条 ESC[ 送前端跨写入拼合
    // 自答）。
    let (b1, p1) = split_trailing_query_prefix(b"abc\x1b[".to_vec());
    assert_eq!(b1, b"abc");
    assert_eq!(p1, b"\x1b[");
    let mut frame = p1;
    frame.extend_from_slice(b"c def");
    let (b2, p2) = split_trailing_query_prefix(frame);
    assert!(p2.is_empty());
    assert_eq!(count_da1_queries(&b2), 1, "拼接后应检测到 DA1 → 触发代答");
    assert_eq!(strip_da1_queries(&b2), b" def", "剥离后前端无泄漏");
}

#[test]
fn split_reassembly_closes_cross_chunk_dsr() {
    // 跨块 DSR 防裂回归：chunk1 尾 = "ESC[6"，chunk2 头 = "n rest"——
    // 拼接后 mirror_dsr_query 命中（漏扣留则半条 ESC[6 送前端跨写入
    // 拼合后以自身位置实答，后端代答丢失）。
    let (b1, p1) = split_trailing_query_prefix(b"abc\x1b[6".to_vec());
    assert_eq!(b1, b"abc");
    assert_eq!(p1, b"\x1b[6");
    let mut frame = p1;
    frame.extend_from_slice(b"n rest");
    let (b2, p2) = split_trailing_query_prefix(frame);
    assert!(p2.is_empty());
    assert!(mirror_dsr_query(&b2), "拼接后应检测到 DSR → 触发 CPR 代答");
}

#[test]
fn split_reassembly_closes_cross_chunk_dsr_strip() {
    // 跨块 DSR 防裂（Win10 门控剥离形态）：拼接后剥离无泄漏——漏扣留
    // 则半条 ESC[6 送前端跨写入拼合自答回灌 → F3 吞键复现。
    let (_b1, p1) = split_trailing_query_prefix(b"abc\x1b[6".to_vec());
    let mut frame = p1;
    frame.extend_from_slice(b"n rest");
    let (b2, p2) = split_trailing_query_prefix(frame);
    assert!(p2.is_empty());
    assert_eq!(
        strip_dsr_queries(&b2),
        b" rest",
        "门控剥离后前端无 DSR 泄漏"
    );
}

// ─── inject_da1_response / inject_cpr_response 动作级（Vec 直当 writer）───

#[test]
fn inject_da1_response_writes_vt420_identity() {
    let mut buf: Vec<u8> = Vec::new();
    inject_da1_response(&mut buf);
    assert_eq!(buf, b"\x1b[?64;22c");
}

#[test]
fn inject_da1_response_every_query_answered() {
    // 防复发回归：每次查询都应答（旧「每会话一次」语义下第二次查询
    // 无应答——接管后前端不再自答，后端漏答 = 应用干等）。
    let mut buf: Vec<u8> = Vec::new();
    inject_da1_response(&mut buf);
    inject_da1_response(&mut buf);
    assert_eq!(buf, b"\x1b[?64;22c\x1b[?64;22c");
}

#[test]
fn inject_cpr_response_writes_home_position() {
    // DSR 握手代答：启动期光标恒在 1;1。
    let mut buf: Vec<u8> = Vec::new();
    inject_cpr_response(&mut buf);
    assert_eq!(buf, b"\x1b[1;1R");
}

// ─── strip_and_answer 状态机族（接管合并防回归，新增）───

#[test]
fn strip_and_answer_first_block_all_startup_keeps_window_open() {
    // 整块启动序列 → None 且不置 drained，窗口保持开放。
    let mut sp = StreamProcessor::default();
    let mut w: Vec<u8> = Vec::new();
    assert_eq!(sp.strip_and_answer(b"\x1b]0;pwsh\x07\x1b[2J", &mut w), None);
    assert!(!sp.startup_drained);
    // 第二轮仍按窗口内语义剥离。
    let out = sp.strip_and_answer(b"\x1b[HPS> ", &mut w);
    assert_eq!(out, Some(b"PS> ".to_vec()));
    assert!(sp.startup_drained);
}

#[test]
fn strip_and_answer_da1_answered_and_stripped_every_time() {
    // 每查必答 + 不透传：首轮 DA1 → 代答 + 剥离；次轮再来一条仍答。
    let mut sp = StreamProcessor::default();
    let mut w: Vec<u8> = Vec::new();
    assert_eq!(
        sp.strip_and_answer(b"\x1b[2J\x1b[chello", &mut w),
        Some(b"hello".to_vec())
    );
    assert_eq!(
        sp.strip_and_answer(b"\x1b[c world", &mut w),
        Some(b" world".to_vec())
    );
    assert_eq!(w, b"\x1b[?64;22c\x1b[?64;22c");
}

#[test]
fn strip_and_answer_suppresses_only_bringup_da1() {
    let mut sp = StreamProcessor {
        bringup_da1_pending: true,
        ..Default::default()
    };
    let mut w: Vec<u8> = Vec::new();
    assert_eq!(sp.strip_and_answer(b"\x1b[6n\x1b[c\x1b[0c", &mut w), None);
    assert_eq!(w, b"\x1b[?64;22c\x1b[1;1R", "首条宿主 DA1 不得迟答");
    w.clear();
    assert_eq!(sp.strip_and_answer(b"\x1b[c\x1b[0c", &mut w), None);
    assert_eq!(w, b"\x1b[?64;22c\x1b[?64;22c", "逐条代答");
}

#[test]
fn strip_and_answer_dsr_answered_only_in_window() {
    // 启动窗口内 DSR → CPR 代答且剥离；窗口外再来 DSR → 不答。
    let mut sp = StreamProcessor::default();
    let mut w: Vec<u8> = Vec::new();
    assert_eq!(
        sp.strip_and_answer(b"\x1b[6nhi", &mut w),
        Some(b"hi".to_vec())
    );
    assert_eq!(w, b"\x1b[1;1R");
    let _ = sp.strip_and_answer(b"x\x1b[6ny", &mut w);
    assert_eq!(w, b"\x1b[1;1R", "窗口外 DSR 不再代答");
}

#[test]
fn strip_and_answer_dsr_outside_window_gated_by_strip_dsr() {
    // 门控开启（Win10 家族）：窗口外 DSR 剥离不透传。
    let mut sp = StreamProcessor::default();
    sp.set_strip_dsr(true);
    let mut w: Vec<u8> = Vec::new();
    let _ = sp.strip_and_answer(b"ready", &mut w);
    assert_eq!(
        sp.strip_and_answer(b"a\x1b[6nb", &mut w),
        Some(b"ab".to_vec())
    );

    // 门控关闭（Win11+）：窗口外 DSR 透传交 Term 实答。
    let mut sp = StreamProcessor::default();
    let mut w: Vec<u8> = Vec::new();
    let _ = sp.strip_and_answer(b"ready", &mut w);
    assert_eq!(
        sp.strip_and_answer(b"a\x1b[6nb", &mut w),
        Some(b"a\x1b[6nb".to_vec())
    );
}

#[test]
fn strip_and_answer_cross_chunk_da1_reassembly() {
    // 跨块防裂：chunk1 尾 ESC[ → None + 扣留；chunk2 拼回后检测命中
    // 代答 + 剥离（半条序列绝不送 parser）。
    let mut sp = StreamProcessor::default();
    let mut w: Vec<u8> = Vec::new();
    assert_eq!(
        sp.strip_and_answer(b"abc\x1b[", &mut w),
        Some(b"abc".to_vec())
    );
    assert_eq!(
        sp.strip_and_answer(b"c def", &mut w),
        Some(b" def".to_vec())
    );
    assert_eq!(w, b"\x1b[?64;22c", "拼回后 DA1 检测命中并代答");
}

#[test]
fn strip_and_answer_eof_pending_flush() {
    // EOF 冲刷：扣留残片原样吐出，不丢字节。
    let mut sp = StreamProcessor::default();
    let mut w: Vec<u8> = Vec::new();
    let _ = sp.strip_and_answer(b"tail\x1b[6", &mut w);
    assert_eq!(sp.take_pending_query(), b"\x1b[6");
    assert!(sp.take_pending_query().is_empty(), "冲刷后无残留");
}

/// 名额泄漏回归守卫：裸 `JoinHandle` drop = detach，线程返回值
/// (EventLoop 含 Pty) 永存、名额泄漏。`EventLoopHandle` 的 Drop
/// 兜底 shutdown+join——不显式 join 时 Pty 也必须随返回值析构。
#[test]
fn handle_drop_without_join_reclaims_pty() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct DropCountPty {
        drops: Arc<AtomicUsize>,
        reader: io::Empty,
        writer: io::Sink,
    }

    impl Drop for DropCountPty {
        fn drop(&mut self) {
            self.drops.fetch_add(1, Ordering::SeqCst);
        }
    }

    impl tty::EventedReadWrite for DropCountPty {
        type Reader = io::Empty;
        type Writer = io::Sink;

        unsafe fn register(
            &mut self,
            _: &Arc<Poller>,
            _: PollingEvent,
            _: PollMode,
        ) -> io::Result<()> {
            Ok(())
        }

        fn reregister(&mut self, _: &Arc<Poller>, _: PollingEvent, _: PollMode) -> io::Result<()> {
            Ok(())
        }

        fn deregister(&mut self, _: &Arc<Poller>) -> io::Result<()> {
            Ok(())
        }

        fn reader(&mut self) -> &mut Self::Reader {
            &mut self.reader
        }

        fn writer(&mut self) -> &mut Self::Writer {
            &mut self.writer
        }
    }

    impl tty::EventedPty for DropCountPty {
        fn next_child_event(&mut self) -> Option<tty::ChildEvent> {
            None
        }
    }

    impl event::OnResize for DropCountPty {
        fn on_resize(&mut self, _: WindowSize) {}
    }

    let drops = Arc::new(AtomicUsize::new(0));
    let pty = DropCountPty {
        drops: drops.clone(),
        reader: io::empty(),
        writer: io::sink(),
    };
    let size = TermSize::new(20, 2);
    let terminal = Arc::new(FairMutex::new(Term::new(
        Config::default(),
        &size,
        VoidListener,
    )));
    let event_loop = EventLoop::new(terminal, VoidListener, pty, false).unwrap();

    let handle = event_loop.spawn();
    drop(handle); // 不显式 join——Drop 兜底必须取回返回值。

    assert_eq!(drops.load(Ordering::SeqCst), 1, "Pty 未随返回值析构");
}
