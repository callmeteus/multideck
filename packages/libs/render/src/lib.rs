//! Renders [`KeyView`] models into device ready RGBA images.
//!
//! The same renderer feeds the on screen preview and the bytes sent to a
//! device, so what the editor shows is exactly what the hardware gets.

mod icons;

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};

use image::{ImageEncoder, RgbaImage};
use multideck_core::{KeyImageFormat, KeyView, Rgb};
use thiserror::Error;

pub use icons::icon_path;

/// Failures while rasterizing or encoding a key image.
#[derive(Debug, Error)]
pub enum RenderError {
    /// The generated SVG could not be parsed.
    #[error("invalid key SVG: {0}")]
    Svg(String),
    /// The image could not be encoded for the device.
    #[error("could not encode key image: {0}")]
    Encode(String),
}

/// Converts [`KeyView`]s into RGBA images, with a small cache.
pub struct KeyRenderer {
    font_family: String,
    fontdb: Arc<fontdb::Database>,
    cache: Mutex<HashMap<u64, Arc<RgbaImage>>>,
}

impl std::fmt::Debug for KeyRenderer {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("KeyRenderer")
            .field("font_family", &self.font_family)
            .finish_non_exhaustive()
    }
}

impl Default for KeyRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl KeyRenderer {
    /// Builds a renderer using the best font available on the system.
    pub fn new() -> Self {
        let mut database = fontdb::Database::new();
        database.load_system_fonts();
        let font_family = pick_font(&database);
        tracing::debug!(family = %font_family, "key renderer ready");
        Self {
            font_family,
            fontdb: Arc::new(database),
            cache: Mutex::new(HashMap::new()),
        }
    }

    /// Renders one key at the given square size.
    pub fn render(&self, key: &KeyView, size: u32) -> Arc<RgbaImage> {
        let size = size.clamp(32, 512);
        let cache_key = cache_key(key, size);
        if let Some(hit) = self
            .cache
            .lock()
            .expect("cache lock is not poisoned")
            .get(&cache_key)
        {
            return Arc::clone(hit);
        }

        let mut image = self.render_base(key, size);
        if let Some(path) = key.icon.as_deref()
            && let Some(icon) = self.render_icon(path, key, size)
        {
            let offset_x = (size.saturating_sub(icon.width())) / 2;
            let offset_y = ((size as f32) * 0.12) as u32;
            image::imageops::overlay(&mut image, &icon, offset_x as i64, offset_y as i64);
        }

        let image = Arc::new(image);
        let mut cache = self.cache.lock().expect("cache lock is not poisoned");
        if cache.len() > 512 {
            cache.clear();
        }
        cache.insert(cache_key, Arc::clone(&image));
        image
    }

    /// Encodes an image into the format expected by a device.
    pub fn encode(
        &self,
        image: &RgbaImage,
        format: KeyImageFormat,
    ) -> Result<Vec<u8>, RenderError> {
        match format {
            KeyImageFormat::Raw => Ok(image.as_raw().clone()),
            KeyImageFormat::Bmp => {
                let mut buffer = Vec::new();
                image::codecs::bmp::BmpEncoder::new(&mut buffer)
                    .write_image(
                        image.as_raw(),
                        image.width(),
                        image.height(),
                        image::ExtendedColorType::Rgba8,
                    )
                    .map_err(|error| RenderError::Encode(error.to_string()))?;
                Ok(buffer)
            }
            KeyImageFormat::Png => {
                let mut buffer = Vec::new();
                image::codecs::png::PngEncoder::new(&mut buffer)
                    .write_image(
                        image.as_raw(),
                        image.width(),
                        image.height(),
                        image::ExtendedColorType::Rgba8,
                    )
                    .map_err(|error| RenderError::Encode(error.to_string()))?;
                Ok(buffer)
            }
            KeyImageFormat::Jpeg => {
                let mut buffer = Vec::new();
                let rgb = image::DynamicImage::ImageRgba8(image.clone()).to_rgb8();
                image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buffer, 92)
                    .encode(
                        rgb.as_raw(),
                        rgb.width(),
                        rgb.height(),
                        image::ExtendedColorType::Rgb8,
                    )
                    .map_err(|error| RenderError::Encode(error.to_string()))?;
                Ok(buffer)
            }
        }
    }

    fn render_base(&self, key: &KeyView, size: u32) -> RgbaImage {
        let s = size as f32;
        let radius = s * 0.14;
        let background = format!("#{}", key.background.hex());
        let foreground = format!("#{}", key.foreground.hex());
        let accent = format!("#{}", key.accent.hex());

        let has_icon = key.icon.is_some();
        let icon_band = if has_icon { s * 0.42 } else { 0.0 };
        let title = ellipsize(&key.title, 12);
        let subtitle = key.subtitle.as_deref().map(|text| ellipsize(text, 14));

        let title_size = if subtitle.is_some() {
            s * 0.15
        } else {
            s * 0.17
        };
        let subtitle_size = s * 0.11;
        let title_y = if has_icon {
            if subtitle.is_some() {
                s * 0.66
            } else {
                s * 0.72
            }
        } else if subtitle.is_some() {
            s * 0.5
        } else {
            s * 0.57
        };
        let subtitle_y = title_y + subtitle_size * 1.25;

        let mut elements = String::new();
        if key.active {
            elements.push_str(&format!(
                r#"<rect x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" rx="{:.1}" fill="none" stroke="{}" stroke-width="{:.1}" opacity="0.95"/>"#,
                s * 0.035,
                s * 0.035,
                s * 0.93,
                s * 0.93,
                radius * 0.8,
                accent,
                (s * 0.02).max(2.0)
            ));
        }
        if key.is_empty() {
            elements.push_str(&format!(
                r#"<text x="{cx:.1}" y="{cy:.1}" font-family="{family}" font-size="{fs:.1}" fill="{fg}" text-anchor="middle" opacity="0.55">+</text>"#,
                cx = s * 0.5,
                cy = s * 0.62,
                family = escape(&self.font_family),
                fs = s * 0.28,
                fg = foreground,
            ));
        } else {
            elements.push_str(&format!(
                r#"<text x="{cx:.1}" y="{ty:.1}" font-family="{family}" font-size="{fs:.1}" font-weight="600" fill="{fg}" text-anchor="middle">{title}</text>"#,
                cx = s * 0.5,
                ty = title_y,
                family = escape(&self.font_family),
                fs = title_size,
                fg = foreground,
                title = escape(&title),
            ));
            if let Some(subtitle) = subtitle {
                elements.push_str(&format!(
                    r#"<text x="{cx:.1}" y="{sy:.1}" font-family="{family}" font-size="{fs:.1}" fill="{fg}" text-anchor="middle" opacity="0.72">{text}</text>"#,
                    cx = s * 0.5,
                    sy = subtitle_y,
                    family = escape(&self.font_family),
                    fs = subtitle_size,
                    fg = foreground,
                    text = escape(&subtitle),
                ));
            }
        }

        let svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{s}" height="{s}" viewBox="0 0 {s} {s}"><rect width="{s}" height="{s}" rx="{rx:.1}" fill="{bg}"/>{elements}</svg>"#,
            s = s,
            rx = radius,
            bg = background,
            elements = elements,
        );

        match self.rasterize(&svg, size) {
            Some(image) => image,
            None => {
                let mut fallback = RgbaImage::new(size, size);
                for pixel in fallback.pixels_mut() {
                    *pixel =
                        image::Rgba([key.background.0, key.background.1, key.background.2, 255]);
                }
                let _ = &icon_band;
                fallback
            }
        }
    }

    fn render_icon(&self, path: &str, key: &KeyView, size: u32) -> Option<RgbaImage> {
        let icon_size = ((size as f32) * 0.3).round() as u32;
        let color = format!("#{}", key.foreground.hex());
        let svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{s}" height="{s}" viewBox="0 0 24 24"><path d="{path}" fill="{color}"/></svg>"#,
            s = icon_size,
            path = escape(path),
            color = color,
        );
        let mut image = self.rasterize(&svg, icon_size)?;
        if key.active {
            tint(&mut image, key.accent);
        }
        Some(image)
    }

    fn rasterize(&self, svg: &str, size: u32) -> Option<RgbaImage> {
        let options = usvg::Options {
            fontdb: Arc::clone(&self.fontdb),
            font_family: self.font_family.clone(),
            font_size: (size as f32) * 0.16,
            ..Default::default()
        };

        let tree = match usvg::Tree::from_str(svg, &options) {
            Ok(tree) => tree,
            Err(error) => {
                tracing::warn!(%error, "could not parse generated svg");
                return None;
            }
        };

        let mut pixmap = resvg::tiny_skia::Pixmap::new(size, size)?;
        resvg::render(
            &tree,
            resvg::tiny_skia::Transform::identity(),
            &mut pixmap.as_mut(),
        );

        let mut image = RgbaImage::new(size, size);
        for (index, pixel) in pixmap.pixels().iter().enumerate() {
            let color = pixel.demultiply();
            let offset = index * 4;
            image.as_mut()[offset] = color.red();
            image.as_mut()[offset + 1] = color.green();
            image.as_mut()[offset + 2] = color.blue();
            image.as_mut()[offset + 3] = color.alpha();
        }
        Some(image)
    }
}

fn cache_key(key: &KeyView, size: u32) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    key.hash(&mut hasher);
    size.hash(&mut hasher);
    hasher.finish()
}

fn pick_font(database: &fontdb::Database) -> String {
    const CANDIDATES: [&str; 6] = [
        "Segoe UI",
        "Inter",
        "DejaVu Sans",
        "Noto Sans",
        "Liberation Sans",
        "Arial",
    ];
    for candidate in CANDIDATES {
        let available = database.faces().any(|face| {
            face.families
                .iter()
                .any(|(family, _)| family.eq_ignore_ascii_case(candidate))
        });
        if available {
            return candidate.to_string();
        }
    }
    database
        .faces()
        .next()
        .and_then(|face| face.families.first().map(|(family, _)| family.clone()))
        .unwrap_or_else(|| "sans-serif".to_string())
}

fn ellipsize(text: &str, max_chars: usize) -> String {
    let count = text.chars().count();
    if count <= max_chars {
        return text.to_string();
    }
    let mut truncated: String = text.chars().take(max_chars.saturating_sub(1)).collect();
    truncated.push('…');
    truncated
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn tint(image: &mut RgbaImage, color: Rgb) {
    for pixel in image.pixels_mut() {
        if pixel.0[3] > 0 {
            pixel.0[0] = ((pixel.0[0] as u16 + color.0 as u16) / 2) as u8;
            pixel.0[1] = ((pixel.0[1] as u16 + color.1 as u16) / 2) as u8;
            pixel.0[2] = ((pixel.0[2] as u16 + color.2 as u16) / 2) as u8;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_a_key_with_icon_and_title() {
        let renderer = KeyRenderer::new();
        let mut key = KeyView::empty(0);
        key.title = "Play".into();
        key.subtitle = Some("Spotify".into());
        key.icon = icon_path("media").map(str::to_string);
        key.background = Rgb::new(40, 60, 120);
        key.foreground = Rgb::new(255, 255, 255);

        let image = renderer.render(&key, 144);
        assert_eq!(image.width(), 144);
        assert_eq!(image.height(), 144);

        let center = image.get_pixel(72, 72);
        assert!(center.0[3] > 0, "center pixel must be opaque");
    }

    #[test]
    fn encodes_jpeg_bmp_and_png() {
        let renderer = KeyRenderer::new();
        let image = RgbaImage::from_pixel(32, 32, image::Rgba([10, 20, 30, 255]));

        for format in [
            KeyImageFormat::Jpeg,
            KeyImageFormat::Bmp,
            KeyImageFormat::Png,
        ] {
            let bytes = renderer.encode(&image, format).expect("encode");
            assert!(bytes.len() > 16, "{format:?} produced too few bytes");
        }
    }

    #[test]
    fn cache_returns_the_same_allocation() {
        let renderer = KeyRenderer::new();
        let key = KeyView::empty(1);
        let first = renderer.render(&key, 96);
        let second = renderer.render(&key, 96);
        assert!(Arc::ptr_eq(&first, &second));
    }
}
