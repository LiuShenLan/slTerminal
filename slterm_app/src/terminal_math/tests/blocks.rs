use super::*;

// 块级公式的硬换行、空行桥接与截图形态恢复。

/// Agent TUI 的硬换行：块级形态（`\[ \]`、裸 `[`、`$$`）保留原有恢复规则。
#[test]
fn screenshot_display_formulas_survive_agent_hard_wraps() {
    for rows in [
        vec![r"\[\displaystyle \frac{d}{dx}\sin", r"x=\cos x\]"],
        vec!["$$", r"\lim_{x\to0}\frac{\sin", r"x}{x}=1", "$$"],
    ] {
        let overlays = scan_grid(&TextGrid::from_rows(&rows));
        assert!(!overlays.is_empty(), "expected wrapped formula in {rows:?}");
        for overlay in overlays {
            compile_formula(&overlay.source, true, 18.0, 1.0, DEFAULT_LIMITS).unwrap_or_else(
                |error| panic!("wrapped formula failed: {:?}: {error:?}", overlay.source),
            );
        }
    }
    // 同一批内容的行内形态被硬换行拆开时保持原文，不再跨行拼接。
    assert!(
        scan_grid(&TextGrid::from_rows(&[
            r"• $\lim_{x\to0}\frac{\sin",
            r"x}{x}=1$，$PV=nRT$",
        ]))
        .iter()
        .all(|overlay| !overlay.source.contains("lim")),
        "行内公式不得跨真实换行拼接"
    );
}

#[test]
fn display_math_can_cross_hard_terminal_rows() {
    assert_eq!(
        sources(&["answer:", "$$", r"\frac{1}{2} + x^2", "$$", "done"]),
        vec![(r"\frac{1}{2} + x^2".into(), true)]
    );
    assert_eq!(
        sources(&[r"\[\sum_{i=1}^n i\]"]),
        vec![(r"\sum_{i=1}^n i".into(), true)]
    );

    let multiline = sources(&[
        "$$",
        r"\begin{aligned}",
        r"f(x) &= x^2 \\",
        r"g(x) &= x+1",
        r"\end{aligned}",
        "$$",
    ]);
    assert_eq!(multiline.len(), 1);
    assert!(multiline[0].0.contains('\n'));
    assert!(multiline[0].1);
}

#[test]
fn stray_display_opener_in_prose_does_not_swallow_following_formulas() {
    let screen = [
        "行内，$$,\\[等公式都输出一些",
        "",
        "• 行内公式：质能方程 (E = mc^2)，二次方程的求根公式是 (x=\\frac{-b\\pm\\sqrt{b^2-4ac}}{2a})。",
        "单美元符号行内公式：欧拉公式 $e^{ix}=\\cos x+i\\sin x$。",
        "双美元符号块级公式：",
        "$$",
        "\\int_{-\\infty}^{+\\infty} e^{-x^2},dx=\\sqrt{\\pi}",
        "$$",
        "方括号块级公式：",
        "\\[",
        "\\sum_{n=1}^{\\infty}\\frac{1}{n^2}=\\frac{\\pi^2}{6}",
        "\\]",
        "带编号的公式：",
        "[",
        "\\boxed{\\nabla\\cdot\\mathbf{E}=\\frac{\\rho}{\\varepsilon_0}}",
        "\\tag{1}",
        "]",
    ];
    let expected = vec![
        ("E = mc^2".into(), false),
        (r"x=\frac{-b\pm\sqrt{b^2-4ac}}{2a}".into(), false),
        (r"e^{ix}=\cos x+i\sin x".into(), false),
        (
            r"\int_{-\infty}^{+\infty} e^{-x^2},dx=\sqrt{\pi}".into(),
            true,
        ),
        (
            r"\sum_{n=1}^{\infty}\frac{1}{n^2}=\frac{\pi^2}{6}".into(),
            true,
        ),
        (
            // 块内行断在抽取后保留；与下方 padding 一起折叠，因为 TeX 把
            // 任意连续空白视作相同。
            concat!(
                r"\boxed{\nabla\cdot\mathbf{E}=\frac{\rho}{\varepsilon_0}}",
                " ",
                r"\tag{1}",
            )
            .into(),
            true,
        ),
    ];

    // `extract` 只裁剪整个跨度，多行源码仍会在换行前携带每行空白 padding。
    // 空白对 TeX 无意义，因此按折叠后的空白比较，而不是把终端宽度钉进期望。
    let extracted: Vec<(String, bool)> = sources(&screen)
        .into_iter()
        .map(|(source, display)| {
            (
                source.split_whitespace().collect::<Vec<_>>().join(" "),
                display,
            )
        })
        .collect();
    assert_eq!(extracted, expected);
    assert!(
        sources(&screen)[5].0.contains('\n'),
        "the block keeps its real row break"
    );
    assert!(
        scan_grid(&TextGrid::from_rows(&screen))
            .iter()
            .flat_map(|overlay| overlay.spans.iter())
            .all(|span| span.row != 0),
        "prose on the first row must stay literal",
    );
}

/// `cat math.txt`：块的开头 `$$` 滚出视口顶部之后，视口里第一个 `$$` 是
/// **闭合**。它曾经跟下面那个块的定界符配对，把 27 行正文、三个方括号块
/// 连同末尾的 `$$` 块吞成一条候选——而且编译成功，于是整片区域被一张渲染
/// 图盖掉，中文正文以数学字形出现在图里。
#[test]
fn orphan_display_closer_at_the_viewport_top_stops_at_the_blank_row() {
    let screen = [
        "  $$",
        "",
        "  方括号块级公式：",
        "",
        "  [",
        r"  \sum_{n=1}^{\infty}\frac{1}{n^2}=\frac{\pi^2}{6}",
        "  ]",
        "",
        "  带编号的公式：",
        "",
        "  [",
        r"  \boxed{\nabla\cdot\mathbf{E}=\frac{\rho}{\varepsilon_0}}",
        r"  \tag{1}",
        "  ]",
        "",
        "",
        r"$$(F, D_{\text{few}}) \xrightarrow{\text{Prompting}} \boxed{C}",
        r"  \boxed{(F_{\text{ref}}, F_{\text{neg}})} \xrightarrow{\text{ToT Search}}$$",
    ];
    let expected = [
        r"\sum_{n=1}^{\infty}\frac{1}{n^2}=\frac{\pi^2}{6}",
        r"\boxed{\nabla\cdot\mathbf{E}=\frac{\rho}{\varepsilon_0}} \tag{1}",
        concat!(
            r"(F, D_{\text{few}}) \xrightarrow{\text{Prompting}} \boxed{C} ",
            r"\boxed{(F_{\text{ref}}, F_{\text{neg}})} \xrightarrow{\text{ToT Search}}",
        ),
    ];

    let extracted = sources(&screen);
    let collapsed: Vec<String> = extracted
        .iter()
        .map(|(source, _)| source.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect();
    assert_eq!(collapsed, expected, "正文和孤立闭合必须保持原文");
    assert!(extracted.iter().all(|(_, display)| *display));

    // 孤立闭合要留成 pending，历史回看才有机会把滚出去的真公式配回来。
    let scan = scan_grid_result(&TextGrid::from_rows(&screen));
    assert!(
        matches!(
            scan.unmatched_display,
            Some((position, DisplayDelimiterKind::Dollars)) if position.row == 0
        ),
        "orphan closer must stay pending, got {:?}",
        scan.unmatched_display
    );
}

/// Codex 的形状，按源码推导而非截图：`codex-rs/tui/src/markdown_render.rs`
/// 只开 strikethrough/tables，不认识 `$$`；块里以 `- ` / `+ ` / `* ` 起行
/// 的公式行被 pulldown-cmark 当成打断段落的列表项，`start_list` 在前面插
/// 一行空行，`start_item` 把记号一律画成 `- `，后续行（含闭合 `$$`）作为
/// 列表项续行缩进两格。「空行终止搜索」的规则会把这些块全判成孤立开头。
#[test]
fn tui_paragraph_gap_inside_a_standalone_display_block_is_bridged() {
    let cases: [(&[&str], &str); 5] = [
        (
            &[
                "$$",
                r"R_{\mu\nu} - \frac{1}{2} R g_{\mu\nu}",
                "",
                r"- \Lambda g_{\mu\nu} = \frac{8\pi G}{c^4} T_{\mu\nu}",
                "  $$",
            ],
            r"R_{\mu\nu} - \frac{1}{2} R g_{\mu\nu} - \Lambda g_{\mu\nu} = \frac{8\pi G}{c^4} T_{\mu\nu}",
        ),
        (
            &[
                "  $$",
                r"  i\hbar \frac{\partial}{\partial t} \Psi",
                "",
                r"  - \frac{\hbar^2}{2m} \nabla^2 \Psi + V \Psi = 0",
                "    $$",
            ],
            r"i\hbar \frac{\partial}{\partial t} \Psi - \frac{\hbar^2}{2m} \nabla^2 \Psi + V \Psi = 0",
        ),
        // 列表项之后还有普通续行：`\end{aligned}` 和闭合都缩进两格。
        (
            &[
                "$$",
                r"\begin{aligned}",
                r"F &= ma \\",
                "",
                r"- \nabla V &= m \ddot{x}",
                r"  \end{aligned}",
                "  $$",
            ],
            r"\begin{aligned} F &= ma \\ - \nabla V &= m \ddot{x} \end{aligned}",
        ),
        // 两次被切：块里有两行以减号开头。
        (
            &[
                "$$",
                r"x^2",
                "",
                r"- 2xy",
                "",
                r"- y^2 = (x - y)^2 - 2y^2",
                "  $$",
            ],
            r"x^2 - 2xy - y^2 = (x - y)^2 - 2y^2",
        ),
        // 没有反斜杠命令也要认：`a^2` 的上标就是证据。
        (
            &["$$", r"a^2 + b^2", "", r"- c^2 = 0", "  $$"],
            r"a^2 + b^2 - c^2 = 0",
        ),
    ];

    for (screen, expected) in cases {
        let extracted = sources(screen);
        assert_eq!(
            extracted.len(),
            1,
            "{screen:?} must yield one display formula"
        );
        let (source, display) = &extracted[0];
        assert!(display);
        assert_eq!(
            source.split_whitespace().collect::<Vec<_>>().join(" "),
            expected
        );
        compile_formula(source, true, 18.0, 1.0, DEFAULT_LIMITS)
            .unwrap_or_else(|error| panic!("{expected} must compile: {error:?}"));
        let scan = scan_grid_result(&TextGrid::from_rows(screen));
        assert!(
            scan.unmatched_display.is_none(),
            "{screen:?} left a pending delimiter"
        );
    }
}

/// 桥接空行的四道闸，每道各一个反例。松开任何一道，`cat math.txt`
/// 那种孤立闭合就会重新把下面的正文吞进公式。
#[test]
fn display_block_gap_bridging_stops_at_prose_double_gaps_and_inline_closers() {
    // 空行后面是正文：没有数学证据，搜索到此为止。
    let prose: &[&str] = &["$$", r"x^2", "", "所以上式成立。", "$$"];
    // 连续两行空行是真正的分段，不是 TUI 塞的列表间距。
    let double_gap: &[&str] = &["$$", r"x^2", "", "", r"- y^2", "  $$"];
    // 跨过空行之后闭合必须独占一行。
    let inline_closer: &[&str] = &["$$", r"x^2", "", r"- y^2 = z$$", "后文"];
    // 开头不独占一行（单行块被硬折行）不享受桥接。
    let inline_opener: &[&str] = &[r"$$x^2 +", "", r"- y^2", "  $$"];
    // 开头后面紧跟空行：块里还没有内容，这是孤立闭合的形状。
    let empty_head: &[&str] = &["$$", "", r"- y^2", "  $$"];
    for screen in [prose, double_gap, inline_closer, inline_opener, empty_head] {
        let scan = scan_grid_result(&TextGrid::from_rows(screen));
        assert!(
            scan.overlays.is_empty(),
            "{screen:?} must not bridge the gap, got {:?}",
            scan.overlays
                .iter()
                .map(|overlay| overlay.source.as_ref())
                .collect::<Vec<_>>()
        );
        assert!(
            scan.unmatched_display.is_some(),
            "{screen:?} must stay pending"
        );
    }
}

/// 桥接有行预算：闭合离开头太远就当没有。预算和裸 `[` 块共用同一个数。
#[test]
fn display_block_gap_bridging_has_a_row_budget() {
    // 闭合落在第 DISPLAY_BLOCK_GAP_SEARCH_ROWS + 1 行：超出一行。
    let mut screen: Vec<String> = vec!["$$".into(), r"x^2".into(), String::new()];
    screen.extend((0..DISPLAY_BLOCK_GAP_SEARCH_ROWS - 2).map(|index| format!("- y_{index}")));
    screen.push("  $$".into());
    let rows: Vec<&str> = screen.iter().map(String::as_str).collect();
    assert!(
        sources(&rows).is_empty(),
        "closer past the row budget must not pair"
    );

    // 少一行内容，闭合正好落在预算内。
    screen.truncate(screen.len() - 2);
    screen.push("  $$".into());
    let rows: Vec<&str> = screen.iter().map(String::as_str).collect();
    assert_eq!(
        sources(&rows).len(),
        1,
        "closer inside the row budget pairs"
    );
}

/// 同一个渲染器把 `\[` 吃成 `[`，再用同一手法切块：裸方括号块走同样的桥。
#[test]
fn tui_paragraph_gap_inside_a_bare_bracket_block_is_bridged() {
    let screen = [
        "[",
        r"\nabla \cdot \mathbf{E} = \frac{\rho}{\varepsilon_0}",
        "",
        r"- \nabla \times \mathbf{B} = 0",
        "  ]",
    ];
    let extracted = sources(&screen);
    assert_eq!(extracted.len(), 1);
    assert!(extracted[0].1);
    assert_eq!(
        extracted[0]
            .0
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" "),
        r"\nabla \cdot \mathbf{E} = \frac{\rho}{\varepsilon_0} - \nabla \times \mathbf{B} = 0",
    );

    // 反例同 `$$`：空行后是正文就停。
    let prose = [
        "[",
        r"\nabla \cdot \mathbf{E} = 0",
        "",
        "其中 E 是电场。",
        "  ]",
    ];
    assert!(sources(&prose).is_empty());
}

/// 用户 09-03 截图（图 #10，Claude Code 输出）里七个 `$$` 块在终端网格上的
/// 原样。Claude Code 的 Markdown 渲染器留下两种形状：
/// * 独占一行的 `=` 是 setext 标题下划线，被吃掉后只剩一行空行（傅里叶、
///   Attention）；
/// * 反斜杠转义被消费：`\\` 变成 `\`（cases / pmatrix / aligned 的行尾），
///   `\,` 变成 `,`（高斯、傅里叶）。
///
/// 前者的 `=` 在网格里已不存在，这里只要求块被完整配对并能编译；后者的
/// 行尾单 `\` 必须仍能编译成换行。
#[test]
fn claude_code_display_blocks_from_screenshot_pair_and_compile() {
    let cases: [(&[&str], &str); 7] = [
        (&["$$", r"e^{i\pi}+1=0", "$$"], r"e^{i\pi}+1=0"),
        (
            &[
                "$$",
                r"\int_{-\infty}^{+\infty} e^{-x^2},dx=\sqrt{\pi}",
                "$$",
            ],
            r"\int_{-\infty}^{+\infty} e^{-x^2},dx=\sqrt{\pi}",
        ),
        (
            &[
                "$$",
                r"\widehat{f}(\xi)",
                "",
                r"\int_{-\infty}^{+\infty}",
                r"f(x)e^{-2\pi i x\xi},dx",
                "$$",
            ],
            r"\widehat{f}(\xi) \int_{-\infty}^{+\infty} f(x)e^{-2\pi i x\xi},dx",
        ),
        (
            &[
                "$$",
                r"\operatorname{Attention}(Q,K,V)",
                "",
                r"\operatorname{softmax}\left(",
                r"\frac{QK^{\mathsf T}}{\sqrt{d_k}}",
                r"\right)V",
                "$$",
            ],
            r"\operatorname{Attention}(Q,K,V) \operatorname{softmax}\left( \frac{QK^{\mathsf T}}{\sqrt{d_k}} \right)V",
        ),
        (
            &[
                "$$",
                r"\phi(x)=",
                r"\begin{cases}",
                r"x, & x\ge 0,\",
                r"\alpha(e^x-1), & x<0.",
                r"\end{cases}",
                "$$",
            ],
            r"\phi(x)= \begin{cases} x, & x\ge 0,\ \alpha(e^x-1), & x<0. \end{cases}",
        ),
        (
            &[
                "$$",
                "A=",
                r"\begin{pmatrix}",
                r"a_{11} & a_{12} & \cdots & a_{1n}\",
                r"a_{21} & a_{22} & \cdots & a_{2n}\",
                r"\vdots & \vdots & \ddots & \vdots\",
                r"a_{m1} & a_{m2} & \cdots & a_{mn}",
                r"\end{pmatrix}",
                "$$",
            ],
            concat!(
                r"A= \begin{pmatrix} a_{11} & a_{12} & \cdots & a_{1n}\ ",
                r"a_{21} & a_{22} & \cdots & a_{2n}\ \vdots & \vdots & \ddots & \vdots\ ",
                r"a_{m1} & a_{m2} & \cdots & a_{mn} \end{pmatrix}",
            ),
        ),
        (
            &[
                "$$",
                r"\begin{aligned}",
                r"(a+b)^3",
                r"&=(a+b)(a+b)^2\",
                r"&=(a+b)(a^2+2ab+b^2)\",
                r"&=a^3+3a^2b+3ab^2+b^3",
                r"\end{aligned}",
                "$$",
            ],
            concat!(
                r"\begin{aligned} (a+b)^3 &=(a+b)(a+b)^2\ &=(a+b)(a^2+2ab+b^2)\ ",
                r"&=a^3+3a^2b+3ab^2+b^3 \end{aligned}",
            ),
        ),
    ];

    for (screen, expected) in cases {
        // Claude Code 把回答整体缩进两格；缩进与否都必须配对。
        let indented: Vec<String> = screen.iter().map(|row| format!("  {row}")).collect();
        let indented: Vec<&str> = indented.iter().map(String::as_str).collect();
        for rows in [screen, indented.as_slice()] {
            let extracted = sources(rows);
            assert_eq!(
                extracted.len(),
                1,
                "{rows:?} must yield one display formula"
            );
            let (source, display) = &extracted[0];
            assert!(display);
            assert_eq!(
                source.split_whitespace().collect::<Vec<_>>().join(" "),
                expected
            );
            compile_formula(source, true, 18.0, 1.0, DEFAULT_LIMITS)
                .unwrap_or_else(|error| panic!("{expected} must compile: {error:?}"));
            let scan = scan_grid_result(&TextGrid::from_rows(rows));
            assert!(
                scan.unmatched_display.is_none(),
                "{rows:?} left a pending delimiter"
            );
        }
    }
}

#[test]
fn long_display_formula_with_trailing_literal_dollar_remains_renderable() {
    let source = concat!(
        r"(F, D_{few}) \xrightarrow{\text{Prompting}} \boxed{C} ",
        r"\xrightarrow{\text{MultiAgent}} \boxed{(F_{ref}, F_{neg})} ",
        r"\xrightarrow{\text{ToT Search}} ",
    );
    let line = format!("$${source}$$$");

    assert_eq!(sources(&[&line]), vec![(source.trim_end().into(), true)]);
    let layout = compile_formula(source, true, 18.0, 1.0, DEFAULT_LIMITS)
        .expect("the long display formula must reach native layout");
    assert!(layout.metrics.width > 0.0 && layout.metrics.height > 0.0);
}

#[test]
fn screenshot_cli_formulas_reach_the_shared_compiler() {
    let samples = [
        sources(&["$$", r"x=\frac{-b\pm\sqrt{b^2-4ac}}{2a}", "$$"]),
        sources(&[
            "$$",
            r"f(x)=\begin{cases}",
            "x^2,&x\\geq 0\\",
            r"-x,&x<0",
            r"\end{cases}",
            "$$",
        ]),
        sources(&[
            "$$",
            r"A=\begin{pmatrix}",
            r"1&amp;2&amp;3\&amp;nbsp;",
            r"4&amp;5&amp;6\&amp;#160;",
            r"7&amp;8&amp;9",
            r"\end{pmatrix}",
            "$$",
        ]),
    ];

    for extracted in samples {
        let [(source, true)] = extracted.as_slice() else {
            panic!("expected one display formula, got {extracted:?}");
        };
        let layout = compile_formula(source, true, 18.0, 1.0, DEFAULT_LIMITS)
            .unwrap_or_else(|error| panic!("CLI formula failed: {source:?}: {error:?}"));
        assert!(layout.metrics.width > 0.0 && layout.metrics.height > 0.0);
    }
}

#[test]
fn assistant_aligned_formula_with_row_spacing_reaches_native_layout() {
    let source = r"\begin{aligned}
\Psi(x,t)
&=
\sum_{n=1}^{\infty}
c_n
\sqrt{\frac{2}{L}}
\sin\left(\frac{n\pi x}{L}\right)
\exp\left(-\frac{i n^2\pi^2\hbar}{2mL^2}t\right), \\[6pt]
\int_{0}^{L}\left|\Psi(x,t)\right|^2\,dx
&= 1,
\qquad
E_n=\frac{n^2\pi^2\hbar^2}{2mL^2}, \\[6pt]
\mathbf{A}^{-1}
&=
\frac{1}{ad-bc}
\begin{bmatrix}
d & -b \\
-c & a
\end{bmatrix},
\qquad ad-bc\ne 0, \\[6pt]
\lim_{N\to\infty}
\sum_{k=1}^{N}\frac{(-1)^{k+1}}{k^2}
&=
\frac{\pi^2}{12},
\qquad
\int_{-\infty}^{\infty}e^{-x^2}\,dx=\sqrt{\pi}.
\end{aligned}";

    let layout = compile_formula(source, true, 18.0, 1.0, DEFAULT_LIMITS)
        .unwrap_or_else(|error| panic!("formula compile failed: {error:?}"));

    assert!(!layout.glyphs.is_empty());
}
