//! 终端调色板(M3.1 最小面:`Palette` 类型 + 暗色默认;`resolve`/`query_reply`/
//! `is_dark` 等 vte 解析面归 M3.2 随渲染元素随迁)。
//!
//! 默认值与上游 deep-space 暗色主题逐字段一致(D06-3:M3 只保一份内置暗色,
//! 仅暗色定位约束);主题矫正器(`TerminalColorResolver`)归 06 篇。

use gpui::Rgba;

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
#[allow(dead_code)] // 字段读取面随 M3.2 渲染元素(网格/光标/选区绘制)点亮。
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
}
