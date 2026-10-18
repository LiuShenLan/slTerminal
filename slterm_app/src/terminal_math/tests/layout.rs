use super::*;

// 尺寸拟合、墨迹预算、覆盖掩码与视口扩展。

#[test]
fn display_formula_with_blank_neighbours_keeps_the_terminal_font_size() {
    let fitted = fitted_size(&[
        "                    ",
        "$$                  ",
        r"\sum_{i=1}^{n} i^2  ",
        "$$                  ",
        "                    ",
    ]);
    assert_eq!(
        fitted, TEST_FONT_PX,
        "blank-neighbour display math must not shrink"
    );
}

/// The whole point of the blank-row budget: the same source keeps one size
/// whether the emitter put it on one line or spread it over three.
#[test]
fn display_math_size_does_not_depend_on_how_many_rows_the_source_used() {
    let one_line = fitted_size(&[
        "                              ",
        r"$$\sum_{i=1}^{n} i^2$$        ",
        "                              ",
    ]);
    let three_lines = fitted_size(&[
        "                              ",
        "$$                            ",
        r"\sum_{i=1}^{n} i^2            ",
        "$$                            ",
        "                              ",
    ]);
    assert_eq!(one_line, three_lines);
    assert_eq!(one_line, TEST_FONT_PX);
}

/// A row hemmed in on both sides by prose that reaches under the formula
/// is the one case where the budget really is a single line gap. The
/// formula must then trade layout style — and only as a last resort a few
/// percent of size — for staying inside it, because the clip crops exactly
/// what the fit could not absorb.
#[test]
fn tall_formula_hemmed_in_by_prose_stays_inside_the_line_gap_budget() {
    let rows = &[
        "prose above that runs under the formula",
        r"$$\frac{x^2+1}{y-1}$$                  ",
        "prose below that runs under the formula",
    ];
    let grid = TextGrid::from_rows(rows);
    let overlays = scan_grid_with_hints(&grid);
    assert_eq!(overlays.len(), 1);
    let size = test_size(grid.columns as f32, 8.0);

    let mut state = TerminalMathState::default();
    let prepared = prepare_overlays(&mut state, &overlays, &size, TEST_FONT_PX, 1.0);
    let prepared = prepared[0].expect("formula must render");
    assert_eq!(
        prepared.bleed_top,
        size.cell_height() * DISPLAY_BLEED_INTO_PROSE,
        "display math should leave a small prose clearance above",
    );
    assert_eq!(
        prepared.bleed_bottom,
        size.cell_height() * DISPLAY_BLEED_INTO_PROSE,
        "display math should leave a small prose clearance below",
    );
    assert!(
        !prepared.display_style,
        "the compact style must be tried before any size is given up",
    );
    assert!(
        prepared.fitted_pixel_size >= TEST_FONT_PX * 0.9,
        "compact style should cost at most a few percent, got {}",
        prepared.fitted_pixel_size,
    );

    // The invariant behind the whole scheme: re-laid-out ink stays inside
    // bounds plus the prose-side budgets and the deliberate overrun
    // tolerance, so the clip only ever trims an antialiasing edge.
    let layout = state
        .layout(
            overlays[0].formula_id,
            &overlays[0].source,
            prepared.fitted_pixel_size,
            1.0,
            prepared.display_style,
        )
        .expect("fitted layout");
    let budget = size.cell_height()
        * (1.0 + DISPLAY_BLEED_INTO_PROSE * 2.0)
        * (1.0 + HEIGHT_OVERRUN_TOLERANCE);
    assert!(
        layout.metrics.height + layout.metrics.depth <= budget + 0.5,
        "fitted ink {} must fit the prose budget {}",
        layout.metrics.height + layout.metrics.depth,
        budget,
    );
}

#[test]
fn display_math_uses_a_smaller_prose_bleed_than_inline_math() {
    let display_grid = TextGrid::from_rows(&["above", "$$x^2$$", "below"]);
    let display_overlays = scan_grid_with_hints(&display_grid);
    let display_size = test_size(display_grid.columns as f32, display_grid.rows.len() as f32);
    let display_overlay = &display_overlays[0];
    let display_bounds = display_overlay
        .bounds(&display_size)
        .expect("display bounds");
    let display_bleed = display_overlay.vertical_bleed(&display_size, (0, 0));

    let inline_grid = TextGrid::from_rows(&["value $x^2$ grows"]);
    let inline_overlays = scan_grid_with_hints(&inline_grid);
    let inline_size = test_size(inline_grid.columns as f32, inline_grid.rows.len() as f32);
    let inline_overlay = &inline_overlays[0];
    let inline_bleed = inline_overlay.vertical_bleed(&inline_size, (0, 0));

    assert!(display_overlay.display);
    assert!(!inline_overlay.display);
    assert!(display_bleed.0 < inline_bleed.0);
    assert!(display_bleed.1 < inline_bleed.1);
    assert_eq!(
        display_bleed.0,
        display_bounds.height() * DISPLAY_BLEED_INTO_PROSE
    );
}

/// Prose that stops well before the formula's columns is not in the way:
/// the ink of a centred block lands where those rows are empty, so it may
/// use their height and keep both the display style and the font size.
#[test]
fn short_prose_neighbours_do_not_shrink_a_centred_block() {
    let rows = &[
        "其中：                                  ",
        r"$$\sum_{i=1}^{n} i = \frac{n(n+1)}{2}$$",
        "下面继续说明。                            ",
    ];
    let grid = TextGrid::from_rows(rows);
    let overlays = scan_grid_with_hints(&grid);
    assert_eq!(overlays.len(), 1);

    let mut state = TerminalMathState::default();
    let size = test_size(grid.columns as f32, 8.0);
    let prepared = prepare_overlays(&mut state, &overlays, &size, TEST_FONT_PX, 1.0);
    let prepared = prepared[0].expect("formula must render");
    assert_eq!(prepared.fitted_pixel_size, TEST_FONT_PX);
    assert!(
        prepared.display_style,
        "there is room for the block style here"
    );
}

#[test]
fn inline_formula_between_prose_keeps_the_terminal_font_size() {
    let fitted = fitted_size(&["value $x^2$ grows   "]);
    assert_eq!(
        fitted, TEST_FONT_PX,
        "a superscript must fit the line-gap budget without shrinking",
    );
}

#[test]
fn adjacent_formula_rows_do_not_share_vertical_clip_budget() {
    let rows = [
        r"left $\frac{a_1+b^2}{c_i-d^3}+x_i^2$",
        r"mid $\int_0^\infty e^{-x^2}dx+\sum_{k=1}^n k^2$",
        r"right $\frac{\sum_{i=1}^n x_i}{\sqrt{1+x_{j-1}^2}}+y_{m+1}$",
    ];
    let grid = TextGrid::from_rows(&rows);
    let overlays = scan_grid_with_hints(&grid);
    assert_eq!(overlays.len(), 3);
    let size = test_size(grid.columns as f32, grid.rows.len() as f32);
    let mut state = TerminalMathState::default();
    let prepared = prepare_overlays(&mut state, &overlays, &size, TEST_FONT_PX, 1.0);

    for (index, overlay) in overlays.iter().enumerate() {
        let bounds = overlay.bounds(&size).expect("formula bounds");
        let metrics = TerminalMathState::default()
            .layout(
                overlay.formula_id,
                &overlay.source,
                TEST_FONT_PX,
                1.0,
                false,
            )
            .expect("formula layout")
            .metrics;
        let columns = ink_columns(overlay, &size, bounds, bounds.right, metrics.width);
        let absorbed = overlay.absorbable_rows(columns);
        let (above, below) = overlay.vertical_bleed(&size, absorbed);
        if index > 0 {
            assert_eq!(
                above, 0.0,
                "formula row {index} must not bleed into row above"
            );
        }
        if index + 1 < overlays.len() {
            assert_eq!(
                below, 0.0,
                "formula row {index} must not bleed into row below"
            );
        }

        let fitted = prepared[index].expect("dense formula must render");
        let layout = state
            .layout(
                overlay.formula_id,
                &overlay.source,
                fitted.fitted_pixel_size,
                1.0,
                fitted.display_style,
            )
            .expect("fitted formula layout");
        let available_height = bounds.height() + fitted.bleed_top + fitted.bleed_bottom;
        assert!(
            layout.metrics.height + layout.metrics.depth <= available_height + 0.5,
            "formula row {index} ink {} exceeds its clip budget {}",
            layout.metrics.height + layout.metrics.depth,
            available_height,
        );
    }
}

/// The acceptance rule: formulas with the same delimiter should read at one size;
/// at one size. Groups may differ from each other — an inline fraction is
/// meant to be more compact than a block one — but within a group a reader
/// scanning an answer must not see one formula shrunk against the others.
#[test]
fn formulas_of_the_same_kind_render_at_one_size() {
    let rows = &[
        "核心想法：与其硬选一个 prompt，不如软组合一堆 prompt 组件。      ",
        "                                                              ",
        "- 用 query 对所有组件算注意力权重，然后加权求和出一组 prompt：   ",
        r"$$P_i = \sum_j a_{i,j} \cdot c_j$$                            ",
        "- 这样每个任务的 prompt 是所有组件的连续加权组合。              ",
        "                                                              ",
        "单行块紧贴正文：                                                ",
        r"$$\sum_{i=1}^{n} i = \frac{n(n+1)}{2}$$                       ",
        "下面继续说明。                                                  ",
        "                                                              ",
        r"高斯积分：$$\int_{-\infty}^{\infty} e^{-x^2}dx = \sqrt{\pi}$$   ",
        "                                                              ",
        "$$                                                            ",
        r"A = \begin{pmatrix}1 & 2 \\ 3 & 4\end{pmatrix}                ",
        "$$                                                            ",
        "                                                              ",
        r"内联的分式 $\frac{a+b}{c-d}$ 与根号 $\sqrt{x^2+y^2}$ 混排收尾。 ",
        r"行内的求和 $\sum_{i=1}^{n} i$ 和上标 $x^2$ 也在同一段里。       ",
    ];
    let grid = TextGrid::from_rows(rows);
    let overlays = scan_grid_with_hints(&grid);
    let size = test_size(grid.columns as f32, grid.rows.len() as f32);
    let mut state = TerminalMathState::default();
    let prepared = prepare_overlays(&mut state, &overlays, &size, TEST_FONT_PX, 1.0);

    let mut sizes: BTreeMap<bool, Vec<(String, f32)>> = BTreeMap::new();
    for (overlay, prepared) in overlays.iter().zip(&prepared) {
        let prepared = prepared.expect("every formula in the sample must render");
        sizes
            .entry(overlay.display)
            .or_default()
            .push((overlay.source.to_string(), prepared.fitted_pixel_size));
    }
    assert_eq!(sizes.len(), 2, "sample must exercise both kinds");
    for (display, group) in sizes {
        assert!(
            group.len() >= 4,
            "each kind needs several samples, got {group:?}"
        );
        // A hard formula boundary may require a small local reduction to
        // keep a script intact; unconstrained neighbours remain at the
        // terminal size and no formula is allowed to become unreadably small.
        assert!(
            group.iter().all(|(_, size)| *size >= TEST_FONT_PX * 0.9),
            "{} formulas must stay readable: {group:?}",
            if display { "block" } else { "inline" },
        );
    }
}

/// The ceiling behind the raised script sizes: a formula sharing its row
/// with prose may grow past one row, but never past two — beyond that it
/// stops being ink in a line gap and starts being ink on the neighbours.
#[test]
fn inline_math_never_grows_past_two_rows() {
    let sources = [
        r"$\frac{a+b}{c-d}$",
        r"$\int_0^\infty e^{-x^2}dx$",
        r"$\sqrt{\frac{a+b}{c+d}}$",
        r"$x_{i+1}^2+y_{j-1}^2$",
        r"$\sum_{i=1}^{n} i$",
    ];
    let size = test_size(60.0, 4.0);
    for source in sources {
        let row = format!("值 {source} 收敛                        ");
        let grid = TextGrid::from_rows(&[&row]);
        let overlays = scan_grid_with_hints(&grid);
        assert_eq!(overlays.len(), 1, "{source}");

        let mut state = TerminalMathState::default();
        let prepared = prepare_overlays(&mut state, &overlays, &size, TEST_FONT_PX, 1.0);
        let prepared = prepared[0].expect("formula must render");
        let layout = state
            .layout(
                overlays[0].formula_id,
                &overlays[0].source,
                prepared.fitted_pixel_size,
                1.0,
                prepared.display_style,
            )
            .expect("fitted layout");
        let rows = (layout.metrics.height + layout.metrics.depth) / size.cell_height();
        assert!(rows <= 2.0, "{source} rendered {rows} rows tall");
    }
}

#[test]
fn display_math_mask_only_covers_the_formula_source() {
    let grid = TextGrid::from_rows(&[
        "     explanation in the main pane                                  $0.00 spent      ",
        "     $$E = mc^2$$                                                                    ",
    ]);
    let overlays = scan_grid_with_hints(&grid);
    let coverage = CoverageMask::build(&overlays, &[Some(())]);

    assert!(
        coverage.covers(Point::new(1, Column(5))),
        "formula source is hidden"
    );
    assert!(
        coverage.covers(Point::new(1, Column(16))),
        "formula source is hidden"
    );
    assert!(
        !coverage.covers(Point::new(1, Column(17))),
        "centering space keeps its TUI background"
    );
    assert!(
        !coverage.covers(Point::new(1, Column(41))),
        "trailing TUI background is preserved"
    );
}

#[test]
fn display_math_without_a_right_pane_uses_the_viewport() {
    let grid = TextGrid::from_rows(&[
        "     ordinary prose across one terminal row                                      ",
        "     $$E = mc^2$$                                                                  ",
    ]);
    let overlays = scan_grid_with_hints(&grid);

    assert_eq!(overlays.len(), 1);
    assert_eq!(overlays[0].widen_right_to, Some(grid.columns));
}

/// 诊断用：打印各场景的缩放比例，`cargo test diagnose -- --nocapture`。
#[test]
fn diagnose_sizes() {
    let blank = " ".repeat(80);
    let blank = blank.as_str();
    let long = r"\pm\quad \mp\quad \times\quad \div\quad \cdot\quad \ast\quad \circ\quad \bullet\quad \oplus\quad \otimes";
    let cases: Vec<(&str, Vec<&str>, bool)> = vec![
        (
            "display 3行 sum",
            vec![blank, "$$", r"\sum_{i=1}^{n} i^2", "$$", blank],
            false,
        ),
        (
            "display 1行 sum",
            vec![blank, r"$$\sum_{i=1}^{n} i^2$$", blank],
            false,
        ),
        (
            "display 1行 frac",
            vec![blank, r"$$\frac{x^2+1}{y-1}$$", blank],
            false,
        ),
        (
            "display 1行 矩阵",
            vec![blank, r"$$A=\begin{pmatrix}1&2\\3&4\end{pmatrix}$$", blank],
            false,
        ),
        (
            "display 3行 矩阵",
            vec![
                blank,
                "$$",
                r"A=\begin{pmatrix}1&2\\3&4\end{pmatrix}",
                "$$",
                blank,
            ],
            false,
        ),
        (
            "display 3行 大矩阵",
            vec![
                blank,
                "$$",
                r"A=\begin{pmatrix}1&2&3\\4&5&6\\7&8&9\end{pmatrix}",
                "$$",
                blank,
            ],
            false,
        ),
        (
            "display 3行 长串",
            vec![blank, "$$", long, "$$", blank],
            false,
        ),
        (
            "display 3行 紧贴正文",
            vec!["前文", "$$", r"\frac{x^2+1}{y-1}", "$$", "后文"],
            false,
        ),
        (
            "display 1行 紧贴正文",
            vec![
                "- 来一张图片，用 query 对所有组件算注意力权重，然后加权求和出一组 prompt：",
                r"$$P_i = \sum_j attention_{i,j} \cdot component_j$$",
                "- 这样每个任务的 prompt 不是从池子里挑一个，而是所有组件的连续加权组合。",
            ],
            false,
        ),
        (
            "display 1行 sum 夹住",
            vec![
                "前面的说明文字：",
                r"$$\sum_{i=1}^{n} i = \frac{n(n+1)}{2}$$",
                "下面继续说明。",
            ],
            false,
        ),
        // 用户 08-05 截图的真实排版：上下都是跑满整行的长正文，公式的
        // 墨迹列被两侧的字覆盖，一行的高度就是全部预算。
        (
            "display 长正文夹住 sum",
            vec![
                "单行块紧贴正文：- 来一张图片，用 query 对所有组件算注意力权重，然后加权求和出一组 prompt：",
                r"$$\sum_{i=1}^{n} i = \frac{n(n+1)}{2}$$",
                "下面继续说明。- 来一张图片，用 query 对所有组件算注意力权重，然后加权求和出一组 prompt：",
            ],
            false,
        ),
        (
            "display 长正文夹住 int",
            vec![
                "- 来一张图片，用 query 对所有组件算注意力权重，然后加权求和出一组 prompt：",
                r"高斯积分：$$\int_{-\infty}^{\infty} e^{-x^2}dx = \sqrt{\pi}$$",
                "- 来一张图片，用 query 对所有组件算注意力权重，然后加权求和出一组 prompt：",
            ],
            false,
        ),
        (
            "display 1行 矩阵夹住",
            vec![
                "前面的说明：",
                r"$$A=\begin{pmatrix}1&2\\3&4\end{pmatrix}$$",
                "下面继续。",
            ],
            false,
        ),
        ("inline x^2", vec!["value $x^2$ grows"], true),
        ("inline frac", vec![r"value $\frac{a}{b}$ grows"], true),
        (
            "inline 大 frac",
            vec![r"value $\frac{x^2+1}{y-1}$ grows"],
            true,
        ),
        ("inline sqrt", vec![r"value $\sqrt{x^2+y^2}$ grows"], true),
        ("inline sum", vec![r"value $\sum_{i=1}^{n} i$ grows"], true),
        (
            "inline int",
            vec![r"value $\int_0^\infty e^{-x^2}dx$ ok"],
            true,
        ),
        (
            "inline 短源码",
            vec![r"值 $x_{i+1}^2+y_{j-1}^2$ 收敛"],
            true,
        ),
    ];
    println!(
        "\n{:<22} {:>6} {:>6} {:>16} {:>16}",
        "场景", "比例", "样式", "墨迹 宽×高", "预算 宽×高"
    );
    for (name, rows, expected_inline) in cases {
        let grid = TextGrid::from_rows(&rows);
        let overlays = scan_grid_with_hints(&grid);
        assert_eq!(overlays.len(), 1, "{name}: expected one formula");
        let overlay = &overlays[0];
        assert_eq!(
            !overlay.display, expected_inline,
            "{name}: unexpected formula style"
        );
        let size = test_size(grid.columns as f32, grid.rows.len() as f32);
        let mut state = TerminalMathState::default();

        let bounds = overlay.bounds(&size).expect("bounds");
        let right = overlay.widen_right_to.map_or(bounds.right, |column| {
            (size.padding_x() + column as f32 * size.cell_width()).max(bounds.right)
        });
        let base = state
            .layout(
                overlay.formula_id,
                &overlay.source,
                TEST_FONT_PX,
                1.0,
                overlay.display,
            )
            .expect("layout")
            .metrics;
        let absorbed =
            overlay.absorbable_rows(ink_columns(overlay, &size, bounds, right, base.width));
        let (bleed_top, bleed_bottom) = overlay.vertical_bleed(&size, absorbed);
        let budget_width = right - bounds.left - FORMULA_INSET * 2.0;
        let budget_height = bounds.height() + bleed_top + bleed_bottom;

        let prepared = prepare_overlays(&mut state, &overlays, &size, TEST_FONT_PX, 1.0);
        let prepared = prepared[0].expect("formula must render");
        println!(
            "{name:<22} {:>5.0}% {:>6} {:>7.0}×{:<8.0} {:>7.0}×{:<8.0}",
            prepared.fitted_pixel_size / TEST_FONT_PX * 100.0,
            if prepared.display_style {
                "块级"
            } else {
                "行内"
            },
            base.width,
            base.height + base.depth,
            budget_width,
            budget_height,
        );
    }
}
