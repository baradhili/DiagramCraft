//! B-spline/NURBS evaluation for the Visio spline-family geometry rows
//! (#188): `SplineStart`/`SplineKnot` (non-rational B-splines) and
//! `NURBSTo` (rational). One evaluator — the `dc::svg` renderer, the
//! dc-ui hit walk, and the jump polylines all flatten through it so
//! rendering, hit-testing, and hops agree by construction.
//!
//! Cell semantics per [MS-VSDX] §2.4.2.7/§2.4.2.14/§2.4.2.15 and the
//! `NURBS()` function §2.5.3.113, with the assembly conventions of
//! Visio's own import (libvisio's `collectNURBSTo`): control points =
//! [previous row endpoint] + [interior points] + [row endpoint];
//! knots = [first] + [interior knots] + [second-to-last] + [last];
//! weights = [first weight] + [interior weights] + [last weight].

/// A parsed `NURBS(...)` function's arguments (§2.5.3.113):
/// `NURBS(knotLast, degree, xType, yType, (x, y, knot, weight)*)`.
/// `x_type`/`y_type` zero mark the group coordinates as relative
/// (fractions of Width/Height), anything else as shape coordinates.
#[derive(Debug, Clone, PartialEq)]
pub struct NurbsFormula {
    pub last_knot: f64,
    pub degree: usize,
    pub x_type: f64,
    pub y_type: f64,
    /// (x, y, knot, weight) groups — the INTERIOR control points.
    pub points: Vec<(f64, f64, f64, f64)>,
}

/// Parse the `NURBS(...)` function text stored on a NURBSTo row's E
/// cell (the cached V and the formula F are the same text in the
/// corpus). Separators are commas and/or whitespace. `None` when the
/// text is not a NURBS function (e.g. `Themed`) or the argument list
/// is truncated.
pub fn parse_nurbs(text: &str) -> Option<NurbsFormula> {
    let inner = text
        .trim()
        .strip_prefix("NURBS")?
        .trim()
        .strip_prefix("(")?;
    let inner = inner.strip_suffix(")")?;
    let mut args: Vec<f64> = Vec::new();
    for tok in inner
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|s| !s.is_empty())
    {
        // Reject non-finite parses (#218 review): "inf"/"NaN"/1e999
        // all parse as f64 and would poison the spline with NaN
        // geometry — callers keep their straight-chord fallback.
        args.push(tok
            .parse::<f64>()
            .ok()
            .and_then(|v| v.is_finite().then_some(v))?);
    }
    if args.len() < 4 {
        return None;
    }
    let last_knot = args[0];
    let degree = args[1];
    if !(degree.is_finite() && (0.0..=25.0).contains(&degree) && degree.fract() == 0.0) {
        return None;
    }
    let degree = degree as usize;
    let x_type = args[2];
    let y_type = args[3];
    // Groups of exactly (x, y, knot, weight); a trailing partial
    // group means the text is truncated — reject rather than guess.
    let rest = &args[4..];
    if !rest.len().is_multiple_of(4) {
        return None;
    }
    let mut points = Vec::with_capacity(rest.len() / 4);
    for g in rest.as_chunks::<4>().0 {
        points.push((g[0], g[1], g[2], g[3]));
    }
    Some(NurbsFormula {
        last_knot,
        degree,
        x_type,
        y_type,
        points,
    })
}

/// One rational B-spline ready for evaluation: the FULL control-point
/// list (P0 = the previous row's endpoint through the row endpoint),
/// per-point weights, and the knot vector. `degree` 0..=25.
#[derive(Debug, Clone)]
pub struct SplineData {
    pub degree: usize,
    pub points: Vec<(f64, f64)>,
    pub weights: Vec<f64>,
    pub knots: Vec<f64>,
}

impl SplineData {
    /// Sanitize into an evaluable curve: knots forced non-decreasing,
    /// degree clamped to the point count, then the vector rebuilt the
    /// clamped way — `degree + 1` copies of the first knot, at most
    /// `n − degree − 1` interior knots, `degree + 1` copies of the
    /// last knot (exactly `n + degree + 1` entries) — normalized to
    /// 0..1. Weights pad/truncate to the point count (pad with 1 — an
    /// unweighted point). `None` when there is not enough data to
    /// draw a curve at all.
    pub fn sanitized(mut self) -> Option<SplineData> {
        let n = self.points.len();
        if n < 2 || self.knots.len() < 2 {
            return None;
        }
        self.weights.resize(n, 1.0);
        self.weights.truncate(n);
        // Non-decreasing (a bogus negative step is clamped — Visio's
        // import does the same); a non-finite knot poisons the order,
        // so drop the curve.
        for i in 1..self.knots.len() {
            if !self.knots[i].is_finite() || !self.knots[i - 1].is_finite() {
                return None;
            }
            if self.knots[i] < self.knots[i - 1] {
                self.knots[i] = self.knots[i - 1];
            }
        }
        // #233 rider (MisterDiagram knot-precision evidence): Visio
        // serializes NURBS knots at 14 significant digits, so adjacent
        // "repeated" knots can arrive differing in their last bits —
        // coalesce near-equal neighbors to the larger value (their
        // reconciliation rule) so the flattener never divides by a
        // phantom knot difference. Two review corrections (#278):
        // the tolerance is bounded by the parameter SPAN, not the
        // knots' absolute magnitude (a huge-offset unit-span domain
        // used to collapse onto itself and flatten to the chord), and
        // a whole RUN of near-equal knots reconciles to the run's
        // maximum — the old forward pass left [a,b,b,b] from
        // [a,a,b,b], keeping a phantom span inside the run.
        let span = (self.knots[self.knots.len() - 1] - self.knots[0]).abs();
        let mut i = 0;
        while i < self.knots.len() {
            let mut j = i;
            while j + 1 < self.knots.len() {
                let (a, b) = (self.knots[j], self.knots[j + 1]);
                let tol = 16.0 * f64::EPSILON * span.min(a.abs().max(b.abs()).max(1.0));
                if (b - a).abs() <= tol {
                    j += 1;
                } else {
                    break;
                }
            }
            if j > i {
                // Non-decreasing after the clamp pass, so the run's
                // maximum is its last member.
                let hi = self.knots[j];
                for k in i..=j {
                    self.knots[k] = hi;
                }
            }
            i = j + 1;
        }
        let d = self.degree.clamp(1, (n - 1).max(1));
        let first = self.knots[0];
        let last = *self.knots.last().unwrap();
        // Strip the endpoint runs the rows record (Visio splines
        // repeat the endpoint knots with low multiplicity); keep at
        // most the n − d − 1 interior knots the math allows.
        let eps = (last - first).abs().max(1.0) * 1e-9;
        let lead = self.knots.iter().take_while(|k| **k - first <= eps).count();
        let trail = self
            .knots
            .iter()
            .rev()
            .take_while(|k| last - **k <= eps)
            .count();
        let mid_hi = self.knots.len().saturating_sub(trail.min(self.knots.len()));
        let mid_lo = lead.min(mid_hi);
        let mut interior = self.knots[mid_lo..mid_hi].to_vec();
        let allowed = n.saturating_sub(d + 1);
        if interior.len() > allowed {
            interior.truncate(allowed);
        }
        // Sparse inputs (fewer interior knots than n − d − 1) would
        // leave the vector short of n + d + 1 — de_boor indexes up to
        // knots[n + d] — so pad the interior (repeat its last knot, or
        // `first` when there is none) until the assembled vector is
        // EXACTLY n + d + 1 entries.
        let pad_from = *interior.last().unwrap_or(&first);
        while interior.len() < allowed {
            interior.push(pad_from);
        }
        let mut knots = vec![first; d + 1];
        knots.append(&mut interior);
        knots.extend(std::iter::repeat_n(last, d + 1));
        let span = knots.last().unwrap() - knots[0];
        let span = if span.abs() < 1e-12 { 1.0 } else { span };
        let base = knots[0];
        for k in &mut knots {
            *k = (*k - base) / span;
        }
        Some(SplineData {
            degree: d,
            points: self.points,
            weights: self.weights,
            knots,
        })
    }

    /// Flatten to polyline points: rational De Boor over the parametric
    /// span `[knots[degree], knots[points.len()]]`, `samples` per knot
    /// span. The first/last points are the exact endpoints.
    pub fn flatten(&self, samples: usize) -> Vec<(f64, f64)> {
        let np = self.points.len();
        let d = self.degree;
        if np < 2 || d == 0 || self.knots.len() < d + 2 {
            return self.points.clone();
        }
        let t0 = self.knots[d.min(self.knots.len() - 1)];
        let t1 = self.knots[np.min(self.knots.len() - 1)];
        if t1 <= t0 {
            return vec![self.points[0], *self.points.last().unwrap()];
        }
        let segments = (np - d).max(1); // knot spans inside the range
        let total = samples.max(1) * segments;
        let mut out = Vec::with_capacity(total + 1);
        for i in 0..=total {
            let t = t0 + (t1 - t0) * (i as f64 / total as f64);
            out.push(self.de_boor(t));
        }
        out
    }

    /// Rational De Boor's algorithm at parameter `t`.
    fn de_boor(&self, t: f64) -> (f64, f64) {
        let np = self.points.len();
        let n = np - 1;
        let d = self.degree;
        // Find the knot span: the largest index k with knots[k] <= t
        // (clamped into [d, n]).
        let mut k = d;
        while k < n && t >= self.knots[k + 1] {
            k += 1;
        }
        // Homogeneous coordinates (weights applied), then project.
        let mut xs: Vec<f64> = (0..=d)
            .map(|i| self.points[k - d + i].0 * self.weights[k - d + i])
            .collect();
        let mut ys: Vec<f64> = (0..=d)
            .map(|i| self.points[k - d + i].1 * self.weights[k - d + i])
            .collect();
        let mut ws: Vec<f64> = (0..=d).map(|i| self.weights[k - d + i]).collect();
        for r in 1..=d {
            for j in (r..=d).rev() {
                let i = k - d + j;
                let denom = self.knots[i + d - r + 1] - self.knots[i];
                let alpha = if denom.abs() < 1e-12 {
                    0.0
                } else {
                    ((t - self.knots[i]) / denom).clamp(0.0, 1.0)
                };
                xs[j] = (1.0 - alpha) * xs[j - 1] + alpha * xs[j];
                ys[j] = (1.0 - alpha) * ys[j - 1] + alpha * ys[j];
                ws[j] = (1.0 - alpha) * ws[j - 1] + alpha * ws[j];
            }
        }
        let w = ws[d];
        if w.abs() < 1e-12 {
            return self.points[n.min(self.points.len() - 1)];
        }
        (xs[d] / w, ys[d] / w)
    }
}

/// Re-emit a `NURBS()` formula's text — the #188 control-drag path
/// rewrites the E cell with the moved group coordinates (the data
/// lives IN the formula, so the customization stays a formula).
pub fn render_nurbs(f: &NurbsFormula) -> String {
    let mut s = format!(
        "NURBS({}, {}, {}, {}",
        f.last_knot, f.degree, f.x_type, f.y_type
    );
    for (x, y, k, w) in &f.points {
        s.push_str(&format!(", {},{},{},{}", x, y, k, w));
    }
    s.push(')');
    s
}

/// The Visio spline accumulator ([MS-VSDX] §2.4.2.15/§2.4.2.14): a
/// `SplineStart` row opens the spline (X/Y = the second control
/// point; A = second knot; B = first knot; C = last knot; D =
/// degree), each `SplineKnot` row adds a control point and its knot,
/// and the LAST SplineKnot's endpoint closes the curve. The caller
/// resolves row cells (formula-preferred) and feeds plain values, so
/// the renderer and the hit walk share one assembly.
#[derive(Debug, Clone)]
pub struct SplineAcc {
    pub start: (f64, f64),
    pub p1: (f64, f64),
    pub second_knot: f64,
    pub first_knot: f64,
    pub last_knot: f64,
    pub degree: usize,
    pub knots: Vec<f64>,
    pub points: Vec<(f64, f64)>,
}

impl SplineAcc {
    /// Open a spline at the previous row's endpoint `start` with the
    /// SplineStart row's cells.
    #[allow(clippy::too_many_arguments)]
    pub fn start(
        start: (f64, f64),
        p1: (f64, f64),
        second_knot: f64,
        first_knot: f64,
        last_knot: f64,
        degree: f64,
    ) -> SplineAcc {
        SplineAcc {
            start,
            p1,
            second_knot,
            first_knot,
            last_knot,
            degree: degree.round().clamp(0.0, 25.0) as usize,
            knots: Vec::new(),
            points: Vec::new(),
        }
    }

    /// One SplineKnot row: its endpoint and knot.
    pub fn knot(&mut self, p: (f64, f64), knot: f64) {
        self.points.push(p);
        self.knots.push(knot);
    }

    /// Assemble the evaluable spline: control points = [start, P1] +
    /// SplineKnot points; knots = [first, second] + knots + [last];
    /// weights all 1 (§2.4.2 splines are non-rational). `None` when
    /// the row set is too thin to curve (no SplineKnot followed it).
    pub fn flush(self) -> Option<SplineData> {
        if self.points.is_empty() {
            return None;
        }
        let mut points = vec![self.start, self.p1];
        points.extend(self.points);
        let mut knots = vec![self.first_knot, self.second_knot];
        knots.extend(self.knots);
        knots.push(self.last_knot);
        let weights = vec![1.0; points.len()];
        SplineData {
            degree: self.degree,
            points,
            weights,
            knots,
        }
        .sanitized()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// #278 review: the near-equal knot coalescing reconciles whole
    /// RUNS to the run's maximum (the old forward pass left
    /// [a,b,b,b] from [a,a,b,b], keeping a phantom span), and its
    /// tolerance is bounded by the parameter SPAN — a huge-offset
    /// unit-span domain stays distinct instead of collapsing onto
    /// itself and flattening the curve to its chord.
    #[test]
    fn knot_reconciliation_is_run_based_and_span_bounded() {
        // A FOUR-member interior run [a,a,b,b] (b-a within the
        // span-bounded tolerance): the whole run reconciles to its
        // maximum. A two-member run would also reconcile under the
        // old pairwise loop, so this shape is what pins the fix
        // (Greptile, #278: [a,a,b,b] used to come back [a,b,b,b]).
        // Degree 1 with 6 points retains all four interior knots.
        let a = 0.5_f64;
        let b = 0.5 + 1e-16;
        let run = SplineData {
            degree: 1,
            points: vec![
                (0.0, 0.0),
                (1.0, 0.0),
                (2.0, 0.0),
                (3.0, 0.0),
                (4.0, 0.0),
                (5.0, 0.0),
            ],
            weights: vec![],
            knots: vec![0.0, a, a, b, b, 1.0],
        }
        .sanitized()
        .expect("sanitizes");
        // Assembled: [0,0, interior×4, 1,1] — the interior sits at
        // indices 2..=5.
        for k in &run.knots[2..=5] {
            assert_eq!(
                *k, run.knots[5],
                "the whole near-equal run reconciles to one knot"
            );
        }
        assert!(
            run.knots[2].to_bits() == b.to_bits(),
            "reconciled to the run's maximum, not its first member"
        );
        // A unit span parked at ~2^50: the old magnitude-based
        // tolerance (~4) fused the endpoints; the span-bounded one
        // must not, so the quadratic keeps its apex.
        let big = 1125899906842624.0;
        let wide = SplineData {
            degree: 2,
            points: vec![(0.0, 0.0), (1.0, 2.0), (2.0, 0.0)],
            weights: vec![],
            knots: vec![big, big + 1.0],
        }
        .sanitized()
        .expect("sanitizes");
        let pts = wide.flatten(16);
        assert!(pts.len() > 2, "the curve survives, not just the chord");
        let apex = pts.iter().map(|p| p.1).fold(f64::MIN, f64::max);
        // The Bézier quadratic's true apex is 1.0 (half the control
        // height); the old domain collapse flattened it to the y=0
        // chord.
        assert!(
            (apex - 1.0).abs() < 1e-9,
            "the quadratic keeps its apex: {apex}"
        );
    }

    /// A clamped cubic (4 control points, knots 0,0,0,0,1,1,1,1) with
    /// all weights 1 IS a cubic Bézier: the midpoint lands on the
    /// standard Bézier formula (P0 + 3C1 + 3C2 + P3) / 8.
    #[test]
    fn clamped_cubic_matches_bezier_midpoint() {
        let data = SplineData {
            degree: 3,
            points: vec![(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 2.0)],
            weights: vec![1.0; 4],
            knots: vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
        }
        .sanitized()
        .unwrap();
        let pts = data.flatten(16);
        let want = (
            (0.0 + 3.0 * 1.0 + 3.0 * 1.0 + 0.0) / 8.0,
            (0.0 + 3.0 * 0.0 + 3.0 * 1.0 + 2.0) / 8.0,
        );
        let mid = &pts[pts.len() / 2];
        assert!((mid.0 - want.0).abs() < 1e-6 && (mid.1 - want.1).abs() < 1e-6);
        assert!((pts[0].0 - 0.0).abs() < 1e-9 && (pts[0].1 - 0.0).abs() < 1e-9);
        let last = pts.last().unwrap();
        assert!((last.0 - 0.0).abs() < 1e-9 && (last.1 - 2.0).abs() < 1e-9);
    }

    /// Rational: a circle quadrant (weights 1, √2/2, √2/2, 1) — the
    /// weight pulls the middle control's influence.
    #[test]
    fn weights_shape_the_curve() {
        let s2 = 2f64.sqrt() / 2.0;
        let plain = SplineData {
            degree: 3,
            points: vec![(1.0, 0.0), (1.0, 1.0), (0.0, 1.0), (0.0, 0.0)],
            weights: vec![1.0, 1.0, 1.0, 1.0],
            knots: vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.1, 1.1, 1.1],
        };
        let weighted = SplineData {
            weights: vec![1.0, s2, s2, 1.0],
            ..plain.clone()
        };
        let a = plain.sanitized().unwrap().flatten(32);
        let b = weighted.sanitized().unwrap().flatten(32);
        let ma = &a[a.len() / 2];
        let mb = &b[b.len() / 2];
        // The weighted mid differs from the unweighted one.
        assert!(
            ((ma.0 - mb.0).abs() + (ma.1 - mb.1).abs()) > 1e-4,
            "weights must shape the curve: {ma:?} vs {mb:?}"
        );
    }

    /// The corpus NURBS formula text parses: mixed comma/space
    /// separators, (x, y, knot, weight) groups.
    #[test]
    fn corpus_nurbs_formula_parses() {
        let f = parse_nurbs(
            "NURBS(1, 3, 0, 0, 0.4447472312476,0.96240845000249,0,1, 0.44478216818642,0.96233371531065,0,1)",
        )
        .expect("the corpus formula parses");
        assert_eq!(f.degree, 3);
        assert_eq!(f.last_knot, 1.0);
        assert_eq!(f.x_type, 0.0);
        assert_eq!(f.points.len(), 2);
        assert_eq!(f.points[0], (0.4447472312476, 0.96240845000249, 0.0, 1.0));
        assert!(parse_nurbs("Themed").is_none());
        assert!(
            parse_nurbs("NURBS(1, 3, 0, 0, 1, 2, 3)").is_none(),
            "truncated group"
        );
    }
}
