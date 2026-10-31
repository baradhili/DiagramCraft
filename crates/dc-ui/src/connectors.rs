//! Connector authoring (GitHub #108, FS-014): the Connector Tool's
//! 1D connector shapes, static/dynamic glue decisions and their
//! corpus-shaped persistence, dynamic attachment and route math, and
//! the reroute command building. Pure logic here (page inches, y-up);
//! the app adapts model geometry in and commands out.
//!
//! Format notes are clean-room from the shipped corpus: the dynamic
//! connector is an *unrotated* shape (`ObjType=2`, `GlueType=2`) with
//! signed `Width = EndX-BeginX` / `Height = EndY-BeginY` and the route
//! carried as Geometry rows in the begin-corner frame; static glue to
//! a connection point writes `EndX F='PAR(PNT(Sheet.N!Connections.Xn,
//! …))'` plus a `<Connect FromPart='12' ToPart='100+row'/>`; dynamic
//! glue-to-shape writes `F='GUARD(_WALKGLUE(EndTrigger,…))'`, trigger
//! cells, and `<Connect ToCell='PinX' ToPart='3'/>` (CONNEC/CAUSEF
//! specimens). `ShapeRouteStyle`: 1 right-angle, 2 straight (the
//! Visio enum; the corpus writes 0 default and 3 flowchart-NS).

use vsdx::model::{Cell, Row, Section, Shape, SheetChild};

/// Corpus `FromPart` codes for the glued connector cells.
pub const PART_BEGIN_X: i64 = 9;
pub const PART_END_X: i64 = 12;
/// Corpus `ToPart` for glue-to-shape (walkable).
pub const PART_WALKGLUE: i64 = 3;
/// `ShapeRouteStyle` values we write (Visio enum).
pub const ROUTE_RIGHT_ANGLE: &str = "1";
pub const ROUTE_STRAIGHT: &str = "2";

/// One end of a connector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum End {
    Begin,
    End,
}

impl End {
    pub fn cell(self) -> &'static str {
        match self {
            End::Begin => "BeginX",
            End::End => "EndX",
        }
    }
    pub fn from_part(self) -> i64 {
        match self {
            End::Begin => PART_BEGIN_X,
            End::End => PART_END_X,
        }
    }
    /// The end's own reroute trigger cell.
    pub fn trigger_cell(self) -> &'static str {
        match self {
            End::Begin => "BegTrigger",
            End::End => "EndTrigger",
        }
    }
    /// The Y companion of `cell()`.
    pub fn y_cell(self) -> &'static str {
        match self {
            End::Begin => "BeginY",
            End::End => "EndY",
        }
    }
}

/// Bake a glue formula onto one endpoint's cells (pre-`AddShape`):
/// the attach position as the cached value, the formula for the file.
pub fn bake_end(shape: &mut Shape, end: End, attach: (f64, f64), f: &str) {
    shape.sheet.set_cell(
        end.cell(),
        Cell {
            v: Some(fmt(attach.0)),
            f: Some(f.to_string()),
            ..Cell::default()
        },
    );
    shape.sheet.set_cell(
        end.y_cell(),
        Cell {
            v: Some(fmt(attach.1)),
            f: Some(f.to_string()),
            ..Cell::default()
        },
    );
}

/// Bake the reroute trigger for dynamic glue (corpus:
/// `GUARD(_XFTRIGGER(Sheet.N!EventXFMod))`).
pub fn bake_trigger(shape: &mut Shape, end: End, target: i64) {
    shape.sheet.set_cell(
        end.trigger_cell(),
        Cell {
            v: Some("2".into()),
            f: Some(format!("GUARD(_XFTRIGGER(Sheet.{target}!EventXFMod))")),
            ..Cell::default()
        },
    );
}

/// Rewrite the three LineTo rows (IX 2..=4) of a connector's geometry
/// to a route (`SetRowCell` writes after creation; this is the
/// pre-creation variant).
pub fn rewrite_route_rows(shape: &mut Shape, rows: &[(f64, f64); 3]) {
    for sec in shape.sheet.children.iter_mut() {
        if let SheetChild::Section(geo) = sec {
            if geo.name != "Geometry" {
                continue;
            }
            for (i, (x, y)) in rows.iter().enumerate() {
                let ix = (i + 2) as i64;
                if let Some(row) = geo.rows.iter_mut().find(|r| r.ix == ix) {
                    for (cell, v) in [("X", *x), ("Y", *y)] {
                        match row.cells.iter_mut().find(|(n, _)| n == cell) {
                            Some((_, c)) => *c = Cell::value(fmt(v)),
                            None => row.cells.push((cell.to_string(), Cell::value(fmt(v)))),
                        }
                    }
                }
            }
            return;
        }
    }
}

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
pub fn is_connector_in(doc: &vsdx::Document, chain: &[&Shape], s: &Shape) -> bool {
    is_connector(s)
        || doc
            .resolve_cell(s, chain, "ObjType")
            .and_then(|c| c.numeric())
            .is_some_and(|v| v == 2.0)
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
pub fn geometry_hit_segments(
    s: &Shape,
    master_root: Option<&Shape>,
    origin: (f64, f64),
    size: (f64, f64),
    master_size: (f64, f64),
) -> Vec<((f64, f64), (f64, f64))> {
    // (rows, owning width/height for Rel rows, x/y scale onto the
    // instance). `origin` is the LOCAL frame's page origin
    // (PinX-LocPinX, PinY-LocPinY) — NOT the normalized bbox corner:
    // the corpus connector's Width is SIGNED (EndX-BeginX), so the
    // begin corner can be the max-x corner and a bbox-corner mapping
    // mirrors the route onto whatever sits left of it (#173).
    let mut sections: Vec<&Section> = s.sheet.sections_named("Geometry");
    let (own, scale) = if sections.is_empty() {
        let Some(root) = master_root else {
            return Vec::new();
        };
        sections = root.sheet.sections_named("Geometry");
        if sections.is_empty() {
            return Vec::new();
        }
        (
            master_size,
            if master_size.0.abs() > 1e-6 && master_size.1.abs() > 1e-6 {
                (size.0 / master_size.0, size.1 / master_size.1)
            } else {
                (1.0, 1.0)
            },
        )
    } else {
        (size, (1.0, 1.0))
    };
    // Row-local → shape-local: relative rows are fractions of the
    // owning size; then scale (master → instance) onto the page rect.
    let local = |t: &str, x: f64, y: f64| -> (f64, f64) {
        if t.starts_with("Rel") {
            (x * own.0, y * own.1)
        } else {
            (x, y)
        }
    };
    let to_page = |p: (f64, f64)| (origin.0 + p.0 * scale.0, origin.1 + p.1 * scale.1);
    let quad = |c0: (f64, f64), ctrl: (f64, f64), c1: (f64, f64), u: f64| -> (f64, f64) {
        (
            (1.0 - u).powi(2) * c0.0 + 2.0 * u * (1.0 - u) * ctrl.0 + u * u * c1.0,
            (1.0 - u).powi(2) * c0.1 + 2.0 * u * (1.0 - u) * ctrl.1 + u * u * c1.1,
        )
    };
    let mut pts: Vec<(f64, f64)> = Vec::new();
    let mut cur: Option<(f64, f64)> = None; // shape-local
    const MAX_ROWS: usize = 64;
    for geo in &sections {
        for row in geo.rows.iter().take(MAX_ROWS) {
            let t = row.t.as_deref().unwrap_or("");
            let (Some(x), Some(y)) = (
                row.cell("X").and_then(|c| c.numeric()),
                row.cell("Y").and_then(|c| c.numeric()),
            ) else {
                continue;
            };
            let end = local(t, x, y);
            match t {
                "MoveTo" | "RelMoveTo" => {
                    cur = Some(end);
                    pts.push(to_page(end));
                }
                "ArcTo" | "RelArcTo" => {
                    // |A| = sagitta, bulging right of travel; the
                    // quadratic control that reproduces the apex.
                    let a = row.cell("A").and_then(|c| c.numeric()).unwrap_or(0.0);
                    if let Some(c0) = cur {
                        let d = (end.0 - c0.0, end.1 - c0.1);
                        let len = d.0.hypot(d.1).max(1e-9);
                        let mid = (
                            (c0.0 + end.0) / 2.0 + d.1 / len * a,
                            (c0.1 + end.1) / 2.0 - d.0 / len * a,
                        );
                        let ctrl = (2.0 * mid.0 - (c0.0 + end.0) / 2.0, 2.0 * mid.1 - (c0.1 + end.1) / 2.0);
                        for u in [0.25, 0.5, 0.75] {
                            pts.push(to_page(quad(c0, ctrl, end, u)));
                        }
                    }
                    pts.push(to_page(end));
                    cur = Some(end);
                }
                "EllipticalArcTo" | "RelEllipticalArcTo" => {
                    // (A,B) is an ON-curve point → quadratic control.
                    match (row.cell("A").and_then(|c| c.numeric()), row.cell("B").and_then(|c| c.numeric())) {
                        (Some(a), Some(b)) if cur.is_some() => {
                            let c0 = cur.unwrap();
                            let ctrl = (2.0 * a - (c0.0 + end.0) / 2.0, 2.0 * b - (c0.1 + end.1) / 2.0);
                            for u in [0.25, 0.5, 0.75] {
                                pts.push(to_page(quad(c0, ctrl, end, u)));
                            }
                        }
                        _ => {}
                    }
                    pts.push(to_page(end));
                    cur = Some(end);
                }
                _ if t.ends_with("To") || t.contains("Spline") || t == "InfiniteLine" => {
                    // LineTo plus the v0.2 chord approximations.
                    pts.push(to_page(end));
                    cur = Some(end);
                }
                _ => {}
            }
        }
    }
    pts.windows(2).map(|w| (w[0], w[1])).collect()
}

fn set(cells: &mut Vec<SheetChild>, n: &str, v: f64) {
    cells.push(SheetChild::Cell(n.to_string(), Cell::value(fmt(v))));
}

fn fmt(v: f64) -> String {
    format!("{v:.6}")
}

/// A synthetic dynamic connector (clean-room cells, no Microsoft
/// content) from `begin` to `end`, page inches y-up: the corpus
/// model — signed Width/Height derived from the endpoints, pin at
/// the midpoint, an end arrowhead, and a fixed 4-row geometry
/// (MoveTo + 3 LineTo) so every later route is plain `SetRowCell`
/// writes (straight = the two elbow rows degenerate onto the end).
pub fn connector_shape(id: i64, begin: (f64, f64), end: (f64, f64)) -> Shape {
    let (dx, dy) = (end.0 - begin.0, end.1 - begin.1);
    let mut s = Shape {
        id,
        type_: Some("Shape".into()),
        name_u: Some(format!("Dynamic connector.{id}")),
        ..Shape::default()
    };
    let cells = &mut s.sheet.children;
    set(cells, "ObjType", 2.0);
    set(cells, "GlueType", 2.0);
    set(cells, "BeginX", begin.0);
    set(cells, "BeginY", begin.1);
    set(cells, "EndX", end.0);
    set(cells, "EndY", end.1);
    set(cells, "PinX", begin.0 + dx / 2.0);
    set(cells, "PinY", begin.1 + dy / 2.0);
    set(cells, "Width", dx);
    set(cells, "Height", dy);
    set(cells, "LocPinX", dx / 2.0);
    set(cells, "LocPinY", dy / 2.0);
    set(cells, "Angle", 0.0);
    set(cells, "FlipX", 0.0);
    set(cells, "FlipY", 0.0);
    set(cells, "ResizeMode", 0.0);
    set(cells, "EndArrow", 4.0);
    set(cells, "ShapeRouteStyle", 1.0); // right-angle, Visio's default look
    // Geometry: begin-corner frame — MoveTo (0,0), then 3 LineTo rows.
    // Straight route: both elbows degenerate onto (dx, dy).
    let mut geo = Section {
        name: "Geometry".into(),
        ix: 0,
        ..Section::default()
    };
    for (n, v) in [("NoFill", "1"), ("NoLine", "0"), ("NoShow", "0"), ("NoSnap", "0")] {
        geo.cells.push((n.to_string(), Cell::value(v)));
    }
    let row = |ix: i64, t: &str, x: f64, y: f64| Row {
        t: Some(t.into()),
        ix,
        cells: vec![
            ("X".to_string(), Cell::value(fmt(x))),
            ("Y".to_string(), Cell::value(fmt(y))),
        ],
        ..Row::default()
    };
    geo.rows.push(row(1, "MoveTo", 0.0, 0.0));
    geo.rows.push(row(2, "LineTo", dx, dy));
    geo.rows.push(row(3, "LineTo", dx, dy));
    geo.rows.push(row(4, "LineTo", dx, dy));
    s.sheet.children.push(SheetChild::Section(geo));
    s
}

/// A glue decision for one endpoint.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GlueKind {
    /// Glued to a specific connection-point row (`ToPart = 100+ix`).
    Static { row_ix: i64 },
    /// Glued to the whole shape — the endpoint walks on reroute.
    Dynamic,
}

/// Where an endpoint glued: the target shape, the glue kind, and the
/// page position the endpoint lands on.
#[derive(Debug, Clone, Copy)]
pub struct GlueTarget {
    pub shape_id: i64,
    pub kind: GlueKind,
    pub attach: (f64, f64),
}

/// One glue candidate: a non-connector shape's page rect and its
/// connection-point rows (row IX → page position).
#[derive(Debug, Clone, Default)]
pub struct GlueCandidate {
    pub id: i64,
    pub rect: (f64, f64, f64, f64),
    pub conn_rows: Vec<(i64, (f64, f64))>,
}

/// The Visio Connector Tool's drop semantics: a connection point
/// within the threshold wins (static glue, `glue_connection_points`
/// gate), otherwise a drop on a shape glues to the whole shape
/// (dynamic — Visio's base capability whenever glue is active).
pub fn glue_decision(
    cands: &[GlueCandidate],
    p: (f64, f64),
    toward: (f64, f64),
    threshold: f64,
    glue_active: bool,
    glue_connection_points: bool,
) -> Option<GlueTarget> {
    if !glue_active {
        return None;
    }
    if glue_connection_points {
        let mut best: Option<(f64, &GlueCandidate, i64, (f64, f64))> = None;
        for c in cands {
            for (ix, q) in &c.conn_rows {
                let d = (q.0 - p.0).hypot(q.1 - p.1);
                if d <= threshold && best.is_none_or(|(bd, ..)| d < bd) {
                    best = Some((d, c, *ix, *q));
                }
            }
        }
        if let Some((_, c, ix, q)) = best {
            return Some(GlueTarget {
                shape_id: c.id,
                kind: GlueKind::Static { row_ix: ix },
                attach: q,
            });
        }
    }
    // Dynamic glue: the drop is on (or at the edge of — within the
    // threshold) the shape. Visio glues a connector dropped on a
    // shape's boundary even when no connection point is involved.
    for c in cands {
        let (x, y, w, h) = c.rect;
        if p.0 >= x - threshold
            && p.0 <= x + w + threshold
            && p.1 >= y - threshold
            && p.1 <= y + h + threshold
        {
            return Some(GlueTarget {
                shape_id: c.id,
                kind: GlueKind::Dynamic,
                attach: attach_point(c.rect, toward),
            });
        }
    }
    None
}

/// The best dynamic attachment on a shape's border: where the segment
/// from the rect's center toward `toward` (the connector's other
/// endpoint) crosses it — the side facing the other shape; nearest
/// border point as the fallback.
pub fn attach_point(rect: (f64, f64, f64, f64), toward: (f64, f64)) -> (f64, f64) {
    let (x, y, w, h) = rect;
    let c = (x + w / 2.0, y + h / 2.0);
    let (dx, dy) = (toward.0 - c.0, toward.1 - c.1);
    if dx.abs() < 1e-9 && dy.abs() < 1e-9 {
        return c; // degenerate: the other end sits on the center
    }
    // Scale the center→toward vector so it reaches the border: the
    // axis whose half-extent runs out first clips.
    let sx = if dx.abs() > 1e-9 { (w / 2.0) / dx.abs() } else { f64::INFINITY };
    let sy = if dy.abs() > 1e-9 { (h / 2.0) / dy.abs() } else { f64::INFINITY };
    let t = sx.min(sy);
    (c.0 + dx * t, c.1 + dy * t)
}

/// The static-glue endpoint formula (corpus): one `PAR(PNT(...))`
/// expression written on both the X and Y cells of the endpoint.
pub fn static_glue_formula(target: i64, row_ix: i64) -> String {
    format!("PAR(PNT(Sheet.{target}!Connections.X{row_ix},Sheet.{target}!Connections.Y{row_ix}))")
}

/// The dynamic-glue endpoint formula (corpus): `_WALKGLUE`.
pub fn walkglue_formula(end: End) -> String {
    let (own, other) = match end {
        End::Begin => ("BegTrigger", "EndTrigger"),
        End::End => ("EndTrigger", "BegTrigger"),
    };
    format!("GUARD(_WALKGLUE({own},{other},WalkPreference))")
}

/// A glue write's `<Connect>` record.
pub fn glue_connect(connector: i64, end: End, target: &GlueTarget) -> dc_core::Connect {
    let (to_cell, to_part) = match target.kind {
        GlueKind::Static { row_ix } => (format!("Connections.X{row_ix}"), 100 + row_ix),
        GlueKind::Dynamic => ("PinX".to_string(), PART_WALKGLUE),
    };
    dc_core::Connect::new(
        connector,
        end.cell(),
        end.from_part(),
        target.shape_id,
        to_cell,
        to_part,
    )
}

/// The route polyline (page inches) between two endpoints under a
/// style: straight is the chord; right-angle runs a Z with the elbow
/// pair on the mid-perpendicular of the dominant axis.
pub fn route(style: &str, begin: (f64, f64), end: (f64, f64)) -> Vec<(f64, f64)> {
    if style == ROUTE_STRAIGHT {
        return vec![begin, end];
    }
    let (dx, dy) = (end.0 - begin.0, end.1 - begin.1);
    if dx.abs() < 1e-6 || dy.abs() < 1e-6 {
        return vec![begin, end]; // already orthogonal
    }
    if dx.abs() >= dy.abs() {
        let mx = begin.0 + dx / 2.0;
        vec![begin, (mx, begin.1), (mx, end.1), end]
    } else {
        let my = begin.1 + dy / 2.0;
        vec![begin, (begin.0, my), (end.0, my), end]
    }
}

/// Geometry rows 2..=4 (local = page − begin) for a route polyline.
/// Straight/degenerate points simply repeat — 4 rows always.
pub fn route_rows(begin: (f64, f64), points: &[(f64, f64)]) -> [(f64, f64); 3] {
    let local = |p: (f64, f64)| (p.0 - begin.0, p.1 - begin.1);
    let last = points[points.len() - 1];
    let e1 = points.get(1).copied().unwrap_or(last);
    let e2 = points.get(2).copied().unwrap_or(points.get(1).copied().unwrap_or(last));
    [local(e1), local(e2), local(last)]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: (f64, f64), b: (f64, f64)) -> bool {
        (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9
    }

    /// #172: a master-based connector carries `ObjType=2` on its
    /// master root — the chain-aware check must see it.
    #[test]
    fn is_connector_in_resolves_the_master() {
        let mut root = Shape {
            id: 1,
            type_: Some("Shape".into()),
            ..Shape::default()
        };
        root.sheet.set_cell("ObjType", Cell::value("2"));
        let mut m = vsdx::Master::default();
        m.shapes.push(root);
        let mut doc = vsdx::Document::default();
        doc.masters.push(m);
        let inst = Shape {
            id: 5,
            type_: Some("Shape".into()),
            master: Some(0),
            ..Shape::default()
        };
        assert!(!is_connector(&inst), "locally it is not marked");
        let chain: Vec<&Shape> = vec![&inst];
        assert!(
            is_connector_in(&doc, &chain, &inst),
            "the master's ObjType=2 resolves through the chain"
        );
    }

    /// #172: the hit segments follow the geometry rows — an arc's
    /// belly gets sampled points OFF the chord, so a click there can
    /// hit the connector instead of a neighbor's bbox.
    #[test]
    fn hit_segments_sample_the_arc_belly() {
        let mut s = Shape::default();
        let mut geo = Section {
            name: "Geometry".into(),
            ix: 0,
            ..Section::default()
        };
        let row = |ix: i64, t: &str, cells: &[(&str, &str)]| Row {
            t: Some(t.into()),
            ix,
            cells: cells
                .iter()
                .map(|(n, v)| (n.to_string(), Cell::value(v.to_string())))
                .collect(),
            ..Row::default()
        };
        // 0..4 wide, arc from (0,0) to (4,0) bulging up through (2,2).
        geo.rows.push(row(1, "MoveTo", &[("X", "0"), ("Y", "0")]));
        geo.rows.push(row(
            2,
            "EllipticalArcTo",
            &[("X", "4"), ("Y", "0"), ("A", "2"), ("B", "2")],
        ));
        s.sheet.children.push(SheetChild::Section(geo));
        let segs = geometry_hit_segments(&s, None, (10.0, 10.0), (4.0, 4.0), (1.0, 1.0));
        assert!(!segs.is_empty(), "the arc produced segments");
        // The sampled interior sits above the chord (y > 10 in page
        // space, y-up): the belly is covered.
        let belly = segs
            .iter()
            .any(|(a, b)| a.1 > 10.05 || b.1 > 10.05);
        assert!(belly, "interior samples leave the chord: {segs:?}");
    }

    /// #172: with no local rows the MASTER root's geometry stands in,
    /// scaled from the master's size to the instance rect.
    #[test]
    fn hit_segments_fall_back_to_master_rows_scaled() {
        let inst = Shape {
            id: 2,
            type_: Some("Shape".into()),
            ..Shape::default()
        };
        let mut root = Shape {
            id: 1,
            type_: Some("Shape".into()),
            ..Shape::default()
        };
        root.sheet.set_cell("Width", Cell::value("2"));
        root.sheet.set_cell("Height", Cell::value("2"));
        let mut geo = Section {
            name: "Geometry".into(),
            ix: 0,
            ..Section::default()
        };
        let row = |ix: i64, t: &str, x: &str, y: &str| Row {
            t: Some(t.into()),
            ix,
            cells: vec![
                ("X".to_string(), Cell::value(x.to_string())),
                ("Y".to_string(), Cell::value(y.to_string())),
            ],
            ..Row::default()
        };
        geo.rows.push(row(1, "MoveTo", "0", "0"));
        geo.rows.push(row(2, "LineTo", "2", "2"));
        root.sheet.children.push(SheetChild::Section(geo));
        // Instance rect is 2x the master: local (2,2) → page (10+4,
        // 10+4).
        let segs = geometry_hit_segments(&inst, Some(&root), (10.0, 10.0), (4.0, 4.0), (2.0, 2.0));
        assert_eq!(
            segs,
            vec![((10.0, 10.0), (14.0, 14.0))],
            "master rows scale onto the instance rect"
        );
    }

    #[test]
    fn connector_shape_carries_the_corpus_model() {
        let s = connector_shape(7, (1.0, 2.0), (3.0, 5.0));
        assert!(is_connector(&s));
        let get = |n: &str| s.sheet.cell(n).and_then(|c| c.numeric()).unwrap();
        assert_eq!(get("ObjType") as i64, 2);
        assert_eq!(get("GlueType") as i64, 2);
        assert!((get("Width") - 2.0).abs() < 1e-9);
        assert!((get("Height") - 3.0).abs() < 1e-9);
        assert!((get("PinX") - 2.0).abs() < 1e-9 && (get("PinY") - 3.5).abs() < 1e-9);
        // 4-row geometry, straight out of the box.
        let geo = s.sheet.sections_named("Geometry")[0];
        assert_eq!(geo.rows.len(), 4);
        let last = geo.rows[3].cell("X").and_then(|c| c.numeric()).unwrap();
        assert!((last - 2.0).abs() < 1e-9);
        // Signed Height: end left/below the begin still works.
        let s = connector_shape(8, (3.0, 5.0), (1.0, 2.0));
        let get = |n: &str| s.sheet.cell(n).and_then(|c| c.numeric()).unwrap();
        assert!((get("Width") + 2.0).abs() < 1e-9 && (get("Height") + 3.0).abs() < 1e-9);
    }

    #[test]
    fn static_glue_beats_dynamic_near_a_connection_point() {
        let cands = [GlueCandidate {
            id: 5,
            rect: (0.0, 0.0, 2.0, 2.0),
            conn_rows: vec![(1, (2.0, 1.0))], // right edge middle
        }];
        // Near the row → static.
        let t = glue_decision(&cands, (2.02, 1.01), (5.0, 1.0), 0.08, true, true).unwrap();
        assert_eq!(t.shape_id, 5);
        assert_eq!(t.kind, GlueKind::Static { row_ix: 1 });
        assert!(near(t.attach, (2.0, 1.0)));
        // Inside the shape, far from the row → dynamic, on the facing
        // side (toward (5,1) → the right edge).
        let t = glue_decision(&cands, (0.5, 1.0), (5.0, 1.0), 0.08, true, true).unwrap();
        assert_eq!(t.kind, GlueKind::Dynamic);
        assert!(near(t.attach, (2.0, 1.0)));
        // Connection-point glue disabled → dynamic only.
        let t = glue_decision(&cands, (2.02, 1.01), (5.0, 1.0), 0.08, true, false).unwrap();
        assert_eq!(t.kind, GlueKind::Dynamic);
        // Off every shape → no glue.
        assert!(glue_decision(&cands, (9.0, 9.0), (5.0, 1.0), 0.08, true, true).is_none());
        // Glue inactive → nothing, even on a shape.
        assert!(glue_decision(&cands, (0.5, 1.0), (5.0, 1.0), 0.08, false, true).is_none());
    }

    #[test]
    fn attach_picks_the_facing_side() {
        let r = (0.0, 0.0, 2.0, 1.0);
        assert!(near(attach_point(r, (5.0, 0.5)), (2.0, 0.5)), "right side");
        assert!(near(attach_point(r, (-5.0, 0.5)), (0.0, 0.5)), "left side");
        assert!(near(attach_point(r, (1.0, 9.0)), (1.0, 1.0)), "top");
        assert!(near(attach_point(r, (1.0, -9.0)), (1.0, 0.0)), "bottom");
        // The other end inside the shape → degenerate center.
        assert!(near(attach_point(r, (1.0, 0.5)), (1.0, 0.5)));
    }

    #[test]
    fn static_formula_matches_the_corpus_shape() {
        assert_eq!(
            static_glue_formula(3, 2),
            "PAR(PNT(Sheet.3!Connections.X2,Sheet.3!Connections.Y2))"
        );
        assert_eq!(
            walkglue_formula(End::End),
            "GUARD(_WALKGLUE(EndTrigger,BegTrigger,WalkPreference))"
        );
        assert_eq!(
            walkglue_formula(End::Begin),
            "GUARD(_WALKGLUE(BegTrigger,EndTrigger,WalkPreference))"
        );
    }

    #[test]
    fn connect_records_carry_corpus_part_codes() {
        let t = GlueTarget {
            shape_id: 4,
            kind: GlueKind::Static { row_ix: 1 },
            attach: (0.0, 0.0),
        };
        let c = glue_connect(9, End::End, &t);
        assert_eq!(
            (c.from_sheet, c.from_cell.as_str(), c.from_part),
            (9, "EndX", 12)
        );
        assert_eq!(
            (c.to_sheet, c.to_cell.as_str(), c.to_part),
            (4, "Connections.X1", 101)
        );
        let t = GlueTarget {
            shape_id: 4,
            kind: GlueKind::Dynamic,
            attach: (0.0, 0.0),
        };
        let c = glue_connect(9, End::Begin, &t);
        assert_eq!(
            (c.from_part, c.to_cell.as_str(), c.to_part),
            (9, "PinX", 3)
        );
    }

    #[test]
    fn routes_are_straight_or_z_orthogonal() {
        let b = (0.0, 0.0);
        assert_eq!(route(ROUTE_STRAIGHT, b, (2.0, 2.0)), vec![b, (2.0, 2.0)]);
        // Horizontal-dominant: elbows on the mid-x.
        let r = route(ROUTE_RIGHT_ANGLE, b, (2.0, 1.0));
        assert_eq!(r, vec![b, (1.0, 0.0), (1.0, 1.0), (2.0, 1.0)]);
        // Vertical-dominant: elbows on the mid-y.
        let r = route(ROUTE_RIGHT_ANGLE, b, (1.0, 2.0));
        assert_eq!(r, vec![b, (0.0, 1.0), (1.0, 1.0), (1.0, 2.0)]);
        // Already orthogonal → chord.
        assert_eq!(route(ROUTE_RIGHT_ANGLE, b, (2.0, 0.0)), vec![b, (2.0, 0.0)]);
        // Rows are begin-relative; short routes repeat the end.
        assert_eq!(route_rows(b, &[b, (2.0, 1.0)]), [(2.0, 1.0), (2.0, 1.0), (2.0, 1.0)]);
        let pts = route(ROUTE_RIGHT_ANGLE, b, (2.0, 1.0));
        assert_eq!(route_rows(b, &pts), [(1.0, 0.0), (1.0, 1.0), (2.0, 1.0)]);
    }
}
