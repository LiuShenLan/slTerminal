//! 文档/图片 tab 与打开路由(07 篇 M8.1)。
//!
//! [`openable_in_app`] 是双击路由单判定(图片 → markdown → 可映射源码),
//! fileViewers 注册表值集并入此判定(07 篇照抄点 19/38,策略链机制不采纳)。
//! 图片 tab 本步点亮后台 BGRA 解码 + fit-width 占位缩放;完整缩放/拖拽
//! 几何状态机(ImageView,0.25×–8×)归 M8.3。
//!
//! Document tab 在 M8.1 复用 [`TextFileView`] 只读骨架(预览渲染态归 M8.3)。

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    App, Bounds, ContentMask, Context, Corners, InteractiveElement as _, IntoElement,
    ParentElement as _, Pixels, Render, RenderImage, Styled as _, Window, div, px,
};
use gpui_component::ActiveTheme as _;
use image::Frame;

use crate::i18n::Message;

pub use super::file_editor::TextFileView as DocTabView;

/// M8.3 之前图片 tab 的占位缩放:初始 fit-width,恒定 1.0。
const PLACEHOLDER_ZOOM: f32 = 1.0;

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

/// 只读图片查看 tab(后台 BGRA 解码;缩放占位见 [`PLACEHOLDER_ZOOM`])。
pub struct ImageTabView {
    pub path: PathBuf,
    pub title: String,
    /// 后台解码的像素(BGRA 帧);None = 解码中或失败。
    image: Option<Arc<RenderImage>>,
    error: Option<String>,
    /// 上一帧查看区矩形(窗口坐标);事件换算用,绘制读当帧 bounds。
    area: Rc<RefCell<Bounds<Pixels>>>,
}

impl ImageTabView {
    pub fn new(path: PathBuf, cx: &mut Context<'_, Self>) -> Self {
        let title = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());
        let mut this = Self {
            path,
            title,
            image: None,
            error: None,
            area: Rc::new(RefCell::new(Bounds::default())),
        };
        this.spawn_decode(cx);
        this
    }

    /// 重新读盘(同路径再次打开时宿主调用)。
    pub fn reload(&mut self, cx: &mut Context<'_, Self>) {
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
                    Ok(image) => view.image = Some(image),
                    Err(error) => view.error = Some(error),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 测试/诊断:解码是否完成(像素帧在内存)。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub fn is_loaded_for_test(&self) -> bool {
        self.image.is_some()
    }

    /// 测试/诊断:占位缩放值(M8.3 换成 ImageView 的 zoom 状态)。
    #[cfg(all(test, feature = "gpui-test-support"))]
    pub fn zoom_for_test(&self) -> f32 {
        PLACEHOLDER_ZOOM
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

/// fit-width 占位几何:宽度贴满查看区,按原始纵横比缩放,垂直居中。
fn placeholder_rect(
    image: &RenderImage,
    area: (f32, f32, f32, f32),
    zoom: f32,
) -> (f32, f32, f32, f32) {
    let size = image.size(0);
    let width = size.width.0.max(1) as f32;
    let height = size.height.0.max(1) as f32;
    let base = area.2.max(1.0) / width.max(1.0);
    let draw_w = width * base * zoom;
    let draw_h = height * base * zoom;
    (
        area.0 + (area.2 - draw_w) * 0.5,
        area.1 + (area.3 - draw_h) * 0.5,
        draw_w.max(1.0),
        draw_h.max(1.0),
    )
}

impl Render for ImageTabView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let muted = cx.theme().muted_foreground;
        let store = self.area.clone();
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
                let target = placeholder_rect(&image, area, PLACEHOLDER_ZOOM);
                let target_bounds = Bounds::new(
                    gpui::point(px(target.0), px(target.1)),
                    gpui::size(px(target.2), px(target.3)),
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
            .size_full()
            .relative()
            .overflow_hidden()
            .bg(cx.theme().background)
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

    /// 占位几何:宽度贴满、纵横比保持、垂直居中(缩放占位合同)。
    #[test]
    fn placeholder_rect_fills_width_and_centers_vertically() {
        let pixels = vec![0u8; 4 * 4 * 2];
        let image = RenderImage::new([Frame::new(
            image::RgbaImage::from_raw(4, 2, pixels).expect("4x2 RGBA"),
        )]);
        let rect = placeholder_rect(&image, (10.0, 20.0, 200.0, 100.0), 1.0);
        assert_eq!(rect.2, 200.0, "宽度贴满查看区");
        assert_eq!(rect.3, 100.0, "纵横比 = 4:2 → 高度减半");
        assert!(rect.0 >= 10.0);
    }
}
