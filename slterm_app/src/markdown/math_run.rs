//! markdown 阅读器的数学适配(08 篇管线 B,`MathRun`/`measure_math`/
//! `fit_math_run`)。
//!
//! 引擎/缓存/字体全部来自 `crate::math`;本模块只决定一条公式在阅读列里
//! 占多宽、缩到多小,绝不独立规范化/解析/排版。公式参与行宽测量;过宽
//! 按线性比例缩字号,低于 [`MIN_READABLE_MATH_PX`] 即放弃并回退源码。

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use crate::math::cache::{FormulaCacheKey, MathLayoutCache};
use crate::math::layout::{MathLayout, MathMetrics};
use crate::math::{DEFAULT_LIMITS, MIN_READABLE_MATH_PX, compile_formula};

use super::{MathMode, MathSource};

/// 与终端覆盖层同一余量:布局对字号线性,2% 吸收取整误差,保证最右侧
/// 抗锯齿像素不越出阅读列。
const FIT_MARGIN: f32 = 0.98;

/// 一条公式的布局结果与当前名义字号。
#[derive(Clone, Debug)]
pub(crate) struct MathRun {
    pub source: MathSource,
    /// `None` = 编译失败或已缩到不可读,调用方回退源码文本。
    pub layout: Option<Arc<MathLayout>>,
    /// 当前名义字号(光学补偿前)。缓存键/fit/draw 全持名义字号。
    pub nominal_font_size: f32,
}

impl MathRun {
    /// 公式几何;不可读/失败时 `None`。
    pub(crate) fn metrics(&self) -> Option<MathMetrics> {
        self.layout.as_ref().map(|layout| layout.metrics)
    }

    /// 是否应回退到源码文本。
    pub(crate) fn is_fallback(&self) -> bool {
        self.layout.is_none()
    }
}

fn formula_id(source: &str, mode: MathMode) -> u64 {
    let mut hasher = DefaultHasher::new();
    mode.hash(&mut hasher);
    source.hash(&mut hasher);
    hasher.finish()
}

fn cache_key(
    source: &str,
    mode: MathMode,
    nominal_font_size: f32,
    pixels_per_point: f32,
) -> FormulaCacheKey {
    FormulaCacheKey::new(
        formula_id(source, mode),
        nominal_font_size,
        pixels_per_point,
        matches!(mode, MathMode::Display),
    )
}

/// 编译/取缓存一条公式,产出参与行宽测量的 [`MathRun`]。
pub(crate) fn measure_math(
    cache: &mut MathLayoutCache,
    source: &MathSource,
    base_font_size: f32,
    pixels_per_point: f32,
) -> Option<MathRun> {
    if !base_font_size.is_finite() || base_font_size <= 0.0 {
        return None;
    }
    let key = cache_key(
        source.as_str(),
        source.mode,
        base_font_size,
        pixels_per_point,
    );
    let layout = cache
        .get_or_insert_with(key, || {
            compile_formula(
                source.as_str(),
                matches!(source.mode, MathMode::Display),
                base_font_size,
                pixels_per_point,
                DEFAULT_LIMITS,
            )
        })
        .ok()?
        .clone();
    Some(MathRun {
        source: source.clone(),
        layout: Some(Arc::new(layout)),
        nominal_font_size: base_font_size,
    })
}

/// 过宽公式按阅读列宽缩放;不可读时把 `layout` 清空(调用方回退源码)。
/// 返回是否仍可渲染。
pub(crate) fn fit_math_run(
    cache: &mut MathLayoutCache,
    run: &mut MathRun,
    max_width: f32,
    pixels_per_point: f32,
) -> bool {
    if !max_width.is_finite() || max_width <= 0.0 {
        run.layout = None;
        return false;
    }
    let Some(metrics) = run.metrics() else {
        return false;
    };
    if metrics.width <= max_width {
        return true;
    }
    let fitted = run.nominal_font_size * (max_width / metrics.width.max(1.0)) * FIT_MARGIN;
    if !fitted.is_finite() || fitted < MIN_READABLE_MATH_PX {
        run.layout = None;
        return false;
    }
    let key = cache_key(
        run.source.as_str(),
        run.source.mode,
        fitted,
        pixels_per_point,
    );
    let layout = cache
        .get_or_insert_with(key, || {
            compile_formula(
                run.source.as_str(),
                matches!(run.source.mode, MathMode::Display),
                fitted,
                pixels_per_point,
                DEFAULT_LIMITS,
            )
        })
        .ok()
        .cloned();
    match layout {
        Some(layout) => {
            run.nominal_font_size = fitted;
            run.layout = Some(Arc::new(layout));
            true
        }
        None => {
            run.layout = None;
            false
        }
    }
}

#[cfg(test)]
mod math_run_tests {
    use super::*;
    use crate::markdown::{MathSource, TextRef};

    fn source(text: &str, mode: MathMode) -> MathSource {
        MathSource {
            source: TextRef::generated(text.to_owned().into_boxed_str()),
            mode,
        }
    }

    #[test]
    fn inline_and_display_math_measure_into_layouts() {
        let mut cache = MathLayoutCache::default();
        for mode in [MathMode::Inline, MathMode::Display] {
            let run = measure_math(&mut cache, &source(r"x^2 + \frac{1}{2}", mode), 16.0, 1.0)
                .expect("valid formula measures");
            assert!(!run.is_fallback());
            assert!(run.metrics().unwrap().width > 0.0);
            assert_eq!(run.nominal_font_size, 16.0);
        }
    }

    #[test]
    fn invalid_source_falls_back_without_panicking() {
        let mut cache = MathLayoutCache::default();
        let run = measure_math(
            &mut cache,
            &source(r"\frac{1}{", MathMode::Inline),
            16.0,
            1.0,
        );
        assert!(run.is_none());
        let run = measure_math(&mut cache, &source("x", MathMode::Inline), 0.0, 1.0);
        assert!(run.is_none());
    }

    #[test]
    fn fitting_shrinks_to_the_reading_column_and_refuses_below_the_floor() {
        let mut cache = MathLayoutCache::default();
        let mut run = measure_math(
            &mut cache,
            &source(r"\frac{a+b+c+d+e}{f+g+h+i+j}", MathMode::Display),
            16.0,
            1.0,
        )
        .unwrap();
        let natural = run.metrics().unwrap().width;
        assert!(fit_math_run(&mut cache, &mut run, natural, 1.0));
        assert_eq!(run.nominal_font_size, 16.0, "放得下不得缩放");

        assert!(fit_math_run(&mut cache, &mut run, natural / 2.0, 1.0));
        assert!(run.nominal_font_size < 16.0);
        assert!(run.metrics().unwrap().width <= natural / 2.0 + 0.5);

        // 比可读底线还窄:回退源码,不发明第三种排版。
        assert!(!fit_math_run(&mut cache, &mut run, 1.0, 1.0));
        assert!(run.is_fallback());
    }

    #[test]
    fn same_source_shares_one_cache_entry_across_measures() {
        let mut cache = MathLayoutCache::default();
        let a = measure_math(&mut cache, &source("x", MathMode::Inline), 16.0, 1.0).unwrap();
        let b = measure_math(&mut cache, &source("x", MathMode::Inline), 16.0, 1.0).unwrap();
        assert_eq!(
            a.metrics().unwrap(),
            b.metrics().unwrap(),
            "同一源码/字号必须命中同一布局"
        );
        // inline 与 display 是不同缓存键。
        let display = measure_math(&mut cache, &source("x", MathMode::Display), 16.0, 1.0).unwrap();
        assert!(display.metrics().is_some());
    }
}
