//! 初始原生窗口与首个终端网格共用同一条尺寸规则。
//!
//! 量的是配置基准字号,不是持久化的终端缩放。平台能给出显示器 DPI 时
//! 在窗口亮相前提供最终尺寸;否则保留建成后再量窗口的回退路径。
//!
//! 侧栏宽度锚(M5.3b 起):ActivityBar + 内容区默认宽度 + 拖拽热区;
//! `chrome_size` 的垂直量(34 标题栏 + 16 + 16 + 2 圆整)与 TerminalView 的
//! padding(py 8×2 = 16 含在内)同链——改任一侧,启动零 resize 锚即断。

use gpui::{App, Pixels, Size, Window, px, size};

use crate::gpui_terminal::view::TerminalView;

fn chrome_size(sidebar_width: f32) -> Size<Pixels> {
    // 侧栏、沟槽、终端 padding,外加既有的 2px 圆整余量。
    size(
        px(sidebar_width + 16.0 + 24.0 + 2.0),
        px(34.0 + 16.0 + 16.0 + 2.0),
    )
}

fn default_size(
    metrics: (Pixels, Pixels),
    grid: (f32, f32),
    sidebar_width: f32,
    display_limit: Option<Size<Pixels>>,
) -> Size<Pixels> {
    let chrome = chrome_size(sidebar_width);
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
    // 很小的配置字号也保留既有的 resize 下限;比下限还小的显示器仍握有上界。
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

/// 启动网格:M3 恒默认画布(持久化网格恢复归 05 篇会话链)。
fn startup_grid() -> (f32, f32) {
    (
        f32::from(TerminalView::DEFAULT_GRID_COLUMNS),
        f32::from(TerminalView::DEFAULT_GRID_LINES),
    )
}

pub(super) fn preferred_size(cx: &App, sidebar_width: f32) -> Option<Size<Pixels>> {
    let scale = crate::platform::startup::primary_display_scale()?;
    Some(fit_preflight_size(
        default_size(
            TerminalView::startup_cell_metrics_at_scale(scale, cx),
            startup_grid(),
            sidebar_width,
            display_limit(cx),
        ),
        cx,
    ))
}

fn same_device_size(actual: Size<Pixels>, requested: Size<Pixels>, scale: f32) -> bool {
    // 窗口放置把逻辑尺寸量化到设备像素;亚像素往返差不能再排一次异步
    // SetWindowPos。
    (f32::from(actual.width - requested.width) * scale).abs() < 1.0
        && (f32::from(actual.height - requested.height) * scale).abs() < 1.0
}

pub(in crate::app_shell) fn prepare_initial_grid(
    window: &mut Window,
    cx: &mut App,
    sidebar_width: f32,
    fit_window_to_default_grid: bool,
) -> (u16, u16) {
    let (cell_w, line_h) = TerminalView::cell_metrics(window, cx);
    let actual = window.bounds().size;
    let target = if fit_window_to_default_grid {
        let requested = default_size(
            TerminalView::startup_cell_metrics(window, cx),
            startup_grid(),
            sidebar_width,
            display_limit(cx),
        );
        let requested = if crate::platform::startup::primary_display_scale().is_some() {
            fit_preflight_size(requested, cx)
        } else {
            // 无预检 DPI 的平台保留原尺寸策略。
            requested
        };
        if same_device_size(actual, requested, window.scale_factor()) {
            actual
        } else {
            // 建成期 DPI 变化、预检失败与不支持的平台保留「量窗口」回退,
            // 不靠猜。
            window.resize(requested);
            requested
        }
    } else {
        actual
    };
    let chrome = chrome_size(sidebar_width);
    let cols = ((f32::from(target.width - chrome.width) / f32::from(cell_w)) + 0.001)
        .floor()
        .max(2.0) as u16;
    let rows = ((f32::from(target.height - chrome.height) / f32::from(line_h)) + 0.001)
        .floor()
        .max(2.0) as u16;
    (cols, rows)
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
            default_size(metrics, grid, 230.0, None),
            size(px(1200.0), px(668.0))
        );
        assert_eq!(
            default_size(metrics, grid, 230.0, Some(size(px(1000.0), px(600.0)))),
            size(px(1000.0), px(600.0)),
        );
        assert_eq!(
            default_size(metrics, grid, 300.0, None),
            size(px(1270.0), px(668.0)),
        );
    }

    #[test]
    fn startup_size_skips_only_device_pixel_rounding_differences() {
        let requested = size(px(1200.0), px(668.0));
        for scale in [1.0, 1.25, 1.5, 2.0] {
            assert!(same_device_size(requested, requested, scale));
            assert!(same_device_size(
                size(requested.width + px(0.5 / scale), requested.height),
                requested,
                scale,
            ));
            assert!(!same_device_size(
                size(px(1080.0), px(720.0)),
                requested,
                scale
            ));
            assert!(!same_device_size(
                size(requested.width + px(2.0 / scale), requested.height),
                requested,
                scale,
            ));
        }
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
