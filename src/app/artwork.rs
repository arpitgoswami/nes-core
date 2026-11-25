//! Cover art loading with graceful fallbacks.
//!
//! A ROM can ship next to a `name.png` or `name.jpg` cover. When no artwork is
//! present the library draws a deterministic generated tile instead.

use eframe::egui;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub struct ArtworkCache {
    cache: HashMap<PathBuf, Option<egui::TextureHandle>>,
}

impl ArtworkCache {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }

    pub fn texture(&mut self, ctx: &egui::Context, rom_path: &Path) -> Option<egui::TextureHandle> {
        if let Some(cached) = self.cache.get(rom_path) {
            return cached.clone();
        }
        let loaded = load_cover(ctx, rom_path);
        self.cache.insert(rom_path.to_path_buf(), loaded.clone());
        loaded
    }
}

fn load_cover(ctx: &egui::Context, rom_path: &Path) -> Option<egui::TextureHandle> {
    for extension in ["png", "jpg", "jpeg"] {
        let candidate = rom_path.with_extension(extension);
        if !candidate.is_file() {
            continue;
        }
        let Ok(image) = image::open(&candidate) else {
            continue;
        };
        let rgba = image.resize(384, 216, image::imageops::FilterType::Triangle);
        let rgba = rgba.to_rgba8();
        let color_image = egui::ColorImage::from_rgba_unmultiplied(
            [rgba.width() as usize, rgba.height() as usize],
            rgba.as_raw(),
        );
        let name = format!("cover-{}", candidate.display());
        return Some(ctx.load_texture(name, color_image, egui::TextureOptions::LINEAR));
    }
    None
}

/// Stable accent color for generated artwork.
pub fn accent_for(rom_id: u64) -> egui::Color32 {
    let hue = (rom_id % 360) as f32 / 360.0;
    let (r, g, b) = hsv_to_rgb(hue, 0.62, 0.85);
    egui::Color32::from_rgb(r, g, b)
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (u8, u8, u8) {
    let i = (h * 6.0).floor();
    let f = h * 6.0 - i;
    let p = v * (1.0 - s);
    let q = v * (1.0 - f * s);
    let t = v * (1.0 - (1.0 - f) * s);
    let (r, g, b) = match (i as i32) % 6 {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    ((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8)
}
