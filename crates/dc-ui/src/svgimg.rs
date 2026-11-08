//! SVG → texture rasterization for the canvas, with a small cache.
//! The GUI deliberately renders through the same SVG backend the CLI
//! uses (build plan M5: "rendering via the SVG backend first"), so what
//! you see is what `dc render` produces.

use egui::{ColorImage, Context, TextureHandle, TextureId, TextureOptions};

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};

/// Kittest/snapshot mode: resolve every font-family to the embedded
/// Carlito instead of the installed system faces, so the goldens
/// render identically on every machine (CI runners have no MS fonts;
/// dev workstations do). Set once by the kittest harnesses before the
/// first frame; the shipped app never sets it.
static PIN_EMBEDDED: AtomicBool = AtomicBool::new(false);

/// Test hook for the kittest harnesses (see `PIN_EMBEDDED`).
#[doc(hidden)]
pub fn pin_embedded_fonts_for_tests() {
    PIN_EMBEDDED.store(true, Ordering::Relaxed);
}

/// Kittest/snapshot mode #2 (GitHub #92): force ALL rasterization
/// synchronous — the raster pool runs jobs inline on the calling
/// thread, so goldens never depend on worker timing (a miss either
/// has its texture this frame or the version genuinely changed,
/// never because a worker was slow). Set once by the kittest
/// harnesses; there is deliberately no way to unset it.
static FORCE_SYNC: AtomicBool = AtomicBool::new(false);

/// Test hook for the kittest harnesses (see `FORCE_SYNC`).
#[doc(hidden)]
pub fn force_sync_raster_for_tests() {
    FORCE_SYNC.store(true, Ordering::Relaxed);
}

pub(crate) fn sync_forced() -> bool {
    FORCE_SYNC.load(Ordering::Relaxed)
}

/// One-call kittest determinism: pinned fonts + synchronous rasters
/// + wrap metrics from the embedded face only (#21 — without the
/// metrics pin, a machine with different installed fonts would wrap
/// lines differently inside the very SVG the pinned raster draws).
#[doc(hidden)]
pub fn pin_determinism_for_tests() {
    pin_embedded_fonts_for_tests();
    force_sync_raster_for_tests();
    dc::textmetrics::pin_embedded_for_tests();
}

/// Rasterize an SVG string to a `target_width`-pixel-wide RGBA image
/// (height follows the viewBox aspect). The resolution, database and
/// parse itself live in `dc::raster` — the shared export/canvas stack
/// (#35); this half adds the GUI's kittest pin and the egui image.
///
/// Fonts: the SVG backend emits `font-family` fallback chains per text
/// element (GitHub #17); `dc::raster::parse_svg` resolves the first
/// installed family and pins unresolvable chains to the embedded
/// Carlito. System fonts stay available as per-glyph fallback for
/// scripts Carlito lacks (CJK, Arabic, …).
///
/// Determinism: the kittest goldens run under `pin_embedded_fonts_for_tests`
/// (vendored faces only), and their fixture documents carry no Character
/// Font cells, so they render the same on every machine; real
/// documents render with the user's real fonts.
pub fn parse_svg(svg: &str, kind: crate::perf::Kind) -> anyhow::Result<resvg::usvg::Tree> {
    let db = dc::raster::font_db();
    let pinned = PIN_EMBEDDED.load(Ordering::Relaxed);
    let resolved = {
        let _s = crate::perf::span(crate::perf::Stage::Resolve(kind));
        dc::raster::resolve_svg(svg, &db, pinned)
    };
    let _s = crate::perf::span(crate::perf::Stage::Parse(kind));
    dc::raster::parse_resolved(&resolved, &db)
}

/// Render a parsed tree at a `target_width`-pixel-wide RGBA image
/// (height follows the viewBox aspect) — the per-width half of the
/// pipeline (#91): pixmap alloc, transform, render, RGBA copy.
pub fn render_tree(
    tree: &resvg::usvg::Tree,
    target_width: f32,
    kind: crate::perf::Kind,
) -> anyhow::Result<ColorImage> {
    // Widest texture we ever allocate (kittest caps at 2048; GPUs
    // allow more but upscaling a 2048px raster is fine on screen).
    const MAX_TEX: u32 = 2048;
    let size = tree.size();
    let aspect = size.height().max(1e-3) / size.width().max(1e-3);
    let w = target_width.ceil().clamp(1.0, MAX_TEX as f32) as u32;
    let h = ((target_width * aspect).ceil().max(1.0)) as u32;
    let (w, h) = (w, h.min(MAX_TEX));
    let t = resvg::usvg::Transform::from_scale(
        w as f32 / size.width().max(1e-3),
        h as f32 / size.height().max(1e-3),
    );
    render_tree_transform(tree, t, w, h, kind)
}

/// Render a parsed tree through an explicit transform into a
/// `w`×`h` pixmap — [`render_tree`]'s inner half, exposed for the
/// #219 damage-region patcher: the patch renders the SAME
/// (post-edit, full-layer) tree with [`render_tree`]'s own scale for
/// the existing texture's dimensions, post-translated by an INTEGER
/// pixel offset. An integer shift moves the sample grid by whole
/// pixels, so the patch's pixels are bit-identical to the
/// corresponding region of a full re-render at the same size — the
/// regression test in `layers` pins that identity.
pub fn render_tree_transform(
    tree: &resvg::usvg::Tree,
    t: resvg::usvg::Transform,
    w: u32,
    h: u32,
    kind: crate::perf::Kind,
) -> anyhow::Result<ColorImage> {
    let _r = crate::perf::span(crate::perf::Stage::Render(kind));
    let pixmap = dc::raster::render_pixmap(tree, t, w, h)?;
    let data: &[u8] = pixmap.data();
    Ok(ColorImage::from_rgba_unmultiplied(
        [w as usize, h as usize],
        data,
    ))
}

/// Rasterize an SVG string in one shot — parse + render. The
/// fixed-width callers (stencil thumbnails, icons) pay this once per
/// cached texture; the page path goes through [`TextureCache`], which
/// reuses the parsed tree across raster widths (#91).
pub fn rasterize(
    svg: &str,
    target_width: f32,
    kind: crate::perf::Kind,
) -> anyhow::Result<ColorImage> {
    render_tree(&parse_svg(svg, kind)?, target_width, kind)
}

/// Cache of page rasters keyed by (page part, model version, px-per-inch
/// bucket). Any model mutation bumps `version`, invalidating everything.
/// The parsed `usvg::Tree` is cached per (part, version) — the
/// width-independent half of the raster (#91) — so a ppi-bucket change
/// (a zoom step) re-RENDERS the held tree instead of re-resolving and
/// re-parsing the SVG. The parse/render itself runs on the raster pool
/// (#92): a miss posts a job and returns `None` (paint the
/// placeholder) until the completion lands in `App`'s pool drain.
#[derive(Default)]
pub struct TextureCache {
    map: HashMap<(String, u64, u32), TextureHandle>,
    trees: HashMap<(String, u64), crate::rasterpool::Tree>,
    /// Raster misses queued on the pool, not yet landed (#92).
    inflight_rasters: std::collections::HashSet<(String, u64, u32)>,
    /// (part, version) whose parse job is on the pool (#92).
    inflight_trees: std::collections::HashSet<(String, u64)>,
    /// Landed worker failures — surfaced once as the `Err`, then
    /// retried by re-posting (the pre-#92 per-frame retry semantics).
    failures: HashMap<(String, u64, u32), String>,
}

impl TextureCache {
    /// Raster for `(key_part, version, px_per_in)`, building the SVG via
    /// `build_svg` ONLY when the parsed tree is missing (#90/#91): the
    /// cache — not the caller — decides when the page is serialized and
    /// parsed. A texture miss with a live tree (a ppi-bucket change)
    /// re-renders that tree at the new width; the build closure and the
    /// resolve/parse stages run at most once per (part, version), never
    /// per frame. Under the async pool (#92) a miss returns `Ok(None)`
    /// until the worker's result lands (`on_page_done`); with the sync
    /// seam (kittest) the job runs inline and the handle is back this
    /// frame. A failed build/parse inserts nothing (the failure shows
    /// once, then the next frame retries).
    #[allow(clippy::too_many_arguments)]
    pub fn get(
        &mut self,
        ctx: &Context,
        pool: &mut crate::rasterpool::RasterPool,
        page_w_in: f64,
        key_part: &str,
        version: u64,
        px_per_in: f32,
        mut build_svg: impl FnMut() -> anyhow::Result<String>,
    ) -> anyhow::Result<Option<&TextureHandle>> {
        let bucket = (px_per_in * 4.0).round().max(4.0) as u32; // ±0.25 ppi
        let key = (key_part.to_string(), version, bucket);
        if !self.map.contains_key(&key) {
            if let Some(msg) = self.failures.remove(&key) {
                return Err(anyhow::anyhow!(msg));
            }
            if self.inflight_rasters.contains(&key) {
                return Ok(None); // on a worker; lands in a later drain
            }
            let tree_key = (key_part.to_string(), version);
            let width = page_w_in as f32 * px_per_in;
            let job = if let Some(tree) = self.trees.get(&tree_key).cloned() {
                // Bucket change with a live tree: render only — no
                // rebuild, no resolve, no parse (#91).
                crate::rasterpool::RasterJob::Render { tree, width }
            } else if self.inflight_trees.contains(&tree_key) {
                // The parse is on a worker; this bucket's render posts
                // on a later frame, once the tree has landed.
                return Ok(None);
            } else {
                let svg = build_svg()?;
                self.inflight_trees.insert(tree_key.clone());
                crate::rasterpool::RasterJob::Parse { svg, width }
            };
            use crate::rasterpool::Posted;
            match pool.post(
                crate::rasterpool::RasterKey::Page {
                    part: key_part.to_string(),
                    version,
                    bucket,
                },
                job,
            ) {
                Posted::Sync(image, tree) => {
                    // The job already ran — clear the pre-post mark so a
                    // failed parse (which never inserts a tree) is not
                    // parked in-flight forever, and surface failures
                    // directly: the sync seam keeps the pre-#92
                    // semantics of one inline attempt per get.
                    self.inflight_trees.remove(&tree_key);
                    if let Err(e) = &image {
                        return Err(anyhow::anyhow!("{e:#}"));
                    }
                    self.finish(ctx, key.clone(), tree_key, image, tree);
                }
                Posted::Queued => {
                    self.inflight_rasters.insert(key.clone());
                }
                Posted::Capped => {
                    // The job never ran; undo the tree mark so the
                    // retry next frame reposts it.
                    self.inflight_trees.remove(&tree_key);
                    return Ok(None);
                }
            }
        }
        // Keep at most a few live entries per page; drop stale versions —
        // the trees with the same policy (one live tree per page entry).
        self.map
            .retain(|(p, v, _), _| p != key_part || *v == version);
        self.trees
            .retain(|(p, v), _| p != key_part || *v == version);
        Ok(self.map.get(&key))
    }

    /// Upload a finished image (or record its failure) and cache a
    /// landed tree.
    fn finish(
        &mut self,
        ctx: &Context,
        key: (String, u64, u32),
        tree_key: (String, u64),
        image: anyhow::Result<ColorImage>,
        tree: Option<crate::rasterpool::Tree>,
    ) {
        if let Some(t) = tree {
            self.trees.insert(tree_key, t);
        }
        match image {
            Ok(image) => {
                let tex = {
                    let _s = crate::perf::span(crate::perf::Stage::Upload(crate::perf::Kind::Page));
                    ctx.load_texture(
                        format!("page-{}-{}", key.0, key.1),
                        image,
                        TextureOptions::LINEAR,
                    )
                };
                self.map.insert(key, tex);
            }
            Err(e) => {
                self.failures.insert(key, format!("{e:#}"));
            }
        }
    }

    /// Land a worker completion for a page key (#92) — called from
    /// `App`'s pool drain. Results for stale versions insert
    /// harmlessly and are evicted by the next `get`'s retain.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn on_page_done(
        &mut self,
        ctx: &Context,
        part: String,
        version: u64,
        bucket: u32,
        done: anyhow::Result<ColorImage>,
        tree: Option<crate::rasterpool::Tree>,
        was_parse: bool,
    ) {
        let key = (part, version, bucket);
        self.inflight_rasters.remove(&key);
        let tree_key = (key.0.clone(), key.1);
        if was_parse {
            self.inflight_trees.remove(&tree_key);
        }
        self.finish(ctx, key, tree_key, done, tree);
    }

    pub fn clear(&mut self) {
        self.map.clear();
        self.trees.clear();
        self.inflight_rasters.clear();
        self.inflight_trees.clear();
        self.failures.clear();
    }

    /// The live texture id for a cache key, if it has been rasterized
    /// (test observability for #58's thumbnail invalidation).
    pub fn id_for(&self, key_part: &str, version: u64, px_per_in: f32) -> Option<TextureId> {
        let bucket = (px_per_in * 4.0).round().max(4.0) as u32; // ±0.25 ppi
        self.map
            .get(&(key_part.to_string(), version, bucket))
            .map(|t| t.id())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Face stability under the pinned (kittest) mode: the pinned
    /// database resolves the VENDORED faces (#145), so an Arial chain
    /// rasterizes the vendored Liberation Sans (identically on every
    /// machine — the goldens' determinism) and the default/Calibri
    /// chain rasterizes the vendored Carlito. What may never happen
    /// is a chain leaking onto a system face under the pin.
    #[test]
    fn rasterized_text_is_face_stable_when_pinned() {
        pin_embedded_fonts_for_tests();
        let head = r#"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="50"><text x="10" y="35" font-size="24""#;
        let tail = ">Hamburg fi 51</text></svg>";
        let arial_chain =
            format!(r#"{head} font-family="Arial, 'Liberation Sans', Carlito, sans-serif"{tail}"#);
        let explicit_liberation = format!(r#"{head} font-family="Liberation Sans"{tail}"#);
        let explicit_carlito = format!(r#"{head} font-family="Carlito"{tail}"#);
        let calibri_chain = format!(r#"{head} font-family="Calibri, Carlito, sans-serif"{tail}"#);
        let bare = format!(r#"{head}{tail}"#);
        let a = rasterize(&arial_chain, 200.0, crate::perf::Kind::Page).unwrap();
        let l = rasterize(&explicit_liberation, 200.0, crate::perf::Kind::Page).unwrap();
        let b = rasterize(&explicit_carlito, 200.0, crate::perf::Kind::Page).unwrap();
        let d = rasterize(&calibri_chain, 200.0, crate::perf::Kind::Page).unwrap();
        let c = rasterize(&bare, 200.0, crate::perf::Kind::Page).unwrap();
        assert_eq!(a.size, l.size);
        assert_eq!(
            a.pixels, l.pixels,
            "Arial chain == vendored Liberation Sans"
        );
        assert_ne!(
            a.pixels, b.pixels,
            "Liberation and Carlito are different faces — the chain must not silently fall to Carlito"
        );
        assert_eq!(d.pixels, b.pixels, "default-family chain == Carlito");
        // The no-family default is Carlito (`Options::font_family`).
        assert_eq!(c.pixels, b.pixels, "no-family default == Carlito");
    }

    /// Machine-dependent by design (GitHub #17's requirement: honor
    /// installed fonts): asserts the workstation's MS core fonts are
    /// found through the same shared database the app builds. Run
    /// explicitly on a box that has them:
    /// `cargo test -p dc-ui --lib -- --ignored`
    #[test]
    #[ignore = "asserts fonts installed on this workstation"]
    fn installed_ms_fonts_resolve_on_this_machine() {
        let db = dc::raster::font_db();
        assert_eq!(
            dc::raster::installed_family(&db, "Arial").as_deref(),
            Some("Arial")
        );
        assert_eq!(
            dc::raster::installed_family(&db, "Calibri").as_deref(),
            Some("Calibri")
        );
    }

    /// A minimal parseable page: usvg needs a size to derive the raster
    /// height from.
    fn tiny_svg() -> anyhow::Result<String> {
        Ok(r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><rect width="10" height="10" fill="red"/></svg>"#.into())
    }

    /// #90: the build closure runs at most once per (part, version) —
    /// repeated `get`s with the same key pay only the lookup, and only
    /// a version bump, another page, or eviction of the stale version
    /// builds again. A failed build must not be cached.
    #[test]
    fn build_closure_runs_once_per_key() {
        use std::cell::Cell;
        let ctx = Context::default();
        let mut pool = crate::rasterpool::RasterPool::new(0); // inline, no threads
        let mut cache = TextureCache::default();
        // The closure owns the counter (Cell so asserts can read it
        // between calls).
        let calls = Cell::new(0u32);
        let mut build = || {
            calls.set(calls.get() + 1);
            tiny_svg()
        };
        // Same key three times — the third inside the same ±0.25 ppi
        // bucket — one build.
        for &ppi in &[96.0f32, 96.0, 96.05] {
            cache
                .get(&ctx, &mut pool, 4.0, "page1", 1, ppi, &mut build)
                .unwrap()
                .unwrap();
        }
        assert_eq!(calls.get(), 1, "cached gets must not rebuild the SVG");
        // Version bump → rebuild (the SVG may have changed).
        cache
            .get(&ctx, &mut pool, 4.0, "page1", 2, 96.0, &mut build)
            .unwrap()
            .unwrap();
        assert_eq!(calls.get(), 2);
        // New ppi bucket, same version → NO rebuild: the parsed tree is
        // reused and only re-rendered (#91).
        cache
            .get(&ctx, &mut pool, 4.0, "page1", 2, 300.0, &mut build)
            .unwrap()
            .unwrap();
        assert_eq!(calls.get(), 2, "a bucket change must not re-serialize");
        // Another page part builds under its own key.
        cache
            .get(&ctx, &mut pool, 4.0, "page2", 2, 300.0, &mut build)
            .unwrap()
            .unwrap();
        assert_eq!(calls.get(), 3);
        // The v1 page1 entry was evicted by the retain on the v2 get, so
        // asking for it again rebuilds (eviction policy unchanged, #90).
        cache
            .get(&ctx, &mut pool, 4.0, "page1", 1, 96.0, &mut build)
            .unwrap()
            .unwrap();
        assert_eq!(calls.get(), 4);
        // A failed build inserts nothing: the very next get retries.
        let mut failing = || -> anyhow::Result<String> {
            calls.set(calls.get() + 1);
            Err(anyhow::anyhow!("boom"))
        };
        assert!(cache
            .get(&ctx, &mut pool, 4.0, "badpage", 1, 96.0, &mut failing)
            .is_err());
        assert!(cache
            .get(&ctx, &mut pool, 4.0, "badpage", 1, 96.0, &mut failing)
            .is_err());
        assert_eq!(calls.get(), 6, "failed builds are not cached");
        assert!(
            cache.id_for("badpage", 1, 96.0).is_none(),
            "no texture may exist for a failed build"
        );
        // A parse failure is not cached either (the tree inserts only
        // after a successful parse).
        let mut unparseable = || -> anyhow::Result<String> {
            calls.set(calls.get() + 1);
            Ok("<not-svg".into())
        };
        assert!(cache
            .get(&ctx, &mut pool, 4.0, "badpage2", 1, 96.0, &mut unparseable)
            .is_err());
        assert!(cache
            .get(&ctx, &mut pool, 4.0, "badpage2", 1, 96.0, &mut unparseable)
            .is_err());
        assert_eq!(calls.get(), 8, "failed parses are not cached");
    }

    /// #91: rasterizing one tree at several widths costs one build +
    /// one parse total. The build closure is the only SVG source, so it
    /// never re-invoking across buckets proves the tree was reused
    /// (and the differing texture widths prove each bucket really
    /// rasterized) — observable without the global perf counters.
    #[test]
    fn bucket_sweep_parses_once() {
        use std::cell::Cell;
        let ctx = Context::default();
        let mut pool = crate::rasterpool::RasterPool::new(0); // inline, no threads
        let mut cache = TextureCache::default();
        let calls = Cell::new(0u32);
        let mut build = || {
            calls.set(calls.get() + 1);
            tiny_svg()
        };
        for &ppi in &[96.0f32, 192.0, 300.0] {
            cache
                .get(&ctx, &mut pool, 4.0, "page1", 7, ppi, &mut build)
                .unwrap()
                .unwrap();
        }
        assert_eq!(calls.get(), 1, "three buckets, one build+parse");
        // Each bucket rasterized at its own width: 4 in × ppi.
        let mut width = |ppi: f32, build: &mut dyn FnMut() -> anyhow::Result<String>| {
            cache
                .get(&ctx, &mut pool, 4.0, "page1", 7, ppi, build)
                .unwrap()
                .unwrap()
                .size()[0]
        };
        assert_eq!(width(96.0, &mut build), 384);
        assert_eq!(width(192.0, &mut build), 768);
        assert_eq!(width(300.0, &mut build), 1200);
        assert_eq!(calls.get(), 1, "re-gets still build nothing");
    }
}
