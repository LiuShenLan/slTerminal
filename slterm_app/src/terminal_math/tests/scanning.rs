use super::*;

// 定界符识别、内容证据与行内折行恢复。

#[test]
fn recognizes_cli_math_delimiters_and_utf8_prose() {
    assert_eq!(
        sources(&[r"中文 \(x^2+y^2=z^2\) and $\alpha+1$"]),
        vec![("x^2+y^2=z^2".into(), false), (r"\alpha+1".into(), false)]
    );
}

#[test]
fn agent_reasoning_style_is_distinct_from_final_answer_style() {
    let normal = rgb(240, 240, 240);

    let codex_reasoning = overlay_with_source_style(Flags::DIM, normal);
    assert!(formula_uses_reasoning_style(&codex_reasoning));

    let final_answer = overlay_with_source_style(Flags::empty(), normal);
    assert!(!formula_uses_reasoning_style(&final_answer));
}

/// 美元定界符按数学片段恢复硬折行；BRE 也会使用的反斜杠括号仍只跨软折行。
#[test]
fn explicit_inline_formulas_keep_their_delimiter_specific_wrap_boundaries() {
    for mut grid in [
        TextGrid::from_rows(&["$e^    ", r"{i\pi}+1=0$"]),
        TextGrid::from_rows(&[r"\(e^    ", r"{i\pi}+1=0\)"]),
    ] {
        grid.wrapped[0] = true;
        let overlays = scan_grid(&grid);
        assert_eq!(overlays.len(), 1, "软换行内要接起来");
        assert!(overlays[0].source.contains("e^"));
        assert!(overlays[0].source.contains(r"{i\pi}+1=0"));

        grid.wrapped[0] = false;
        let is_dollar = grid.character(GridPosition { row: 0, column: 0 }) == Some('$');
        assert_eq!(scan_grid(&grid).len(), usize::from(is_dollar));
    }
}

#[test]
fn compact_equations_use_the_same_rule_for_every_standard_delimiter() {
    let expected_inline = vec![("E = mc^2".into(), false)];
    let expected_display = vec![("E = mc^2".into(), true)];
    assert_eq!(sources(&["$E = mc^2$"]), expected_inline);
    assert_eq!(sources(&[r"\(E = mc^2\)"]), expected_inline);
    assert_eq!(sources(&["$$E = mc^2$$"]), expected_display);
    assert_eq!(sources(&[r"\[E = mc^2\]"]), expected_display);
}

#[test]
fn markdown_unescaped_bare_brackets_render_as_display_math() {
    // 部分 AI CLI 的 markdown 渲染会吃掉 `\[` `\]` `\,` 的反斜杠
    // （它们是 markdown 标点转义），`\int`/`\frac` 不是转义所以幸存，
    // 屏幕上只剩裸 `[` 块——2026-08-17 截图的真实形态。
    assert_eq!(
        sources(&["[", r"\int_0^1 x^2,dx = \frac{1}{3}", "]"]),
        vec![(r"\int_0^1 x^2,dx = \frac{1}{3}".into(), true)]
    );
    assert_eq!(
        sources(&["[", r"\sum_{i=1}^{n} i = \frac{n(n+1)}{2}", "]"]),
        vec![(r"\sum_{i=1}^{n} i = \frac{n(n+1)}{2}".into(), true)]
    );
    assert_eq!(
        sources(&[r"[ \lim_{x\to 0}\frac{\sin x}{x}=1 ]"]),
        vec![(r"\lim_{x\to 0}\frac{\sin x}{x}=1".into(), true)]
    );
    // Codex CLI 把 `\[ E = mc^2 \]` 渲染成三行普通行，因为其 Markdown
    // 层消费了括号转义。
    assert_eq!(
        sources(&["[", "E = mc^2", "]"]),
        vec![("E = mc^2".into(), true)]
    );
    // Codex 的列表渲染器把圆点留在开定界符上。
    assert_eq!(
        sources(&[
            "• [",
            r"  \int_{-\infty}^{+\infty} e^{-x^2}\,dx=\sqrt{\pi}",
            "  ]"
        ]),
        vec![(
            r"\int_{-\infty}^{+\infty} e^{-x^2}\,dx=\sqrt{\pi}".into(),
            true
        )]
    );
}

#[test]
fn bare_bracket_interval_inside_formula_does_not_close_early() {
    // 内容行里的 `[0, 1]` 区间：其 `]` 不独占一行，没有闭合资格。
    assert_eq!(
        sources(&["[", r"x \in [0, 1]", "]"]),
        vec![(r"x \in [0, 1]".into(), true)]
    );
}

#[test]
fn bare_brackets_without_tex_evidence_stay_literal() {
    assert!(sources(&["[", "  1, 2, 3,", "]"]).is_empty());
    assert!(sources(&[r#"["alpha", "beta"]"#]).is_empty());
    assert!(sources(&["[INFO] server started"]).is_empty());
    assert!(sources(&["[x^2]"]).is_empty());
    assert!(sources(&["result [ x^2 ] done"]).is_empty());
    assert!(sources(&["[", r"C:\temp\integral.txt", "]"]).is_empty());
}

#[test]
fn markdown_unescaped_bare_parens_render_inline() {
    assert_eq!(
        sources(&[r"也可以使用 (\sqrt{x^2+y^2}) 表示行内公式"]),
        vec![(r"\sqrt{x^2+y^2}".into(), false)]
    );
    assert_eq!(
        sources(&["• 质能方程为 (E=mc^2)。"]),
        vec![("E=mc^2".into(), false)]
    );
    let sequence = r"a_n=a_1+(n-1)d";
    assert_eq!(
        sources(&[&format!("• ({sequence})，done")]),
        vec![(sequence.into(), false)]
    );
    compile_formula(sequence, false, 18.0, 1.0, DEFAULT_LIMITS)
        .expect("the recovered sequence formula must compile");
    // 括号深度配对：`\sin(x)` 的内层 `)` 不提前截断。
    assert_eq!(
        sources(&[r"值 (\sin(x)) 收敛"]),
        vec![(r"\sin(x)".into(), false)]
    );
}

#[test]
fn codex_line_with_multiple_inline_formulas_keeps_bare_paren_math() {
    let source = r"$E = mc^2$，$a^2 + b^2 = c^2$，$e^{i\pi}+1=0$，(\displaystyle \int_0^1 x^2,dx=\frac13)，$\sum_{n=1}^{\infty}\frac1{n^2}=\frac{\pi^2}{6}$";
    let formulas = sources(&[source]);
    assert_eq!(formulas.len(), 5);
    assert_eq!(
        formulas[3],
        (r"\displaystyle \int_0^1 x^2,dx=\frac13".into(), false)
    );
    compile_formula(&formulas[3].0, false, 18.0, 1.0, DEFAULT_LIMITS)
        .expect("Codex's unbraced fraction arguments must compile");
}

#[test]
fn implicit_product_equations_use_the_standard_delimiter_rule() {
    let expected = vec![("F=ma".into(), false), ("PV=nRT".into(), false)];
    assert_eq!(sources(&[r"\(F=ma\), \(PV=nRT\)"]), expected);
    assert_eq!(sources(&["$F=ma$, $PV=nRT$"]), expected);
    assert_eq!(
        sources(&[r"(\displaystyle F=ma), (\displaystyle PV=nRT)"]),
        vec![
            (r"\displaystyle F=ma".into(), false),
            (r"\displaystyle PV=nRT".into(), false),
        ]
    );

    assert_eq!(sources(&[r"\(key=value\)"]), Vec::new());
    assert_eq!(sources(&["$key=value$"]), Vec::new());
    assert!(sources(&["setting (key=value)"]).is_empty());
}

#[test]
fn bare_parens_reject_regex_prose_and_single_letter_escapes() {
    assert!(sources(&[r"grep -E (\d+) input.txt"]).is_empty());
    assert!(sources(&[r"match (\w*) here"]).is_empty());
    assert!(sources(&["plain (normal prose) text"]).is_empty());
    assert!(sources(&["setting (key=value)"]).is_empty());
    assert!(sources(&[r"case (\n) newline"]).is_empty());
    assert!(sources(&[r"path (C:\temp\frac) oops"]).is_empty());
}

/// 定界符表明意图，内容提供证据——**两者都要**。只看定界符会把终端里
/// 满地的 shell sigil、提示符和价格全渲染成公式，所以这些形状必须保持
/// 原文。它们不是理论候选：`HOME`、`npm install`、`PATH=/tmp` 在
/// pulldown-latex 里都能成功解析成公式事件，判定放过就是真替换。
#[test]
fn explicit_delimiters_still_require_content_evidence() {
    assert!(sources(&[r"escaped \$x$"]).is_empty());
    // 成对美元定界符明确表示数字公式；真实金额使用单个美元符号，仍保持原文。
    assert_eq!(
        sources(&["values $5$ and $12.50$"]),
        vec![("5".into(), false), ("12.50".into(), false)]
    );
    assert!(sources(&["price $5 and $12.50"]).is_empty());
    assert!(sources(&["env $LONG_VARIABLE$"]).is_empty());
    assert!(sources(&["quote $USD 20$ today"]).is_empty());
    assert!(sources(&["literal $hello$ text"]).is_empty());
    assert!(sources(&["config $PATH=/tmp$"]).is_empty());
    assert!(sources(&[r"plain \(normal prose\)"]).is_empty());
    assert!(sources(&["$ $", r"\(  \)", "$$  $$"]).is_empty());
    // 路径不是公式：`explicit_operand` 把标识符卡在一个字母，`foo/bar`
    // 的两侧都不合格，所以 `/` 不构成紧凑运算符证据。
    assert!(sources(&["path $foo/bar$"]).is_empty());
    // 真数学照旧：display 块的 lax 证据。
    assert_eq!(sources(&["$$E = mc^2$$"]), vec![("E = mc^2".into(), true)]);
    assert_eq!(sources(&["$a/b$"]), vec![("a/b".into(), false)]);
}

/// 普通 shell 里真实出现过的形状，一个都不许变成公式。
/// 每一行都对应实测过的误报。
#[test]
fn ordinary_shell_output_stays_literal() {
    // 变量 sigil 成对出现，中间那截会被当成源码。
    assert!(sources(&["echo $HOME $USER"]).is_empty());
    assert!(sources(&["echo $HOME"]).is_empty());
    // sh 家族提示符后的命令不具备公式证据，不能借下一个提示符闭合。
    assert!(sources(&["$ npm install", "$ npm test"]).is_empty());
    assert!(sources(&["user@host:~$ echo hello", "hello", "user@host:~$ ls"]).is_empty());
    // 行内定界符不跨真实换行，两行各自的 `$` 不配对。
    assert!(sources(&["$x", "$y"]).is_empty());
    assert!(sources(&["export $FOO=1", "export $BAR=2"]).is_empty());
    // 价格与提交信息。
    assert!(sources(&["cost $5 vs $7 today"]).is_empty());
    assert!(sources(&["fix: charge $9.99", "feat: refund $9.99"]).is_empty());
    // BRE 捕获组：`\(…\)` 撞 sed/grep 的日常写法。
    assert!(sources(&[r"sed 's/\(abc\)/x/' input.txt"]).is_empty());
    assert!(sources(&[r"grep '\(foo\|bar\)' log.txt"]).is_empty());
}

/// 终端软折行和 AI 的硬折行都可接数学片段，但硬折行不能穿过普通正文。
#[test]
fn inline_formula_crosses_soft_wrap_and_math_only_hard_rows() {
    let mut wrapped = TextGrid::from_rows(&["prefix $x^2", "+ y^2$ tail"]);
    wrapped.wrapped[0] = true;
    assert_eq!(
        scan_grid(&wrapped).len(),
        1,
        "soft wrap 内的行内公式要接起来"
    );

    let hard = TextGrid::from_rows(&["prefix $x^2", "+ y^2$ tail"]);
    assert_eq!(
        scan_grid(&hard).len(),
        1,
        "有数学证据的硬折行仍属于同一公式"
    );
    assert!(sources(&["prefix $x^2", "ordinary prose", "+ y^2$ tail"]).is_empty());
}

/// display 定界符照旧跨真实换行：`$$` / `\[ \]` 占住它们之间的整块，
/// Agent TUI 硬换行的块级公式靠这条恢复。
#[test]
fn display_formula_still_crosses_a_real_newline() {
    assert_eq!(sources(&["$$", r"\frac{1}{2}", "$$"]).len(), 1);
    assert_eq!(sources(&[r"\[", r"\sum_{i=1}^n i", r"\]"]).len(), 1);
}

/// 基线证据层漏掉的两个真公式。`explicit_operand` 把标识符卡在一个字母，
/// 物理里省略乘号的 `mc^2` / `nRT` 全被误杀；阶乘则压根没有证据项。
#[test]
fn implicit_products_and_factorials_are_math() {
    assert_eq!(sources(&["$E=mc^2$"]), vec![("E=mc^2".into(), false)]);
    assert_eq!(sources(&["$E = mc^2$"]), vec![("E = mc^2".into(), false)]);
    assert_eq!(sources(&["$PV=nRT$"]), vec![("PV=nRT".into(), false)]);
    assert_eq!(sources(&["$F=ma$"]), vec![("F=ma".into(), false)]);
    assert_eq!(sources(&["$n!$"]), vec![("n!".into(), false)]);
    assert_eq!(sources(&[r"\(E=mc^2\)"]), vec![("E=mc^2".into(), false)]);
    // 放宽到三个字母不等于放开散文：这些仍然保持原文。
    assert!(sources(&["$key=value$"]).is_empty());
    assert!(sources(&["$PATH=/tmp$"]).is_empty());
    assert!(sources(&["$npm install$"]).is_empty());
    assert!(sources(&["$Hello!$"]).is_empty());
}

/// 失败候选必须有预算：`MAX_VISIBLE_FORMULAS` 只限成功数，不限失败数。
/// 满屏未闭合的 `(` 曾让单帧扫描从 ~2ms 涨到 ~322ms（50×200 debug）。
#[test]
fn unclosed_bare_delimiters_stay_within_budget() {
    let dense: String = std::iter::repeat_n("(x ", 66).collect();
    let rows: Vec<&str> = std::iter::repeat_n(dense.as_str(), 50).collect();
    let grid = TextGrid::from_rows(&rows);
    let plain: String = std::iter::repeat_n("plain text ", 18).collect();
    let plain_rows: Vec<&str> = std::iter::repeat_n(plain.as_str(), 50).collect();
    let baseline_grid = TextGrid::from_rows(&plain_rows);

    let start = std::time::Instant::now();
    assert!(scan_grid(&grid).is_empty(), "未闭合的裸括号不产生公式");
    let dense_cost = start.elapsed();
    let start = std::time::Instant::now();
    assert!(scan_grid(&baseline_grid).is_empty());
    let baseline_cost = start.elapsed();

    // 预算把每个候选的搜索钉在常数行数上，于是总成本跟着格子数走而不是
    // 格子数的平方。放宽到 12 倍是给 CI 抖动留的余量，回归会是三位数倍。
    assert!(
        dense_cost < baseline_cost * 12,
        "未闭合括号扫描 {dense_cost:?} 相对基准 {baseline_cost:?} 退化成二次复杂度"
    );
}

#[test]
fn single_dollar_accepts_only_explicit_math_shapes() {
    assert_eq!(
        sources(&["$x$ $x_1$ $2+2$ $a/b$ $x=y$ $f(x)$ $f(x)=0$"]).len(),
        7
    );
    assert_eq!(sources(&[r"$\frac{1}{2}$ $\sin x$ $x^2$"]).len(), 3);
}

#[test]
fn issue420_inline_atoms_and_polynomials_do_not_consume_neighboring_formulas() {
    assert_eq!(
        sources(&[
            r"其中 $E$ 表示能量，$0$ 是初值，$2.71828$ 是近似值；$ax^2+bx+c=0$，$\pm$ 表示两个解。"
        ]),
        vec![
            ("E".into(), false),
            ("0".into(), false),
            ("2.71828".into(), false),
            ("ax^2+bx+c=0".into(), false),
            (r"\pm".into(), false),
        ]
    );
    assert_eq!(
        sources(&[r"for $USD$ use $x$ and $\ln(1+x)$"]),
        vec![("x".into(), false), (r"\ln(1+x)".into(), false)]
    );
}

#[test]
fn issue420_inline_tex_recovers_bounded_agent_line_breaks() {
    let found = sources(&[
        r"定积分示例：$\displaystyle\int_0^1 x^2,dx",
        r"  = \dfrac{1}{3}$（表示函数在区间 $[0,1]$ 上的积分）",
    ]);
    assert_eq!(found.len(), 2, "{found:?}");
    assert!(found[0].0.starts_with(r"\displaystyle\int_0^1"));
    assert!(found[0].0.ends_with(r"\dfrac{1}{3}"));
    assert_eq!(found[1], ("[0,1]".into(), false));
    assert_eq!(
        sources(&[r"formula $ \frac{1}{2} $ ends"]),
        vec![(r"\frac{1}{2}".into(), false)]
    );
    assert!(sources(&["price $5", "another amount $7"]).is_empty());
    assert_eq!(
        sources(&[r"unclosed $\frac{1}{2}", "", "next $x$"]),
        vec![("x".into(), false)]
    );
    assert_eq!(
        sources(&[r"unclosed $\frac{1}{2}", "ordinary text", "next $x$"]),
        vec![("x".into(), false)]
    );
}

#[test]
fn single_dollar_uses_the_standard_formula_rule_without_agent_context() {
    assert_eq!(sources(&["plain $x$ text"]), vec![("x".into(), false)]);
    assert_eq!(
        sources(&["质能方程 $E = mc^2$，其中 c 为光速。"]),
        vec![("E = mc^2".into(), false)]
    );
    assert_eq!(
        sources(&[
            r"二次方程：$x = \dfrac{-b \pm \sqrt{b^2 - 4ac}}{2a}$",
            r"斯特林公式：$n! \approx \sqrt{2\pi n}\left(\dfrac{n}{e}\right)^n$",
        ],),
        vec![
            (r"x = \dfrac{-b \pm \sqrt{b^2 - 4ac}}{2a}".into(), false),
            (
                r"n! \approx \sqrt{2\pi n}\left(\dfrac{n}{e}\right)^n".into(),
                false
            ),
        ]
    );
}
