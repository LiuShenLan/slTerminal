//! 有界、串行的壁纸准备:UI 线程零文件 I/O。
//!
//! 预算数值照抄上游实测值(09 篇 M10.7,契约数值非快照):文件上限、
//! 解码预算、纹理上限与边长上限约束的是本加载器持有的资源,不是进程
//! 总量,也不含解码器未公开的临时分配。跨窗口同一时刻只有一个准备任务。

use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use image::{DynamicImage, ImageDecoder, ImageError, ImageReader, RgbaImage};

/// 编码文件字节上限。
const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;
/// 单次解码分配预算。
const MAX_DECODE_BYTES: u64 = 128 * 1024 * 1024;
/// 绘制纹理像素字节上限(2048² × 4 = 16 MiB,受此预算进一步收紧)。
pub(super) const MAX_IMAGE_BYTES: u64 = 8 * 1024 * 1024;
/// 绘制纹理边长上限。
const MAX_IMAGE_EDGE: u32 = 2048;
/// 设置页预览边长上限。
const MAX_PREVIEW_EDGE: u32 = 512;
/// 运行时与预览共用的准备闸:同时只跑一个解码任务。
static PREPARATION: Mutex<()> = Mutex::new(());

/// 文件缓存戳:修改时间 + 字节数;任一不等即重解码。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct FileStamp {
    modified: Option<SystemTime>,
    bytes: u64,
}

pub(super) struct Request {
    pub path: PathBuf,
    pub cached: Option<FileStamp>,
    /// 代际闸门与请求快照;后台线程只读,不持有 UI 状态。
    pub generation: Arc<AtomicU64>,
    pub version: u64,
}

impl Request {
    /// 代际失配 = 结果作废。不做协作式中断:decode 跑完即弃,
    /// 代码比半途取消简单(09 篇壁纸代际失效图)。
    pub(super) fn cancelled(&self) -> bool {
        self.generation.load(Ordering::Acquire) != self.version
    }
}

pub(super) struct LoadedImage {
    /// BGRA 通道序(与 GPUI `RenderImage` 的像素合同一致)。
    pub pixels: RgbaImage,
    /// 布局宽高按 2560 长边归一,独立于纹理降采样质量。
    pub layout_width: u32,
    pub layout_height: u32,
    pub stamp: FileStamp,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum LoadError {
    TooLarge,
    Read,
    Decode,
}

impl From<ImageError> for LoadError {
    fn from(error: ImageError) -> Self {
        match error {
            ImageError::Limits(_) => Self::TooLarge,
            ImageError::IoError(_) => Self::Read,
            _ => Self::Decode,
        }
    }
}

/// 运行时壁纸加载(严格预算;已是缓存戳则返回 `Ok(None)`)。
pub(super) fn load(request: Request) -> Result<Option<LoadedImage>, LoadError> {
    let _preparation = PREPARATION
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    load_inner(request)
}

/// 设置页预览加载:同一条有界管线,结果再压到 512 边/1 MiB 以内。
///
/// 消费点 = 外观页壁纸预览(`wallpaper::load_preview_async`,后台执行器)。
pub(super) fn load_preview(request: Request) -> Result<Option<LoadedImage>, LoadError> {
    let _preparation = PREPARATION
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let generation = request.generation.clone();
    let version = request.version;
    let Some(mut loaded) = load_inner(request)? else {
        return Ok(None);
    };
    if generation.load(Ordering::Acquire) != version {
        return Ok(None);
    }
    if loaded.pixels.width() > MAX_PREVIEW_EDGE || loaded.pixels.height() > MAX_PREVIEW_EDGE {
        loaded.pixels =
            image::imageops::thumbnail(&loaded.pixels, MAX_PREVIEW_EDGE, MAX_PREVIEW_EDGE);
    }
    if generation.load(Ordering::Acquire) != version {
        return Ok(None);
    }
    Ok(Some(loaded))
}

fn load_inner(request: Request) -> Result<Option<LoadedImage>, LoadError> {
    if request.cancelled() {
        return Ok(None);
    }
    let file = File::open(&request.path).map_err(|_| LoadError::Read)?;
    let metadata = file.metadata().map_err(|_| LoadError::Read)?;
    let stamp = FileStamp {
        modified: metadata.modified().ok(),
        bytes: metadata.len(),
    };
    if stamp.bytes > MAX_FILE_BYTES {
        return Err(LoadError::TooLarge);
    }
    if request.cached.as_ref() == Some(&stamp) {
        return Ok(None);
    }

    // 流式喂解码器,不额外保留整文件 Vec。
    let mut reader = ImageReader::new(BufReader::new(file))
        .with_guessed_format()
        .map_err(|_| LoadError::Read)?;
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(MAX_DECODE_BYTES);
    limits.max_image_width = Some(32768);
    limits.max_image_height = Some(32768);
    reader.limits(limits);
    let decoder = reader.into_decoder()?;
    let (width, height) = decoder.dimensions();
    admit_dimensions(width, height, decoder.total_bytes())?;
    if request.cancelled() {
        return Ok(None);
    }
    let (target_width, target_height) = bounded_dimensions(width, height);
    let mut decoded = DynamicImage::from_decoder(decoder)?;
    if request.cancelled() {
        return Ok(None);
    }
    if (width, height) != (target_width, target_height) {
        // 整数面积缩略避免 Triangle 的全宽 RGBA32F 中间层;先缩再转 RGBA,
        // 16-bit 输入同样受益。
        decoded = decoded.thumbnail_exact(target_width, target_height);
    }
    let mut pixels = decoded.into_rgba8();
    if request.cancelled() {
        return Ok(None);
    }
    // GPUI 纹理合同是 BGRA;透明度不烘进像素(壁纸 opacity 由图层控制)。
    for pixel in pixels.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
    // 布局几何独立于纹理质量:长边归一到 2560。
    let layout_scale = (2560.0_f64 / f64::from(width.max(height))).min(1.0);
    let layout_width = (f64::from(width) * layout_scale).round().max(1.0) as u32;
    let layout_height = (f64::from(height) * layout_scale).round().max(1.0) as u32;
    Ok(Some(LoadedImage {
        pixels,
        layout_width,
        layout_height,
        stamp,
    }))
}

/// 解码前按声明尺寸与总字节先做准入,避免先分配再拒绝。
fn admit_dimensions(width: u32, height: u32, decoded_bytes: u64) -> Result<(), LoadError> {
    let rgba_bytes = u64::from(width)
        .checked_mul(u64::from(height))
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or(LoadError::TooLarge)?;
    if width == 0 || height == 0 {
        return Err(LoadError::Decode);
    }
    if decoded_bytes > MAX_DECODE_BYTES || rgba_bytes > MAX_DECODE_BYTES {
        return Err(LoadError::TooLarge);
    }
    Ok(())
}

/// 按边长与纹理字节双预算等比缩小。
fn bounded_dimensions(width: u32, height: u32) -> (u32, u32) {
    let pixels = f64::from(width) * f64::from(height);
    let scale = (f64::from(MAX_IMAGE_EDGE) / f64::from(width.max(height)))
        .min((MAX_IMAGE_BYTES as f64 / (pixels * 4.0)).sqrt())
        .min(1.0);
    (
        (f64::from(width) * scale).floor().max(1.0) as u32,
        (f64::from(height) * scale).floor().max(1.0) as u32,
    )
}

#[cfg(test)]
mod image_loader_tests {
    use super::*;

    fn request(path: PathBuf) -> Request {
        Request {
            path,
            cached: None,
            generation: Arc::new(AtomicU64::new(1)),
            version: 1,
        }
    }

    /// 输出预算对超大正方形/全景/竖图都成立,且小图不放大。
    #[test]
    fn output_budget_handles_large_square_panorama_and_portrait() {
        for (w, h) in [(7680, 4320), (4096, 4096), (32000, 8), (8, 32000), (7, 3)] {
            let (width, height) = bounded_dimensions(w, h);
            assert!(width <= MAX_IMAGE_EDGE && height <= MAX_IMAGE_EDGE);
            assert!(u64::from(width) * u64::from(height) * 4 <= MAX_IMAGE_BYTES);
            assert!(width <= w && height <= h);
        }
        assert_eq!(bounded_dimensions(7, 3), (7, 3));
    }

    /// 源预算必须在分配像素前拒绝。
    #[test]
    fn source_budget_is_checked_before_allocating_pixels() {
        assert!(admit_dimensions(7680, 4320, 7680 * 4320 * 4).is_ok());
        assert_eq!(admit_dimensions(10000, 10000, 1), Err(LoadError::TooLarge));
        assert_eq!(
            admit_dimensions(16, 16, MAX_DECODE_BYTES + 1),
            Err(LoadError::TooLarge)
        );
        assert_eq!(admit_dimensions(0, 16, 0), Err(LoadError::Decode));
    }

    /// 预览输出 ≤512 边/1 MiB,布局几何与 BGRA 通道序保持。
    #[test]
    fn preview_has_a_one_mib_output_budget_and_keeps_layout_and_bgra() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("preview.png");
        RgbaImage::from_pixel(1200, 800, image::Rgba([10, 20, 30, 255]))
            .save(&path)
            .unwrap();
        let loaded = load_preview(request(path)).unwrap().unwrap();
        assert!(
            loaded.pixels.width() <= MAX_PREVIEW_EDGE && loaded.pixels.height() <= MAX_PREVIEW_EDGE
        );
        assert!(loaded.pixels.len() <= 1024 * 1024);
        assert_eq!((loaded.layout_width, loaded.layout_height), (1200, 800));
        assert_eq!(loaded.pixels.get_pixel(0, 0).0, [30, 20, 10, 255]);
    }

    /// 代际失配的预览请求连文件都不打开。
    #[test]
    fn cancelled_preview_does_not_open_or_decode_a_file() {
        let request = request(PathBuf::from("does-not-exist.png"));
        request.generation.fetch_add(1, Ordering::Release);
        assert!(load_preview(request).unwrap().is_none());
    }

    /// BGRA 转换不烘透明度;文件戳不变即复用缓存。
    #[test]
    fn pixels_move_to_bgra_without_baking_opacity_and_unchanged_files_are_reused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("wallpaper.png");
        RgbaImage::from_pixel(3, 2, image::Rgba([10, 20, 30, 128]))
            .save(&path)
            .unwrap();
        let loaded = load(request(path.clone())).unwrap().unwrap();
        assert_eq!(loaded.pixels.get_pixel(0, 0).0, [30, 20, 10, 128]);
        let mut cached = request(path.clone());
        cached.cached = Some(loaded.stamp);
        assert!(load(cached).unwrap().is_none());
        let mut stale = request(path);
        stale.version = 0;
        assert!(load(stale).unwrap().is_none());
    }

    /// 缺失/损坏文件以错误降级,不产出渲染资源。
    #[test]
    fn corrupt_and_missing_images_fail_without_a_render_resource() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.png");
        assert!(matches!(load(request(path.clone())), Err(LoadError::Read)));
        std::fs::write(&path, b"not an image").unwrap();
        assert!(matches!(load(request(path)), Err(LoadError::Decode)));
    }
}
