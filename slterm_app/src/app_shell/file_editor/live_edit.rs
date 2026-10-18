//! 聚焦块 WYSIWYG 输入(照上游同构)。
//!
//! 聚焦块换稳定原生输入,行内格式经 `Projection` 从源码位置投影为装饰;
//! 格式标记留在源码里。IME 组合更新绝不替换输入实体或选区;每次有效变更
//! 写回规范源码(文档事务),因此草稿/撤销/保存与源码态同栈。

use super::block_structure::PartKind;
use super::inline_edit::{Projection, changed_span};
use super::*;
use gpui::EntityInputHandler as _;
use gpui::Styled as _;
use gpui_component::input::{TextDecoration, TextDecorationCollection};
use std::ops::Range;

pub(super) struct LiveEdit {
    pub(super) block: usize,
    pub(super) input: Entity<InputState>,
    pub(super) range: Range<usize>,
    pub(super) source: String,
    pub(super) projection: Projection,
    pub(super) part: Option<usize>,
    pub(super) kind: PartKind,
    pub(super) last_selection: Range<usize>,
    changed: bool,
    heading: Option<u8>,
    pending_click: Option<PendingClick>,
    pub(super) decorations: TextDecorationCollection,
    _subscription: Subscription,
    _observation: Subscription,
}

struct PendingClick {
    position: Point<Pixels>,
    text: SharedString,
    selection: Range<usize>,
}

pub(super) fn decorations(projection: &Projection, cx: &App) -> Vec<TextDecoration> {
    projection
        .marks()
        .map(|(range, marks)| {
            TextDecoration::new(
                range,
                gpui::HighlightStyle {
                    font_weight: marks.bold.then_some(gpui::FontWeight::BOLD),
                    font_style: marks.italic.then_some(gpui::FontStyle::Italic),
                    color: marks.link.then_some(cx.theme().link),
                    underline: marks.link.then_some(gpui::UnderlineStyle {
                        thickness: px(1.0),
                        ..Default::default()
                    }),
                    strikethrough: marks.strike.then_some(gpui::StrikethroughStyle {
                        thickness: px(1.0),
                        ..Default::default()
                    }),
                    background_color: marks.code.then_some(Hsla::from(cx.theme().tokens.muted)),
                    ..Default::default()
                },
            )
        })
        .collect()
}

impl TextFileView {
    pub(super) fn preview_editable(&self) -> bool {
        self.live_mode
            && self.render_active
            && self.preview
            && !self.loading
            && !self.outline.limited
            && self
                .document
                .as_ref()
                .is_some_and(|document| !document.read_only)
    }

    pub(super) fn document_input_focused(&self, window: &Window, cx: &App) -> bool {
        self.focus.is_focused(window)
            || (!self.preview && self.input.read(cx).focus_handle(cx).is_focused(window))
            || self
                .live_edit
                .as_ref()
                .is_some_and(|edit| edit.input.read(cx).focus_handle(cx).is_focused(window))
    }

    pub(super) fn begin_live_edit(
        &mut self,
        block: usize,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.begin_live_edit_at(block, None, window, cx);
    }

    pub(super) fn begin_live_edit_at(
        &mut self,
        block: usize,
        click: Option<&gpui::ClickEvent>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let part = self
            .outline
            .structures
            .get(block)
            .and_then(Option::as_ref)
            .filter(|structure| !structure.parts.is_empty())
            .map(|_| 0);
        self.begin_live_part_at(block, part, click, window, cx);
    }

    pub(super) fn begin_live_part_at(
        &mut self,
        block: usize,
        part: Option<usize>,
        click: Option<&gpui::ClickEvent>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if let Some(gpui::ClickEvent::Mouse(click)) = click {
            let movement = click.up.position - click.down.position;
            if f32::from(movement.x).abs() + f32::from(movement.y).abs() > 4.0 {
                // 完成的阅读器拖拽属于共享选区协调器;此处开输入会丢掉选区。
                return;
            }
        }
        if !self.preview_editable() {
            return;
        }
        if self
            .live_edit
            .as_ref()
            .is_some_and(|edit| edit.block == block && edit.part == part)
        {
            return;
        }
        self.finish_live_edit(cx);
        self.resume_edit = false;
        let block_range = if self.input.read(cx).text().len() == 0 {
            0..0
        } else {
            let Some(range) = self.outline.source_ranges.get(block).cloned() else {
                return;
            };
            range
        };
        // 激活开销与阅读器单块预算一致:超限块退回源码态 + 光标定位。
        if block_range.len() > 32 * 1024 {
            self.notice = Some(EditorNotice::PreviewLimited);
            self.toggle_preview(window, cx);
            self.input.update(cx, |input, cx| {
                input.set_selected_range(block_range.start..block_range.start, cx)
            });
            return;
        }
        let target = part.and_then(|part| {
            self.outline
                .structures
                .get(block)?
                .as_ref()?
                .parts
                .get(part)
        });
        let range = target.map_or(block_range.clone(), |part| {
            block_range.start + part.range.start..block_range.start + part.range.end
        });
        let kind = target.map_or(PartKind::Rich, |part| part.kind.clone());
        let Some(source) = self.source_slice(range.clone(), cx) else {
            return;
        };
        let projection = if matches!(kind, PartKind::Rich) {
            Projection::new(&source)
        } else {
            Projection::raw(&source)
        };
        let heading = target.map_or_else(|| self.outline.heading_level(block), |part| part.heading);
        let style = decorations(&projection, cx);
        let (input, collection) = {
            let input = cx.new(|cx| {
                let mut input = InputState::new(window, cx).auto_grow(1, 40).soft_wrap(true);
                if let PartKind::Code(language) = &kind {
                    input = input
                        .code_editor(language.clone())
                        .line_number(false)
                        .indent_guides(false)
                        .soft_wrap(false);
                }
                input.set_value(projection.text.clone(), window, cx);
                input.set_selected_range(projection.text.len()..projection.text.len(), cx);
                input
            });
            let collection = input.update(cx, |input, cx| {
                input.create_decorations_collection(style, cx)
            });
            (input, collection)
        };
        let subscription = cx.subscribe_in(&input, window, |view, input, event, window, cx| {
            if !view
                .live_edit
                .as_ref()
                .is_some_and(|edit| edit.input == *input)
            {
                return;
            }
            match event {
                InputEvent::Change => {
                    view.update_live_edit(window, cx);
                    view.apply_input_rule(window, cx);
                    view.sync_live_selection(window, cx);
                }
                InputEvent::Blur => {
                    view.update_live_edit(window, cx);
                    view.finish_live_edit(cx);
                }
                _ => {}
            }
        });
        let observation = cx.observe_in(&input, window, |view, input, window, cx| {
            if view
                .live_edit
                .as_ref()
                .is_some_and(|edit| edit.input == input)
            {
                view.update_live_edit(window, cx);
                view.sync_live_selection(window, cx);
            }
        });
        self.history.barrier();
        self.live_edit = Some(LiveEdit {
            block,
            input: input.clone(),
            range,
            source,
            last_selection: projection.text.len()..projection.text.len(),
            projection,
            part,
            kind,
            changed: false,
            heading,
            pending_click: click.map(|click| PendingClick {
                position: click.position(),
                text: input.read(cx).value(),
                selection: input.read(cx).selected_range(),
            }),
            decorations: collection,
            _subscription: subscription,
            _observation: observation,
        });
        self.preview_task = None;
        self.stop_preview_selection_scroll();
        self.invalidate_live_block(block);
        input.update(cx, |input, cx| input.focus(window, cx));
        cx.notify();
    }

    fn finish_live_click(
        &mut self,
        input: &Entity<InputState>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(edit) = self.live_edit.as_mut().filter(|edit| edit.input == *input) else {
            return;
        };
        let Some(click) = edit.pending_click.take() else {
            return;
        };
        let consumed = input.update(cx, |input, cx| {
            // 布局前到达的输入/选区/IME 拥有光标,延后的首次点击不得覆盖。
            if edit.changed
                || input.text().slice(..) != click.text.as_ref()
                || input.selected_range() != click.selection
                || input.marked_text_range(window, cx).is_some()
                || !input.focus_handle(cx).is_focused(window)
            {
                return true;
            }
            let Some(offset) = input.offset_for_point(click.position) else {
                return false;
            };
            input.set_selected_range(offset..offset, cx);
            true
        });
        if !consumed {
            edit.pending_click = Some(click);
        }
    }

    pub(super) fn update_live_edit(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let Some(edit) = self.live_edit.as_mut() else {
            return;
        };
        // 焦点/选区/光标闪烁都会 notify 同一实体:先比 Rope,避免物化正文。
        if edit.input.read(cx).text().slice(..) == edit.projection.text.as_str() {
            return;
        }
        let text = edit.input.read(cx).value();
        let source = self.input.read(cx).text();
        if !source
            .try_slice(edit.range.clone())
            .is_ok_and(|slice| slice == edit.source.as_str())
        {
            self.finish_live_edit(cx);
            return;
        }
        let mut next = edit.projection.replace(&text);
        if edit.part.is_some_and(|part| {
            self.outline.structures[edit.block]
                .as_ref()
                .is_some_and(|structure| structure.root.table(part).is_some())
        }) {
            next = super::structure_commands::table_cell_source(&next);
        }
        let (removed, inserted) = changed_span(&edit.source, &next);
        let range = edit.range.start + removed.start..edit.range.start + removed.end;
        let close_on_same_line = edit.source.is_empty()
            && !next.is_empty()
            && matches!(edit.kind, PartKind::Code(_) | PartKind::Math)
            && source.try_slice(edit.range.end..).is_ok_and(|tail| {
                let prefix: String = tail.chars().take(3).collect();
                ["```", "~~~", "$$"]
                    .iter()
                    .any(|fence| prefix.starts_with(fence))
            });
        let mut replacement = next[inserted].to_owned();
        if close_on_same_line {
            replacement.push('\n');
        }
        let delta = replacement.len() as isize - range.len() as isize;
        self.input.update(cx, |input, cx| {
            input.set_selected_range(range, cx);
            input.replace(replacement, window, cx);
        });
        edit.range.end = edit.range.start + next.len();
        edit.changed = true;
        self.preview_stale = true;
        // `InputState::replace` 是静默更新(不触发 Change 订阅):共享历史与
        // 脏标记在这里显式维护,渲染态键入才与源码态同栈。
        let rope = self.input.read(cx).text().clone();
        self.history.record_rope(&rope);
        self.content_revision = self.content_revision.wrapping_add(1);
        self.dirty = self.saved_text.as_deref() != Some(self.input.read(cx).value().as_ref());
        let valid = edit.projection.accept(&text, &next);
        edit.source = next;
        edit.decorations.set(decorations(&edit.projection, cx), cx);
        let block = edit.block;
        if let Some(part) = edit.part
            && let Some(structure) = self.outline.structures[block].as_mut()
        {
            let structure = std::sync::Arc::make_mut(structure);
            structure.replace_part(part, edit.source.len() + usize::from(close_on_same_line));
            structure.parts[part].range.end -= usize::from(close_on_same_line);
        }
        for (index, span) in self.outline.source_ranges.iter_mut().enumerate() {
            if index > block {
                span.start = span.start.saturating_add_signed(delta);
            }
            if index >= block {
                span.end = span.end.saturating_add_signed(delta);
            }
        }
        self.invalidate_live_block(block);
        if !valid {
            self.finish_live_edit(cx);
        }
        cx.notify();
    }

    pub(super) fn invalidate_live_block(&self, block: usize) {
        let top = self.scroll.logical_scroll_top();
        if self.outline.blocks.is_empty() {
            self.scroll.reset(1);
        } else {
            self.scroll.splice(block..block + 1, 1);
        }
        self.scroll.scroll_to(top);
    }

    pub(super) fn finish_live_edit(&mut self, cx: &mut Context<'_, Self>) {
        if let Some(edit) = self.live_edit.take() {
            self.last_edit_cursor = Some(super::activity::EditCursor {
                anchor: edit.range.start,
                selection: {
                    let range = edit.input.read(cx).selected_range();
                    edit.projection.source_offset(range.start)
                        ..edit.projection.source_offset(range.end)
                },
            });
            // 后台重排完成前,刚编辑的文本必须保持可见。
            if edit.changed
                && let Some(range) = self.outline.source_ranges.get(edit.block)
                && let Some(source) = self.source_slice(range.clone(), cx)
                && let Some(block) = self.outline.blocks.get_mut(edit.block)
            {
                *block = source;
            }
            if let Some(slot) = self.blocks.borrow_mut().get_mut(edit.block) {
                *slot = None;
            }
            self.invalidate_live_block(edit.block);
            self.history.barrier();
            if self.preview_stale {
                self.schedule_preview(cx);
            }
            cx.notify();
        }
    }

    pub(super) fn render_live_block(
        &self,
        block: usize,
        cx: &mut Context<'_, Self>,
    ) -> Option<gpui::AnyElement> {
        let edit = self.live_edit.as_ref().filter(|edit| edit.block == block)?;
        if edit.part.is_some() {
            return None;
        }
        Some(self.render_live_input(cx))
    }

    pub(super) fn render_live_input(&self, cx: &mut Context<'_, Self>) -> gpui::AnyElement {
        let edit = self.live_edit.as_ref().expect("active input");
        let code = matches!(edit.kind, PartKind::Code(_));
        let math = matches!(edit.kind, PartKind::Math | PartKind::Source);
        div()
            .id("markdown-live-block")
            .debug_selector(|| "markdown-live-block".to_owned())
            .key_context("MarkdownLive")
            .relative()
            .w_full()
            .min_w_0()
            .when(edit.heading.is_some() && edit.part.is_none(), |block| {
                block.pb(gpui::rems(0.3))
            })
            .child(
                Input::new(&edit.input)
                    .appearance(false)
                    .bordered(false)
                    .focus_bordered(false)
                    .rounded(px(0.0))
                    .w_full()
                    .px_0()
                    .py_0()
                    .font_family(cx.theme().font_family.clone())
                    .text_size(px(reader_presentation::heading_size(edit.heading)))
                    .line_height(gpui::relative(reader_presentation::LINE_HEIGHT))
                    .when(edit.heading.is_some(), |input| {
                        input.font_weight(reader_presentation::heading_weight(edit.heading))
                    })
                    .when(code || math, |input| {
                        input
                            .font_family(cx.theme().mono_font_family.clone())
                            .text_size(px(13.0))
                            .line_height(gpui::relative(1.8))
                    })
                    .when(code, |input| {
                        input.h(px(
                            edit.projection.text.lines().count().clamp(1, 40) as f32 * 23.4
                        ))
                    }),
            )
            .when(edit.pending_click.is_some(), |block| {
                let owner = cx.weak_entity();
                let input = edit.input.clone();
                block.child(
                    gpui::canvas(
                        |_, _, _| (),
                        move |_, _, window, cx| {
                            // Input 在 paint 阶段保存字形位置;下一帧回调早于
                            // 布局,不能用来交接点击。
                            window.defer(cx, move |window, cx| {
                                let _ = owner.update(cx, |view, cx| {
                                    view.finish_live_click(&input, window, cx);
                                });
                            });
                        },
                    )
                    .absolute()
                    .inset_0(),
                )
            })
            .into_any_element()
    }
}
