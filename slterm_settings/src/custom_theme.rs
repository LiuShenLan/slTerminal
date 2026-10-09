//! 用户自建主题的值模型。
//!
//! 本模块刻意不含任何文件格式与序列化代码:导入/导出在应用边界把文件
//! 翻译成 [`ThemeDefinition`],settings crate 保持小而零依赖的值权威。
//! 仅暗色:外观枚举与亮暗双权重混合不迁,派生规则为暗色单权重。

use core::fmt;

use crate::{
    BlurMode, CursorShapeName, ExactTermColors, MAX_PANE_CARD_DIVIDER, MAX_PANE_CARD_GUTTER,
    MAX_PANE_CARD_RADIUS, Rgb8, Rgba8, ThemeCardGeometry, ThemeName, format_hex_rgb, parse_hex_rgb,
};

/// 完整用户主题。`base` 记录起点内置主题快照的元数据,不建运行时继承链;
/// 克隆定义自含全部字符串与数组。
#[derive(Clone, Debug, PartialEq)]
pub struct ThemeDefinition {
    pub name: String,
    pub base: ThemeName,
    pub terminal: TerminalThemeColors,
    pub ui: ThemeUiColors,
    pub typography: ThemeTypography,
    pub layout: ThemeLayout,
    pub effects: ThemeEffects,
}

impl ThemeDefinition {
    /// 把内置主题解析为可编辑快照。快照刻意具体:日后改内置主题不会
    /// 变异已导入/已编辑的自定义主题。
    pub fn from_builtin(base: ThemeName) -> Self {
        let term = base.term_theme();
        let palette = base.reviewed_palette();
        // 主题只声明背景而未带完整色表时的暗色兜底:从语义槽合成。
        let exact = term.exact.unwrap_or_else(|| ExactTermColors {
            foreground: palette.foreground,
            ansi: fallback_ansi(palette),
            cursor: Some(palette.accent),
            cursor_text: Some(term.background),
            cursor_stroke: Some(palette.accent),
            selection_foreground: Some(palette.foreground),
            selection_background: Some(palette.shell),
        });

        let indexed = IndexedPalette::from_ansi(exact.ansi);

        Self {
            name: base.prompt_name().to_owned(),
            base,
            terminal: TerminalThemeColors {
                background: term.background,
                foreground: exact.foreground,
                cursor: exact.cursor,
                cursor_text: exact.cursor_text,
                cursor_stroke: exact.cursor_stroke,
                selection_foreground: exact.selection_foreground,
                selection_background: exact.selection_background,
                palette: indexed,
            },
            ui: ThemeUiColors::from_palette(palette),
            typography: ThemeTypography::default(),
            layout: ThemeLayout {
                card: base.card_geometry(),
                padding_x: None,
                padding_y: None,
            },
            effects: ThemeEffects::default(),
        }
    }

    /// 适配器别名:把内置值称作快照。
    pub fn snapshot(base: ThemeName) -> Self {
        Self::from_builtin(base)
    }

    /// 应用或导出前校验全部用户可编辑标量。
    pub fn validate(&self) -> Result<(), ThemeValidationError> {
        let name = self.name.trim();
        if name.is_empty() {
            return Err(ThemeValidationError::EmptyName);
        }
        if name.len() > MAX_THEME_NAME_BYTES {
            return Err(ThemeValidationError::NameTooLong);
        }
        if self.name.chars().any(char::is_control) {
            return Err(ThemeValidationError::NameContainsControlCharacter);
        }
        self.typography.validate()?;
        self.layout.validate()?;
        self.effects.validate()?;
        Ok(())
    }

    /// 从解析后的终端值派生 UI 锚点(仅暗色单权重)。这是 `ui.derive`
    /// 的唯一共享规则:适配器必须调用它,不得各自实现背景混合或 ANSI
    /// 兜底。
    pub fn derived_ui(&self) -> ThemeUiColors {
        ThemeUiColors::from_terminal(&self.terminal)
    }

    /// 按主题 `ui.derive` 标志返回显式 UI 值或权威派生值。
    pub fn resolved_ui(&self) -> ThemeUiColors {
        if self.ui.derive {
            self.derived_ui()
        } else {
            self.ui
        }
    }

    /// 返回副本:终端默认文字色改写。
    pub fn with_terminal_foreground(&self, foreground: Rgb8) -> Self {
        let mut theme = self.clone();
        theme.terminal.foreground = foreground;
        theme
    }

    /// 返回副本:光标色改写。`None` = 主题不声明光标覆盖,保留宿主行为。
    pub fn with_cursor(
        &self,
        cursor: Option<Rgb8>,
        cursor_text: Option<Rgb8>,
        cursor_stroke: Option<Rgb8>,
    ) -> Self {
        let mut theme = self.clone();
        theme.terminal.cursor = cursor;
        theme.terminal.cursor_text = cursor_text;
        theme.terminal.cursor_stroke = cursor_stroke;
        theme
    }

    /// 返回副本:选区色改写。
    pub fn with_selection(&self, foreground: Option<Rgb8>, background: Option<Rgb8>) -> Self {
        let mut theme = self.clone();
        theme.terminal.selection_foreground = foreground;
        theme.terminal.selection_background = background;
        theme
    }

    /// 返回副本:改写一个 ANSI 槽位。ANSI 槽位与前 16 个索引色保持镜像,
    /// 导出器不可能对同一终端色产出两个不同值。
    pub fn with_ansi(&self, index: usize, color: Rgb8) -> Option<Self> {
        if index >= 16 {
            return None;
        }
        let mut theme = self.clone();
        theme.terminal.palette.set(index, color);
        Some(theme)
    }

    /// 返回副本:改写一个索引色。0..15 同样合法并按 ANSI 编辑处理。
    pub fn with_indexed(&self, index: usize, color: Rgb8) -> Option<Self> {
        if index >= 256 {
            return None;
        }
        let mut theme = self.clone();
        theme.terminal.palette.set(index, color);
        Some(theme)
    }

    /// 与设置文件同源的颜色文本解析。
    pub fn parse_color(value: &str) -> Option<Rgb8> {
        parse_hex_rgb(value)
    }

    /// 文本/JSON 适配器共用的颜色格式化。
    pub fn format_color(color: Rgb8) -> String {
        format_hex_rgb(color)
    }
}

impl Default for ThemeDefinition {
    fn default() -> Self {
        Self::from_builtin(ThemeName::default())
    }
}

/// 独立于 UI chrome 的终端色。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TerminalThemeColors {
    pub background: Rgb8,
    pub foreground: Rgb8,
    pub cursor: Option<Rgb8>,
    pub cursor_text: Option<Rgb8>,
    pub cursor_stroke: Option<Rgb8>,
    pub selection_foreground: Option<Rgb8>,
    pub selection_background: Option<Rgb8>,
    /// ANSI 0..15 与索引色 16..255 共一张定长表。
    pub palette: IndexedPalette,
}

impl TerminalThemeColors {
    pub fn ansi(&self, index: usize) -> Option<Rgb8> {
        self.palette.get(index)
    }

    pub fn indexed(&self, index: usize) -> Option<Rgb8> {
        self.palette.get(index)
    }
}

/// 完整 xterm 256 色表:前 16 = ANSI 镜像;16..231 色立方 / 232..255
/// 灰阶按 xterm 常量生成,测试逐字节锁定。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndexedPalette {
    pub colors: [Rgb8; 256],
}

impl IndexedPalette {
    pub fn from_ansi(ansi: [Rgb8; 16]) -> Self {
        let mut colors = [[0; 3]; 256];
        colors[..16].copy_from_slice(&ansi);

        const LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];
        for (index, slot) in colors.iter_mut().enumerate().take(232).skip(16) {
            let cube = index - 16;
            *slot = [LEVELS[cube / 36], LEVELS[(cube / 6) % 6], LEVELS[cube % 6]];
        }
        for (index, slot) in colors.iter_mut().enumerate().skip(232) {
            let level = 8 + ((index - 232) * 10) as u8;
            *slot = [level; 3];
        }
        Self { colors }
    }

    pub fn get(&self, index: usize) -> Option<Rgb8> {
        self.colors.get(index).copied()
    }

    pub fn set(&mut self, index: usize, color: Rgb8) -> bool {
        let Some(slot) = self.colors.get_mut(index) else {
            return false;
        };
        *slot = color;
        true
    }

    pub fn ansi_colors(&self) -> [Rgb8; 16] {
        let mut ansi = [[0; 3]; 16];
        ansi.copy_from_slice(&self.colors[..16]);
        ansi
    }

    /// 整表已含全部索引色。
    pub fn indexed_colors(&self) -> &[Rgb8; 256] {
        &self.colors
    }
}

/// UI chrome 语义色。槽位刻意保持小集合;适配器可从稳定锚点派生
/// disabled/hover。终端前景背景逐字不动——唯一共享规则(from_terminal)
/// 不许多适配器各自实现背景混合。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ThemeUiColors {
    /// 镜像文档的 `ui.derive` 标志。
    pub derive: bool,
    pub shell: Rgb8,
    pub background: Rgb8,
    pub foreground: Rgb8,
    pub muted: Rgb8,
    pub accent: Rgb8,
    pub selection: Rgba8,
    pub line: Rgba8,
    pub frame: Rgb8,
    pub error: Rgb8,
    pub success: Rgb8,
    pub warning: Rgb8,
    pub info: Rgb8,
}

impl ThemeUiColors {
    /// 从评审过的内置调色板建 UI 锚点。
    pub fn from_palette(palette: crate::ReviewedPalette) -> Self {
        Self {
            derive: false,
            shell: palette.shell,
            background: palette.background,
            foreground: palette.foreground,
            muted: palette.muted,
            accent: palette.accent,
            selection: palette.selected,
            line: palette.line,
            frame: palette.frame,
            error: palette.red,
            success: palette.green,
            warning: palette.yellow,
            info: palette.blue,
        }
    }

    /// 从完整终端主题建 UI 锚点。终端前景/背景逐字不动;仅暗色:选区
    /// 无显式声明时按暗色单权重 0.18 混合,shell 向白微抬。
    pub fn from_terminal(terminal: &TerminalThemeColors) -> Self {
        let background = terminal.background;
        let foreground = terminal.foreground;
        let muted = terminal.palette.colors[8];
        let accent = terminal
            .cursor
            .or(terminal.cursor_stroke)
            .unwrap_or(terminal.palette.colors[14]);
        let selection = terminal.selection_background.map_or_else(
            || {
                let rgb = mix_rgb(background, foreground, 0.18);
                [rgb[0], rgb[1], rgb[2], 255]
            },
            |rgb| [rgb[0], rgb[1], rgb[2], 255],
        );
        let shell = mix_rgb(background, [255, 255, 255], 0.06);
        Self {
            derive: true,
            shell,
            background,
            foreground,
            muted,
            accent,
            selection,
            line: [muted[0], muted[1], muted[2], 255],
            frame: muted,
            error: terminal.palette.colors[9],
            success: terminal.palette.colors[10],
            warning: terminal.palette.colors[11],
            info: terminal.palette.colors[14],
        }
    }

    pub fn with_derive(mut self, derive: bool) -> Self {
        self.derive = derive;
        self
    }

    /// RGB 分量,供自行施加 alpha 的后端使用。
    pub const fn selection_rgb(self) -> Rgb8 {
        [self.selection[0], self.selection[1], self.selection[2]]
    }

    pub const fn line_rgb(self) -> Rgb8 {
        [self.line[0], self.line[1], self.line[2]]
    }
}

/// 可选排版覆盖;`None` 保留用户全局设置(「显式值权威」同构)。
#[derive(Clone, Debug, PartialEq)]
pub struct ThemeTypography {
    pub font_family: Option<String>,
    pub font_size: Option<f32>,
    pub line_height: Option<f32>,
    pub letter_spacing: Option<f32>,
    pub font_weight: Option<u16>,
    /// 终端文本塑形是否可用连字。内置主题保持既有开启默认;导入文档
    /// 可显式关闭且不在 JSON 边界丢失该选择。
    pub ligatures: bool,
    /// 可选 UI/chrome 字体覆盖;值模型保留它使导入导出往返无损。
    pub ui_font_family: Option<String>,
    pub ui_font_size: Option<f32>,
}

impl Default for ThemeTypography {
    fn default() -> Self {
        Self {
            font_family: None,
            font_size: None,
            line_height: None,
            letter_spacing: None,
            font_weight: None,
            ligatures: true,
            ui_font_family: None,
            ui_font_size: None,
        }
    }
}

impl ThemeTypography {
    fn validate(&self) -> Result<(), ThemeValidationError> {
        if self.font_family.as_deref().is_some_and(|family| {
            family.trim().is_empty()
                || family.len() > MAX_FONT_FAMILY_BYTES
                || family.chars().any(char::is_control)
        }) {
            return Err(ThemeValidationError::InvalidFontFamily);
        }
        if self.ui_font_family.as_deref().is_some_and(|family| {
            family.trim().is_empty()
                || family.len() > MAX_FONT_FAMILY_BYTES
                || family.chars().any(char::is_control)
        }) {
            return Err(ThemeValidationError::InvalidUiFontFamily);
        }
        if self
            .font_size
            .is_some_and(|size| !size.is_finite() || !(4.0..=96.0).contains(&size))
        {
            return Err(ThemeValidationError::InvalidFontSize);
        }
        if self.line_height.is_some_and(|line_height| {
            !line_height.is_finite() || !(0.5..=3.0).contains(&line_height)
        }) {
            return Err(ThemeValidationError::InvalidLineHeight);
        }
        if self
            .letter_spacing
            .is_some_and(|spacing| !spacing.is_finite() || !(0.0..=8.0).contains(&spacing))
        {
            return Err(ThemeValidationError::InvalidLetterSpacing);
        }
        if self
            .font_weight
            .is_some_and(|weight| !(100..=900).contains(&weight))
        {
            return Err(ThemeValidationError::InvalidFontWeight);
        }
        if self
            .ui_font_size
            .is_some_and(|size| !size.is_finite() || !(10.0..=20.0).contains(&size))
        {
            return Err(ThemeValidationError::InvalidUiFontSize);
        }
        Ok(())
    }
}

/// 属于主题而非用户窗口尺寸的布局值。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ThemeLayout {
    pub card: ThemeCardGeometry,
    pub padding_x: Option<f32>,
    pub padding_y: Option<f32>,
}

impl ThemeLayout {
    fn validate(&self) -> Result<(), ThemeValidationError> {
        let card = self.card;
        if !card.radius.is_finite() || !(0.0..=MAX_PANE_CARD_RADIUS).contains(&card.radius) {
            return Err(ThemeValidationError::InvalidLayoutRadius);
        }
        if !card.gutter.is_finite() || !(0.0..=MAX_PANE_CARD_GUTTER).contains(&card.gutter) {
            return Err(ThemeValidationError::InvalidLayoutGutter);
        }
        if !card.divider.is_finite() || !(0.0..=MAX_PANE_CARD_DIVIDER).contains(&card.divider) {
            return Err(ThemeValidationError::InvalidLayoutDivider);
        }
        if self
            .padding_x
            .is_some_and(|padding| !padding.is_finite() || !(0.0..=48.0).contains(&padding))
        {
            return Err(ThemeValidationError::InvalidLayoutPaddingX);
        }
        if self
            .padding_y
            .is_some_and(|padding| !padding.is_finite() || !(0.0..=40.0).contains(&padding))
        {
            return Err(ThemeValidationError::InvalidLayoutPaddingY);
        }
        Ok(())
    }
}

/// 可选视觉特效;路径与 fit/alignment 是渲染层负责的原文值。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ThemeEffects {
    pub opacity: Option<f32>,
    pub blur: Option<BlurMode>,
    pub cursor_shape: Option<CursorShapeName>,
    pub background_image: Option<String>,
    pub background_image_opacity: Option<f32>,
    pub background_image_fit: Option<String>,
    pub background_image_alignment: Option<String>,
    pub background_image_cover_chrome: Option<bool>,
}

impl ThemeEffects {
    fn validate(&self) -> Result<(), ThemeValidationError> {
        if self
            .opacity
            .is_some_and(|opacity| !opacity.is_finite() || !(0.0..=1.0).contains(&opacity))
        {
            return Err(ThemeValidationError::InvalidOpacity);
        }
        if self
            .background_image_opacity
            .is_some_and(|opacity| !opacity.is_finite() || !(0.0..=1.0).contains(&opacity))
        {
            return Err(ThemeValidationError::InvalidBackgroundImageOpacity);
        }
        for value in [
            self.background_image.as_deref(),
            self.background_image_fit.as_deref(),
            self.background_image_alignment.as_deref(),
        ]
        .into_iter()
        .flatten()
        {
            if value.chars().any(char::is_control) {
                return Err(ThemeValidationError::InvalidEffectText);
            }
        }
        Ok(())
    }
}

const MAX_THEME_NAME_BYTES: usize = 128;
const MAX_FONT_FAMILY_BYTES: usize = 256;

/// 主题不能安全应用或导出的原因。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeValidationError {
    EmptyName,
    NameTooLong,
    NameContainsControlCharacter,
    InvalidFontFamily,
    InvalidFontSize,
    InvalidLineHeight,
    InvalidLetterSpacing,
    InvalidFontWeight,
    InvalidUiFontFamily,
    InvalidUiFontSize,
    InvalidLayoutRadius,
    InvalidLayoutGutter,
    InvalidLayoutDivider,
    InvalidLayoutPaddingX,
    InvalidLayoutPaddingY,
    InvalidOpacity,
    InvalidBackgroundImageOpacity,
    InvalidEffectText,
}

impl fmt::Display for ThemeValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::EmptyName => "theme name is empty",
            Self::NameTooLong => "theme name is too long",
            Self::NameContainsControlCharacter => "theme name contains a control character",
            Self::InvalidFontFamily => {
                "font family is empty, too long, or contains a control character"
            }
            Self::InvalidFontSize => "font size must be finite and between 4 and 96",
            Self::InvalidLineHeight => "line height must be finite and between 0.5 and 3",
            Self::InvalidLetterSpacing => "letter spacing must be finite and between 0 and 8",
            Self::InvalidFontWeight => "font weight must be between 100 and 900",
            Self::InvalidUiFontFamily => {
                "UI font family is empty, too long, or contains a control character"
            }
            Self::InvalidUiFontSize => "UI font size must be finite and between 10 and 20",
            Self::InvalidLayoutRadius => "card radius is outside the shared pane range",
            Self::InvalidLayoutGutter => "card gutter is outside the shared pane range",
            Self::InvalidLayoutDivider => "card divider is outside the shared pane range",
            Self::InvalidLayoutPaddingX => "horizontal layout padding is outside the theme range",
            Self::InvalidLayoutPaddingY => "vertical layout padding is outside the theme range",
            Self::InvalidOpacity => "opacity must be finite and between 0 and 1",
            Self::InvalidBackgroundImageOpacity => {
                "background image opacity must be finite and between 0 and 1"
            }
            Self::InvalidEffectText => "effect text contains a control character",
        };
        f.write_str(message)
    }
}

impl std::error::Error for ThemeValidationError {}

/// 主题未带完整色表时的暗色 ANSI 兜底:从 UI 语义槽合成。
fn fallback_ansi(palette: crate::ReviewedPalette) -> [Rgb8; 16] {
    [
        palette.shell,
        palette.red,
        palette.green,
        palette.yellow,
        palette.blue,
        palette.purple,
        palette.cyan,
        palette.foreground,
        palette.frame,
        palette.red,
        palette.green,
        palette.yellow,
        palette.blue,
        palette.purple,
        palette.cyan,
        palette.foreground,
    ]
}

fn mix_rgb(left: Rgb8, right: Rgb8, weight: f32) -> Rgb8 {
    [0, 1, 2].map(|index| {
        (f32::from(left[index]) * (1.0 - weight) + f32::from(right[index]) * weight).round() as u8
    })
}

/// 返回原色 + 冷/暖两个可读候选。
///
/// 第一项永远是 `original`——用户颜色绝不静默改写,候选只供建议。后两项
/// 按 WCAG 2 AA 普通文本 4.5:1 阈值对给定不透明底色校验。合成/半透明
/// 表面的实际渲染结果仍由调用方校验。
pub fn foreground_recommendations(background: Rgb8, original: Rgb8) -> [Rgb8; 3] {
    [
        original,
        readable_tint(background, [100, 178, 224]),
        readable_tint(background, [235, 164, 76]),
    ]
}

/// 两个不透明 sRGB 色的 WCAG 相对对比度。
pub fn wcag_contrast_ratio(foreground: Rgb8, background: Rgb8) -> f64 {
    let a = relative_luminance(foreground);
    let b = relative_luminance(background);
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

/// 不透明前景/背景对是否达到 WCAG AA 普通文本阈值。
pub fn meets_wcag_aa(foreground: Rgb8, background: Rgb8) -> bool {
    wcag_contrast_ratio(foreground, background) >= 4.5
}

fn readable_tint(background: Rgb8, tint: Rgb8) -> Rgb8 {
    let white = [255; 3];
    let black = [0; 3];
    let endpoint =
        if wcag_contrast_ratio(white, background) >= wcag_contrast_ratio(black, background) {
            white
        } else {
            black
        };

    if meets_wcag_aa(tint, background) {
        return tint;
    }
    // 向对比度更优的端点逐步混合,在对比度下限内尽量保留请求的冷/暖色相。
    for step in 1..=255u16 {
        let candidate = blend_rgb(tint, endpoint, step as u8);
        if meets_wcag_aa(candidate, background) {
            return candidate;
        }
    }
    endpoint
}

fn blend_rgb(from: Rgb8, to: Rgb8, amount: u8) -> Rgb8 {
    let amount = u16::from(amount);
    let mut result = [0; 3];
    for index in 0..3 {
        let from = u16::from(from[index]);
        let to = u16::from(to[index]);
        result[index] = ((from * (255 - amount) + to * amount + 127) / 255) as u8;
    }
    result
}

/// 相对亮度:WCAG 2 公式(sRGB 线性化 + 0.2126/0.7152/0.0722 加权)。
fn relative_luminance(color: Rgb8) -> f64 {
    let linear = |channel: u8| {
        let channel = f64::from(channel) / 255.0;
        if channel <= 0.04045 {
            channel / 12.92
        } else {
            ((channel + 0.055) / 1.055).powf(2.4)
        }
    };
    linear(color[0]) * 0.2126 + linear(color[1]) * 0.7152 + linear(color[2]) * 0.0722
}

#[cfg(test)]
mod custom_theme_tests {
    use super::*;

    #[test]
    fn builtin_snapshots_preserve_standard_extended_colors() {
        // 提示符配色不能覆盖应用使用的 xterm 色立方与灰阶。
        for name in ThemeName::BUILTIN {
            let snapshot = ThemeDefinition::from_builtin(name);
            for (index, expected) in [
                (16, [0, 0, 0]),
                (17, [0, 0, 95]),
                (18, [0, 0, 135]),
                (19, [0, 0, 175]),
                (20, [0, 0, 215]),
                (21, [0, 0, 255]),
                (22, [0, 95, 0]),
                (23, [0, 95, 95]),
                (231, [255, 255, 255]),
                (232, [8, 8, 8]),
                (255, [238, 238, 238]),
            ] {
                assert_eq!(
                    snapshot.terminal.palette.colors[index],
                    expected,
                    "{} index {index}",
                    name.prompt_name()
                );
            }
        }
    }

    #[test]
    fn builtin_snapshot_keeps_declared_exact_colors() {
        for name in ThemeName::BUILTIN {
            let Some(exact) = name.term_theme().exact else {
                continue;
            };
            let snapshot = ThemeDefinition::from_builtin(name);
            assert_eq!(
                snapshot.terminal.foreground,
                exact.foreground,
                "{}",
                name.prompt_name()
            );
            assert_eq!(
                snapshot.terminal.palette.ansi_colors(),
                exact.ansi,
                "{}",
                name.prompt_name()
            );
            assert_eq!(
                snapshot.terminal.cursor,
                exact.cursor,
                "{}",
                name.prompt_name()
            );
            assert_eq!(
                snapshot.terminal.selection_background,
                exact.selection_background,
                "{}",
                name.prompt_name()
            );
            // 派生 UI 面:显式 alpha 槽不透明化。
            assert_eq!(snapshot.ui.selection.len(), 4);
        }
    }

    #[test]
    fn ui_derivation_and_explicit_alpha_are_available_to_adapters() {
        let palette = ThemeName::GlassDark.reviewed_palette();
        let mut theme = ThemeDefinition::from_builtin(ThemeName::GlassDark);
        assert_eq!(theme.ui.selection[3], 0x15, "显式 UI 槽保留自带 alpha");
        theme.ui.derive = true;
        assert!(theme.resolved_ui().derive);
        // 派生面不透明;显式面保持原值。
        assert_eq!(theme.resolved_ui().selection[3], 255);
        theme.ui.derive = false;
        assert_eq!(theme.resolved_ui().selection[3], 0x15);

        assert_eq!(
            ThemeUiColors::from_palette(palette).selection,
            palette.selected
        );
        assert_eq!(ThemeUiColors::from_palette(palette).line, palette.line);
    }

    #[test]
    fn ui_anchors_derive_from_terminal_with_the_dark_weight() {
        // 暗色单权重:终端前景/背景逐字不动,选区按 0.18 混合,
        // shell 向白抬 0.06,语义色取 ANSI 亮色系槽位。
        let terminal = ThemeDefinition::from_builtin(ThemeName::Nord).terminal;
        let ui = ThemeUiColors::from_terminal(&terminal);
        assert_eq!(ui.background, terminal.background);
        assert_eq!(ui.foreground, terminal.foreground);
        assert!(ui.derive);
        let expected_shell = mix_rgb(terminal.background, [255, 255, 255], 0.06);
        assert_eq!(ui.shell, expected_shell);
        assert_eq!(ui.muted, terminal.palette.colors[8]);
        assert_eq!(ui.error, terminal.palette.colors[9]);
        assert_eq!(ui.line_rgb(), ui.muted);
        // 选区有显式声明时逐字采用,不参与混合。
        assert_eq!(
            ui.selection[..3],
            terminal.selection_background.expect("nord selection")[..]
        );
    }

    #[test]
    fn indexed_palette_is_complete_and_ansi_is_mirrored() {
        let theme = ThemeDefinition::from_builtin(ThemeName::CatppuccinMocha);
        let ansi = theme.terminal.palette.ansi_colors();
        assert_eq!(&ansi[..], &theme.terminal.palette.colors[..16]);
        assert!(theme.terminal.palette.get(255).is_some());
        assert_eq!(theme.terminal.palette.colors[232], [8; 3]);
        assert_eq!(theme.terminal.palette.colors[255], [238; 3]);
        assert_eq!(theme.terminal.ansi(3), theme.terminal.indexed(3));
    }

    #[test]
    fn edits_clone_without_mutating_the_source() {
        let source = ThemeDefinition::from_builtin(ThemeName::Nord);
        let edited = source.with_terminal_foreground([1, 2, 3]);
        assert_eq!(source.terminal.foreground, [0xf1, 0xf6, 0xff]);
        assert_eq!(edited.terminal.foreground, [1, 2, 3]);
        let edited = source.with_ansi(0, [9, 8, 7]).unwrap();
        assert_eq!(source.terminal.palette.colors[0], [0x3b, 0x42, 0x52]);
        assert_eq!(edited.terminal.palette.colors[0], [9, 8, 7]);
        assert!(source.with_ansi(16, [0, 0, 0]).is_none());
        assert!(source.with_indexed(256, [0, 0, 0]).is_none());
        let with_cursor = source.with_cursor(Some([1, 1, 1]), None, Some([2, 2, 2]));
        assert_eq!(with_cursor.terminal.cursor, Some([1, 1, 1]));
        assert_eq!(with_cursor.terminal.cursor_text, None);
        assert_eq!(source.terminal.cursor, Some([0xe5, 0xe9, 0xf0]));
        let with_selection = source.with_selection(Some([3, 3, 3]), None);
        assert_eq!(
            with_selection.terminal.selection_foreground,
            Some([3, 3, 3])
        );
        let mut palette = source.terminal.palette.clone();
        assert!(!palette.set(256, [0, 0, 0]));
    }

    #[test]
    fn validate_rejects_every_bad_scalar_variant() {
        let good = ThemeDefinition::from_builtin(ThemeName::Linear);
        assert!(good.validate().is_ok());

        // 每例 = 一处变异 + 期望错误;函数指针表驱动,逐变体钉死。
        type MutationCase = (fn(&mut ThemeDefinition), ThemeValidationError);
        let cases: Vec<MutationCase> = vec![
            (
                |theme| theme.name = "  ".into(),
                ThemeValidationError::EmptyName,
            ),
            (
                |theme| theme.name = "x".repeat(129),
                ThemeValidationError::NameTooLong,
            ),
            (
                |theme| theme.name = "a\u{7}".into(),
                ThemeValidationError::NameContainsControlCharacter,
            ),
            (
                |theme| theme.typography.font_family = Some("  ".into()),
                ThemeValidationError::InvalidFontFamily,
            ),
            (
                |theme| theme.typography.font_size = Some(2.0),
                ThemeValidationError::InvalidFontSize,
            ),
            (
                |theme| theme.typography.font_size = Some(f32::NAN),
                ThemeValidationError::InvalidFontSize,
            ),
            (
                |theme| theme.typography.line_height = Some(4.0),
                ThemeValidationError::InvalidLineHeight,
            ),
            (
                |theme| theme.typography.letter_spacing = Some(9.0),
                ThemeValidationError::InvalidLetterSpacing,
            ),
            (
                |theme| theme.typography.font_weight = Some(50),
                ThemeValidationError::InvalidFontWeight,
            ),
            (
                |theme| theme.typography.ui_font_family = Some(String::new()),
                ThemeValidationError::InvalidUiFontFamily,
            ),
            (
                |theme| theme.typography.ui_font_size = Some(30.0),
                ThemeValidationError::InvalidUiFontSize,
            ),
            (
                |theme| theme.layout.card.radius = 99.0,
                ThemeValidationError::InvalidLayoutRadius,
            ),
            (
                |theme| theme.layout.card.gutter = 99.0,
                ThemeValidationError::InvalidLayoutGutter,
            ),
            (
                |theme| theme.layout.card.divider = 99.0,
                ThemeValidationError::InvalidLayoutDivider,
            ),
            (
                |theme| theme.layout.padding_x = Some(49.0),
                ThemeValidationError::InvalidLayoutPaddingX,
            ),
            (
                |theme| theme.layout.padding_y = Some(41.0),
                ThemeValidationError::InvalidLayoutPaddingY,
            ),
            (
                |theme| theme.effects.opacity = Some(f32::NAN),
                ThemeValidationError::InvalidOpacity,
            ),
            (
                |theme| theme.effects.opacity = Some(1.5),
                ThemeValidationError::InvalidOpacity,
            ),
            (
                |theme| theme.effects.background_image_opacity = Some(-0.1),
                ThemeValidationError::InvalidBackgroundImageOpacity,
            ),
            (
                |theme| theme.effects.background_image_fit = Some("a\nb".into()),
                ThemeValidationError::InvalidEffectText,
            ),
        ];
        for (mutate, expected) in cases {
            let mut theme = good.clone();
            mutate(&mut theme);
            assert_eq!(theme.validate(), Err(expected));
            assert!(!expected.to_string().is_empty());
        }
    }
}

#[cfg(test)]
mod wcag_tests {
    use super::*;

    #[test]
    fn contrast_ratio_anchors_black_on_white_at_twenty_one_to_one() {
        let ratio = wcag_contrast_ratio([0, 0, 0], [255, 255, 255]);
        assert!((ratio - 21.0).abs() < 0.01, "{ratio}");
        assert_eq!(wcag_contrast_ratio([1, 2, 3], [1, 2, 3]), 1.0);
        // 前景/背景对称。
        assert_eq!(
            wcag_contrast_ratio([255, 255, 255], [0, 0, 0]),
            wcag_contrast_ratio([0, 0, 0], [255, 255, 255])
        );
    }

    #[test]
    fn meets_wcag_aa_uses_the_four_point_five_threshold() {
        assert!(meets_wcag_aa([0, 0, 0], [255, 255, 255]));
        assert!(!meets_wcag_aa([120, 120, 120], [127, 127, 127]));
        // 白底灰度从黑向亮扫:首个不达标灰阶与最后达标灰阶相邻跨线。
        let background = [255, 255, 255];
        let mut level = 0u8;
        while meets_wcag_aa([level, level, level], background) {
            level += 1;
        }
        assert!(meets_wcag_aa([level - 1, level - 1, level - 1], background));
        assert!(!meets_wcag_aa([level, level, level], background));
    }

    #[test]
    fn recommendations_keep_original_and_meet_wcag_for_solid_backgrounds() {
        for background in [[0, 0, 0], [15, 17, 26], [255, 255, 255], [127, 127, 127]] {
            let recommendations = foreground_recommendations(background, [1, 2, 3]);
            assert_eq!(recommendations[0], [1, 2, 3], "原色永远第一");
            assert!(
                meets_wcag_aa(recommendations[1], background),
                "{background:?} cool"
            );
            assert!(
                meets_wcag_aa(recommendations[2], background),
                "{background:?} warm"
            );
        }
    }

    #[test]
    fn readable_tint_keeps_compliant_colors_untouched() {
        // 已达标的颜色原样返回,不向端点漂移。
        let tint = [240, 240, 240];
        assert_eq!(readable_tint([10, 10, 12], tint), tint);
        // 两端点择优:深底向白端点收敛,浅底向黑端点收敛。
        let dark = readable_tint([10, 10, 12], [20, 20, 20]);
        let light = readable_tint([250, 250, 250], [240, 240, 240]);
        assert!(meets_wcag_aa(dark, [10, 10, 12]));
        assert!(meets_wcag_aa(light, [250, 250, 250]));
    }
}
