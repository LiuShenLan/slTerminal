//! 终端数学覆盖层单元测试。共享夹具集中在本文件，用例按职责分到子模块。

use super::*;

fn rgb(r: u8, g: u8, b: u8) -> Rgb {
    Rgb { r, g, b }
}

fn overlay_with_source_style(flags: Flags, foreground: Rgb) -> FormulaOverlay {
    let mut overlay = scan_grid(&TextGrid::from_rows(&[r"$$E=mc^2$$"]))
        .into_iter()
        .next()
        .expect("test formula");
    overlay.fallback = "$$E=mc^2$$"
        .chars()
        .enumerate()
        .map(|(column, character)| OverlayCell {
            character,
            point: Point::new(0, Column(column)),
            fg: foreground,
            bg_alpha: 1.0,
            flags,
        })
        .collect();
    overlay
}

fn compact_prepared(cells: usize) -> Option<PreparedFormula> {
    Some(PreparedFormula {
        fitted_pixel_size: TEST_FONT_PX,
        display_style: false,
        bleed_top: 0.0,
        bleed_bottom: 0.0,
        box_right: 0.0,
        centered: true,
        compact_cells: Some(cells),
    })
}

fn sources(rows: &[&str]) -> Vec<(String, bool)> {
    scan_grid(&TextGrid::from_rows(rows))
        .into_iter()
        .map(|formula| (formula.source.to_string(), formula.display))
        .collect()
}

fn grid_at(rows: &[&str], absolute_top: usize, scrolled_out: usize) -> TextGrid {
    let mut grid = TextGrid::from_rows(rows);
    grid.absolute_top = absolute_top;
    grid.scrolled_out = scrolled_out;
    grid
}

fn remember_visible(state: &mut TerminalMathState, grid: &TextGrid) {
    state.synchronize_grid(grid);
    for overlay in scan_grid(grid) {
        state.remember(grid, &overlay);
    }
}

/// Cell geometry proportional to a real terminal font: monospace advance
/// is ~0.6 em and line height ~1.3 em, so budgets in these tests mean the
/// same thing they mean on screen.
fn test_size(columns: f32, lines: f32) -> MathViewport {
    MathViewport::new(columns * 12.0, lines * 26.0, 12.0, 26.0, 0.0, 0.0)
}

/// Nominal terminal font size behind [`test_size`].
const TEST_FONT_PX: f32 = 20.0;

fn fitted_size(rows: &[&str]) -> f32 {
    let grid = TextGrid::from_rows(rows);
    let overlays = scan_grid_with_hints(&grid);
    assert_eq!(
        overlays.len(),
        1,
        "expected exactly one formula in {rows:?}"
    );
    let size = test_size(grid.columns as f32, grid.rows.len() as f32);
    let mut state = TerminalMathState::default();
    let prepared = prepare_overlays(&mut state, &overlays, &size, TEST_FONT_PX, 1.0);
    prepared[0].expect("formula must render").fitted_pixel_size
}

// 按职责分组的测试子模块。
mod blocks;
mod cost;
mod layout;
mod persistence;
mod projection;
mod scanning;
