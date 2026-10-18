//! 文档/图片 tab 与打开路由(07 篇 M8.1 → M8.3)。
//!
//! [`openable_in_app`] 是双击路由单判定(图片 → markdown → 可映射源码),
//! fileViewers 注册表值集并入此判定(07 篇照抄点 19/38,策略链机制不采纳)。
//!
//! 图片 tab:0.25×–8× 缩放、1.18 步进、指针锚点缩放、拖拽平移、后台 BGRA
//! 解码与同路径 reload;绘制按当帧 bounds 换算(不依赖上一帧矩形)。M8.3 起
//! 图片 tab 持有独立焦点,工作区动作可到达。
//!
//! Document tab 自 M8.3 起是 [`TextFileView`] 的渲染态 + 源码态双模态。

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    App, Bounds, ContentMask, Context, Corners, FocusHandle, Focusable, InteractiveElement as _,
    IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement as _,
    Pixels, Render, RenderImage, ScrollWheelEvent, Styled as _, Window, div, px,
};
use gpui_component::ActiveTheme as _;
use image::Frame;

use crate::i18n::Message;

pub use super::file_editor::TextFileView as DocTabView;

const MIN_ZOOM: f32 = 0.25;
const MAX_ZOOM: f32 = 8.0;
const ZOOM_PER_STEP: f32 = 1.18;

/// 图片扩展名判定(上游 image_viewer 值集:png/jpg/jpeg/webp/bmp)。
pub fn image_viewable(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|extension| extension.to_str())
            .map(|extension| extension.to_ascii_lowercase())
            .as_deref(),
        Some("png" | "jpg" | "jpeg" | "webp" | "bmp")
    )
}

/// Markdown 文档判定(Document tab 的路由臂;源码/文本归 Code tab)。
pub fn markdown_viewable(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|extension| extension.to_str())
            .map(|extension| extension.to_ascii_lowercase())
            .as_deref(),
        Some("md" | "markdown")
    )
}

/// 双击路由单判定:图片 → markdown → 可映射源码;未命中交系统处理器
/// (系统打开动作归 M8.4 file_tree 入口,本判定只回答「应用内能否打开」)。
pub fn openable_in_app(path: &Path) -> bool {
    image_viewable(path) || markdown_viewable(path) || super::code_tab::viewable_file(path)
}

/// 图片几何状态机(照抄 image_viewer):缩放/平移/锚点/边缘钳制纯数学,
/// 绘制不依赖上一帧 bounds。
#[derive(Clone)]
struct ImageGeometry {
    dimensions: Option<(u32, u32)>,
    zoom: f32,
    pan: (f32, f32),
    drag_last: Option<(f32, f32)>,
}

impl ImageGeometry {
    fn open(path: &Path) -> Self {
        Self {
            dimensions: image_dimensions(path).ok(),
            zoom: 1.0,
            pan: (0.0, 0.0),
            drag_last: None,
        }
    }

    fn reload(&mut self, path: &Path) {
        self.dimensions = image_dimensions(path).ok();
    }

    fn set_dimensions(&mut self, dimensions: (u32, u32)) {
        if self.dimensions.is_none() {
            self.dimensions = Some(dimensions);
        }
    }

    /// 指针锚点缩放:被检查的像素留在指针下。
    fn zoom_by(&mut self, steps: f32, anchor: (f32, f32), area: (f32, f32, f32, f32)) -> bool {
        if steps.abs() < f32::EPSILON || self.dimensions.is_none() {
            return false;
        }
        let old_zoom = self.zoom;
        let next_zoom = (old_zoom * ZOOM_PER_STEP.powf(steps)).clamp(MIN_ZOOM, MAX_ZOOM);
        if (next_zoom - old_zoom).abs() < f32::EPSILON {
            return false;
        }
        let old = self.target_rect(area);
        let u = ((anchor.0 - old.0) / old.2.max(1.0)).clamp(0.0, 1.0);
        let v = ((anchor.1 - old.1) / old.3.max(1.0)).clamp(0.0, 1.0);
        self.zoom = next_zoom;
        let (draw_w, draw_h) = self.draw_size(area);
        let centered_x = area.0 + (area.2 - draw_w) * 0.5;
        let centered_y = area.1 + (area.3 - draw_h) * 0.5;
        self.pan.0 = anchor.0 - u * draw_w - centered_x;
        self.pan.1 = anchor.1 - v * draw_h - centered_y;
        self.clamp_pan(area);
        true
    }

    fn begin_drag(&mut self, point: (f32, f32), area: (f32, f32, f32, f32)) -> bool {
        if !contains(area, point) {
            return false;
        }
        self.drag_last = Some(point);
        true
    }

    fn drag_to(&mut self, point: (f32, f32), area: (f32, f32, f32, f32)) -> bool {
        let Some(previous) = self.drag_last.replace(point) else {
            return false;
        };
        let before = self.pan;
        self.pan.0 += point.0 - previous.0;
        self.pan.1 += point.1 - previous.1;
        self.clamp_pan(area);
        self.pan != before
    }

    fn dragging(&self) -> bool {
        self.drag_last.is_some()
    }

    fn end_drag(&mut self) -> bool {
        self.drag_last.take().is_some()
    }

    /// 渲染矩形(zoom/pan/边缘钳制后的最终几何)。
    fn render_rect(&self, area: (f32, f32, f32, f32)) -> (f32, f32, f32, f32) {
        self.target_rect(area)
    }

    /// 初始比例铺满视口宽度;缩放/平移相对稳定基线,resize 不累积漂移。
    fn draw_size(&self, area: (f32, f32, f32, f32)) -> (f32, f32) {
        let Some((width, height)) = self.dimensions else {
            return (area.2, area.3);
        };
        let base = area.2.max(1.0) / width.max(1) as f32;
        (
            width as f32 * base * self.zoom,
            height as f32 * base * self.zoom,
        )
    }

    fn target_rect(&self, area: (f32, f32, f32, f32)) -> (f32, f32, f32, f32) {
        let (draw_w, draw_h) = self.draw_size(area);
        let max_pan_x = ((draw_w - area.2) * 0.5).max(0.0);
        let max_pan_y = ((draw_h - area.3) * 0.5).max(0.0);
        let pan_x = self.pan.0.clamp(-max_pan_x, max_pan_x);
        let pan_y = self.pan.1.clamp(-max_pan_y, max_pan_y);
        (
            area.0 + (area.2 - draw_w) * 0.5 + pan_x,
            area.1 + (area.3 - draw_h) * 0.5 + pan_y,
            draw_w,
            draw_h,
        )
    }

    fn clamp_pan(&mut self, area: (f32, f32, f32, f32)) {
        let (draw_w, draw_h) = self.draw_size(area);
        let max_x = ((draw_w - area.2) * 0.5).max(0.0);
        let max_y = ((draw_h - area.3) * 0.5).max(0.0);
        self.pan.0 = self.pan.0.clamp(-max_x, max_x);
        self.pan.1 = self.pan.1.clamp(-max_y, max_y);
    }
}

fn contains(area: (f32, f32, f32, f32), point: (f32, f32)) -> bool {
    point.0 >= area.0 && point.0 < area.0 + area.2 && point.1 >= area.1 && point.1 < area.1 + area.3
}

fn image_dimensions(path: &Path) -> Result<(u32, u32), String> {
    image::ImageReader::open(path)
        .map_err(|error| format!("open {path:?}: {error}"))?
        .with_guessed_format()
        .map_err(|error| format!("detect image format: {error}"))?
        .into_dimensions()
        .map_err(|error| format!("read image dimensions: {error}"))
}

/// 只读图片查看 tab(后台 BGRA 解码;M8.3 真实缩放/平移几何)。
pub struct ImageTabView {
    pub path: PathBuf,
    pub title: String,
    /// 共享几何状态机(zoom/pan/锚点/钳制)。
    geometry: ImageGeometry,
    /// 后台解码的像素(BGRA 帧);None = 解码中或失败。
    image: Option<Arc<RenderImage>>,
    error: Option<String>,
    /// 上一帧查看区矩形(窗口坐标);事件换算用,绘制读当帧 bounds。
    area: Rc<RefCell<Bounds<Pixels>>>,
    focus: FocusHandle,
}

impl Focusable for ImageTabView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl ImageTabView {
    pub fn new(path: PathBuf, cx: &mut Context<'_, Self>) -> Self {
        let title = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());
        let geometry = ImageGeometry::open(&path);
        let mut this = Self {
            path,
            title,
            geometry,
            image: None,
            error: None,
            area: Rc::new(RefCell::new(Bounds::default())),
            focus: cx.focus_handle(),
        };
        this.spawn_decode(cx);
        this
    }

    /// 焦点句柄(工作区动作分派与焦点下沉入口)。
    pub fn focus_handle(&self, cx: &App) -> FocusHandle {
        <Self as Focusable>::focus_handle(self, cx)
    }

    /// 重新读盘(同路径再次打开时宿主调用)。
    pub fn reload(&mut self, cx: &mut Context<'_, Self>) {
        self.geometry.reload(&self.path);
        self.image = None;
        self.error = None;
        self.spawn_decode(cx);
        cx.notify();
    }

    fn spawn_decode(&mut self, cx: &mut Context<'_, Self>) {
        let path = self.path.clone();
        let task = cx
            .background_executor()
            .spawn(async move { decode_bgra(&path) });
        cx.spawn(async move |this, cx| {
            let decoded = task.await;
            let _ = this.update(cx, |view, cx| {
                match decoded {
                    Ok(image) => {
                        let size = image.size(0);
                        view.geometry.set_dimensions((
                            size.width.0.max(1) as u32,
                            size.height.0.max(1) as u32,
                        ));
                        view.image = Some(image);
                    }
                    Err(error) => view.error = Some(error),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn area_tuple(&self) -> (f32, f32, f32, f32) {
        let bounds = *self.area.borrow();
        (
            f32::from(bounds.origin.x),
            f32::from(bounds.origin.y),
            f32::from(bounds.size.width),
            f32::from(bounds.size.height),
        )
    }

    fn on_scroll(&mut self, event: &ScrollWheelEvent, _: &mut Window, cx: &mut Context<'_, Self>) {
        let steps = event.delta.pixel_delta(px(40.0)).y.as_f32() / 40.0;
        let anchor = (f32::from(event.position.x), f32::from(event.position.y));
        if self.geometry.zoom_by(steps, anchor, self.area_tuple()) {
            cx.notify();
        }
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        window.focus(&self.focus, cx);
        let point = (f32::from(event.position.x), f32::from(event.position.y));
        if self.geometry.begin_drag(point, self.area_tuple()) {
            cx.notify();
        }
    }

    fn on_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        _: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if !self.geometry.dragging() {
            return;
        }
        if event.pressed_button != Some(MouseButton::Left) {
            let _ = self.geometry.end_drag();
            return;
        }
        let point = (f32::from(event.position.x), f32::from(event.position.y));
        if self.geometry.drag_to(point, self.area_tuple()) {
            cx.notify();
        }
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, cx: &mut Context<'_, Self>) {
        if self.geometry.end_drag() {
            cx.notify();
        }
    }

    /// 测试/诊断:解码是否完成(像素帧在内存)。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub fn is_loaded_for_test(&self) -> bool {
        self.image.is_some()
    }

    /// 测试/诊断:当前缩放倍率。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub fn zoom_for_test(&self) -> f32 {
        self.geometry.zoom
    }

    /// 测试/诊断:独立焦点是否在图片 tab 上。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub fn focus_is_active_for_test(&self, window: &Window, cx: &App) -> bool {
        self.focus.contains_focused(window, cx)
    }

    /// 测试专用:按步进缩放(滚轮事件换算的单测入口)。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub fn zoom_for_test_steps(&mut self, steps: f32, cx: &mut Context<'_, Self>) {
        let area = self.area_tuple();
        if self
            .geometry
            .zoom_by(steps, (area.0 + area.2 * 0.5, area.1 + area.3 * 0.5), area)
        {
            cx.notify();
        }
    }

    fn status_text(&self, cx: &App) -> Option<String> {
        let language = cx.global::<crate::app_shell::config::Settings>().language;
        if let Some(error) = &self.error {
            return Some(language.format(Message::EditorImageDecodeFailed, &[("error", error)]));
        }
        None
    }

    fn loading_text(&self, cx: &App) -> Option<&'static str> {
        if self.image.is_none() && self.error.is_none() {
            let language = cx.global::<crate::app_shell::config::Settings>().language;
            return Some(language.text(Message::EditorImageLoading));
        }
        None
    }
}

/// RGBA8 → BGRA(gpui 帧通道序);在后台 executor 运行。
fn decode_bgra(path: &Path) -> Result<Arc<RenderImage>, String> {
    let bytes =
        std::fs::read(path).map_err(|error| format!("无法读取 {}: {error}", path.display()))?;
    let mut rgba = image::load_from_memory(&bytes)
        .map_err(|error| format!("无法解码 {}: {error}", path.display()))?
        .into_rgba8();
    for pixel in rgba.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
    Ok(Arc::new(RenderImage::new([Frame::new(rgba)])))
}

impl Render for ImageTabView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let muted = cx.theme().muted_foreground;
        let store = self.area.clone();
        let geometry_snapshot = self.geometry.clone();
        let image = self.image.clone();
        // paint 闭包吃当帧 bounds:矩形现算,没有首帧空窗。
        let painter = gpui::canvas(
            move |bounds, _, _| {
                *store.borrow_mut() = bounds;
                bounds
            },
            move |_, bounds: Bounds<Pixels>, window, _| {
                let Some(image) = image else { return };
                let area = (
                    f32::from(bounds.origin.x),
                    f32::from(bounds.origin.y),
                    f32::from(bounds.size.width),
                    f32::from(bounds.size.height),
                );
                let target = geometry_snapshot.render_rect(area);
                let target_bounds = Bounds::new(
                    gpui::point(px(target.0), px(target.1)),
                    gpui::size(px(target.2.max(1.0)), px(target.3.max(1.0))),
                );
                window.with_content_mask(Some(ContentMask { bounds }), |window| {
                    let _ = window.paint_image(
                        target_bounds,
                        target_bounds,
                        Corners::all(px(0.0)),
                        image,
                        0,
                        false,
                    );
                });
            },
        )
        .absolute()
        .inset_0();

        let loading = self.loading_text(cx);
        let status = self.status_text(cx);
        div()
            .debug_selector(|| "slterm-image-tab".to_owned())
            .id("slterm-image-tab")
            .key_context("ImageTab")
            .track_focus(&self.focus)
            .size_full()
            .relative()
            .overflow_hidden()
            .bg(cx.theme().background)
            .on_scroll_wheel(cx.listener(Self::on_scroll))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .child(painter)
            .when_some(loading, |root, text| {
                root.child(
                    div()
                        .absolute()
                        .top_2()
                        .left_2()
                        .px_2()
                        .py_1()
                        .rounded_md()
                        .text_sm()
                        .text_color(muted)
                        .child(text),
                )
            })
            .when_some(status, |root, text| {
                root.child(
                    div()
                        .absolute()
                        .top_2()
                        .left_2()
                        .px_2()
                        .py_1()
                        .rounded_md()
                        .text_sm()
                        .text_color(muted)
                        .child(text),
                )
            })
    }
}

#[cfg(test)]
mod doc_tabs_tests {
    use super::*;
    use std::path::Path;

    /// 三判定正例:图片 / markdown / 可映射源码。
    #[test]
    fn openable_in_app_accepts_images_documents_and_source() {
        for path in ["photo.png", "README.md", "src/main.rs", "page.HTML"] {
            assert!(openable_in_app(Path::new(path)), "{path}");
        }
    }

    /// 三判定反例:未知扩展名/二进制/无扩展名一律交系统处理器。
    #[test]
    fn openable_in_app_rejects_unknown_files() {
        for path in ["archive.zip", "app.exe", "no_extension", "photo.svg"] {
            assert!(!openable_in_app(Path::new(path)), "{path}");
        }
    }

    /// 图片值集与 markdown 值集大小写不敏感,且互不越界。
    #[test]
    fn image_and_markdown_sets_stay_disjoint() {
        assert!(image_viewable(Path::new("A.JPEG")));
        assert!(image_viewable(Path::new("b.WEBP")));
        assert!(!image_viewable(Path::new("c.md")));
        assert!(markdown_viewable(Path::new("D.MARKDOWN")));
        assert!(!markdown_viewable(Path::new("e.rs")));
    }

    fn view(dimensions: (u32, u32)) -> ImageGeometry {
        ImageGeometry {
            dimensions: Some(dimensions),
            zoom: 1.0,
            pan: (0.0, 0.0),
            drag_last: None,
        }
    }

    /// 初始宽度铺满视口。
    #[test]
    fn initial_image_width_fills_the_viewport() {
        let image = view((1600, 900));
        let area = (20.0, 30.0, 800.0, 600.0);
        let target = image.target_rect(area);
        assert!((target.0 - area.0).abs() < 0.01);
        assert!((target.2 - area.2).abs() < 0.01);
    }

    /// 缩放保持指针锚点稳定。
    #[test]
    fn zoom_keeps_the_pointer_anchor_stable() {
        let mut image = view((1600, 1600));
        let area = (20.0, 30.0, 800.0, 400.0);
        let anchor = (320.0, 180.0);
        let before = image.target_rect(area);
        let before_u = (anchor.0 - before.0) / before.2;
        let before_v = (anchor.1 - before.1) / before.3;
        assert!(image.zoom_by(2.0, anchor, area));
        let after = image.target_rect(area);
        let after_u = (anchor.0 - after.0) / after.2;
        let after_v = (anchor.1 - after.1) / after.3;
        assert!((before_u - after_u).abs() < 0.001);
        assert!((before_v - after_v).abs() < 0.001);
    }

    /// 缩放档位钳制在 0.25×–8×。
    #[test]
    fn zoom_steps_clamp_to_the_documented_range() {
        let mut image = view((100, 100));
        let area = (0.0, 0.0, 400.0, 400.0);
        assert!(image.zoom_by(100.0, (200.0, 200.0), area));
        assert_eq!(image.zoom, MAX_ZOOM);
        assert!(image.zoom_by(-100.0, (200.0, 200.0), area));
        assert_eq!(image.zoom, MIN_ZOOM);
    }

    /// 拖拽被图片边缘钳制。
    #[test]
    fn dragging_is_clamped_to_the_image_edges() {
        let mut image = view((1600, 900));
        let area = (0.0, 0.0, 800.0, 400.0);
        assert!(image.zoom_by(3.0, (400.0, 200.0), area));
        assert!(image.begin_drag((400.0, 200.0), area));
        image.drag_to((4000.0, 200.0), area);
        let target = image.target_rect(area);
        assert!(target.0 <= area.0 + 0.01);
        assert!(target.0 + target.2 >= area.0 + area.2 - 0.01);
        assert!(image.end_drag());
    }
}
