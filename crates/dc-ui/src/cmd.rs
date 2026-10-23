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

/// Place a new instance of a master onto a page at a pin position
/// (inches). Geometry/format inherit from the master, exactly like a
/// Visio drag-from-stencil drop.
pub struct AddMasterInstance {
    pub page: i64,
    pub master: i64,
    pub pin: (f64, f64),
    id: Option<i64>,
}

impl AddMasterInstance {
    pub fn new(page: i64, master: i64, pin: (f64, f64)) -> AddMasterInstance {
        AddMasterInstance {
            page,
            master,
            pin,
            id: None,
        }
    }
}

impl Command for AddMasterInstance {
    fn label(&self) -> String {
        format!(
            "add master {} instance at ({:.3}, {:.3})",
            self.master, self.pin.0, self.pin.1
        )
    }
    fn apply(&mut self, doc: &mut Document) -> anyhow::Result<()> {
        let id = dc_core::commands::next_shape_id(doc, self.page);
        self.id = Some(id);
        let mut sheet = vsdx::model::Sheet::default();
        sheet.set_cell("PinX", Cell::value(format!("{:.6}", self.pin.0)));
        sheet.set_cell("PinY", Cell::value(format!("{:.6}", self.pin.1)));
        // A 1D master (line/connector: root shape carries Begin/End
        // cells) gets XForm1D snapshots on the instance — a horizontal
        // 1-inch line centered on the drop point — plus the derived
        // XForm literals the renderer reads, exactly like Visio's
        // instancing caches (GitHub #59).
        let one_d = doc.master(self.master).is_some_and(|m| {
            m.shapes
                .iter()
                .any(|s| s.sheet.cell("BeginX").is_some() && s.sheet.cell("EndX").is_some())
        });
        if one_d {
            let (bx, ex) = (self.pin.0 - 0.5, self.pin.0 + 0.5);
            let by = self.pin.1;
            for (n, v) in [
                ("BeginX", bx),
                ("BeginY", by),
                ("EndX", ex),
                ("EndY", by),
                ("Width", 1.0),
                ("Height", 0.0),
                ("LocPinX", 0.5),
                ("LocPinY", 0.0),
                ("Angle", 0.0),
            ] {
                sheet.set_cell(n, Cell::value(format!("{v:.6}")));
            }
        }
        let shape = Shape {
            id,
            type_: Some("Shape".to_string()),
            master: Some(self.master),
            line_style: Some(0),
            fill_style: Some(0),
            text_style: Some(0),
            attrs: vec![
                ("ID".to_string(), id.to_string()),
                ("Type".to_string(), "Shape".to_string()),
                ("Master".to_string(), self.master.to_string()),
                ("LineStyle".to_string(), "0".to_string()),
                ("FillStyle".to_string(), "0".to_string()),
                ("TextStyle".to_string(), "0".to_string()),
            ],
            sheet,
            ..Shape::default()
        };
        let page = doc
            .pages
            .iter_mut()
            .find(|p| p.id == self.page)
            .ok_or_else(|| anyhow::anyhow!("page {} not found", self.page))?;
        page.shapes.push(shape);
        Ok(())
    }
    fn undo(&mut self, doc: &mut Document) -> anyhow::Result<()> {
        let Some(id) = self.id else { return Ok(()) };
        let page = doc
            .pages
            .iter_mut()
            .find(|p| p.id == self.page)
            .ok_or_else(|| anyhow::anyhow!("page {} not found", self.page))?;
        if let Some(pos) = page.shapes.iter().position(|s| s.id == id) {
            page.shapes.remove(pos);
        }
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
            // LocPin follows the shape's own size: scale it only when
            // the size cell itself is proportional.
            if name.starts_with("LocPin") || name.starts_with("TxtLocPin") {
                let size = if axis == "Width" { "Width" } else { "Height" };
                let tsize = if name.starts_with("Txt") {
                    format!("Txt{size}")
                } else {
                    size.to_string()
                };
                let size_scales = source
                    .and_then(|src| scales_with(src.sheet.cell(&tsize), axis))
                    .unwrap_or(true);
                if !size_scales {
                    continue;
                }
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
        let mut ed = doc_with_shape();
        ed.apply(AddMasterInstance::new(0, 7, (3.0, 3.0))).unwrap();
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
}
