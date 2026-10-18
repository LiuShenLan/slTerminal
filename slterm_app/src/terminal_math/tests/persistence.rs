use super::*;

// 公式持久化、滚动/重排裁剪与历史回看。

#[test]
fn completed_formula_replaces_overlapping_streaming_candidate() {
    let grid = TextGrid::from_rows(&["pre $x^2$ suffix"]);
    let outer = scan_grid(&grid).remove(0);
    let mut inner = outer.clone();
    inner.spans[0].start += 1;
    inner.spans[0].end -= 1;

    let mut state = TerminalMathState::default();
    state.remember(&grid, &inner);
    state.remember(&grid, &outer);

    assert_eq!(state.formulas.len(), 1);
    assert!(state.formulas.contains_key(&FormulaAnchor {
        row: 0,
        column: outer.spans[0].start
    }));
}

#[test]
fn active_cli_input_logical_line_is_never_persisted_as_math_output() {
    let mut grid = TextGrid::from_rows(&[
        "prompt $$lim_x{x}$$",
        "continued $x^1$",
        "assistant $$x^2$$",
    ]);
    // The first physical row wraps into the cursor row, so both belong to
    // one live editor buffer and must be excluded together.
    grid.wrapped[0] = true;
    let active_rows = grid
        .logical_rows_containing(1)
        .expect("cursor logical line");
    assert_eq!(active_rows, 0..=1);

    let mut state = TerminalMathState::default();
    state.synchronize_grid(&grid);
    assert!(state.scan_visible_grid(&grid, Some(&active_rows)).is_none());

    let overlays = state.visible_overlays(&grid);
    assert_eq!(overlays.len(), 1);
    assert_eq!(overlays[0].source.as_ref(), "x^2");
}

#[test]
fn unmatched_display_delimiter_on_live_input_does_not_start_history_reconstruction() {
    let grid = TextGrid::from_rows(&["prompt $$"]);
    let active_rows = grid.logical_rows_containing(0).expect("cursor row");
    let mut state = TerminalMathState::default();
    state.synchronize_grid(&grid);

    assert!(state.scan_visible_grid(&grid, Some(&active_rows)).is_none());
    assert!(state.pending_display.is_none());
}

#[test]
fn persisted_formula_survives_partial_scroll_without_rescaling_its_bounds() {
    let mut state = TerminalMathState::default();
    let initial = grid_at(&["$$   ", "x^2  ", "$$   ", "tail "], 40, 0);
    remember_visible(&mut state, &initial);
    assert_eq!(state.formulas.len(), 1);

    let scrolled = grid_at(&["x^2  ", "$$   ", "tail ", "next "], 41, 0);
    state.synchronize_grid(&scrolled);
    assert!(scan_grid(&scrolled).is_empty());
    let overlays = state.visible_overlays(&scrolled);

    assert_eq!(overlays.len(), 1);
    assert_eq!(overlays[0].source.as_ref(), "x^2");
    assert_eq!(overlays[0].spans.first().map(|span| span.row), Some(-1));
    assert_eq!(overlays[0].spans.last().map(|span| span.row), Some(1));
}

#[test]
fn streamed_display_formula_completes_after_opening_scrolls_into_history() {
    let mut state = TerminalMathState::default();
    let opening = grid_at(
        &[
            "$$                      ",
            r"\begin{aligned}         ",
            r"f(x) &= x^2 \\          ",
            "h(x) &= x^3            ",
        ],
        40,
        0,
    );
    state.synchronize_grid(&opening);
    assert!(state.scan_visible_grid(&opening, None).is_none());

    let visible_tail = grid_at(
        &[
            r"g(x) &= x+1            ",
            r"\end{aligned}           ",
            "$$                      ",
            "tail                    ",
        ],
        44,
        0,
    );
    state.synchronize_grid(&visible_tail);
    let history_anchor = state
        .scan_visible_grid(&visible_tail, None)
        .expect("closing delimiter requests the pending opening from history");
    assert_eq!(history_anchor, FormulaAnchor { row: 40, column: 0 });

    let history = grid_at(
        &[
            "$$                      ",
            r"\begin{aligned}         ",
            r"f(x) &= x^2 \\          ",
            "h(x) &= x^3            ",
            "g(x) &= x+1            ",
            r"\end{aligned}           ",
            "$$                      ",
            "tail                    ",
        ],
        40,
        0,
    );
    assert!(state.complete_pending_from_history(&history));

    let overlays = state.visible_overlays(&visible_tail);
    assert_eq!(overlays.len(), 1);
    assert!(overlays[0].source.contains("f(x)"));
    assert_eq!(overlays[0].spans.first().map(|span| span.row), Some(-4));
    assert_eq!(overlays[0].spans.last().map(|span| span.row), Some(2));
}

#[test]
fn visible_content_mismatch_drops_a_persisted_formula() {
    let mut state = TerminalMathState::default();
    let initial = grid_at(&["$$   ", "x^2  ", "$$   ", "tail "], 40, 0);
    remember_visible(&mut state, &initial);

    let changed = grid_at(&["y^2  ", "$$   ", "tail ", "next "], 41, 0);
    state.synchronize_grid(&changed);
    assert!(state.visible_overlays(&changed).is_empty());
    assert!(state.formulas.is_empty());
}

#[test]
fn tui_redraw_blank_interim_frame_keeps_the_persisted_formula() {
    let mut state = TerminalMathState::default();
    let initial = grid_at(&["$$   ", "x^2  ", "$$   ", "tail "], 40, 0);
    remember_visible(&mut state, &initial);
    assert_eq!(state.formulas.len(), 1);

    // A TUI clears a line before repainting it: a partially blanked frame
    // must not evict the formula (this was the input-time flicker).
    let interim = grid_at(&["$$   ", "     ", "$$   ", "tail "], 40, 0);
    state.synchronize_grid(&interim);
    assert_eq!(state.visible_overlays(&interim).len(), 1);
    assert_eq!(state.formulas.len(), 1);

    // Rewritten with different visible content -> genuinely gone.
    let changed = grid_at(&["other", "words", "here ", "tail "], 40, 0);
    state.synchronize_grid(&changed);
    assert!(state.visible_overlays(&changed).is_empty());
}

#[test]
fn orphan_closing_delimiter_recovers_the_formula_from_history() {
    let mut state = TerminalMathState::default();
    // Viewport starts below the formula opener: only the closing `$$` is
    // visible (e.g. the persisted copy was evicted by a redraw glitch).
    let visible = grid_at(&["$$   ", "tail "], 45, 0);
    state.synchronize_grid(&visible);
    assert!(state.scan_visible_grid(&visible, None).is_none());

    let anchor = state
        .scan_visible_grid(&visible, None)
        .expect("stable orphan delimiter requests one history pass");
    assert_eq!(anchor, FormulaAnchor { row: 45, column: 0 });

    let history = grid_at(&["$$   ", "x^2  ", "$$   ", "tail "], 43, 0);
    assert!(state.complete_pending_from_history(&history));

    let overlays = state.visible_overlays(&visible);
    assert_eq!(overlays.len(), 1);
    assert_eq!(overlays[0].source.as_ref(), "x^2");
    assert_eq!(overlays[0].spans.first().map(|span| span.row), Some(-2));

    // The attempt is consumed: the same orphan never re-triggers, even
    // though the scanner keeps reporting it as unmatched every frame.
    assert!(state.scan_visible_grid(&visible, None).is_none());
    assert!(state.scan_visible_grid(&visible, None).is_none());
}

#[test]
fn reflow_history_pruning_and_memory_limits_invalidate_bounded_state() {
    let mut state = TerminalMathState::default();
    for index in 0..MAX_PERSISTED_FORMULAS + 32 {
        let absolute_top = index * 3;
        let grid = grid_at(&["$$ ", "x^2", "$$ "], absolute_top, 0);
        remember_visible(&mut state, &grid);
    }
    assert!(state.formulas.len() <= MAX_PERSISTED_FORMULAS);
    assert!(state.persisted_bytes <= PERSISTED_FORMULA_BUDGET);

    let reflowed = grid_at(&["$$  ", "x^2 ", "$$  "], 0, 0);
    state.synchronize_grid(&reflowed);
    assert!(state.formulas.is_empty());

    let initial = grid_at(&["$$ ", "x^2", "$$ "], 90, 0);
    remember_visible(&mut state, &initial);
    let pruned = grid_at(&["text"], 93, 93);
    state.synchronize_grid(&pruned);
    assert!(state.formulas.is_empty());
}
