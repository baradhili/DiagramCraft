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
//!   untouched: commits are O(shape), flat in page size. A LOCALIZED
//!   edit (a nested sub-shape's cell, #219) does less still: the
//!   command names the edited sub-shape, the store pairs its old∪new
//!   focused ink bounds into a damage rect, and the patch re-renders
//!   just that rect of the existing texture (the same full-layer
//!   serialization, rastered through `render_tree`'s own scale plus an
//!   integer-pixel translate — bit-identical to the full render by
//!   construction) and composites it with `TextureHandle::set_partial`.
//!   Damage past half the texture, a moved layer grid, or an unbounded
//!   edit falls back to the full re-render; stale completions can
//!   never overwrite a fresh patch (version/bucket identity);
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

/// The pixel rect a patch job will replace inside its entry's existing
/// texture (#219) — texture coordinates, computed when the job is
/// planned so the landing needs no geometry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PatchPlan {
    pos: [usize; 2],
    size: [usize; 2],
}

/// A raster job in flight. The (version, bucket) pair is the landing
/// identity — a completion whose pair differs is superseded and
/// discarded. `patch` marks a #219 damage-region job: its landing
/// composites into the EXISTING texture (`set_partial`) instead of
/// replacing it, at the plan's pixel rect.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Inflight {
    version: u64,
    bucket: u32,
    patch: Option<PatchPlan>,
}

struct Entry {
    /// Content version + ppi bucket the current texture renders.
    version: u64,
    bucket: u32,
    tex: Option<TextureHandle>,
    /// The page rect (inches, y-down) the CURRENT texture rasterizes —
    /// its viewBox and its quad's placement. Equal to the model's
    /// bounds except across a #219 patch (the patch keeps the old
    /// texture, so its grid stays the old bounds until the next full
    /// re-render).
    tex_bounds: Option<Rect>,
    /// Retained SVG string and the version it serializes (bucket
    /// changes re-post the same string without re-serializing on the
    /// UI thread; `None` until first built). NOT updated by a patch —
    /// the patch's serialization matches its own version, but keeping
    /// the stale mark here forces the next full re-render to
    /// re-serialize at whatever bounds the model carries then.
    svg: Option<String>,
    svg_version: u64,
    /// Job in flight.
    inflight: Option<Inflight>,
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
    /// Pending damage regions per top-level shape, in page inches
    /// (x, y, w, h) (#219): the old∪new ink bounds of the edited
    /// sub-shape, unioned across every edit since the last composite.
    /// Consumed when the shape's layer next renders (patch or full).
    pending_damage: HashMap<i64, Rect>,
    /// Tops whose pending edit CANNOT be bounded — a sub-less Ink
    /// touch, a missing pre-edit rect, a Move that staled a pending
    /// region. Their next render is a full re-render, never a patch.
    pending_full: std::collections::HashSet<i64>,
    /// Post-transition focused bounds of edited sub-shapes, per
    /// top-level shape (#219): the pre half of the NEXT transition's
    /// damage rect (an undo or redo reverts to exactly this state).
    /// Dropped for a top on any touch that can't pin its damage.
    sub_bounds: SubBounds,
    /// Test counters (#219): damage-region patch posts vs full
    /// re-render posts since the store was created, plus how many
    /// patch completions landed (sync or drained).
    patch_posts: u64,
    full_posts: u64,
    patch_landings: u64,
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
            pending_damage: HashMap::new(),
            pending_full: Default::default(),
            sub_bounds: HashMap::new(),
            patch_posts: 0,
            full_posts: 0,
            patch_landings: 0,
        }
    }
}

/// A page rect in inches (x, y, w, h, y-down) — the shape of bboxes,
/// damage regions, and focused bounds throughout the store.
pub(crate) type Rect = (f64, f64, f64, f64);

/// Post-transition focused bounds of a top's edited sub-shapes.
type SubBounds = HashMap<i64, HashMap<i64, Rect>>;

/// Union two (x, y, w, h) page rects.
fn union_rect(a: Rect, b: Rect) -> Rect {
    let x1 = (a.0 + a.2).max(b.0 + b.2);
    let y1 = (a.1 + a.3).max(b.1 + b.3);
    let nx = a.0.min(b.0);
    let ny = a.1.min(b.1);
    (nx, ny, x1 - nx, y1 - ny)
}

impl ShapeLayers {
    /// Capture the PRE-edit focused ink bounds of every nested Ink
    /// touch a command is about to produce (#219). Call with the
    /// command's own `touched()` on the PRE-apply document, just
    /// before applying. The returned `(page, top, sub, rect)` list
    /// feeds [`ShapeLayers::touch_damage`]'s `pre` argument on the
    /// apply path; undo/redo pass an empty slice and pair against the
    /// per-transition cache instead.
    pub fn harvest_pre_damage(
        &self,
        doc: &vsdx::Document,
        pre_touched: &dc_core::TouchList,
    ) -> Vec<(i64, i64, i64, Rect)> {
        let mut out = Vec::new();
        for t in &pre_touched.shapes {
            if t.touch != dc_core::Touch::Ink {
                continue;
            }
            let Some(sub) = t.sub else { continue };
            let Some(page) = doc.pages.iter().find(|p| p.id == t.page) else {
                continue;
            };
            if let Some(r) = dc::svg::shape_ink_bounds_focused(doc, page, t.shape, Some(sub)) {
                out.push((t.page, t.shape, sub, r));
            }
        }
        out
    }

    /// Route a command's touch list WITH its damage regions (#219).
    /// For every nested Ink touch, the edited sub-shape's post-edit
    /// focused bounds pair with its pre-edit bounds — from `pre` (the
    /// apply path's harvest) or the per-transition cache (undo/redo) —
    /// and the union becomes the shape's pending damage rect. The
    /// cache is written with the POST-transition bounds so the next
    /// transition pairs against exactly this state; any touch that
    /// cannot pin its damage (no sub, unresolvable bounds, missing
    /// pre rect) marks the top unbounded AND drops its cached bounds.
    pub fn touch_damage(
        &mut self,
        doc: &vsdx::Document,
        touched: &dc_core::TouchList,
        pre: &[(i64, i64, i64, Rect)],
        current_page: i64,
    ) {
        let mut damages: HashMap<i64, Rect> = HashMap::new();
        for t in &touched.shapes {
            if t.page != current_page || t.touch != dc_core::Touch::Ink {
                if t.page == current_page {
                    // Any non-ink touch of a top invalidates its
                    // cached sub bounds (a Move translates them, a
                    // Membership restructures them).
                    self.sub_bounds.remove(&t.shape);
                }
                continue;
            }
            let Some(sub) = t.sub else {
                self.pending_full.insert(t.shape);
                self.sub_bounds.remove(&t.shape);
                continue;
            };
            let Some(page) = doc.pages.iter().find(|p| p.id == t.page) else {
                continue;
            };
            let Some(post) = dc::svg::shape_ink_bounds_focused(doc, page, t.shape, Some(sub))
            else {
                self.pending_full.insert(t.shape);
                self.sub_bounds.remove(&t.shape);
                continue;
            };
            // The pre half: the apply path's harvest first, else the
            // per-transition cache (undo/redo revert to exactly the
            // state written by the previous transition). Read BEFORE
            // the cache is overwritten with `post`.
            let pre_rect = pre
                .iter()
                .find(|(p, top, s, _)| *p == t.page && *top == t.shape && *s == sub)
                .map(|(_, _, _, r)| *r)
                .or_else(|| {
                    self.sub_bounds
                        .get(&t.shape)
                        .and_then(|m| m.get(&sub))
                        .copied()
                });
            self.sub_bounds
                .entry(t.shape)
                .or_default()
                .insert(sub, post);
            match pre_rect {
                Some(p) => {
                    let d = damages.entry(t.shape).or_insert(post);
                    *d = union_rect(*d, union_rect(p, post));
                }
                // No pre rect: the edit's old ink is unbounded — full
                // re-render (the cache write above still stands).
                None => {
                    self.pending_full.insert(t.shape);
                }
            }
        }
        self.touch_with_damage(touched, current_page, &damages);
    }

    /// The touch router (#93 invalidation), recording per-shape
    /// damage regions for the #219 patcher. `damages` carries the
    /// rects computed by [`ShapeLayers::touch_damage`]; an Ink touch
    /// without one is unbounded and never patches.
    fn touch_with_damage(
        &mut self,
        touched: &dc_core::TouchList,
        current_page: i64,
        damages: &HashMap<i64, Rect>,
    ) {
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
                    if let Some(d) = damages.get(&t.shape) {
                        // Union across edits that land before the next
                        // composite: each damage covers its own
                        // transition, the texture only shows the first.
                        let e = self.pending_damage.entry(t.shape).or_insert(*d);
                        *e = union_rect(*e, *d);
                    } else {
                        self.pending_full.insert(t.shape);
                    }
                }
                // Pure translation: the texture stays valid; only the
                // placement (bounds) changes. A pending damage region
                // was mapped in the pre-move frame — stale now.
                dc_core::Touch::Move => {
                    if self.pending_damage.contains_key(&t.shape) {
                        self.pending_full.insert(t.shape);
                        self.pending_damage.remove(&t.shape);
                    }
                    self.sub_bounds.remove(&t.shape);
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
                    self.pending_damage.remove(&t.shape);
                    self.pending_full.remove(&t.shape);
                    self.sub_bounds.remove(&t.shape);
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
        self.pending_damage.clear();
        self.pending_full.clear();
        self.sub_bounds.clear();
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
    /// cached as current forever). A #219 patch completion composites
    /// into the existing texture at its planned rect instead of
    /// replacing it — a stale background render can therefore never
    /// overwrite a fresh patch (its version/bucket identity differs),
    /// and a patch whose texture was evicted mid-flight is dropped
    /// (the next frame re-fills).
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
        let (old_bytes, new_bytes, status, landed_patch) = {
            let entry = self.entries.get_mut(&id)?;
            if entry.inflight.as_ref().map(|i| (i.version, i.bucket)) != Some((version, bucket)) {
                return None; // superseded: the live job's landing is still owed
            }
            let patch = entry.inflight.take().and_then(|i| i.patch);
            let v = version;
            match image {
                Ok(image) => {
                    if let Some(plan) = patch {
                        match entry.tex.as_mut() {
                            Some(tex) => {
                                let _s = crate::perf::span(crate::perf::Stage::Upload(
                                    crate::perf::Kind::Page,
                                ));
                                tex.set_partial(plan.pos, image, TextureOptions::LINEAR);
                                entry.version = v;
                                entry.bucket = bucket;
                                // The texture's size (and byte count) is
                                // unchanged; svg/tree stay keyed to their
                                // own serialization.
                                (0, 0, None, true)
                            }
                            // The texture was evicted while the patch
                            // flew: nothing to composite into; the next
                            // frame sees a texture-less entry and
                            // re-fills.
                            None => (0, 0, None, true),
                        }
                    } else {
                        let [iw, ih] = image.size;
                        let bytes = (iw as u64) * (ih as u64) * 4;
                        let tex = {
                            let _s = crate::perf::span(crate::perf::Stage::Upload(
                                crate::perf::Kind::Page,
                            ));
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
                        let b = self.bboxes.get(&id).copied();
                        if b.is_some() {
                            entry.tex_bounds = b;
                        }
                        (old, entry.bytes, None, false)
                    }
                }
                Err(e) => {
                    entry.failed = Some((v, format!("{e:#}")));
                    (0, 0, Some(format!("layer {id}: {e:#}")), patch.is_some())
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
        if landed_patch {
            self.patch_landings += 1;
            self.stamp += 1;
            if let Some(entry) = self.entries.get_mut(&id) {
                entry.last_used = self.stamp;
            }
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
            bbox: e.tex_bounds.or_else(|| self.bboxes.get(&id).copied())?,
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
    /// kittest (#219): damage-region patch jobs posted. Consumed by
    /// the in-module decision tests only.
    #[doc(hidden)]
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn patch_posts_for_test(&self) -> u64 {
        self.patch_posts
    }
    /// kittest (#219): full re-render jobs posted.
    #[doc(hidden)]
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn full_posts_for_test(&self) -> u64 {
        self.full_posts
    }
    /// kittest (#219): patch completions landed (sync or drained).
    #[doc(hidden)]
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn patch_landings_for_test(&self) -> u64 {
        self.patch_landings
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
            // Pending damage state, read before the entry borrow: the
            // #219 patch decision needs it plus the entry's texture.
            let pending_damage = self.pending_damage.get(&shape.id).copied();
            let damage_unbounded = self.pending_full.contains(&shape.id);
            let entry = self.entries.entry(shape.id).or_insert_with(|| Entry {
                version: u64::MAX, // never matches a real version
                bucket,
                tex: None,
                tex_bounds: None,
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
            let current = entry.tex.is_some() && entry.version == version && entry.bucket == bucket;
            if !current
                && entry.inflight.as_ref().map(|i| (i.version, i.bucket)) != Some((version, bucket))
            {
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
                    // Keep the scaled texture this frame, but ask for
                    // another frame so the settle counter can advance
                    // without user input — otherwise the scaled
                    // (blurry) texture persists until some unrelated
                    // event repaints (#203 review).
                    ctx.request_repaint();
                } else if !pool.has_budget() {
                    // The cap defers the WORK, not just the post: skip
                    // before serializing so a big page's fill spreads
                    // over frames instead of bursting O(page) strings
                    // on frame one (#93).
                    continue;
                } else {
                    let patch_plan = pending_damage
                        .filter(|_| !damage_unbounded)
                        .filter(|_| entry.tex.is_some() && entry.tex_bounds.is_some())
                        .and_then(|damage| plan_patch(entry, bbox, damage));
                    if let Some(plan) = patch_plan {
                        // #219 damage-region patch: a localized content
                        // edit re-renders just the damaged rect of the
                        // EXISTING texture and composites it with
                        // set_partial. The patch serializes the SAME
                        // full-layer SVG a full re-render would (post-edit
                        // model, current bounds) but greeks at the
                        // TEXTURE's own ppi — the patch must match the
                        // pixels around it, not the current zoom's intent.
                        let ppi_patch = entry.bucket as f32 / 4.0;
                        let _s =
                            crate::perf::span(crate::perf::Stage::Svg(crate::perf::Kind::Page));
                        match dc::svg::render_shape_layer_opts(
                            doc,
                            page,
                            shape.id,
                            hidden,
                            bbox,
                            skip_text,
                            Some(ppi_patch),
                        ) {
                            Ok((_, svg)) => {
                                let [tex_w, tex_h] =
                                    entry.tex.as_ref().expect("checked above").size();
                                let job = RasterJob::ParseRegion {
                                    svg,
                                    width: tex_w as u32,
                                    height: tex_h as u32,
                                    crop_x: plan.pos[0] as i32,
                                    crop_y: plan.pos[1] as i32,
                                    patch_w: plan.size[0] as u32,
                                    patch_h: plan.size[1] as u32,
                                };
                                self.pending_damage.remove(&shape.id);
                                self.patch_posts += 1;
                                match pool.post(
                                    RasterKey::Shape {
                                        page: page.id,
                                        id: shape.id,
                                        version,
                                        bucket: entry.bucket,
                                    },
                                    job,
                                ) {
                                    Posted::Sync(image, _) => {
                                        // Determinism seam: the patch is
                                        // ready this frame (the kittest
                                        // goldens depend on it).
                                        match image {
                                            Ok(image) => {
                                                if let Some(t) = entry.tex.as_mut() {
                                                    let _s = crate::perf::span(
                                                        crate::perf::Stage::Upload(
                                                            crate::perf::Kind::Page,
                                                        ),
                                                    );
                                                    t.set_partial(
                                                        plan.pos,
                                                        image,
                                                        TextureOptions::LINEAR,
                                                    );
                                                    entry.version = version;
                                                    entry.bucket = bucket;
                                                    self.patch_landings += 1;
                                                }
                                            }
                                            Err(e) => {
                                                entry.failed = Some((version, format!("{e:#}")));
                                                status = Some(format!("layer {}: {e:#}", shape.id));
                                            }
                                        }
                                    }
                                    Posted::Queued => {
                                        entry.inflight = Some(Inflight {
                                            version,
                                            bucket: entry.bucket,
                                            patch: Some(plan),
                                        });
                                    }
                                    // Cap deferral: retried next frame.
                                    Posted::Capped => {
                                        self.pending_damage.insert(
                                            shape.id,
                                            pending_damage.expect("plan implies damage"),
                                        );
                                        self.patch_posts -= 1;
                                    }
                                }
                            }
                            Err(e) => {
                                entry.failed = Some((version, format!("{e:#}")));
                                status = Some(format!("layer {}: {e:#}", shape.id));
                            }
                        }
                    } else {
                        // A full re-render covers every pending damage.
                        self.pending_damage.remove(&shape.id);
                        self.pending_full.remove(&shape.id);
                        self.full_posts += 1;
                        // Tree reuse (#124 item 6.3): a bucket change with
                        // a live tree for THIS content version and an
                        // unchanged greek profile re-renders the tree — no
                        // re-serialize, no re-parse. Otherwise serialize
                        // (also when the new bucket would greek different
                        // text blocks, #124 item 2).
                        let profile_ok = entry.greek_profile_matches(bucket_ppi);
                        let use_tree =
                            entry.tree.is_some() && entry.tree_version == version && profile_ok;
                        if !use_tree
                            && (entry.svg.is_none() || entry.svg_version != version || !profile_ok)
                        {
                            let _s =
                                crate::perf::span(crate::perf::Stage::Svg(crate::perf::Kind::Page));
                            match dc::svg::render_shape_layer_opts(
                                doc,
                                page,
                                shape.id,
                                hidden,
                                bbox,
                                skip_text,
                                Some(ppi),
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
                                        entry.tex_bounds = Some(bbox);
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
                            Posted::Queued => {
                                entry.inflight = Some(Inflight {
                                    version,
                                    bucket,
                                    patch: None,
                                });
                            }
                            // Cap deferral: retried next frame (the pool's
                            // deferred flag repainted).
                            Posted::Capped => {}
                        }
                    }
                }
            }
            // Draw whatever texture exists — a stale-bucket texture
            // keeps showing (scaled) until the new raster lands. The
            // quad places at the TEXTURE's rect: a #219 patch keeps
            // the old texture across a bounds-changing edit until the
            // next full render.
            if let Some(tex) = entry.tex.as_ref() {
                self.drawn.insert(shape.id);
                self.stamp += 1;
                entry.last_used = self.stamp;
                quads.push(ShapeQuad {
                    id: shape.id,
                    tex: tex.id(),
                    bbox: entry.tex_bounds.unwrap_or(bbox),
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

/// The #219 patch plan for a localized edit against an entry's
/// existing texture, or `None` when the edit must fall back to a full
/// layer re-render: the damage is unknown/unbounded, the model's ink
/// bounds no longer match the texture's grid (the quad would stretch
/// the patched content — a grown/shrunk layer needs a fresh texture),
/// the damage does not fit the texture, or it exceeds half the texture
/// area (DesignCraft's rule — beyond that the full re-render is the
/// cheaper path). The rect is snapped OUTWARD to whole texture pixels
/// so it always covers the damage.
fn plan_patch(entry: &Entry, bbox: Rect, damage: Rect) -> Option<PatchPlan> {
    let tex = entry.tex.as_ref()?;
    let tb = entry.tex_bounds?;
    // The model bounds must equal the texture's grid: the patch maps
    // model space onto the texture's pixels, which only holds when
    // both describe the same rect.
    const EPS: f64 = 1e-9;
    let same = (bbox.0 - tb.0).abs() < EPS
        && (bbox.1 - tb.1).abs() < EPS
        && (bbox.2 - tb.2).abs() < EPS
        && (bbox.3 - tb.3).abs() < EPS;
    if !same {
        return None;
    }
    let [tw, th] = tex.size();
    if tw == 0 || th == 0 {
        return None;
    }
    let sx = tw as f64 / tb.2.max(1e-9);
    let sy = th as f64 / tb.3.max(1e-9);
    let x0 = (((damage.0 - tb.0) * sx).floor() as i64).clamp(0, tw as i64);
    let y0 = (((damage.1 - tb.1) * sy).floor() as i64).clamp(0, th as i64);
    let x1 = (((damage.0 + damage.2 - tb.0) * sx).ceil() as i64).clamp(0, tw as i64);
    let y1 = (((damage.1 + damage.3 - tb.1) * sy).ceil() as i64).clamp(0, th as i64);
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    if ((x1 - x0) * (y1 - y0)) as f64 > 0.5 * (tw as f64) * (th as f64) {
        return None;
    }
    Some(PatchPlan {
        pos: [x0 as usize, y0 as usize],
        size: [(x1 - x0) as usize, (y1 - y0) as usize],
    })
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
    use dc_core::Command as _;

    fn inflight(version: u64, bucket: u32) -> Inflight {
        Inflight {
            version,
            bucket,
            patch: None,
        }
    }

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
            sub: None,
        });
        layers.touch_damage(&vsdx::Document::default(), &touched, &[], 0);
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
            tex_bounds: None,
            svg: None,
            svg_version: u64::MAX,
            inflight: Some(Inflight {
                version,
                bucket,
                patch: None,
            }),
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
        assert_eq!(
            e.inflight,
            Some(inflight(2, 384)),
            "superseded landing must not take the mark"
        );
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
            Some(inflight(new_base, 384)),
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
            Some(inflight(re_base, 384)),
            "a pre-clear completion must not consume the mark"
        );
        assert!(layers.entries[&7].tex.is_none());
        // The live identity still lands.
        layers.on_shape_done(&ctx, 1, 7, re_base, 384, Ok(image()), None);
        assert!(layers.entries[&7].tex.is_some());
        assert_eq!(layers.entries[&7].version, re_base);
    }

    // ---- #219: sub-shape damage-region patching ----

    /// A plain group (no masters, no guards): root 1 with two member
    /// rects 2 and 3, plus a separate top-level rect 4. The nested-move
    /// and damage-union tests edit member 3 while member 2 pins the
    /// group's extent so the top-level bounds never move.
    fn plain_group_doc() -> vsdx::Document {
        let member = |id: i64, px: f64, py: f64| {
            let mut m = vsdx::model::Shape {
                id,
                type_: Some("Shape".into()),
                ..vsdx::model::Shape::default()
            };
            m.sheet
                .set_cell("PinX", vsdx::model::Cell::value(format!("{px}")));
            m.sheet
                .set_cell("PinY", vsdx::model::Cell::value(format!("{py}")));
            m.sheet.set_cell("Width", vsdx::model::Cell::value("0.3"));
            m.sheet.set_cell("Height", vsdx::model::Cell::value("0.2"));
            m.sheet
                .set_cell("LocPinX", vsdx::model::Cell::value("0.15"));
            m.sheet.set_cell("LocPinY", vsdx::model::Cell::value("0.1"));
            m
        };
        let mut group = vsdx::model::Shape {
            id: 1,
            type_: Some("Group".into()),
            ..vsdx::model::Shape::default()
        };
        group.sheet.set_cell("PinX", vsdx::model::Cell::value("2"));
        group.sheet.set_cell("PinY", vsdx::model::Cell::value("5"));
        group.sheet.set_cell("Width", vsdx::model::Cell::value("1"));
        group
            .sheet
            .set_cell("Height", vsdx::model::Cell::value("0.6"));
        group
            .sheet
            .set_cell("LocPinX", vsdx::model::Cell::value("0.5"));
        group
            .sheet
            .set_cell("LocPinY", vsdx::model::Cell::value("0.3"));
        group.sub_shapes = vec![member(2, 0.15, 0.3), member(3, 0.85, 0.3)];
        let mut top = member(4, 6.0, 5.0);
        top.type_ = Some("Shape".into());
        vsdx::Document {
            pages: vec![vsdx::model::Page {
                id: 0,
                name: "P".into(),
                shapes: vec![group, top],
                ..vsdx::model::Page::default()
            }],
            ..vsdx::Document::default()
        }
    }

    /// Fill the layers of `doc` at 96 ppi through one sync composite.
    /// One context per test: texture ids are per-context, so a fresh
    /// context per frame would make `tex_id_for_test` comparisons
    /// meaningless.
    fn fill(doc: &vsdx::Document, layers: &mut ShapeLayers, ctx: &egui::Context) {
        let mut pool = RasterPool::new(0);
        let page = &doc.pages[0];
        let (quads, status) = layers.composite(
            ctx,
            &mut pool,
            doc,
            page,
            96.0,
            (0.0, 0.0, 8.5, 11.0),
            None,
            &[],
            None,
        );
        assert!(status.is_none());
        assert!(!quads.is_empty(), "the fixture must produce layers");
    }

    /// Harvest + apply + route one SetCell on a (possibly nested)
    /// shape, the exact sequence `App::apply` runs.
    fn apply_setcell(
        ed: &mut dc_core::Editor,
        layers: &mut ShapeLayers,
        page: i64,
        shape: i64,
        cell: &str,
        v: &str,
    ) {
        let cmd = dc_core::SetCell::new(
            dc_core::Scope::Shape { page, shape },
            cell,
            vsdx::model::Cell::value(v),
        );
        let pre_touched = cmd.touched(&ed.doc);
        let pre = layers.harvest_pre_damage(&ed.doc, &pre_touched);
        let (_, touched) = ed.apply(cmd).unwrap();
        layers.touch_damage(&ed.doc, &touched, &pre, page);
    }

    #[test]
    fn localized_nested_edit_patches_instead_of_full_rerender() {
        let (mut ed, page_id, ids) = crate::group_fixture::taskg_editor();
        let [root, _, icon] = ids;
        let mut layers = ShapeLayers::default();
        layers.set_page(page_id);
        let ctx = egui::Context::default();
        fill(&ed.doc, &mut layers, &ctx);
        assert_eq!(layers.full_posts_for_test(), 1);
        let tex_before = layers.tex_id_for_test(root);

        // A bounds-preserving cell edit on the nested Icon sub (the
        // Body sub FILLS the layer — its damage legitimately falls
        // back; the icon is a small region of a master instance).
        apply_setcell(&mut ed, &mut layers, page_id, icon, "FillForegnd", "2");
        fill(&ed.doc, &mut layers, &ctx);
        assert_eq!(
            layers.patch_posts_for_test(),
            1,
            "the localized edit takes the patch path"
        );
        assert_eq!(
            layers.full_posts_for_test(),
            1,
            "no full re-render was posted for the patched layer"
        );
        assert_eq!(layers.patch_landings_for_test(), 1);
        assert_eq!(
            layers.tex_id_for_test(root),
            tex_before,
            "set_partial composites into the SAME texture"
        );
    }

    #[test]
    fn damage_beyond_half_the_texture_falls_back_to_full_rerender() {
        let (mut ed, page_id, ids) = crate::group_fixture::taskg_editor();
        let [root, body, _] = ids;
        let mut layers = ShapeLayers::default();
        layers.set_page(page_id);
        let ctx = egui::Context::default();
        fill(&ed.doc, &mut layers, &ctx);
        let tex_before = layers.tex_id_for_test(root);

        apply_setcell(&mut ed, &mut layers, page_id, body, "FillForegnd", "2");
        // The Body sub fills the group: its damage is ~the whole layer.
        // Widen it explicitly past the half-texture rule.
        let tb = layers.bboxes[&root];
        layers
            .pending_damage
            .insert(root, (tb.0, tb.1, tb.2 * 0.95, tb.3 * 0.95));
        fill(&ed.doc, &mut layers, &ctx);
        assert_eq!(
            layers.patch_posts_for_test(),
            0,
            "damage past half the texture must not patch"
        );
        assert_eq!(layers.full_posts_for_test(), 2);
        assert_ne!(
            layers.tex_id_for_test(root),
            tex_before,
            "the fallback is a real full re-render"
        );
    }

    #[test]
    fn a_bounds_moving_edit_falls_back_to_full_rerender() {
        let (mut ed, page_id, ids) = crate::group_fixture::taskg_editor();
        let [root, body, _] = ids;
        let mut layers = ShapeLayers::default();
        layers.set_page(page_id);
        let ctx = egui::Context::default();
        fill(&ed.doc, &mut layers, &ctx);
        let tex_before = layers.tex_id_for_test(root);

        // The Body sub IS the group's extent: moving it moves the
        // layer's ink bounds — the old texture's grid no longer maps
        // the model, so the patch is refused.
        apply_setcell(&mut ed, &mut layers, page_id, body, "PinX", "0.6");
        fill(&ed.doc, &mut layers, &ctx);
        assert_eq!(layers.patch_posts_for_test(), 0);
        assert_eq!(layers.full_posts_for_test(), 2);
        assert_ne!(layers.tex_id_for_test(root), tex_before);
    }

    #[test]
    fn a_nested_member_move_patches() {
        // Member 3 moves INSIDE the group while member 2 pins the
        // group's extent: the top-level bounds hold, and the move is a
        // localized old∪new damage rect.
        let doc = plain_group_doc();
        let mut ed = dc_core::Editor::new(doc);
        let mut layers = ShapeLayers::default();
        layers.set_page(0);
        let ctx = egui::Context::default();
        fill(&ed.doc, &mut layers, &ctx);
        let tex_before = layers.tex_id_for_test(1);

        apply_setcell(&mut ed, &mut layers, 0, 3, "PinX", "0.7");
        fill(&ed.doc, &mut layers, &ctx);
        assert_eq!(
            layers.patch_posts_for_test(),
            1,
            "a member move within stable group bounds patches"
        );
        assert_eq!(
            layers.full_posts_for_test(),
            2,
            "only the initial fills (group + sibling) posted fulls"
        );
        assert_eq!(layers.tex_id_for_test(1), tex_before);
    }

    #[test]
    fn damages_union_across_edits_before_the_next_composite() {
        // Two transitions of member 3 land before one composite: the
        // pending region must cover BOTH — the texture still shows the
        // first edit's pre-state.
        let doc = plain_group_doc();
        let mut ed = dc_core::Editor::new(doc);
        let mut layers = ShapeLayers::default();
        layers.set_page(0);
        let ctx = egui::Context::default();
        fill(&ed.doc, &mut layers, &ctx);

        apply_setcell(&mut ed, &mut layers, 0, 3, "PinX", "0.6");
        apply_setcell(&mut ed, &mut layers, 0, 3, "PinX", "0.85");
        let d = layers.pending_damage[&1];
        // The union spans the member at 0.6 AND at 0.85: its width
        // covers both positions plus the stroke pad.
        let member_w = 0.3 + 2.0 * 0.08;
        assert!(
            d.2 > member_w + 0.22,
            "the union must span both member positions, got w={}",
            d.2
        );
    }

    #[test]
    fn undo_pairs_against_the_transition_cache() {
        let doc = plain_group_doc();
        let mut ed = dc_core::Editor::new(doc);
        let mut layers = ShapeLayers::default();
        layers.set_page(0);
        let ctx = egui::Context::default();
        fill(&ed.doc, &mut layers, &ctx);

        // The apply writes the post-transition bounds into the cache;
        // the undo (no pre list) pairs against it — a bounded damage,
        // not the conservative full re-render.
        apply_setcell(&mut ed, &mut layers, 0, 3, "PinX", "0.7");
        assert!(layers.pending_damage.contains_key(&1));
        let (_, touched) = ed.undo().unwrap().unwrap();
        layers.touch_damage(&ed.doc, &touched, &[], 0);
        assert!(
            !layers.pending_full.contains(&1),
            "the undo must pair against the cached post-apply bounds"
        );
        assert!(layers.pending_damage.contains_key(&1));
        fill(&ed.doc, &mut layers, &ctx);
        assert_eq!(layers.patch_posts_for_test(), 1);
    }

    #[test]
    fn a_superseded_patch_completion_never_touches_the_texture() {
        let ctx = egui::Context::default();
        let mut layers = ShapeLayers::default();
        layers.set_page(0);
        layers.entries.insert(7, entry_inflight(1, 384));
        layers.on_shape_done(&ctx, 0, 7, 1, 384, Ok(image_px(4, 4)), None);
        let tex_before = layers.entries[&7].tex.as_ref().map(|t| t.id());
        // A patch job goes in flight (identity (5, 384, patch)).
        let plan = PatchPlan {
            pos: [0, 0],
            size: [2, 2],
        };
        layers.entries.get_mut(&7).unwrap().inflight = Some(Inflight {
            version: 5,
            bucket: 384,
            patch: Some(plan),
        });

        // A completion for an OLDER full job (superseded while the
        // patch flew) is discarded without consuming the mark.
        assert!(layers
            .on_shape_done(&ctx, 0, 7, 4, 384, Ok(image_px(4, 4)), None)
            .is_none());
        assert_eq!(
            layers.entries[&7].inflight,
            Some(Inflight {
                version: 5,
                bucket: 384,
                patch: Some(plan)
            })
        );

        // The live patch lands: set_partial into the SAME texture —
        // identity, size and byte accounting unchanged.
        assert!(layers
            .on_shape_done(&ctx, 0, 7, 5, 384, Ok(image_px(2, 2)), None)
            .is_none());
        let e = &layers.entries[&7];
        assert_eq!(e.inflight, None);
        assert_eq!(
            e.tex.as_ref().map(|t| t.id()),
            tex_before,
            "a patch keeps the texture"
        );
        assert_eq!(e.version, 5);
        assert_eq!(e.bytes, 64, "the texture size is unchanged");
        assert_eq!(layers.patch_landings_for_test(), 1);
        assert_eq!(layers.tex_bytes, 64);
    }

    /// #219 acceptance gate (DesignCraft's patch-vs-full pattern): the
    /// damage-region render must be pixel-identical to the
    /// corresponding region of a whole-layer render at the same
    /// texture dimensions. The patch renders the SAME parsed tree
    /// through `render_tree`'s own scale post-translated by an INTEGER
    /// pixel offset — an integer shift moves the sample grid by whole
    /// pixels, so the rasters agree byte for byte.
    #[test]
    fn patch_region_pixels_equal_the_full_layer_render() {
        crate::svgimg::pin_embedded_fonts_for_tests();
        let (mut ed, page_id, ids) = crate::group_fixture::taskg_editor();
        let [root, body, icon] = ids;
        // Text on the Body sub: the font-shaped path is the risky half.
        let page_i = ed.doc.pages.iter().position(|p| p.id == page_id).unwrap();
        let body_shape = ed.doc.pages[page_i].shapes[0]
            .sub_shapes
            .iter_mut()
            .find(|s| s.id == body)
            .unwrap();
        body_shape.sheet.text = Some("Hello patch".into());
        body_shape.sheet.text_raw = Some("Hello patch".into());

        let doc = &ed.doc;
        let page = &doc.pages[page_i];
        let bounds = dc::svg::shape_ink_bounds(doc, page, root).unwrap();
        let width = (bounds.2 as f32 * 96.0).clamp(1.0, 2048.0);
        let (_, svg) =
            dc::svg::render_shape_layer_opts(doc, page, root, &[], bounds, None, Some(96.0))
                .unwrap();
        let kind = crate::perf::Kind::Page;
        let tree = crate::svgimg::parse_svg(&svg, kind).unwrap();
        let full = crate::svgimg::render_tree(&tree, width, kind).unwrap();
        let [tw, th] = full.size;
        assert!(
            tw > 32 && th > 32,
            "fixture layer must raster to a real texture"
        );

        // The damage rect the store would plan for an icon edit, snapped
        // outward to whole texture pixels (the planner's own rule; the
        // planner itself is pinned by the decision tests above).
        let dmg = dc::svg::shape_ink_bounds_focused(doc, page, root, Some(icon)).unwrap();
        let sx = tw as f64 / bounds.2;
        let sy = th as f64 / bounds.3;
        let x0 = (((dmg.0 - bounds.0) * sx).floor() as i64).clamp(0, tw as i64) as i32;
        let y0 = (((dmg.1 - bounds.1) * sy).floor() as i64).clamp(0, th as i64) as i32;
        let x1 = (((dmg.0 + dmg.2 - bounds.0) * sx).ceil() as i64).clamp(0, tw as i64) as i32;
        let y1 = (((dmg.1 + dmg.3 - bounds.1) * sy).ceil() as i64).clamp(0, th as i64) as i32;
        let (pw, ph) = ((x1 - x0) as u32, (y1 - y0) as u32);
        assert!(pw > 4 && ph > 4, "the patch rect must be a real region");

        // The patch: render_tree's own scale for (tw, th), then the
        // integer crop shift — exactly what RasterJob::ParseRegion runs.
        let size = tree.size();
        let t = resvg::usvg::Transform::from_row(
            tw as f32 / size.width().max(1e-3),
            0.0,
            0.0,
            th as f32 / size.height().max(1e-3),
            -(x0 as f32),
            -(y0 as f32),
        );
        let patch = crate::svgimg::render_tree_transform(&tree, t, pw, ph, kind).unwrap();

        // Byte-exact: patch(i, j) == full(x0 + i, y0 + j).
        for j in 0..ph as usize {
            for i in 0..pw as usize {
                let a = patch.pixels[j * (pw as usize) + i];
                let b = full.pixels[(j + y0 as usize) * tw + (i + x0 as usize)];
                assert_eq!(a, b, "patch pixel ({i}, {j}) diverges from the full render");
            }
        }
    }
}
