//! 终端网格的自定义 GPUI Element。
//!
//! 热路径约定:paint 内只对 `Term` 加一次锁,通过渲染合同
//! (`slterm_terminal::render::RenderSnapshot`)把可见网格快照成纯数据后立刻
//! 放锁;颜色解析(调色板 + OSC 覆盖表)与绘制都在锁外进行。
//!
//! 定位合同:字形落在 `列号 × cell_width`。M3 逐格塑形逐格绘制(连字
//! 合并塑形随 ligatures 模块归 06);编辑、光标和选区不能改变无关格的原点。
//!
//! M3 裁剪(归篇见 13 篇):主题矫正器(06)、公式覆盖层(08)、OSC8 链接
//! 装饰(07/09)、内联图片(02 开放问题 4)、ghost/补全弹窗(08)、overlay
//! 滚动条(M5)、bell 闪烁(09)、app_cursor 臂(13 篇裁决删除)、
//! CJK 字体族/cjk_bold_regular(06)。仅暗色定位:光标/选区色直取 palette,
//! 不做亮暗 alpha 分档。

use gpui::{
    App, Bounds, ContentMask, Element, ElementId, GlobalElementId, Hitbox, HitboxBehavior, Hsla,
    InspectorElementId, LayoutId, Pixels, Rgba, SharedString, Style, TextRun, UnderlineStyle,
    Window, fill, point, px, relative, size,
};
use slterm_terminal::render::{RenderSnapshot, SnapshotConfig, boxdraw};
use vte::ansi::CursorShape;

use super::cursor_painter::{CursorPaint, paint_box_glyph};
use super::view::TerminalView;

pub struct TerminalElement {
    view: gpui::Entity<TerminalView>,
}

impl TerminalElement {
    pub fn new(view: gpui::Entity<TerminalView>) -> Self {
        Self { view }
    }

    /// 在一次锁内通过渲染合同截取快照;锁外解析颜色与绘制。
    fn snapshot(&self, rows: usize, cols: usize, cx: &App) -> Option<RenderSnapshot> {
        let view = self.view.read(cx);
        let session = view.session.as_ref()?;
        let term = session.term.lock();
        // 分段只反映内容与宽度类,绝不掺入光标状态:per-cell 绘制下反色只是
        // 换色,若让闪烁相位改变分段,整行会随闪烁重塑形而跳字。
        Some(RenderSnapshot::capture(
            &term,
            &SnapshotConfig {
                rows: rows as u16,
                cols: cols as u16,
            },
        ))
    }
}

pub struct TermLayout {
    cell_width: Pixels,
    line_height: Pixels,
    rows: usize,
    cols: usize,
    #[allow(dead_code)] // 命中区域本体:光标悬停样式等消费者随链接族归 07/09。
    hitbox: Hitbox,
}

impl gpui::IntoElement for TerminalElement {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl Element for TerminalElement {
    type RequestLayoutState = ();
    type PrepaintState = TermLayout;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = relative(1.).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let (font, font_size) = {
            let view = self.view.read(cx);
            (view.font.clone(), view.font_size)
        };
        let sample = window.text_system().shape_line(
            SharedString::new_static("M"),
            font_size,
            &[TextRun {
                len: 1,
                font,
                color: Hsla::default(),
                background_color: None,
                underline: None,
                strikethrough: None,
            }],
            None,
        );
        let scale = window.scale_factor();
        let view = self.view.read(cx);
        let cell_width = view.cell_width_for_advance(sample.width.as_f32(), scale);
        let line_height =
            view.line_height_for_metrics(sample.ascent.as_f32() + sample.descent.as_f32(), scale);

        // 网格裁定(floor、最小网格、resize 合流)全部由渲染合同的
        // ViewportTracker 在 view 内完成;元素只上报内容矩形与度量。
        self.view.update(cx, |view, cx| {
            view.set_layout(
                bounds.origin,
                cell_width,
                line_height,
                bounds.size,
                scale,
                cx,
            );
        });

        let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);

        let view = self.view.read(cx);
        // 有界、自愿开启的取证:PTY resize trace 无法区分「prepaint 没到」与
        // 「视口仍等于 spawn 网格」。
        static TRACE_ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        static TRACE_SAMPLES: std::sync::atomic::AtomicUsize =
            std::sync::atomic::AtomicUsize::new(0);
        if *TRACE_ENABLED.get_or_init(|| std::env::var_os("SLTERM_RESIZE_TRACE").is_some())
            && TRACE_SAMPLES.fetch_add(1, std::sync::atomic::Ordering::Relaxed) < 256
        {
            crate::app_shell::try_write_stderr(format_args!(
                "[slterm:resize-trace] prepaint pane={} origin=({:.2},{:.2}) content={:.2}x{:.2} cell={:.2}x{:.2} scale={:.2} viewport={}x{}",
                view.pane_id.0,
                bounds.origin.x.as_f32(),
                bounds.origin.y.as_f32(),
                bounds.size.width.as_f32(),
                bounds.size.height.as_f32(),
                cell_width.as_f32(),
                line_height.as_f32(),
                scale,
                view.grid_cols(),
                view.grid_rows(),
            ));
        }
        TermLayout {
            cell_width,
            line_height,
            rows: view.grid_rows(),
            cols: view.grid_cols(),
            hitbox,
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        layout: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let theme = self.view.read(cx).palette.clone();
        // 元素不画底色:容器统一负责底色层,元素只画单元格背景、文字与
        // 光标——默认背景的格子保持透明。

        let focus_handle = self.view.read(cx).focus_handle.clone();
        window.handle_input(
            &focus_handle,
            gpui::ElementInputHandler::new(bounds, self.view.clone()),
            cx,
        );

        let focused = focus_handle.is_focused(window);
        // 只有光标本身参与闪烁;IME 预编辑仍复用同一个坐标锚点。
        let cursor_visible = self.view.read(cx).cursor_visible();
        let Some(snap) = self.snapshot(layout.rows, layout.cols, cx) else {
            return;
        };
        let overrides = snap.color_overrides;

        let cell_rect = |row: usize, start: usize, count: usize| -> Bounds<Pixels> {
            Bounds::new(
                point(
                    bounds.origin.x + layout.cell_width * start as f32,
                    bounds.origin.y + layout.line_height * row as f32,
                ),
                size(layout.cell_width * count as f32, layout.line_height),
            )
        };

        let cursor_paint = snap.cursor.as_ref().map(|cursor| CursorPaint {
            rect: Bounds::new(
                point(
                    bounds.origin.x + layout.cell_width * cursor.col as f32,
                    bounds.origin.y + layout.line_height * cursor.row as f32,
                ),
                size(
                    layout.cell_width * if cursor.wide { 2.0 } else { 1.0 },
                    layout.line_height,
                ),
            ),
            shape: cursor.shape,
            focused,
            visible: cursor_visible,
            block_color: theme.cursor,
            stroke: match cursor.shape {
                CursorShape::Beam | CursorShape::Underline => {
                    theme.cursor_stroke.unwrap_or(theme.cursor)
                }
                _ => theme.cursor,
            },
            text_color: Some(theme.cursor_text.unwrap_or(theme.background).into()),
        });

        for run in &snap.bg_runs {
            if run.start >= run.end {
                continue;
            }
            let color = theme.resolve(run.color, &overrides, false);
            window.paint_quad(fill(
                cell_rect(
                    run.row as usize,
                    run.start as usize,
                    (run.end - run.start) as usize,
                ),
                color,
            ));
        }
        for run in &snap.selection_runs {
            window.paint_quad(fill(
                cell_rect(
                    run.row as usize,
                    run.start as usize,
                    (run.end - run.start) as usize,
                ),
                theme.selection,
            ));
        }
        let selection_foreground = theme.selection_foreground;
        let selected_foreground = |row: u16, col: u16| -> Option<Rgba> {
            let foreground = selection_foreground?;
            snap.selection_runs
                .iter()
                .any(|run| run.row == row && run.start <= col && col < run.end)
                .then_some(foreground)
        };

        if let Some(cursor) = &cursor_paint {
            cursor.paint_background(window);
        }

        let (font, bold_font, italic_font, bold_italic_font, font_size) = {
            let view = self.view.read(cx);
            (
                view.font.clone(),
                view.font_bold.clone(),
                view.font_italic.clone(),
                view.font_bold_italic.clone(),
                view.font_size,
            )
        };
        let pick_font = |bold: bool, italic: bool| match (bold, italic) {
            (false, false) => font.clone(),
            (true, false) => bold_font.clone(),
            (false, true) => italic_font.clone(),
            (true, true) => bold_italic_font.clone(),
        };
        // 内建几何字形(框线/块元素/Powerline):不走字体,直接以图元填充。
        // 几何由渲染合同裁定(含设备像素吸附),保证盖满单元格、相邻无缝。
        let scale = window.scale_factor();
        for glyph in &snap.box_glyphs {
            let fg = if let Some(foreground) = selected_foreground(glyph.row, glyph.col) {
                foreground
            } else {
                theme.resolve(glyph.fg, &overrides, glyph.bold)
            };
            let span = if glyph.wide { 2.0 } else { 1.0 };
            let Some(prims) = boxdraw::primitives(
                glyph.ch,
                layout.cell_width.as_f32() * span,
                layout.line_height.as_f32(),
                scale,
            ) else {
                continue;
            };
            let origin = point(
                snap_to_device(
                    bounds.origin.x + layout.cell_width * glyph.col as f32,
                    scale,
                ),
                snap_to_device(
                    bounds.origin.y + layout.line_height * glyph.row as f32,
                    scale,
                ),
            );
            paint_box_glyph(window, &prims, origin, fg);
            if let Some(cursor) = &cursor_paint {
                let cell_bounds =
                    Bounds::new(origin, size(layout.cell_width * span, layout.line_height));
                if let Some(color) = cursor.inverted_color(cell_bounds) {
                    window.with_content_mask(
                        Some(ContentMask {
                            bounds: cursor.rect,
                        }),
                        |window| {
                            paint_box_glyph(window, &prims, origin, color.into());
                        },
                    );
                }
            }
        }

        // 逐格绘制文本段。字形原点始终由固定网格决定。
        for seg in &snap.segments {
            for cell in &seg.cells {
                let fg: Hsla = if let Some(foreground) = selected_foreground(seg.row, cell.col) {
                    foreground.into()
                } else {
                    theme.resolve(cell.fg, &overrides, cell.bold).into()
                };
                let underline = cell.underline.then(|| UnderlineStyle {
                    thickness: px(1.0),
                    color: Some(fg),
                    wavy: false,
                });
                let strikethrough = cell.strikethrough.then(|| gpui::StrikethroughStyle {
                    thickness: px(1.0),
                    color: Some(fg),
                });
                let run = TextRun {
                    len: cell.text.len(),
                    font: pick_font(cell.bold, cell.italic),
                    color: fg,
                    background_color: None,
                    underline,
                    strikethrough,
                };
                let origin = point(
                    bounds.origin.x + layout.cell_width * cell.col as f32,
                    bounds.origin.y + layout.line_height * seg.row as f32,
                );
                let shaped = window.text_system().shape_line(
                    SharedString::from(cell.text.clone()),
                    font_size,
                    std::slice::from_ref(&run),
                    None,
                );
                let _ = shaped.paint(
                    origin,
                    layout.line_height,
                    gpui::TextAlign::Left,
                    None,
                    window,
                    cx,
                );
                if let Some(cursor) = &cursor_paint {
                    cursor.paint_inverted_text(
                        &shaped,
                        run,
                        origin,
                        layout.line_height,
                        window,
                        cx,
                    );
                }
            }
        }

        if let Some(cursor) = &snap.cursor {
            // 描边形状画在文本之后;IME 锚点是逻辑网格坐标。
            if let Some(cursor) = &cursor_paint {
                cursor.paint_foreground(window);
            }

            // IME 组合文本锚点与预编辑串:跟随光标单元格。
            let anchor = cell_rect(cursor.row as usize, cursor.col as usize, 1);
            let marked = self.view.read(cx).marked_text.clone();
            self.view.update(cx, |view, _| view.ime_bounds = anchor);
            if let Some(marked) = marked.filter(|m| !m.is_empty()) {
                let marked = single_line(&marked);
                let run = TextRun {
                    len: marked.len(),
                    font: font.clone(),
                    color: theme.foreground.into(),
                    background_color: None,
                    underline: Some(UnderlineStyle {
                        thickness: px(1.0),
                        color: Some(theme.foreground.into()),
                        wavy: false,
                    }),
                    strikethrough: None,
                };
                let shaped = window.text_system().shape_line(
                    SharedString::from(marked),
                    font_size,
                    &[run],
                    None,
                );
                let bg = Bounds::new(anchor.origin, size(shaped.width, layout.line_height));
                window.paint_quad(fill(bg, theme.background));
                let _ = shaped.paint(
                    anchor.origin,
                    layout.line_height,
                    gpui::TextAlign::Left,
                    None,
                    window,
                    cx,
                );
            }
        }
    }
}

/// 预编辑串只画一行:折叠换行,避免多行组合文本把锚点撑出网格。
fn single_line(text: &str) -> String {
    text.lines().next().unwrap_or_default().to_owned()
}

/// 设备像素吸附:几何由渲染合同裁定时,绘制原点对齐物理像素边界。
fn snap_to_device(value: Pixels, scale: f32) -> Pixels {
    let scale = scale.max(0.5);
    px((value.as_f32() * scale).round() / scale)
}
