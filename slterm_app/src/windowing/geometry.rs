//! 窗口几何：按机器持久化尺寸、位置、最大化与显示器信息。
//!
//! 恢复时把几何夹回当前可见工作区；显示器移除或工作区变化后窗口必须
//! 保持标题栏与主要操作可达。

use gpui::{Bounds, Pixels, WindowBounds, point, px, size};
use serde::{Deserialize, Serialize};

/// 显示器工作区的持久化矩形（逻辑像素）。
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct DisplayRect {
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) width: f32,
    pub(crate) height: f32,
}

impl DisplayRect {
    fn from_bounds(bounds: Bounds<Pixels>) -> Self {
        Self {
            x: f32::from(bounds.origin.x),
            y: f32::from(bounds.origin.y),
            width: f32::from(bounds.size.width),
            height: f32::from(bounds.size.height),
        }
    }

    fn to_bounds(self) -> Bounds<Pixels> {
        Bounds::new(
            point(px(self.x), px(self.y)),
            size(px(self.width), px(self.height)),
        )
    }
}

/// 按机器保存的窗口几何；`display` 是保存时所在显示器的工作区。
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct WindowGeometry {
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) width: f32,
    pub(crate) height: f32,
    pub(crate) maximized: bool,
    #[serde(default)]
    pub(crate) display: Option<DisplayRect>,
}

impl WindowGeometry {
    /// 从窗口当前状态取快照；全屏按最大化意图保存（退出后回到最大化）。
    pub(crate) fn from_window(bounds: WindowBounds, display: Option<Bounds<Pixels>>) -> Self {
        let maximized = matches!(
            bounds,
            WindowBounds::Maximized(_) | WindowBounds::Fullscreen(_)
        );
        let bounds = bounds.get_bounds();
        Self {
            x: f32::from(bounds.origin.x),
            y: f32::from(bounds.origin.y),
            width: f32::from(bounds.size.width),
            height: f32::from(bounds.size.height),
            maximized,
            display: display.map(DisplayRect::from_bounds),
        }
    }

    /// 夹回当前可见工作区；没有任何显示器信息时返回 None 交给调用方兜底。
    ///
    /// 保存时的显示器仍在则保留位置，只把越界部分拉回；显示器已移除或
    /// 工作区缩小则落到主显示器，窗口完整位于其工作区内。
    pub(crate) fn restored_bounds(
        &self,
        visible: &[Bounds<Pixels>],
        primary: Option<Bounds<Pixels>>,
    ) -> Option<WindowBounds> {
        let work_area = self.pick_work_area(visible, primary)?;
        let work_width = f32::from(work_area.size.width);
        let work_height = f32::from(work_area.size.height);
        let width = self.width.min(work_width);
        let height = self.height.min(work_height);
        let origin_x = f32::from(work_area.origin.x);
        let origin_y = f32::from(work_area.origin.y);
        let max_x = (origin_x + work_width - width).max(origin_x);
        let max_y = (origin_y + work_height - height).max(origin_y);
        let x = self.x.clamp(origin_x, max_x);
        let y = self.y.clamp(origin_y, max_y);
        let bounds = Bounds::new(point(px(x), px(y)), size(px(width), px(height)));
        Some(if self.maximized {
            WindowBounds::Maximized(bounds)
        } else {
            WindowBounds::Windowed(bounds)
        })
    }

    /// 选择目标工作区：优先与保存显示器交叠最多者，否则主显示器。
    fn pick_work_area(
        &self,
        visible: &[Bounds<Pixels>],
        primary: Option<Bounds<Pixels>>,
    ) -> Option<Bounds<Pixels>> {
        if let Some(display) = self.display {
            let saved = display.to_bounds();
            let best = visible
                .iter()
                .copied()
                .max_by(|left, right| {
                    overlap(saved, *left)
                        .partial_cmp(&overlap(saved, *right))
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .filter(|best| overlap(saved, *best) > 0.0);
            if best.is_some() {
                return best;
            }
        }
        primary.or_else(|| visible.first().copied())
    }
}

/// 两个矩形的交叠面积；无交叠为零。
fn overlap(left: Bounds<Pixels>, right: Bounds<Pixels>) -> f32 {
    let left_x = f32::from(left.origin.x);
    let left_y = f32::from(left.origin.y);
    let right_x = f32::from(right.origin.x);
    let right_y = f32::from(right.origin.y);
    let width = (f32::from(left.origin.x + left.size.width)
        .min(f32::from(right.origin.x + right.size.width))
        - left_x.max(right_x))
    .max(0.0);
    let height = (f32::from(left.origin.y + left.size.height)
        .min(f32::from(right.origin.y + right.size.height))
        - left_y.max(right_y))
    .max(0.0);
    width * height
}

#[cfg(test)]
mod geometry_tests {
    use gpui::{Bounds, Pixels, WindowBounds, point, px, size};

    use super::*;

    fn area(x: f32, y: f32, width: f32, height: f32) -> Bounds<Pixels> {
        Bounds::new(point(px(x), px(y)), size(px(width), px(height)))
    }

    fn assert_inside(bounds: Bounds<Pixels>, work_area: Bounds<Pixels>) {
        assert!(
            bounds.origin.x >= work_area.origin.x
                && bounds.origin.x + bounds.size.width <= work_area.origin.x + work_area.size.width,
            "窗口必须完整落在工作区横向范围内: {bounds:?} / {work_area:?}"
        );
        assert!(
            bounds.origin.y >= work_area.origin.y
                && bounds.origin.y + bounds.size.height
                    <= work_area.origin.y + work_area.size.height,
            "窗口必须完整落在工作区纵向范围内: {bounds:?} / {work_area:?}"
        );
    }

    /// 锚：移除显示器后窗口夹回可见工作区。
    #[test]
    fn windows_geometry_clamped_to_work_area() {
        // 保存在已移除的右侧显示器上。
        let saved = WindowGeometry {
            x: 2200.0,
            y: 200.0,
            width: 1200.0,
            height: 800.0,
            maximized: false,
            display: Some(DisplayRect {
                x: 2000.0,
                y: 0.0,
                width: 1920.0,
                height: 1080.0,
            }),
        };
        let primary = area(0.0, 0.0, 1920.0, 1040.0);

        let restored = saved
            .restored_bounds(&[primary], Some(primary))
            .expect("必须产生可见几何");
        assert!(!matches!(restored, WindowBounds::Maximized(_)));
        assert_inside(restored.get_bounds(), primary);
    }

    /// 显示器仍在时保留位置，只夹回越界部分。
    #[test]
    fn geometry_keeps_position_when_the_display_still_exists() {
        let saved = WindowGeometry {
            x: 100.0,
            y: 120.0,
            width: 900.0,
            height: 700.0,
            maximized: false,
            display: Some(DisplayRect {
                x: 0.0,
                y: 0.0,
                width: 1920.0,
                height: 1040.0,
            }),
        };
        let primary = area(0.0, 0.0, 1920.0, 1040.0);

        let bounds = saved
            .restored_bounds(&[primary], Some(primary))
            .unwrap()
            .get_bounds();
        assert_eq!(bounds.origin, point(px(100.0), px(120.0)));
        assert_inside(bounds, primary);
    }

    /// DPI 或工作区变化后尺寸收缩，位置回到可达范围。
    #[test]
    fn geometry_shrinks_into_a_smaller_work_area() {
        let saved = WindowGeometry {
            x: 1000.0,
            y: 800.0,
            width: 1200.0,
            height: 900.0,
            maximized: false,
            display: Some(DisplayRect {
                x: 0.0,
                y: 0.0,
                width: 1920.0,
                height: 1040.0,
            }),
        };
        let smaller = area(0.0, 0.0, 1280.0, 720.0);

        let bounds = saved
            .restored_bounds(&[smaller], Some(smaller))
            .unwrap()
            .get_bounds();
        assert_eq!(bounds.size, size(px(1200.0), px(720.0)));
        assert_inside(bounds, smaller);
    }

    /// 最大化窗口按最大化恢复，还原尺寸同样夹回可见区。
    #[test]
    fn maximized_geometry_restores_maximized_with_visible_restore_bounds() {
        let saved = WindowGeometry {
            x: 2200.0,
            y: 200.0,
            width: 1200.0,
            height: 800.0,
            maximized: true,
            display: None,
        };
        let primary = area(0.0, 0.0, 1920.0, 1040.0);

        let restored = saved.restored_bounds(&[primary], Some(primary)).unwrap();
        assert!(matches!(restored, WindowBounds::Maximized(_)));
        assert_inside(restored.get_bounds(), primary);
    }

    /// 窗口快照只记录窗口化几何；全屏按最大化意图保存。
    #[test]
    fn window_snapshot_records_bounds_and_display() {
        let bounds = Bounds::new(point(px(10.0), px(20.0)), size(px(800.0), px(600.0)));
        let display = area(0.0, 0.0, 1920.0, 1040.0);

        let windowed = WindowGeometry::from_window(WindowBounds::Windowed(bounds), Some(display));
        assert!(!windowed.maximized);
        assert_eq!(windowed.x, 10.0);
        assert_eq!(windowed.width, 800.0);
        assert_eq!(
            windowed.display,
            Some(DisplayRect {
                x: 0.0,
                y: 0.0,
                width: 1920.0,
                height: 1040.0
            })
        );

        let maximized = WindowGeometry::from_window(WindowBounds::Maximized(bounds), Some(display));
        assert!(maximized.maximized);

        let fullscreen =
            WindowGeometry::from_window(WindowBounds::Fullscreen(bounds), Some(display));
        assert!(fullscreen.maximized, "全屏按最大化意图保存");
    }

    /// 没有任何显示器信息时不伪造几何。
    #[test]
    fn geometry_without_any_display_has_no_restore_target() {
        let saved = WindowGeometry {
            x: 0.0,
            y: 0.0,
            width: 800.0,
            height: 600.0,
            maximized: false,
            display: None,
        };
        assert!(saved.restored_bounds(&[], None).is_none());
    }
}
