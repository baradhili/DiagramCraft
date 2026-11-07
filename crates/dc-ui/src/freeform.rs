//! The Freeform tool (#132, URS-1225): a sampled pen stroke becomes a
//! new shape's Geometry section — ordinary editable rows that save and
//! round-trip. Input → geometry in three passes: Douglas-Peucker
//! simplification, a greedy arc fit (runs that a circle explains
//! within tolerance become `ArcTo` sagitta rows), then the bbox-frame
//! shape build (the same local-frame convention the renderer and the
//! #137 vertex handles use).

use vsdx::model::{Cell, Row, Section, Shape, SheetChild};

/// Douglas-Peucker polyline simplification, page inches → the fewer
/// points that still stay within `tol` of the input stroke.
pub fn simplify(points: &[(f64, f64)], tol: f64) -> Vec<(f64, f64)> {
    if points.len() < 3 {
        return points.to_vec();
    }
    // Recursive split: the point farthest from the [first, last]
    // chord splits the run; within tolerance it collapses.
    fn run(pts: &[(f64, f64)], tol: f64, out: &mut Vec<(f64, f64)>) {
        if pts.len() <= 2 {
            out.extend_from_slice(pts);
            return;
        }
        let (a, b) = (pts[0], pts[pts.len() - 1]);
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let len = dx.hypot(dy);
        let (mut best_i, mut best_d) = (0usize, -1.0f64);
        for (i, p) in pts.iter().enumerate().skip(1).take(pts.len() - 2) {
            let d = if len < 1e-12 {
                (p.0 - a.0).hypot(p.1 - a.1)
            } else {
                ((p.0 - a.0) * dy - (p.1 - a.1) * dx).abs() / len
            };
            if d > best_d {
                best_d = d;
                best_i = i;
            }
        }
        if best_d > tol {
            run(&pts[..=best_i], tol, out);
            out.pop();
            run(&pts[best_i..], tol, out);
        } else {
            out.push(a);
            out.push(b);
        }
    }
    let mut out = Vec::new();
    run(points, tol, &mut out);
    out
}

/// One fitted segment in page inches: a line, or a circular arc with
/// the Visio sagitta (|A| = height over the chord, positive bulging
/// right of travel — the renderer's own `ArcTo` convention).
#[derive(Debug, Clone, PartialEq)]
pub enum Seg {
    Line((f64, f64), (f64, f64)),
    Arc((f64, f64), (f64, f64), f64),
}

/// A circle through three points, `(center, radius)`.
fn circle3(a: (f64, f64), b: (f64, f64), c: (f64, f64)) -> Option<((f64, f64), f64)> {
    // Standard perpendicular-bisector solve.
    let d = 2.0 * (a.0 * (b.1 - c.1) + b.0 * (c.1 - a.1) + c.0 * (a.1 - b.1));
    if d.abs() < 1e-12 {
        return None;
    }
    let ux = ((a.0 * a.0 + a.1 * a.1) * (b.1 - c.1)
        + (b.0 * b.0 + b.1 * b.1) * (c.1 - a.1)
        + (c.0 * c.0 + c.1 * c.1) * (a.1 - b.1))
        / d;
    let uy = ((a.0 * a.0 + a.1 * a.1) * (c.0 - b.0)
        + (b.0 * b.0 + b.1 * b.1) * (a.0 - c.0)
        + (c.0 * c.0 + c.1 * c.1) * (b.0 - a.0))
        / d;
    let r = (a.0 - ux).hypot(a.1 - uy);
    if !r.is_finite() || r < 1e-9 {
        return None;
    }
    Some(((ux, uy), r))
}

/// Greedy fit over a simplified polyline: extend each run while every
/// point sits on one circle within `tol` (and the arc stays under a
/// half circle — beyond that, straight lines interpolate saner).
/// Runs of fewer than three points stay lines.
pub fn fit_segments(pts: &[(f64, f64)], tol: f64) -> Vec<Seg> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i + 1 < pts.len() {
        // Longest fitting run from i — at least FOUR points: three
        // define any circle exactly, so a sharp corner would
        // "fit" without a fourth point validating it.
        let mut best_j = i + 1;
        let mut best: Option<f64> = None;
        let max_j = (i + 12).min(pts.len() - 1); // bound the lookahead
        for j in (i + 3)..=max_j {
            let mid = pts[i + 1 + (j - i - 1) / 2];
            let Some((center, r)) = circle3(pts[i], mid, pts[j]) else {
                break;
            };
            // Every intermediate point must sit within tol of the
            // fitted circle AND on the arc's side of the run chord —
            // a same-circle point on the other side belongs to the
            // complementary arc, which the single sagitta row cannot
            // represent (the emitted arc would cut through it).
            let (dx, dy) = (pts[j].0 - pts[i].0, pts[j].1 - pts[i].1);
            let side_of = |p: (f64, f64)| dx * (p.1 - pts[i].1) - dy * (p.0 - pts[i].0);
            let run_side = side_of(mid);
            let ok = pts[i + 1..j].iter().all(|p| {
                let dist = (p.0 - center.0).hypot(p.1 - center.1);
                (dist - r).abs() < tol && r - dist < tol && run_side * side_of(*p) > 0.0
            });
            if !ok {
                break;
            }
            // Under a half circle: the sagitta must stay under the
            // half-chord.
            let (dx, dy) = (pts[j].0 - pts[i].0, pts[j].1 - pts[i].1);
            let chord = dx.hypot(dy);
            let a = sagitta(pts[i], pts[j], center, r);
            // #203 review: reject MAJOR-arc runs. A run wrapping past
            // 180° puts every point on the CENTER side of the chord —
            // the same-side check passes — but `sagitta()` then
            // returns the minor-arc height (h = r − dm ≈ 0 for a
            // wrap: every point sits r from the center) whose sign
            // bulges AWAY from the stroke. The emitted ArcTo would be
            // a near-flat chord cutting across the wrap, and the
            // `|a| > chord/2` cap cannot fire on that near-zero
            // value. A valid (minor-arc) run bulges OPPOSITE the
            // center, so the run side and the sagitta sign must
            // disagree: break when they agree.
            if chord < 1e-9 || a.abs() > chord / 2.0 {
                break;
            }
            best_j = j;
            best = Some(a);
        }
        if let Some(a) = best {
            out.push(Seg::Arc(pts[i], pts[best_j], a));
            i = best_j;
        } else {
            out.push(Seg::Line(pts[i], pts[i + 1]));
            i += 1;
        }
    }
    out
}

/// The signed sagitta of the circle `center`/`r` over the chord
/// a→b: positive when the arc bulges right of travel.
fn sagitta(a: (f64, f64), b: (f64, f64), center: (f64, f64), r: f64) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let chord = dx.hypot(dy);
    if chord < 1e-9 {
        return 0.0;
    }
    let mid = ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
    let dm = (mid.0 - center.0).hypot(mid.1 - center.1);
    // The center sits left of travel → the arc bulges right.
    let left = dx * (center.1 - mid.1) - dy * (center.0 - mid.0) > 0.0;
    let h = r - dm;
    if left {
        h
    } else {
        -h
    }
}

/// Build the freeform shape: a top-level 2D shape whose Geometry
/// section carries the fitted segments in the bbox-local frame (the
/// renderer's and #137 vertex handles' convention). Ordinary rows —
/// editable in-app, saved and round-tripped like any shape.
/// Build the freeform shape; `None` for a stroke that simplifies to
/// fewer than two points (nothing to draw).
pub fn build_shape(id: i64, stroke: &[(f64, f64)]) -> Option<Shape> {
    let simplified = simplify(stroke, 0.01);
    if simplified.len() < 2 {
        return None;
    }
    let segs = fit_segments(&simplified, 0.01);
    let xs = simplified.iter().map(|p| p.0);
    let ys = simplified.iter().map(|p| p.1);
    let (x0, y0) = (
        xs.clone().fold(f64::INFINITY, f64::min),
        ys.clone().fold(f64::INFINITY, f64::min),
    );
    let (x1, y1) = (
        xs.fold(f64::NEG_INFINITY, f64::max),
        ys.fold(f64::NEG_INFINITY, f64::max),
    );
    let (w, h) = ((x1 - x0).max(1e-4), (y1 - y0).max(1e-4));
    let mut s = Shape {
        id,
        type_: Some("Shape".into()),
        name_u: Some(format!("Freeform.{id}")),
        ..Shape::default()
    };
    let cells = &mut s.sheet.children;
    for (n, v) in [
        ("PinX", x0 + w / 2.0),
        ("PinY", y0 + h / 2.0),
        ("Width", w),
        ("Height", h),
        ("LocPinX", w / 2.0),
        ("LocPinY", h / 2.0),
        ("FillPattern", 0.0),
    ] {
        cells.push(SheetChild::Cell(
            n.to_string(),
            Cell::value(format!("{v:.6}")),
        ));
    }
    let mut sec = Section {
        name: "Geometry".into(),
        ix: 0,
        ..Section::default()
    };
    for (n, v) in [
        ("NoFill", "0"),
        ("NoLine", "0"),
        ("NoShow", "0"),
        ("NoSnap", "0"),
    ] {
        sec.cells.push((n.to_string(), Cell::value(v)));
    }
    let mut ix = 1i64;
    let first = simplified[0];
    sec.rows.push(Row {
        t: Some("MoveTo".into()),
        ix,
        cells: vec![
            ("X".to_string(), Cell::value(format!("{:.6}", first.0 - x0))),
            ("Y".to_string(), Cell::value(format!("{:.6}", first.1 - y0))),
        ],
        ..Row::default()
    });
    ix += 1;
    for seg in &segs {
        let (a, b, a_cell) = match seg {
            Seg::Line(a, b) => (a, b, None),
            Seg::Arc(a, b, sag) => (a, b, Some(*sag)),
        };
        let mut cells = vec![
            ("X".to_string(), Cell::value(format!("{:.6}", b.0 - x0))),
            ("Y".to_string(), Cell::value(format!("{:.6}", b.1 - y0))),
        ];
        if let Some(sag) = a_cell {
            cells.push(("A".to_string(), Cell::value(format!("{sag:.6}"))));
        }
        sec.rows.push(Row {
            t: Some(if a_cell.is_some() { "ArcTo" } else { "LineTo" }.into()),
            ix,
            cells,
            ..Row::default()
        });
        let _ = a;
        ix += 1;
    }
    s.sheet.children.push(SheetChild::Section(sec));
    Some(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Simplification collapses a dense straight run to its ends and
    /// keeps the corners of a zigzag.
    #[test]
    fn simplify_keeps_corners_and_collapses_runs() {
        let dense: Vec<(f64, f64)> = (0..50).map(|i| (i as f64 * 0.02, 0.0)).collect();
        assert_eq!(simplify(&dense, 0.01).len(), 2, "a straight run collapses");
        let zig = vec![(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (2.0, 1.0)];
        let s = simplify(&zig, 0.01);
        assert_eq!(s.len(), 4, "corners survive: {s:?}");
    }

    /// A sampled semicircle fits ONE arc whose sagitta ≈ the radius;
    /// a zigzag stays all lines.
    #[test]
    fn fit_arcs_find_curves_and_keep_zigzags() {
        // Semicircle of radius 1 sampled finely, translated to (2, 2).
        let stroke: Vec<(f64, f64)> = (0..=36)
            .map(|i| {
                let t = i as f64 / 36.0 * std::f64::consts::PI;
                (2.0 - t.cos(), 2.0 + t.sin())
            })
            .collect();
        let segs = fit_segments(&simplify(&stroke, 0.005), 0.01);
        assert!(
            matches!(segs[0], Seg::Arc(..)),
            "the semicircle fits an arc: {segs:?}"
        );
        let zig = vec![(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (2.0, 1.0)];
        let segs = fit_segments(&zig, 0.01);
        assert!(
            segs.iter().all(|s| matches!(s, Seg::Line(..))),
            "a zigzag stays lines: {segs:?}"
        );
    }

    /// #203 review: a stroke wrapping ~240° around one circle — the
    /// radial fit alone would accept the WHOLE wrap as one arc (all
    /// points on the circle), but the emitted sagitta arc only covers
    /// the ≤180° span. The side check must split the run, and every
    /// EMITTED segment must follow the stroke (each segment's arc
    /// midpoint lands near a stroke sample).
    #[test]
    fn fit_splits_a_240_degree_wrap_and_tracks_the_stroke() {
        // 240° of a radius-2 circle centered (3, 3), sampled finely;
        // the stroke starts at angle 150° so the wrap crosses the
        // chord of every candidate run.
        let stroke: Vec<(f64, f64)> = (0..=60)
            .map(|i| {
                let a = (150.0 + 240.0 * i as f64 / 60.0).to_radians();
                (3.0 + 2.0 * a.cos(), 3.0 + 2.0 * a.sin())
            })
            .collect();
        let segs = fit_segments(&simplify(&stroke, 0.005), 0.01);
        assert!(segs.len() > 1, "a 240° wrap splits: {segs:?}");

        // Each emitted segment (chord a→b, sagitta s) has its arc
        // midpoint at chord_mid + s·right-of-travel; that midpoint
        // must land within 2·tol·(arc factor) of the STROKE — i.e.
        // the emitted geometry follows the samples instead of
        // shortcutting across the circle's far side.
        for seg in &segs {
            let (a, b, s) = match seg {
                Seg::Arc(a, b, s) => (a, b, *s),
                Seg::Line(a, b) => {
                    // A line segment's mid must also track the stroke.
                    let mid = ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
                    let near = stroke
                        .iter()
                        .any(|p| (p.0 - mid.0).hypot(p.1 - mid.1) < 0.2);
                    assert!(near, "line mid tracks the stroke: {a:?}→{b:?}");
                    continue;
                }
            };
            let (dx, dy) = (b.0 - a.0, b.1 - a.1);
            let len = dx.hypot(dy).max(1e-9);
            let mid = ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
            let arc_mid = (mid.0 + dy / len * s, mid.1 - dx / len * s);
            let near = stroke
                .iter()
                .any(|p| (p.0 - arc_mid.0).hypot(p.1 - arc_mid.1) < 0.2);
            assert!(
                near,
                "arc mid {arc_mid:?} tracks the stroke (a={a:?} b={b:?} s={s})"
            );
        }
    }

    /// #203 review (the requested fixture): a ~300° wrap on a
    /// radius-0.5 circle — small enough that the 12-point lookahead
    /// covers runs past 180°, where the radial + side checks pass but
    /// the emitted minor-arc sagitta is ≈ 0 and the "arc" is a flat
    /// chord cutting across the wrap. Every emitted segment's arc
    /// midpoint must land near a stroke sample.
    #[test]
    fn fit_300_degree_wrap_tracks_the_stroke() {
        let stroke: Vec<(f64, f64)> = (0..=60)
            .map(|i| {
                let a = (300.0 * i as f64 / 60.0).to_radians();
                (0.5 * a.cos(), 0.5 * a.sin())
            })
            .collect();
        let segs = fit_segments(&simplify(&stroke, 0.005), 0.01);
        for seg in &segs {
            let (a_pt, b_pt, s) = match seg {
                Seg::Arc(a, b, s) => (*a, *b, *s),
                Seg::Line(a, b) => {
                    let mid = ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
                    let near = stroke
                        .iter()
                        .any(|p| (p.0 - mid.0).hypot(p.1 - mid.1) < 0.05);
                    assert!(
                        near,
                        "line segment mid {mid:?} must track the stroke: {a:?}→{b:?}"
                    );
                    continue;
                }
            };
            let (dx, dy) = (b_pt.0 - a_pt.0, b_pt.1 - a_pt.1);
            let len = dx.hypot(dy).max(1e-9);
            let mid = ((a_pt.0 + b_pt.0) / 2.0, (a_pt.1 + b_pt.1) / 2.0);
            let arc_mid = (mid.0 + dy / len * s, mid.1 - dx / len * s);
            let near = stroke
                .iter()
                .any(|p| (p.0 - arc_mid.0).hypot(p.1 - arc_mid.1) < 0.05);
            assert!(
                near,
                "arc mid {arc_mid:?} tracks the stroke (a={a_pt:?} b={b_pt:?} s={s})"
            );
        }
    }

    /// The built shape: bbox frame cells, MoveTo + the fitted rows,
    /// and the geometry in LOCAL coordinates.
    #[test]
    fn build_shape_frames_the_stroke() {
        let stroke = vec![(1.0, 1.0), (2.0, 1.0), (3.0, 2.0)];
        let s = build_shape(7, &stroke).expect("a nonempty stroke builds");
        let num = |n: &str| {
            s.sheet
                .cell(n)
                .and_then(|c| c.numeric())
                .unwrap_or(f64::NAN)
        };
        assert!((num("Width") - 2.0).abs() < 1e-6, "w={}", num("Width"));
        assert!((num("Height") - 1.0).abs() < 1e-6, "h={}", num("Height"));
        assert!((num("PinX") - 2.0).abs() < 1e-6 && (num("PinY") - 1.5).abs() < 1e-6);
        let secs = s.sheet.sections_named("Geometry");
        assert_eq!(secs.len(), 1);
        let rows = &secs[0].rows;
        assert_eq!(rows[0].t.as_deref(), Some("MoveTo"));
        assert_eq!(rows.len(), 3, "two segments after MoveTo");
        // Local frame: the first point is the stroke's origin offset.
        let x1 = rows[0].cell("X").and_then(|c| c.numeric()).unwrap();
        let y1 = rows[0].cell("Y").and_then(|c| c.numeric()).unwrap();
        assert!(x1.abs() < 1e-6 && y1.abs() < 1e-6, "local origin");
    }
}
