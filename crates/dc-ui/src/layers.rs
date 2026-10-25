//! Per-shape canvas layers (GitHub #93, #66 sub-issue 5).
//!
//! The canvas stops consuming one whole-page SVG texture and composites
//! one quad per TOP-LEVEL shape subtree instead, in z-order, over the
//! paper the canvas already paints. Each layer is produced by the SAME
//! SVG backend (`dc::svg::render_shape_layer` — the identical
//! `render_shape` walk at absolute page coordinates), rasterized by the
//! #92 pool, and cached per (shape, content version, ppi bucket):
//!
//! - an **edit** dirties only the touched shape (`dc_core::Touch` from
//!   the command layer) — its layer re-renders, everything else is
//!   untouched: commits are O(shape), flat in page size;
//! - a **move** (PinX/PinY on a top-level shape) re-renders NOTHING —
//!   the ink is identical relative to its bounds, so the quad just
//!   places at the translated bounds;
//! - **zoom** (bucket change) re-renders on the pool off-thread while
//!   the old texture keeps showing (no blanking);
//! - **culling**: only shapes whose bounds intersect the viewport are
//!   rasterized, so texture memory is bounded by what is on screen,
//!   not by page size (a 2000-shape page at 800% zoom keeps ~a screen
//!   of layers).
//!
//! The whole-page `render_page_with` path stays as the export/print
//! consumer and the Info thumbnail; group-edit mode (#78) composites
//! only the open group's layer — its subtree at page positions through
//! the normal group transforms, exactly the #78 scoped render.

use crate::rasterpool::{Posted, RasterJob, RasterKey, RasterPool};
use egui::{Context, TextureHandle, TextureId, TextureOptions};
use std::collections::HashMap;

/// One compositing unit for a frame: a layer texture at its bounds in
/// page coordinates (inches, y-down viewBox space — `canvas::show`
/// maps it to the screen).
pub struct ShapeQuad {
    pub tex: TextureId,
    pub bbox: (f64, f64, f64, f64),
}

struct Entry {
    /// Content version + ppi bucket the current texture renders.
    version: u64,
    bucket: u32,
    tex: Option<TextureHandle>,
    /// Retained SVG string and the version it serializes (bucket
    /// changes re-post the same string without re-serializing on the
    /// UI thread; `None` until first built).
    svg: Option<String>,
    svg_version: u64,
    /// Job in flight: (version, bucket).
    inflight: Option<(u64, u32)>,
    /// (version, message) of the last failure — retried when the next
    /// edit bumps the version, not every frame.
    failed: Option<(u64, String)>,
}

/// The per-shape layer store for the CURRENT page (cleared on page or
/// document switch — refill is progressive and off-thread).
#[derive(Default)]
pub struct ShapeLayers {
    page: Option<i64>,
    entries: HashMap<i64, Entry>,
    /// Content version per shape id (a global counter; Ink bumps it).
    versions: HashMap<i64, u64>,
    next_version: u64,
    /// Bounds (page inches, y-down) per shape id; recomputed when
    /// stale (first sight, Ink, Move).
    bboxes: HashMap<i64, (f64, f64, f64, f64)>,
    stale_bbox: std::collections::HashSet<i64>,
    gone: std::collections::HashSet<i64>,
}

impl ShapeLayers {
    /// Route a command's touch list (#93 invalidation).
    pub fn touch(&mut self, touched: &dc_core::TouchList, current_page: i64) {
        for p in &touched.pages {
            if *p == current_page {
                self.clear();
                return;
            }
        }
        for t in &touched.shapes {
            if t.page != current_page {
                continue;
            }
            match t.touch {
                dc_core::Touch::Ink => {
                    self.next_version += 1;
                    self.versions.insert(t.shape, self.next_version);
                    self.stale_bbox.insert(t.shape);
                    self.gone.remove(&t.shape);
                }
                // Pure translation: the texture stays valid; only the
                // placement (bounds) changes.
                dc_core::Touch::Move => {
                    self.stale_bbox.insert(t.shape);
                }
                // Left the page (a fresh id needs nothing — created on
                // demand); drop any stale entry.
                dc_core::Touch::Membership => {
                    self.entries.remove(&t.shape);
                    self.bboxes.remove(&t.shape);
                    self.stale_bbox.remove(&t.shape);
                    self.versions.remove(&t.shape);
                    self.gone.insert(t.shape);
                }
                // Paint order is read live; nothing cached depends on it.
                dc_core::Touch::Order => {}
            }
        }
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.versions.clear();
        self.bboxes.clear();
        self.stale_bbox.clear();
        self.gone.clear();
    }

    /// Page/doc switch: layers are per current page.
    pub fn set_page(&mut self, page: i64) {
        if self.page != Some(page) {
            self.page = Some(page);
            self.clear();
        }
    }

    /// Land a worker completion for a shape layer (#92 drain).
    /// Returns a failure message to surface once, if any.
    pub(crate) fn on_shape_done(
        &mut self,
        ctx: &Context,
        page: i64,
        id: i64,
        image: anyhow::Result<egui::ColorImage>,
    ) -> Option<String> {
        let entry = self.entries.get_mut(&id)?;
        let (v, bucket) = entry.inflight.take()?;
        match image {
            Ok(image) => {
                let tex = {
                    let _s = crate::perf::span(crate::perf::Stage::Upload(crate::perf::Kind::Page));
                    ctx.load_texture(format!("layer-{page}-{id}"), image, TextureOptions::LINEAR)
                };
                entry.tex = Some(tex);
                entry.version = v;
                entry.bucket = bucket;
                None
            }
            Err(e) => {
                entry.failed = Some((v, format!("{e:#}")));
                Some(format!("layer {id}: {e:#}"))
            }
        }
    }

    /// kittest (#93): the layer texture id for a shape, if landed.
    #[doc(hidden)]
    pub fn tex_id_for_test(&self, id: i64) -> Option<TextureId> {
        self.entries.get(&id).and_then(|e| e.tex.as_ref().map(|t| t.id()))
    }

    /// kittest (#93): how many layers have landed textures.
    #[doc(hidden)]
    pub fn landed_count_for_test(&self) -> usize {
        self.entries.values().filter(|e| e.tex.is_some()).count()
    }

    /// The frame pass: refresh stale bounds, post raster jobs for
    /// entries that are missing/stale/out-of-bucket (viewport-culled,
    /// capped by the pool), and return the z-ordered quads to
    /// composite. `viewport` is the visible page rect in inches
    /// (y-down); `group_edit` limits compositing to one top-level
    /// group (#78). `ppi` is the current zoom's pixels-per-inch.
    /// Returns (quads, one-shot status message).
    #[allow(clippy::too_many_arguments)]
    pub fn composite(
        &mut self,
        ctx: &Context,
        pool: &mut RasterPool,
        doc: &vsdx::Document,
        page: &vsdx::Page,
        ppi: f32,
        viewport: (f64, f64, f64, f64),
        group_edit: Option<i64>,
        hidden: &[i64],
    ) -> (Vec<ShapeQuad>, Option<String>) {
        self.set_page(page.id);
        let bucket = (ppi * 4.0).round().max(4.0) as u32; // ±0.25 ppi
        let mut quads = Vec::new();
        let mut status = None;
        let hidden_member = |s: &vsdx::Shape| -> bool {
            s.sheet
                .cell("LayerMember")
                .and_then(|c| c.numeric())
                .is_some_and(|m| hidden.contains(&(m as i64)))
        };
        for shape in &page.shapes {
            if hidden_member(shape) {
                continue;
            }
            if let Some(g) = group_edit {
                if shape.id != g {
                    continue;
                }
            }
            if self.gone.contains(&shape.id) {
                self.gone.remove(&shape.id);
            }
            // Bounds: computed on first sight and after Ink/Move —
            // O(subtree) cell resolution, paid only for dirty shapes.
            if self.stale_bbox.contains(&shape.id) || !self.bboxes.contains_key(&shape.id) {
                match dc::svg::shape_ink_bounds(doc, page, shape.id) {
                    Some(b) => {
                        self.bboxes.insert(shape.id, b);
                        self.stale_bbox.remove(&shape.id);
                    }
                    // Unresolvable geometry: nothing to draw or raster.
                    None => continue,
                }
            }
            let bbox = self.bboxes[&shape.id];
            // Culling: rasterize (and draw) only what the viewport can
            // show, plus a margin for smooth scrolling.
            let (vx, vy, vw, vh) = viewport;
            let m = 0.5; // inches of look-ahead
            let on_screen = bbox.0 < vx + vw + m
                && bbox.0 + bbox.2 > vx - m
                && bbox.1 < vy + vh + m
                && bbox.1 + bbox.3 > vy - m;
            if !on_screen {
                continue;
            }
            let version = self.versions.get(&shape.id).copied().unwrap_or(0);
            let entry = self.entries.entry(shape.id).or_insert_with(|| Entry {
                version: u64::MAX, // never matches a real version
                bucket,
                tex: None,
                svg: None,
                svg_version: u64::MAX,
                inflight: None,
                failed: None,
            });
            let current = entry.tex.is_some()
                && entry.version == version
                && entry.bucket == bucket;
            if !current && entry.inflight != Some((version, bucket)) {
                // Retry a failure only when the content moved on.
                if let Some((failed_at, _)) = &entry.failed {
                    if *failed_at == version {
                        continue;
                    }
                    entry.failed = None;
                }
                // The cap defers the WORK, not just the post: skip
                // before serializing so a big page's fill spreads over
                // frames instead of bursting O(page) strings on frame
                // one (#93).
                if !pool.has_budget() {
                    continue;
                }
                // Serialize the layer (UI thread, O(shape) — only for
                // dirty shapes; #90 retired the per-frame whole-page
                // build). The string is retained per version so bucket
                // changes re-post without re-serializing.
                if entry.svg.is_none() || entry.svg_version != version {
                    let _s = crate::perf::span(crate::perf::Stage::Svg(crate::perf::Kind::Page));
                    match dc::svg::render_shape_layer(doc, page, shape.id, hidden, bbox) {
                        Ok(svg) => {
                            entry.svg = Some(svg);
                            entry.svg_version = version;
                        }
                        Err(e) => {
                            entry.failed = Some((version, format!("{e:#}")));
                            status = Some(format!("layer {}: {e:#}", shape.id));
                            continue;
                        }
                    }
                }
                let svg = entry.svg.clone().unwrap_or_default();
                let width = (bbox.2 as f32 * ppi).clamp(1.0, 2048.0);
                match pool.post(
                    RasterKey::Shape {
                        page: page.id,
                        id: shape.id,
                    },
                    RasterJob::Parse { svg, width },
                ) {
                    Posted::Sync(image, _) => {
                        // Determinism seam: the raster is ready this
                        // frame (the kittest goldens depend on it).
                        match image {
                            Ok(image) => {
                                let tex = {
                                    let _s = crate::perf::span(crate::perf::Stage::Upload(
                                        crate::perf::Kind::Page,
                                    ));
                                    ctx.load_texture(
                                        format!("layer-{}-{}", page.id, shape.id),
                                        image,
                                        TextureOptions::LINEAR,
                                    )
                                };
                                entry.tex = Some(tex);
                                entry.version = version;
                                entry.bucket = bucket;
                            }
                            Err(e) => {
                                entry.failed = Some((version, format!("{e:#}")));
                                status = Some(format!("layer {}: {e:#}", shape.id));
                            }
                        }
                    }
                    Posted::Queued => entry.inflight = Some((version, bucket)),
                    // Cap deferral: retried next frame (the pool's
                    // deferred flag repainted).
                    Posted::Capped => {}
                }
            }
            // Draw whatever texture exists — a stale-bucket texture
            // keeps showing (scaled) until the new raster lands.
            if let Some(tex) = entry.tex.as_ref() {
                quads.push(ShapeQuad {
                    tex: tex.id(),
                    bbox,
                });
            }
        }
        (quads, status)
    }
}
