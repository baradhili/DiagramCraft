//! The snap & glue engine (GitHub #109, FS-015): per-target snap and
//! glue configuration persisted in the document's `DocumentSettings`,
//! a snap engine that translates a dragged shape's alignment box onto
//! the enabled targets (ruler subdivisions, grid, guides, alignment
//! boxes, shape extensions, shape intersections, connection points),
//! glue-target snapping for endpoint drags (guides, shape handles,
//! shape vertices, connection points), and the dynamic-grid alignment
//! and spacing hints shown while dragging (URS-1088).
//!
//! Pure math over page-space inches (y-up) — no egui types, so every
//! rule is unit-testable; the app adapts model geometry in and hint
//! geometry out. Guides have no object yet (#113's slice); the engine
//! consumes them the day they exist.

use vsdx::model::{glue_bits, snap_bits, DocSnapGlue};

/// Snap & glue toggles, one per Visio target (URS-1100/URS-1112),
/// persisted through [`DocSnapGlue`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SnapSettings {
    pub snap_active: bool,
    pub snap_ruler_subdivisions: bool,
    pub snap_grid: bool,
    pub snap_guides: bool,
    pub snap_alignment_boxes: bool,
    pub snap_shape_extensions: bool,
    pub snap_shape_intersections: bool,
    pub snap_connection_points: bool,
    pub glue_active: bool,
    pub glue_guides: bool,
    pub glue_shape_handles: bool,
    pub glue_shape_vertices: bool,
    pub glue_connection_points: bool,
    /// The dynamic grid aid (URS-1088) — alignment/spacing hints while
    /// dragging, independent of whether snapping itself is on.
    pub dynamic_grid: bool,
}

impl Default for SnapSettings {
    fn default() -> Self {
        Self::from_doc(DocSnapGlue::CORPUS_DEFAULT)
    }
}

impl SnapSettings {
    /// From the document's persisted bitmasks (the shipped 2016 corpus
    /// defaults when the document says nothing — see `DocSnapGlue`).
    /// Visio has no separate "active" bit: `Active` off is a zero
    /// mask, so a zero mask reads back as inactive.
    pub fn from_doc(d: DocSnapGlue) -> Self {
        let s = d.snap;
        let g = d.glue;
        SnapSettings {
            snap_active: s != 0,
            snap_ruler_subdivisions: s & snap_bits::RULER_SUBDIVISIONS != 0,
            snap_grid: s & snap_bits::GRID != 0,
            snap_guides: s & snap_bits::GUIDES != 0,
            snap_alignment_boxes: s & snap_bits::ALIGNMENT_BOXES != 0,
            snap_shape_extensions: s & snap_bits::SHAPE_EXTENSIONS != 0,
            snap_shape_intersections: s & snap_bits::SHAPE_INTERSECTIONS != 0,
            snap_connection_points: s & snap_bits::CONNECTION_POINTS != 0,
            glue_active: g != 0,
            glue_guides: g & glue_bits::GUIDES != 0,
            glue_shape_handles: g & glue_bits::SHAPE_HANDLES != 0,
            glue_shape_vertices: g & glue_bits::SHAPE_VERTICES != 0,
            glue_connection_points: g & glue_bits::CONNECTION_POINTS != 0,
            dynamic_grid: d.dynamic_grid,
        }
    }

    /// The persisted form: an inactive side collapses to a zero mask
    /// (Visio's representation of "Active" off).
    pub fn to_doc(self) -> DocSnapGlue {
        let snap = if self.snap_active {
            (self.snap_ruler_subdivisions as u32 * snap_bits::RULER_SUBDIVISIONS)
                | (self.snap_grid as u32 * snap_bits::GRID)
                | (self.snap_guides as u32 * snap_bits::GUIDES)
                | (self.snap_alignment_boxes as u32 * snap_bits::ALIGNMENT_BOXES)
                | (self.snap_shape_extensions as u32 * snap_bits::SHAPE_EXTENSIONS)
                | (self.snap_shape_intersections as u32 * snap_bits::SHAPE_INTERSECTIONS)
                | (self.snap_connection_points as u32 * snap_bits::CONNECTION_POINTS)
        } else {
            0
        };
        let glue = if self.glue_active {
            (self.glue_guides as u32 * glue_bits::GUIDES)
                | (self.glue_shape_handles as u32 * glue_bits::SHAPE_HANDLES)
                | (self.glue_shape_vertices as u32 * glue_bits::SHAPE_VERTICES)
                | (self.glue_connection_points as u32 * glue_bits::CONNECTION_POINTS)
        } else {
            0
        };
        DocSnapGlue {
            snap,
            glue,
            dynamic_grid: self.dynamic_grid,
        }
    }
}

/// One non-dragged shape as the snap engine sees it: its alignment box
/// (page inches, y-up, origin at the bottom-left), geometry vertices,
/// and connection points, all in page space.
#[derive(Debug, Clone, Default)]
pub struct SnapTarget {
    pub id: i64,
    pub rect: (f64, f64, f64, f64),
    pub vertices: Vec<(f64, f64)>,
    pub connection_points: Vec<(f64, f64)>,
}

impl SnapTarget {
    /// The shape's snap-relevant x coordinates: left, center, right.
    fn xs(&self) -> [f64; 3] {
        [
            self.rect.0,
            self.rect.0 + self.rect.2 / 2.0,
            self.rect.0 + self.rect.2,
        ]
    }
    /// The shape's snap-relevant y coordinates: bottom, center, top.
    fn ys(&self) -> [f64; 3] {
        [
            self.rect.1,
            self.rect.1 + self.rect.3 / 2.0,
            self.rect.1 + self.rect.3,
        ]
    }
}

/// A drawing guide (#113's guides slice feeding the #109 engine): a
/// full-width/height line or a guide point, carrying the guide
/// shape's id so glue provenance can name it.
#[derive(Debug, Clone, Copy)]
pub enum Guide {
    /// A horizontal guide constrains y; a vertical one constrains x.
    Line {
        id: i64,
        horizontal: bool,
        at: f64,
    },
    /// A guide point constrains both axes.
    Point { id: i64, x: f64, y: f64 },
}

/// Everything a snap decision needs, in page inches (y-up).
#[derive(Debug, Clone, Copy)]
pub struct SnapCtx<'a> {
    pub targets: &'a [SnapTarget],
    pub guides: &'a [Guide],
    /// Displayed-grid line spacing (5 mm in display units).
    pub grid_step_in: f64,
    /// The finest ruler subdivision interval currently drawn.
    pub ruler_step_in: f64,
    /// Snap distance: a candidate farther than this (inches) never
    /// wins. The app converts the screen threshold (Visio snap
    /// strength, in px) through the current zoom.
    pub threshold_in: f64,
}

/// Which target class produced a snap — provenance for the
/// dynamic-grid hint and status notes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SnapHit {
    Grid,
    Ruler,
    Guide(i64),
    AlignmentBox(i64),
    ShapeExtension(i64),
    /// A crossing of one target's x-extension with a target's
    /// y-extension (the dotted crosses Visio draws).
    ShapeIntersection,
    ConnectionPoint(i64),
    /// A target alignment-box selection handle (glue side).
    ShapeHandle(i64),
    /// A target geometry vertex (glue side).
    ShapeVertex(i64),
}

/// The outcome of snapping a dragged shape's live alignment box.
#[derive(Debug, Clone, Copy, Default)]
pub struct SnapResult {
    /// Translation to add to the drag delta (0.0 on a free axis).
    pub dx: f64,
    pub dy: f64,
    pub x_hit: Option<(f64, SnapHit)>,
    pub y_hit: Option<(f64, SnapHit)>,
}

/// The current best candidate for one axis.
struct AxisCand {
    at: f64,
    cost: f64,
    hit: SnapHit,
}

/// Weigh one axis candidate against the current best: strictly
/// cheaper wins, so enumeration order (specific geometry first,
/// abstract grid last) decides exact ties.
fn consider_axis(
    at: f64,
    hit: SnapHit,
    sources: &[f64; 3],
    threshold: f64,
    best: &mut Option<AxisCand>,
) {
    for &src in sources {
        let d = (at - src).abs();
        if d <= threshold && best.as_ref().is_none_or(|b| d < b.cost) {
            *best = Some(AxisCand { at, cost: d, hit });
        }
    }
}

/// Snap a dragged shape's live alignment box (`rect`, page inches
/// y-up): each axis independently finds the cheapest
/// source-feature → target pair within the threshold. Grid and ruler
/// targets are infinite line families — their candidate is the
/// nearest step multiple per source feature — while alignment boxes
/// are finite lines that require overlap on the other axis, and
/// extensions cross without limit. Point targets (shape
/// intersections, connection points) join the same per-axis contest
/// through their coordinates.
pub fn snap_translation(rect: (f64, f64, f64, f64), ctx: &SnapCtx, s: &SnapSettings) -> SnapResult {
    if !s.snap_active {
        return SnapResult::default();
    }
    let sources_x = [rect.0, rect.0 + rect.2 / 2.0, rect.0 + rect.2];
    let sources_y = [rect.1, rect.1 + rect.3 / 2.0, rect.1 + rect.3];
    let mut best_x: Option<AxisCand> = None;
    let mut best_y: Option<AxisCand> = None;

    for line in axis_lines(ctx, s) {
        if let Some((lo, hi)) = line.span {
            // A finite line (alignment-box edge) only counts when the
            // dragged box overlaps it on the other axis.
            let (lo_d, hi_d) = if line.vertical {
                (rect.1, rect.1 + rect.3)
            } else {
                (rect.0, rect.0 + rect.2)
            };
            if hi_d < lo || lo_d > hi {
                continue;
            }
        }
        if line.vertical {
            consider_axis(line.at, line.hit, &sources_x, ctx.threshold_in, &mut best_x);
        } else {
            consider_axis(line.at, line.hit, &sources_y, ctx.threshold_in, &mut best_y);
        }
    }
    for (enabled, step, hit) in [
        (s.snap_grid, ctx.grid_step_in, SnapHit::Grid),
        (s.snap_ruler_subdivisions, ctx.ruler_step_in, SnapHit::Ruler),
    ] {
        if !enabled || !step.is_finite() || step <= 1e-6 {
            continue;
        }
        for src in sources_x {
            consider_axis(
                round_to(src, step),
                hit,
                &sources_x,
                ctx.threshold_in,
                &mut best_x,
            );
        }
        for src in sources_y {
            consider_axis(
                round_to(src, step),
                hit,
                &sources_y,
                ctx.threshold_in,
                &mut best_y,
            );
        }
    }
    if s.snap_shape_intersections || s.snap_connection_points {
        for (p, hit) in point_targets(ctx, s.snap_shape_intersections, s.snap_connection_points) {
            consider_axis(p.0, hit, &sources_x, ctx.threshold_in, &mut best_x);
            consider_axis(p.1, hit, &sources_y, ctx.threshold_in, &mut best_y);
        }
    }
    if s.snap_guides {
        for g in ctx.guides {
            if let Guide::Point { id, x, y } = *g {
                consider_axis(x, SnapHit::Guide(id), &sources_x, ctx.threshold_in, &mut best_x);
                consider_axis(y, SnapHit::Guide(id), &sources_y, ctx.threshold_in, &mut best_y);
            }
        }
    }
    let mut res = SnapResult::default();
    if let Some(c) = best_x {
        res.dx = c.at - nearest(sources_x, c.at);
        res.x_hit = Some((c.at, c.hit));
    }
    if let Some(c) = best_y {
        res.dy = c.at - nearest(sources_y, c.at);
        res.y_hit = Some((c.at, c.hit));
    }
    res
}

/// The best point-snap candidate so far: (snapped point, hit, cost).
type PtBest = Option<((f64, f64), SnapHit, f64)>;

/// Snap a single dragged point (a 1D endpoint under glue settings, a
/// geometry vertex under snap settings) to the enabled targets.
/// Line targets keep the free coordinate; point targets snap both.
/// Returns the snapped point and what it landed on.
pub fn point_snap(
    p: (f64, f64),
    ctx: &SnapCtx,
    s: &SnapSettings,
    glue: bool,
) -> Option<((f64, f64), SnapHit)> {
    let mut best: PtBest = None;
    let consider_line =
        |at: f64, vertical: bool, hit: SnapHit, span: Option<(f64, f64)>, best: &mut PtBest| {
            if let Some((lo, hi)) = span {
                let other = if vertical { p.1 } else { p.0 };
                if other < lo || other > hi {
                    return;
                }
            }
            let (snapped, d) = if vertical {
                ((at, p.1), (at - p.0).abs())
            } else {
                ((p.0, at), (at - p.1).abs())
            };
            if d <= ctx.threshold_in && best.as_ref().is_none_or(|(_, _, c)| d < *c) {
                *best = Some((snapped, hit, d));
            }
        };
    let consider_point = |q: (f64, f64), hit: SnapHit, best: &mut PtBest| {
        let d = (q.0 - p.0).hypot(q.1 - p.1);
        if d <= ctx.threshold_in && best.as_ref().is_none_or(|(_, _, c)| d < *c) {
            *best = Some((q, hit, d));
        }
    };

    if glue {
        if !s.glue_active {
            return None;
        }
        if s.glue_guides {
            for g in ctx.guides {
                match *g {
                    Guide::Line { id, horizontal, at } => {
                        consider_line(at, !horizontal, SnapHit::Guide(id), None, &mut best);
                    }
                    Guide::Point { id, x, y } => {
                        consider_point((x, y), SnapHit::Guide(id), &mut best);
                    }
                }
            }
        }
        for t in ctx.targets {
            if s.glue_shape_handles {
                // Visio's selection handles: four corners, four edge
                // midpoints.
                let (x0, y0, w, h) = t.rect;
                let (xc, yc) = (x0 + w / 2.0, y0 + h / 2.0);
                for q in [
                    (x0, y0),
                    (x0 + w, y0),
                    (x0, y0 + h),
                    (x0 + w, y0 + h),
                    (xc, y0),
                    (xc, y0 + h),
                    (x0, yc),
                    (x0 + w, yc),
                ] {
                    consider_point(q, SnapHit::ShapeHandle(t.id), &mut best);
                }
            }
            if s.glue_shape_vertices {
                for v in &t.vertices {
                    consider_point(*v, SnapHit::ShapeVertex(t.id), &mut best);
                }
            }
            if s.glue_connection_points {
                for c in &t.connection_points {
                    consider_point(*c, SnapHit::ConnectionPoint(t.id), &mut best);
                }
            }
        }
    } else {
        if !s.snap_active {
            return None;
        }
        for line in axis_lines(ctx, s) {
            consider_line(line.at, line.vertical, line.hit, line.span, &mut best);
        }
        for (enabled, step, hit) in [
            (s.snap_grid, ctx.grid_step_in, SnapHit::Grid),
            (s.snap_ruler_subdivisions, ctx.ruler_step_in, SnapHit::Ruler),
        ] {
            if !enabled || !step.is_finite() || step <= 1e-6 {
                continue;
            }
            consider_line(round_to(p.0, step), true, hit, None, &mut best);
            consider_line(round_to(p.1, step), false, hit, None, &mut best);
        }
        for (q, hit) in point_targets(ctx, s.snap_shape_intersections, s.snap_connection_points) {
            consider_point(q, hit, &mut best);
        }
        if s.snap_guides {
            for g in ctx.guides {
                if let Guide::Point { id, x, y } = *g {
                    consider_point((x, y), SnapHit::Guide(id), &mut best);
                }
            }
        }
    }
    best.map(|(p, hit, _)| (p, hit))
}

/// A dynamic-grid hint, in page inches (y-up).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Hint {
    /// A full-canvas dashed alignment line: vertical at `x = at`,
    /// horizontal at `y = at`.
    Align { vertical: bool, at: f64 },
    /// Equal-spacing gaps around the dragged shape: two segments
    /// (x0, y0, x1, y1) and their shared length.
    Spacing {
        a: (f64, f64, f64, f64),
        b: (f64, f64, f64, f64),
        len_in: f64,
    },
}

/// Dynamic-grid feedback (URS-1088): dashed alignment lines where the
/// dragged box's centers/edges line up with other shapes', and
/// equal-spacing gaps when it sits evenly between two neighbors.
/// Independent of `snap_active` — the aid is a view-time dressing,
/// matching Visio's separate Dynamic Grid toggle.
pub fn dynamic_hints(rect: (f64, f64, f64, f64), ctx: &SnapCtx) -> Vec<Hint> {
    let mut hints = Vec::new();
    let dxs = [rect.0, rect.0 + rect.2 / 2.0, rect.0 + rect.2];
    let dys = [rect.1, rect.1 + rect.3 / 2.0, rect.1 + rect.3];
    // ONE alignment line per axis (#168): Visio's dynamic grid shows
    // the alignment the shape is actually approaching. The previous
    // per-pair emission drew a line for EVERY within-threshold
    // coincidence between the dragged box's 3 positions and every
    // target's 3 — a dense, grid-aligned page lit up with a stack of
    // lines at neighbouring shapes' internal offsets. Keep the
    // NEAREST coincidence per axis; ties break to the smaller
    // coordinate so the choice is deterministic. Both sides stay
    // top-level bboxes only (targets are the page's top-level
    // shapes; sub-shapes never contribute).
    for vertical in [true, false] {
        let mut best: Option<(f64, f64)> = None; // (|delta|, at)
        for t in ctx.targets {
            let (drag, tgt) = if vertical {
                (dxs, t.xs())
            } else {
                (dys, t.ys())
            };
            for (&ds, &ts) in drag.iter().zip(tgt.iter()) {
                let d = (ds - ts).abs();
                if d <= ctx.threshold_in
                    && best.is_none_or(|(bd, ba)| d < bd || (d == bd && ts < ba))
                {
                    best = Some((d, ts));
                }
            }
        }
        if let Some((_, at)) = best {
            hints.push(Hint::Align { vertical, at });
        }
    }
    // Equal spacing: the dragged box evenly between the nearest
    // neighbor on each side of an axis, gaps equal within a quarter of
    // the snap threshold (sub-visual slack, like Visio's feel).
    let tol = (ctx.threshold_in / 4.0).max(1e-3);
    let (x0, y0, w, h) = rect;
    for vertical in [false, true] {
        let (drag_lo, drag_hi) = if vertical {
            (y0, y0 + h)
        } else {
            (x0, x0 + w)
        };
        let axis_extent = |t: &SnapTarget| {
            if vertical {
                (t.rect.1, t.rect.1 + t.rect.3)
            } else {
                (t.rect.0, t.rect.0 + t.rect.2)
            }
        };
        let cross = |t: &SnapTarget| {
            if vertical {
                t.rect.0 + t.rect.2 / 2.0
            } else {
                t.rect.1 + t.rect.3 / 2.0
            }
        };
        let mut before: Option<(f64, f64)> = None; // (gap, cross pos)
        let mut after: Option<(f64, f64)> = None;
        for t in ctx.targets {
            let (lo, hi) = axis_extent(t);
            if hi <= drag_lo {
                let g = drag_lo - hi;
                if before.as_ref().is_none_or(|(cg, _)| g < *cg) {
                    before = Some((g, cross(t)));
                }
            } else if lo >= drag_hi {
                let g = lo - drag_hi;
                if after.as_ref().is_none_or(|(cg, _)| g < *cg) {
                    after = Some((g, cross(t)));
                }
            }
        }
        if let (Some((gb, _pb)), Some((ga, pa))) = (before, after) {
            if (gb - ga).abs() <= tol && gb > 1e-4 {
                let (a, b) = if vertical {
                    let cx = x0 + w / 2.0;
                    (
                        (cx, y0 - gb, cx, y0),
                        (pa, y0 + h, pa, y0 + h + ga),
                    )
                } else {
                    let cy = y0 + h / 2.0;
                    (
                        (x0 - gb, cy, x0, cy),
                        (x0 + w, cy, x0 + w + ga, cy),
                    )
                };
                hints.push(Hint::Spacing {
                    a,
                    b,
                    len_in: (gb + ga) / 2.0,
                });
            }
        }
    }
    hints
}

/// The enumerable line targets: alignment-box edges (finite),
/// shape extensions (infinite), and guides — in priority order,
/// specific geometry first.
fn axis_lines(ctx: &SnapCtx, s: &SnapSettings) -> Vec<LineTarget> {
    let mut lines = Vec::new();
    for t in ctx.targets {
        for x in t.xs() {
            if s.snap_alignment_boxes {
                lines.push(LineTarget {
                    vertical: true,
                    at: x,
                    span: Some((t.rect.1, t.rect.1 + t.rect.3)),
                    hit: SnapHit::AlignmentBox(t.id),
                });
            }
            if s.snap_shape_extensions {
                lines.push(LineTarget {
                    vertical: true,
                    at: x,
                    span: None,
                    hit: SnapHit::ShapeExtension(t.id),
                });
            }
        }
        for y in t.ys() {
            if s.snap_alignment_boxes {
                lines.push(LineTarget {
                    vertical: false,
                    at: y,
                    span: Some((t.rect.0, t.rect.0 + t.rect.2)),
                    hit: SnapHit::AlignmentBox(t.id),
                });
            }
            if s.snap_shape_extensions {
                lines.push(LineTarget {
                    vertical: false,
                    at: y,
                    span: None,
                    hit: SnapHit::ShapeExtension(t.id),
                });
            }
        }
    }
    if s.snap_guides {
        for g in ctx.guides {
            if let Guide::Line { id, horizontal, at } = *g {
                lines.push(LineTarget {
                    vertical: !horizontal,
                    at,
                    span: None,
                    hit: SnapHit::Guide(id),
                });
            }
        }
    }
    lines
}

/// Point targets: shape-extension crossings, connection points, and
/// (gated by the caller's snap-guides flag handled at the call sites)
/// guide points.
fn point_targets(
    ctx: &SnapCtx,
    intersections: bool,
    connection_points: bool,
) -> Vec<((f64, f64), SnapHit)> {
    let mut pts = Vec::new();
    if intersections {
        for a in ctx.targets {
            for b in ctx.targets {
                for x in a.xs() {
                    for y in b.ys() {
                        pts.push(((x, y), SnapHit::ShapeIntersection));
                    }
                }
            }
        }
    }
    if connection_points {
        for t in ctx.targets {
            for c in &t.connection_points {
                pts.push((*c, SnapHit::ConnectionPoint(t.id)));
            }
        }
    }
    pts
}

struct LineTarget {
    vertical: bool,
    at: f64,
    span: Option<(f64, f64)>,
    hit: SnapHit,
}

/// Nearest step multiple (`(v/step).round() * step`, 0-safe).
fn round_to(v: f64, step: f64) -> f64 {
    (v / step).round() * step
}

/// The source value nearest to `v` (the feature a target coordinate
/// actually snapped to).
fn nearest(values: [f64; 3], v: f64) -> f64 {
    values
        .iter()
        .copied()
        .min_by(|a, b| {
            (a - v)
                .abs()
                .partial_cmp(&(b - v).abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .unwrap_or(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    const THRESH: f64 = 0.08; // ~8 px at 100 %

    fn ctx<'a>(targets: &'a [SnapTarget], guides: &'a [Guide]) -> SnapCtx<'a> {
        SnapCtx {
            targets,
            guides,
            grid_step_in: 5.0 / 25.4, // 5 mm
            ruler_step_in: 1.0 / 25.4, // 1 mm subdivision
            threshold_in: THRESH,
        }
    }

    fn box_at(x: f64, y: f64, w: f64, h: f64) -> (f64, f64, f64, f64) {
        (x, y, w, h)
    }

    #[test]
    fn settings_roundtrip_corpus_defaults() {
        let s = SnapSettings::default();
        // 295 = grid + ruler + guides + connection points + extensions
        assert!(s.snap_active && s.snap_grid && s.snap_ruler_subdivisions);
        assert!(s.snap_guides && s.snap_connection_points && s.snap_shape_extensions);
        assert!(!s.snap_alignment_boxes && !s.snap_shape_intersections);
        // 9 = guides + connection points
        assert!(s.glue_active && s.glue_guides && s.glue_connection_points);
        assert!(!s.glue_shape_handles && !s.glue_shape_vertices);
        assert!(s.dynamic_grid);
        assert_eq!(s.to_doc(), DocSnapGlue::CORPUS_DEFAULT);
    }

    #[test]
    fn inactive_side_persists_a_zero_mask() {
        let s = SnapSettings {
            snap_active: false,
            glue_active: false,
            ..SnapSettings::default()
        };
        let d = s.to_doc();
        assert_eq!(d.snap, 0);
        assert_eq!(d.glue, 0);
        let back = SnapSettings::from_doc(d);
        assert!(!back.snap_active && !back.glue_active);
    }

    #[test]
    fn snap_disabled_is_a_no_op() {
        let s = SnapSettings {
            snap_active: false,
            ..SnapSettings::default()
        };
        let targets = [SnapTarget {
            id: 1,
            rect: box_at(0.0, 0.0, 1.0, 1.0),
            ..Default::default()
        }];
        let r = snap_translation(box_at(0.95, 0.0, 1.0, 1.0), &ctx(&targets, &[]), &s);
        assert_eq!(r.dx, 0.0);
        assert_eq!(r.dy, 0.0);
        assert!(r.x_hit.is_none() && r.y_hit.is_none());
    }

    #[test]
    fn grid_snaps_to_the_nearest_multiple() {
        let s = SnapSettings {
            snap_alignment_boxes: false,
            snap_shape_extensions: false,
            snap_connection_points: false,
            snap_ruler_subdivisions: false,
            ..SnapSettings::default()
        };
        // Box chosen so the LEFT edge is the only feature near a 5 mm
        // multiple (center and right are off by more than the
        // threshold), making the snap unambiguous.
        let r = snap_translation(box_at(0.190, 0.0, 1.03, 1.0), &ctx(&[], &[]), &s);
        let five_mm = 5.0 / 25.4;
        assert!((r.dx - (five_mm - 0.190)).abs() < 1e-9);
        assert_eq!(r.x_hit.as_ref().map(|(_, h)| *h), Some(SnapHit::Grid));
    }

    #[test]
    fn alignment_box_needs_axis_overlap() {
        let s = SnapSettings {
            snap_grid: false,
            snap_shape_extensions: false,
            snap_connection_points: false,
            snap_ruler_subdivisions: false,
            snap_alignment_boxes: true,
            ..SnapSettings::default()
        };
        // Target sits at x∈[0,1]; dragged box left edge 0.95 is within
        // the threshold of x=1 but the boxes never overlap in y, so
        // the finite alignment edge is not a target…
        let targets = [SnapTarget {
            id: 7,
            rect: box_at(0.0, 5.0, 1.0, 1.0),
            ..Default::default()
        }];
        let r = snap_translation(box_at(0.95, 0.0, 1.0, 1.0), &ctx(&targets, &[]), &s);
        assert!(r.x_hit.is_none(), "no y-overlap → no alignment snap");
        // …while the same box overlapping in y snaps its left edge to
        // the target's right edge.
        let targets = [SnapTarget {
            id: 7,
            rect: box_at(0.0, 0.2, 1.0, 1.0),
            ..Default::default()
        }];
        let r = snap_translation(box_at(0.95, 0.0, 1.0, 1.0), &ctx(&targets, &[]), &s);
        assert_eq!(r.x_hit.as_ref().map(|(at, h)| (*at, *h)), Some((1.0, SnapHit::AlignmentBox(7))));
        assert!((r.dx - 0.05).abs() < 1e-9);
    }

    #[test]
    fn shape_extensions_snap_without_overlap() {
        let s = SnapSettings {
            snap_grid: false,
            snap_alignment_boxes: false,
            snap_connection_points: false,
            snap_ruler_subdivisions: false,
            snap_shape_extensions: true,
            ..SnapSettings::default()
        };
        let targets = [SnapTarget {
            id: 3,
            rect: box_at(0.0, 5.0, 1.0, 1.0),
            ..Default::default()
        }];
        let r = snap_translation(box_at(0.95, 0.0, 1.0, 1.0), &ctx(&targets, &[]), &s);
        assert_eq!(
            r.x_hit.as_ref().map(|(_, h)| *h),
            Some(SnapHit::ShapeExtension(3)),
            "extensions are infinite lines"
        );
    }

    #[test]
    fn nearer_target_wins() {
        let s = SnapSettings {
            snap_grid: false,
            snap_alignment_boxes: true,
            snap_shape_extensions: false,
            snap_connection_points: false,
            snap_ruler_subdivisions: false,
            ..SnapSettings::default()
        };
        let targets = [
            SnapTarget {
                id: 1,
                rect: box_at(0.0, 0.0, 0.97, 1.0), // right edge 0.97 → 0.02 away
                ..Default::default()
            },
            SnapTarget {
                id: 2,
                rect: box_at(0.0, 0.0, 1.04, 1.0), // right edge 1.04 → 0.09 away (beyond)
                ..Default::default()
            },
        ];
        let r = snap_translation(box_at(0.95, 0.0, 1.0, 1.0), &ctx(&targets, &[]), &s);
        assert_eq!(
            r.x_hit.as_ref().map(|(at, h)| (*at, *h)),
            Some((0.97, SnapHit::AlignmentBox(1)))
        );
    }

    #[test]
    fn beyond_the_threshold_never_snaps() {
        let s = SnapSettings {
            snap_grid: false,
            snap_alignment_boxes: true,
            snap_shape_extensions: false,
            snap_connection_points: false,
            snap_ruler_subdivisions: false,
            ..SnapSettings::default()
        };
        // Target placed so every feature pair (x and y) is farther
        // than the threshold, while the boxes still overlap in y —
        // eligibility is not the question, distance is.
        let targets = [SnapTarget {
            id: 1,
            rect: box_at(0.1, 0.12, 0.5, 0.19),
            ..Default::default()
        }];
        let r = snap_translation(box_at(0.95, 0.0, 1.0, 1.0), &ctx(&targets, &[]), &s);
        assert!(r.x_hit.is_none() && r.y_hit.is_none(), "must not snap: {r:?}");
        assert_eq!((r.dx, r.dy), (0.0, 0.0));
    }

    #[test]
    fn intersections_snap_both_axes() {
        let s = SnapSettings {
            snap_grid: false,
            snap_alignment_boxes: false,
            snap_shape_extensions: false,
            snap_connection_points: false,
            snap_ruler_subdivisions: false,
            snap_shape_intersections: true,
            ..SnapSettings::default()
        };
        // Target A spans x∈[0,1] at y=5; target B spans y∈[0,1] at
        // x=5. Their extension crossing at (5, 5) drags both axes when
        // the dragged box center sits near it.
        let targets = [
            SnapTarget {
                id: 1,
                rect: box_at(0.0, 4.5, 1.0, 1.0),
                ..Default::default()
            },
            SnapTarget {
                id: 2,
                rect: box_at(4.5, 0.0, 1.0, 1.0),
                ..Default::default()
            },
        ];
        // Dragged box center at (5.04, 4.96) — both axes within the
        // threshold of the crossing (5, 5).
        let r = snap_translation(box_at(4.54, 4.46, 1.0, 1.0), &ctx(&targets, &[]), &s);
        assert_eq!(
            r.x_hit.as_ref().map(|(_, h)| *h),
            Some(SnapHit::ShapeIntersection)
        );
        assert_eq!(
            r.y_hit.as_ref().map(|(_, h)| *h),
            Some(SnapHit::ShapeIntersection)
        );
        assert!((r.dx - (-0.04)).abs() < 1e-9);
        assert!((r.dy - 0.04).abs() < 1e-9);
    }

    #[test]
    fn glue_snaps_an_endpoint_to_a_connection_point() {
        let s = SnapSettings::default(); // glue: guides + connection points
        let targets = [SnapTarget {
            id: 9,
            rect: box_at(0.0, 0.0, 2.0, 1.0),
            connection_points: vec![(1.0, 1.0), (2.0, 0.5)],
            ..Default::default()
        }];
        let (p, hit) = point_snap((1.05, 1.03), &ctx(&targets, &[]), &s, true)
            .expect("within threshold of (1,1)");
        assert!((p.0 - 1.0).abs() < 1e-9 && (p.1 - 1.0).abs() < 1e-9);
        assert_eq!(hit, SnapHit::ConnectionPoint(9));
        // A disabled glue target never sticks.
        let mut off = s;
        off.glue_connection_points = false;
        assert!(point_snap((1.05, 1.03), &ctx(&targets, &[]), &off, true).is_none());
        // Glue inactive entirely → no candidates at all.
        let mut inactive = s;
        inactive.glue_active = false;
        assert!(point_snap((1.05, 1.03), &ctx(&targets, &[]), &inactive, true).is_none());
    }

    #[test]
    fn glue_snaps_to_shape_handles_and_vertices_when_enabled() {
        let s = SnapSettings {
            glue_connection_points: false,
            glue_shape_handles: true,
            glue_shape_vertices: true,
            ..SnapSettings::default()
        };
        let targets = [SnapTarget {
            id: 4,
            rect: box_at(0.0, 0.0, 1.0, 1.0),
            vertices: vec![(0.5, 1.2)],
            ..Default::default()
        }];
        // Near the top-right handle (1,1).
        let (p, hit) = point_snap((0.97, 1.02), &ctx(&targets, &[]), &s, true).unwrap();
        assert!((p.0 - 1.0).abs() < 1e-9 && (p.1 - 1.0).abs() < 1e-9);
        assert_eq!(hit, SnapHit::ShapeHandle(4));
        // Near the geometry vertex.
        let (p, hit) = point_snap((0.52, 1.18), &ctx(&targets, &[]), &s, true).unwrap();
        assert!((p.0 - 0.5).abs() < 1e-9 && (p.1 - 1.2).abs() < 1e-9);
        assert_eq!(hit, SnapHit::ShapeVertex(4));
    }

    #[test]
    fn point_snap_snap_side_uses_grid_lines() {
        let s = SnapSettings {
            snap_alignment_boxes: false,
            snap_shape_extensions: false,
            snap_connection_points: false,
            snap_shape_intersections: false,
            snap_ruler_subdivisions: false,
            ..SnapSettings::default()
        };
        let five_mm = 5.0 / 25.4;
        let (p, hit) = point_snap((0.199, 0.5), &ctx(&[], &[]), &s, false).unwrap();
        assert!((p.0 - five_mm).abs() < 1e-9, "x rounds to 5 mm");
        assert!((p.1 - 0.5).abs() < 1e-9, "y stays free");
        assert_eq!(hit, SnapHit::Grid);
    }

    #[test]
    fn hints_align_centers_and_edges() {
        let targets = [
            SnapTarget {
                id: 1,
                rect: box_at(0.0, 0.0, 2.0, 1.0),
                ..Default::default()
            },
            SnapTarget {
                id: 2,
                rect: box_at(4.0, 3.0, 2.0, 2.0),
                ..Default::default()
            },
        ];
        // #168: ONE alignment line per axis, at the NEAREST
        // coincidence (ties break to the smaller coordinate). The
        // dragged box (1.05, 0.03, 1, 1) has bottom 0.03, center
        // 0.53, top 1.03 — all three within threshold of target 1's
        // 0.0/0.5/1.0 at equal 0.03 deltas; the tie picks 0.0, and
        // the x side has no coincidence in threshold… except the
        // 2.0-vs-2.05 top-edge pair at 0.05 (inside the 0.1
        // threshold), which wins its axis.
        let hints = dynamic_hints(box_at(1.05, 0.03, 1.0, 1.0), &ctx(&targets, &[]));
        let aligns = |vertical: bool| {
            hints
                .iter()
                .filter(|h| matches!(h, Hint::Align { vertical: v, .. } if *v == vertical))
                .count()
        };
        assert_eq!(aligns(true), 1, "one vertical line: {hints:?}");
        assert_eq!(aligns(false), 1, "one horizontal line: {hints:?}");
        let at_of = |vertical: bool| {
            hints.iter().find_map(|h| match h {
                Hint::Align { vertical: v, at } if *v == vertical => Some(*at),
                _ => None,
            })
        };
        assert!((at_of(true).unwrap() - 2.0).abs() < 1e-9, "{hints:?}");
        assert!((at_of(false).unwrap() - 0.0).abs() < 1e-9, "{hints:?}");
    }

    #[test]
    fn hints_report_equal_spacing() {
        // Dragged box (1.0 wide) between two 1-inch boxes with 0.5 in
        // gaps on each side.
        let targets = [
            SnapTarget {
                id: 1,
                rect: box_at(0.0, 0.0, 1.0, 1.0),
                ..Default::default()
            },
            SnapTarget {
                id: 2,
                rect: box_at(3.0, 0.0, 1.0, 1.0),
                ..Default::default()
            },
        ];
        let hints = dynamic_hints(box_at(1.5, 0.0, 1.0, 1.0), &ctx(&targets, &[]));
        match hints.iter().find(|h| matches!(h, Hint::Spacing { .. })) {
            Some(Hint::Spacing { len_in, .. }) => {
                assert!((len_in - 0.5).abs() < 1e-9);
            }
            other => panic!("expected a spacing hint, got {other:?} ({hints:?})"),
        }
        // Uneven gaps → no spacing hint.
        let targets = [
            SnapTarget {
                id: 1,
                rect: box_at(0.0, 0.0, 1.0, 1.0),
                ..Default::default()
            },
            SnapTarget {
                id: 2,
                rect: box_at(4.0, 0.0, 1.0, 1.0),
                ..Default::default()
            },
        ];
        let hints = dynamic_hints(box_at(1.5, 0.0, 1.0, 1.0), &ctx(&targets, &[]));
        assert!(
            !hints.iter().any(|h| matches!(h, Hint::Spacing { .. })),
            "0.5 vs 1.5 gaps are not equal"
        );
    }

    #[test]
    fn no_targets_no_hints() {
        assert!(dynamic_hints(box_at(0.0, 0.0, 1.0, 1.0), &ctx(&[], &[])).is_empty());
    }
}

#[cfg(test)]
mod guide_tests {
    use super::*;

    const THRESH: f64 = 0.08;

    #[test]
    fn guide_line_snaps_its_axis() {
        let s = SnapSettings {
            snap_active: true,
            snap_grid: false,
            snap_ruler_subdivisions: false,
            snap_alignment_boxes: false,
            snap_shape_extensions: false,
            snap_connection_points: false,
            ..SnapSettings::default()
        };
        let guides = [Guide::Line {
            id: 5,
            horizontal: false,
            at: 2.5,
        }];
        let ctx = SnapCtx {
            targets: &[],
            guides: &guides,
            grid_step_in: 5.0 / 25.4,
            ruler_step_in: 1.0 / 25.4,
            threshold_in: THRESH,
        };
        // Left edge 0.04 in off the guide's x.
        let r = snap_translation((2.54, 0.0, 1.0, 1.0), &ctx, &s);
        assert_eq!(
            r.x_hit.as_ref().map(|(at, h)| (*at, *h)),
            Some((2.5, SnapHit::Guide(5)))
        );
        assert!((r.dx - (-0.04)).abs() < 1e-9);
        // The free axis is unconstrained.
        assert!(r.y_hit.is_none());
    }

    #[test]
    fn guide_point_snaps_both_axes() {
        let s = SnapSettings {
            snap_active: true,
            snap_grid: false,
            snap_ruler_subdivisions: false,
            snap_alignment_boxes: false,
            snap_shape_extensions: false,
            snap_connection_points: false,
            ..SnapSettings::default()
        };
        let guides = [Guide::Point { id: 8, x: 3.0, y: 4.0 }];
        let ctx = SnapCtx {
            targets: &[],
            guides: &guides,
            grid_step_in: 5.0 / 25.4,
            ruler_step_in: 1.0 / 25.4,
            threshold_in: THRESH,
        };
        let r = snap_translation((2.96, 3.94, 1.0, 1.0), &ctx, &s);
        assert_eq!(
            r.x_hit.as_ref().map(|(_, h)| *h),
            Some(SnapHit::Guide(8))
        );
        assert_eq!(
            r.y_hit.as_ref().map(|(_, h)| *h),
            Some(SnapHit::Guide(8))
        );
    }

    #[test]
    fn endpoint_glues_to_a_guide_line_and_point() {
        let s = SnapSettings::default(); // glue: guides + connection points
        let guides = [
            Guide::Line {
                id: 5,
                horizontal: false,
                at: 2.5,
            },
            Guide::Point { id: 8, x: 3.0, y: 4.0 },
        ];
        let ctx = SnapCtx {
            targets: &[],
            guides: &guides,
            grid_step_in: 5.0 / 25.4,
            ruler_step_in: 1.0 / 25.4,
            threshold_in: THRESH,
        };
        // Near the line: x snaps, y stays free.
        let (p, hit) = point_snap((2.55, 6.0), &ctx, &s, true).unwrap();
        assert!((p.0 - 2.5).abs() < 1e-9 && (p.1 - 6.0).abs() < 1e-9);
        assert_eq!(hit, SnapHit::Guide(5));
        // Near the point: both axes land on it.
        let (p, hit) = point_snap((3.05, 3.95), &ctx, &s, true).unwrap();
        assert!((p.0 - 3.0).abs() < 1e-9 && (p.1 - 4.0).abs() < 1e-9);
        assert_eq!(hit, SnapHit::Guide(8));
        // Glue off → no candidates.
        let mut off = s;
        off.glue_guides = false;
        assert!(point_snap((2.55, 6.0), &ctx, &off, true).is_none());
    }
}
