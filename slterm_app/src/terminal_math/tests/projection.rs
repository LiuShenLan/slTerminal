use super::*;

// 投影、视觉列与源边界命中。

#[test]
fn compact_projection_moves_only_render_coordinates() {
    let grid = TextGrid::from_rows(&["pre $x^2$ suffix"]);
    let original = grid.rows.clone();
    let overlays = scan_grid(&grid);
    assert_eq!(overlays.len(), 1);
    let span = overlays[0].spans[0];
    let projection = LineProjection::build(&overlays, &[compact_prepared(2)]);

    assert!(
        projection
            .project_cell(Point::new(0, Column(span.start)), 80)
            .is_none()
    );
    let suffix = projection
        .project_cell(Point::new(0, Column(span.end)), 80)
        .expect("suffix remains visible");
    assert_eq!(suffix.column.0, span.start + 2);
    assert_eq!(
        grid.rows, original,
        "projection must not mutate terminal cells"
    );
}

#[test]
fn compact_projection_retains_background_under_visual_formula_box() {
    let grid = TextGrid::from_rows(&["pre $x^2$ suffix"]);
    let overlays = scan_grid(&grid);
    let span = overlays[0].spans[0];
    let projection = LineProjection::build(&overlays, &[compact_prepared(2)]);

    let first = projection
        .project_formula_background(Point::new(0, Column(span.start)), 80)
        .expect("first visual formula cell keeps its background");
    let second = projection
        .project_formula_background(Point::new(0, Column(span.start + 1)), 80)
        .expect("second visual formula cell keeps its background");
    assert_eq!(first.column.0, span.start);
    assert_eq!(second.column.0, span.start + 1);
    assert!(
        projection
            .project_formula_background(Point::new(0, Column(span.start + 2)), 80)
            .is_none(),
        "source cells beyond the compact formula width must not create a background strip"
    );
}

#[test]
fn disabled_projection_keeps_fixed_columns() {
    let grid = TextGrid::from_rows(&["pre $x^2$                         sidebar"]);
    let overlays = scan_grid(&grid);
    let span = overlays[0].spans[0];
    let prepared = [compact_prepared(2)];
    let mut state = TerminalMathState::default();

    state.update_projection(&overlays, &prepared, false);

    let source = state
        .project_formula_background(Point::new(0, Column(span.start)), 80)
        .expect("formula background remains at its TUI column");
    let sidebar_column = grid.rows[0]
        .iter()
        .position(|cell| *cell == Some('s'))
        .unwrap();
    let sidebar = state
        .project_cell(Point::new(0, Column(sidebar_column)), 80)
        .expect("sidebar remains visible");
    assert_eq!(source.column.0, span.start);
    assert_eq!(sidebar.column.0, sidebar_column);
}

#[test]
fn compact_projection_discards_overlapping_streaming_candidate() {
    let grid = TextGrid::from_rows(&["pre $x^2$ suffix"]);
    let outer = scan_grid(&grid).remove(0);
    let outer_span = outer.spans[0];
    let mut inner = outer.clone();
    inner.spans[0].start += 1;
    inner.spans[0].end -= 1;
    let mut state = TerminalMathState::default();
    let retained = state.update_projection_with_survivors(
        &[inner, outer],
        &[compact_prepared(1), compact_prepared(2)],
        true,
    );
    let projection = state.projection_snapshot();

    assert_eq!(projection.spans.len(), 1);
    assert_eq!(projection.spans[0].source_start, outer_span.start);
    assert_eq!(projection.spans[0].source_end, outer_span.end);
    assert_eq!(
        retained,
        [false, true],
        "GPUI must drop the same overlapping candidate"
    );
}

#[test]
fn compact_projection_accumulates_multiple_formula_shifts() {
    let grid = TextGrid::from_rows(&["a $x^2$ b $y^2$ c"]);
    let overlays = scan_grid(&grid);
    assert_eq!(overlays.len(), 2);
    let prepared = [compact_prepared(2), compact_prepared(2)];
    let projection = LineProjection::build(&overlays, &prepared);
    let last = overlays[1].spans[0];
    let source_reduction: usize = overlays
        .iter()
        .map(|overlay| overlay.spans[0].end - overlay.spans[0].start - 2)
        .sum();

    let suffix = projection
        .project_cell(Point::new(0, Column(last.end)), 80)
        .expect("suffix remains visible");
    assert_eq!(suffix.column.0, last.end - source_reduction);
}

#[test]
fn formula_hit_testing_returns_source_boundaries() {
    let grid = TextGrid::from_rows(&["pre $x^2$ suffix"]);
    let overlays = scan_grid(&grid);
    let span = overlays[0].spans[0];
    let projection = LineProjection::build(&overlays, &[compact_prepared(2)]);

    let (left, left_side) =
        projection.source_from_visual(Point::new(0, Column(span.start)), Side::Left);
    let (right, right_side) =
        projection.source_from_visual(Point::new(0, Column(span.start + 1)), Side::Right);
    assert_eq!((left.column.0, left_side), (span.start, Side::Left));
    assert_eq!((right.column.0, right_side), (span.end - 1, Side::Right));
}

#[test]
fn formula_hit_testing_preserves_nonzero_viewport_origin() {
    let grid = TextGrid::from_rows(&["pre $x^2$ suffix"]);
    let overlays = scan_grid(&grid);
    let span = overlays[0].spans[0];
    let mut state = TerminalMathState::default();
    state.update_projection(&overlays, &[compact_prepared(2)], true);
    let viewport_origin = Line(-7);

    let (left, left_side) = state.source_point(
        Point::new(viewport_origin, Column(span.start)),
        Side::Left,
        viewport_origin,
    );
    let (suffix, suffix_side) = state.source_point(
        Point::new(viewport_origin, Column(span.start + 2)),
        Side::Left,
        viewport_origin,
    );

    assert_eq!(
        (left.line, left.column.0, left_side),
        (viewport_origin, span.start, Side::Left)
    );
    assert_eq!(
        (suffix.line, suffix.column.0, suffix_side),
        (viewport_origin, span.end, Side::Left)
    );
}
