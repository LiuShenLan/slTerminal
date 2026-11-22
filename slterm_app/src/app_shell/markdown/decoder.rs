//! Markdown 图片的有界静态解码。

use std::io::Cursor;

use image::{DynamicImage, ImageDecoder as _, ImageReader, RgbaImage};
use slterm_document::{SanitizedSvg, SvgError};

use super::MarkdownResourceError;

const MAX_RESOURCE_BYTES: usize = 20 * 1024 * 1024;
const MAX_DECODE_BYTES: u64 = 128 * 1024 * 1024;
const MAX_IMAGE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_IMAGE_EDGE: u32 = 8192;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MarkdownImageKind {
    Raster,
    Svg,
}

pub(crate) struct BoundedMarkdownImageDecoder;

impl BoundedMarkdownImageDecoder {
    pub(crate) fn decode(
        bytes: &[u8],
        kind: MarkdownImageKind,
    ) -> Result<RgbaImage, MarkdownResourceError> {
        if bytes.len() > MAX_RESOURCE_BYTES {
            return Err(MarkdownResourceError::TooLarge);
        }
        match kind {
            MarkdownImageKind::Raster => decode_raster(bytes),
            MarkdownImageKind::Svg => decode_svg(bytes),
        }
    }
}

fn decode_raster(bytes: &[u8]) -> Result<RgbaImage, MarkdownResourceError> {
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|_| MarkdownResourceError::Decode)?;
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(MAX_DECODE_BYTES);
    limits.max_image_width = Some(MAX_IMAGE_EDGE);
    limits.max_image_height = Some(MAX_IMAGE_EDGE);
    reader.limits(limits);
    let decoder = reader.into_decoder().map_err(map_image_error)?;
    let (width, height) = decoder.dimensions();
    admit_dimensions(width, height, decoder.total_bytes())?;
    let decoded = DynamicImage::from_decoder(decoder)
        .map_err(map_image_error)?
        .into_rgba8();
    Ok(decoded)
}

fn decode_svg(bytes: &[u8]) -> Result<RgbaImage, MarkdownResourceError> {
    let source = std::str::from_utf8(bytes).map_err(|_| MarkdownResourceError::Decode)?;
    let sanitized = SanitizedSvg::parse(source).map_err(|error| match error {
        SvgError::TooLarge => MarkdownResourceError::TooLarge,
        SvgError::Unsafe => MarkdownResourceError::Unsafe,
        SvgError::Malformed => MarkdownResourceError::Decode,
    })?;
    let options = resvg::usvg::Options {
        resources_dir: None,
        image_href_resolver: resvg::usvg::ImageHrefResolver {
            resolve_data: Box::new(|_, _, _| None),
            resolve_string: Box::new(|_, _| None),
        },
        ..resvg::usvg::Options::default()
    };
    let tree = resvg::usvg::Tree::from_str(sanitized.as_str(), &options)
        .map_err(|_| MarkdownResourceError::Decode)?;
    let size = tree.size().to_int_size();
    admit_dimensions(size.width(), size.height(), 0)?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(size.width(), size.height())
        .ok_or(MarkdownResourceError::TooLarge)?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    );
    let mut rgba = Vec::with_capacity(size.width() as usize * size.height() as usize * 4);
    for pixel in pixmap.pixels() {
        let color = pixel.demultiply();
        rgba.extend_from_slice(&[color.red(), color.green(), color.blue(), color.alpha()]);
    }
    RgbaImage::from_raw(size.width(), size.height(), rgba).ok_or(MarkdownResourceError::Decode)
}

fn admit_dimensions(width: u32, height: u32, decoded: u64) -> Result<(), MarkdownResourceError> {
    let rgba = u64::from(width)
        .checked_mul(u64::from(height))
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or(MarkdownResourceError::TooLarge)?;
    if width == 0 || height == 0 {
        return Err(MarkdownResourceError::Decode);
    }
    if width > MAX_IMAGE_EDGE
        || height > MAX_IMAGE_EDGE
        || decoded > MAX_DECODE_BYTES
        || rgba > MAX_IMAGE_BYTES
    {
        return Err(MarkdownResourceError::TooLarge);
    }
    Ok(())
}

fn map_image_error(error: image::ImageError) -> MarkdownResourceError {
    match error {
        image::ImageError::Limits(_) => MarkdownResourceError::TooLarge,
        _ => MarkdownResourceError::Decode,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ExtendedColorType, ImageEncoder as _};
    use slterm_document::MarkdownDocument;

    #[test]
    fn markdown_unsafe_html_svg_blocked() {
        let document = MarkdownDocument::parse(
            "<div><script>alert(1)</script><svg><script>alert(2)</script></svg><kbd>Safe</kbd></div>\n",
        );
        assert!(document.visible_text().contains("Safe"));
        assert!(!document.visible_text().contains("alert"));

        let dangerous = [
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"><script>alert(3)</script></svg>"##,
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"><image href="https://example.com/external.png"/></svg>"##,
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"><style>@font-face { src: url(https://example.com/font.woff2); }</style></svg>"##,
            r##"<!DOCTYPE svg [<!ENTITY xxe SYSTEM "file:///outside.txt">]><svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"><title>&xxe;</title></svg>"##,
        ];
        for source in dangerous {
            assert_eq!(
                BoundedMarkdownImageDecoder::decode(source.as_bytes(), MarkdownImageKind::Svg),
                Err(MarkdownResourceError::Unsafe)
            );
        }
    }

    #[test]
    fn svg_rasterizes_to_static_straight_rgba() {
        let source = r##"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="1"><rect width="2" height="1" fill="#ff0000"/></svg>"##;
        let decoded =
            BoundedMarkdownImageDecoder::decode(source.as_bytes(), MarkdownImageKind::Svg).unwrap();
        assert_eq!(decoded.dimensions(), (2, 1));
        assert_eq!(decoded.get_pixel(0, 0).0, [255, 0, 0, 255]);

        let translucent = r##"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"><rect width="1" height="1" fill="#ff0000" fill-opacity="0.5"/></svg>"##;
        let decoded =
            BoundedMarkdownImageDecoder::decode(translucent.as_bytes(), MarkdownImageKind::Svg)
                .unwrap();
        let pixel = decoded.get_pixel(0, 0).0;
        assert!(pixel[0] > 200 && pixel[3] > 100 && pixel[3] < 200);
    }

    #[test]
    fn raster_decode_keeps_only_the_first_frame() {
        let first = image::RgbaImage::from_pixel(2, 1, image::Rgba([1, 2, 3, 255]));
        let second = image::RgbaImage::from_pixel(2, 1, image::Rgba([9, 8, 7, 255]));
        let frames = vec![image::Frame::new(first), image::Frame::new(second)];
        let mut bytes = Vec::new();
        image::codecs::gif::GifEncoder::new(&mut bytes)
            .encode_frames(frames)
            .unwrap();
        let decoded =
            BoundedMarkdownImageDecoder::decode(&bytes, MarkdownImageKind::Raster).unwrap();
        assert_eq!(decoded.dimensions(), (2, 1));
        assert_eq!(decoded.get_pixel(0, 0).0, [1, 2, 3, 255]);
    }

    #[test]
    fn image_dimensions_and_encoded_bytes_fail_before_large_output_allocation() {
        let oversized_svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="100000" height="100000"><rect width="100000" height="100000"/></svg>"##;
        assert_eq!(
            BoundedMarkdownImageDecoder::decode(oversized_svg.as_bytes(), MarkdownImageKind::Svg,),
            Err(MarkdownResourceError::TooLarge)
        );
        let bytes = vec![0; 20 * 1024 * 1024 + 1];
        assert_eq!(
            BoundedMarkdownImageDecoder::decode(&bytes, MarkdownImageKind::Raster),
            Err(MarkdownResourceError::TooLarge)
        );

        let mut png = Vec::new();
        image::codecs::png::PngEncoder::new(&mut png)
            .write_image(&[255, 0, 0, 255], 1, 1, ExtendedColorType::Rgba8)
            .unwrap();
        assert!(BoundedMarkdownImageDecoder::decode(&png, MarkdownImageKind::Raster).is_ok());
    }
}
