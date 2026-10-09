//! 内置主题目录:终端与 chrome 适配器共享的静态目录。
//!
//! 「终端色表」与「UI chrome 色」两套数据分离、各自单源;值静态,适配器
//! 不得去饱和 accent 或合成第二条选中色带。仅暗色:亮成员与 `is_light`
//! 语义不迁,目录 = 上游暗色子集 + Linear 默认主题(D06-3)。

use crate::{ExactTermColors, Rgb8, Rgba8, TermTheme, ThemeName};

#[derive(Clone, Copy, Debug)]
pub struct FreshPalette {
    pub shell: Rgb8,
    pub surface: Rgb8,
    pub accent: Rgb8,
    pub foreground: Rgb8,
    pub muted: Rgb8,
}

impl ThemeName {
    /// 可选内置主题的唯一目录。目录即合同:注册即入列;退休标识的机制位
    /// 由 `available` 承载(当前无退休 id——本仓无历史发布)。
    pub const BUILTIN: [Self; 8] = [
        Self::Linear,
        Self::BreezeDark,
        Self::MintDark,
        Self::Nord,
        Self::CatppuccinMocha,
        Self::CatppuccinFrappe,
        Self::CatppuccinMacchiato,
        Self::GlassDark,
    ];

    pub const BUILTIN_NAMES: [&'static str; Self::BUILTIN.len()] = {
        let mut names = [""; Self::BUILTIN.len()];
        let mut index = 0;
        while index < Self::BUILTIN.len() {
            names[index] = Self::BUILTIN[index].prompt_name();
            index += 1;
        }
        names
    };

    /// 退休主题名映射到继任主题的机制位:老配置可解析、选择器不出现。
    /// 当前目录无退休成员,映射恒等。
    pub const fn available(self) -> Self {
        self
    }

    pub fn fresh_palette(self) -> Option<FreshPalette> {
        let palette = self.reviewed_palette();
        Some(FreshPalette {
            shell: palette.shell,
            surface: palette.background,
            accent: palette.accent,
            foreground: palette.foreground,
            muted: palette.muted,
        })
    }
}

const fn rgb(value: u32) -> Rgb8 {
    [(value >> 16) as u8, (value >> 8) as u8, value as u8]
}

/// 评审过的 UI 语义槽位集。值静态;适配器只引用槽,不得自行混合。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReviewedPalette {
    pub shell: Rgb8,
    pub background: Rgb8,
    pub foreground: Rgb8,
    pub muted: Rgb8,
    pub accent: Rgb8,
    pub selected: Rgba8,
    pub line: Rgba8,
    pub red: Rgb8,
    pub green: Rgb8,
    pub yellow: Rgb8,
    pub blue: Rgb8,
    pub purple: Rgb8,
    pub cyan: Rgb8,
    pub frame: Rgb8,
}

impl ReviewedPalette {
    /// selected 前景色按 alpha 叠到面板底的 CSS 式合成。
    pub const fn code_background(self) -> Rgb8 {
        let alpha = self.selected[3] as u32;
        let mut color = [0; 3];
        let mut index = 0;
        while index < 3 {
            color[index] = ((self.selected[index] as u32 * alpha
                + self.background[index] as u32 * (255 - alpha)
                + 127)
                / 255) as u8;
            index += 1;
        }
        color
    }
}

impl ThemeName {
    pub const fn reviewed_palette(self) -> ReviewedPalette {
        match self.available() {
            Self::Linear => ReviewedPalette {
                // 旧栈配色契约定稿值;selected/line 是半透明白/accent 的
                // alpha 槽位,frame 取自绘标题栏 chrome 底。
                shell: [0x10, 0x10, 0x12],
                background: [0x0a, 0x0a, 0x0b],
                foreground: [0xb3, 0xae, 0xa6],
                muted: [0x8a, 0x85, 0x7d],
                accent: [0x6e, 0x9f, 0xf2],
                selected: [0x6e, 0x9f, 0xf2, 33],
                line: [0xff, 0xff, 0xff, 14],
                red: [0xd9, 0x70, 0x6b],
                green: [0x86, 0xbb, 0x7a],
                yellow: [0xd6, 0xb2, 0x5e],
                blue: [0x6e, 0x9f, 0xf2],
                purple: [0xb4, 0x8c, 0xe0],
                cyan: [0x6f, 0xbf, 0xc4],
                frame: [0x14, 0x14, 0x16],
            },
            Self::BreezeDark => ReviewedPalette {
                shell: [0x18, 0x23, 0x2e],
                background: [0x20, 0x2e, 0x3b],
                foreground: [0xe0, 0xea, 0xf3],
                muted: [0xa5, 0xb6, 0xc7],
                accent: [0x91, 0xbc, 0xdf],
                selected: [0x2b, 0x3c, 0x4b, 255],
                line: [0x34, 0x46, 0x54, 255],
                red: [0xe4, 0x9a, 0x9d],
                green: [0x9b, 0xc5, 0xa6],
                yellow: [0xe0, 0xc3, 0x8b],
                blue: [0x91, 0xbc, 0xdf],
                purple: [0xc5, 0xad, 0xdd],
                cyan: [0x91, 0xce, 0xcf],
                frame: [0x50, 0x65, 0x79],
            },
            Self::MintDark => ReviewedPalette {
                shell: [0x19, 0x2a, 0x26],
                background: [0x21, 0x37, 0x30],
                foreground: [0xdf, 0xee, 0xe7],
                muted: [0xa6, 0xbf, 0xb2],
                accent: [0x8b, 0xcb, 0xb3],
                selected: [0x2d, 0x44, 0x3b, 255],
                line: [0x3a, 0x50, 0x46, 255],
                red: [0xe4, 0x9a, 0x9d],
                green: [0x9b, 0xc5, 0xa6],
                yellow: [0xe0, 0xc3, 0x8b],
                blue: [0x91, 0xbc, 0xdf],
                purple: [0xc5, 0xad, 0xdd],
                cyan: [0x91, 0xce, 0xcf],
                frame: [0x52, 0x6f, 0x62],
            },
            Self::Nord => ReviewedPalette {
                shell: [0x2e, 0x34, 0x40],
                background: [0x2e, 0x34, 0x40],
                foreground: [0xe5, 0xe9, 0xf0],
                muted: [0xab, 0xb5, 0xc7],
                accent: [0x88, 0xc0, 0xd0],
                selected: [0x3b, 0x42, 0x52, 255],
                line: [0x43, 0x4c, 0x5e, 255],
                red: [0xbf, 0x61, 0x6a],
                green: [0xa3, 0xbe, 0x8c],
                yellow: [0xeb, 0xcb, 0x8b],
                blue: [0x81, 0xa1, 0xc1],
                purple: [0xb4, 0x8e, 0xad],
                cyan: [0x88, 0xc0, 0xd0],
                frame: [0x65, 0x72, 0x86],
            },
            // Catppuccin/palette (MIT): https://github.com/catppuccin/palette
            // Base/Mantle surfaces, Text/Subtext 1 ink, Lavender accent.
            Self::CatppuccinMocha => ReviewedPalette {
                shell: [0x18, 0x18, 0x25],
                background: [0x1e, 0x1e, 0x2e],
                foreground: [0xcd, 0xd6, 0xf4],
                muted: [0xa6, 0xad, 0xc8],
                accent: [0xb4, 0xbe, 0xfe],
                selected: [0x31, 0x32, 0x44, 255],
                line: [0x45, 0x47, 0x5a, 255],
                red: [0xf3, 0x8b, 0xa8],
                green: [0xa6, 0xe3, 0xa1],
                yellow: [0xf9, 0xe2, 0xaf],
                blue: [0x89, 0xb4, 0xfa],
                purple: [0xcb, 0xa6, 0xf7],
                cyan: [0x94, 0xe2, 0xd5],
                frame: [0x6c, 0x70, 0x86],
            },
            Self::CatppuccinFrappe => ReviewedPalette {
                shell: [0x29, 0x2c, 0x3c],
                background: [0x30, 0x34, 0x46],
                foreground: [0xc6, 0xd0, 0xf5],
                muted: [0xb5, 0xbf, 0xe2],
                accent: [0xba, 0xbb, 0xf1],
                selected: [0x41, 0x45, 0x59, 255],
                line: [0x51, 0x57, 0x6d, 255],
                red: [0xe7, 0x82, 0x84],
                green: [0xa6, 0xd1, 0x89],
                yellow: [0xe5, 0xc8, 0x90],
                blue: [0x8c, 0xaa, 0xee],
                purple: [0xca, 0x9e, 0xe6],
                cyan: [0x81, 0xc8, 0xbe],
                frame: [0x73, 0x79, 0x94],
            },
            Self::CatppuccinMacchiato => ReviewedPalette {
                shell: [0x1e, 0x20, 0x30],
                background: [0x24, 0x27, 0x3a],
                foreground: [0xca, 0xd3, 0xf5],
                muted: [0xb8, 0xc0, 0xe0],
                accent: [0xb7, 0xbd, 0xf8],
                selected: [0x36, 0x3a, 0x4f, 255],
                line: [0x49, 0x4d, 0x64, 255],
                red: [0xed, 0x87, 0x96],
                green: [0xa6, 0xda, 0x95],
                yellow: [0xee, 0xd4, 0x9f],
                blue: [0x8a, 0xad, 0xf4],
                purple: [0xc6, 0xa0, 0xf6],
                cyan: [0x8b, 0xd5, 0xca],
                frame: [0x6e, 0x73, 0x8d],
            },
            Self::GlassDark => ReviewedPalette {
                shell: [0x44, 0x44, 0x45],
                background: [0x40, 0x43, 0x4b],
                foreground: [0xf7, 0xf8, 0xff],
                muted: [0xc4, 0xca, 0xd6],
                accent: [0xbb, 0xc9, 0xed],
                selected: [0xff, 0xff, 0xff, 0x15],
                line: [0x6b, 0x72, 0x86, 0x40],
                red: [0xff, 0x8a, 0x8a],
                green: [0xa8, 0xd4, 0x6f],
                yellow: [0xe8, 0xc7, 0x78],
                blue: [0x8d, 0xb7, 0xff],
                purple: [0xd1, 0xa3, 0xff],
                cyan: [0x7f, 0xd6, 0xc2],
                frame: [0x7e, 0x8b, 0x9d],
            },
        }
    }
}

/// fresh 形态主题(目录中非手写分支的成员)的终端色表:背景取 surface,
/// ANSI-16 逐主题定制,光标/选区从语义槽派生。
pub(crate) fn fresh_terminal(name: ThemeName) -> TermTheme {
    let palette = name.fresh_palette().expect("fresh theme");
    let ansi = match name {
        ThemeName::CatppuccinMocha => [
            0x45475a, 0xf38ba8, 0xa6e3a1, 0xf9e2af, 0x89b4fa, 0xf5c2e7, 0x94e2d5, 0xbac2de,
            0x585b70, 0xf38ba8, 0xa6e3a1, 0xf9e2af, 0x89b4fa, 0xf5c2e7, 0x94e2d5, 0xa6adc8,
        ],
        // 官方 ANSI 0–15,含独立亮色系(2026-09-12 上游校准)。
        ThemeName::CatppuccinFrappe => [
            0x51576d, 0xe78284, 0xa6d189, 0xe5c890, 0x8caaee, 0xf4b8e4, 0x81c8be, 0xa5adce,
            0x626880, 0xe67172, 0x8ec772, 0xd9ba73, 0x7b9ef0, 0xf2a4db, 0x5abfb5, 0xb5bfe2,
        ],
        ThemeName::CatppuccinMacchiato => [
            0x494d64, 0xed8796, 0xa6da95, 0xeed49f, 0x8aadf4, 0xf5bde6, 0x8bd5ca, 0xa5adcb,
            0x5b6078, 0xec7486, 0x8ccf7f, 0xe1c682, 0x78a1f6, 0xf2a9dd, 0x63cbc0, 0xb8c0e0,
        ],
        ThemeName::GlassDark => [
            0x252a35, 0xff8a8a, 0xa8d46f, 0xe8c778, 0x8db7ff, 0xd1a3ff, 0x7fd6c2, 0xe3e6f0,
            0x747b8e, 0xffb0a8, 0xc8ea90, 0xf2da9a, 0xb2ccff, 0xe0c2ff, 0xa3e6d8, 0xffffff,
        ],
        // 暗色默认 ANSI 表(BreezeDark/MintDark 及未来 fresh 暗色成员)。
        _ => [
            0x263b49, 0xe49a9d, 0x9bc5a6, 0xe0c38b, 0x91bcdf, 0xc5addd, 0x91cecf, 0xdfeaf1,
            0x90a5b5, 0xf0b2b3, 0xb1dcc0, 0xeed4a5, 0xb0d1ed, 0xd9c4ed, 0xb0e1dc, 0xf2f7fa,
        ],
    }
    .map(rgb);
    TermTheme {
        background: palette.surface,
        exact: Some(ExactTermColors {
            foreground: palette.foreground,
            ansi,
            cursor: Some(palette.accent),
            cursor_text: Some(palette.surface),
            cursor_stroke: Some(palette.accent),
            selection_foreground: Some(palette.foreground),
            selection_background: Some(palette.shell),
        }),
    }
}

#[cfg(test)]
mod themes_tests {
    use super::*;

    #[test]
    fn builtin_catalog_is_dark_only_and_names_match_saved_values() {
        assert_eq!(ThemeName::BUILTIN.len(), ThemeName::BUILTIN_NAMES.len());
        for (theme, name) in ThemeName::BUILTIN.into_iter().zip(ThemeName::BUILTIN_NAMES) {
            assert_eq!(theme.prompt_name(), name);
            assert_eq!(ThemeName::from_prompt_name(name), Some(theme));
        }
        assert_eq!(
            ThemeName::BUILTIN[0],
            ThemeName::Linear,
            "默认主题居目录首位"
        );
        // 退休映射机制位:当前无退休 id,映射恒等。
        for theme in ThemeName::BUILTIN {
            assert_eq!(theme.available(), theme);
        }
    }

    #[test]
    fn builtin_palettes_keep_a_readable_foreground() {
        // 前景/底色的 WCAG 对比度下限:内置目录逐成员钉死。
        fn luma(color: Rgb8) -> f64 {
            color
                .into_iter()
                .zip([0.2126, 0.7152, 0.0722])
                .map(|(channel, weight)| {
                    let channel = f64::from(channel) / 255.0;
                    (if channel <= 0.04045 {
                        channel / 12.92
                    } else {
                        ((channel + 0.055) / 1.055).powf(2.4)
                    }) * weight
                })
                .sum::<f64>()
        }
        for name in ThemeName::BUILTIN {
            let palette = name.fresh_palette().expect("fresh palette");
            let terminal = name.term_theme();
            assert_eq!(terminal.background, palette.surface);
            let foreground = luma(palette.foreground);
            let surface = luma(palette.surface);
            assert!(
                (foreground.max(surface) + 0.05) / (foreground.min(surface) + 0.05) >= 7.0,
                "{} foreground/surface contrast",
                name.prompt_name()
            );
        }
    }

    #[test]
    fn fresh_terminal_derives_cursor_and_selection_from_palette_slots() {
        for name in [
            ThemeName::BreezeDark,
            ThemeName::MintDark,
            ThemeName::CatppuccinMocha,
            ThemeName::CatppuccinFrappe,
            ThemeName::CatppuccinMacchiato,
            ThemeName::GlassDark,
        ] {
            let palette = name.reviewed_palette();
            let exact = name.term_theme().exact.expect("exact colors");
            assert_eq!(exact.cursor, Some(palette.accent), "{}", name.prompt_name());
            assert_eq!(
                exact.selection_background,
                Some(palette.shell),
                "{}",
                name.prompt_name()
            );
        }
        // Catppuccin Frappe 的独立亮色系(上游校准)逐字节锚定。
        let frappe = ThemeName::CatppuccinFrappe.term_theme().exact.unwrap();
        assert_eq!(frappe.ansi[8], rgb(0x626880));
        assert_eq!(frappe.ansi[9], rgb(0xe67172));
        assert_ne!(frappe.ansi[1], frappe.ansi[9]);
    }

    #[test]
    fn code_background_composites_selected_over_background() {
        let palette = ThemeName::Nord.reviewed_palette();
        // selected alpha = 255:合成结果就是 selected 本身。
        assert_eq!(palette.code_background(), palette.selected[..3]);
        let glass = ThemeName::GlassDark.reviewed_palette();
        // alpha 21:向底色回落。
        let composited = glass.code_background();
        assert_ne!(composited, glass.selected[..3]);
        let expected = (0xffu32 * 0x15 + 0x40u32 * (255 - 0x15) + 127) / 255;
        assert_eq!(composited[0], expected as u8);
    }
}
