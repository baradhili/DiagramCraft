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
/// Center-to-center (#108's curved routing): the Visio enum value
/// whose routes render as a smooth curve. The curve itself persists
/// in the geometry — row 2 becomes a `CubicBezTo` whose A/B/C/D
/// controls are Width/Height formulas (mid-x, the endpoints' own
/// heights — an S, never the chord-collinear straight), so reroutes
/// (which rewrite only the X/Y/size cells) keep the curve tracking
/// the frame.
pub const ROUTE_CURVED: &str = "16";

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

// #187: the geometry walk, crossing math, and connector checks moved to
// `dc::geom` (the export overlay computes crossings on the dc side);
// re-exported here so every dc-ui call site and test stays put.
pub use dc::geom::{
    arc_row, cubic_row, geometry_hit_segments, hit_frame, is_connector, is_connector_in,
    line_jumps, row_cell_value, seg_cross, ArcRow, CubicRow, HitFrame, Jump, JumpLine,
    JUMP_RADIUS_IN,
};
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
    for (n, v) in [
        ("NoFill", "1"),
        ("NoLine", "0"),
        ("NoShow", "0"),
        ("NoSnap", "0"),
    ] {
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

/// One connector's line-jump inputs (#108): its page-space polyline
/// (y-up inches) and stroke, in z order (page order).
#[derive(Debug, Clone)]
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
    glue_decision_ex(
        cands,
        p,
        toward,
        threshold,
        glue_active,
        glue_connection_points,
        true,
    )
}

/// [`glue_decision`] with the dynamic-glue halo control (#202): the
/// endpoint-drop commit passes `halo = false` — Visio glues a dragged
/// endpoint to a shape only when the drop lands ON it; the
/// threshold-wide ring around every shape made the endpoints feel
/// hard-glued (they re-attached without the cursor ever touching the
/// shape) and confined them to the shape's edges afterwards. The
/// Connector Tool keeps the halo (drawing a connector ONTO a shape's
/// vicinity is the tool's whole gesture).
pub fn glue_decision_ex(
    cands: &[GlueCandidate],
    p: (f64, f64),
    toward: (f64, f64),
    threshold: f64,
    glue_active: bool,
    glue_connection_points: bool,
    halo: bool,
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
    let pad = if halo { threshold } else { 0.0 };
    for c in cands {
        let (x, y, w, h) = c.rect;
        if p.0 >= x - pad && p.0 <= x + w + pad && p.1 >= y - pad && p.1 <= y + h + pad {
            return Some(GlueTarget {
                shape_id: c.id,
                kind: GlueKind::Dynamic,
                attach: attach_point(c.rect, toward),
            });
        }
    }
    None
}

/// The connection-point affordances for one frame (#177): what the
/// canvas paints while a connector endpoint gesture is live and what
/// it paints for a selected glued connector. All positions are page
/// inches, y-up; nothing here persists.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ConnPointFx {
    /// Connection points of the approached shapes — the square-with-×
    /// reveal symbols (Visio shows the shape's points as the endpoint
    /// nears it).
    pub reveal: Vec<(f64, f64)>,
    /// The connection point the drop would statically glue to — the
    /// live green-square highlight.
    pub glue_target: Option<(f64, f64)>,
    /// The glued attachment sites of the selected connector — green
    /// circles.
    pub glued: Vec<(f64, f64)>,
    /// The connection point selected by the Connection Point tool
    /// (#108) — the selection ring awaiting Delete.
    pub tool_selected: Option<(f64, f64)>,
}

/// Reveal + glue-target highlight for a live endpoint position
/// (#177): every candidate whose rect (inflated by `near_in`)
/// contains the point reveals its connection points, and when the
/// drop semantics would glue to a connection point that point is the
/// highlight. `toward` only shapes dynamic attach points, which the
/// overlay does not draw — pass the point itself when unknown.
pub fn conn_point_fx(
    cands: &[GlueCandidate],
    p: (f64, f64),
    toward: (f64, f64),
    threshold: f64,
    near_in: f64,
    glue_active: bool,
    glue_connection_points: bool,
) -> ConnPointFx {
    let mut fx = ConnPointFx::default();
    if !glue_active {
        return fx;
    }
    for c in cands {
        let (x, y, w, h) = c.rect;
        if p.0 >= x - near_in
            && p.0 <= x + w + near_in
            && p.1 >= y - near_in
            && p.1 <= y + h + near_in
        {
            fx.reveal.extend(c.conn_rows.iter().map(|(_, q)| *q));
        }
    }
    if glue_connection_points {
        if let Some(t) = glue_decision(cands, p, toward, threshold, true, true) {
            if matches!(t.kind, GlueKind::Static { .. }) {
                fx.glue_target = Some(t.attach);
            }
        }
    }
    fx
}

/// The glued attachment sites of a connector (#177): one per
/// `<Connect>` record — static glue (ToPart 100+row) sits on the
/// target's connection point (looked up in the same candidates the
/// drop path uses); dynamic glue-to-shape (ToPart 3) sits at the
/// endpoint's resolved position.
pub fn glued_sites(
    doc: &vsdx::model::Document,
    page: &vsdx::model::Page,
    conn_id: i64,
    cands: &[GlueCandidate],
) -> Vec<(f64, f64)> {
    let mut out = Vec::new();
    use dc_core::connects::ConnectsHost;
    for c in page.connects().list.iter() {
        if c.from_sheet != conn_id {
            continue;
        }
        if c.to_part >= 100 {
            let row_ix = c.to_part - 100;
            if let Some(q) = cands
                .iter()
                .find(|cand| cand.id == c.to_sheet)
                .and_then(|cand| cand.conn_rows.iter().find(|(ix, _)| *ix == row_ix))
            {
                out.push(q.1);
            }
        } else {
            // Dynamic: the endpoint's own resolved position.
            let Some(conn) = vsdx::model::find_shape_by_id(&page.shapes, conn_id) else {
                continue;
            };
            let Ok(chain) = dc_core::scope::shape_chain(doc, page.id, conn_id) else {
                continue;
            };
            let (xn, yn) = if c.from_cell == "BeginX" {
                ("BeginX", "BeginY")
            } else {
                ("EndX", "EndY")
            };
            if let (Some(x), Some(y)) = (
                doc.resolve_cell(conn, &chain, xn).and_then(|v| v.numeric()),
                doc.resolve_cell(conn, &chain, yn).and_then(|v| v.numeric()),
            ) {
                out.push((x, y));
            }
        }
    }
    out
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
    let sx = if dx.abs() > 1e-9 {
        (w / 2.0) / dx.abs()
    } else {
        f64::INFINITY
    };
    let sy = if dy.abs() > 1e-9 {
        (h / 2.0) / dy.abs()
    } else {
        f64::INFINITY
    };
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

/// #180: the documented Visio unglue-all operation — "select one or
/// more connectors and press any arrow key" ([MS Learn] Glue or unglue
/// connectors; the page's Ctrl is for multi-SELECTING, not a key
/// modifier). Builds every listed connector's release as one batch:
/// plain-value [`SetGluedCell`] writes that replace the glue formulas
/// at the ends' RESOLVED positions (the connector does not move), and
/// [`dc_core::RemoveConnect`]s for the connectors' records — highest
/// index first, because each removal shifts the tail. Locked ends
/// refuse inside `SetGluedCell`; the caller wraps the batch in one
/// transactional `Macro`, so a refusal rolls the whole operation back.
/// Returns the commands and the ids of the connectors that release
/// (empty = nothing glued; the caller falls through to nudge).
pub fn unglue_cmds(
    doc: &vsdx::model::Document,
    page: &vsdx::model::Page,
    ids: &[i64],
) -> (Vec<Box<dyn dc_core::Command>>, Vec<i64>) {
    use dc_core::connects::ConnectsHost;
    let connects = page.connects();
    let mut cmds: Vec<Box<dyn dc_core::Command>> = Vec::new();
    let mut releases: Vec<usize> = Vec::new();
    let mut released: Vec<i64> = Vec::new();
    for &sid in ids {
        let Some(sh) = vsdx::model::find_shape_by_id(&page.shapes, sid) else {
            continue;
        };
        let Ok(chain) = dc_core::scope::shape_chain(doc, page.id, sid) else {
            continue;
        };
        if !is_connector_in(doc, &chain, sh) {
            continue;
        }
        let mut any = false;
        for end in [End::Begin, End::End] {
            // A glued end: its X cell carries the glue formula (static
            // `PAR(PNT(...))` or dynamic `_WALKGLUE`) or a Connect
            // record names it — either way the record drops and the
            // formula yields to its resolved value (the body-drag
            // release's write set, without the drag).
            let glued = dc_core::commands::effective_formula(doc, sh, &chain, end.cell()).is_some()
                || connects
                    .list
                    .iter()
                    .any(|c| c.from_sheet == sid && c.from_cell == end.cell());
            if !glued {
                continue;
            }
            let v = |n: &str| doc.resolve_cell(sh, &chain, n).and_then(|c| c.numeric());
            let (Some(x), Some(y)) = (v(end.cell()), v(end.y_cell())) else {
                continue;
            };
            any = true;
            for (name, val) in [(end.cell(), x), (end.y_cell(), y)] {
                cmds.push(
                    Box::new(dc_core::SetGluedCell::new(
                        dc_core::Scope::Shape {
                            page: page.id,
                            shape: sid,
                        },
                        name,
                        Cell::value(fmt(val)),
                    )) as Box<dyn dc_core::Command>,
                );
            }
        }
        if any {
            released.push(sid);
            releases.extend(
                connects
                    .list
                    .iter()
                    .enumerate()
                    .filter(|(_, c)| {
                        c.from_sheet == sid
                            && matches!(c.from_cell.as_str(), "BeginX" | "EndX")
                    })
                    .map(|(i, _)| i),
            );
        }
    }
    releases.sort_unstable_by(|a, b| b.cmp(a));
    releases.dedup();
    for idx in releases {
        cmds.push(Box::new(dc_core::RemoveConnect::new(
            dc_core::ConnectsOwner::Page(page.id),
            idx,
        )));
    }
    (cmds, released)
}


/// The route polyline (page inches) between two endpoints under a
/// style: straight is the chord; right-angle runs a Z with the elbow
/// pair on the mid-perpendicular of the dominant axis; curved returns
/// the chord too — the curve rides the `CubicBezTo` A/B/C/D cells
/// ([`curved_row_extras`]), not extra polyline points.
pub fn route(style: &str, begin: (f64, f64), end: (f64, f64)) -> Vec<(f64, f64)> {
    if style == ROUTE_STRAIGHT || style == ROUTE_CURVED {
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

/// The curved route's Bézier control cells for geometry row 2
/// (#108): both controls sit at the frame's mid-x with the BEGIN's
/// and END's own heights — horizontal end tangents, the smooth S of
/// Visio's center-to-center look. Chord-THIRD controls would be
/// collinear with the endpoints and render as a straight segment
/// (CodeRabbit). Written as FORMULAS of the frame so a reroute's
/// size rewrite re-tracks them automatically. Returns (cell, cached
/// value) pairs in row-local inches for the current `dx`/`dy`.
pub fn curved_row_extras(dx: f64, dy: f64) -> [(&'static str, String, f64); 4] {
    [
        ("A", "Width*0.5".to_string(), dx / 2.0),
        ("B", "Height*0".to_string(), 0.0),
        ("C", "Width*0.5".to_string(), dx / 2.0),
        ("D", "Height*1".to_string(), dy),
    ]
}

/// Geometry rows 2..=4 (local = page − begin) for a route polyline.
/// Straight/degenerate points simply repeat — 4 rows always.
pub fn route_rows(begin: (f64, f64), points: &[(f64, f64)]) -> [(f64, f64); 3] {
    let local = |p: (f64, f64)| (p.0 - begin.0, p.1 - begin.1);
    let last = points[points.len() - 1];
    let e1 = points.get(1).copied().unwrap_or(last);
    let e2 = points
        .get(2)
        .copied()
        .unwrap_or(points.get(1).copied().unwrap_or(last));
    [local(e1), local(e2), local(last)]
}


/// The localized endpoint word (#257 value-side vocabulary): the
/// endpoint macros' labels carry the reading-order word, not the cell
/// name (which stays model vocabulary).
pub fn endpoint_word(end: End) -> String {
    match end {
        End::Begin => crate::i18n::strs().msg_endpoint_begin(),
        End::End => crate::i18n::strs().msg_endpoint_end(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: (f64, f64), b: (f64, f64)) -> bool {
        (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9
    }

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
        // Many samples along the curve, and some OFF the (0,0)→(4,2)
        // chord (the interior points pull the belly up through
        // (1,0.5)/(3,1.5) — chord y at x=1 is 0.5, belly is above it).
        assert!(
            segs.len() > 8,
            "the NURBS hit polyline is dense: {}",
            segs.len()
        );
        // Segments are page space (the frame is identity here); some
        // sample must sit off the (0,0)→(4,2) chord — the interior
        // points pull the belly above it.
        let off = segs
            .iter()
            .any(|(a, _)| (0.5 * a.0 + 2.0 - a.1).abs() > 0.05);
        assert!(off, "hit samples leave the chord");
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
    /// A plain axis-aligned frame at `origin` with the given sizes
    /// (rotation-free hit tests).
    fn plain_frame(origin: (f64, f64), size: (f64, f64), master_size: (f64, f64)) -> HitFrame {
        HitFrame {
            pin: (origin.0 + size.0 / 2.0, origin.1 + size.1 / 2.0),
            loc_pin: (size.0 / 2.0, size.1 / 2.0),
            size,
            master_size,
            angle: 0.0,
            flip: (1.0, 1.0),
            env: vsdx::formula::Env::new(),
        }
    }

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
        let segs =
            geometry_hit_segments(&s, None, &plain_frame((10.0, 10.0), (4.0, 4.0), (1.0, 1.0)));
        assert!(!segs.is_empty(), "the arc produced segments");
        // The sampled interior sits above the chord (y > 10 in page
        // space, y-up): the belly is covered.
        let belly = segs.iter().any(|(a, b)| a.1 > 10.05 || b.1 > 10.05);
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
        let segs = geometry_hit_segments(
            &inst,
            Some(&root),
            &plain_frame((10.0, 10.0), (4.0, 4.0), (2.0, 2.0)),
        );
        assert_eq!(
            segs,
            vec![((10.0, 10.0), (14.0, 14.0))],
            "master rows scale onto the instance rect"
        );
    }

    /// #179: a corpus arc connector (utilisation.vsdx) carries local
    /// rows whose Y cells are ABSENT and X/A/B are `F="Inh"` — the
    /// coordinates live in the master's formulas. The walk must
    /// resolve them (formula in instance terms / inherited value),
    /// not drop the rows.
    #[test]
    fn hit_segments_resolve_inherited_row_cells() {
        let mut inst = Shape {
            id: 2,
            type_: Some("Shape".into()),
            master: Some(0),
            ..Shape::default()
        };
        inst.sheet.set_cell("Width", Cell::value("2"));
        inst.sheet.set_cell("Height", Cell::value("1"));
        let mut geo = Section {
            name: "Geometry".into(),
            ix: 0,
            ..Section::default()
        };
        // The owner-file pattern: MoveTo with only X (F=Inh), an
        // EllipticalArcTo with X/A/B and NO Y anywhere on the
        // instance.
        let inh = || Cell {
            f: Some("Inh".into()),
            ..Cell::default()
        };
        geo.rows.push(Row {
            t: Some("MoveTo".into()),
            ix: 1,
            cells: vec![("X".to_string(), inh())],
            ..Row::default()
        });
        geo.rows.push(Row {
            t: Some("EllipticalArcTo".into()),
            ix: 2,
            cells: vec![
                ("X".to_string(), inh()),
                ("A".to_string(), inh()),
                ("B".to_string(), inh()),
            ],
            ..Row::default()
        });
        inst.sheet.children.push(SheetChild::Section(geo));
        // The master: MoveTo (0, Height*0), EllipticalArcTo
        // (Width*1, Height*0) through (Width*0.5, Height*1) — a
        // half-height arc over the full instance width.
        let mut root = Shape {
            id: 1,
            type_: Some("Shape".into()),
            ..Shape::default()
        };
        root.sheet.set_cell("Width", Cell::value("1"));
        root.sheet.set_cell("Height", Cell::value("1"));
        let fcell = |f: &str| Cell {
            f: Some(f.into()),
            ..Cell::default()
        };
        let mut mgeo = Section {
            name: "Geometry".into(),
            ix: 0,
            ..Section::default()
        };
        mgeo.rows.push(Row {
            t: Some("MoveTo".into()),
            ix: 1,
            cells: vec![
                ("X".to_string(), fcell("Width*0")),
                ("Y".to_string(), fcell("Height*0")),
            ],
            ..Row::default()
        });
        mgeo.rows.push(Row {
            t: Some("EllipticalArcTo".into()),
            ix: 2,
            cells: vec![
                ("X".to_string(), fcell("Width*1")),
                ("Y".to_string(), fcell("Height*0")),
                ("A".to_string(), fcell("Width*0.5")),
                ("B".to_string(), fcell("Height*1")),
            ],
            ..Row::default()
        });
        root.sheet.children.push(SheetChild::Section(mgeo));
        // Instance is 2x1 at origin (10, 10): the arc runs (10,10) →
        // (12,10) bulging through (11, 11) — page-space belly y > 10.
        let frame = HitFrame {
            pin: (11.0, 10.5),
            loc_pin: (1.0, 0.5),
            size: (2.0, 1.0),
            master_size: (1.0, 1.0),
            angle: 0.0,
            flip: (1.0, 1.0),
            env: [("Width".to_string(), 2.0), ("Height".to_string(), 1.0)]
                .into_iter()
                .collect(),
        };
        let segs = geometry_hit_segments(&inst, Some(&root), &frame);
        assert!(!segs.is_empty(), "inherited rows still produce segments");
        let last = segs.last().unwrap().1;
        assert!(
            (last.0 - 12.0).abs() < 1e-9 && (last.1 - 10.0).abs() < 1e-9,
            "the arc's end lands on (Width, 0) = (12, 10): {last:?}"
        );
        assert!(
            segs.iter().any(|(a, b)| a.1 > 10.5 || b.1 > 10.5),
            "the master's Height*1 formula bulges the belly in INSTANCE terms: {segs:?}"
        );
    }

    /// #179: a rotated connector (utilisation's shape 1017 carries
    /// Angle ≈ −0.98 rad) — the segments must rotate with the ink,
    /// and `page_bounds` must widen past the unrotated rect.
    #[test]
    fn hit_segments_rotate_by_angle() {
        let mut s = Shape::default();
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
        geo.rows.push(row(2, "LineTo", "2", "0"));
        s.sheet.children.push(SheetChild::Section(geo));
        // A flat 2×1 box rotated 90° around the pin (1, 10): the
        // local +x axis maps to page +y. Local (0,0) is (−1,−0.5)
        // from the loc pin → rotated to (0.5,−1) → page (1.5, 9);
        // local (2,0) → page (1.5, 11).
        let frame = HitFrame {
            pin: (1.0, 10.0),
            loc_pin: (1.0, 0.5),
            size: (2.0, 1.0),
            master_size: (2.0, 1.0),
            angle: std::f64::consts::FRAC_PI_2,
            flip: (1.0, 1.0),
            env: vsdx::formula::Env::new(),
        };
        let segs = geometry_hit_segments(&s, None, &frame);
        assert_eq!(segs.len(), 1, "one segment");
        let (a, b) = segs[0];
        assert!(
            (a.0 - 1.5).abs() < 1e-9 && (a.1 - 9.0).abs() < 1e-9,
            "the begin rotated 90°: {a:?}"
        );
        assert!(
            (b.0 - 1.5).abs() < 1e-9 && (b.1 - 11.0).abs() < 1e-9,
            "the end rotated onto +y: {b:?}"
        );
        // Unrotated the box spans 2 wide × 1 tall; rotated the spans
        // SWAP — the gate must follow the rotation (#179).
        let (x0, y0, x1, y1) = frame.page_bounds();
        assert!(
            (x1 - x0 - 1.0).abs() < 1e-9 && (y1 - y0 - 2.0).abs() < 1e-9,
            "the rotated bounds swap the axis spans: {x0}..{x1} × {y0}..{y1}"
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
        assert!(near(attach_point(r, (1.0, 9.0)), (1.0, 1.0)), "{}", crate::i18n::strs().msg_align_top_kind());
        assert!(near(attach_point(r, (1.0, -9.0)), (1.0, 0.0)), "{}", crate::i18n::strs().msg_align_bottom_kind());
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
        assert_eq!((c.from_part, c.to_cell.as_str(), c.to_part), (9, "PinX", 3));
    }

    /// #108: line jumps — a proper crossing hops the horizontal line;
    /// a tie hops the later-in-z line; parallel/near-touching pairs
    /// never hop.
    #[test]
    fn line_jumps_hop_the_horizontal_line() {
        let l = |id: i64, z: usize, pts: &[(f64, f64)]| JumpLine {
            id,
            z,
            weight: 0.01,
            color: (0, 0, 0),
            pts: pts.to_vec(),
        };
        // H crosses V at (1, 1): H hops.
        let lines = [
            l(1, 0, &[(0.0, 1.0), (2.0, 1.0)]),
            l(2, 1, &[(1.0, 0.0), (1.0, 2.0)]),
        ];
        let js = line_jumps(&lines);
        assert_eq!(js.len(), 1);
        assert_eq!(js[0].at, (1.0, 1.0));
        assert!((js[0].dir.0 - 1.0).abs() < 1e-9, "the horizontal hops");
        // Parallel lines never hop.
        assert!(line_jumps(&[
            l(1, 0, &[(0.0, 0.0), (2.0, 0.0)]),
            l(2, 1, &[(0.0, 1.0), (2.0, 1.0)]),
        ])
        .is_empty());
        // Perpendicular tie (both diagonal): the later z hops.
        let js = line_jumps(&[
            l(1, 0, &[(0.0, 0.0), (2.0, 2.0)]),
            l(2, 5, &[(0.0, 2.0), (2.0, 0.0)]),
        ]);
        assert_eq!(js.len(), 1);
        // The later-in-z line 2 hops: its direction (0,2)->(2,0)
        // normalizes to (1/sqrt(2), -1/sqrt(2)).
        let sq = 1.0 / (2.0f64).sqrt();
        assert!(
            (js[0].dir.0 - sq).abs() < 1e-9,
            "the later-z line hops: dir.x = {}",
            js[0].dir.0
        );
        assert!(
            (js[0].dir.1 - (-sq)).abs() < 1e-9,
            "the later-z line hops: dir.y = {}",
            js[0].dir.1
        );
        // Non-crossing segments never hop.
        assert!(line_jumps(&[
            l(1, 0, &[(0.0, 0.0), (1.0, 0.0)]),
            l(2, 1, &[(2.0, 0.0), (2.0, 2.0)]),
        ])
        .is_empty());
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
        assert_eq!(
            route_rows(b, &[b, (2.0, 1.0)]),
            [(2.0, 1.0), (2.0, 1.0), (2.0, 1.0)]
        );
        let pts = route(ROUTE_RIGHT_ANGLE, b, (2.0, 1.0));
        assert_eq!(route_rows(b, &pts), [(1.0, 0.0), (1.0, 1.0), (2.0, 1.0)]);
    }

    /// #108 curved routing: the polyline is the chord (the curve rides
    /// the Bézier cells), and the A/B/C/D extras are mid-x FORMULAS —
    /// an S with horizontal end tangents, never chord-collinear
    /// (collinear controls render straight — CodeRabbit), and a
    /// reroute's size rewrite re-tracks them.
    #[test]
    fn curved_route_is_the_chord_with_bezier_formula_extras() {
        let b = (0.0, 0.0);
        assert_eq!(route(ROUTE_CURVED, b, (2.0, 1.0)), vec![b, (2.0, 1.0)]);
        let extras = curved_row_extras(2.0, 1.0);
        assert_eq!(extras[0].0, "A");
        assert_eq!(extras[0].1, "Width*0.5");
        assert!((extras[0].2 - 1.0).abs() < 1e-9);
        assert_eq!(extras[3].0, "D");
        assert_eq!(extras[3].1, "Height*1");
        assert!((extras[3].2 - 1.0).abs() < 1e-9);
        // Off the chord: (1,0) and (1,1) leave the (0,0)→(2,1) line.
        let (dx, dy) = (2.0, 1.0);
        for (x, y) in [(extras[0].2, extras[1].2), (extras[2].2, extras[3].2)] {
            let cross = dx * y - dy * x; // zero on the chord
            assert!(cross.abs() > 1e-9, "control ({x},{y}) is ON the chord");
        }
        // The renderer draws row 2 as a true cubic Bézier once its
        // type is CubicBezTo with these cells.
        let mut s = connector_shape(1, b, (2.0, 1.0));
        let geo = s
            .sheet
            .children
            .iter_mut()
            .find_map(|c| match c {
                SheetChild::Section(sec) if sec.name == "Geometry" => Some(sec),
                _ => None,
            })
            .unwrap();
        geo.rows[1].t = Some("CubicBezTo".into());
        for (name, f, v) in curved_row_extras(2.0, 1.0) {
            geo.rows[1].cells.push((
                name.to_string(),
                Cell {
                    v: Some(fmt(v)),
                    f: Some(f),
                    ..Cell::default()
                },
            ));
        }
        let doc = vsdx::Document::default();
        let mut page = vsdx::Page {
            id: 0,
            name: "Page-1".into(),
            ..vsdx::Page::default()
        };
        page.shapes.push(s);
        let svg = dc::svg::render_page(&doc, &page).expect("render");
        assert!(
            svg.contains(" C "),
            "the curved route renders as a cubic Bézier: {}",
            &svg[svg.len().saturating_sub(400)..]
        );
    }

    /// #177: the reveal radius shows the approached shape's points,
    /// the glue decision drives the green-square highlight, and glue
    /// being off hides everything.
    #[test]
    fn conn_point_fx_reveals_and_highlights() {
        let cands = vec![GlueCandidate {
            id: 7,
            rect: (2.0, 2.0, 1.0, 1.0),
            conn_rows: vec![(2, (2.0, 2.5)), (3, (3.0, 2.5))],
        }];
        let thr = 8.0 / 96.0;
        // Far away: nothing reveals.
        let fx = conn_point_fx(&cands, (6.0, 6.0), (6.0, 6.0), thr, 0.6, true, true);
        assert!(fx.reveal.is_empty(), "nothing near a far shape");
        assert!(fx.glue_target.is_none());
        // Approaching (inside the 0.6in halo, outside the glue
        // threshold): both points reveal, no highlight.
        let fx = conn_point_fx(&cands, (3.0, 3.55), (3.0, 3.55), thr, 0.6, true, true);
        assert_eq!(fx.reveal.len(), 2, "the approached shape's points reveal");
        assert!(fx.glue_target.is_none(), "near but not gluing");
        // At a connection point: that point becomes the highlight.
        let fx = conn_point_fx(&cands, (2.02, 2.5), (2.02, 2.5), thr, 0.6, true, true);
        assert_eq!(fx.glue_target, Some((2.0, 2.5)));
        // Glue off: Visio hides the affordances entirely.
        let fx = conn_point_fx(&cands, (2.02, 2.5), (2.02, 2.5), thr, 0.6, false, true);
        assert!(fx.reveal.is_empty() && fx.glue_target.is_none());
    }

    /// #177: a connector's glued sites — the static Connect sits on
    /// the target's connection point, the dynamic one on the
    /// endpoint's own position.
    #[test]
    fn glued_sites_static_row_and_dynamic_endpoint() {
        let mut target = Shape {
            id: 5,
            type_: Some("Shape".into()),
            ..Shape::default()
        };
        for (n, v) in [
            ("PinX", "3.0"),
            ("PinY", "3.0"),
            ("Width", "1.0"),
            ("Height", "1.0"),
            ("LocPinX", "0.5"),
            ("LocPinY", "0.5"),
        ] {
            target.sheet.set_cell(n, Cell::value(v));
        }
        let mut conn_section = Section {
            name: "Connection".into(),
            ix: 0,
            ..Section::default()
        };
        conn_section.rows.push(Row {
            ix: 2,
            cells: vec![
                ("X".to_string(), Cell::value("0.5")),
                ("Y".to_string(), Cell::value("1.0")),
            ],
            ..Row::default()
        });
        target
            .sheet
            .children
            .push(SheetChild::Section(conn_section));
        let mut conn = Shape {
            id: 6,
            type_: Some("Shape".into()),
            ..Shape::default()
        };
        for (n, v) in [
            ("BeginX", "1.0"),
            ("BeginY", "1.0"),
            ("EndX", "3.0"),
            ("EndY", "3.5"),
        ] {
            conn.sheet.set_cell(n, Cell::value(v));
        }
        let mut page = vsdx::model::Page {
            id: 0,
            name: "P1".into(),
            attrs: vec![("ID".into(), "0".into()), ("NameU".into(), "P1".into())],
            shapes: vec![target, conn],
            ..vsdx::model::Page::default()
        };
        use dc_core::connects::ConnectsHost;
        let mut list = page.connects();
        list.list.push(dc_core::Connect::new(
            6,
            "EndX",
            12,
            5,
            "Connections.X2",
            102,
        ));
        list.list
            .push(dc_core::Connect::new(6, "BeginX", 9, 5, "PinX", 3));
        page.set_connects(&list);
        let doc = vsdx::model::Document {
            pages: vec![page],
            ..vsdx::model::Document::default()
        };
        let cands = vec![GlueCandidate {
            id: 5,
            rect: (2.5, 2.5, 1.0, 1.0),
            conn_rows: vec![(2, (3.0, 3.5))],
        }];
        let sites = glued_sites(&doc, &doc.pages[0], 6, &cands);
        assert_eq!(sites.len(), 2, "one site per Connect record: {sites:?}");
        assert!(
            near(sites[0], (3.0, 3.5)),
            "static site on the connection point"
        );
        assert!(near(sites[1], (1.0, 1.0)), "dynamic site on the endpoint");
    }
}
