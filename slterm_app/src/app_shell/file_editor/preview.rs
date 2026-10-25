//! Markdown 渲染态与虚拟化(照上游同构)。
//!
//! 块清单来自 `Outline`(与大纲/源码 span 同一份 AST);`gpui::list` 只物化
//! 视口内与 overscan 行,屏外块槽置 `None`。每帧 prepaint 回收未挂载块的
//! `TextViewState` 与内联视图,避免长文档把逐块视图留在内存里。
//!
//! 图片:Markdown 图片段落经自定义块扩展渲染;本地文件缺失时画出占位框
//! (测试与离线文档的可感知形态),存在时交给 gpui 图片管线,缓存归
//! `DocumentImageCache`(虚拟行不得污染进程级资产图)。

use std::cell::RefCell;
use std::collections::HashSet;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, Bounds, IntoElement, MouseButton, ObjectFit, ParentElement as _, Pixels,
    StatefulInteractiveElement as _, Styled as _, StyledImage as _, Window, div, img, px,
};
use gpui_component::ActiveTheme as _;
use gpui_component::WindowExt as _;
use gpui_component::text::{
    MarkdownExtensions, MarkdownNode, TextView, TextViewState, TextViewStyle,
};

use super::*;

const SELECTION_SCROLL_INTERVAL: Duration = Duration::from_millis(16);

fn selection_scroll_delta(
    has_selection: bool,
    pointer_y: Pixels,
    bounds: Bounds<Pixels>,
) -> Option<Pixels> {
    if !has_selection || bounds.size.height <= px(0.0) {
        return None;
    }
    gpui_component::scroll::AutoScroll::compute_delta(pointer_y, bounds)
}

/// Markdown 图片段落(整段只有一张图)的自定义块数据。
#[derive(Clone, Debug)]
struct ImageBlock {
    url: String,
    alt: String,
    link: Option<String>,
    width: Option<f32>,
    height: Option<f32>,
}

fn image_block(node: &::markdown::mdast::Node) -> Option<ImageBlock> {
    let ::markdown::mdast::Node::Paragraph(paragraph) = node else {
        return None;
    };
    if paragraph.children.len() != 1 {
        return None;
    }
    let ::markdown::mdast::Node::Image(image) = &paragraph.children[0] else {
        return None;
    };
    Some(ImageBlock {
        url: image.url.clone(),
        alt: image.alt.clone(),
        link: None,
        width: None,
        height: None,
    })
}

fn is_remote(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://") || lower.starts_with("data:")
}

/// 图片块渲染:能解析到实体就交给 gpui;否则画占位(alt + 路径可读)。
fn render_image_block(
    data: &ImageBlock,
    base: Option<&std::path::Path>,
    cx: &mut gpui::App,
) -> AnyElement {
    let theme = cx.theme();
    let resolved: Option<PathBuf> = if is_remote(&data.url) {
        None
    } else {
        let path = std::path::Path::new(&data.url);
        Some(if path.is_absolute() {
            path.to_path_buf()
        } else if let Some(base) = base {
            base.join(path)
        } else {
            path.to_path_buf()
        })
    };
    let available = is_remote(&data.url) || resolved.as_ref().is_some_and(|path| path.is_file());
    if !available {
        return div()
            .debug_selector(|| "markdown-image-placeholder".to_owned())
            .w_full()
            .min_h(px(72.0))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_1()
            .rounded(px(6.0))
            .border_1()
            .border_color(theme.border)
            .bg(theme.muted)
            .text_size(px(reader_presentation::SECONDARY_SIZE))
            .text_color(theme.muted_foreground)
            .child(if data.alt.is_empty() {
                data.url.clone()
            } else {
                data.alt.clone()
            })
            .into_any_element();
    }
    let source = if is_remote(&data.url) {
        data.url.clone()
    } else {
        resolved.unwrap().to_string_lossy().into_owned()
    };
    let mut image = img(source)
        .debug_selector(|| "markdown-image".to_owned())
        .max_w_full()
        .object_fit(ObjectFit::Contain);
    if let Some(width) = data.width {
        image = image.w(px(width));
    }
    if let Some(height) = data.height {
        image = image.h(px(height));
    }
    if let Some(link) = data.link.clone() {
        image = image
            .cursor_pointer()
            .on_click(move |_, _, cx| cx.open_url(&link));
    }
    image.into_any_element()
}

/// 预览扩展:图片段落自定义块(占位/链接/尺寸)。
pub(super) fn extensions(base: Option<PathBuf>) -> MarkdownExtensions {
    let image_base = base.clone();
    MarkdownExtensions::default()
        .block_parser(|node, _| {
            let data = image_block(node)?;
            Some(MarkdownNode::new("slterm-markdown-image", data))
        })
        .block_renderer("slterm-markdown-image", move |node, _, cx| {
            let data = node.data::<ImageBlock>().expect("image block data");
            render_image_block(data, image_base.as_deref(), cx)
        })
}

fn block_frame(index: usize, heading: Option<u8>) -> gpui::Stateful<gpui::Div> {
    div()
        // 每个虚拟行独立身份作用域:结构化块内的单元格/列表项编号从 0 重来。
        .id(("markdown-document-block", index))
        .w_full()
        .max_w(px(reader_presentation::PAGE_WIDTH))
        .mx_auto()
        .min_w_0()
        .min_h(px(reader_presentation::BLOCK_MIN_HEIGHT))
        .pt(px(if index > 0 && heading.is_some() {
            20.0
        } else {
            4.0
        }))
        .pb(px(10.0))
        .debug_selector(move || format!("markdown-preview-block-{index}"))
        .text_size(px(reader_presentation::BODY_SIZE))
        .line_height(gpui::relative(reader_presentation::LINE_HEIGHT))
        .whitespace_normal()
}

impl TextFileView {
    fn start_preview_selection_scroll(&mut self, window: &Window, cx: &mut Context<'_, Self>) {
        if self.preview_selection_scroll_active {
            return;
        }
        self.preview_selection_scroll_active = true;
        self.preview_selection_scroll_epoch = self.preview_selection_scroll_epoch.wrapping_add(1);
        let epoch = self.preview_selection_scroll_epoch;
        let executor = cx.background_executor().clone();
        cx.spawn_in(window, async move |this, cx| {
            let mut refresh_selection = false;
            loop {
                executor.timer(SELECTION_SCROLL_INTERVAL).await;
                let tick = this.update_in(cx, |view, window, cx| {
                    view.preview_selection_scroll_tick(epoch, window, cx)
                });
                let Ok(Some(scrolled)) = tick else { break };
                // 上一拍列表已重排:重发当前指针位置,让窗口级选区端点跟上
                // 新可见块。
                if refresh_selection {
                    let _ = cx.update(|window, cx| {
                        let _ = window.dispatch_event(
                            gpui::PlatformInput::MouseMove(gpui::MouseMoveEvent {
                                position: window.mouse_position(),
                                pressed_button: Some(MouseButton::Left),
                                modifiers: window.modifiers(),
                            }),
                            cx,
                        );
                    });
                }
                refresh_selection = scrolled;
            }
        })
        .detach();
    }

    pub(super) fn stop_preview_selection_scroll(&mut self) {
        if !self.preview_selection_scroll_active {
            return;
        }
        self.preview_selection_scroll_active = false;
        self.preview_selection_scroll_epoch = self.preview_selection_scroll_epoch.wrapping_add(1);
    }

    fn preview_selection_scroll_tick(
        &mut self,
        epoch: u64,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Option<bool> {
        if epoch != self.preview_selection_scroll_epoch || !self.preview_selection_scroll_active {
            return None;
        }
        let bounds = self.scroll.viewport_bounds();
        let Some(delta) = selection_scroll_delta(
            window.has_text_selection(cx),
            window.mouse_position().y,
            bounds,
        ) else {
            return Some(false);
        };
        let before = self.scroll.scroll_px_offset_for_scrollbar();
        self.scroll.scroll_by(delta);
        let scrolled = self.scroll.scroll_px_offset_for_scrollbar() != before;
        if scrolled {
            cx.notify();
        }
        Some(scrolled)
    }

    pub(super) fn render_markdown_preview(&mut self, cx: &mut Context<'_, Self>) -> AnyElement {
        self.preview_images
            .update(cx, |images, _| images.begin_frame());
        self.rendered_blocks.borrow_mut().clear();
        let frame_images = self.preview_images.clone();
        let blocks = self.blocks.clone();
        let rendered_blocks = self.rendered_blocks.clone();
        let frame_blocks = Rc::new(RefCell::new(HashSet::new()));
        let mounted_blocks = frame_blocks.clone();
        let retained_blocks = self.blocks.clone();
        let inline_views = self.inline_views.clone();
        let owner = cx.entity().downgrade();
        let selection_owner = owner.clone();
        let extensions = self.preview_extensions.clone();
        let scroll = self.scroll.clone();
        let live_mode = self.live_mode;
        let style = self.preview_style(cx);
        div()
            .image_cache(self.preview_images.clone())
            .flex_1()
            .min_w_0()
            .h_full()
            .relative()
            .overflow_hidden()
            .px(px(reader_presentation::PAGE_MARGIN))
            .pt(px(reader_presentation::TOP_MARGIN))
            .debug_selector(|| "markdown-preview-viewport".to_owned())
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    this.start_preview_selection_scroll(window, cx);
                }),
            )
            .child(
                gpui::list(scroll.clone(), move |index, window, cx| {
                    rendered_blocks.borrow_mut().push(index);
                    mounted_blocks.borrow_mut().insert(index);
                    let heading = owner
                        .upgrade()
                        .and_then(|file| file.read(cx).outline.heading_level(index));
                    if let Some(element) = owner.upgrade().and_then(|file| {
                        file.update(cx, |file, cx| {
                            file.render_structured_block(index, window, cx)
                        })
                    }) {
                        let structured_owner = owner.clone();
                        return block_frame(index, heading)
                            .when(live_mode, |block| {
                                block.cursor_text().on_click(move |event, window, cx| {
                                    let _ = structured_owner.update(cx, |file, cx| {
                                        file.begin_live_edit_at(index, Some(event), window, cx);
                                    });
                                })
                            })
                            .child(element)
                            .into_any_element();
                    }
                    if let Some(element) = owner.upgrade().and_then(|file| {
                        file.update(cx, |file, cx| file.render_live_block(index, cx))
                    }) {
                        return block_frame(index, heading)
                            .child(element)
                            .into_any_element();
                    }
                    let cached = blocks.borrow().get(index).cloned().flatten();
                    let block = cached.unwrap_or_else(|| {
                        let (source, selected) = owner
                            .upgrade()
                            .map(|owner| {
                                let owner = owner.read(cx);
                                (owner.outline.block_source(index), owner.all_selected)
                            })
                            .unwrap_or_default();
                        let state = cx.new(|cx| TextViewState::markdown(&source, cx));
                        if selected {
                            state.update(cx, |state, cx| state.select_all(cx));
                        }
                        if let Some(slot) = blocks.borrow_mut().get_mut(index) {
                            *slot = Some(state.clone());
                        }
                        state
                    });
                    let edit_owner = owner.clone();
                    let link_owner = owner.clone();
                    let block_extensions = window
                        .use_keyed_state(("reader-code-extensions", index), cx, |_, _| {
                            extensions.clone()
                        })
                        .read(cx)
                        .clone();
                    block_frame(index, heading)
                        .when(live_mode, |block| {
                            block.cursor_text().on_click(move |event, window, cx| {
                                let _ = edit_owner.update(cx, |file, cx| {
                                    file.begin_live_edit_at(index, Some(event), window, cx);
                                });
                            })
                        })
                        .child(
                            TextView::new(&block)
                                .w_full()
                                .min_w_0()
                                .max_w_full()
                                .selectable(true)
                                .scrollable(false)
                                .style(style.clone())
                                .when(live_mode, |text| {
                                    text.on_link_click(move |url, event, window, cx| {
                                        if event.modifiers().control || event.modifiers().platform {
                                            cx.open_url(url);
                                        } else {
                                            let _ = link_owner.update(cx, |file, cx| {
                                                file.begin_live_edit_at(
                                                    index,
                                                    Some(event),
                                                    window,
                                                    cx,
                                                );
                                            });
                                        }
                                    })
                                })
                                .markdown_extensions(block_extensions),
                        )
                        .into_any_element()
                })
                .size_full()
                .max_w(px(reader_presentation::PAGE_WIDTH))
                .mx_auto(),
            )
            .child(
                gpui::canvas(
                    move |_, window, cx| {
                        frame_images.update(cx, |images, cx| images.finish_frame(window, cx));
                        // 虚拟列表当帧挂载完成后回收屏外块槽;活跃输入由文档
                        // 持有,不随块槽释放。
                        let mounted = std::mem::take(&mut *frame_blocks.borrow_mut());
                        inline_views
                            .borrow_mut()
                            .retain(|(block, _), _| mounted.contains(block));
                        if !mounted.is_empty() {
                            for (index, slot) in retained_blocks.borrow_mut().iter_mut().enumerate()
                            {
                                if !mounted.contains(&index) {
                                    *slot = None;
                                }
                            }
                        }
                    },
                    move |_, _, window, _| {
                        let owner = selection_owner.clone();
                        window.on_mouse_event(move |event: &gpui::MouseMoveEvent, phase, _, cx| {
                            if phase.capture() && event.pressed_button != Some(MouseButton::Left) {
                                let _ = owner
                                    .update(cx, |view, _| view.stop_preview_selection_scroll());
                            }
                        });
                        let owner = selection_owner;
                        window.on_mouse_event(move |event: &gpui::MouseUpEvent, phase, _, cx| {
                            if phase.capture() && event.button == MouseButton::Left {
                                let _ = owner
                                    .update(cx, |view, _| view.stop_preview_selection_scroll());
                            }
                        });
                    },
                )
                .absolute()
                .inset_0(),
            )
            .child(
                div()
                    .id("markdown-preview-scrollbar-host")
                    .absolute()
                    .top_0()
                    .right_0()
                    .bottom_0()
                    .w(px(16.0))
                    .debug_selector(|| "markdown-preview-scrollbar".to_owned())
                    .on_hover(cx.listener(|view, hovered: &bool, _, cx| {
                        view.preview_scrollbar_hovered = *hovered;
                        cx.notify();
                    }))
                    .child(
                        gpui_component::scroll::Scrollbar::vertical(&scroll).scrollbar_show(
                            if self.preview_scrollbar_hovered {
                                gpui_component::scroll::ScrollbarShow::Hover
                            } else {
                                gpui_component::scroll::ScrollbarShow::Scrolling
                            },
                        ),
                    ),
            )
            .into_any_element()
    }

    /// 预览文本样式:页面宽/标题档位/行高来自几何常量族;代码块背景取
    /// 主题 muted 槽(禁裸颜色字面量)。
    pub(super) fn preview_style(&self, cx: &mut Context<'_, Self>) -> TextViewStyle {
        let theme = cx.theme();
        TextViewStyle {
            highlight_theme: theme.highlight_theme.clone(),
            is_dark: theme.is_dark(),
            paragraph_gap: gpui::rems(0.7),
            heading_base_font_size: px(reader_presentation::HEADING_BASE),
            table: {
                let mut style = gpui::StyleRefinement::default();
                style.overflow.x = Some(gpui::Overflow::Scroll);
                style
            },
            inline_code: gpui::HighlightStyle {
                background_color: Some(*theme.tokens.muted),
                ..Default::default()
            },
            code_block: {
                let mut style = gpui::StyleRefinement::default();
                style.padding.top = Some(px(16.0).into());
                style.background = Some(theme.tokens.muted.into());
                style
            },
            ..Default::default()
        }
    }

    /// 测试专用:当帧实际物化的块下标(虚拟化断言端)。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(in crate::app_shell) fn rendered_blocks_for_test(&self) -> Vec<usize> {
        self.rendered_blocks.borrow().clone()
    }

    /// 测试专用:滚动预览到指定块(虚拟化滚动更新断言端)。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub(in crate::app_shell) fn scroll_preview_to_for_test(&self, block: usize) {
        self.scroll.scroll_to(gpui::ListOffset {
            item_ix: block,
            offset_in_item: px(0.0),
        });
    }
}

#[cfg(test)]
mod preview_tests {
    use super::*;

    /// 图片段落识别:整段仅一张图才走自定义块;混排段落留给常规内联渲染。
    #[test]
    fn only_single_image_paragraphs_become_image_blocks() {
        let options = ::markdown::ParseOptions::gfm();
        let ::markdown::mdast::Node::Root(root) =
            ::markdown::to_mdast("![alt](a.png)", &options).unwrap()
        else {
            panic!()
        };
        let block = image_block(&root.children[0]).unwrap();
        assert_eq!(block.url, "a.png");
        assert_eq!(block.alt, "alt");
        let ::markdown::mdast::Node::Root(root) =
            ::markdown::to_mdast("before ![alt](a.png)", &options).unwrap()
        else {
            panic!()
        };
        assert!(image_block(&root.children[0]).is_none());
    }

    /// 远端/内联图不走本地占位分支;相对路径按文档目录解析。
    #[test]
    fn remote_and_data_urls_are_never_treated_as_missing_local_files() {
        assert!(is_remote("https://example.com/a.png"));
        assert!(is_remote("DATA:image/png;base64,AAAA"));
        assert!(!is_remote("images/a.png"));
    }

    /// 选区自动滚动只在有选区且指针贴近视口上下缘时触发。
    #[test]
    fn selection_auto_scroll_requires_a_selection_near_a_viewport_edge() {
        let bounds = Bounds::new(
            gpui::point(px(0.0), px(100.0)),
            gpui::size(px(600.0), px(300.0)),
        );
        assert!(selection_scroll_delta(false, px(399.0), bounds).is_none());
        assert!(selection_scroll_delta(true, px(250.0), bounds).is_none());
        assert!(
            selection_scroll_delta(true, px(101.0), bounds).is_some_and(|delta| delta < px(0.0))
        );
        assert!(
            selection_scroll_delta(true, px(399.0), bounds).is_some_and(|delta| delta > px(0.0))
        );
    }
}
