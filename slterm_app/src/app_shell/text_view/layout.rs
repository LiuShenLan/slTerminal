use std::collections::HashMap;

use gpui::{Bounds, Hsla, Pixels, ShapedLine, SharedString};
use slterm_document::{DocumentId, TextStateId};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FontMetricsKey {
    pub(crate) family: SharedString,
    pub(crate) font_size_bits: u32,
    pub(crate) cell_width_bits: u32,
    pub(crate) line_height_bits: u32,
}

/// 一行文本布局的完整失效键；宽度同时决定长行可视前缀。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TextLayoutKey {
    pub(crate) document_id: DocumentId,
    pub(crate) text_state: TextStateId,
    pub(crate) language_revision: u64,
    pub(crate) font: FontMetricsKey,
    pub(crate) wrap_width_bits: u32,
    pub(crate) first_visible_column: usize,
    pub(crate) dpi_bits: u32,
    pub(crate) theme_revision: u64,
}

#[derive(Clone, Copy)]
pub(crate) struct LayoutMetrics {
    pub(crate) bounds: Bounds<Pixels>,
    pub(crate) cell_width: Pixels,
    pub(crate) line_height: Pixels,
}

impl LayoutMetrics {
    pub(crate) fn visible_columns(&self) -> usize {
        (self.bounds.size.width / self.cell_width).ceil().max(1.0) as usize
    }

    pub(crate) fn visible_whole_columns(&self) -> usize {
        (self.bounds.size.width / self.cell_width).floor() as usize
    }
}

#[derive(Clone, PartialEq)]
pub(crate) struct ThemeFingerprint {
    pub(crate) foreground: Hsla,
    pub(crate) accent: Hsla,
    pub(crate) muted_foreground: Hsla,
    pub(crate) info_foreground: Hsla,
    pub(crate) success_foreground: Hsla,
    pub(crate) warning_foreground: Hsla,
    pub(crate) mono_font_family: SharedString,
    pub(crate) mono_font_size: Pixels,
}

#[derive(Default)]
pub(crate) struct LayoutCache {
    key: Option<TextLayoutKey>,
    pub(crate) lines: HashMap<usize, ShapedLine>,
    #[cfg(test)]
    hits: usize,
    #[cfg(test)]
    misses: usize,
}

impl LayoutCache {
    /// 返回是否沿用同一布局键；任一维度变化都清空旧行。
    pub(crate) fn activate(&mut self, key: TextLayoutKey) -> bool {
        if self.key.as_ref() == Some(&key) {
            return true;
        }
        self.lines.clear();
        self.key = Some(key);
        false
    }

    pub(crate) fn get(&mut self, row: usize) -> Option<ShapedLine> {
        let cached = self.lines.get(&row).cloned();
        #[cfg(test)]
        if cached.is_some() {
            self.hits += 1;
        } else {
            self.misses += 1;
        }
        cached
    }

    pub(crate) fn insert(&mut self, row: usize, line: ShapedLine) {
        self.lines.insert(row, line);
    }

    #[cfg(test)]
    pub(crate) fn stats(&self) -> (usize, usize) {
        (self.hits, self.misses)
    }
}

#[cfg(test)]
mod tests {
    use gpui::ShapedLine;

    use super::{FontMetricsKey, LayoutCache, TextLayoutKey};

    fn key() -> TextLayoutKey {
        TextLayoutKey {
            document_id: "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0f2a".parse().unwrap(),
            text_state: "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0f2b".parse().unwrap(),
            language_revision: 0,
            font: FontMetricsKey {
                family: "Mono".into(),
                font_size_bits: 14.0_f32.to_bits(),
                cell_width_bits: 8.0_f32.to_bits(),
                line_height_bits: 20.0_f32.to_bits(),
            },
            wrap_width_bits: 640.0_f32.to_bits(),
            first_visible_column: 0,
            dpi_bits: 1.0_f32.to_bits(),
            theme_revision: 0,
        }
    }

    #[test]
    fn cache_invalidates_for_every_layout_key_dimension() {
        let base = key();
        let other_document = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0f2c".parse().unwrap();
        let other_state = "018f2f7c-8f5a-7c3d-9e2b-4a6f8c1d0f2d".parse().unwrap();
        let variants = vec![
            TextLayoutKey {
                document_id: other_document,
                ..base.clone()
            },
            TextLayoutKey {
                text_state: other_state,
                ..base.clone()
            },
            TextLayoutKey {
                language_revision: 1,
                ..base.clone()
            },
            TextLayoutKey {
                font: FontMetricsKey {
                    family: "Other".into(),
                    ..base.font.clone()
                },
                ..base.clone()
            },
            TextLayoutKey {
                font: FontMetricsKey {
                    font_size_bits: 15.0_f32.to_bits(),
                    ..base.font.clone()
                },
                ..base.clone()
            },
            TextLayoutKey {
                font: FontMetricsKey {
                    cell_width_bits: 9.0_f32.to_bits(),
                    ..base.font.clone()
                },
                ..base.clone()
            },
            TextLayoutKey {
                font: FontMetricsKey {
                    line_height_bits: 21.0_f32.to_bits(),
                    ..base.font.clone()
                },
                ..base.clone()
            },
            TextLayoutKey {
                wrap_width_bits: 320.0_f32.to_bits(),
                ..base.clone()
            },
            TextLayoutKey {
                first_visible_column: 1,
                ..base.clone()
            },
            TextLayoutKey {
                dpi_bits: 2.0_f32.to_bits(),
                ..base.clone()
            },
            TextLayoutKey {
                theme_revision: 1,
                ..base.clone()
            },
        ];

        let mut cache = LayoutCache::default();
        cache.activate(base.clone());
        cache.lines.insert(0, ShapedLine::default());
        assert!(cache.activate(base.clone()));
        assert_eq!(cache.lines.len(), 1);
        for variant in variants {
            cache.activate(base.clone());
            cache.lines.insert(0, ShapedLine::default());
            assert!(!cache.activate(variant));
            assert!(cache.lines.is_empty());
        }
    }
}
