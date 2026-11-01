//! 初始原生窗口与首个终端网格共用同一条尺寸规则。
//!
//! 量的是配置基准字号，不是持久化的终端缩放。平台能给出显示器 DPI 时
//! 在窗口亮相前提供最终尺寸。垂直 chrome 包含标题栏、终端 padding 与圆整余量。

use gpui::{App, Pixels, Size, px, size};

use crate::gpui_terminal::view::TerminalView;

fn chrome_size() -> Size<Pixels> {
    // 终端 padding、沟槽与既有的 2px 圆整余量。
    size(px(16.0 + 24.0 + 2.0), px(34.0 + 16.0 + 16.0 + 2.0))
}

fn default_size(
    metrics: (Pixels, Pixels),
    grid: (f32, f32),
    display_limit: Option<Size<Pixels>>,
) -> Size<Pixels> {
    let chrome = chrome_size();
    let preferred = size(
        metrics.0 * grid.0 + chrome.width,
        metrics.1 * grid.1 + chrome.height,
    );
    display_limit.map_or(preferred, |limit| preferred.min(&limit))
}

fn display_limit(cx: &App) -> Option<Size<Pixels>> {
    cx.primary_display().map(|display| {
        let bounds = display.bounds().size;
        size(bounds.width * 0.95, bounds.height * 0.95)
    })
}

fn fit_native_size(preferred: Size<Pixels>, visible: Option<Size<Pixels>>) -> Size<Pixels> {
    // 很小的配置字号也保留既有的 resize 下限；比下限还小的显示器仍握有上界。
    let preferred = preferred.max(&size(px(760.0), px(540.0)));
    visible.map_or(preferred, |visible| preferred.min(&visible))
}

fn fit_preflight_size(preferred: Size<Pixels>, cx: &App) -> Size<Pixels> {
    fit_native_size(
        preferred,
        cx.primary_display()
            .map(|display| display.visible_bounds().size),
    )
}

/// 启动网格：恒默认画布，持久化几何恢复由新启动协调器负责。
fn startup_grid() -> (f32, f32) {
    (
        f32::from(TerminalView::DEFAULT_GRID_COLUMNS),
        f32::from(TerminalView::DEFAULT_GRID_LINES),
    )
}

pub(super) fn preferred_size(cx: &App) -> Option<Size<Pixels>> {
    let scale = crate::platform::startup::primary_display_scale()?;
    Some(fit_preflight_size(
        default_size(
            TerminalView::startup_cell_metrics_at_scale(scale, cx),
            startup_grid(),
            display_limit(cx),
        ),
        cx,
    ))
}

#[cfg(test)]
mod startup_geometry_tests {
    use super::*;

    #[test]
    fn initial_window_keeps_the_base_font_grid_and_fits_the_display() {
        let metrics = (px(8.0), px(20.0));
        let grid = (
            f32::from(TerminalView::DEFAULT_GRID_COLUMNS),
            f32::from(TerminalView::DEFAULT_GRID_LINES),
        );
        assert_eq!(
            default_size(metrics, grid, None),
            size(px(970.0), px(668.0))
        );
        assert_eq!(
            default_size(metrics, grid, Some(size(px(900.0), px(600.0)))),
            size(px(900.0), px(600.0)),
        );
    }

    #[test]
    fn small_fonts_keep_the_resize_floor_unless_the_display_is_smaller() {
        let small_font = size(px(504.0), px(248.0));
        assert_eq!(
            fit_native_size(small_font, None),
            size(px(760.0), px(540.0))
        );
        assert_eq!(
            fit_native_size(small_font, Some(size(px(1920.0), px(1040.0)))),
            size(px(760.0), px(540.0)),
        );
        assert_eq!(
            fit_native_size(small_font, Some(size(px(640.0), px(480.0)))),
            size(px(640.0), px(480.0)),
        );
        assert_eq!(
            fit_native_size(
                size(px(1200.0), px(668.0)),
                Some(size(px(1000.0), px(600.0)))
            ),
            size(px(1000.0), px(600.0)),
        );
    }
}
