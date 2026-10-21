//! SVG → texture rasterization for the canvas, with a small cache.
//! The GUI deliberately renders through the same SVG backend the CLI
//! uses (build plan M5: "rendering via the SVG backend first"), so what
//! you see is what `dc render` produces.

use egui::{ColorImage, Context, TextureHandle, TextureOptions};

/// Widest texture we ever allocate (kittest caps at 2048; GPUs allow more
/// but upscaling a 2048px raster is fine on screen).
const MAX_TEX: u32 = 2048;
use std::collections::HashMap;

/// Rasterize an SVG string to a `target_width`-pixel-wide RGBA image
/// (height follows the viewBox aspect).
///
/// Fonts: the embedded Carlito set (metric-compatible with Calibri,
/// OFL-1.1 — see assets/fonts/LICENSE-Carlito), NOT system fonts.
/// System fonts made rasters machine-dependent and broke the snapshot
/// tests on CI runners with different font sets.
/// The font database, built once: Carlito Regular/Bold plus generic
/// family mappings — usvg resolves unknown/no font-family through the
/// generic defaults (Arial / Times New Roman), which don't exist in a
/// Carlito-only database and would drop the text entirely.
fn font_db() -> std::sync::Arc<resvg::usvg::fontdb::Database> {
    static DB: std::sync::OnceLock<std::sync::Arc<resvg::usvg::fontdb::Database>> =
        std::sync::OnceLock::new();
    DB.get_or_init(|| {
        let mut db = resvg::usvg::fontdb::Database::new();
        db.load_font_data(include_bytes!("../assets/fonts/Carlito-Regular.ttf").to_vec());
        db.load_font_data(include_bytes!("../assets/fonts/Carlito-Bold.ttf").to_vec());
        db.set_sans_serif_family("Carlito");
        db.set_serif_family("Carlito");
        db.set_monospace_family("Carlito");
        // Last-resort glyph fallback for scripts Carlito lacks (CJK,
        // Arabic, …): better than invisible text. Latin text still
        // resolves to the explicit Carlito families, so test rasters of
        // Latin content stay machine-independent.
        db.load_system_fonts();
        std::sync::Arc::new(db)
    })
    .clone()
}

pub fn rasterize(svg: &str, target_width: f32) -> anyhow::Result<ColorImage> {
    let mut opts = resvg::usvg::Options::default();
    opts.fontdb = font_db();
    let tree = resvg::usvg::Tree::from_str(svg, &opts)
        .map_err(|e| anyhow::anyhow!("parsing page SVG: {e}"))?;
    let size = tree.size();
    let aspect = size.height().max(1e-3) / size.width().max(1e-3);
    let w = target_width.ceil().clamp(1.0, MAX_TEX as f32) as u32;
    let h = ((target_width * aspect as f32).ceil().max(1.0)) as u32;
    let (w, h) = (w, h.min(MAX_TEX));
    let mut pixmap = resvg::tiny_skia::Pixmap::new(w, h)
        .ok_or_else(|| anyhow::anyhow!("allocating {w}×{h} raster"))?;
    let t = resvg::usvg::Transform::from_scale(
        w as f32 / size.width().max(1e-3),
        h as f32 / size.height().max(1e-3),
    );
    resvg::render(&tree, t, &mut pixmap.as_mut());
    let data: &[u8] = pixmap.data();
    Ok(ColorImage::from_rgba_unmultiplied(
        [w as usize, h as usize],
        data,
    ))
}

/// Cache of page rasters keyed by (page part, model version, px-per-inch
/// bucket). Any model mutation bumps `version`, invalidating everything.
#[derive(Default)]
pub struct TextureCache {
    map: HashMap<(String, u64, u32), TextureHandle>,
}

impl TextureCache {
    pub fn get(
        &mut self,
        ctx: &Context,
        svg: &str,
        page_w_in: f64,
        key_part: &str,
        version: u64,
        px_per_in: f32,
    ) -> anyhow::Result<&TextureHandle> {
        let bucket = (px_per_in * 4.0).round().max(4.0) as u32; // ±0.25 ppi
        let key = (key_part.to_string(), version, bucket);
        if !self.map.contains_key(&key) {
            let image = rasterize(svg, page_w_in as f32 * px_per_in)?;
            let tex = ctx.load_texture(format!("page-{key_part}-{version}"), image, TextureOptions::LINEAR);
            self.map.insert(key.clone(), tex);
        }
        // Keep at most a few live entries per page; drop stale versions.
        self.map.retain(|(p, v, _), _| p != key_part || *v == version);
        Ok(&self.map[&key])
    }

    pub fn clear(&mut self) {
        self.map.clear();
    }
}
