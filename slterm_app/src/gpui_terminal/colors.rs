//! 终端调色板:`Palette` 类型 + 暗色默认 + vte `Color` → GPUI `Rgba` 解析面。
//!
//! 默认值与上游 deep-space 暗色主题逐字段一致(D06-3:M3 只保一份内置暗色,
//! 仅暗色定位约束);主题矫正器(`TerminalColorResolver`)归 06 篇。
//!
//! 解析顺序:OSC 4/10/11 的运行时覆盖(`Term::colors()`)优先,其次是
//! `Palette` 静态值。

use gpui::Rgba;
use slterm_terminal::term::color::Colors;
use vte::ansi::{Color, NamedColor, Rgb};

const fn rgb8(r: u8, g: u8, b: u8) -> Rgba {
    Rgba {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
        a: 1.0,
    }
}

/// 一份终端实例的静态配色。
#[derive(Clone)]
pub struct Palette {
    pub foreground: Rgba,
    pub background: Rgba,
    pub bright_foreground: Rgba,
    pub dim_foreground: Rgba,
    /// 块状光标的填充色。
    pub cursor: Rgba,
    /// 实心光标内的文字色;`None` 保留主题无显式值时的背景色回退。
    pub cursor_text: Option<Rgba>,
    /// bar/underline 光标的描边色(可与 block 不同)。
    pub cursor_stroke: Option<Rgba>,
    /// 选区叠加色:半透明叠加近似反色语义。
    pub selection: Rgba,
    /// 显式声明的选区文字色;未声明时保留原字色。
    pub selection_foreground: Option<Rgba>,
    /// ANSI 0-15。
    pub ansi: [Rgba; 16],
    /// Dim 0-7。
    pub dim: [Rgba; 8],
    /// 16-255 的 indexed 覆盖。
    pub indexed: Vec<(u8, Rgba)>,
}

impl Default for Palette {
    fn default() -> Self {
        Palette {
            background: rgb8(0x08, 0x0a, 0x18),
            foreground: rgb8(0xd6, 0xda, 0xea),
            bright_foreground: rgb8(0xd6, 0xda, 0xea),
            dim_foreground: Self::dim_of(rgb8(0xd6, 0xda, 0xea)),
            cursor: rgb8(0x49, 0x4d, 0x72),
            cursor_text: None,
            cursor_stroke: None,
            selection: Rgba {
                a: 0.60,
                ..rgb8(0x49, 0x4d, 0x72)
            },
            selection_foreground: None,
            ansi: [
                // normal
                rgb8(0x1a, 0x1d, 0x2e), // black
                rgb8(0xff, 0x6b, 0x81), // red
                rgb8(0x65, 0xe8, 0x6e), // green
                rgb8(0xf5, 0xc8, 0x4c), // yellow
                rgb8(0x38, 0xa8, 0xff), // blue
                rgb8(0xb4, 0x8c, 0xff), // magenta
                rgb8(0x4f, 0xd6, 0xe0), // cyan
                rgb8(0xd6, 0xda, 0xea), // white
                // bright
                rgb8(0x8d, 0x94, 0xaa),
                rgb8(0xff, 0x8b, 0x9d),
                rgb8(0x86, 0xf0, 0x90),
                rgb8(0xff, 0xda, 0x7a),
                rgb8(0x6c, 0xc0, 0xff),
                rgb8(0xc9, 0xa8, 0xff),
                rgb8(0x73, 0xe4, 0xec),
                rgb8(0xf2, 0xf4, 0xfb),
            ],
            dim: [
                rgb8(0x0f, 0x0f, 0x0f),
                rgb8(0x71, 0x2b, 0x2b),
                rgb8(0x5f, 0x6f, 0x3a),
                rgb8(0xa1, 0x7e, 0x4d),
                rgb8(0x45, 0x68, 0x77),
                rgb8(0x70, 0x4d, 0x68),
                rgb8(0x4d, 0x77, 0x70),
                rgb8(0x8e, 0x8e, 0x8e),
            ],
            indexed: Vec::new(),
        }
    }
}

impl Palette {
    /// dim 系数(与上游同一系数 0.66)。
    pub fn dim_of(color: Rgba) -> Rgba {
        Rgba {
            r: color.r * 0.66,
            g: color.g * 0.66,
            b: color.b * 0.66,
            a: color.a,
        }
    }

    /// 默认背景算暗还是亮。喂给 DECSET 2031 的 `CSI ? 997;N n` 通知
    /// (`Term::set_color_scheme`)。
    ///
    /// 判据取**当前背景色**而不是主题名:程序可以用 OSC 11 改默认底色,
    /// 而 CLI 判断亮暗靠的就是 OSC 11 问到的这个值(`query_reply` 的同一个
    /// 字段),两条渠道必须给同一个答案,否则 app 会在「你说是亮的、我量
    /// 出来是暗的」之间反复横跳。
    ///
    /// 阈值与公式下沉到 core `term::background_is_dark`,与引擎共用同一判据。
    pub fn is_dark(&self) -> bool {
        let to_u8 = |c: f32| (c.clamp(0.0, 1.0) * 255.0).round() as u8;
        let bg = self.background;
        slterm_terminal::term::background_is_dark(to_u8(bg.r), to_u8(bg.g), to_u8(bg.b))
    }

    fn indexed_default(&self, index: u8) -> Rgba {
        if index >= 16
            && let Some((_, color)) = self.indexed.iter().find(|(i, _)| *i == index)
        {
            return *color;
        }
        match index {
            0..=15 => self.ansi[index as usize],
            16..=231 => {
                let idx = index as u16 - 16;
                let comp = |c: u16| -> u8 { if c == 0 { 0 } else { (c * 40 + 55) as u8 } };
                rgb8(comp(idx / 36), comp(idx / 6 % 6), comp(idx % 6))
            }
            232..=255 => {
                let v = 8 + 10 * (index - 232);
                rgb8(v, v, v)
            }
        }
    }

    fn named_default(&self, named: NamedColor) -> Rgba {
        use NamedColor::{
            Background, BrightForeground, Cursor, DimBlack, DimBlue, DimCyan, DimForeground,
            DimGreen, DimMagenta, DimRed, DimWhite, DimYellow, Foreground,
        };
        match named {
            Foreground => self.foreground,
            BrightForeground => self.bright_foreground,
            DimForeground => self.dim_foreground,
            Background => self.background,
            Cursor => self.cursor,
            DimBlack | DimRed | DimGreen | DimYellow | DimBlue | DimMagenta | DimCyan
            | DimWhite => self.dim[named as usize - NamedColor::DimBlack as usize],
            // 0-15 直接落在 ansi 表内。
            other => self.ansi[(other as usize).min(15)],
        }
    }

    /// 解析单元格颜色。`bold` 让 0-7 号命名色提亮为 8-15(经典终端行为)。
    pub fn resolve(&self, color: Color, overrides: &Colors, bold: bool) -> Rgba {
        match color {
            Color::Spec(rgb) => from_ansi_rgb(rgb),
            Color::Indexed(index) => {
                let index = if bold && index < 8 { index + 8 } else { index };
                overrides[index as usize]
                    .map(from_ansi_rgb)
                    .unwrap_or_else(|| self.indexed_default(index))
            }
            Color::Named(named) => {
                let named = if bold && (named as usize) < 8 {
                    // 0-7 与 8-15 一一对应。
                    match named {
                        NamedColor::Black => NamedColor::BrightBlack,
                        NamedColor::Red => NamedColor::BrightRed,
                        NamedColor::Green => NamedColor::BrightGreen,
                        NamedColor::Yellow => NamedColor::BrightYellow,
                        NamedColor::Blue => NamedColor::BrightBlue,
                        NamedColor::Magenta => NamedColor::BrightMagenta,
                        NamedColor::Cyan => NamedColor::BrightCyan,
                        NamedColor::White => NamedColor::BrightWhite,
                        other => other,
                    }
                } else {
                    named
                };
                overrides[named]
                    .map(from_ansi_rgb)
                    .unwrap_or_else(|| self.named_default(named))
            }
        }
    }

    /// OSC 4/10/11 颜色查询(`Event::ColorRequest`)的回答值。
    pub fn query_reply(&self, index: usize, overrides: &Colors) -> Rgb {
        if let Some(rgb) = overrides[index] {
            return rgb;
        }
        let rgba = match index {
            0..=255 => self.indexed_default(index as u8),
            _ => match index_to_named(index) {
                Some(named) => self.named_default(named),
                None => self.foreground,
            },
        };
        Rgb {
            r: (rgba.r * 255.0) as u8,
            g: (rgba.g * 255.0) as u8,
            b: (rgba.b * 255.0) as u8,
        }
    }
}

pub fn from_ansi_rgb(rgb: Rgb) -> Rgba {
    rgb8(rgb.r, rgb.g, rgb.b)
}

fn index_to_named(index: usize) -> Option<NamedColor> {
    use NamedColor::{
        Background, BrightForeground, Cursor, DimBlack, DimBlue, DimCyan, DimForeground, DimGreen,
        DimMagenta, DimRed, DimWhite, DimYellow, Foreground,
    };
    // Colors 的 256..268 段与 NamedColor 的显式判别值对齐。
    [
        Foreground,
        Background,
        Cursor,
        DimBlack,
        DimRed,
        DimGreen,
        DimYellow,
        DimBlue,
        DimMagenta,
        DimCyan,
        DimWhite,
        BrightForeground,
        DimForeground,
    ]
    .into_iter()
    .find(|n| *n as usize == index)
}

#[cfg(test)]
mod colors_tests {
    use super::*;

    /// 暗色默认即契约(仅暗色定位):钉住关键字段,防 palette 编辑漂移。
    #[test]
    fn default_palette_is_the_anchored_dark_theme() {
        let palette = Palette::default();
        let to_u8 = |c: f32| (c * 255.0).round() as u8;
        assert_eq!(
            (
                to_u8(palette.background.r),
                to_u8(palette.background.g),
                to_u8(palette.background.b)
            ),
            (0x08, 0x0a, 0x18)
        );
        assert_eq!(
            (
                to_u8(palette.foreground.r),
                to_u8(palette.foreground.g),
                to_u8(palette.foreground.b)
            ),
            (0xd6, 0xda, 0xea)
        );
        assert_eq!(palette.ansi.len(), 16);
        assert_eq!(palette.dim.len(), 8);
        assert!(palette.indexed.is_empty());
        // dim 前景 = 前景 × 0.66。
        let dim = palette.dim_foreground;
        assert_eq!(to_u8(dim.r), to_u8(palette.foreground.r * 0.66));
    }

    /// bold 提亮:0-7 命名色在 bold 下解析为对应 8-15 亮色(经典终端行为)。
    #[test]
    fn resolve_brightens_named_colors_under_bold() {
        let palette = Palette::default();
        let overrides = Colors::default();
        let plain = palette.resolve(Color::Named(NamedColor::Red), &overrides, false);
        let bold = palette.resolve(Color::Named(NamedColor::Red), &overrides, true);
        assert_eq!(plain, palette.ansi[1]);
        assert_eq!(bold, palette.ansi[9]);
    }

    /// indexed 缺省:16-231 走 6×6×6 立方体公式,232-255 走灰阶。
    #[test]
    fn resolve_indexed_falls_back_to_cube_and_grayscale() {
        let palette = Palette::default();
        let overrides = Colors::default();
        let to_u8 = |c: f32| (c * 255.0).round() as u8;
        let cube = palette.resolve(Color::Indexed(21), &overrides, false);
        assert_eq!((to_u8(cube.r), to_u8(cube.g), to_u8(cube.b)), (0, 0, 255));
        let gray = palette.resolve(Color::Indexed(232), &overrides, false);
        assert_eq!((to_u8(gray.r), to_u8(gray.g), to_u8(gray.b)), (8, 8, 8));
    }

    /// OSC 查询回答:运行时覆盖优先,其次 palette 缺省。
    #[test]
    fn query_reply_prefers_runtime_override() {
        let palette = Palette::default();
        let mut overrides = Colors::default();
        overrides[1] = Some(Rgb { r: 1, g: 2, b: 3 });
        assert_eq!(palette.query_reply(1, &overrides), Rgb { r: 1, g: 2, b: 3 });
        // 未覆盖的命名槽(256 = Foreground)回落 palette 前景。
        let fg = palette.query_reply(256, &overrides);
        assert_eq!((fg.r, fg.g, fg.b), (0xd6, 0xda, 0xea));
    }
}
