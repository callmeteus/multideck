//! Image encoding for the mobile icon protocol.

use image::{ImageEncoder, RgbaImage};

use super::SessionState;
use super::proto;

pub(super) fn encode_icon(source: &RgbaImage, state: &SessionState) -> Option<proto::Image> {
    let (width, height) = state.icon_size;
    if width == 0 || height == 0 {
        return None;
    }
    let scaled = if source.width() == width && source.height() == height {
        source.clone()
    } else {
        image::imageops::resize(source, width, height, image::imageops::FilterType::Lanczos3)
    };

    let supported = |format: proto::ImageFormat| {
        state.formats.is_empty() || state.formats.contains(&(format as i32))
    };
    let (format, bytes, row_bytes) = if supported(proto::ImageFormat::Png) {
        (proto::ImageFormat::Png, encode_png(&scaled)?, 0)
    } else if supported(proto::ImageFormat::Jpg) {
        (proto::ImageFormat::Jpg, encode_jpeg(&scaled)?, 0)
    } else if supported(proto::ImageFormat::Bmp) {
        (proto::ImageFormat::Bmp, encode_bmp(&scaled)?, 0)
    } else if supported(proto::ImageFormat::Lz4) {
        let raw = scaled.into_raw();
        let compressed = lz4_flex::compress(&raw);
        (proto::ImageFormat::Lz4, compressed, width * 4)
    } else {
        tracing::warn!("phone offered no supported image format; falling back to PNG");
        (proto::ImageFormat::Png, encode_png(&scaled)?, 0)
    };

    Some(proto::Image {
        format: format as i32,
        r#type: proto::ImageType::Opaque as i32,
        size: Some(proto::Size { width, height }),
        row_bytes,
        image: bytes,
    })
}

fn encode_png(image: &RgbaImage) -> Option<Vec<u8>> {
    let mut buffer = Vec::new();
    image::codecs::png::PngEncoder::new(&mut buffer)
        .write_image(
            image.as_raw(),
            image.width(),
            image.height(),
            image::ExtendedColorType::Rgba8,
        )
        .ok()?;
    Some(buffer)
}

fn encode_jpeg(image: &RgbaImage) -> Option<Vec<u8>> {
    let mut buffer = Vec::new();
    let rgb = image::DynamicImage::ImageRgba8(image.clone()).to_rgb8();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buffer, 92)
        .encode(
            rgb.as_raw(),
            rgb.width(),
            rgb.height(),
            image::ExtendedColorType::Rgb8,
        )
        .ok()?;
    Some(buffer)
}

fn encode_bmp(image: &RgbaImage) -> Option<Vec<u8>> {
    let mut buffer = Vec::new();
    image::codecs::bmp::BmpEncoder::new(&mut buffer)
        .write_image(
            image.as_raw(),
            image.width(),
            image.height(),
            image::ExtendedColorType::Rgba8,
        )
        .ok()?;
    Some(buffer)
}
