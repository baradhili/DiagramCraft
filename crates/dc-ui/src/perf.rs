//! Render-path stage counters (GitHub #89).
//!
//! Cheap, always-on cumulative timers around the canvas pipeline
//! stages: full-page SVG serialization, font-chain resolve, usvg
//! parse, resvg render, texture upload — each attributed to its
//! context (page, stencil thumbnail, icon) — plus the whole-frame
//! span. The headless bench (`tests/render_perf.rs`) resets and reads
//! them; a production frame pays one `Instant::now` and two relaxed
//! atomic adds per recorded stage, noise next to the work measured.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

/// Which rasterization context a stage belongs to.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    Page,
    Thumb,
    Icon,
}

const KINDS: [Kind; 3] = [Kind::Page, Kind::Thumb, Kind::Icon];

/// One measured pipeline stage.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Stage {
    /// One `App::draw` (≈ one frame under the harnesses).
    Frame,
    /// SVG string built (`render_page_with` / `render_master`).
    Svg(Kind),
    /// Font-family chains resolved (string rewrite).
    Resolve(Kind),
    /// `usvg::Tree::from_str`.
    Parse(Kind),
    /// Pixmap alloc + `resvg::render` + RGBA copy.
    Render(Kind),
    /// `ctx.load_texture` upload.
    Upload(Kind),
}

const SLOTS: usize = 1 + 5 * 3;
static COUNT: [AtomicU64; SLOTS] = [const { AtomicU64::new(0) }; SLOTS];
static NS: [AtomicU64; SLOTS] = [const { AtomicU64::new(0) }; SLOTS];

fn kind_idx(kind: Kind) -> usize {
    match kind {
        Kind::Page => 0,
        Kind::Thumb => 1,
        Kind::Icon => 2,
    }
}

impl Stage {
    fn slot(self) -> usize {
        match self {
            Stage::Frame => 0,
            Stage::Svg(k) => 1 + kind_idx(k),
            Stage::Resolve(k) => 4 + kind_idx(k),
            Stage::Parse(k) => 7 + kind_idx(k),
            Stage::Render(k) => 10 + kind_idx(k),
            Stage::Upload(k) => 13 + kind_idx(k),
        }
    }

    fn from_slot(slot: usize) -> Stage {
        match slot {
            0 => Stage::Frame,
            1..=3 => Stage::Svg(KINDS[slot - 1]),
            4..=6 => Stage::Resolve(KINDS[slot - 4]),
            7..=9 => Stage::Parse(KINDS[slot - 7]),
            10..=12 => Stage::Render(KINDS[slot - 10]),
            _ => Stage::Upload(KINDS[slot - 13]),
        }
    }

    /// Report name (`"page/parse"`, `"frame"`, …).
    pub fn name(self) -> &'static str {
        match self {
            Stage::Frame => "frame",
            Stage::Svg(k) => match k {
                Kind::Page => "page/svg",
                Kind::Thumb => "thumb/svg",
                Kind::Icon => "icon/svg",
            },
            Stage::Resolve(k) => match k {
                Kind::Page => "page/resolve",
                Kind::Thumb => "thumb/resolve",
                Kind::Icon => "icon/resolve",
            },
            Stage::Parse(k) => match k {
                Kind::Page => "page/parse",
                Kind::Thumb => "thumb/parse",
                Kind::Icon => "icon/parse",
            },
            Stage::Render(k) => match k {
                Kind::Page => "page/render",
                Kind::Thumb => "thumb/render",
                Kind::Icon => "icon/render",
            },
            Stage::Upload(k) => match k {
                Kind::Page => "page/upload",
                Kind::Thumb => "thumb/upload",
                Kind::Icon => "icon/upload",
            },
        }
    }
}

/// Times a stage until drop (RAII; `#[must_use]` so a bare call that
/// would measure nothing is caught at compile time).
#[must_use = "the span records on drop; bind it for the region to measure"]
pub struct Span {
    stage: Stage,
    t: Instant,
}

impl Drop for Span {
    fn drop(&mut self) {
        let slot = self.stage.slot();
        NS[slot].fetch_add(self.t.elapsed().as_nanos() as u64, Ordering::Relaxed);
        COUNT[slot].fetch_add(1, Ordering::Relaxed);
    }
}

/// Start timing a stage.
pub fn span(stage: Stage) -> Span {
    Span { stage, t: Instant::now() }
}

/// Zero every counter (test/bench use; safe mid-run).
pub fn reset() {
    for c in COUNT.iter() {
        c.store(0, Ordering::Relaxed);
    }
    for n in NS.iter() {
        n.store(0, Ordering::Relaxed);
    }
}

/// One stage's cumulative count and nanoseconds.
#[derive(Copy, Clone, Debug)]
pub struct Stat {
    pub stage: &'static str,
    pub count: u64,
    pub ns: u64,
}

/// Read every stage, in fixed slot order (zero-count stages included;
/// callers filter).
pub fn snapshot() -> Vec<Stat> {
    (0..SLOTS)
        .map(|slot| Stat {
            stage: Stage::from_slot(slot).name(),
            count: COUNT[slot].load(Ordering::Relaxed),
            ns: NS[slot].load(Ordering::Relaxed),
        })
        .collect()
}

/// `after - before` per stage (both from [`snapshot`], same order).
pub fn diff(before: &[Stat], after: &[Stat]) -> Vec<Stat> {
    before
        .iter()
        .zip(after)
        .map(|(b, a)| Stat {
            stage: a.stage,
            count: a.count.saturating_sub(b.count),
            ns: a.ns.saturating_sub(b.ns),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spans_count_and_accumulate() {
        reset();
        {
            let _s = span(Stage::Frame);
        }
        let one = snapshot();
        assert_eq!(one[0].count, 1);
        assert!(one[0].ns > 0);
        let before = snapshot();
        {
            let _s = span(Stage::Frame);
        }
        let d = diff(&before, &snapshot());
        assert_eq!(d[0].count, 1);
        reset();
        assert_eq!(snapshot()[0].count, 0);
    }
}
