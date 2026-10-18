//! 公式位图的 GPUI 呈现(终端覆盖层绘制面)。
//!
//! 管线与 markdown 阅读器同源:`compile_formula` 产出后端无关绘制指令 →
//! `MathGlyphRasterizer` 按物理像素把整条公式合成为 BGRA 位图 →
//! `window.paint_image` 上屏;数学字体缺字的字符用 gpui 文本系统按基线
//! 叠加补画。
//!
//! 走位图而不是 gpui 字形管线:数学排版要按字形 ID 绘制(拉伸变体、装配件
//! 都不经 cmap),而 gpui 的 `GlyphId` 构造器是 crate 私有的。位图路线让
//! 终端覆盖层与阅读器共享唯一一支栅格化器。
//!
//! 失败合同:编译失败/超预算的公式回退源码文本(调用方不遮源格)。

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use gpui::{Bounds, Corners, Pixels, RenderImage, SharedString, Window, point, px, size};
use image::Frame;

use crate::math::bitmap::{compose, required_bytes};
use crate::math::cache::{FormulaCacheKey, MathLayoutCache};
use crate::math::layout::MathLayout;
use crate::math::rasterizer::MathGlyphRasterizer;

/// 位图缓存预算:与上游科学渲染缓存同量级。超预算时整表丢弃(公式位图
/// 重建便宜,逐项 LRU 的记账复杂度不值得)。
const IMAGE_CACHE_BUDGET: usize = 48 * 1024 * 1024;

/// 位图内的定位信息(物理 px):paint 时把位图基线吸附到元素基线。
#[derive(Clone, Copy, Debug)]
pub(crate) struct ImageGeometry {
    /// 位图顶边到公式基线的距离。
    pub baseline: u32,
    /// 位图左边相对公式盒左缘的外扩(出血)。
    pub pad: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct ImageKey {
    source: String,
    display: bool,
    pixel_size_bits: u32,
    pixels_per_point_bits: u32,
    raster_scale_bits: u32,
    color: [u8; 3],
}

struct CachedImage {
    image: Arc<RenderImage>,
    geometry: ImageGeometry,
}

struct MathImages {
    rasterizer: Option<MathGlyphRasterizer>,
    entries: HashMap<ImageKey, CachedImage>,
    used: usize,
}

impl MathImages {
    fn new() -> Self {
        Self {
            rasterizer: MathGlyphRasterizer::new().ok(),
            entries: HashMap::new(),
            used: 0,
        }
    }

    fn image(
        &mut self,
        key: ImageKey,
        layout: &MathLayout,
        raster_scale: f32,
        color: [u8; 3],
    ) -> Option<(Arc<RenderImage>, ImageGeometry)> {
        if let Some(entry) = self.entries.get(&key) {
            return Some((Arc::clone(&entry.image), entry.geometry));
        }
        let rasterizer = self.rasterizer.as_ref()?;
        let expected = required_bytes(layout, raster_scale)?;
        if expected > IMAGE_CACHE_BUDGET {
            return None;
        }
        let bitmap = compose(rasterizer, layout, raster_scale, color)?;
        let geometry = ImageGeometry {
            baseline: bitmap.baseline,
            pad: bitmap.pad,
            width: bitmap.width,
            height: bitmap.height,
        };
        let buffer = image::RgbaImage::from_raw(bitmap.width, bitmap.height, bitmap.pixels)?;
        let image = Arc::new(RenderImage::new([Frame::new(buffer)]));
        let bytes = expected;
        if self.used.saturating_add(bytes) > IMAGE_CACHE_BUDGET {
            self.entries.clear();
            self.used = 0;
        }
        self.entries.insert(
            key,
            CachedImage {
                image: Arc::clone(&image),
                geometry,
            },
        );
        self.used = self.used.saturating_add(bytes);
        Some((image, geometry))
    }
}

fn images() -> &'static Mutex<MathImages> {
    static IMAGES: OnceLock<Mutex<MathImages>> = OnceLock::new();
    IMAGES.get_or_init(|| Mutex::new(MathImages::new()))
}

/// paint 与位图预检共享的布局缓存(4MB LRU 的进程级实例)。公式源码在
/// 终端覆盖层没有稳定编号,按源码+模式哈希成键。
fn layouts() -> &'static Mutex<MathLayoutCache> {
    static LAYOUTS: OnceLock<Mutex<MathLayoutCache>> = OnceLock::new();
    LAYOUTS.get_or_init(|| Mutex::new(MathLayoutCache::default()))
}

fn formula_id(source: &str, display: bool) -> u64 {
    use std::hash::{Hash as _, Hasher as _};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    display.hash(&mut hasher);
    source.hash(&mut hasher);
    hasher.finish()
}

/// 取/编译一条公式布局。`pixels_per_point` 只影响显式长度换算,与缓存键
/// 一起参与;失败返回 `None`,调用方回退源码。
pub(crate) fn layout_for(
    source: &str,
    display: bool,
    pixel_size: f32,
    pixels_per_point: f32,
) -> Option<Arc<MathLayout>> {
    let key = FormulaCacheKey::new(
        formula_id(source, display),
        pixel_size,
        pixels_per_point,
        display,
    );
    layouts()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get_or_insert_with(key, || {
            crate::math::compile_formula(
                source,
                display,
                pixel_size,
                pixels_per_point,
                crate::math::DEFAULT_LIMITS,
            )
        })
        .ok()
        .cloned()
        .map(Arc::new)
}

fn image_key(
    source: &str,
    display: bool,
    pixel_size: f32,
    pixels_per_point: f32,
    raster_scale: f32,
    color: gpui::Rgba,
) -> ImageKey {
    ImageKey {
        source: source.to_owned(),
        display,
        pixel_size_bits: pixel_size.to_bits(),
        pixels_per_point_bits: pixels_per_point.to_bits(),
        raster_scale_bits: raster_scale.to_bits(),
        color: [
            (color.r * 255.0).round() as u8,
            (color.g * 255.0).round() as u8,
            (color.b * 255.0).round() as u8,
        ],
    }
}

/// 位图预检:能合成返回 true 并顺手入缓存,paint 消费同一键。
pub(crate) fn can_compose(
    source: &str,
    display: bool,
    pixel_size: f32,
    pixels_per_point: f32,
    raster_scale: f32,
    color: gpui::Rgba,
    layout: &MathLayout,
) -> bool {
    let key = image_key(
        source,
        display,
        pixel_size,
        pixels_per_point,
        raster_scale,
        color,
    );
    images()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .image(key, layout, raster_scale, color_bytes(color))
        .is_some()
}

fn color_bytes(color: gpui::Rgba) -> [u8; 3] {
    [
        (color.r * 255.0).round() as u8,
        (color.g * 255.0).round() as u8,
        (color.b * 255.0).round() as u8,
    ]
}

/// 把一条公式的位图贴到指定基线(物理像素吸附)。文本兜底由调用方按自己的
/// 裁剪合同补画。
#[allow(clippy::too_many_arguments)]
pub(crate) fn paint_formula_image(
    source: &str,
    display: bool,
    pixel_size: f32,
    pixels_per_point: f32,
    layout: &MathLayout,
    left: Pixels,
    baseline: Pixels,
    color: gpui::Rgba,
    window: &mut Window,
) {
    let raster_scale = window.scale_factor();
    let key = image_key(
        source,
        display,
        pixel_size,
        pixels_per_point,
        raster_scale,
        color,
    );
    let cached = images()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .image(key, layout, raster_scale, color_bytes(color));
    let Some((image, geometry)) = cached else {
        return;
    };
    // 位图物理像素与屏幕 1:1:原点在物理网格上取整后再折回逻辑坐标。
    let left_physical = (f32::from(left) * raster_scale).round();
    let baseline_physical = (f32::from(baseline) * raster_scale).round();
    let origin = point(
        px((left_physical - geometry.pad as f32) / raster_scale),
        px((baseline_physical - geometry.baseline as f32) / raster_scale),
    );
    let image_size = size(
        px(geometry.width as f32 / raster_scale),
        px(geometry.height as f32 / raster_scale),
    );
    let _ = window.paint_image(
        Bounds::new(origin, image_size),
        Bounds::new(origin, image_size),
        Corners::default(),
        image,
        0,
        false,
    );
}

/// 数学字体缺字的字符(公式里的中文等):整字符落在裁剪窗内才画,避免
/// 半个汉字。坐标合同与 [`crate::terminal_math::plan_overlay_draw`] 一致。
///
/// 参数数量由绘制合同固定(布局/字号/原点/裁剪窗/单元格/颜色/窗口/
/// 文本系统上下文),逐一列出比临时结构体更直接。
#[allow(clippy::too_many_arguments)]
pub(crate) fn paint_text_ops(
    layout: &MathLayout,
    fitted_pixel_size: f32,
    origin_x: f32,
    baseline_y: f32,
    clip_left: f32,
    clip_top: f32,
    clip_right: f32,
    clip_bottom: f32,
    cell: (f32, f32),
    color: gpui::Rgba,
    window: &mut Window,
    cx: &mut gpui::App,
) {
    let text_style = window.text_style();
    for op in &layout.text {
        let scale = op.pixel_size / fitted_pixel_size.max(f32::EPSILON);
        let x = origin_x + op.x;
        let width = cell.0 * scale;
        let height = cell.1 * scale;
        let top = baseline_y + op.baseline_y - height * 0.8;
        if x < clip_left || x + width > clip_right || top < clip_top || top + height > clip_bottom {
            continue;
        }
        let mut buffer = [0u8; 4];
        let text: SharedString = op.character.encode_utf8(&mut buffer).to_string().into();
        let mut run = text_style.to_run(text.len());
        run.color = color.into();
        let line = window.text_system().shape_line(
            text,
            px(op.pixel_size),
            std::slice::from_ref(&run),
            None,
        );
        let target = point(px(x), px(baseline_y + op.baseline_y) - line.ascent);
        let _ = line.paint(
            target,
            line.ascent + line.descent,
            gpui::TextAlign::Left,
            None,
            window,
            cx,
        );
    }
}

#[cfg(test)]
mod math_render_tests {
    use super::*;
    use crate::math::DEFAULT_LIMITS;
    use crate::math::compile_formula;

    #[test]
    fn composable_formula_caches_one_immutable_bitmap() {
        let layout = compile_formula(r"x^2 + y^2", false, 16.0, 1.0, DEFAULT_LIMITS).unwrap();
        let color = gpui::Rgba {
            r: 0.9,
            g: 0.9,
            b: 0.9,
            a: 1.0,
        };
        assert!(can_compose(
            "x^2 + y^2",
            false,
            16.0,
            1.0,
            1.0,
            color,
            &layout
        ));
        let first = images()
            .lock()
            .unwrap()
            .entries
            .keys()
            .next()
            .cloned()
            .expect("bitmap cached");
        assert!(can_compose(
            "x^2 + y^2",
            false,
            16.0,
            1.0,
            1.0,
            color,
            &layout
        ));
        let second = images()
            .lock()
            .unwrap()
            .entries
            .keys()
            .next()
            .cloned()
            .expect("bitmap cached");
        assert_eq!(first, second, "同键必须复用同一位图");
    }

    #[test]
    fn invalid_scale_rejects_composition() {
        let layout = compile_formula("x", false, 16.0, 1.0, DEFAULT_LIMITS).unwrap();
        let color = gpui::Rgba::default();
        assert!(!can_compose(
            "x",
            false,
            16.0,
            1.0,
            f32::NAN,
            color,
            &layout
        ));
        assert!(!can_compose("x", false, 16.0, 1.0, 0.0, color, &layout));
    }
}
