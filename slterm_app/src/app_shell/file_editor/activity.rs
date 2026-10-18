//! 渲染资产释放与草稿续接(07 篇 M8.3 照抄点 6)。
//!
//! 渲染属于可见 tab:切走时清块槽/内联视图/图片激活与滚动态;草稿、撤销
//! 历史、大纲与结构索引存活。切回时按需重排(脏预览先走 250ms 防抖),
//! 不改动未变更文档的解析结果。

use super::*;

/// 切走前记录的光标位置:块 + 选区源码 span(渲染态续接用)。
pub(super) struct EditCursor {
    pub(super) anchor: usize,
    pub(super) selection: std::ops::Range<usize>,
}

impl TextFileView {
    pub(in crate::app_shell) fn set_render_active(
        &mut self,
        active: bool,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.render_active == active {
            return;
        }
        self.render_active = active;
        if !active {
            self.update_live_edit(window, cx);
            self.finish_live_edit(cx);
            self.resume_edit = self.last_edit_cursor.is_some();
            self.preview_task = None;
            self.revision = self.revision.wrapping_add(1);
            self.inline_views.borrow_mut().clear();
            self.rendered_blocks.borrow_mut().clear();
            for block in self.blocks.borrow_mut().iter_mut() {
                *block = None;
            }
            self.preview_scrollbar_hovered = false;
        } else if self.preview_stale {
            self.schedule_preview(cx);
        }
        self.preview_images.update(cx, |images, cx| {
            images.set_active(active && self.preview, window, cx);
        });
        cx.notify();
    }

    /// 切回可见 tab 且重排完成后,把光标恢复到上次编辑的块与位置。
    pub(super) fn restore_edit_cursor(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        if !self.resume_edit || !self.render_active || self.preview_stale || self.loading {
            return;
        }
        self.resume_edit = false;
        let Some(cursor) = self.last_edit_cursor.take() else {
            return;
        };
        let block = self
            .outline
            .edit_block_at(cursor.anchor.min(self.input.read(cx).text().len()));
        self.resume_live_at(block, cursor.anchor, window, cx);
        if let Some(edit) = &self.live_edit {
            let start = edit.projection.visible_offset(cursor.selection.start);
            let end = edit.projection.visible_offset(cursor.selection.end);
            edit.input
                .update(cx, |input, cx| input.set_selected_range(start..end, cx));
        }
    }
}
