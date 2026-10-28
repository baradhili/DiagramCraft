//! Off-thread rasterization (GitHub #92, #66 sub-issue 4).
//!
//! The expensive half of the raster pipeline — font resolve, usvg
//! parse, resvg render (`parse_svg` + `render_tree`) — runs on a
//! couple of parked worker threads; the UI thread only does the cheap
//! `ctx.load_texture` when completions drain at the start of a frame.
//! Jobs are plain `(key, svg-or-tree, width)` in, `ColorImage` out,
//! so a worker result crosses back cleanly, and a Parse job also
//! returns the parsed tree — #91's parse-once / render-per-bucket
//! reuse survives the move off-thread.
//!
//! Determinism seam: `svgimg::force_sync_raster_for_tests()` makes
//! `post` run every job inline on the calling thread (see
//! `svgimg::pin_embedded_fonts_for_tests` for the pattern), so the
//! kittest goldens never depend on worker timing — a miss either has
//! its texture this frame (sync) or paints the placeholder because
//! the version genuinely changed, never because a worker was slow.

use crate::svgimg::{self, render_tree, parse_svg};
use egui::ColorImage;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};

/// Parsed tree shared between the cache and Render jobs.
pub(crate) type Tree = Arc<resvg::usvg::Tree>;

/// Which cache slot a completed raster fills.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub(crate) enum RasterKey {
    /// A page (or Info) texture: `(key_part, version, bucket)` — the
    /// `TextureCache` map key.
    Page {
        part: String,
        version: u64,
        bucket: u32,
    },
    /// A stencil master thumbnail, keyed by the source's session
    /// serial + master id — master ids alone collide across stencils
    /// (#63/#85; serial 0 is the document's own master set).
    Thumb { serial: u64, master: i64 },
    /// A per-shape canvas layer (#93): `RasterKey::Shape { page, id,
    /// version, bucket }`. The version/bucket travel with the job so
    /// `on_shape_done` can discard a superseded completion (a newer
    /// job was posted while this one was in flight) without consuming
    /// the live job's inflight mark.
    Shape {
        page: i64,
        id: i64,
        version: u64,
        bucket: u32,
    },
}

impl RasterKey {
    fn kind(&self) -> crate::perf::Kind {
        match self {
            RasterKey::Page { .. } | RasterKey::Shape { .. } => crate::perf::Kind::Page,
            RasterKey::Thumb { .. } => crate::perf::Kind::Thumb,
        }
    }
}

/// What to rasterize.
#[derive(Clone)]
pub(crate) enum RasterJob {
    /// Resolve + parse + render — the full pipeline for a new SVG
    /// string. Returns the image AND the parsed tree.
    Parse { svg: String, width: f32 },
    /// Re-render an already-parsed tree at a new width (#91 bucket
    /// change; the SVG string is gone, only the tree is reused).
    Render { tree: Tree, width: f32 },
}

/// One completed job.
pub(crate) struct RasterDone {
    pub key: RasterKey,
    pub image: anyhow::Result<ColorImage>,
    /// The tree from a `Parse` job (`None` for `Render` jobs — the
    /// tree is already cached — and for a FAILED parse).
    pub tree: Option<Tree>,
    /// True for `Parse` jobs, successful or not: the caller's
    /// tree-in-flight mark must clear either way.
    pub was_parse: bool,
}

/// The outcome of `RasterPool::post`.
pub(crate) enum Posted {
    /// Sync seam: the job already ran inline — here is its result.
    Sync(anyhow::Result<ColorImage>, Option<Tree>),
    /// Queued on a worker; the result lands in `drain()`.
    Queued,
    /// The per-frame post cap is exhausted; retry next frame (the
    /// pool already asked for a repaint).
    Capped,
}

/// At most this many jobs are QUEUED per frame (#92: memory stays
/// bounded on huge documents; the stencil pane populates
/// progressively). Sync posts bypass the cap — inline execution is
/// the deterministic pre-#92 behavior the goldens pin. 32 since #93:
/// per-shape layer jobs are small, and a big page's progressive fill
/// should complete in ~a second, not crawl at 8/frame.
const PER_FRAME_POST_CAP: usize = 32;

fn run(key: RasterKey, job: RasterJob) -> RasterDone {
    let kind = key.kind();
    let was_parse = matches!(job, RasterJob::Parse { .. });
    let (image, tree) = match job {
        RasterJob::Parse { svg, width } => match parse_svg(&svg, kind) {
            Ok(tree) => {
                let tree = Arc::new(tree);
                (render_tree(&tree, width, kind), Some(tree as Tree))
            }
            Err(e) => (Err(e), None),
        },
        RasterJob::Render { tree, width } => (render_tree(&tree, width, kind), None),
    };
    RasterDone { key, image, tree, was_parse }
}

/// A tiny fixed worker pool. Idle workers park on the work channel
/// (no spinning); dropping the pool closes the channel and joins them.
pub struct RasterPool {
    tx: Option<Sender<(RasterKey, RasterJob)>>,
    rx_done: Receiver<RasterDone>,
    workers: Vec<std::thread::JoinHandle<()>>,
    posted_this_frame: usize,
    /// High-water mark of `posted_this_frame` between drains — the
    /// most jobs any single frame ever queued (the #92 async test
    /// asserts this stays at the cap; a count-after-N-frames assert
    /// would race mount's run-to-stable pump, which is
    /// machine-speed-dependent).
    peak_posts_per_frame: usize,
    /// Lifetime count of queued jobs (sync/inline posts excluded).
    total_posts: u64,
    /// A post was cap-rejected since the last drain — App turns this
    /// into a repaint so the deferred post isn't stuck waiting for
    /// input.
    deferred: bool,
}

impl Default for RasterPool {
    fn default() -> Self {
        // No worker threads in sync-forced (kittest) processes — posts
        // run inline anyway (see `post`), so don't strand idle threads.
        Self::new(if svgimg::sync_forced() { 0 } else { 2 })
    }
}

impl RasterPool {
    /// `workers == 0` builds a pool with no threads (every post runs
    /// inline); used by tests and as the base of the sync seam.
    pub fn new(workers: usize) -> Self {
        if workers == 0 {
            return Self::inline();
        }
        let (tx_done, rx_done) = channel::<RasterDone>();
        let (tx, rx_work) = channel::<(RasterKey, RasterJob)>();
        let rx_work = Arc::new(Mutex::new(rx_work));
        let workers = (0..workers)
            .map(|_| {
                let rx_work = Arc::clone(&rx_work);
                let tx_done = tx_done.clone();
                std::thread::spawn(move || loop {
                    // Scoped lock: recv parks without holding it.
                    let job = rx_work.lock().unwrap().recv();
                    match job {
                        Ok((key, job)) => {
                            if tx_done.send(run(key, job)).is_err() {
                                break; // pool dropped
                            }
                        }
                        Err(_) => break, // channel closed: shut down
                    }
                })
            })
            .collect();
        Self {
            tx: Some(tx),
            rx_done,
            workers,
            posted_this_frame: 0,
            peak_posts_per_frame: 0,
            total_posts: 0,
            deferred: false,
        }
    }

    /// Inline-only pool (no threads).
    fn inline() -> Self {
        let (_tx_done, rx_done) = channel::<RasterDone>();
        Self {
            tx: None,
            rx_done,
            workers: Vec::new(),
            posted_this_frame: 0,
            peak_posts_per_frame: 0,
            total_posts: 0,
            deferred: false,
        }
    }

    /// Queue (or inline) a raster job.
    pub(crate) fn post(&mut self, key: RasterKey, job: RasterJob) -> Posted {
        // The determinism seam (#92): tests force synchronous
        // rasterization regardless of how this pool was built.
        if svgimg::sync_forced() || self.tx.is_none() {
            let RasterDone { image, tree, .. } = run(key, job);
            return Posted::Sync(image, tree);
        }
        if self.posted_this_frame >= PER_FRAME_POST_CAP {
            self.deferred = true;
            return Posted::Capped;
        }
        self.posted_this_frame += 1;
        self.peak_posts_per_frame = self.peak_posts_per_frame.max(self.posted_this_frame);
        self.total_posts += 1;
        // A send failure means every worker died (a resvg panic would
        // have taken the thread down); run the real job inline so the
        // raster still lands rather than silently never arriving.
        if self
            .tx
            .as_ref()
            .unwrap()
            .send((key.clone(), job.clone()))
            .is_err()
        {
            let RasterDone { image, tree, .. } = run(key, job);
            return Posted::Sync(image, tree);
        }
        Posted::Queued
    }

    /// Land every completion currently queued, resetting the per-frame
    /// post budget and the deferred flag. Call once per frame, before
    /// layout.
    pub(crate) fn drain(&mut self) -> Vec<RasterDone> {
        self.posted_this_frame = 0;
        self.deferred = false;
        let mut out = Vec::new();
        while let Ok(done) = self.rx_done.try_recv() {
            out.push(done);
        }
        out
    }

    /// Whether a cap-rejected post is waiting for the next frame.
    pub fn take_deferred(&mut self) -> bool {
        std::mem::take(&mut self.deferred)
    }

    /// The per-frame post cap (test hook: the async contract asserts
    /// against the real constant, not a copy).
    pub fn post_cap(&self) -> usize {
        PER_FRAME_POST_CAP
    }

    /// Most jobs any single frame ever queued (test hook, #92).
    pub fn debug_peak_posts_per_frame(&self) -> usize {
        self.peak_posts_per_frame
    }

    /// Lifetime queued-job count, sync/inline posts excluded (test
    /// hook, #92).
    pub fn debug_total_posts(&self) -> u64 {
        self.total_posts
    }

    /// Whether `post` would queue right now (per-frame budget left).
    /// Callers with expensive job construction (serializing a layer
    /// SVG) probe this first so the cap defers the WORK, not just the
    /// post.
    pub(crate) fn has_budget(&self) -> bool {
        self.tx.is_none() || svgimg::sync_forced() || self.posted_this_frame < PER_FRAME_POST_CAP
    }
}

impl Drop for RasterPool {
    fn drop(&mut self) {
        self.tx = None; // close the work channel: workers exit
        for w in self.workers.drain(..) {
            let _ = w.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tiny_svg() -> String {
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><rect width="10" height="10" fill="red"/></svg>"#.into()
    }

    fn page_key(part: &str) -> RasterKey {
        RasterKey::Page { part: part.into(), version: 1, bucket: 384 }
    }

    /// Drain with a deadline: worker completions land asynchronously,
    /// so poll until the expected count arrives (or fail). The budget
    /// is generous — parallel test threads can starve the worker for
    /// seconds on a loaded machine.
    fn drain_until(pool: &mut RasterPool, want: usize) -> Vec<RasterDone> {
        for _ in 0..600 {
            let got = pool.drain();
            if got.len() >= want {
                return got;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        panic!("no completion after 12s");
    }

    #[test]
    fn one_worker_pool_round_trips_a_parse_job_with_the_tree() {
        let mut pool = RasterPool::new(1);
        let key = page_key("p1");
        match pool.post(
            key.clone(),
            RasterJob::Parse { svg: tiny_svg(), width: 100.0 },
        ) {
            Posted::Queued => {}
            _ => panic!("expected Queued"),
        }
        let done = drain_until(&mut pool, 1).remove(0);
        assert_eq!(done.key, key);
        let img = done.image.unwrap();
        assert_eq!(img.size, [100, 100]);
        assert!(done.tree.is_some(), "a Parse job returns the tree for #91 reuse");
        // The returned tree re-renders at another width (Render job).
        let tree = done.tree.unwrap();
        match pool.post(
            page_key("p1"),
            RasterJob::Render { tree, width: 50.0 },
        ) {
            Posted::Queued => {}
            _ => panic!("expected Queued"),
        }
        let done = drain_until(&mut pool, 1).remove(0);
        assert_eq!(done.image.unwrap().size, [50, 50]);
        assert!(done.tree.is_none(), "Render jobs do not re-return the tree");
    }

    #[test]
    fn per_frame_cap_defers_then_clears_on_drain() {
        let mut pool = RasterPool::new(1);
        // Completions land whenever the worker finishes — accumulate
        // across every drain instead of assuming none arrive early
        // (the mid-test budget-reset drain discards what it finds).
        let mut landed = Vec::new();
        for i in 0..PER_FRAME_POST_CAP {
            assert!(matches!(
                pool.post(
                    page_key(&format!("p{i}")),
                    RasterJob::Parse { svg: tiny_svg(), width: 10.0 }
                ),
                Posted::Queued
            ));
        }
        assert!(matches!(
            pool.post(
                page_key("over"),
                RasterJob::Parse { svg: tiny_svg(), width: 10.0 }
            ),
            Posted::Capped
        ));
        assert!(pool.take_deferred(), "the capped post flags a repaint");
        assert!(!pool.take_deferred(), "the flag is one-shot");
        landed.extend(pool.drain()); // resets the budget
        assert!(matches!(
            pool.post(
                page_key("over"),
                RasterJob::Parse { svg: tiny_svg(), width: 10.0 }
            ),
            Posted::Queued
        ));
        for _ in 0..600 {
            landed.extend(pool.drain());
            if landed.len() > PER_FRAME_POST_CAP {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert_eq!(
            landed.len(),
            PER_FRAME_POST_CAP + 1,
            "every posted job must complete"
        );
    }

    /// The zero-thread pool runs inline — the shape the sync seam
    /// takes when tests force synchronous rasterization.
    #[test]
    fn inline_pool_is_synchronous() {
        let mut pool = RasterPool::inline();
        match pool.post(
            page_key("p1"),
            RasterJob::Parse { svg: tiny_svg(), width: 64.0 },
        ) {
            Posted::Sync(image, tree) => {
                assert_eq!(image.unwrap().size, [64, 64]);
                assert!(tree.is_some());
            }
            _ => panic!("expected Sync"),
        }
        assert!(pool.drain().is_empty(), "nothing was queued");
    }
}
