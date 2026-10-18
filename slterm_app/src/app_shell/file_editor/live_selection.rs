//! 行内源码显影跟随光标(07 篇 M8.3 照抄点 8)。
//!
//! 光标进入/离开带格式文本的边界时,只切换投影的显影范围并重设输入文本与
//! 装饰;这不是文档编辑,不产生撤销项,也不替换输入实体。

use super::inline_edit::Projection;
use super::*;
use gpui::EntityInputHandler as _;

impl TextFileView {
    pub(super) fn sync_live_selection(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let Some(edit) = self.live_edit.as_mut() else {
            return;
        };
        let input = &edit.input;
        let selection = input.read(cx).selected_range();
        if selection == edit.last_selection {
            return;
        }
        // 区间选择(含拖拽中)拥有自己的几何:指针下不得改文本或方向。
        if !selection.is_empty()
            || !edit.projection.rich
            || input.update(cx, |input, cx| {
                input.marked_text_range(window, cx).is_some()
            })
        {
            edit.last_selection = selection;
            cx.notify();
            return;
        }
        let reveal = edit.projection.reveal_at(selection.start);
        edit.last_selection = selection.clone();
        if reveal == edit.projection.revealed() {
            return;
        }
        let source_cursor = edit.projection.source_offset(selection.start);
        let projection = Projection::with_reveal(&edit.source, reveal);
        let cursor = projection.visible_offset(source_cursor);
        // 至多重写当前有界块;set_value 抑制 Change 事件,规范源码与共享
        // 历史不变。
        input.update(cx, |input, cx| {
            input.set_value(projection.text.clone(), window, cx);
            input.set_selected_range(cursor..cursor, cx);
        });
        edit.last_selection = cursor..cursor;
        edit.decorations
            .set(super::live_edit::decorations(&projection, cx), cx);
        edit.projection = projection;
        let block = edit.block;
        self.invalidate_live_block(block);
        cx.notify();
    }
}
