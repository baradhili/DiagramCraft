//! The geometry placement frame, hit walk, and connector-crossing
//! math (#187): the pieces the EXPORT path needs on the dc side —
//! moved verbatim from dc-ui's `connectors.rs` (which re-exports them,
//! so every call site stayed put). The line-jump overlay the canvas
//! paints and the one `dc::svg` now appends share THIS code, so the
//! two can't drift.

use vsdx::model::{Cell, Row, Section, Shape};

/// `ObjType=2` — the routable connector marker (corpus Dynamic
/// connector).
pub fn is_connector(s: &Shape) -> bool {
    s.sheet
        .cell("ObjType")
        .and_then(|c| c.numeric())
        .is_some_and(|v| v == 2.0)
}

/// The chain-aware form (#172): a master-based connector instance
/// carries `ObjType=2` on its MASTER, not locally — the local-only
/// check missed every Visio-authored connector (endpoint drags fell
/// to the plain-1D path and were refused by the glue formulas).
///
/// #226 (MisterDiagram parity evidence, utilisation.vsdx): Visio also
/// authors 1D glue connectors WITHOUT ObjType — the corpus Line-arc
/// master marks itself with the glue engine's signature instead
/// (`GlueType` + `BegTrigger`/`EndTrigger`, no ObjType at all). A
/// resolved `BeginX` plus a glue trigger on either end is that
/// signature: the endpoint cells exist (XForm1D) and the glue engine
/// owns them. Plain 1D lines (the blank template's Line master) carry
/// no triggers and stay out — they are not glue shapes.
pub fn is_connector_in(doc: &vsdx::Document, chain: &[&Shape], s: &Shape) -> bool {
    if is_connector(s) {
        return true;
    }
    // The #174 chain-resolved form: a master-based connector carries
    // ObjType=2 on its MASTER.
    if doc
        .resolve_cell(s, chain, "ObjType")
        .and_then(|c| c.numeric())
        .is_some_and(|v| v == 2.0)
    {
        return true;
    }
    let resolved = |n: &str| doc.resolve_cell(s, chain, n);
    let one_d = resolved("BeginX").is_some();
    let glue_owned = resolved("BegTrigger").is_some() || resolved("EndTrigger").is_some();
    one_d && glue_owned
}

/// The connector's ink as page-space segments for hit-testing (#172):
/// the rendered line follows the GEOMETRY rows — an arc or elbow
/// leaves the Begin→End chord, so chord-only hit-testing made curved
/// connectors unselectable where another shape's bbox overlaps the
/// belly. Rows are shape-local (0..w × 0..h, y-up) and map onto
/// `rect` (the #137 vertex-handle convention); relative rows carry
/// fractions of the owning size; arcs are sampled by the quadratic
/// through their on-curve control (the renderer's own degenerate-arc
/// fallback), 3 interior samples per curve. When the instance has no
/// local Geometry the master root's first section stands in, scaled
/// from the master's size to the instance's.
/// Everything the hit walk needs to place a shape's geometry rows on
/// the page (#173/#179): the placement frame INCLUDING rotation and
/// flips (the renderer's `translate(Pin) rotate(-Angle) scale(Flip)`
/// composed into page y-up space), and the cell-evaluation
/// environment — geometry row formulas (`Width*1`, `Height*0.5`)
/// evaluate against the INSTANCE's sizes.
#[derive(Debug, Clone)]

pub struct HitFrame {
    pub pin: (f64, f64),
    pub loc_pin: (f64, f64),
    pub size: (f64, f64),
    pub master_size: (f64, f64),
    pub angle: f64,
    /// (+1/−1, +1/−1) from FlipX/FlipY.
    pub flip: (f64, f64),
    pub env: vsdx::formula::Env,
}

impl HitFrame {
    /// Local (row-space, post-Rel scaling) → page: the renderer's
    /// transform in y-up coordinates.
    pub fn to_page(&self, p: (f64, f64)) -> (f64, f64) {
        let (dx, dy) = (
            (p.0 - self.loc_pin.0) * self.flip.0,
            (p.1 - self.loc_pin.1) * self.flip.1,
        );
        let (sn, cs) = self.angle.sin_cos();
        (
            self.pin.0 + dx * cs - dy * sn,
            self.pin.1 + dx * sn + dy * cs,
        )
    }

    /// Page → local: the exact inverse of [`HitFrame::to_page`] — the
    /// #202 curve-control drags map the pointer into row space through
    /// it (rotation and flips included).
    pub fn to_local(&self, p: (f64, f64)) -> (f64, f64) {
        let (dx, dy) = (p.0 - self.pin.0, p.1 - self.pin.1);
        let (sn, cs) = (-self.angle).sin_cos();
        let (dx, dy) = (dx * cs - dy * sn, dx * sn + dy * cs);
        (
            dx / self.flip.0 + self.loc_pin.0,
            dy / self.flip.1 + self.loc_pin.1,
        )
    }

    /// The axis-aligned bounds of the shape's local box through the
    /// full frame — a rotated connector's ink lives OUTSIDE its
    /// unrotated Pin/Width/Height rect, so gates must use this.
    pub fn page_bounds(&self) -> (f64, f64, f64, f64) {
        let (w, h) = self.size;
        let c = [
            self.to_page((0.0, 0.0)),
            self.to_page((w, 0.0)),
            self.to_page((w, h)),
            self.to_page((0.0, h)),
        ];
        let xs = [c[0].0, c[1].0, c[2].0, c[3].0];
        let ys = [c[0].1, c[1].1, c[2].1, c[3].1];
        (
            xs.iter().cloned().fold(f64::INFINITY, f64::min),
            ys.iter().cloned().fold(f64::INFINITY, f64::min),
            xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
            ys.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
        )
    }
}

/// Build a shape's [`HitFrame`] from resolved cells (a minimal
/// renderer-parity env: the names geometry formulas reference).
pub fn hit_frame(
    doc: &vsdx::model::Document,
    chain: &[&Shape],
    s: &Shape,
    master_size: (f64, f64),
) -> HitFrame {
    let get = |n: &str| doc.resolve_cell(s, chain, n).and_then(|c| c.numeric());
    let size = (get("Width").unwrap_or(1.0), get("Height").unwrap_or(1.0));
    let mut env = vsdx::formula::Env::new();
    for n in [
        "PinX", "PinY", "Width", "Height", "LocPinX", "LocPinY", "Angle",
    ] {
        if let Some(v) = get(n) {
            env.insert(n.to_string(), v);
        }
    }
    HitFrame {
        pin: (get("PinX").unwrap_or(0.0), get("PinY").unwrap_or(0.0)),
        loc_pin: (
            get("LocPinX").unwrap_or(size.0 / 2.0),
            get("LocPinY").unwrap_or(size.1 / 2.0),
        ),
        size,
        master_size,
        angle: get("Angle").unwrap_or(0.0),
        flip: (
            if get("FlipX").unwrap_or(0.0) != 0.0 {
                -1.0
            } else {
                1.0
            },
            if get("FlipY").unwrap_or(0.0) != 0.0 {
                -1.0
            } else {
                1.0
            },
        ),
        env,
    }
}

/// The #202 curved-connector facts: one shape's first local
/// `CubicBezTo` row with its Bézier walk state, shape-local through
/// [`HitFrame`]'s formula-preferred resolution (the same precedence
/// the renderer's `row_num` uses for evaluable formulas).
#[derive(Debug, Clone, Copy)]
pub struct CubicRow {
    pub section_ix: i64,
    pub row_ix: i64,
    /// The previous row's endpoint — the Bézier's start.
    pub start: (f64, f64),
    /// The row's own endpoint.
    pub end: (f64, f64),
    /// The (A,B) control.
    pub ctrl1: (f64, f64),
    /// The (C,D) control.
    pub ctrl2: (f64, f64),
}

impl CubicRow {
    /// The cubic Bézier point at `t` (0..=1).
    pub fn at(&self, t: f64) -> (f64, f64) {
        let u = 1.0 - t;
        let b = |a: f64, c: f64, d: f64, e: f64| {
            u * u * u * a + 3.0 * u * u * t * c + 3.0 * u * t * t * d + t * t * t * e
        };
        (
            b(self.start.0, self.ctrl1.0, self.ctrl2.0, self.end.0),
            b(self.start.1, self.ctrl1.1, self.ctrl2.1, self.end.1),
        )
    }

    /// The t=0.5 point — the #202 `LineMid` seat on a curved
    /// connector (the chord midpoint is off the curve).
    pub fn mid(&self) -> (f64, f64) {
        self.at(0.5)
    }
}

/// The shape's first OWN-geometry `CubicBezTo` row, resolved through
/// `frame` (master-inherited rows stay unhanded, the #137 rule).
pub fn cubic_row(s: &Shape, frame: &HitFrame) -> Option<CubicRow> {
    for geo in s.sheet.sections_named("Geometry") {
        let mut cur: Option<(f64, f64)> = None;
        for row in &geo.rows {
            let cell = |name: &str| -> f64 {
                row_cell_value(row.cell(name), None, 1.0, &frame.env).unwrap_or(0.0)
            };
            let (x, y) = (cell("X"), cell("Y"));
            if row.t.as_deref() == Some("CubicBezTo") {
                let (a, b) = (cell("A"), cell("B"));
                let (c, d) = (cell("C"), cell("D"));
                return Some(CubicRow {
                    section_ix: geo.ix,
                    row_ix: row.ix,
                    start: cur.unwrap_or((0.0, 0.0)),
                    end: (x, y),
                    ctrl1: (a, b),
                    ctrl2: (c, d),
                });
            }
            cur = Some((x, y));
        }
    }
    None
}

/// One shape's first `EllipticalArcTo` row — the #226 arc twin of
/// [`CubicRow`]: the corpus Line-arc masters (utilisation.vsdx) shape
/// their whole ink as one arc, so the `LineMid` handle must seat ON
/// the curve, not on the begin→end chord. Resolution mirrors the hit
/// walk's row walk (own rows + the master rows the instance dropped,
/// ix-ordered, Rel rows as size fractions, master-section caches
/// scaled) so the seat sits on the RENDERED curve — the renderer and
/// hit walk draw the same quadratic through the on-curve control.
#[derive(Debug, Clone, Copy)]
pub struct ArcRow {
    /// The previous row's endpoint — the arc's start.
    pub start: (f64, f64),
    /// The row's own endpoint.
    pub end: (f64, f64),
    /// The ON-curve control (the row's A/B).
    pub on_curve: (f64, f64),
}

impl ArcRow {
    /// The quadratic control reproducing an on-curve apex (the
    /// renderer's degenerate-arc fallback).
    fn ctrl(&self) -> (f64, f64) {
        (
            2.0 * self.on_curve.0 - (self.start.0 + self.end.0) / 2.0,
            2.0 * self.on_curve.1 - (self.start.1 + self.end.1) / 2.0,
        )
    }

    /// The curve point at `u` (0..=1).
    pub fn at(&self, u: f64) -> (f64, f64) {
        let (ctrl, v) = (self.ctrl(), 1.0 - u);
        (
            v * v * self.start.0 + 2.0 * v * u * ctrl.0 + u * u * self.end.0,
            v * v * self.start.1 + 2.0 * v * u * ctrl.1 + u * u * self.end.1,
        )
    }

    /// The t=0.5 point — the `LineMid` seat ON the curve (the chord
    /// midpoint hangs off it).
    pub fn mid(&self) -> (f64, f64) {
        self.at(0.5)
    }
}

/// The shape's first `EllipticalArcTo` row through [`HitFrame`]'s
/// resolution; `master_root` stands in when the shape has no own
/// Geometry (like [`geometry_hit_segments`]).
pub fn arc_row(s: &Shape, master_root: Option<&Shape>, frame: &HitFrame) -> Option<ArcRow> {
    let mut sections: Vec<&Section> = s.sheet.sections_named("Geometry");
    let (own, scale) = if sections.is_empty() {
        let root = master_root?;
        sections = root.sheet.sections_named("Geometry");
        let ms = frame.master_size;
        (
            ms,
            if ms.0.abs() > 1e-6 && ms.1.abs() > 1e-6 {
                (frame.size.0 / ms.0, frame.size.1 / ms.1)
            } else {
                (1.0, 1.0)
            },
        )
    } else {
        (frame.size, (1.0, 1.0))
    };
    let master_rows: Vec<&Row> = master_root
        .and_then(|r| r.sheet.sections_named("Geometry").into_iter().next())
        .map(|g| g.rows.iter().collect())
        .unwrap_or_default();
    let mut walk: Vec<(&Row, Option<&Row>)> = Vec::new();
    for geo in &sections {
        for row in geo.rows.iter().take(64) {
            walk.push((row, master_rows.iter().find(|r| r.ix == row.ix).copied()));
        }
    }
    for r in master_rows.iter().take(64) {
        if !walk.iter().any(|(row, _)| row.ix == r.ix) {
            walk.push((r, None));
        }
    }
    walk.sort_by_key(|(row, _)| row.ix);
    let cell = |row: &Row, master: Option<&Row>, name: &str| -> Option<f64> {
        let axis = if name == "X" || name == "A" { scale.0 } else { scale.1 };
        row_cell_value(row.cell(name), master.and_then(|m| m.cell(name)), axis, &frame.env)
    };
    // The hit walk's endpoint placement: Rel rows resolve ×own size,
    // then master-section caches scale into instance units.
    let placed = |v: (f64, f64), rel: bool| -> (f64, f64) {
        let v = if rel { (v.0 * own.0, v.1 * own.1) } else { v };
        if scale != (1.0, 1.0) { (v.0 * scale.0, v.1 * scale.1) } else { v }
    };
    let mut cur: Option<(f64, f64)> = None;
    for (row, master) in &walk {
        let t = row
            .t
            .as_deref()
            .or(master.and_then(|m| m.t.as_deref()))
            .unwrap_or("");
        let (x, y) = (cell(row, *master, "X"), cell(row, *master, "Y"));
        let rel = t.starts_with("Rel");
        let end = match (x, y) {
            (Some(x), Some(y)) => placed((x, y), rel),
            _ => (0.0, 0.0),
        };
        if t == "EllipticalArcTo" || t == "RelEllipticalArcTo" {
            // A/B ride `cell` alone — the hit walk applies no rel or
            // post-scale to the on-curve control.
            return match (cell(row, *master, "A"), cell(row, *master, "B"), cur) {
                (Some(a), Some(b), Some(start)) => Some(ArcRow { start, end, on_curve: (a, b) }),
                _ => None,
            };
        }
        cur = Some(end);
    }
    None
}

/// Resolve one geometry-row cell to a number through inheritance
/// (#179): the instance's literal, then the instance's formula
/// against the instance env, then the MASTER's formula (evaluated in
/// instance terms — `Width*1` means the instance's Width), then the
/// master's cached value scaled master→instance. Omitted cells
/// default to 0 (Visio's default) at the caller. `pub(crate)` so the
/// #202 curve-control handles position and commit through the SAME
/// resolution the renderer and hit walk use.
pub fn row_cell_value(
    inst: Option<&Cell>,
    master: Option<&Cell>,
    axis_scale: f64,
    env: &vsdx::formula::Env,
) -> Option<f64> {
    let eval = |f: &str| -> Option<f64> {
        vsdx::formula::eval(f, env)
            .ok()
            .and_then(|v| v.as_f64())
            .filter(|v| v.is_finite())
    };
    let literal = |c: &Cell| -> Option<f64> {
        let plain =
            c.f.as_deref()
                .is_none_or(|f| f == "Inh" || f == "No Formula");
        plain.then(|| c.numeric()).flatten()
    };
    if let Some(c) = inst {
        if let Some(v) = literal(c) {
            return Some(v);
        }
        if let Some(v) = c.f.as_deref().and_then(&eval) {
            return Some(v);
        }
        // A formula that failed to evaluate: its cached V is the
        // last-known instance value — better than dropping the row.
        if let Some(v) = c.numeric() {
            return Some(v);
        }
    }
    if let Some(c) = master {
        if let Some(v) = c.f.as_deref().and_then(&eval) {
            return Some(v);
        }
        if let Some(v) = literal(c) {
            return Some(v * axis_scale);
        }
        if let Some(v) = c.numeric() {
            return Some(v * axis_scale);
        }
    }
    None
}

/// The connector hit segments of a shape's geometry in PAGE space
/// (#173/#179): walks the shape's Geometry rows — merging inherited
/// rows and cells from the master — sampling arcs, and mapping every
/// point through the full placement frame (rotation + flips).
pub fn geometry_hit_segments(
    s: &Shape,
    master_root: Option<&Shape>,
    frame: &HitFrame,
) -> Vec<((f64, f64), (f64, f64))> {
    let mut sections: Vec<&Section> = s.sheet.sections_named("Geometry");
    // (owning width/height for Rel rows, x/y scale). The scale only
    // applies when the WHOLE section is the master's (its cached
    // literals are in master units); per-cell inheritance evaluates
    // master formulas in instance terms and scales only cached
    // literals (#179).
    let (own, scale) = if sections.is_empty() {
        let Some(root) = master_root else {
            return Vec::new();
        };
        sections = root.sheet.sections_named("Geometry");
        if sections.is_empty() {
            return Vec::new();
        }
        let ms = frame.master_size;
        (
            ms,
            if ms.0.abs() > 1e-6 && ms.1.abs() > 1e-6 {
                (frame.size.0 / ms.0, frame.size.1 / ms.1)
            } else {
                (1.0, 1.0)
            },
        )
    } else {
        (frame.size, (1.0, 1.0))
    };
    // The master rows of the FIRST geometry section, for per-cell
    // inheritance and master-only rows (shape 1017-style connectors
    // inherit their MoveTo entirely).
    let master_rows: Vec<&Row> = master_root
        .and_then(|r| r.sheet.sections_named("Geometry").into_iter().next())
        .map(|g| g.rows.iter().collect::<Vec<_>>())
        .unwrap_or_default();
    let master_row = |ix: i64| master_rows.iter().find(|r| r.ix == ix).copied();
    let axis_scale = |cell: &str| {
        if cell == "X" || cell == "A" {
            scale.0
        } else {
            scale.1
        }
    };
    let cell = |row: &Row, master: Option<&Row>, name: &str| -> Option<f64> {
        row_cell_value(
            row.cell(name),
            master.and_then(|m| m.cell(name)),
            axis_scale(name),
            &frame.env,
        )
    };
    let to_page = |p: (f64, f64)| -> (f64, f64) { frame.to_page(p) };
    let quad = |c0: (f64, f64), ctrl: (f64, f64), c1: (f64, f64), u: f64| -> (f64, f64) {
        (
            (1.0 - u).powi(2) * c0.0 + 2.0 * u * (1.0 - u) * ctrl.0 + u * u * c1.0,
            (1.0 - u).powi(2) * c0.1 + 2.0 * u * (1.0 - u) * ctrl.1 + u * u * c1.1,
        )
    };
    let mut pts: Vec<(f64, f64)> = Vec::new();
    let mut cur: Option<(f64, f64)> = None; // shape-local
                                            // #188: the spline accumulator — SplineStart opens, SplineKnot
                                            // rows extend, anything else flushes; NURBSTo rows evaluate
                                            // standalone. The flattened samples ARE the hit polyline, so
                                            // hit-testing agrees with the rendered ink (crate::curve::flatten).
    let mut spline_acc: Option<crate::curve::SplineAcc> = None;
    const MAX_ROWS: usize = 64;
    // The ordered walk: the instance's rows, then master-only rows
    // (by ix) the instance never overrode.
    let mut walk: Vec<(&Row, Option<&Row>)> = Vec::new();
    for geo in &sections {
        for row in geo.rows.iter().take(MAX_ROWS) {
            walk.push((row, master_row(row.ix)));
        }
    }
    for r in master_rows.iter().take(MAX_ROWS) {
        if !walk.iter().any(|(row, _)| row.ix == r.ix) {
            walk.push((r, None));
        }
    }
    walk.sort_by_key(|(row, _)| row.ix);
    for (row, master) in walk.iter().take(MAX_ROWS) {
        let t = row
            .t
            .as_deref()
            .or(master.and_then(|m| m.t.as_deref()))
            .unwrap_or("");
        // Omitted geometry cells default to 0 (the corpus arcs omit Y
        // and inherit it); a row nothing resolves is still walked as a
        // 0-coordinate row — never dropped (#179).
        let x = cell(row, *master, "X").unwrap_or(0.0);
        let y = cell(row, *master, "Y").unwrap_or(0.0);
        let rel = t.starts_with("Rel");
        let end = if rel { (x * own.0, y * own.1) } else { (x, y) };
        // Master-section cached literals are in master units.
        let end = if scale != (1.0, 1.0) {
            (end.0 * scale.0, end.1 * scale.1)
        } else {
            end
        };
        match t {
            "MoveTo" | "RelMoveTo" => {
                cur = Some(end);
                pts.push(to_page(end));
            }
            "ArcTo" | "RelArcTo" => {
                // |A| = sagitta, bulging right of travel; the
                // quadratic control that reproduces the apex.
                let a = cell(row, *master, "A").unwrap_or(0.0);
                if let Some(c0) = cur {
                    let d = (end.0 - c0.0, end.1 - c0.1);
                    let len = d.0.hypot(d.1).max(1e-9);
                    let mid = (
                        (c0.0 + end.0) / 2.0 + d.1 / len * a,
                        (c0.1 + end.1) / 2.0 - d.0 / len * a,
                    );
                    let ctrl = (
                        2.0 * mid.0 - (c0.0 + end.0) / 2.0,
                        2.0 * mid.1 - (c0.1 + end.1) / 2.0,
                    );
                    for u in [0.25, 0.5, 0.75] {
                        pts.push(to_page(quad(c0, ctrl, end, u)));
                    }
                }
                pts.push(to_page(end));
                cur = Some(end);
            }
            "EllipticalArcTo" | "RelEllipticalArcTo" => {
                // (A,B) is an ON-curve point → quadratic control.
                let a = cell(row, *master, "A");
                let b = cell(row, *master, "B");
                if let (Some(a), Some(b), Some(c0)) = (a, b, cur) {
                    let ctrl = (
                        2.0 * a - (c0.0 + end.0) / 2.0,
                        2.0 * b - (c0.1 + end.1) / 2.0,
                    );
                    for u in [0.25, 0.5, 0.75] {
                        pts.push(to_page(quad(c0, ctrl, end, u)));
                    }
                }
                pts.push(to_page(end));
                cur = Some(end);
            }
            "SplineStart" => {
                // #188: opens a spline (§2.4.2.15) — X/Y is the
                // second control point; A/B/C/D are second/first/
                // last knots and the degree.
                if let Some(acc) = spline_acc.take() {
                    if let Some(data) = acc.flush() {
                        for p in crate::curve::SplineData::flatten(&data, 16)
                            .into_iter()
                            .skip(1)
                        {
                            pts.push(to_page(p));
                        }
                    }
                }
                spline_acc = Some(crate::curve::SplineAcc::start(
                    cur.unwrap_or(end),
                    end,
                    cell(row, *master, "A").unwrap_or(0.0),
                    cell(row, *master, "B").unwrap_or(0.0),
                    cell(row, *master, "C").unwrap_or(1.0),
                    cell(row, *master, "D").unwrap_or(3.0),
                ));
                cur = Some(end);
            }
            "SplineKnot" => match spline_acc.as_mut() {
                Some(acc) => {
                    acc.knot(end, cell(row, *master, "A").unwrap_or(0.0));
                    cur = Some(end);
                }
                None => {
                    pts.push(to_page(end));
                    cur = Some(end);
                }
            },
            "NURBSTo" => {
                if let Some(acc) = spline_acc.take() {
                    if let Some(data) = acc.flush() {
                        for p in crate::curve::SplineData::flatten(&data, 16)
                            .into_iter()
                            .skip(1)
                        {
                            pts.push(to_page(p));
                        }
                    }
                }
                // §2.4.2.7: the E cell's NURBS() formula carries the
                // interior points (fractions of Width/Height when
                // xType/yType = 0) with their knots and weights; the
                // row cells frame them and X/Y is the endpoint.
                let e_text = row
                    .cell("E")
                    .and_then(|c| c.v.clone().or_else(|| c.f.clone()))
                    .unwrap_or_default();
                let mut drew = false;
                if let Some(f) = crate::curve::parse_nurbs(&e_text) {
                    let (fx, fy) = if f.x_type == 0.0 {
                        (own.0, 1.0)
                    } else {
                        (1.0, 1.0)
                    };
                    let (fx, fy) = if f.y_type == 0.0 {
                        (fx, own.1)
                    } else {
                        (fx, fy)
                    };
                    let start = cur.unwrap_or(end);
                    let mut points = vec![start];
                    let mut knots = vec![cell(row, *master, "C").unwrap_or(0.0)];
                    let mut weights = vec![cell(row, *master, "D").unwrap_or(1.0)];
                    for (gx, gy, gk, gw) in &f.points {
                        points.push((gx * fx, gy * fy));
                        knots.push(*gk);
                        weights.push(*gw);
                    }
                    knots.push(cell(row, *master, "A").unwrap_or(0.0));
                    knots.push(f.last_knot);
                    weights.push(cell(row, *master, "B").unwrap_or(1.0));
                    points.push(end);
                    let built = crate::curve::SplineData {
                        degree: f.degree,
                        points,
                        weights,
                        knots,
                    }
                    .sanitized();
                    if let Some(data) = built {
                        for p in crate::curve::SplineData::flatten(&data, 16)
                            .into_iter()
                            .skip(1)
                        {
                            pts.push(to_page(p));
                        }
                        cur = Some(end);
                        drew = true;
                    }
                }
                if !drew {
                    pts.push(to_page(end));
                    cur = Some(end);
                }
            }
            _ if t.ends_with("To") || t.contains("Spline") || t == "InfiniteLine" => {
                // LineTo plus the v0.2 polyline/infinite-line chords.
                if let Some(acc) = spline_acc.take() {
                    if let Some(data) = acc.flush() {
                        for p in crate::curve::SplineData::flatten(&data, 16)
                            .into_iter()
                            .skip(1)
                        {
                            pts.push(to_page(p));
                        }
                    }
                }
                pts.push(to_page(end));
                cur = Some(end);
            }
            _ => {
                if let Some(acc) = spline_acc.take() {
                    if let Some(data) = acc.flush() {
                        for p in crate::curve::SplineData::flatten(&data, 16)
                            .into_iter()
                            .skip(1)
                        {
                            pts.push(to_page(p));
                        }
                    }
                }
            }
        }
    }
    if let Some(acc) = spline_acc.take() {
        if let Some(data) = acc.flush() {
            for p in crate::curve::SplineData::flatten(&data, 16)
                .into_iter()
                .skip(1)
            {
                pts.push(to_page(p));
            }
        }
    }
    pts.windows(2).map(|w| (w[0], w[1])).collect()
}

pub struct JumpLine {
    pub id: i64,
    pub z: usize,
    pub weight: f64,
    /// (r, g, b) — the stroke color both overlays repaint.
    pub color: (u8, u8, u8),
    pub pts: Vec<(f64, f64)>,
}

/// A computed hop (#108): where the jumping line crosses the under
/// line, both directions (unit), and both strokes. Visio's default
/// crossing treatment — the more-horizontal line hops (ties: the
/// later in z), arc style, never persisted.
#[derive(Debug, Clone, Copy)]
pub struct Jump {
    pub at: (f64, f64),
    /// Unit vector along the JUMPING line.
    pub dir: (f64, f64),
    /// Unit vector along the under line.
    pub under: (f64, f64),
    pub weight: f64,
    pub under_weight: f64,
    pub color: (u8, u8, u8),
    pub under_color: (u8, u8, u8),
}

/// Visio's medium line-jump radius (inches).
pub const JUMP_RADIUS_IN: f64 = 0.12;

/// Proper segment intersection (interior of both).
pub fn seg_cross(
    a0: (f64, f64),
    a1: (f64, f64),
    b0: (f64, f64),
    b1: (f64, f64),
) -> Option<(f64, f64)> {
    let (r, s) = ((a1.0 - a0.0, a1.1 - a0.1), (b1.0 - b0.0, b1.1 - b0.1));
    let denom = r.0 * s.1 - r.1 * s.0;
    if denom.abs() < 1e-12 {
        return None; // parallel
    }
    let qp = (b0.0 - a0.0, b0.1 - a0.1);
    let t = (qp.0 * s.1 - qp.1 * s.0) / denom;
    let u = (qp.0 * r.1 - qp.1 * r.0) / denom;
    ((1e-9..=1.0 - 1e-9).contains(&t) && (1e-9..=1.0 - 1e-9).contains(&u))
        .then_some((a0.0 + t * r.0, a0.1 + t * r.1))
}

/// Every crossing hop among the lines (#108): pairwise proper
/// segment intersections; the more-horizontal segment jumps.
pub fn line_jumps(lines: &[JumpLine]) -> Vec<Jump> {
    let mut out = Vec::new();
    for i in 0..lines.len() {
        for j in (i + 1)..lines.len() {
            for w1 in lines[i].pts.windows(2) {
                for w2 in lines[j].pts.windows(2) {
                    let Some(at) = seg_cross(w1[0], w1[1], w2[0], w2[1]) else {
                        continue;
                    };
                    let unit = |a: (f64, f64), b: (f64, f64)| {
                        let d = (b.0 - a.0, b.1 - a.1);
                        let l = d.0.hypot(d.1).max(1e-9);
                        (d.0 / l, d.1 / l)
                    };
                    let (d1, d2) = (unit(w1[0], w1[1]), unit(w2[0], w2[1]));
                    // The more-horizontal line hops; a tie gives the
                    // later-in-z line the hop.
                    let i_jumps = d1.1.abs() < d2.1.abs()
                        || (d1.1.abs() - d2.1.abs()).abs() < 1e-9 && lines[i].z > lines[j].z;
                    let (jmp, und) = if i_jumps {
                        (&lines[i], &lines[j])
                    } else {
                        (&lines[j], &lines[i])
                    };
                    let (d, du) = if i_jumps { (d1, d2) } else { (d2, d1) };
                    out.push(Jump {
                        at,
                        dir: d,
                        under: du,
                        weight: jmp.weight,
                        under_weight: und.weight,
                        color: jmp.color,
                        under_color: und.color,
                    });
                }
            }
        }
    }
    out
}

/// Parse a `#rrggbb` line-color cell into the overlay's (r, g, b).
fn parse_hex_rgb(v: &str) -> Option<(u8, u8, u8)> {
    let hex = v.strip_prefix('#')?;
    if hex.len() != 6 {
        return None;
    }
    Some((
        u8::from_str_radix(&hex[0..2], 16).ok()?,
        u8::from_str_radix(&hex[2..4], 16).ok()?,
        u8::from_str_radix(&hex[4..6], 16).ok()?,
    ))
}

/// The page's connector crossings (#187 export parity): every
/// top-level connector's polyline — the same `hit_frame` +
/// [`geometry_hit_segments`] walk the canvas jump overlay uses,
/// formula-preferred, curve-aware — crossed with Visio's rule
/// ([`line_jumps`]: the more-horizontal line hops, ties to the later
/// z). Hidden-layer members and guides skip like the render walk.
pub fn page_jumps(doc: &vsdx::Document, page: &vsdx::Page, hidden: &[i64]) -> Vec<Jump> {
    let mut lines: Vec<JumpLine> = Vec::new();
    for (z, s) in page.shapes.iter().enumerate() {
        if s.sheet
            .cell("LayerMember")
            .and_then(|c| c.numeric())
            .is_some_and(|m| hidden.contains(&(m as i64)))
        {
            continue;
        }
        if s.type_.as_deref() == Some("Guide") {
            continue;
        }
        // Top-level chain: the shape plus its master root.
        let mut chain = vec![s];
        let msize = s
            .master
            .and_then(|mid| doc.master(mid))
            .and_then(|m| m.shapes.first())
            .map(|r| {
                chain.push(r);
                let g = |n: &str| {
                    doc.resolve_cell(r, &[r], n)
                        .and_then(|c| c.numeric())
                        .unwrap_or(1.0)
                };
                (g("Width"), g("Height"))
            })
            .unwrap_or((1.0, 1.0));
        if !is_connector_in(doc, &chain, s) {
            continue;
        }
        let frame = hit_frame(doc, &chain, s, msize);
        let master_root = chain.get(1).copied();
        let segs = geometry_hit_segments(s, master_root, &frame);
        if segs.is_empty() {
            continue;
        }
        // The polyline the crossing math walks: the segments' endpoints.
        let mut pts: Vec<(f64, f64)> = Vec::with_capacity(segs.len() + 1);
        pts.push(segs[0].0);
        for (_, b) in &segs {
            pts.push(*b);
        }
        let weight = doc
            .resolve_cell(s, &chain, "LineWeight")
            .and_then(|c| c.numeric())
            .unwrap_or(0.01);
        let color = doc
            .resolve_cell(s, &chain, "LineColor")
            .and_then(|c| c.v.clone())
            .and_then(|v| parse_hex_rgb(&v))
            .unwrap_or((0x42, 0x42, 0x42));
        lines.push(JumpLine {
            id: s.id,
            z,
            weight,
            color,
            pts,
        });
    }
    line_jumps(&lines)
}

#[cfg(test)]
mod tests {
    use super::*;
    use vsdx::model::SheetChild;

    /// #188: a NURBSTo row's hit segments follow the EVALUATED curve
    /// (the E formula's interior points pull the polyline off the
    /// MoveTo→endpoint chord), so a click on the curve's belly hits.
    #[test]
    fn nurbs_hit_segments_leave_the_chord() {
        let mut s = Shape {
            id: 1,
            type_: Some("Shape".into()),
            ..Shape::default()
        };
        for (n, v) in [
            ("PinX", 2.0),
            ("PinY", 1.0),
            ("Width", 4.0),
            ("Height", 2.0),
            ("LocPinX", 2.0),
            ("LocPinY", 1.0),
        ] {
            s.sheet.set_cell(n, Cell::value(format!("{v}")));
        }
        let e = "NURBS(1, 3, 0, 0, 1,0.5,0,1, 3,1.5,0,1)";
        let row = |t: &str, ix: i64, cells: Vec<(&str, f64)>| Row {
            t: Some(t.into()),
            ix,
            cells: cells
                .into_iter()
                .map(|(n, v)| (n.to_string(), Cell::value(format!("{v}"))))
                .collect(),
            ..Row::default()
        };
        let mut sec = Section {
            name: "Geometry".into(),
            ix: 0,
            ..Section::default()
        };
        sec.rows = vec![
            row("MoveTo", 1, vec![("X", 0.0), ("Y", 0.0)]),
            row(
                "NURBSTo",
                2,
                vec![
                    ("X", 4.0),
                    ("Y", 2.0),
                    ("A", 0.0),
                    ("B", 1.0),
                    ("C", 0.0),
                    ("D", 1.0),
                ],
            ),
        ];
        let nurbs_row = &mut sec.rows[1];
        nurbs_row.cells.push((
            "E".to_string(),
            Cell {
                v: Some(e.to_string()),
                f: Some(e.to_string()),
                ..Cell::default()
            },
        ));
        s.sheet
            .children
            .push(SheetChild::Section(std::mem::take(&mut sec)));

        let doc = vsdx::Document::default();
        let chain = vec![&s];
        let frame = hit_frame(&doc, &chain, &s, (1.0, 1.0));
        let segs = geometry_hit_segments(&s, None, &frame);
        // Segments are page space (the frame is identity here); some
        // sample must sit off the (0,0)→(4,2) chord — the interior
        // points pull the belly above it.
        assert!(
            segs.len() > 8,
            "the NURBS hit polyline is dense: {}",
            segs.len()
        );
        let off = segs
            .iter()
            .any(|(a, _)| (0.5 * a.0 + 2.0 - a.1).abs() > 0.05);
        assert!(off, "hit samples leave the chord");
    }

    /// #187: two crossing connectors produce one hop; a rotated
    /// jumper crosses too (slice 2 — the Angle gate is gone).
    #[test]
    fn page_jumps_crosses_axis_and_rotated() {
        let connector = |id: i64, pin: (f64, f64), w: f64, h: f64, angle: f64| {
            let mut s = Shape {
                id,
                type_: Some("Shape".into()),
                ..Shape::default()
            };
            for (n, v) in [
                ("PinX", pin.0),
                ("PinY", pin.1),
                ("Width", w),
                ("Height", h),
                ("LocPinX", w / 2.0),
                ("LocPinY", h / 2.0),
                ("Angle", angle),
                ("ObjType", 2.0),
            ] {
                s.sheet.set_cell(n, Cell::value(format!("{v}")));
            }
            let row = |ix: i64, x: f64, y: f64| Row {
                t: Some("LineTo".into()),
                ix,
                cells: vec![
                    ("X".to_string(), Cell::value(format!("{x}"))),
                    ("Y".to_string(), Cell::value(format!("{y}"))),
                ],
                ..Row::default()
            };
            let mut sec = Section {
                name: "Geometry".into(),
                ix: 0,
                ..Section::default()
            };
            sec.rows = vec![
                Row {
                    t: Some("MoveTo".into()),
                    ix: 1,
                    cells: vec![
                        ("X".to_string(), Cell::value("0")),
                        ("Y".to_string(), Cell::value("0")),
                    ],
                    ..Row::default()
                },
                row(2, w, h),
            ];
            s.sheet.children.push(SheetChild::Section(sec));
            s
        };
        // A horizontal connector (1,1)..(5,1) and a 45°-rotated one
        // through the same page area.
        let mut page = vsdx::Page {
            id: 0,
            name: "T".into(),
            ..vsdx::Page::default()
        };
        page.shapes.push(connector(1, (3.0, 1.0), 4.0, 0.0, 0.0));
        page.shapes.push(connector(
            2,
            (3.0, 1.0),
            3.0,
            0.0,
            -std::f64::consts::FRAC_PI_4,
        ));
        let doc = vsdx::Document::default();
        let jumps = page_jumps(&doc, &page, &[]);
        assert_eq!(jumps.len(), 1, "one crossing: {jumps:?}");
        // The more-horizontal line hops: the AXIS line jumps (dy 0
        // beats the 45° line's 0.707); the rotated one is the under.
        let j = &jumps[0];
        assert!(
            j.dir.1.abs() < 1e-6 && j.under.0.abs() > 0.5,
            "the axis line hops over the rotated one: dir {:?} under {:?}",
            j.dir,
            j.under
        );
    }
}
