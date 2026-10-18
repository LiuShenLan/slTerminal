//! 壁纸尺寸与对齐的纯数学面,独立于 GPUI 渲染。
//!
//! 照上游 `renderer/image_layout.rs` 裁剪:只保留壁纸消费的 fit/alignment
//! 解析与 `wallpaper_rect`;主题编辑器的下拉语义(settings_value/next)
//! 归设置主题体系,不在此重复。

/// 壁纸适应模式(CSS background-size 语义)。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BackgroundImageFit {
    /// 拉伸到窗口,不保比例。
    Fill,
    /// 保比例,整图可见(留黑边)。
    Uniform,
    /// 保比例,裁掉溢出(CSS `cover`)。
    #[default]
    UniformToFill,
    /// 按图片原始像素尺寸绘制。
    None,
}

impl BackgroundImageFit {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "fill" | "stretch" => Some(Self::Fill),
            "uniform" | "contain" => Some(Self::Uniform),
            "uniform_to_fill" | "uniformtofill" | "cover" => Some(Self::UniformToFill),
            "none" | "native" => Some(Self::None),
            _ => None,
        }
    }
}

/// 壁纸相对窗口的九宫格锚点。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BackgroundImageAlignment {
    TopLeft,
    Top,
    TopRight,
    Left,
    #[default]
    Center,
    Right,
    BottomLeft,
    Bottom,
    BottomRight,
}

impl BackgroundImageAlignment {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().replace('_', "-").as_str() {
            "top-left" => Some(Self::TopLeft),
            "top" => Some(Self::Top),
            "top-right" => Some(Self::TopRight),
            "left" => Some(Self::Left),
            "center" | "centre" => Some(Self::Center),
            "right" => Some(Self::Right),
            "bottom-left" => Some(Self::BottomLeft),
            "bottom" => Some(Self::Bottom),
            "bottom-right" => Some(Self::BottomRight),
            _ => None,
        }
    }

    /// 对齐锚点系数(0=贴起始边,0.5=居中,1=贴结束边)。
    pub(crate) const fn factors(self) -> (f32, f32) {
        match self {
            Self::TopLeft => (0.0, 0.0),
            Self::Top => (0.5, 0.0),
            Self::TopRight => (1.0, 0.0),
            Self::Left => (0.0, 0.5),
            Self::Center => (0.5, 0.5),
            Self::Right => (1.0, 0.5),
            Self::BottomLeft => (0.0, 1.0),
            Self::Bottom => (0.5, 1.0),
            Self::BottomRight => (1.0, 1.0),
        }
    }
}

/// 计算图片在目标区域中的落点 `(x, y, width, height)`。
///
/// 输入输出均为同一坐标系(调用方负责逻辑/物理像素换算);目标与图片
/// 尺寸下限 1,避免零尺寸除零。
pub(crate) fn wallpaper_rect(
    window_w: f32,
    window_h: f32,
    image_w: f32,
    image_h: f32,
    fit: BackgroundImageFit,
    alignment: BackgroundImageAlignment,
) -> (f32, f32, f32, f32) {
    let window_w = window_w.max(1.0);
    let window_h = window_h.max(1.0);
    let image_w = image_w.max(1.0);
    let image_h = image_h.max(1.0);

    let (draw_w, draw_h) = match fit {
        BackgroundImageFit::Fill => (window_w, window_h),
        BackgroundImageFit::Uniform => {
            let scale = (window_w / image_w).min(window_h / image_h);
            (image_w * scale, image_h * scale)
        }
        BackgroundImageFit::UniformToFill => {
            let scale = (window_w / image_w).max(window_h / image_h);
            (image_w * scale, image_h * scale)
        }
        BackgroundImageFit::None => (image_w, image_h),
    };
    let (align_x, align_y) = alignment.factors();
    let x0 = (window_w - draw_w) * align_x;
    let y0 = (window_h - draw_h) * align_y;
    (x0, y0, draw_w, draw_h)
}

#[cfg(test)]
mod image_layout_tests {
    use super::*;

    #[test]
    fn fit_modes_preserve_the_expected_geometry() {
        // 200x100 图片铺进 100x100 区域。
        assert_eq!(
            wallpaper_rect(
                100.0,
                100.0,
                200.0,
                100.0,
                BackgroundImageFit::Fill,
                BackgroundImageAlignment::TopLeft,
            ),
            (0.0, 0.0, 100.0, 100.0),
        );
        assert_eq!(
            wallpaper_rect(
                100.0,
                100.0,
                200.0,
                100.0,
                BackgroundImageFit::Uniform,
                BackgroundImageAlignment::Center,
            ),
            (0.0, 25.0, 100.0, 50.0),
        );
        assert_eq!(
            wallpaper_rect(
                100.0,
                100.0,
                200.0,
                100.0,
                BackgroundImageFit::UniformToFill,
                BackgroundImageAlignment::Center,
            ),
            (-50.0, 0.0, 200.0, 100.0),
        );
        assert_eq!(
            wallpaper_rect(
                100.0,
                100.0,
                200.0,
                100.0,
                BackgroundImageFit::None,
                BackgroundImageAlignment::BottomRight,
            ),
            (-100.0, 0.0, 200.0, 100.0),
        );
    }

    #[test]
    fn zero_sizes_are_clamped_to_one_pixel() {
        assert_eq!(
            wallpaper_rect(
                0.0,
                0.0,
                0.0,
                0.0,
                BackgroundImageFit::Uniform,
                BackgroundImageAlignment::Center,
            ),
            (0.0, 0.0, 1.0, 1.0),
        );
    }

    #[test]
    fn fit_and_alignment_parsers_tolerate_case_and_separators() {
        assert_eq!(
            BackgroundImageFit::parse(" Uniform_To_Fill "),
            Some(BackgroundImageFit::UniformToFill)
        );
        assert_eq!(
            BackgroundImageFit::parse("contain"),
            Some(BackgroundImageFit::Uniform)
        );
        assert_eq!(
            BackgroundImageFit::parse("stretch"),
            Some(BackgroundImageFit::Fill)
        );
        assert_eq!(
            BackgroundImageFit::parse("native"),
            Some(BackgroundImageFit::None)
        );
        assert_eq!(BackgroundImageFit::parse("bogus"), None);

        assert_eq!(
            BackgroundImageAlignment::parse("TOP_LEFT"),
            Some(BackgroundImageAlignment::TopLeft)
        );
        assert_eq!(
            BackgroundImageAlignment::parse("centre"),
            Some(BackgroundImageAlignment::Center)
        );
        assert_eq!(
            BackgroundImageAlignment::parse("bottom-right"),
            Some(BackgroundImageAlignment::BottomRight)
        );
        assert_eq!(BackgroundImageAlignment::parse("middle"), None);
    }
}
