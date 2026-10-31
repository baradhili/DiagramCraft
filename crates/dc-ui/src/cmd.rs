//! GUI-authored editing commands (implementing [`dc_core::Command`]) —
//! the pieces a canvas needs beyond the dc-core basics. All values are
//! in Visio's internal inches; the mm/in distinction is display-only
//! (see `units.rs`).

use dc_core::commands::{remove_cell, SetCell};
use dc_core::Command;
use dc_core::Scope;
use vsdx::model::{Cell, Document, Shape};

/// Move a shape's pin to a new page position (inches).
pub struct MoveShape {
    shape: i64,
    to: (f64, f64),
    x: SetCell,
    y: SetCell,
}

impl MoveShape {
    pub fn new(page: i64, shape: i64, to_pin: (f64, f64)) -> MoveShape {
        MoveShape {
            shape,
            to: to_pin,
            x: SetCell::new(
                Scope::Shape { page, shape },
                "PinX",
                Cell::value(format!("{:.6}", to_pin.0)),
            ),
            y: SetCell::new(
                Scope::Shape { page, shape },
                "PinY",
                Cell::value(format!("{:.6}", to_pin.1)),
            ),
        }
    }
}

impl Command for MoveShape {
    fn touched(&self, doc: &Document) -> dc_core::TouchList {
        // Union of the two PinX/PinY SetCells → a pure Move touch
        // (#93): the layer texture stays valid under translation.
        let mut all = self.x.touched(doc);
        all.merge(self.y.touched(doc));
        all
    }

    fn label(&self) -> String {
        format!(
            "move shape {} to ({:.3}, {:.3})",
            self.shape, self.to.0, self.to.1
        )
    }
    fn apply(&mut self, doc: &mut Document) -> anyhow::Result<()> {
        self.x.apply(doc)?;
        self.y.apply(doc)?;
        Ok(())
    }
    fn undo(&mut self, doc: &mut Document) -> anyhow::Result<()> {
        self.y.undo(doc)?;
        self.x.undo(doc)?;
        Ok(())
    }
}

/// Refresh a flattened group's inherited cell caches after a resize
/// (GitHub #59). Visio instances carry `F='Inh'` snapshots of every
/// formula-driven cell — sub-shape XForms, the Txt* block, Connection
/// and Geometry row coordinates. Resizing only the top-level sheet
/// leaves those caches at the old size, and the renderer (which treats
/// caches as authoritative, like Visio at load) keeps composing the
/// drawing at it: the selection box grows around a stale shape.
/// Visio refreshes the caches when an edit changes what they evaluate
/// to; this emits the same refresh as SetCell/SetRowCell commands,
/// riding in the resize's Macro so one undo reverts everything.
///
/// The refresh re-derives each cache by **evaluating the master's
/// formula at the instance's new dims** (`vsdx::model::instance_env`,
/// the same environment the renderer's master mirroring uses) — which
/// sees through User-cell indirections like `TxtPinX =
/// GUARD(User.TxtPinX)` where `User.TxtPinX = Width/2` (the substring
/// heuristic that scaled caches before could not, and the icon/text
/// drifted as the shape grew). Refreshed cells keep `F='Inh'`, so a
/// later resize refreshes them again from the master's formula — no
/// compounding staleness. Ratio scaling (the shape's own size ratio
/// per axis) remains as the fallback when a formula cannot be
/// evaluated; fixed formulas (`GUARD(4MM)` icons) evaluate to their
/// constant and stay put. Cells with a real local formula (GUARD,
/// functions) are never touched — they re-evaluate, they don't cache.
pub fn group_cache_refresh(
    doc: &Document,
    page: i64,
    shape: i64,
    kx: f64,
    ky: f64,
) -> Vec<Box<dyn dc_core::Command>> {
    let Some(p) = doc.pages.iter().find(|p| p.id == page) else {
        return Vec::new();
    };
    let Some(root) = vsdx::model::find_shape_by_id(&p.shapes, shape) else {
        return Vec::new();
    };
    // Instance environments at the current (pre-resize) and new group
    // dims: the master root's layout formulas — including User.*
    // indirections — re-evaluate against them (instance User overrides
    // such as ShapeLayout win; cached F='Inh' User rows do not).
    let master_root = doc.instance_source(root, &[]);
    let dim = |n: &str| {
        doc.resolve_cell(root, &[], n)
            .and_then(|c| c.numeric())
            .filter(|v| v.is_finite() && *v > 1e-9)
    };
    let (old_w, old_h) = (dim("Width").unwrap_or(1.0), dim("Height").unwrap_or(1.0));
    let (env_old, env_new) = match &master_root {
        Some(m) => (
            Some(vsdx::model::instance_env(m, root, old_w, old_h)),
            Some(vsdx::model::instance_env(m, root, old_w * kx, old_h * ky)),
        ),
        None => (None, None),
    };
    let mut cmds = Vec::new();
    walk(
        root,
        root,
        page,
        true,
        doc,
        env_old.as_ref(),
        env_new.as_ref(),
        (kx, ky),
        &mut cmds,
    );
    cmds
}

/// One (cell name, axis) pair: the axis variable whose mention in the
/// master's formula marks the value as proportional (ratio fallback).
const X_CELLS: &[&str] = &[
    "PinX",
    "Width",
    "LocPinX",
    "TxtPinX",
    "TxtWidth",
    "TxtLocPinX",
];
const Y_CELLS: &[&str] = &[
    "PinY",
    "Height",
    "LocPinY",
    "TxtPinY",
    "TxtHeight",
    "TxtLocPinY",
];

/// Does the master's formula for `name` make the value proportional to
/// the axis? `None` = no master cell at all (nothing inherited).
fn scales_with(mcell: Option<&vsdx::model::Cell>, axis: &str) -> Option<bool> {
    let c = mcell?;
    Some(match c.f.as_deref() {
        Some(f) if f != "Inh" => f.contains(axis),
        // No formula on the master cell: XForm/Txt cells are literal
        // local values (size-independent); geometry row cells are
        // proportional by the renderer's convention (handled by the
        // caller passing `literal_proportional`).
        _ => false,
    })
}

/// The master cell's formula when it is a real (evaluable) one.
fn real_formula(cell: Option<&vsdx::model::Cell>) -> Option<&str> {
    cell.and_then(|c| c.f.as_deref())
        .filter(|f| *f != "Inh" && *f != "No Formula")
}

/// Evaluate a master-source cell's formula in `env`.
fn eval_in(cell: Option<&vsdx::model::Cell>, env: &vsdx::formula::Env) -> Option<f64> {
    let f = real_formula(cell)?;
    vsdx::formula::eval(f, env)
        .ok()
        .and_then(|v| v.as_f64())
        .filter(|v| v.is_finite())
}

/// A refreshed inherited cache: the new value under a preserved
/// `F='Inh'`, so the next resize refreshes it from the master's
/// formula again instead of meeting a frozen literal.
fn cached(v: f64) -> Cell {
    Cell {
        v: Some(format!("{v:.6}")),
        f: Some("Inh".into()),
        ..Cell::default()
    }
}

#[allow(clippy::too_many_arguments)]
fn walk(
    root: &Shape,
    sh: &Shape,
    page: i64,
    is_root: bool,
    doc: &Document,
    env_old: Option<&vsdx::formula::Env>,
    env_new: Option<&vsdx::formula::Env>,
    (kx, ky): (f64, f64),
    cmds: &mut Vec<Box<dyn dc_core::Command>>,
) {
    // The inheritance source for this shape's cells: the master's
    // corresponding shape (the root's master root, a sub's master sub).
    let chain: Vec<&Shape> = vec![root];
    let source = doc.instance_source(sh, &chain);

    // The environment this shape's source cells evaluate in, and the
    // per-axis size ratio for the fallback path. Root cells evaluate in
    // the root env (bare Width/Height are the group's own); sub cells
    // in a child env — qualified root entries only, seeded with the
    // sub's own effective size (a bare name in a child formula means
    // the child's own cell, `vsdx::model::qualified_env`).
    let mut cell_env: Option<vsdx::formula::Env> = None;
    let mut ratio = (kx, ky);
    if is_root {
        cell_env = env_new.cloned();
    } else if let (Some(src), Some(eo), Some(en)) = (source, env_old, env_new) {
        let qo = vsdx::model::qualified_env(eo);
        let mut qn = vsdx::model::qualified_env(en);
        // The sub's effective size at the old/new dims: its master
        // formulas re-evaluated (`GUARD(4MM)` stays fixed, a
        // `Sheet.N!Width*1` container stretches with the group).
        let eff = |name: &str, env: &vsdx::formula::Env, own: Option<f64>| {
            eval_in(src.sheet.cell(name), env)
                .filter(|v| *v > 1e-9)
                .or(own)
        };
        let own = |n: &str| sh.sheet.cell(n).and_then(|c| c.numeric());
        let w_old = eff("Width", &qo, own("Width"));
        let h_old = eff("Height", &qo, own("Height"));
        let w_new = eff("Width", &qn, w_old.map(|v| v * kx));
        let h_new = eff("Height", &qn, h_old.map(|v| v * ky));
        ratio = (
            match (w_old, w_new) {
                (Some(a), Some(b)) if a > 1e-9 => b / a,
                _ => kx,
            },
            match (h_old, h_new) {
                (Some(a), Some(b)) if a > 1e-9 => b / a,
                _ => ky,
            },
        );
        if let (Some(w), Some(h)) = (w_new, h_new) {
            qn.insert("Width".into(), w);
            qn.insert("Height".into(), h);
        }
        cell_env = Some(qn);
    }

    for (names, axis) in [(X_CELLS, "Width"), (Y_CELLS, "Height")] {
        for name in names {
            // The root's own XForm cells are written by the resize
            // itself; only its Txt* block is cache-refreshed.
            if is_root && !name.starts_with("Txt") {
                continue;
            }
            let Some(cell) = sh.sheet.cell(name) else {
                continue;
            };
            if cell.f.as_deref() != Some("Inh") {
                continue;
            }
            let Some(v) = cell.numeric() else { continue };
            let scope = Scope::Shape { page, shape: sh.id };
            // Authoritative path: re-evaluate the master's formula at
            // the instance's new dims.
            if let Some(env) = cell_env.as_ref() {
                if let Some(nv) = source.and_then(|src| eval_in(src.sheet.cell(name), env)) {
                    cmds.push(Box::new(SetCell::new(scope, *name, cached(nv))));
                    // Seed the derived text sizes so later TxtLocPin
                    // formulas (`TxtWidth*0.5`) evaluate too.
                    if is_root && matches!(*name, "TxtWidth" | "TxtHeight") {
                        if let Some(env) = cell_env.as_mut() {
                            env.insert((*name).to_string(), nv);
                        }
                    }
                    continue;
                }
            }
            // Fallback: ratio scaling, gated by the axis mention in the
            // master's formula (a fixed formula keeps its value).
            // LocPin follows the shape's own size: evaluation already
            // handled indirect size formulas here, so this gate only
            // keeps ratio scaling off a fixed-size shape's LocPin.
            if name.starts_with("LocPin") || name.starts_with("TxtLocPin") {
                let tsize = if name.starts_with("Txt") {
                    format!("Txt{axis}")
                } else {
                    axis.to_string()
                };
                let size_scales = source
                    .and_then(|src| scales_with(src.sheet.cell(&tsize), axis))
                    .unwrap_or(true);
                if !size_scales {
                    continue;
                }
            }
            let proportional = source
                .and_then(|src| scales_with(src.sheet.cell(name), axis))
                .unwrap_or(true);
            if !proportional {
                continue;
            }
            let k = if axis == "Width" { ratio.0 } else { ratio.1 };
            cmds.push(Box::new(SetCell::new(scope, *name, cached(v * k))));
        }
    }
    // Row coordinates (Geometry, Connection): X/A/C follow Width,
    // Y/B/D follow Height — by formula evaluation when the master row
    // has one, else by the shape's own size ratio (a master cell
    // without a formula is proportional by the renderer's convention).
    if let Some(source) = source {
        const ROW_X: &[&str] = &["X", "A", "C"];
        const ROW_Y: &[&str] = &["Y", "B", "D"];
        for (own_secs, src_secs) in [
            (
                sh.sheet.sections_named("Geometry"),
                source.sheet.sections_named("Geometry"),
            ),
            (
                sh.sheet.sections_named("Connection"),
                source.sheet.sections_named("Connection"),
            ),
        ] {
            for (si, sec) in own_secs.iter().enumerate() {
                let src_sec = src_secs.get(si);
                for row in &sec.rows {
                    use dc_core::commands::RowSel;
                    let sel = match &row.n {
                        Some(n) => RowSel::Named(n.clone()),
                        None => RowSel::Ix(row.ix),
                    };
                    let src_row = src_sec.and_then(|ss| {
                        ss.rows.iter().find(|r| match &sel {
                            RowSel::Ix(ix) => r.ix == *ix,
                            RowSel::Named(n) => r.n.as_deref() == Some(n.as_str()),
                        })
                    });
                    for (name, axis) in ROW_X
                        .iter()
                        .map(|n| (*n, "Width"))
                        .chain(ROW_Y.iter().map(|n| (*n, "Height")))
                    {
                        let Some(cell) = row.cell(name) else { continue };
                        if cell.f.as_deref() != Some("Inh") {
                            continue;
                        }
                        let Some(v) = cell.numeric() else { continue };
                        let scope = Scope::Shape { page, shape: sh.id };
                        if let Some(env) = cell_env.as_ref() {
                            if let Some(nv) = src_row.and_then(|r| eval_in(r.cell(name), env)) {
                                cmds.push(Box::new(dc_core::commands::SetRowCell::new(
                                    scope,
                                    &sec.name,
                                    sec.ix,
                                    sel.clone(),
                                    name,
                                    cached(nv),
                                )));
                                continue;
                            }
                        }
                        let proportional = src_row
                            .and_then(|r| real_formula(r.cell(name)))
                            .map(|f| f.contains(axis))
                            .unwrap_or(true);
                        if !proportional {
                            continue;
                        }
                        let k = if axis == "Width" { ratio.0 } else { ratio.1 };
                        cmds.push(Box::new(dc_core::commands::SetRowCell::new(
                            scope,
                            &sec.name,
                            sec.ix,
                            sel.clone(),
                            name,
                            cached(v * k),
                        )));
                    }
                }
            }
        }
    }
    for sub in &sh.sub_shapes {
        walk(
            root,
            sub,
            page,
            false,
            doc,
            env_old,
            env_new,
            (kx, ky),
            cmds,
        );
    }
}

/// Delete every cell of a shape's own sheet and the shape itself is NOT
/// handled here — see dc_core::commands::RemoveShape for removal. This
/// helper exists for tests: exact restoration of a moved shape.
#[allow(dead_code)]
pub(crate) fn strip_pin_cells(doc: &mut Document, page: i64, shape: i64) {
    let scope = Scope::Shape { page, shape };
    if let Ok(sheet) = dc_core::scope::sheet_mut(doc, &scope) {
        let _ = remove_cell(sheet, "PinX");
        let _ = remove_cell(sheet, "PinY");
    }
}

#[allow(dead_code)]
pub(crate) fn pin_of(doc: &Document, page: i64, shape: i64) -> Option<(f64, f64)> {
    let scope = Scope::Shape { page, shape };
    let sheet = dc_core::scope::sheet(doc, &scope).ok()?;
    let x = sheet.cell("PinX")?.numeric()?;
    let y = sheet.cell("PinY")?.numeric()?;
    Some((x, y))
}

// ---------------------------------------------------------------------------
// Connector rerouting (GitHub #108)
// ---------------------------------------------------------------------------

/// Deferred connector reroute: re-attach and re-route every connector
/// glued to the moved/resized shapes, evaluated against the document
/// state AT APPLY TIME (it rides at the tail of a move/resize Macro,
/// so the moved shapes' cells are already updated — building the
/// reroute eagerly would read stale, pre-move geometry).
///
/// Static glue follows the connection row; dynamic glue re-attaches on
/// the facing side; the geometry rows carry the new route
/// (`FixedRoute=1` connectors stay put). Cached values are rewritten,
/// the corpus formulas kept. Undo restores every touched cell.
pub struct RerouteConnectors {
    page: i64,
    moved: Vec<i64>,
    inner: Vec<Box<dyn dc_core::Command>>,
}

impl RerouteConnectors {
    pub fn new(page: i64, moved: Vec<i64>) -> RerouteConnectors {
        RerouteConnectors {
            page,
            moved,
            inner: Vec::new(),
        }
    }
}

impl Command for RerouteConnectors {
    fn touched(&self, _doc: &Document) -> dc_core::TouchList {
        let mut all = dc_core::TouchList::default();
        for c in &self.inner {
            all.merge(c.touched(_doc));
        }
        all
    }

    fn label(&self) -> String {
        format!("reroute connectors for {} moved shape(s)", self.moved.len())
    }

    fn apply(&mut self, doc: &mut Document) -> anyhow::Result<()> {
        // Pass 1 (immutable): plan every reroute against the current
        // (post-move) document.
        struct Plan {
            conn_id: i64,
            cells: Vec<(String, f64, Option<String>)>,
            rows: Vec<(i64, f64, f64)>,
        }
        // Both ends may glue to moved shapes (a two-shape selection
        // drag): each connector is planned ONCE from both new
        // endpoints — per-Connect plans would race on stale opposite
        // ends (CodeRabbit).
        let mut plans: Vec<Plan> = Vec::new();
        {
            use dc_core::connects::ConnectsHost;
            use std::collections::BTreeMap;
            let Some(page) = doc.pages.iter().find(|p| p.id == self.page) else {
                return Ok(());
            };
            let connects = page.connects();
            let mut by_connector: BTreeMap<i64, Vec<&dc_core::connects::Connect>> =
                BTreeMap::new();
            for c in &connects.list {
                if !self.moved.contains(&c.to_sheet) {
                    continue;
                }
                by_connector.entry(c.from_sheet).or_default().push(c);
            }
            let trect = |t: &vsdx::model::Shape| -> Option<(f64, f64, f64, f64)> {
                let ch = dc_core::scope::shape_chain(doc, self.page, t.id).ok()?;
                let g = |n: &str| {
                    doc.resolve_cell(t, &ch, n)
                        .and_then(|v| v.numeric())
                };
                let (px, py) = (g("PinX")?, g("PinY")?);
                let (w, h) = (g("Width").unwrap_or(1.0), g("Height").unwrap_or(1.0));
                let (lx, ly) = (
                    g("LocPinX").unwrap_or(w / 2.0),
                    g("LocPinY").unwrap_or(h / 2.0),
                );
                let (x, y) = (px - lx, py - ly);
                Some((x.min(x + w), y.min(y + h), w.abs(), h.abs()))
            };
            for (conn_id, recs) in by_connector {
                let Some(conn) = vsdx::model::find_shape_by_id(&page.shapes, conn_id)
                else {
                    continue;
                };
                let Ok(chain) = dc_core::scope::shape_chain(doc, self.page, conn.id)
                else {
                    continue;
                };
                // #174: chain-aware — master-based connectors (every
                // Visio-authored one) carry ObjType=2 on the MASTER;
                // the local-only check skipped them, so moving a shape
                // never moved its glued connectors.
                if !crate::connectors::is_connector_in(doc, &chain, conn) {
                    continue;
                }
                if conn
                    .sheet
                    .cell("FixedRoute")
                    .and_then(|v| v.numeric())
                    .is_some_and(|v| v == 1.0)
                {
                    continue;
                }
                let get = |n: &str| {
                    doc.resolve_cell(conn, &chain, n)
                        .and_then(|v| v.numeric())
                };
                let (Some(bx), Some(by), Some(ex), Some(ey)) =
                    (get("BeginX"), get("BeginY"), get("EndX"), get("EndY"))
                else {
                    continue;
                };
                // Resolve every moved end's attachment against the
                // post-move shapes BEFORE any write.
                let mut beg = (bx, by);
                let mut endp = (ex, ey);
                let mut end_cells: Vec<(String, f64, Option<String>)> = Vec::new();
                for c in recs {
                    let end = if c.from_cell == "BeginX" {
                        crate::connectors::End::Begin
                    } else {
                        crate::connectors::End::End
                    };
                    let Some(target) = vsdx::model::find_shape_by_id(&page.shapes, c.to_sheet)
                    else {
                        continue;
                    };
                    let other = match end {
                        crate::connectors::End::Begin => endp,
                        crate::connectors::End::End => beg,
                    };
                    let attach = if c.to_part >= 100 {
                        let row_ix = c.to_part - 100;
                        trect(target).and_then(|(rx, ry, _, _)| {
                            let find = |sheet: &vsdx::model::Sheet| -> Option<(f64, f64)> {
                                for sec in sheet.sections_named("Connection") {
                                    if let Some(row) =
                                        sec.rows.iter().find(|r| r.ix == row_ix)
                                    {
                                        if let (Some(x), Some(y)) = (
                                            row.cell("X").and_then(|v| v.numeric()),
                                            row.cell("Y").and_then(|v| v.numeric()),
                                        ) {
                                            return Some((rx + x, ry + y));
                                        }
                                    }
                                }
                                None
                            };
                            find(&target.sheet).or_else(|| {
                                target
                                    .master
                                    .and_then(|mid| doc.master(mid))
                                    .and_then(|m| m.shapes.first())
                                    .and_then(|root| find(&root.sheet))
                            })
                        })
                    } else {
                        trect(target)
                            .map(|r| crate::connectors::attach_point(r, other))
                    };
                    let Some(attach) = attach else { continue };
                    match end {
                        crate::connectors::End::Begin => beg = attach,
                        crate::connectors::End::End => endp = attach,
                    }
                    let endf = conn.sheet.cell(end.cell()).and_then(|v| v.f.clone());
                    end_cells.push((end.cell().to_string(), attach.0, endf.clone()));
                    end_cells.push((end.y_cell().to_string(), attach.1, endf));
                }
                if end_cells.is_empty() {
                    continue;
                }
                // Frame + route once, from both new endpoints.
                let style = get("ShapeRouteStyle")
                    .map(|v| format!("{}", v as i64))
                    .unwrap_or_else(|| crate::connectors::ROUTE_RIGHT_ANGLE.into());
                let pts = crate::connectors::route(&style, beg, endp);
                let rows = crate::connectors::route_rows(beg, &pts);
                let mut cells = end_cells;
                // #175: a derived cell whose effective formula
                // REFERENCES the endpoints/sizes must have its cached V
                // refreshed — the renderer reads the cache, and skipping
                // it (the #174 rule) left the XForm of the OLD geometry
                // under moved endpoints. The write goes through
                // SetGluedCell (GUARD-waived, formula kept); a
                // formula-free cell stays a plain SetCell; a CONSTANT
                // guard (Height=GUARD(0.19685DL), Angle=GUARD(0DA))
                // does not depend on the endpoints and is skipped.
                let eff = |n: &str| {
                    dc_core::commands::effective_formula(doc, conn, &chain, n)
                };
                let formula_tracks = |n: &str| -> Option<String> {
                    let f = eff(n)?;
                    let u = f.to_uppercase();
                    ["BEGINX", "ENDX", "BEGINY", "ENDY", "WIDTH", "HEIGHT"]
                        .iter()
                        .any(|k| u.contains(k))
                        .then_some(f)
                };
                let (w_new, h_new) = (endp.0 - beg.0, endp.1 - beg.1);
                // LocPin derives from the EFFECTIVE size: when Height is
                // a constant guard the band keeps its height, so LocPinY
                // stays half the kept height.
                let h_eff = if formula_tracks("Height").is_some() {
                    h_new
                } else {
                    get("Height").unwrap_or(h_new)
                };
                let w_eff = if formula_tracks("Width").is_some() {
                    w_new
                } else {
                    get("Width").unwrap_or(w_new)
                };
                for (n, v) in [
                    ("PinX", (beg.0 + endp.0) / 2.0),
                    ("PinY", (beg.1 + endp.1) / 2.0),
                    ("Width", w_new),
                    ("Height", h_new),
                    ("LocPinX", w_eff / 2.0),
                    ("LocPinY", h_eff / 2.0),
                ] {
                    match formula_tracks(n) {
                        None if eff(n).is_none() => {
                            cells.push((n.to_string(), v, None));
                        }
                        Some(f) => {
                            cells.push((n.to_string(), v, Some(f)));
                        }
                        // Constant guard or otherwise static: skip.
                        None => {}
                    }
                }
                // #174/#175: only rewrite route rows the connector
                // ALREADY has — SetRowCell materializes missing rows,
                // which would grow a 2-row arc master (MoveTo +
                // EllipticalArcTo) into a 4-row elbow polyline.
                let existing: Vec<i64> = {
                    let mut v: Vec<i64> = conn
                        .sheet
                        .sections_named("Geometry")
                        .first()
                        .map(|g| g.rows.iter().map(|r| r.ix).collect::<Vec<_>>())
                        .unwrap_or_default();
                    v.sort_unstable();
                    v
                };
                let planned_rows: Vec<(i64, f64, f64)> = if conn.master.is_some() {
                    // #175: a MASTER-based connector's rows are its own
                    // frame (a fixed-height band's LineTo(Width, H/2)),
                    // NOT the synthetic begin-corner frame — writing
                    // (endp−beg) deltas there broke the render (#175).
                    // Translate the EXISTING literal row coordinates by
                    // the endpoint's page delta, per axis, only where
                    // that axis's size tracks the endpoints. Rows whose
                    // cells carry formulas evaluate against the
                    // refreshed caches in the renderer and are left to
                    // it.
                    let dx = if formula_tracks("Width").is_some() {
                        endp.0 - ex
                    } else {
                        0.0
                    };
                    let dy = if formula_tracks("Height").is_some() {
                        endp.1 - ey
                    } else {
                        0.0
                    };
                    let literal = |c: Option<&vsdx::Cell>| -> Option<f64> {
                        let c = c?;
                        let literal = c
                            .f
                            .as_deref()
                            .is_none_or(|f| f == "Inh" || f == "No Formula");
                        literal.then(|| c.numeric())?
                    };
                    conn.sheet
                        .sections_named("Geometry")
                        .first()
                        .map(|g| {
                            g.rows
                                .iter()
                                .filter(|r| (2..=4).contains(&r.ix))
                                .filter_map(|r| {
                                    let x = literal(r.cell("X"))?;
                                    let y = literal(r.cell("Y"))?;
                                    Some((r.ix, x + dx, y + dy))
                                })
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default()
                } else {
                    // The synthetic connector's own frame: the planned
                    // route, filtered to rows it has; a connector with
                    // fewer rows than the route frame puts the ENDPOINT
                    // on its LAST row.
                    let mut planned: Vec<(i64, f64, f64)> = rows
                        .iter()
                        .enumerate()
                        .map(|(i, (x, y))| ((i + 2) as i64, *x, *y))
                        .filter(|(ix, _, _)| existing.contains(ix))
                        .collect();
                    let max_ix =
                        existing.iter().copied().filter(|ix| (2..=4).contains(ix)).max();
                    if let (Some(last), Some(max_ix)) = (planned.last_mut(), max_ix) {
                        if max_ix < 4 {
                            *last = (max_ix, endp.0 - beg.0, endp.1 - beg.1);
                        }
                    }
                    planned
                };
                plans.push(Plan {
                    conn_id: conn.id,
                    cells,
                    rows: planned_rows,
                });
            }
        }
        // Pass 2 (mutable): apply the planned commands, capturing each
        // for undo.
        for plan in plans {
            let scope = dc_core::Scope::Shape {
                page: self.page,
                shape: plan.conn_id,
            };
            for (n, v, f) in plan.cells {
                // Formula-carrying writes are the glue engine
                // updating its own GUARDed endpoint caches —
                // SetGluedCell (plain derived cells stay SetCell).
                let mut c = if f.is_some() {
                    Box::new(dc_core::SetGluedCell::new(
                        scope.clone(),
                        n,
                        Cell {
                            v: Some(format!("{v:.6}")),
                            f,
                            ..Cell::default()
                        },
                    )) as Box<dyn Command>
                } else {
                    Box::new(SetCell::new(
                        scope.clone(),
                        n,
                        Cell {
                            v: Some(format!("{v:.6}")),
                            f,
                            ..Cell::default()
                        },
                    )) as Box<dyn Command>
                };
                c.apply(doc)?;
                self.inner.push(c);
            }
            for (ix, x, y) in plan.rows {
                for (cell, v) in [("X", x), ("Y", y)] {
                    let mut c = dc_core::SetRowCell::new(
                        scope.clone(),
                        "Geometry",
                        0,
                        dc_core::commands::RowSel::Ix(ix),
                        cell,
                        Cell::value(format!("{v:.6}")),
                    );
                    c.apply(doc)?;
                    self.inner.push(Box::new(c));
                }
            }
        }
        Ok(())
    }

    fn undo(&mut self, doc: &mut Document) -> anyhow::Result<()> {
        for c in self.inner.iter_mut().rev() {
            c.undo(doc)?;
        }
        self.inner.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dc_core::Editor;

    fn doc_with_shape() -> Editor {
        let mut shape = Shape {
            id: 1,
            type_: Some("Shape".into()),
            ..Shape::default()
        };
        shape.sheet.set_cell("PinX", Cell::value("1.5"));
        shape.sheet.set_cell("PinY", Cell::value("2.5"));
        let page = vsdx::model::Page {
            id: 0,
            name: "P1".into(),
            attrs: vec![("ID".into(), "0".into()), ("NameU".into(), "P1".into())],
            shapes: vec![shape],
            ..vsdx::model::Page::default()
        };
        Editor::new(Document {
            pages: vec![page],
            ..Document::default()
        })
    }

    #[test]
    fn move_shape_undo_restores_exact_cells() {
        let mut ed = doc_with_shape();
        ed.apply(MoveShape::new(0, 1, (4.0, 5.0))).unwrap();
        assert_eq!(pin_of(&ed.doc, 0, 1), Some((4.0, 5.0)));
        ed.undo().unwrap();
        // exact original strings, not reformatted numbers
        let scope = Scope::Shape { page: 0, shape: 1 };
        let sheet = dc_core::scope::sheet(&ed.doc, &scope).unwrap();
        assert_eq!(sheet.cell("PinX").unwrap().v.as_deref(), Some("1.5"));
        assert_eq!(sheet.cell("PinY").unwrap().v.as_deref(), Some("2.5"));
    }

    #[test]
    fn add_master_instance_undo_removes() {
        // AddMasterInstance now lives in dc-core (#84); keep the
        // behavioral pin here through the re-exported command.
        let mut ed = doc_with_shape();
        ed.doc.masters.push(vsdx::Master {
            id: 7,
            name_u: Some("M".into()),
            ..vsdx::Master::default()
        });
        ed.apply(dc_core::AddMasterInstance::new(0, 7, (3.0, 3.0)))
            .unwrap();
        assert!(vsdx::model::find_shape_by_id(&ed.doc.pages[0].shapes, 2).is_some());
        let s = vsdx::model::find_shape_by_id(&ed.doc.pages[0].shapes, 2).unwrap();
        assert_eq!(s.master, Some(7));
        assert_eq!(pin_of(&ed.doc, 0, 2), Some((3.0, 3.0)));
        ed.undo().unwrap();
        assert!(vsdx::model::find_shape_by_id(&ed.doc.pages[0].shapes, 2).is_none());
        // ID is reallocated identically on redo
        ed.redo().unwrap();
        assert!(vsdx::model::find_shape_by_id(&ed.doc.pages[0].shapes, 2).is_some());
    }

    /// #97 §4 relocation: the group-cache-refresh semantics (formerly a
    /// 340-line kittest) tested at the command layer — the app's resize
    /// commit is SetCells for the new dims + group_cache_refresh in one
    /// Macro, and that Macro is applied directly here.
    #[test]
    fn group_cache_refresh_reevaluates_inh_caches_through_user_indirections() {
        use crate::group_fixture::taskg_editor;
        let (mut ed, page, [root, sub, icon]) = taskg_editor();
        let num = |ed: &dc_core::Editor, id: i64, k: &str| {
            vsdx::model::find_shape_by_id(&ed.doc.pages.iter().find(|p| p.id == page).unwrap().shapes, id)
                .unwrap()
                .sheet
                .cell(k)
                .and_then(|c| c.numeric())
                .unwrap_or(f64::NAN)
        };

        // Resize 1×0.5 → 1.3×0.7 the way the app commits it.
        let (kx, ky) = (1.3f64, 1.4f64);
        let mut cmds: Vec<Box<dyn dc_core::Command>> = vec![
            Box::new(SetCell::new(
                Scope::Shape { page, shape: root },
                "Width",
                Cell::value("1.3"),
            )),
            Box::new(SetCell::new(
                Scope::Shape { page, shape: root },
                "Height",
                Cell::value("0.7"),
            )),
        ];
        cmds.extend(group_cache_refresh(&ed.doc, page, root, kx, ky));
        ed.apply(dc_core::Macro::new("resize shape", cmds)).unwrap();
        let (w1, h1) = (1.3f64, 0.7f64);

        // Sub caches scaled with the group.
        assert!((num(&ed, sub, "Width") - kx).abs() < 1e-3, "sub Width: {}", num(&ed, sub, "Width"));
        assert!((num(&ed, sub, "PinX") - 0.5 * kx).abs() < 1e-3, "sub PinX: {}", num(&ed, sub, "PinX"));
        // Sub geometry row caches scaled too.
        let s = vsdx::model::find_shape_by_id(
            &ed.doc.pages.iter().find(|p| p.id == page).unwrap().shapes, sub).unwrap();
        let geo = &s.sheet.sections_named("Geometry")[0];
        let far_x = geo.rows.iter()
            .find(|r| r.ix == 3).unwrap()
            .cell("X").and_then(|c| c.numeric()).unwrap();
        assert!((far_x - 1.0 * kx).abs() < 1e-3, "sub geometry X scaled: {far_x}");

        // The Txt block followed, including the cells the User
        // indirection hides: TxtPinX = GUARD(User.TxtPinX) with
        // User.TxtPinX = Width/2 must re-center.
        assert!((num(&ed, root, "TxtWidth") - w1).abs() < 1e-3, "TxtWidth: {}", num(&ed, root, "TxtWidth"));
        assert!((num(&ed, root, "TxtPinX") - w1 / 2.0).abs() < 1e-3, "TxtPinX: {}", num(&ed, root, "TxtPinX"));
        assert!((num(&ed, root, "TxtPinY") - h1 / 2.0).abs() < 1e-3, "TxtPinY: {}", num(&ed, root, "TxtPinY"));
        assert!((num(&ed, root, "TxtLocPinX") - w1 / 2.0).abs() < 1e-3, "TxtLocPinX: {}", num(&ed, root, "TxtLocPinX"));
        // Refreshed cells keep F='Inh' — a later resize must refresh
        // them again, not meet frozen literals.
        let s = vsdx::model::find_shape_by_id(
            &ed.doc.pages.iter().find(|p| p.id == page).unwrap().shapes, root).unwrap();
        assert_eq!(s.sheet.cell("TxtPinX").unwrap().f.as_deref(), Some("Inh"));

        // The fixed-size icon kept its GUARD'ed size and X offset, while
        // PinY = GUARD(Sheet.1!User.IconY) with
        // User.IconY = Height - User.InfoPanelHeight/2 re-evaluated at
        // the new height (the instance's cached User.IconY must not
        // freeze it).
        assert!((num(&ed, icon, "Width") - 0.157480).abs() < 1e-6, "icon Width: {}", num(&ed, icon, "Width"));
        assert!((num(&ed, icon, "PinX") - 0.1).abs() < 1e-6, "icon PinX: {}", num(&ed, icon, "PinX"));
        assert!((num(&ed, icon, "PinY") - (h1 - 0.1)).abs() < 1e-3, "icon PinY: {}", num(&ed, icon, "PinY"));

        // A SECOND resize refreshes again (the Inh markers survived).
        let mut cmds: Vec<Box<dyn dc_core::Command>> = vec![
            Box::new(SetCell::new(
                Scope::Shape { page, shape: root },
                "Width",
                Cell::value("1.56"),
            )),
        ];
        cmds.extend(group_cache_refresh(&ed.doc, page, root, 1.56 / w1, 1.0));
        ed.apply(dc_core::Macro::new("resize shape", cmds)).unwrap();
        assert!((num(&ed, root, "TxtPinX") - 0.78).abs() < 1e-3, "second refresh: {}", num(&ed, root, "TxtPinX"));

        // Each undo restores one resize's whole refresh; two undos
        // return to the authored caches.
        ed.undo().unwrap();
        assert!((num(&ed, root, "TxtPinX") - w1 / 2.0).abs() < 1e-3, "undo #1");
        ed.undo().unwrap();
        assert!((num(&ed, sub, "Width") - 1.0).abs() < 1e-6, "undo restored sub cache");
        assert!((num(&ed, root, "TxtPinX") - 0.5).abs() < 1e-6, "undo restored TxtPinX");
        assert!((num(&ed, icon, "PinY") - 0.4).abs() < 1e-6, "undo restored icon PinY");
    }
}

#[cfg(test)]
mod reroute_master_tests {
    use super::*;

    // ---- #174: reroute must move MASTER-based connectors ----------------

    use dc_core::connects::ConnectsHost;
    use vsdx::model::{Cell as VCell, Row, Section, SheetChild};

    fn vc(n: &str, v: &str) -> (String, VCell) {
        (n.to_string(), VCell::value(v.to_string()))
    }

    /// A document like the owner's connector-test.vsdx: a rectangle,
    /// a MASTER-based Dynamic connector (ObjType=2 lives on the
    /// master root; the instance carries only placement cells + local
    /// geometry rows), and a Connect gluing the connector's EndX to
    /// the rectangle (static, ToPart 100+row on Connection row 0).
    fn doc_with_master_connector(to_part: i64, arc: bool) -> (dc_core::Editor, i64) {
        let mut ed = dc_core::Editor::new_blank();
        let page_id = ed.doc.pages[0].id;
        // The target rectangle at (2..4, 5..6) with connection row 0
        // at its top-left (2, 6).
        let mut target = Shape {
            id: 10,
            type_: Some("Shape".into()),
            ..Shape::default()
        };
        for (n, v) in [
            ("PinX", "3.0"),
            ("PinY", "5.5"),
            ("Width", "2.0"),
            ("Height", "1.0"),
            ("LocPinX", "1.0"),
            ("LocPinY", "0.5"),
        ] {
            target.sheet.set_cell(n, VCell::value(v));
        }
        // Connection row 0 at the rect's left edge mid-height (local
        // (0, 0.5) → page (2, 5.5)): the static-glue anchor.
        let mut connections = Section {
            name: "Connection".into(),
            ix: 0,
            ..Section::default()
        };
        connections.rows.push(Row {
            ix: 0,
            cells: vec![vc("X", "0"), vc("Y", "0.5")],
            ..Row::default()
        });
        target.sheet.children.push(SheetChild::Section(connections));
        let mut conn_rows = vec![
            Row {
                t: Some("MoveTo".into()),
                ix: 1,
                cells: vec![vc("X", "0"), vc("Y", "0")],
                ..Row::default()
            },
        ];
        if arc {
            conn_rows.push(Row {
                t: Some("EllipticalArcTo".into()),
                ix: 2,
                cells: vec![
                    vc("X", "1.0"),
                    vc("Y", "0.1"),
                    vc("A", "0.5"),
                    vc("B", "0.35"),
                ],
                ..Row::default()
            });
        } else {
            conn_rows.push(Row {
                t: Some("LineTo".into()),
                ix: 2,
                cells: vec![vc("X", "1.0"), vc("Y", "0.0")],
                ..Row::default()
            });
            conn_rows.push(Row {
                t: Some("LineTo".into()),
                ix: 3,
                cells: vec![vc("X", "1.0"), vc("Y", "0.0")],
                ..Row::default()
            });
            conn_rows.push(Row {
                t: Some("LineTo".into()),
                ix: 4,
                cells: vec![vc("X", "1.0"), vc("Y", "0.0")],
                ..Row::default()
            });
        }
        // The connector: master carries ObjType=2 + GUARDed derived
        // cells (the corpus pattern the old checks missed).
        let mut root = Shape {
            id: 1,
            type_: Some("Shape".into()),
            ..Shape::default()
        };
        root.sheet.set_cell("ObjType", VCell::value("2"));
        // The corpus dynamic-connector master pattern (#175): every
        // XForm cell derives from the endpoints through GUARDs.
        for (n, f) in [
            ("PinX", "GUARD((BeginX+EndX)/2)"),
            ("PinY", "GUARD((BeginY+EndY)/2)"),
            ("Width", "GUARD(EndX-BeginX)"),
            ("Height", "GUARD(EndY-BeginY)"),
            ("LocPinX", "GUARD(Width*0.5)"),
            ("LocPinY", "GUARD(Height*0.5)"),
        ] {
            root.sheet.set_cell(
                n,
                VCell {
                    f: Some(f.into()),
                    ..VCell::default()
                },
            );
        }
        let mut m = vsdx::Master::default();
        m.shapes.push(root);
        ed.doc.masters.push(m);
        let mut conn = Shape {
            id: 11,
            type_: Some("Shape".into()),
            master: Some(0),
            ..Shape::default()
        };
        for (n, v) in [
            ("BeginX", "1.0"),
            ("BeginY", "6.0"),
            ("EndX", "2.0"),
            ("EndY", "6.0"),
            ("PinX", "1.5"),
            ("PinY", "6.0"),
            ("Width", "1.0"),
            ("LocPinX", "0.5"),
            ("LocPinY", "0.0"),
        ] {
            conn.sheet.set_cell(n, VCell::value(v));
        }
        // A fixed-height band master (the #175 file's convention): the
        // instance overrides Height with a CONSTANT guard.
        conn.sheet.set_cell(
            "Height",
            VCell {
                v: Some("0.2".into()),
                f: Some("GUARD(0.2DL)".into()),
                ..VCell::default()
            },
        );
        let mut geo = Section {
            name: "Geometry".into(),
            ix: 0,
            ..Section::default()
        };
        geo.rows = conn_rows;
        conn.sheet.children.push(SheetChild::Section(geo));
        ed.doc.pages[0].shapes.push(target);
        ed.doc.pages[0].shapes.push(conn);
        let mut list = ed.doc.pages[0].connects();
        list.list.push(dc_core::connects::Connect::new(
            11,
            "EndX",
            12,
            10,
            "Connections.X1",
            to_part,
        ));
        ed.doc.pages[0].set_connects(&list);
        (ed, page_id)
    }

    /// #174: a master-based connector's glued endpoint follows its
    /// target after RerouteConnectors — the local-only ObjType check
    /// used to skip it entirely.
    #[test]
    fn reroute_moves_master_based_connector_static_glue() {
        let (mut ed, page_id) = doc_with_master_connector(100, false);
        // Move the target right by 1in (PinX 3.0 → 4.0).
        for sh in ed.doc.pages[0].shapes.iter_mut() {
            if sh.id == 10 {
                sh.sheet.set_cell("PinX", VCell::value("4.0"));
            }
        }
        let mut cmd = RerouteConnectors::new(page_id, vec![10]);
        cmd.apply(&mut ed.doc).unwrap();
        let conn = ed
            .doc
            .pages[0]
            .shapes
            .iter()
            .find(|s| s.id == 11)
            .unwrap();
        let ex = conn.sheet.cell("EndX").and_then(|c| c.numeric()).unwrap();
        assert!(
            (ex - 3.0).abs() < 1e-6,
            "static glue follows the connection point: EndX {ex} (want 3.0)"
        );
        cmd.undo(&mut ed.doc).unwrap();
        let ex = ed
            .doc
            .pages[0]
            .shapes
            .iter()
            .find(|s| s.id == 11)
            .unwrap()
            .sheet
            .cell("EndX")
            .and_then(|c| c.numeric())
            .unwrap();
        assert!((ex - 2.0).abs() < 1e-6, "undo restores EndX to 2.0: {ex}");
    }

    /// Dynamic glue (ToPart 3): the endpoint walks to the target's
    /// facing edge after the move.
    #[test]
    fn reroute_moves_master_based_connector_dynamic_glue() {
        let (mut ed, page_id) = doc_with_master_connector(3, false);
        for sh in ed.doc.pages[0].shapes.iter_mut() {
            if sh.id == 10 {
                sh.sheet.set_cell("PinX", VCell::value("4.0"));
            }
        }
        let mut cmd = RerouteConnectors::new(page_id, vec![10]);
        cmd.apply(&mut ed.doc).unwrap();
        let conn = ed
            .doc
            .pages[0]
            .shapes
            .iter()
            .find(|s| s.id == 11)
            .unwrap();
        let ex = conn.sheet.cell("EndX").and_then(|c| c.numeric()).unwrap();
        assert!(
            (ex - 3.0).abs() < 1e-6,
            "dynamic glue walks to the facing (left) edge: EndX {ex} (want 3.0)"
        );
    }

    /// An ARC connector keeps its 2-row geometry — the reroute must
    /// not materialize the tool connector's 4-row route frame onto a
    /// master whose rows are MoveTo + EllipticalArcTo.
    #[test]
    fn reroute_never_grows_an_arc_connectors_rows() {
        let (mut ed, page_id) = doc_with_master_connector(100, true);
        for sh in ed.doc.pages[0].shapes.iter_mut() {
            if sh.id == 10 {
                sh.sheet.set_cell("PinX", VCell::value("4.0"));
            }
        }
        let mut cmd = RerouteConnectors::new(page_id, vec![10]);
        cmd.apply(&mut ed.doc).unwrap();
        let conn = ed
            .doc
            .pages[0]
            .shapes
            .iter()
            .find(|s| s.id == 11)
            .unwrap();
        let geo = conn.sheet.sections_named("Geometry").remove(0);
        assert_eq!(
            geo.rows.len(),
            2,
            "the arc keeps its MoveTo + EllipticalArcTo rows: {:?}",
            geo.rows.iter().map(|r| r.t.clone()).collect::<Vec<_>>()
        );
        assert_eq!(geo.rows[1].t.as_deref(), Some("EllipticalArcTo"));
        // #175: its endpoint row TRANSLATED IN ITS OWN FRAME — X by
        // the endpoint's x delta, Y unchanged (the constant-height
        // band does not track dy).
        let x = geo.rows[1].cell("X").and_then(|c| c.numeric()).unwrap();
        assert!(
            (x - 2.0).abs() < 1e-3,
            "the arc's end row tracks the new endpoint: X {x}"
        );
        let y = geo.rows[1].cell("Y").and_then(|c| c.numeric()).unwrap();
        assert!((y - 0.1).abs() < 1e-6, "the band's row Y is kept: {y}");
        // The guarded caches refreshed through their guards; the
        // constant band guard untouched.
        let (pinx_v, pinx_f) = {
            let c = conn.sheet.cell("PinX").unwrap();
            (c.v.clone(), c.f.clone())
        };
        assert!(
            pinx_v.as_deref().is_some_and(|v| (v.parse::<f64>().unwrap() - 2.0).abs() < 1e-6),
            "PinX cache = (B+E)/2 = 2.0: {pinx_v:?}"
        );
        assert!(
            pinx_f.as_deref().is_some_and(|f| f.contains("GUARD")),
            "the guard stays on the refreshed cell: {pinx_f:?}"
        );
        let h = conn.sheet.cell("Height").unwrap();
        assert!(
            h.numeric().unwrap().abs() - 0.2 < 1e-9,
            "the constant band height is untouched: {:?}",
            h.v
        );
    }
}
