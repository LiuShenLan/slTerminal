//! 阅读器几何常量族(07 篇 M8.3 照抄点 14):单点定义,消费方零字面量。
//!
//! 数值按 slTerminal 视觉重定(设计稿允许);页面宽/侧栏宽/标题字号档位
//! 都从这里取,渲染代码不得另写字面量。

pub(super) const PAGE_WIDTH: f32 = 720.0;
pub(super) const PAGE_MARGIN: f32 = 40.0;
pub(super) const TOP_MARGIN: f32 = 24.0;
pub(super) const BODY_SIZE: f32 = 14.0;
pub(super) const HEADING_BASE: f32 = BODY_SIZE;
pub(super) const LINE_HEIGHT: f32 = 1.7;
pub(super) const OUTLINE_WIDTH: f32 = 248.0;
pub(super) const OUTLINE_INDENT: f32 = 12.0;
pub(super) const OUTLINE_ROW_HEIGHT: f32 = 32.0;
/// 大纲与信息视图共享同一面板宽度:保留可读导航列,同时给正文留足空间。
pub(super) const DETAILS_MIN_WIDTH: f32 = 216.0;
pub(super) const DETAILS_MAX_WIDTH: f32 = 360.0;
/// 可见分隔线只有 1px,指针命中区刻意加宽,不依赖精确落点。
pub(super) const DETAILS_RESIZE_HIT_WIDTH: f32 = 8.0;
pub(super) const CHROME_SIZE: f32 = 13.0;
pub(super) const SECONDARY_SIZE: f32 = 12.0;
pub(super) const ICON_SIZE: f32 = 16.0;
pub(super) const TOOLBAR_HEIGHT: f32 = 52.0;
pub(super) const PANEL_HEADER_HEIGHT: f32 = 44.0;
pub(super) const CONTROL_HEIGHT: f32 = 32.0;
pub(super) const ACTION_HEIGHT: f32 = 36.0;
pub(super) const PANEL_PADDING: f32 = 20.0;
/// 空块占位高度:虚拟列表对空段落也按行高估计,避免零高行无法命中。
pub(super) const BLOCK_MIN_HEIGHT: f32 = 32.0;

pub(super) fn clamp_details_width(width: f32) -> f32 {
    width.clamp(DETAILS_MIN_WIDTH, DETAILS_MAX_WIDTH)
}

/// 文档层级跟随正文字号缩放;聚焦输入的标题字号与渲染标题保持同一档。
pub(super) fn heading_size(level: Option<u8>) -> f32 {
    BODY_SIZE
        * match level {
            Some(1) => 2.0,
            Some(2) => 1.5,
            Some(3) => 1.25,
            Some(4) => 1.125,
            _ => 1.0,
        }
}

pub(super) fn heading_weight(level: Option<u8>) -> gpui::FontWeight {
    match level {
        Some(1) => gpui::FontWeight::BOLD,
        Some(2..=5) => gpui::FontWeight::SEMIBOLD,
        Some(6) => gpui::FontWeight::MEDIUM,
        _ => gpui::FontWeight::NORMAL,
    }
}

#[cfg(test)]
mod reader_presentation_tests {
    use super::*;

    /// 详情侧栏钳制:低于下限/高于上限都收敛,区间内原样。
    #[test]
    fn details_width_clamps_to_the_readable_range() {
        assert_eq!(clamp_details_width(100.0), DETAILS_MIN_WIDTH);
        assert_eq!(clamp_details_width(300.0), 300.0);
        assert_eq!(clamp_details_width(600.0), DETAILS_MAX_WIDTH);
    }

    /// 标题档位:正文基准逐级放大,六级与普通文本回落到基准。
    #[test]
    fn heading_sizes_scale_from_the_body_size() {
        assert_eq!(heading_size(Some(1)), BODY_SIZE * 2.0);
        assert_eq!(heading_size(Some(3)), BODY_SIZE * 1.25);
        assert_eq!(heading_size(Some(6)), BODY_SIZE);
        assert_eq!(heading_size(None), BODY_SIZE);
    }
}
