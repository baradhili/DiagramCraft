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
    /// The top-level shape id (drag feedback suppresses/ghosts by id).
    pub id: i64,
    pub tex: TextureId,
    pub bbox: (f64, f64, f64, f64),
}

/// Past this many inches outside the viewport an entry's GPU texture
/// is released. Draw culling keeps 0.5in of look-ahead; eviction lags
/// further out so scrolling along the boundary doesn't thrash
/// rasterize/evict.
const EVICT_MARGIN: f64 = 2.0;

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
    /// Default version for shapes with no touch history. Advances
    /// monotonically on every clear so entries created after the
    /// clear never share an identity (version, bucket) with a job
    /// posted before it — a stale completion landing in a re-created
    /// entry would otherwise be indistinguishable from the live job's.
    base_version: u64,
    /// Bounds (page inches, y-down) per shape id; recomputed when
    /// stale (first sight, Ink, Move).
    bboxes: HashMap<i64, (f64, f64, f64, f64)>,
    stale_bbox: std::collections::HashSet<i64>,
    gone: std::collections::HashSet<i64>,
    /// The shape whose TEXT is currently suppressed in layer renders
    /// (the in-place editor, #22). When it changes, only the two
    /// affected shapes' rasters change — the previously suppressed
    /// shape's texture must gain its text back, the newly suppressed
    /// one's must lose it — so their content versions bump and the
    /// rest of the store stays cached (#22 review).
    text_skip: Option<i64>,
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
                    self.next_version = self.next_version.max(self.base_version) + 1;
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
        // Monotonic: past the old base AND every touched version, so
        // repeated clears with no edits in between still advance.
        self.base_version = self.base_version.max(self.next_version) + 1;
    }

    /// Page/doc switch: layers are per current page.
    pub fn set_page(&mut self, page: i64) {
        if self.page != Some(page) {
            self.page = Some(page);
            self.clear();
        }
    }

    /// Land a worker completion for a shape layer (#92 drain).
    /// Returns a failure message to surface once, if any. A
    /// completion whose (version, bucket) does not match the entry's
    /// inflight job is superseded — a newer job was posted while this
    /// one was in flight — and is discarded WITHOUT touching the
    /// inflight mark (taking it would orphan the live job: its own
    /// landing would find nothing and the stale image would stay
    /// cached as current forever).
    pub(crate) fn on_shape_done(
        &mut self,
        ctx: &Context,
        page: i64,
        id: i64,
        version: u64,
        bucket: u32,
        image: anyhow::Result<egui::ColorImage>,
    ) -> Option<String> {
        // A completion for a different page than the one being
        // composited is stale across a page switch (ids collide across
        // pages; the store was cleared on the switch).
        if self.page != Some(page) {
            return None;
        }
        let entry = self.entries.get_mut(&id)?;
        if entry.inflight != Some((version, bucket)) {
            return None; // superseded: the live job's landing is still owed
        }
        entry.inflight = None;
        let v = version;
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

    /// The landed quad for a shape — the drag feedback ghosts these
    /// (#96): a move preview is the texture translated, no re-raster.
    pub fn quad_for(&self, id: i64) -> Option<ShapeQuad> {
        let e = self.entries.get(&id)?;
        let tex = e.tex.as_ref()?;
        Some(ShapeQuad {
            id,
            tex: tex.id(),
            bbox: self.bboxes.get(&id).copied()?,
        })
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
    /// kittest (#22 review): the content version a shape's entry is
    /// keyed by — unchanged exactly when nothing invalidated it (the
    /// ppi bucket may still re-raster a valid entry, so texture ids
    /// are NOT a cache probe).
    #[doc(hidden)]
    pub fn version_for_test(&self, id: i64) -> u64 {
        self.versions.get(&id).copied().unwrap_or(self.base_version)
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
        skip_text: Option<i64>,
    ) -> (Vec<ShapeQuad>, Option<String>) {
        // Text suppression toggle (#22): only the previously and newly
        // skipped shapes' rasters change — bump their content versions
        // (the Ink discipline) so both the retained SVG and the texture
        // re-render under the new setting. The rest of the store stays
        // cached (a change used to clear it all; #22 review).
        if self.text_skip != skip_text {
            for id in [self.text_skip, skip_text].into_iter().flatten() {
                self.next_version = self.next_version.max(self.base_version) + 1;
                self.versions.insert(id, self.next_version);
            }
            self.text_skip = skip_text;
        }
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
                // #93's memory bound: culling stopped RASTERIZING
                // off-screen shapes, but landed textures were kept
                // forever — texture memory tracked everything ever
                // visible, not the screen. Release the texture well
                // past the draw margin; the retained svg/version make
                // the re-post cheap when the shape scrolls back. (A
                // job already in flight may still land once more; the
                // next frame releases it again.)
                let far = bbox.0 < vx + vw + EVICT_MARGIN
                    && bbox.0 + bbox.2 > vx - EVICT_MARGIN
                    && bbox.1 < vy + vh + EVICT_MARGIN
                    && bbox.1 + bbox.3 > vy - EVICT_MARGIN;
                if !far {
                    if let Some(e) = self.entries.get_mut(&shape.id) {
                        e.tex = None;
                    }
                }
                continue;
            }
            let version = self
                .versions
                .get(&shape.id)
                .copied()
                .unwrap_or(self.base_version);
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
                    match dc::svg::render_shape_layer_opts(
                        doc, page, shape.id, hidden, bbox, skip_text,
                    ) {
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
                        version,
                        bucket,
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
                    id: shape.id,
                    tex: tex.id(),
                    bbox,
                });
            }
        }
        (quads, status)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image() -> egui::ColorImage {
        egui::ColorImage::new([2, 2], vec![egui::Color32::RED; 4])
    }

    fn entry_inflight(version: u64, bucket: u32) -> Entry {
        Entry {
            version: u64::MAX,
            bucket,
            tex: None,
            svg: None,
            svg_version: u64::MAX,
            inflight: Some((version, bucket)),
            failed: None,
        }
    }

    /// A completion for a job that was superseded while in flight (an
    /// edit bumped the version and composite posted a newer job) is
    /// discarded without consuming the live job's inflight mark —
    /// otherwise the stale image would be cached as current and the
    /// live job's own landing would find no mark and vanish.
    #[test]
    fn superseded_shape_completion_does_not_consume_the_inflight_mark() {
        let mut layers = ShapeLayers::default();
        layers.set_page(0); // the page guard requires a current page
        layers.entries.insert(7, entry_inflight(2, 384));
        let ctx = egui::Context::default();
        // The stale v1 completion lands first (two workers, out of
        // order): discarded, inflight untouched.
        assert!(layers
            .on_shape_done(&ctx, 0, 7, 1, 384, Ok(image()))
            .is_none());
        let e = &layers.entries[&7];
        assert_eq!(e.inflight, Some((2, 384)), "superseded landing must not take the mark");
        assert!(e.tex.is_none(), "superseded image must not be cached");
        // The live v2 job lands: stored, mark cleared.
        assert!(layers
            .on_shape_done(&ctx, 0, 7, 2, 384, Ok(image()))
            .is_none());
        let e = &layers.entries[&7];
        assert_eq!(e.inflight, None);
        assert!(e.tex.is_some());
        assert_eq!(e.version, 2);
        assert_eq!(e.bucket, 384);
    }

    /// Completions that outlive their entry — a page switch (ids
    /// collide across pages) or a document reload (same page id) —
    /// must never land in the re-created entry, whose default version
    /// equals the post-clear base while the stale job carries a
    /// pre-clear identity. Pre-fix both sides defaulted to version 0
    /// and were indistinguishable.
    #[test]
    fn stale_completions_across_clear_and_page_switch_are_rejected() {
        let ctx = egui::Context::default();
        let mut layers = ShapeLayers::default();
        layers.set_page(0);
        let old_base = layers.base_version;
        layers.entries.insert(7, entry_inflight(old_base, 384));

        // Page switch: the store clears and the base advances; the new
        // page's colliding shape-7 entry gets the NEW base as its
        // default identity.
        layers.set_page(1);
        let new_base = layers.base_version;
        assert!(new_base > old_base, "clear must advance the version base");
        layers.entries.insert(7, entry_inflight(new_base, 384));
        // The old page's completion is rejected by the page guard.
        layers.on_shape_done(&ctx, 0, 7, old_base, 384, Ok(image()));
        assert_eq!(
            layers.entries[&7].inflight,
            Some((new_base, 384)),
            "a foreign-page completion must not consume the mark"
        );
        assert!(layers.entries[&7].tex.is_none());

        // Same-page reload (App::load clears in place): the page guard
        // passes, so the version identity must reject the stale job.
        layers.clear();
        let re_base = layers.base_version;
        assert!(re_base > new_base);
        layers.entries.insert(7, entry_inflight(re_base, 384));
        layers.on_shape_done(&ctx, 1, 7, new_base, 384, Ok(image()));
        assert_eq!(
            layers.entries[&7].inflight,
            Some((re_base, 384)),
            "a pre-clear completion must not consume the mark"
        );
        assert!(layers.entries[&7].tex.is_none());
        // The live identity still lands.
        layers.on_shape_done(&ctx, 1, 7, re_base, 384, Ok(image()));
        assert!(layers.entries[&7].tex.is_some());
        assert_eq!(layers.entries[&7].version, re_base);
    }
}
