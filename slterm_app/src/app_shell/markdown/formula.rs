//! 本地公式的不可变位图缓存与失败降级。

use std::collections::HashMap;
use std::sync::Arc;

use sha2::{Digest as _, Sha256};

use crate::math::bitmap::{MathBitmap, compose};
use crate::math::rasterizer::MathGlyphRasterizer;
use crate::math::{
    DEFAULT_LIMITS, MIN_READABLE_MATH_PX, MathError, MathErrorKind, compile_formula_source,
    pixels_per_point,
};

const FORMULA_CACHE_BYTES: usize = 24 * 1024 * 1024;

/// 设备像素位图 → 逻辑显示尺寸。
///
/// 位图按「窗口 scale × 阅读 zoom」光栅化，而 GPUI 的 `RenderImage` 固定以
/// 1.0 的 scale factor 呈现；呈现层必须把窗口 scale 除回来，否则高 DPI 下
/// 公式会按缩放因子被放大。阅读 zoom 已经体现在位图里，逻辑尺寸也要保留
/// 它，故只除窗口 scale。
pub(crate) fn logical_formula_size(
    pixel_width: f32,
    pixel_height: f32,
    scale_factor: f32,
) -> (f32, f32) {
    let scale = if scale_factor.is_finite() && scale_factor > 0.0 {
        scale_factor
    } else {
        1.0
    };
    (pixel_width / scale, pixel_height / scale)
}

#[derive(Clone, Debug)]
pub(crate) struct FormulaRequest {
    source: String,
    display: bool,
    font_size: f32,
    scale_factor: f32,
    zoom: f32,
    theme_revision: u64,
    color: [u8; 3],
}

impl FormulaRequest {
    pub(crate) fn new(
        source: &str,
        display: bool,
        font_size: f32,
        scale_factor: f32,
        zoom: f32,
        theme_revision: u64,
        color: [u8; 3],
    ) -> Self {
        Self {
            source: source.to_owned(),
            display,
            font_size,
            scale_factor,
            zoom,
            theme_revision,
            color,
        }
    }

    pub(crate) fn source(&self) -> &str {
        &self.source
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct FormulaBitmapKey {
    source_hash: [u8; 32],
    display: bool,
    font_size_bits: u32,
    pixels_per_point_bits: u32,
    raster_scale_bits: u32,
    theme_revision: u64,
    color: [u8; 3],
}

impl FormulaBitmapKey {
    fn new(request: &FormulaRequest) -> Self {
        Self {
            source_hash: Sha256::digest(request.source.as_bytes()).into(),
            display: request.display,
            font_size_bits: request.font_size.to_bits(),
            pixels_per_point_bits: pixels_per_point(request.scale_factor).to_bits(),
            raster_scale_bits: (request.scale_factor * request.zoom).to_bits(),
            theme_revision: request.theme_revision,
            color: request.color,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum FormulaFailureKind {
    Compile(MathErrorKind),
    Unreadable,
    Bitmap,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FormulaFailure {
    source: String,
    kind: FormulaFailureKind,
}

impl FormulaFailure {
    pub(crate) fn source(&self) -> &str {
        &self.source
    }

    pub(crate) fn kind(&self) -> &FormulaFailureKind {
        &self.kind
    }
}

pub(crate) struct FormulaBitmapCache {
    rasterizer: MathGlyphRasterizer,
    entries: HashMap<FormulaBitmapKey, FormulaCacheEntry>,
    budget: usize,
    used: usize,
    clock: u64,
}

struct FormulaCacheEntry {
    bitmap: Arc<MathBitmap>,
    touched: u64,
}

impl FormulaBitmapCache {
    pub(crate) fn new() -> Result<Self, MathError> {
        Self::with_budget(FORMULA_CACHE_BYTES)
    }

    fn with_budget(budget: usize) -> Result<Self, MathError> {
        Ok(Self {
            rasterizer: MathGlyphRasterizer::new()?,
            entries: HashMap::new(),
            budget,
            used: 0,
            clock: 0,
        })
    }

    /// 只在后台调用；返回值是不可变位图，失败保留源码供块级降级。
    pub(crate) fn render(
        &mut self,
        request: &FormulaRequest,
    ) -> Result<Arc<MathBitmap>, FormulaFailure> {
        let key = FormulaBitmapKey::new(request);
        if let Some(entry) = self.entries.get_mut(&key) {
            self.clock = self.clock.saturating_add(1);
            entry.touched = self.clock;
            return Ok(entry.bitmap.clone());
        }
        let failure = |kind| FormulaFailure {
            source: request.source.clone(),
            kind,
        };
        if request.font_size < MIN_READABLE_MATH_PX {
            return Err(failure(FormulaFailureKind::Unreadable));
        }
        let ppp = pixels_per_point(request.scale_factor);
        let layout = compile_formula_source(
            &request.source,
            request.display,
            request.font_size,
            ppp,
            DEFAULT_LIMITS,
        )
        .map_err(|error| failure(FormulaFailureKind::Compile(error.kind)))?;
        let raster_scale = request.scale_factor * request.zoom;
        let bitmap = compose(&self.rasterizer, &layout, raster_scale, request.color)
            .ok_or_else(|| failure(FormulaFailureKind::Bitmap))?;
        let bytes = bitmap.pixels.len();
        if bytes > self.budget {
            return Err(failure(FormulaFailureKind::Bitmap));
        }
        let bitmap = Arc::new(bitmap);
        while self.used.saturating_add(bytes) > self.budget {
            let oldest = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.touched)
                .map(|(key, _)| key.clone());
            let Some(oldest) = oldest else { break };
            self.remove(&oldest);
        }
        self.clock = self.clock.saturating_add(1);
        self.entries.insert(
            key,
            FormulaCacheEntry {
                bitmap: bitmap.clone(),
                touched: self.clock,
            },
        );
        self.used += bytes;
        Ok(bitmap)
    }

    fn remove(&mut self, key: &FormulaBitmapKey) {
        if let Some(entry) = self.entries.remove(key) {
            self.used -= entry.bitmap.pixels.len();
        }
    }

    #[cfg(test)]
    fn entries(&self) -> usize {
        self.entries.len()
    }

    #[cfg(test)]
    fn used_bytes(&self) -> usize {
        self.used
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::MathErrorKind;

    fn request() -> FormulaRequest {
        FormulaRequest::new("x+1", false, 18.0, 1.0, 1.0, 0, [10, 20, 30])
    }

    #[test]
    fn formula_logical_size_undoes_window_scale_but_keeps_reading_zoom() {
        // raster = 窗口 1.5 × zoom 2.0：300×150 设备像素对应 200×100 逻辑像素。
        assert_eq!(logical_formula_size(300.0, 150.0, 1.5), (200.0, 100.0));
        assert_eq!(logical_formula_size(40.0, 20.0, 1.0), (40.0, 20.0));
        // 非法 scale 不得产生 NaN/Inf 或零尺寸。
        assert_eq!(logical_formula_size(8.0, 4.0, 0.0), (8.0, 4.0));
        assert_eq!(logical_formula_size(8.0, 4.0, f32::NAN), (8.0, 4.0));
        assert_eq!(logical_formula_size(8.0, 4.0, f32::INFINITY), (8.0, 4.0));
    }

    #[test]
    fn formula_bitmap_cache_keys_cover_source_layout_theme_and_zoom() {
        let mut cache = FormulaBitmapCache::new().unwrap();
        let base = request();
        let first = cache.render(&base).unwrap();
        let again = cache.render(&base).unwrap();
        assert!(Arc::ptr_eq(&first, &again));
        assert_eq!(cache.entries(), 1);
        assert!(first.pixels.chunks_exact(4).any(|pixel| pixel[3] != 0));
        assert!(
            first
                .pixels
                .chunks_exact(4)
                .all(|pixel| { pixel[..3] == [30, 20, 10] })
        );

        for changed in [
            FormulaRequest::new("x+2", false, 18.0, 1.0, 1.0, 0, [10, 20, 30]),
            FormulaRequest::new("x+1", true, 18.0, 1.0, 1.0, 0, [10, 20, 30]),
            FormulaRequest::new("x+1", false, 20.0, 1.0, 1.0, 0, [10, 20, 30]),
            FormulaRequest::new("x+1", false, 18.0, 2.0, 1.0, 0, [10, 20, 30]),
            FormulaRequest::new("x+1", false, 18.0, 1.0, 1.1, 0, [10, 20, 30]),
            FormulaRequest::new("x+1", false, 18.0, 1.0, 1.0, 1, [10, 20, 30]),
            FormulaRequest::new("x+1", false, 18.0, 1.0, 1.0, 0, [11, 20, 30]),
        ] {
            let rendered = cache.render(&changed).unwrap();
            assert!(!Arc::ptr_eq(&first, &rendered));
        }
        assert_eq!(cache.entries(), 8);
    }

    #[test]
    fn formula_failures_preserve_source_and_do_not_poison_other_formulas() {
        let mut cache = FormulaBitmapCache::new().unwrap();
        let invalid =
            FormulaRequest::new(r"\def\bad{value}", false, 18.0, 1.0, 1.0, 0, [10, 20, 30]);
        let failure = cache.render(&invalid).err().unwrap();
        assert_eq!(failure.source(), invalid.source());
        assert_eq!(
            failure.kind(),
            &FormulaFailureKind::Compile(MathErrorKind::ForbiddenCommand)
        );
        assert_eq!(cache.entries(), 0);

        let unreadable = FormulaRequest::new("x", false, 5.0, 1.0, 1.0, 0, [0, 0, 0]);
        assert_eq!(
            cache.render(&unreadable).err().unwrap().kind(),
            &FormulaFailureKind::Unreadable
        );
        let oversized = FormulaRequest::new("x", false, 18.0, 1.0, 10_000.0, 0, [0, 0, 0]);
        assert_eq!(
            cache.render(&oversized).err().unwrap().kind(),
            &FormulaFailureKind::Bitmap
        );
        assert!(cache.render(&request()).is_ok());
    }

    #[test]
    fn formula_bitmap_lru_stays_within_its_byte_budget() {
        let mut cache = FormulaBitmapCache::new().unwrap();
        let first = cache.render(&request()).unwrap();
        let first_bytes = first.pixels.len();
        let mut constrained = FormulaBitmapCache::with_budget(first_bytes).unwrap();
        constrained.render(&request()).unwrap();
        constrained
            .render(&FormulaRequest::new(
                "x+2",
                false,
                18.0,
                1.0,
                1.0,
                0,
                [10, 20, 30],
            ))
            .unwrap();
        assert_eq!(constrained.entries(), 1);
        assert_eq!(constrained.used_bytes(), first_bytes);
    }
}
