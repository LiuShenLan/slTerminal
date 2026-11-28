//! DocumentTextView 的元素树：正文画布、查找/跳转/补全栏、状态条与动作路由。

use gpui::{
    Context, InteractiveElement as _, MouseButton, MouseMoveEvent, MouseUpEvent,
    ParentElement as _, Render, ScrollWheelEvent, Styled as _, Window, div, px,
};
use gpui_component::{ActiveTheme as _, h_flex};

use super::DocumentTextEvent;
use super::DocumentTextView;
use super::{
    CloseEditorCompletion, CloseEditorGoto, CloseEditorSearch, CompleteEditor, FindEditor,
    FindNextEditor, FindPreviousEditor, GoToLineEditor, IndentEditor, OutdentEditor, SaveAsFile,
    SaveFile, SelectDown, SelectLeft, SelectRight, SelectUp, ToggleFoldEditor,
};
use gpui_component::input::{
    Backspace, Copy, Delete, Enter, MoveDown, MoveLeft, MoveRight, MoveToEndOfLine,
    MoveToStartOfLine, MoveUp, Paste, Redo, SelectAll, SelectToEndOfLine, SelectToStartOfLine,
    Undo,
};

impl Render for DocumentTextView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<'_, Self>) -> impl gpui::IntoElement {
        let canvas = self.text_canvas(cx);
        let language_selector = self.language_selector(cx);
        let wrap_button = self.wrap_button(cx);
        let line_ending_button = self.line_ending_button(cx);
        let read_only_button = self.read_only_button(cx);
        let save_as_button = self.save_as_button(cx);
        let system_open = self.system_open_button(cx);
        let git_markers = self.git_markers(cx);
        // 行标显隐属于瞬态投影：编辑域 gutter 宽度必须稳定，否则正文位移会触发换行重排。
        let git_gutter_width = if self.git_gutter { px(8.0) } else { px(0.0) };
        let focus = self.focus.clone();
        let search_bar = self.search_bar(cx);
        let goto_bar = self.goto_bar(cx);
        let completion_popup = self.completion_popup(cx);
        let status_bar = self.status_bar(cx);
        let editor = div()
            .key_context("DocumentText")
            .track_focus(&focus)
            .relative()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .overflow_hidden()
            .on_action(cx.listener(|view, _: &MoveLeft, _, cx| {
                view.move_horizontal(true, false, cx);
            }))
            .on_action(cx.listener(|view, _: &MoveRight, _, cx| {
                view.move_horizontal(false, false, cx);
            }))
            .on_action(cx.listener(|view, _: &MoveUp, _, cx| {
                view.move_vertical(false, false, cx);
            }))
            .on_action(cx.listener(|view, _: &MoveDown, _, cx| {
                view.move_vertical(true, false, cx);
            }))
            .on_action(cx.listener(|view, _: &SelectLeft, _, cx| {
                view.move_horizontal(true, true, cx);
            }))
            .on_action(cx.listener(|view, _: &SelectRight, _, cx| {
                view.move_horizontal(false, true, cx);
            }))
            .on_action(cx.listener(|view, _: &SelectUp, _, cx| {
                view.move_vertical(false, true, cx);
            }))
            .on_action(cx.listener(|view, _: &SelectDown, _, cx| {
                view.move_vertical(true, true, cx);
            }))
            .on_action(cx.listener(|view, _: &MoveToStartOfLine, _, cx| {
                view.move_line_boundary(false, false, cx);
            }))
            .on_action(cx.listener(|view, _: &MoveToEndOfLine, _, cx| {
                view.move_line_boundary(true, false, cx);
            }))
            .on_action(cx.listener(|view, _: &SelectToStartOfLine, _, cx| {
                view.move_line_boundary(false, true, cx);
            }))
            .on_action(cx.listener(|view, _: &SelectToEndOfLine, _, cx| {
                view.move_line_boundary(true, true, cx);
            }))
            .on_action(cx.listener(|view, _: &Backspace, _, cx| view.backspace(cx)))
            .on_action(cx.listener(|view, _: &Delete, _, cx| view.delete(cx)))
            .on_action(cx.listener(|view, _: &Enter, _, cx| view.insert_newline(cx)))
            .on_action(cx.listener(|view, _: &SelectAll, _, cx| view.select_all(cx)))
            .on_action(cx.listener(|view, _: &Undo, _, cx| view.travel_history(false, cx)))
            .on_action(cx.listener(|view, _: &Redo, _, cx| view.travel_history(true, cx)))
            .on_action(cx.listener(|view, _: &Copy, _, cx| view.copy(cx)))
            .on_action(cx.listener(|view, _: &Paste, _, cx| view.paste(cx)))
            .on_action(cx.listener(|_, _: &SaveFile, _, cx| {
                cx.emit(DocumentTextEvent::Save);
            }))
            .on_action(cx.listener(|_, _: &SaveAsFile, _, cx| {
                cx.emit(DocumentTextEvent::SaveAs);
            }))
            .on_action(cx.listener(|view, _: &GoToLineEditor, window, cx| {
                view.open_goto(window, cx);
            }))
            .on_action(cx.listener(|view, _: &ToggleFoldEditor, _, cx| {
                let line = view.logical_line_of_byte(view.selection.head);
                view.toggle_fold_at_line(line, cx);
            }))
            .on_action(cx.listener(|view, _: &CompleteEditor, _, cx| {
                view.trigger_completion(cx);
            }))
            .on_action(cx.listener(|view, _: &CloseEditorCompletion, _, cx| {
                view.close_completion(cx);
            }))
            .on_action(cx.listener(|view, _: &CloseEditorGoto, window, cx| {
                view.close_goto(window, cx);
            }))
            .on_action(cx.listener(|view, _: &IndentEditor, _, cx| {
                if !view.accept_completion(cx) {
                    view.indent(false, cx);
                }
            }))
            .on_action(cx.listener(|view, _: &OutdentEditor, _, cx| {
                view.indent(true, cx);
            }))
            .on_mouse_down(MouseButton::Left, cx.listener(DocumentTextView::mouse_down))
            .on_mouse_move(cx.listener(|view, event: &MouseMoveEvent, _, cx| {
                view.mouse_move(event, cx);
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|view, event: &MouseUpEvent, _, _| {
                    view.mouse_up(event);
                }),
            )
            .on_scroll_wheel(cx.listener(|view, event: &ScrollWheelEvent, _, cx| {
                view.scroll_wheel(event, cx);
            }))
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .overflow_hidden()
                    .child(div().size_full().pl(git_gutter_width).child(canvas))
                    .children(git_markers)
                    .children(completion_popup),
            )
            .child(
                h_flex()
                    .h(px(28.0))
                    .flex_shrink_0()
                    .items_center()
                    .justify_between()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .child(status_bar)
                    .child(
                        h_flex()
                            .items_center()
                            .flex_shrink_0()
                            .child(wrap_button)
                            .children(line_ending_button)
                            .children(read_only_button)
                            .children(save_as_button)
                            .children(language_selector),
                    ),
            )
            .children(
                system_open
                    .map(|button| div().absolute().top(px(8.0)).right(px(8.0)).child(button)),
            );
        div()
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .min_w_0()
            .min_h_0()
            .overflow_hidden()
            .on_action(cx.listener(|view, _: &FindEditor, window, cx| {
                view.open_search(window, cx);
            }))
            .on_action(cx.listener(|view, _: &FindNextEditor, _, cx| {
                view.move_search(true, cx);
            }))
            .on_action(cx.listener(|view, _: &FindPreviousEditor, _, cx| {
                view.move_search(false, cx);
            }))
            .on_action(cx.listener(|view, _: &CloseEditorSearch, window, cx| {
                view.close_search(window, cx);
            }))
            .children(search_bar)
            .children(goto_bar)
            .child(editor)
    }
}
