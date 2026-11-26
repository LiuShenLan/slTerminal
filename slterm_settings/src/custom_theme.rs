//! 用户自建主题的值模型。
//!
//! 本模块刻意不含任何文件格式与序列化代码:导入/导出在应用边界把文件
//! 翻译成 [`ThemeDefinition`],settings crate 保持小而零依赖的值权威。
//! 仅暗色:外观枚举与亮暗双权重混合不迁,派生规则为暗色单权重。

use core::fmt;

use crate::{ExactTermColors, Rgb8, Rgba8, ThemeName, format_hex_rgb, parse_hex_rgb};

/// 完整用户主题。`base` 记录起点内置主题快照的元数据,不建运行时继承链;
/// 克隆定义自含全部字符串与数组。
#[derive(Clone, Debug, PartialEq)]
pub struct ThemeDefinition {
    pub name: String,
    pub base: ThemeName,
    pub terminal: TerminalThemeColors,
    pub ui: ThemeUiColors,
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
                ansi: exact.ansi,
            },
            ui: ThemeUiColors::from_palette(palette),
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

    /// 返回副本:改写一个 ANSI 槽位。16–255 不是可编辑槽位。
    pub fn with_ansi(&self, index: usize, color: Rgb8) -> Option<Self> {
        if index >= 16 {
            return None;
        }
        let mut theme = self.clone();
        theme.terminal.ansi[index] = color;
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
    /// ANSI 0..15；索引色 16..255 只按 xterm 公式派生。
    pub ansi: [Rgb8; 16],
}

impl TerminalThemeColors {
    pub fn ansi(&self, index: usize) -> Option<Rgb8> {
        self.ansi.get(index).copied()
    }

    /// 完整 xterm 256 色索引：16 色以外不保存第二份色表。
    pub fn indexed(&self, index: usize) -> Option<Rgb8> {
        match index {
            0..=15 => self.ansi.get(index).copied(),
            _ => u8::try_from(index).ok().map(xterm_indexed_color),
        }
    }
}

/// xterm 16–255 索引色的唯一派生公式（6×6×6 色立方 + 灰阶）。
pub fn xterm_indexed_color(index: u8) -> Rgb8 {
    const LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];
    match index {
        16..=231 => {
            let cube = usize::from(index - 16);
            [LEVELS[cube / 36], LEVELS[(cube / 6) % 6], LEVELS[cube % 6]]
        }
        _ => {
            let level = 8 + (index - 232) * 10;
            [level; 3]
        }
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
        let muted = terminal.ansi[8];
        let accent = terminal
            .cursor
            .or(terminal.cursor_stroke)
            .unwrap_or(terminal.ansi[14]);
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
            error: terminal.ansi[9],
            success: terminal.ansi[10],
            warning: terminal.ansi[11],
            info: terminal.ansi[14],
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

const MAX_THEME_NAME_BYTES: usize = 128;

/// 主题不能安全应用或导出的原因。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeValidationError {
    EmptyName,
    NameTooLong,
    NameContainsControlCharacter,
}

impl fmt::Display for ThemeValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::EmptyName => "theme name is empty",
            Self::NameTooLong => "theme name is too long",
            Self::NameContainsControlCharacter => "theme name contains a control character",
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
                    snapshot.terminal.indexed(index),
                    Some(expected),
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
            assert_eq!(snapshot.terminal.ansi, exact.ansi, "{}", name.prompt_name());
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
        assert_eq!(ui.muted, terminal.ansi[8]);
        assert_eq!(ui.error, terminal.ansi[9]);
        assert_eq!(ui.line_rgb(), ui.muted);
        // 选区有显式声明时逐字采用,不参与混合。
        assert_eq!(
            ui.selection[..3],
            terminal.selection_background.expect("nord selection")[..]
        );
    }

    #[test]
    fn indexed_colors_are_derived_from_the_single_xterm_formula() {
        let theme = ThemeDefinition::from_builtin(ThemeName::CatppuccinMocha);
        assert_eq!(theme.terminal.indexed(3), theme.terminal.ansi(3));
        assert_eq!(theme.terminal.indexed(232), Some([8; 3]));
        assert_eq!(theme.terminal.indexed(255), Some([238; 3]));
        assert_eq!(theme.terminal.indexed(256), None);
        // 前 16 色与派生索引色来自同一函数，不会出现第二份表。
        for index in 0..16usize {
            assert_eq!(
                theme.terminal.indexed(index),
                Some(theme.terminal.ansi[index])
            );
        }
        assert_eq!(xterm_indexed_color(231), [255, 255, 255]);
    }

    #[test]
    fn edits_clone_without_mutating_the_source() {
        let source = ThemeDefinition::from_builtin(ThemeName::Nord);
        let edited = source.with_terminal_foreground([1, 2, 3]);
        assert_eq!(source.terminal.foreground, [0xf1, 0xf6, 0xff]);
        assert_eq!(edited.terminal.foreground, [1, 2, 3]);
        let edited = source.with_ansi(0, [9, 8, 7]).unwrap();
        assert_eq!(source.terminal.ansi[0], [0x3b, 0x42, 0x52]);
        assert_eq!(edited.terminal.ansi[0], [9, 8, 7]);
        assert!(source.with_ansi(16, [0, 0, 0]).is_none());
        let with_cursor = source.with_cursor(Some([1, 1, 1]), None, Some([2, 2, 2]));
        assert_eq!(with_cursor.terminal.cursor, Some([1, 1, 1]));
        assert_eq!(with_cursor.terminal.cursor_text, None);
        assert_eq!(source.terminal.cursor, Some([0xe5, 0xe9, 0xf0]));
        let with_selection = source.with_selection(Some([3, 3, 3]), None);
        assert_eq!(
            with_selection.terminal.selection_foreground,
            Some([3, 3, 3])
        );
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
mod no_contrast_gate_tests {
    /// 主题合同：不做对比度校验、提示或自动回退；实现与模块文档都不得
    /// 保留门槛承诺。
    #[test]
    fn theme_sources_do_not_validate_contrast() {
        let needle = concat!("wc", "ag");
        for (name, source) in [
            ("custom_theme.rs", include_str!("custom_theme.rs")),
            ("themes.rs", include_str!("themes.rs")),
        ] {
            assert!(
                !source.to_ascii_lowercase().contains(needle),
                "{name} 不得保留对比度门槛实现或承诺"
            );
        }
    }
}
