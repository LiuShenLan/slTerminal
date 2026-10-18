//! 相邻内联视图共享段落复制(照上游同构)。
//!
//! 内联 run 的弱句柄只在虚拟行挂载期间存活,与编辑历史无关;显式复制时
//! 把同一段落的选中 run 按源码空白还原成连续文本。

use super::*;
use gpui_component::WindowExt as _;
use std::collections::BTreeMap;

impl TextFileView {
    pub(super) fn inline_selected_text(&self, window: &mut Window, cx: &mut App) -> Option<String> {
        let mut groups: BTreeMap<usize, BTreeMap<usize, String>> = BTreeMap::new();
        for (&(block, part), state) in self.inline_views.borrow().iter() {
            let Some(state) = state.upgrade() else {
                continue;
            };
            let selected = state.read(cx).selected_text();
            if !selected.trim().is_empty() {
                groups.entry(block).or_default().insert(part, selected);
            }
        }
        if groups.is_empty() {
            return None;
        }
        let mut native = BTreeMap::new();
        let mut copied = BTreeMap::new();
        for (block, selected) in groups {
            let structure = self.outline.structures.get(block)?.as_ref()?;
            // 列表/表格/引用的外层分隔符有独立选区策略,交给常规文档渲染。
            if !matches!(
                structure.root,
                super::block_structure::StructureNode::Inline(_)
            ) {
                return None;
            }
            let first = *selected.first_key_value()?.0;
            let last = *selected.last_key_value()?.0;
            let range = self.outline.source_ranges.get(block)?;
            let source = self.source_slice(range.clone(), cx)?;
            let mut text = String::new();
            native.insert(
                block,
                selected.values().cloned().collect::<Vec<_>>().join("\n"),
            );
            for part in first..=last {
                let part_source = source.get(structure.parts.get(part)?.range.clone())?;
                let rendered = super::inline_edit::Projection::new(part_source).text;
                // TextView 序列化段落带尾随换行;那不是内联片段之间的边界,
                // 作者空白按源码在下方还原。
                let value = selected
                    .get(&part)
                    .map_or(rendered.as_str(), String::as_str)
                    .trim_end_matches(['\r', '\n']);
                let leading = part_source.len() - part_source.trim_start().len();
                let trailing = part_source.len() - part_source.trim_end().len();
                if (part > first || value == rendered)
                    && !value.starts_with(&part_source[..leading])
                {
                    text.push_str(&part_source[..leading]);
                }
                text.push_str(value);
                if (part < last || value == rendered)
                    && !value.ends_with(&part_source[part_source.len() - trailing..])
                {
                    text.push_str(&part_source[part_source.len() - trailing..]);
                }
            }
            copied.insert(block, text);
        }
        for (index, state) in self.blocks.borrow().iter().enumerate() {
            let Some(state) = state else { continue };
            let text = state.read(cx).selected_text();
            if !text.trim().is_empty() {
                native.insert(index, text.clone());
                copied.insert(index, text);
            }
        }
        // 选区里含不属于本组件的元素(如表格单元)时不替换;仅显式复制检查。
        let native = native.into_values().collect::<Vec<_>>().join("\n");
        let global = window.selected_text(cx);
        if !native.split_whitespace().eq(global.split_whitespace()) {
            return None;
        }
        Some(copied.into_values().collect::<Vec<_>>().join("\n"))
    }
}
