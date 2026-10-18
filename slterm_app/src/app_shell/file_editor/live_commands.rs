//! 文档级编辑命令桥(07 篇 M8.3):格式化输入 ↔ 规范源码缓冲区。
//!
//! 所有程序化替换都走 `input.replace`(组件语义为静默:不触发 Change 订阅),
//! 因此这里显式维护共享历史、脏标记与预览失效;撤销统一由
//! `travel_history` 驱动(Undo/Redo 动作在 mod.rs 捕获,内建栈被拦截)。

use super::inline_edit::changed_span;
use super::*;

impl TextFileView {
    pub(super) fn format_live_selection(
        &mut self,
        marker: &str,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(edit) = &self.live_edit else {
            return;
        };
        let selection = edit.input.read(cx).selected_range();
        let Some(formatted) = edit.projection.toggle_mark(selection.clone(), marker) else {
            return;
        };
        let block = edit.block;
        let cursor = edit.range.start;
        let mut source = self.input.read(cx).value().to_string();
        self.history.record(&source);
        source.replace_range(edit.range.clone(), &formatted);
        self.finish_live_edit(cx);
        self.replace_document_text(&source, window, cx);
        self.history.record(&source);
        self.apply_outline(Outline::parse(&source), cx);
        self.resume_live_at(block, cursor, window, cx);
        if let Some(edit) = &self.live_edit {
            edit.input
                .update(cx, |input, cx| input.set_selected_range(selection, cx));
        }
    }

    /// 统一撤销/重做:源码态与渲染态同栈。调用方(动作捕获)负责阻止
    /// 输入实体内建栈继续处理。
    pub(super) fn travel_history(
        &mut self,
        redo: bool,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if (self.preview && !self.preview_editable())
            || self.loading
            || self.document.as_ref().is_none_or(|doc| doc.read_only)
        {
            return;
        }
        self.history.record(&self.input.read(cx).value());
        let Some((text, selection)) = self.history.travel(redo) else {
            return;
        };
        let editing = self.live_edit.is_some();
        self.finish_live_edit(cx);
        self.replace_document_text(&text, window, cx);
        self.input.update(cx, |input, cx| {
            input.set_selected_range(selection.clone(), cx)
        });
        // 非 markdown 源码 tab 没有大纲/渲染态:只回写文本与光标。
        if !self.markdown {
            cx.notify();
            return;
        }
        let mut outline = Outline::parse(&text);
        let block = editing.then(|| outline.edit_block_at(selection.start));
        self.apply_outline(outline, cx);
        if let Some(block) = block {
            self.resume_live_at(block, selection.start, window, cx);
        } else {
            self.schedule_preview(cx);
        }
    }

    pub(super) fn split_live_paragraph(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> bool {
        let Some(edit) = &self.live_edit else {
            return false;
        };
        let inline_block = self
            .outline
            .structures
            .get(edit.block)
            .and_then(Option::as_ref)
            .is_some_and(|structure| {
                matches!(
                    structure.root,
                    super::block_structure::StructureNode::Inline(_)
                )
            });
        if !edit.projection.rich || (edit.part.is_some() && !inline_block) {
            return false;
        }
        let selection = edit.input.read(cx).selected_range();
        let (range, left, right) = if inline_block {
            let range = self.outline.source_ranges[edit.block].clone();
            let Some(source) = self.source_slice(range.clone(), cx) else {
                return false;
            };
            let offset = edit.range.start - range.start;
            let reveal = edit
                .projection
                .revealed()
                .map(|span| offset + span.start..offset + span.end);
            let projection = super::inline_edit::Projection::with_reveal(&source, reveal);
            let start =
                projection.visible_offset(offset + edit.projection.source_offset(selection.start));
            let end =
                projection.visible_offset(offset + edit.projection.source_offset(selection.end));
            let left = projection.replace(&projection.text[..start]);
            let right = paragraph_source(&projection.replace(&projection.text[end..]));
            (range, left, right)
        } else {
            let left = edit
                .projection
                .replace(&edit.projection.text[..selection.start]);
            let right = paragraph_source(
                &edit
                    .projection
                    .replace(&edit.projection.text[selection.end..]),
            );
            (edit.range.clone(), left, right)
        };
        let cursor = range.start + left.len() + 2;
        let mut source = self.input.read(cx).value().to_string();
        self.history.record(&source);
        source.replace_range(range, &format!("{left}\n\n{right}"));
        self.finish_live_edit(cx);
        self.replace_document_text(&source, window, cx);
        self.history.record(&source);
        let mut outline = Outline::parse(&source);
        let block = outline.edit_block_at(cursor);
        self.apply_outline(outline, cx);
        self.begin_live_edit(block, window, cx);
        if let Some(edit) = &self.live_edit {
            edit.input
                .update(cx, |input, cx| input.set_selected_range(0..0, cx));
        }
        self.scroll.scroll_to_reveal_item(block);
        true
    }

    /// 程序化替换规范文本:静默输入更新 + 脏标记/预览失效。
    ///
    /// 历史由调用方在事务边界记录(结构提交/撤销各一次),避免重复入栈。
    pub(super) fn replace_document_text(
        &mut self,
        text: &str,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let source = self.input.read(cx).value();
        if source.as_ref() == text {
            return;
        }
        let (removed, inserted) = changed_span(&source, text);
        self.suppress_history = true;
        self.input.update(cx, |input, cx| {
            input.set_selected_range(removed, cx);
            input.replace(text[inserted].to_owned(), window, cx);
        });
        self.suppress_history = false;
        self.content_revision = self.content_revision.wrapping_add(1);
        self.dirty = self.saved_text.as_deref() != Some(text);
        if self.markdown {
            self.preview_stale = true;
        }
        cx.notify();
    }
}

/// 标题段落只保留下标文本 span:块源码里不含 `#` 标记。
pub(super) fn paragraph_source(source: &str) -> String {
    if let Ok(markdown::mdast::Node::Root(root)) =
        markdown::to_mdast(source, &markdown::ParseOptions::gfm())
        && let Some(markdown::mdast::Node::Heading(heading)) = root.children.first()
        && let (Some(first), Some(last)) = (
            heading
                .children
                .first()
                .and_then(markdown::mdast::Node::position),
            heading
                .children
                .last()
                .and_then(markdown::mdast::Node::position),
        )
    {
        return source[first.start.offset..last.end.offset].to_owned();
    }
    source.to_owned()
}

#[cfg(test)]
mod live_commands_tests {
    use super::paragraph_source;

    /// 标题源码剥离标记;普通段落原样。
    #[test]
    fn heading_sources_drop_their_markers_only() {
        assert_eq!(paragraph_source("## Title **bold**"), "Title **bold**");
        assert_eq!(paragraph_source("plain text"), "plain text");
    }
}
