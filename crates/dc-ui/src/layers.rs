//! Per-shape canvas layers (GitHub #93, #66 sub-issue 5).
//!
//! The canvas stops consuming one whole-page SVG texture and composites
//! one quad per TOP-LEVEL shape subtree instead, in z-order, over the
//! paper the canvas already paints. Each layer is produced by the SAME
//! SVG backend (`dc::svg::render_shape_layer_opts` — the identical
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
//!   the old texture keeps showing (no blanking) — and since #124 it
//!   re-RENDERS the retained parse (item 6.3: the parsed tree is kept
//!   per content version, so a bucket crossing never re-serializes or
//!   re-parses) and a crossing mid-gesture waits for the bucket to
//!   settle for a few frames first (item 6.2: open's fit-settle
//!   hysteresis used to re-raster the visible set at every nudge);
//! - **greeking** (item 2): a layer serialized at a bucket whose ppi
//!   renders some text block under [`dc::svg::TEXT_GREEK_PX`] device
//!   pixels carries gray bars instead of glyphs for those blocks —
//!   skipping the font/glyph-outline work nobody can read at that
//!   zoom — and each entry remembers which blocks greeked so a bucket
//!   change knows whether it can reuse the retained parse or must
//!   re-serialize;
//! - **culling**: only shapes whose bounds intersect the viewport are
//!   rasterized, so texture memory is bounded by what is on screen,
//!   not by page size (a 2000-shape page at 800% zoom keeps ~a screen
//!   of layers) — and since #124 item 4 also by a byte budget: once
//!   cached textures exceed it, the least-recently-shown ones release
//!   their GPU memory (SVG/tree retained) until it fits.
//!
//! The whole-page `render_page_with` path stays as the export/print
//! consumer and the Info thumbnail; group-edit mode (#78) composites
//! only the open group's layer — its subtree at page positions through
//! the normal group transforms, exactly the #78 scoped render.

use crate::rasterpool::{Posted, RasterJob, RasterKey, RasterPool, Tree};
use egui::{Context, TextureHandle, TextureId, TextureOptions};
use std::collections::HashMap;
use std::sync::Arc;

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

/// A ppi-bucket crossing re-posts only after the bucket has been
/// unchanged for this many consecutive frames (#124 item 6.2, the
/// fit-settle debounce): open's scrollbar-hysteresis nudges the zoom
/// across buckets for a few frames, and each crossing used to
/// re-raster the whole visible set. First fills and content edits
/// never defer — only pure re-buckets of a shape whose texture is
/// already showing (scaled) do. Frame-count based so kittest stays
/// deterministic.
const SETTLE_FRAMES: u32 = 3;

/// Total RGBA bytes of cached layer textures the store keeps before
/// least-recently-used entries start releasing GPU memory (#124 item
/// 4, DesignCraft's 768 MB decoded-image budget analogue). Off-screen
/// eviction (EVICT_MARGIN) is spatial; this one is volumetric — a
/// huge page at high zoom keeps a screen's worth of layers, never a
/// page's worth.
const TEXTURE_BUDGET_BYTES: u64 = 768 * 1024 * 1024;

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
    /// Parsed tree from the entry's last Parse job, keyed by the
    /// content version it renders (#124 item 6.3): a ppi-bucket
    /// change re-RENDERS the held tree instead of re-parsing — the
    /// same parse-once/render-per-bucket reuse the page path has had
    /// since #91, now per layer. Dropped when the content version
    /// moves on; a texture release (eviction) keeps the tree, so a
    /// scroll-back re-renders without re-parsing.
    tree: Option<Tree>,
    tree_version: u64,
    /// Greek profile of the retained SVG/tree (#124 item 2): every
    /// text size (inches) the serialization met and how many of them
    /// greeked at the ppi it was built for. A bucket change re-uses
    /// the tree/SVG only while a bucket at the new ppi would greek
    /// the SAME blocks (greeked-ness is monotone in size, so equal
    /// counts mean equal sets).
    greek_sizes: Option<Arc<[f64]>>,
    greek_count: usize,
    /// RGBA bytes of the landed texture (LRU accounting, #124 item 4).
    bytes: u64,
    /// Monotonic visibility stamp — the store's LRU clock (item 4).
    last_used: u64,
}

/// The per-shape layer store for the CURRENT page (cleared on page or
/// document switch — refill is progressive and off-thread).
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
    /// Bucket-settle state (#124 item 6.2): the last bucket seen and
    /// how many consecutive frames it has been unchanged.
    last_bucket: Option<u32>,
    settle: u32,
    /// LRU clock and cache-volume accounting (#124 item 4).
    stamp: u64,
    tex_bytes: u64,
    budget_bytes: u64,
    /// Shapes drawn THIS frame — the budget never evicts a texture
    /// the current frame is about to show (the just-landed one
    /// included; they are re-bumped next frame anyway).
    drawn: std::collections::HashSet<i64>,
}

impl Default for ShapeLayers {
    fn default() -> Self {
        Self {
            page: None,
            entries: HashMap::new(),
            versions: HashMap::new(),
            next_version: 0,
            base_version: 0,
            bboxes: HashMap::new(),
            stale_bbox: Default::default(),
            gone: Default::default(),
            text_skip: None,
            last_bucket: None,
            settle: 0,
            stamp: 0,
            tex_bytes: 0,
            budget_bytes: TEXTURE_BUDGET_BYTES,
            drawn: Default::default(),
        }
    }
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
                    // Byte accounting before the drop (#124 item 4).
                    if let Some(e) = self.entries.remove(&t.shape) {
                        if e.tex.is_some() {
                            self.tex_bytes = self.tex_bytes.saturating_sub(e.bytes);
                        }
                    }
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
        self.tex_bytes = 0;
        self.drawn.clear();
        // The bucket re-settles for the new content: a fresh store's
        // first fills post immediately anyway, but the counter starts
        // from zero so the fit-settle debounce still guards the
        // re-buckets that follow open's zoom hysteresis.
        self.last_bucket = None;
        self.settle = 0;
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
    #[allow(clippy::too_many_arguments)] // the landing carries its full identity (page/id/version/bucket) + tree
    pub(crate) fn on_shape_done(
        &mut self,
        ctx: &Context,
        page: i64,
        id: i64,
        version: u64,
        bucket: u32,
        image: anyhow::Result<egui::ColorImage>,
        tree: Option<Tree>,
    ) -> Option<String> {
        // A completion for a different page than the one being
        // composited is stale across a page switch (ids collide across
        // pages; the store was cleared on the switch).
        if self.page != Some(page) {
            return None;
        }
        // Entry mutations scoped so the store-level LRU accounting
        // below can take `self` again.
        let (old_bytes, new_bytes, status) = {
            let entry = self.entries.get_mut(&id)?;
            if entry.inflight != Some((version, bucket)) {
                return None; // superseded: the live job's landing is still owed
            }
            entry.inflight = None;
            let v = version;
            match image {
                Ok(image) => {
                    let [iw, ih] = image.size;
                    let bytes = (iw as u64) * (ih as u64) * 4;
                    let tex = {
                        let _s =
                            crate::perf::span(crate::perf::Stage::Upload(crate::perf::Kind::Page));
                        ctx.load_texture(
                            format!("layer-{page}-{id}"),
                            image,
                            TextureOptions::LINEAR,
                        )
                    };
                    // Cache the parsed tree beside the texture (#124
                    // item 6.3): bucket changes re-render it instead
                    // of re-parsing, until the content version moves on.
                    if let Some(tree) = tree {
                        entry.tree = Some(tree);
                        entry.tree_version = v;
                    }
                    let old = entry.bytes;
                    entry.tex = None; // release a replaced texture handle
                    entry.bytes = bytes;
                    entry.tex = Some(tex);
                    entry.version = v;
                    entry.bucket = bucket;
                    (old, entry.bytes, None)
                }
                Err(e) => {
                    entry.failed = Some((v, format!("{e:#}")));
                    (0, 0, Some(format!("layer {id}: {e:#}")))
                }
            }
        };
        self.tex_bytes = self.tex_bytes + new_bytes - old_bytes;
        if new_bytes > 0 {
            self.stamp += 1;
            if let Some(entry) = self.entries.get_mut(&id) {
                entry.last_used = self.stamp;
            }
            self.enforce_budget(id);
        }
        status
    }

    /// Enforce the texture byte budget (item 4): release
    /// least-recently-shown textures — never the freshly landed one,
    /// never a texture this frame draws — until under budget. The
    /// released entries keep their SVG/tree, so re-showing them is a
    /// re-render, not a re-raster from scratch.
    fn enforce_budget(&mut self, just_landed: i64) {
        if self.tex_bytes <= self.budget_bytes {
            return;
        }
        let drawn = &self.drawn;
        let mut order: Vec<(u64, i64)> = self
            .entries
            .iter()
            .filter(|(id, e)| e.tex.is_some() && **id != just_landed && !drawn.contains(*id))
            .map(|(id, e)| (e.last_used, *id))
            .collect();
        order.sort_unstable();
        for (_, id) in order {
            if self.tex_bytes <= self.budget_bytes {
                break;
            }
            let Some(entry) = self.entries.get_mut(&id) else { continue };
            let Some(tex) = entry.tex.take() else { continue };
            drop(tex);
            self.tex_bytes = self.tex_bytes.saturating_sub(entry.bytes);
            entry.bytes = 0;
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

    /// kittest (#124 item 4): total RGBA bytes of the cached textures —
    /// the LRU budget's volume probe.
    #[doc(hidden)]
    pub fn tex_bytes_for_test(&self) -> u64 {
        self.tex_bytes
    }

    /// kittest (#124 item 4): pin the texture byte budget for an
    /// eviction test (the production default is 768 MiB).
    #[doc(hidden)]
    pub fn set_texture_budget_for_test(&mut self, bytes: u64) {
        self.budget_bytes = bytes;
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
        // Bucket-settle state (#124 item 6.2): how many consecutive
        // frames this exact bucket has held. First fills and content
        // edits never wait for it; only pure re-buckets do.
        if self.last_bucket != Some(bucket) {
            self.settle = 0;
        } else {
            self.settle = self.settle.saturating_add(1);
        }
        self.last_bucket = Some(bucket);
        let bucket_settled = self.settle >= SETTLE_FRAMES;
        let bucket_ppi = bucket as f32 / 4.0;
        // Budget enforcement runs after the loop (a method call would
        // conflict with the iteration's entry borrow); remember what
        // landed inline this frame so the LRU never evicts it.
        let mut just_landed: Option<i64> = None;
        self.drawn.clear();
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
            // Guides are canvas-overlay furniture (#113), never ink:
            // no raster layer for them.
            if shape.type_.as_deref() == Some("Guide") {
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
                        if e.tex.take().is_some() {
                            self.tex_bytes = self.tex_bytes.saturating_sub(e.bytes);
                            e.bytes = 0;
                        }
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
                tree: None,
                tree_version: u64::MAX,
                greek_sizes: None,
                greek_count: 0,
                bytes: 0,
                last_used: 0,
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
                // Settle gate (#124 item 6.2): while the zoom is still
                // moving, a shape whose texture already shows its
                // current content keeps that texture (scaled) and the
                // re-bucket waits for the bucket to hold for
                // SETTLE_FRAMES frames. First fills (no texture) and
                // content edits (version moved) post immediately.
                let rebucket_only = entry.tex.is_some() && entry.version == version;
                if rebucket_only && !bucket_settled {
                    // fall through to the quad push below
                } else if !pool.has_budget() {
                    // The cap defers the WORK, not just the post: skip
                    // before serializing so a big page's fill spreads
                    // over frames instead of bursting O(page) strings
                    // on frame one (#93).
                    continue;
                } else {
                    // Tree reuse (#124 item 6.3): a bucket change with
                    // a live tree for THIS content version and an
                    // unchanged greek profile re-renders the tree — no
                    // re-serialize, no re-parse. Otherwise serialize
                    // (also when the new bucket would greek different
                    // text blocks, #124 item 2).
                    let profile_ok = entry.greek_profile_matches(bucket_ppi);
                    let use_tree = entry.tree.is_some()
                        && entry.tree_version == version
                        && profile_ok;
                    if !use_tree
                        && (entry.svg.is_none()
                            || entry.svg_version != version
                            || !profile_ok)
                    {
                        let _s =
                            crate::perf::span(crate::perf::Stage::Svg(crate::perf::Kind::Page));
                        match dc::svg::render_shape_layer_opts(
                            doc, page, shape.id, hidden, bbox, skip_text, Some(ppi),
                        ) {
                            Ok((sizes, svg)) => {
                                entry.svg = Some(svg);
                                entry.svg_version = version;
                                entry.greek_count = dc::svg::greeked_count(&sizes, bucket_ppi);
                                entry.greek_sizes = if sizes.is_empty() {
                                    None
                                } else {
                                    Some(sizes.into())
                                };
                            }
                            Err(e) => {
                                entry.failed = Some((version, format!("{e:#}")));
                                status = Some(format!("layer {}: {e:#}", shape.id));
                                continue;
                            }
                        }
                    }
                    let width = (bbox.2 as f32 * ppi).clamp(1.0, 2048.0);
                    let job = if use_tree {
                        RasterJob::Render {
                            tree: entry.tree.clone().expect("use_tree implies a tree"),
                            width,
                        }
                    } else {
                        RasterJob::Parse {
                            svg: entry.svg.clone().unwrap_or_default(),
                            width,
                        }
                    };
                    match pool.post(
                        RasterKey::Shape {
                            page: page.id,
                            id: shape.id,
                            version,
                            bucket,
                        },
                        job,
                    ) {
                        Posted::Sync(image, tree) => {
                            // Determinism seam: the raster is ready this
                            // frame (the kittest goldens depend on it).
                            match image {
                                Ok(image) => {
                                    let [iw, ih] = image.size;
                                    let new_bytes = (iw as u64) * (ih as u64) * 4;
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
                                    if let Some(tree) = tree {
                                        entry.tree = Some(tree);
                                        entry.tree_version = version;
                                    }
                                    self.tex_bytes = self.tex_bytes + new_bytes - entry.bytes;
                                    entry.bytes = new_bytes;
                                    entry.tex = Some(tex);
                                    entry.version = version;
                                    entry.bucket = bucket;
                                    self.stamp += 1;
                                    entry.last_used = self.stamp;
                                    just_landed = Some(shape.id);
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
            }
            // Draw whatever texture exists — a stale-bucket texture
            // keeps showing (scaled) until the new raster lands.
            if let Some(tex) = entry.tex.as_ref() {
                self.drawn.insert(shape.id);
                self.stamp += 1;
                entry.last_used = self.stamp;
                quads.push(ShapeQuad {
                    id: shape.id,
                    tex: tex.id(),
                    bbox,
                });
            }
        }
        // #124 item 4: hold the cache to its byte budget once this
        // frame's landings and draws are accounted for.
        if let Some(id) = just_landed {
            self.enforce_budget(id);
        }
        (quads, status)
    }
}

impl Entry {
    /// Whether a layer rendered at `bucket_ppi` would serialize the
    /// same greek profile the retained SVG/tree was built with (#124
    /// item 2): the same number of blocks greeks — and since
    /// greeked-ness is monotone in size, the same SET of blocks — so
    /// the retained bytes are still exactly what this bucket renders.
    fn greek_profile_matches(&self, bucket_ppi: f32) -> bool {
        match &self.greek_sizes {
            // No text sizes recorded (text-free layer, or never
            // serialized): nothing can change.
            None => true,
            Some(sizes) => dc::svg::greeked_count(sizes, bucket_ppi) == self.greek_count,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image() -> egui::ColorImage {
        egui::ColorImage::new([2, 2], vec![egui::Color32::RED; 4])
    }

    fn image_px(w: usize, h: usize) -> egui::ColorImage {
        egui::ColorImage::new([w, h], vec![egui::Color32::RED; w * h])
    }

    /// #124 item 4: once a landing pushes the texture volume over the
    /// byte budget, the least-recently-shown layers release their GPU
    /// textures first (SVG/tree retained — a re-show is a re-render),
    /// and neither the just-landed texture nor one drawn this frame is
    /// ever the evicted one. Enforcement runs per landing, so the
    /// budget is pinned after warm-up to simulate a cache that grew
    /// past it.
    #[test]
    fn texture_budget_evicts_lru_first() {
        let ctx = egui::Context::default();
        let mut layers = ShapeLayers::default();
        layers.set_page(0);
        // Warm three 64-byte textures (4×4 RGBA) under the default.
        for id in [1, 2, 3] {
            layers.entries.insert(id, entry_inflight(1, 384));
            layers.on_shape_done(&ctx, 0, id, 1, 384, Ok(image_px(4, 4)), None);
        }
        assert_eq!(layers.tex_bytes, 192);
        // Explicit stamps (landings already stamped 1..3): shape 2 is
        // the LRU one, shape 1 the most recently shown.
        layers.entries.get_mut(&2).unwrap().last_used = 1;
        layers.entries.get_mut(&3).unwrap().last_used = 2;
        layers.entries.get_mut(&1).unwrap().last_used = 3;
        // Pin the budget (two 64-byte textures fit; three do not) —
        // the next landing must trim shape 2 and shape 3, keeping the
        // recently shown shape 1 and the fresh landing.
        layers.set_texture_budget_for_test(140);
        layers.entries.insert(4, entry_inflight(1, 384));
        layers.on_shape_done(&ctx, 0, 4, 1, 384, Ok(image_px(4, 4)), None);
        assert!(layers.entries[&4].tex.is_some(), "the fresh texture survives");
        assert!(layers.entries[&2].tex.is_none(), "the LRU texture is evicted first");
        assert!(layers.entries[&3].tex.is_none(), "the next-LRU texture goes too");
        assert!(layers.entries[&1].tex.is_some(), "the recently shown texture survives");
        assert_eq!(layers.tex_bytes, 128, "accounting tracks the evictions");
        // Marking a texture as drawn this frame protects it: with the
        // same squeeze, shape 1 (drawn) stays and the new LRU goes.
        layers.drawn.insert(1);
        layers.entries.insert(5, entry_inflight(1, 384));
        layers.on_shape_done(&ctx, 0, 5, 1, 384, Ok(image_px(4, 4)), None);
        assert!(layers.entries[&1].tex.is_some(), "a drawn-this-frame texture is protected");
        assert!(layers.entries[&4].tex.is_none(), "the new LRU goes instead");
        assert!(layers.entries[&5].tex.is_some(), "the fresh texture survives");
    }

    /// #124 item 4: membership drops and page/doc clears keep the byte
    /// accounting exact — a stale tex_bytes would make the budget evict
    /// textures it does not actually hold.
    #[test]
    fn membership_and_clear_release_their_bytes() {
        let ctx = egui::Context::default();
        let mut layers = ShapeLayers::default();
        layers.set_page(0);
        layers.entries.insert(7, entry_inflight(1, 384));
        layers.on_shape_done(&ctx, 0, 7, 1, 384, Ok(image_px(4, 4)), None);
        assert_eq!(layers.tex_bytes, 64);
        let mut touched = dc_core::TouchList::default();
        touched.shapes.push(dc_core::Touched {
            page: 0,
            shape: 7,
            touch: dc_core::Touch::Membership,
        });
        layers.touch(&touched, 0);
        assert_eq!(layers.tex_bytes, 0, "a membership drop releases its bytes");

        // A landing followed by clear(): the whole store's volume
        // resets with its entries.
        layers.entries.insert(9, entry_inflight(2, 384));
        layers.on_shape_done(&ctx, 0, 9, 2, 384, Ok(image_px(4, 4)), None);
        assert_eq!(layers.tex_bytes, 64);
        layers.clear();
        assert_eq!(layers.tex_bytes, 0, "clear resets the volume");
    }

    /// #124 item 2: the greek profile decides tree reuse — a bucket
    /// whose ppi would greek the same blocks keeps the profile (the
    /// retained bytes are exact for it), while a bucket that crosses a
    /// block's threshold reports a mismatch so the layer re-serializes.
    #[test]
    fn greek_profile_matches_only_when_no_block_flips() {
        let mut e = entry_inflight(1, 384);
        // A layer with 0.02 in and 0.2 in text: at 384 ppi (4 px and
        // 76.8 px em) the small block greeks, the large one doesn't.
        e.greek_sizes = Some(vec![0.02, 0.2].into());
        e.greek_count = dc::svg::greeked_count(&[0.02, 0.2], 96.0);
        assert!(e.greek_profile_matches(96.0), "the serialized ppi matches itself");
        assert!(e.greek_profile_matches(97.0), "no flip between 96 and 97 ppi");
        // At 48 ppi the large block still renders (9.6 px em) and the
        // small one greeks: same profile.
        assert!(e.greek_profile_matches(48.0), "0.02 in still greeks at 48 ppi");
        // At 190 ppi the small block clears 4 px (3.8 → 0.02×190=3.8
        // still greeks; 0.02×210=4.2 clears): crossing the boundary
        // flips the profile.
        assert!(!e.greek_profile_matches(210.0), "0.02 in clears 4 px at 210 ppi");
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
            tree: None,
            tree_version: u64::MAX,
            greek_sizes: None,
            greek_count: 0,
            bytes: 0,
            last_used: 0,
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
            .on_shape_done(&ctx, 0, 7, 1, 384, Ok(image()), None)
            .is_none());
        let e = &layers.entries[&7];
        assert_eq!(e.inflight, Some((2, 384)), "superseded landing must not take the mark");
        assert!(e.tex.is_none(), "superseded image must not be cached");
        // The live v2 job lands: stored, mark cleared.
        assert!(layers
            .on_shape_done(&ctx, 0, 7, 2, 384, Ok(image()), None)
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
        layers.on_shape_done(&ctx, 0, 7, old_base, 384, Ok(image()), None);
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
        layers.on_shape_done(&ctx, 1, 7, new_base, 384, Ok(image()), None);
        assert_eq!(
            layers.entries[&7].inflight,
            Some((re_base, 384)),
            "a pre-clear completion must not consume the mark"
        );
        assert!(layers.entries[&7].tex.is_none());
        // The live identity still lands.
        layers.on_shape_done(&ctx, 1, 7, re_base, 384, Ok(image()), None);
        assert!(layers.entries[&7].tex.is_some());
        assert_eq!(layers.entries[&7].version, re_base);
    }
}
