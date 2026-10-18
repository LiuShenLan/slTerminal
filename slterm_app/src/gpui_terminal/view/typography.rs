//! 终端字体构造与单元格度量。
//!
//! 终端视图与其元素必须共用同一条设备像素取整规则;把 shaping、宽、高、
//! 行高策略收口在此,spawn 网格才不会漂离首帧 prepaint(启动零 resize 锚)。
//!
//! M3 常量锚(设置键回填点):`CellWidthModeName` 恒 Compact(函数签名
//! 保 mode 参,调用点传死)、`font_offset` 恒 (0,0)、行高倍率恒 None——
//! 上游这三项来自设置文件,本仓 M3 Settings 白名单未收,取常量
//! 保「所有度量路径同值」的合同。

use gpui::{
    App, Font, FontFeatures, FontStyle, FontWeight, Hsla, Pixels, SharedString, TextRun, Window,
    WindowTextSystem, px,
};
use slterm_settings::CellWidthModeName;

use crate::app_shell::assets::REQUIRED_FONT_FAMILY;
use crate::app_shell::config::Settings;

/// M3 传死的字宽模式(换设置键真读)。
pub(super) const M3_CELL_WIDTH_MODE: CellWidthModeName = CellWidthModeName::Compact;
/// M3 传死的字体偏移(逻辑像素;上游 Windows 默认 (0,4) 随设置键回填)。
pub(super) const M3_FONT_OFFSET: (f32, f32) = (0.0, 0.0);

/// 逗号分隔字族链解析:去空白、去空项、保序去重。
fn font_family_chain(value: &str) -> Vec<String> {
    let mut families: Vec<String> = Vec::new();
    for family in value.split(',') {
        let family = family.trim();
        if family.is_empty() {
            continue;
        }
        if !families.iter().any(|existing| existing == family) {
            families.push(family.to_owned());
        }
    }
    families
}

/// 构造带兜底链的 GPUI 字体:用户首选字族在前,内嵌 Maple 恒兜底在列
/// (NF 图标码点只有内嵌字形同源才跨机器可比对)。
pub(super) fn gpui_font_with_fallbacks(value: &str) -> Font {
    let mut families = font_family_chain(value);
    if families.is_empty() {
        families.push(REQUIRED_FONT_FAMILY.to_owned());
    }
    let primary = families.remove(0);
    if !primary.eq_ignore_ascii_case(REQUIRED_FONT_FAMILY)
        && !families
            .iter()
            .any(|family| family.eq_ignore_ascii_case(REQUIRED_FONT_FAMILY))
    {
        families.push(REQUIRED_FONT_FAMILY.to_owned());
    }
    let mut font = gpui::font(primary);
    if !families.is_empty() {
        font.fallbacks = Some(gpui::FontFallbacks::from_fonts(families));
    }
    font
}

/// 显式写全每个连字特性位,含关闭态:平台缺省各异,Windows 后端会忽略
/// 空特性列表。
pub(super) fn mono_font(
    family: &str,
    weight: FontWeight,
    style: FontStyle,
    ligatures: bool,
) -> Font {
    let enabled = u32::from(ligatures);
    Font {
        weight,
        style,
        features: FontFeatures(std::sync::Arc::new(vec![
            ("calt".to_owned(), enabled),
            ("liga".to_owned(), enabled),
            ("clig".to_owned(), enabled),
            ("kern".to_owned(), 0),
        ])),
        ..gpui_font_with_fallbacks(family)
    }
}

/// 对一个 shaped 单元宽应用设备像素取整合同。
pub(super) fn effective_cell_width(
    raw_width: f32,
    mode: CellWidthModeName,
    scale: f32,
    offset_x: f32,
) -> Pixels {
    let scale = scale.max(0.5);
    let device_width = raw_width * scale + offset_x;
    let device_width = match mode {
        CellWidthModeName::Compact => device_width.floor(),
        CellWidthModeName::Relaxed => device_width.round(),
    };
    px(device_width.max(1.0) / scale)
}

/// 保留 shaped 字体度量,除非主题显式给了行高。偏移一律在设备像素域
/// 施加并下取整。
pub(super) fn effective_line_height(natural_height: f32, offset_y: f32, scale: f32) -> Pixels {
    let scale = scale.max(0.5);
    px(((natural_height * scale + offset_y).floor().max(1.0)) / scale)
}

pub(super) fn effective_line_height_with_theme(
    natural_height: f32,
    font_size: f32,
    line_height_multiplier: Option<f32>,
    offset_y: f32,
    scale: f32,
) -> Pixels {
    let target = line_height_multiplier
        .map(|multiplier| (font_size * multiplier).max(1.0))
        .unwrap_or(natural_height);
    effective_line_height(target, offset_y, scale)
}

pub(super) fn line_height_for_view(
    font_size: Pixels,
    line_height_multiplier: Option<f32>,
    natural_height: f32,
    offset_y: f32,
    scale: f32,
) -> Pixels {
    effective_line_height_with_theme(
        natural_height,
        font_size.as_f32(),
        line_height_multiplier,
        offset_y,
        scale,
    )
}

#[allow(clippy::too_many_arguments)] // 照抄上游签名:度量参数即契约,不折叠。
fn measure_cell_metrics(
    text_system: &WindowTextSystem,
    scale: f32,
    family: &str,
    font_size: Pixels,
    mode: CellWidthModeName,
    offset_x: f32,
    offset_y: f32,
    line_height_multiplier: Option<f32>,
) -> (Pixels, Pixels) {
    let font = mono_font(family, FontWeight::NORMAL, FontStyle::Normal, false);
    let sample = text_system.shape_line(
        SharedString::new_static("M"),
        font_size,
        &[TextRun {
            len: 1,
            font,
            color: Hsla::default(),
            background_color: None,
            underline: None,
            strikethrough: None,
        }],
        None,
    );
    (
        effective_cell_width(sample.width.as_f32(), mode, scale, offset_x),
        effective_line_height_with_theme(
            sample.ascent.as_f32() + sample.descent.as_f32(),
            font_size.as_f32(),
            line_height_multiplier,
            offset_y,
            scale,
        ),
    )
}

/// 读取 (字族, 字号) —— M3 Settings 白名单内的两项;偏移/模式/倍率取
/// 模块常量(见文件头 M3 常量锚)。
fn m3_metric_inputs(cx: &App) -> (String, f32) {
    match cx.try_global::<Settings>() {
        Some(settings) => (settings.font_family.clone(), settings.font_size_px),
        None => (REQUIRED_FONT_FAMILY.to_owned(), 15.0),
    }
}

pub(super) fn cell_metrics(window: &Window, cx: &App) -> (Pixels, Pixels) {
    let (family, font_size) = m3_metric_inputs(cx);
    measure_cell_metrics(
        window.text_system(),
        window.scale_factor(),
        &family,
        px(font_size),
        M3_CELL_WIDTH_MODE,
        M3_FONT_OFFSET.0,
        M3_FONT_OFFSET.1,
        None,
    )
}

/// 启动尺寸用基础字号;持久化的终端缩放只在窗口建成后影响活网格
/// (缩放键域归 06)。
pub(super) fn startup_cell_metrics(window: &Window, cx: &App) -> (Pixels, Pixels) {
    measure_startup_cell_metrics(window.text_system(), window.scale_factor(), cx)
}

/// 同一套字体系统与取整规则可以在 HWND 存在前先量出窗口尺寸。
pub(super) fn startup_cell_metrics_at_scale(scale: f32, cx: &App) -> (Pixels, Pixels) {
    let text_system = WindowTextSystem::new(cx.text_system().clone());
    measure_startup_cell_metrics(&text_system, scale, cx)
}

fn measure_startup_cell_metrics(
    text_system: &WindowTextSystem,
    scale: f32,
    cx: &App,
) -> (Pixels, Pixels) {
    let (family, font_size) = m3_metric_inputs(cx);
    measure_cell_metrics(
        text_system,
        scale,
        &family,
        px(font_size),
        M3_CELL_WIDTH_MODE,
        M3_FONT_OFFSET.0,
        M3_FONT_OFFSET.1,
        None,
    )
}
