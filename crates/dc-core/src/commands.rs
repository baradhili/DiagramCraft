//! Editing commands — reversible mutations over the [`vsdx`] model.
//!
//! Every command captures what it needs in `apply` to restore the exact
//! prior state in `undo` (verbatim, including original attribute lists),
//! so an undone edit leaves the serialized package byte-identical.

use anyhow::{bail, Context, Result};
use vsdx::model::{Attrs, Cell, Document, IndexChild, Page, Row, Shape};

use crate::connects::{Connect, ConnectsOwner};
use crate::scope::{find_shape_mut, sheet_mut, Scope};

// ---------------------------------------------------------------------------
// Cell mutators
// ---------------------------------------------------------------------------

/// Replace a cell by name with an exact stored [`Cell`] (attrs verbatim);
/// appends when absent. Unlike [`vsdx::model::Sheet::set_cell`] this does
/// not re-synthesize the attribute list — used to restore prior state.
pub fn restore_cell(sheet: &mut vsdx::model::Sheet, name: &str, cell: Cell) {
    for child in sheet.children.iter_mut() {
        if let vsdx::model::SheetChild::Cell(n, slot) = child {
            if n == name {
                *slot = cell;
                return;
            }
        }
    }
    sheet
        .children
        .push(vsdx::model::SheetChild::Cell(name.to_string(), cell));
}

/// Remove a cell by name; returns the removed `(name, cell)` when present.
pub fn remove_cell(sheet: &mut vsdx::model::Sheet, name: &str) -> Option<(String, Cell)> {
    let idx = sheet
        .children
        .iter()
        .position(|c| matches!(c, vsdx::model::SheetChild::Cell(n, _) if n == name))?;
    match sheet.children.remove(idx) {
        vsdx::model::SheetChild::Cell(n, c) => Some((n, c)),
        _ => None,
    }
}

/// Address one row of a section: by `IX` (Geometry and friends) or by
/// name (`N` attribute: User/Prop/Layer rows).
#[derive(Debug, Clone, PartialEq)]
pub enum RowSel {
    Ix(i64),
    Named(String),
}

fn row_index(rows: &[Row], sel: &RowSel) -> Option<usize> {
    rows.iter().position(|r| match sel {
        RowSel::Ix(ix) => r.ix == *ix,
        RowSel::Named(n) => r.n.as_deref() == Some(n.as_str()),
    })
}

/// Set a cell inside a section row (replace preserving position, or push).
pub fn set_row_cell(
    sheet: &mut vsdx::model::Sheet,
    section: &str,
    section_ix: i64,
    row: &RowSel,
    name: &str,
    cell: Cell,
) -> Result<()> {
    let sec = sheet
        .children
        .iter_mut()
        .find_map(|c| match c {
            vsdx::model::SheetChild::Section(s) if s.name == section && s.ix == section_ix => {
                Some(s)
            }
            _ => None,
        })
        .with_context(|| format!("section {section}:{section_ix} not found"))?;
    let ri = row_index(&sec.rows, row)
        .with_context(|| format!("row {row:?} not found in section {section}:{section_ix}"))?;
    let row = &mut sec.rows[ri];
    match row.cells.iter_mut().find(|(n, _)| n == name) {
        Some((_, slot)) => *slot = cell,
        None => row.cells.push((name.to_string(), cell)),
    }
    Ok(())
}

/// Read a cell inside a section row.
pub fn row_cell<'a>(
    sheet: &'a vsdx::model::Sheet,
    section: &str,
    section_ix: i64,
    row: &RowSel,
    name: &str,
) -> Result<Option<&'a Cell>> {
    let sec = sheet
        .children
        .iter()
        .find_map(|c| match c {
            vsdx::model::SheetChild::Section(s) if s.name == section && s.ix == section_ix => {
                Some(s)
            }
            _ => None,
        })
        .with_context(|| format!("section {section}:{section_ix} not found"))?;
    let ri = row_index(&sec.rows, row)
        .with_context(|| format!("row {row:?} not found in section {section}:{section_ix}"))?;
    Ok(sec.rows[ri].cell(name))
}

/// Replace a section-row cell with an exact stored [`Cell`]; removes the
/// cell when `cell` is `None` (restores "was absent").
pub fn restore_row_cell(
    sheet: &mut vsdx::model::Sheet,
    section: &str,
    section_ix: i64,
    row: &RowSel,
    name: &str,
    cell: Option<Cell>,
) -> Result<()> {
    let sec = sheet
        .children
        .iter_mut()
        .find_map(|c| match c {
            vsdx::model::SheetChild::Section(s) if s.name == section && s.ix == section_ix => {
                Some(s)
            }
            _ => None,
        })
        .with_context(|| format!("section {section}:{section_ix} not found"))?;
    let ri = row_index(&sec.rows, row)
        .with_context(|| format!("row {row:?} not found in section {section}:{section_ix}"))?;
    let row = &mut sec.rows[ri];
    match cell {
        Some(c) => match row.cells.iter_mut().find(|(n, _)| n == name) {
            Some((_, slot)) => *slot = c,
            None => row.cells.push((name.to_string(), c)),
        },
        None => {
            if let Some(pos) = row.cells.iter().position(|(n, _)| n == name) {
                row.cells.remove(pos);
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Cell / text commands
// ---------------------------------------------------------------------------

/// Set (or replace) one sheet cell. `new.attrs` is synthesized as
/// `N`/`V`/`U`/`F` on replace, matching [`vsdx::model::Sheet::set_cell`].
#[derive(Debug, Clone)]
pub struct SetCell {
    pub scope: Scope,
    pub name: String,
    pub new: Cell,
    old: Option<Cell>,
}

impl SetCell {
    pub fn new(scope: Scope, name: impl Into<String>, new: Cell) -> SetCell {
        SetCell {
            scope,
            name: name.into(),
            new,
            old: None,
        }
    }
}

impl crate::Command for SetCell {

    fn touched(&self, doc: &Document) -> crate::TouchList {
        match &self.scope {
            Scope::Shape { page, shape } => {
                let Some(root) = crate::scope::top_level_ancestor(doc, *page, *shape) else {
                    return crate::TouchList::page(*page);
                };
                // PinX/PinY on the top-level shape itself is a pure
                // translation (MoveShape and friends); anything else —
                // including a sub-shape's pin, which moves ink WITHIN
                // its group — is ink.
                let move_only =
                    root == *shape && (self.name == "PinX" || self.name == "PinY");
                crate::TouchList {
                    pages: vec![],
                    shapes: vec![crate::Touched {
                        page: *page,
                        shape: root,
                        touch: if move_only { crate::Touch::Move } else { crate::Touch::Ink },
                    }],
                }
            }
            // Page-sheet geometry (PageWidth…); styles and masters are
            // inherited by every instance — whole-page dirty either way.
            Scope::PageSheet { page } => crate::TouchList::page(*page),
            _ => crate::TouchList::all_pages(doc),
        }
    }

    fn label(&self) -> String {
        format!("set {}:{} = {:?}", self.scope, self.name, self.new.v)
    }
    fn apply(&mut self, doc: &mut Document) -> Result<()> {
        if let Some(reason) = protection_refusal(doc, &self.scope, &self.name, &self.new) {
            bail!("{reason}");
        }
        let (name, new) = (self.name.clone(), self.new.clone());
        let s = sheet_mut(doc, &self.scope)?;
        self.old = s.cell(&name).cloned();
        s.set_cell(&name, new);
        Ok(())
    }
    fn undo(&mut self, doc: &mut Document) -> Result<()> {
        let s = sheet_mut(doc, &self.scope)?;
        match self.old.clone() {
            Some(old) => restore_cell(s, &self.name, old),
            None => {
                remove_cell(s, &self.name);
            }
        }
        Ok(())
    }
}

/// The Protection lock that guards `cell`, if any (Visio's geometry
/// locks — the same four the canvas gestures honor).
fn lock_for(cell: &str) -> Option<&'static str> {
    Some(match cell {
        "Width" => "LockWidth",
        "Height" => "LockHeight",
        "BeginX" | "BeginY" => "LockBegin",
        "EndX" | "EndY" => "LockEnd",
        _ => return None,
    })
}

/// A Protection flag is on when its cell (sheet-level or a Protection
/// section cell/row) resolves non-zero — on the instance or its master
/// source.
fn sheet_locked(sheet: &vsdx::model::Sheet, name: &str) -> bool {
    let on = |c: Option<&Cell>| {
        c.and_then(|c| c.numeric())
            .map(|v| v != 0.0)
            .unwrap_or(false)
    };
    on(sheet.cell(name))
        || sheet
            .sections_named("Protection")
            .iter()
            .any(|sec| on(sec.cell(name)) || sec.rows.iter().any(|r| on(r.cell(name))))
}

/// The effective formula for `name` on `sh`: the instance cell's own
/// formula, or — when the instance carries only an `F='Inh'` snapshot
/// (`resolve_cell` prefers the instance cell for xform cells) — the
/// master-source formula behind it. A `GUARD(` there guards the
/// instance even though the snapshot's `V` sits in front of it.
pub fn effective_formula(
    doc: &Document,
    sh: &vsdx::model::Shape,
    chain: &[&vsdx::model::Shape],
    name: &str,
) -> Option<String> {
    let mut f = doc
        .resolve_cell(sh, chain, name)
        .and_then(|c| c.f.clone())
        .filter(|f| f != "Inh");
    if f.is_none() {
        f = doc
            .instance_source(sh, chain)
            .and_then(|m| m.sheet.cell(name))
            .and_then(|c| c.f.clone())
            .filter(|f| f != "Inh");
    }
    f
}


/// Group-member scaling for a group resize (#69, #26's v1 boundary):
/// under the no-scale rendering convention a group's size lives in
/// its members' local cells, so resizing the group must scale each
/// descendant's LITERAL Xform cells — positions and loc-pins always,
/// sizes unless protected. The #15 per-shape rules, expressed against
/// this codebase's machinery:
///
/// - a cell with a real formula anywhere in its resolution chain
///   (`GUARD(…)`, a proportional master formula) is NOT written —
///   guarded sizes keep their dimensions, and formula-driven cells
///   belong to the #97 cache-refresh walk that runs alongside the
///   resize macro;
/// - a Protection lock (`LockWidth`/`LockHeight`) keeps the size;
///   the position still scales;
/// - nested groups recurse with the SAME factors: each level's
///   members scale exactly once (the child group's own Xform scales
///   as a member, and the no-scale convention means its members need
///   the same factor applied directly).
pub fn scale_group_members(
    doc: &Document,
    page: i64,
    group: i64,
    kx: f64,
    ky: f64,
) -> Vec<Box<dyn crate::Command>> {
    if ((kx - 1.0).abs() < 1e-9) && ((ky - 1.0).abs() < 1e-9) {
        return Vec::new();
    }
    let Some(p) = doc.pages.iter().find(|p| p.id == page) else {
        return Vec::new();
    };
    let Some(root) = vsdx::model::find_shape_by_id(&p.shapes, group) else {
        return Vec::new();
    };
    let mut cmds = Vec::new();
    scale_members_of(doc, page, root, kx, ky, &mut cmds);
    cmds
}

/// One group's direct members; nested groups recurse after their own
/// Xform scales.
fn scale_members_of(
    doc: &Document,
    page: i64,
    group: &vsdx::model::Shape,
    kx: f64,
    ky: f64,
    cmds: &mut Vec<Box<dyn crate::Command>>,
) {
    for m in &group.sub_shapes {
        let chain = crate::scope::shape_chain(doc, page, m.id).unwrap_or_default();
        for (name, k) in [
            ("PinX", kx),
            ("PinY", ky),
            ("LocPinX", kx),
            ("LocPinY", ky),
            ("Width", kx),
            ("Height", ky),
        ] {
            if (k - 1.0).abs() < 1e-9 {
                continue; // a guarded group axis: nothing to scale
            }
            let is_size = matches!(name, "Width" | "Height");
            if is_size && sheet_locked(&m.sheet, lock_for(name).unwrap_or("")) {
                continue;
            }
            // A real formula anywhere in the resolution chain marks the
            // cell as guarded or refresh-driven — never ours to write.
            if effective_formula(doc, m, &chain, name).is_some() {
                continue;
            }
            let Some(cell) = m.sheet.cell(name) else {
                // No local cell: a resolved LITERAL behind it scales as
                // an instance snapshot (Visio materializes on edit —
                // master instances inside groups carry no local
                // Width/Height, exactly this case).
                let Some(v) = doc
                    .resolve_cell(m, &chain, name)
                    .and_then(|c| c.numeric())
                    .filter(|v| v.is_finite())
                else {
                    continue;
                };
                cmds.push(Box::new(crate::SetCell::new(
                    crate::Scope::Shape {
                        page,
                        shape: m.id,
                    },
                    name,
                    Cell::value(format!("{:.6}", v * k)),
                )));
                continue;
            };
            // Only LITERAL local cells are ours (an `F='Inh'` snapshot
            // with a formula behind it was filtered above).
            if cell
                .f
                .as_deref()
                .is_some_and(|f| f != "Inh" && f != "No Formula")
            {
                continue;
            }
            let Some(v) = cell.numeric() else { continue };
            cmds.push(Box::new(crate::SetCell::new(
                crate::Scope::Shape {
                    page,
                    shape: m.id,
                },
                name,
                Cell::value(format!("{:.6}", v * k)),
            )));
        }
        if m.type_.as_deref() == Some("Group") {
            scale_members_of(doc, page, m, kx, ky, cmds);
        }
    }
}

/// Visio protection semantics shared by every surface (#97 §6 item 6):
/// a **plain value** write to a shape cell is refused when the cell —
/// resolved instance → master — carries a `GUARD(...)` formula, or the
/// Protection section locks the cell's axis (LockWidth/Height/Begin/
/// End, on the instance or its master). Writes that carry a formula
/// are the documented escape hatch and always allowed: ShapeSheet
/// editing (the CLI's `--formula`) and the flattened-group cache
/// refresh's `F='Inh'` snapshots legitimately land under guarded
/// master formulas. Before this lived only in the GUI's gestures,
/// `dc edit --cell` silently clobbered every protection the GUI
/// honored.
pub fn protection_refusal(doc: &Document, scope: &Scope, name: &str, new: &Cell) -> Option<String> {
    if new.f.is_some() {
        return None;
    }
    let Scope::Shape { page, shape } = scope else {
        return None;
    };
    let p = doc.pages.iter().find(|p| p.id == *page)?;
    let sh = vsdx::model::find_shape_by_id(&p.shapes, *shape)?;
    let chain = crate::scope::shape_chain(doc, *page, *shape).ok()?;
    if let Some(f) = effective_formula(doc, sh, &chain, name) {
        let guard = f.trim().trim_start_matches('=').trim();
        if guard.to_ascii_uppercase().starts_with("GUARD(") {
            return Some(format!("{name} is GUARDed ({f}) — edit the formula to change it"));
        }
    }
    if let Some(lock) = lock_for(name) {
        let locked = sheet_locked(&sh.sheet, lock)
            || doc
                .instance_source(sh, &chain)
                .map(|src| sheet_locked(&src.sheet, lock))
                .unwrap_or(false);
        if locked {
            return Some(format!("{lock} is set (Protection section) — {name} is locked"));
        }
    }
    None
}

/// Set one cell of a section row (e.g. a geometry row coordinate).
#[derive(Debug, Clone)]
pub struct SetRowCell {
    pub scope: Scope,
    pub section: String,
    pub section_ix: i64,
    pub row: RowSel,
    pub name: String,
    pub new: Cell,
    old: Option<Cell>,
    /// Set when apply() created the local section and/or row (Visio
    /// materializes inherited sections on first edit); undo removes them.
    created_section: bool,
    created_row: bool,
}

impl SetRowCell {
    pub fn new(
        scope: Scope,
        section: impl Into<String>,
        section_ix: i64,
        row: RowSel,
        name: impl Into<String>,
        new: Cell,
    ) -> SetRowCell {
        SetRowCell {
            scope,
            section: section.into(),
            section_ix,
            row,
            name: name.into(),
            new,
            old: None,
            created_section: false,
            created_row: false,
        }
    }
}

impl crate::Command for SetRowCell {

    fn touched(&self, doc: &Document) -> crate::TouchList {
        match &self.scope {
            Scope::Shape { page, shape } => match crate::scope::top_level_ancestor(doc, *page, *shape) {
                Some(root) => crate::TouchList {
                    pages: vec![],
                    shapes: vec![crate::Touched { page: *page, shape: root, touch: crate::Touch::Ink }],
                },
                None => crate::TouchList::page(*page),
            },
            Scope::PageSheet { page } => crate::TouchList::page(*page),
            _ => crate::TouchList::all_pages(doc),
        }
    }

    fn label(&self) -> String {
        format!(
            "set {}:{}[{:?}].{} = {:?}",
            self.scope, self.section, self.row, self.name, self.new.v
        )
    }
    fn apply(&mut self, doc: &mut Document) -> Result<()> {
        // Visio materializes inherited sections/rows on first edit: when
        // the local sheet lacks the section or row, create it seeded with
        // the inherited cells (effective sheet) so untouched values keep
        // their master's appearance.
        let inherited_sec: Option<vsdx::model::Section> = (|| {
            let Scope::Shape {
                page: page_id,
                shape: shape_id,
            } = &self.scope
            else {
                return None;
            };
            let page = doc.pages.iter().find(|p| p.id == *page_id)?;
            let sh = crate::scope::find_shape(&page.shapes, *shape_id)?;
            let chain = crate::scope::shape_chain(doc, *page_id, *shape_id).ok()?;
            let eff = doc.effective_sheet(sh, &chain);
            eff.sections_named(&self.section)
                .into_iter()
                .find(|s| s.ix == self.section_ix)
                .cloned()
        })();
        let inherited_row: Option<vsdx::model::Row> = inherited_sec.as_ref().and_then(|sec| {
            match &self.row {
                RowSel::Ix(ix) => sec.rows.iter().find(|r| r.ix == *ix),
                RowSel::Named(n) => sec.rows.iter().find(|r| r.n.as_deref() == Some(n.as_str())),
            }
            .cloned()
        });

        let s = sheet_mut(doc, &self.scope)?;
        let has_section = s.children.iter().any(|c| {
            matches!(c, vsdx::model::SheetChild::Section(sec) if sec.name == self.section && sec.ix == self.section_ix)
        });
        if !has_section {
            // Seed with the FULL inherited section: an empty partial
            // section would shadow the master's other rows (rich-text
            // runs, multiple Character rows) once it exists locally.
            let sec = match inherited_sec {
                Some(sec) => sec,
                None => vsdx::model::Section {
                    name: self.section.clone(),
                    ix: self.section_ix,
                    ..Default::default()
                },
            };
            s.children.push(vsdx::model::SheetChild::Section(sec));
            self.created_section = true;
        }
        {
            let sec = s
                .children
                .iter_mut()
                .find_map(|c| match c {
                    vsdx::model::SheetChild::Section(sec)
                        if sec.name == self.section && sec.ix == self.section_ix =>
                    {
                        Some(sec)
                    }
                    _ => None,
                })
                .expect("section just ensured");
            let has_row = match &self.row {
                RowSel::Ix(ix) => sec.rows.iter().any(|r| r.ix == *ix),
                RowSel::Named(n) => sec.rows.iter().any(|r| r.n.as_deref() == Some(n.as_str())),
            };
            if !has_row {
                let mut row = inherited_row.unwrap_or_default();
                if let RowSel::Ix(ix) = &self.row {
                    row.ix = *ix;
                }
                if let RowSel::Named(n) = &self.row {
                    row.n = Some(n.clone());
                }
                sec.rows.push(row);
                self.created_row = true;
            }
        }
        self.old = row_cell(s, &self.section, self.section_ix, &self.row, &self.name)?.cloned();
        set_row_cell(
            s,
            &self.section,
            self.section_ix,
            &self.row,
            &self.name,
            self.new.clone(),
        )?;
        Ok(())
    }
    fn undo(&mut self, doc: &mut Document) -> Result<()> {
        let s = sheet_mut(doc, &self.scope)?;
        restore_row_cell(
            s,
            &self.section,
            self.section_ix,
            &self.row,
            &self.name,
            self.old.clone(),
        )?;
        if self.created_row && !self.created_section {
            // We added a row to a pre-existing section: remove just it.
            if let Some(sec) = s.children.iter_mut().find_map(|c| match c {
                vsdx::model::SheetChild::Section(sec)
                    if sec.name == self.section && sec.ix == self.section_ix =>
                {
                    Some(sec)
                }
                _ => None,
            }) {
                let ri = match &self.row {
                    RowSel::Ix(ix) => sec.rows.iter().position(|r| r.ix == *ix),
                    RowSel::Named(n) => sec
                        .rows
                        .iter()
                        .position(|r| r.n.as_deref() == Some(n.as_str())),
                };
                if let Some(i) = ri {
                    sec.rows.remove(i);
                }
            }
            self.created_row = false;
        }
        if self.created_section {
            // We created the section (seeded wholesale from the inherited
            // one); every row in it is ours, so undo removes it whole.
            if let Some(i) = s.children.iter().position(|c| {
                matches!(c, vsdx::model::SheetChild::Section(sec) if sec.name == self.section && sec.ix == self.section_ix)
            }) {
                s.children.remove(i);
            }
            self.created_section = false;
        }
        Ok(())
    }
}

/// Set a sheet's text (plain text; markup is escaped on store). Text
/// belongs to shape sheets — page and master sub-shape scopes.
#[derive(Debug, Clone)]
pub struct SetText {
    pub scope: Scope,
    pub text: String,
    old: Option<(Option<String>, Option<String>)>, // (text, text_raw)
}

impl SetText {
    pub fn new(scope: Scope, text: impl Into<String>) -> SetText {
        SetText {
            scope,
            text: text.into(),
            old: None,
        }
    }
}

impl crate::Command for SetText {

    fn touched(&self, doc: &Document) -> crate::TouchList {
        match &self.scope {
            Scope::Shape { page, shape } => match crate::scope::top_level_ancestor(doc, *page, *shape) {
                Some(root) => crate::TouchList {
                    pages: vec![],
                    shapes: vec![crate::Touched { page: *page, shape: root, touch: crate::Touch::Ink }],
                },
                None => crate::TouchList::page(*page),
            },
            Scope::PageSheet { page } => crate::TouchList::page(*page),
            _ => crate::TouchList::all_pages(doc),
        }
    }

    fn label(&self) -> String {
        format!("set text of {} = {:?}", self.scope, self.text)
    }
    fn apply(&mut self, doc: &mut Document) -> Result<()> {
        let s = sheet_mut(doc, &self.scope)?;
        self.old = Some((s.text.clone(), s.text_raw.clone()));
        let escaped = self
            .text
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;");
        s.text = Some(self.text.clone());
        s.text_raw = Some(escaped);
        Ok(())
    }
    fn undo(&mut self, doc: &mut Document) -> Result<()> {
        let s = sheet_mut(doc, &self.scope)?;
        let (text, raw) = self.old.clone().unwrap_or((None, None));
        s.text = text;
        s.text_raw = raw;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Shape commands
// ---------------------------------------------------------------------------

/// Index path to a shape: each element is an index into that level's
/// container; the last element is the shape's slot. Empty = not found.
fn shape_path(shapes: &[Shape], id: i64, prefix: &mut Vec<usize>) -> bool {
    for (i, s) in shapes.iter().enumerate() {
        prefix.push(i);
        if s.id == id {
            return true;
        }
        if shape_path(&s.sub_shapes, id, prefix) {
            return true;
        }
        prefix.pop();
    }
    false
}

fn take_shape(shapes: &mut Vec<Shape>, id: i64) -> Option<Shape> {
    let mut path = Vec::new();
    if !shape_path(shapes, id, &mut path) {
        return None;
    }
    let (last, container) = path.split_last()?;
    let mut cur: &mut Vec<Shape> = shapes;
    for i in container {
        cur = &mut cur[*i].sub_shapes;
    }
    Some(cur.remove(*last))
}

/// Next free shape ID on a page (max existing + 1, 1-based start).
pub fn next_shape_id(doc: &Document, page: i64) -> i64 {
    fn walk_all(shapes: &[Shape], max: &mut i64) {
        for s in shapes {
            if s.id > *max {
                *max = s.id;
            }
            walk_all(&s.sub_shapes, max);
        }
    }
    let mut max = 0i64;
    if let Some(p) = doc.pages.iter().find(|p| p.id == page) {
        walk_all(&p.shapes, &mut max);
    }
    max + 1
}

/// Add a shape to a page (top level, or into a group by `parent`).
#[derive(Debug, Clone)]
pub struct AddShape {
    pub page: i64,
    pub parent: Option<i64>,
    pub shape: Shape,
}

impl crate::Command for AddShape {

    fn touched(&self, _doc: &Document) -> crate::TouchList {
        // Added INTO a group: the group's content changed (Ink on its
        // root); added at top level: new membership.
        match self.parent {
            None => crate::TouchList {
                pages: vec![],
                shapes: vec![crate::Touched { page: self.page, shape: self.shape.id, touch: crate::Touch::Membership }],
            },
            Some(_) => crate::TouchList::page(self.page),
        }
    }

    fn label(&self) -> String {
        format!(
            "add shape {} to page:{}{}",
            self.shape.id,
            self.page,
            self.parent
                .map(|p| format!(" group:{p}"))
                .unwrap_or_default()
        )
    }
    fn apply(&mut self, doc: &mut Document) -> Result<()> {
        let pi = doc
            .pages
            .iter()
            .position(|p| p.id == self.page)
            .ok_or_else(|| anyhow::anyhow!("page {} not found", self.page))?;
        let page = &mut doc.pages[pi];
        match self.parent {
            None => page.shapes.push(self.shape.clone()),
            Some(pid) => {
                let group = find_shape_mut(&mut page.shapes, pid).ok_or_else(|| {
                    anyhow::anyhow!("group shape {pid} not found on page {}", self.page)
                })?;
                group.sub_shapes.push(self.shape.clone());
            }
        }
        Ok(())
    }
    fn undo(&mut self, doc: &mut Document) -> Result<()> {
        let pi = doc
            .pages
            .iter()
            .position(|p| p.id == self.page)
            .ok_or_else(|| anyhow::anyhow!("page {} not found", self.page))?;
        let page = &mut doc.pages[pi];
        match self.parent {
            None => {
                take_shape(&mut page.shapes, self.shape.id)
                    .ok_or_else(|| anyhow::anyhow!("shape {} vanished", self.shape.id))?;
            }
            Some(pid) => {
                let group = find_shape_mut(&mut page.shapes, pid)
                    .ok_or_else(|| anyhow::anyhow!("group shape {pid} not found"))?;
                take_shape(&mut group.sub_shapes, self.shape.id)
                    .ok_or_else(|| anyhow::anyhow!("shape {} vanished", self.shape.id))?;
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Master instancing and import (#84)
// ---------------------------------------------------------------------------

/// Next free master ID (max existing + 1, 1-based start).
pub fn next_master_id(doc: &Document) -> i64 {
    doc.masters.iter().map(|m| m.id).max().unwrap_or(0) + 1
}

/// First value of a masters-index attribute, by name.
fn master_attr<'a>(m: &'a vsdx::model::Master, name: &str) -> Option<&'a str> {
    m.attrs
        .iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.as_str())
}

/// The document-local master a stencil master maps to, if it was
/// already imported: same non-empty `BaseID`, or — when the source
/// carries `MatchByName='1'` — same `NameU` (the VSDX reuse keys
/// Visio itself uses, #84).
pub fn find_imported(doc: &Document, src: &vsdx::model::Master) -> Option<i64> {
    if let Some(base) = master_attr(src, "BaseID").filter(|s| !s.is_empty()) {
        if let Some(m) = doc
            .masters
            .iter()
            .find(|m| master_attr(m, "BaseID") == Some(base))
        {
            return Some(m.id);
        }
    }
    if master_attr(src, "MatchByName") == Some("1") {
        if let Some(name) = src.name_u.as_deref() {
            if let Some(m) = doc
                .masters
                .iter()
                .find(|m| m.name_u.as_deref() == Some(name))
            {
                return Some(m.id);
            }
        }
    }
    None
}

/// Copy a master from an open stencil into the drawing's Masters
/// collection on first use (#84, Visio's self-contained-drawing rule).
///
/// The source master is cloned at construction — a `Command` cannot
/// hold a reference into another `Document`. The copy gets a fresh
/// document-local id and a document-unique part name; the borrowed
/// `<Rel>` index child is dropped (the write path assigns a fresh rId,
/// `vsdx::write` masters reconciliation). `BaseID`/`NameU` travel
/// verbatim, so re-drops dedupe through [`find_imported`] and reuse
/// the imported copy instead of duplicating.
pub struct ImportMaster {
    source: vsdx::model::Master,
    /// What apply did: a fresh copy (undo removes it) or a dedupe hit
    /// (undo is a no-op).
    outcome: Option<ImportOutcome>,
}

enum ImportOutcome {
    /// A compatible master was already present; undo is a no-op.
    Reused,
    /// A fresh copy was appended at `pos`; undo removes it.
    Fresh { pos: usize },
}

impl ImportMaster {
    pub fn new(source: vsdx::model::Master) -> ImportMaster {
        ImportMaster {
            source,
            outcome: None,
        }
    }
}

impl crate::Command for ImportMaster {
    fn touched(&self, _doc: &Document) -> crate::TouchList {
        // Master definitions carry no page ink; the instance placed
        // alongside this in the same macro reports its own membership.
        crate::TouchList::none()
    }

    fn label(&self) -> String {
        format!(
            "import master {}",
            self.source.name_u.as_deref().unwrap_or("-")
        )
    }

    fn apply(&mut self, doc: &mut Document) -> Result<()> {
        if find_imported(doc, &self.source).is_some() {
            self.outcome = Some(ImportOutcome::Reused);
            return Ok(());
        }
        let id = next_master_id(doc);
        let mut m = self.source.clone();
        m.id = id;
        if let Some(slot) = m.attrs.iter_mut().find(|(k, _)| k == "ID") {
            slot.1 = id.to_string();
        } else {
            m.attrs.insert(0, ("ID".to_string(), id.to_string()));
        }
        // Document-unique part name: one past the highest existing
        // `master{N}.xml` — never a borrowed stencil part name.
        let next_part = 1 + doc
            .masters
            .iter()
            .filter_map(|m| {
                let stem = m.part.rsplit('/').next()?;
                stem.strip_prefix("master")?
                    .strip_suffix(".xml")?
                    .parse::<usize>()
                    .ok()
            })
            .max()
            .unwrap_or(0);
        m.part = format!("visio/masters/master{next_part}.xml");
        // The stencil's rIds belong to the stencil's rels, not ours.
        m.children
            .retain(|c| !matches!(c, IndexChild::Rel(_)));
        doc.masters.push(m);
        self.outcome = Some(ImportOutcome::Fresh {
            pos: doc.masters.len() - 1,
        });
        Ok(())
    }

    fn undo(&mut self, doc: &mut Document) -> Result<()> {
        match self.outcome.take() {
            Some(ImportOutcome::Fresh { pos }) => {
                if pos < doc.masters.len() {
                    doc.masters.remove(pos);
                }
            }
            Some(ImportOutcome::Reused) | None => {}
        }
        Ok(())
    }
}

/// One undoable place-from-stencil step (#84): import the master into
/// the drawing if it is not already there (Visio first-use copy), then
/// place the instance — one undo removes both together. The instance's
/// master id is precomputed from `doc` (deterministic: the import
/// assigns `next_master_id` at apply time against the same state).
pub fn place_master_from_stencil(
    doc: &Document,
    page: i64,
    src: &vsdx::model::Master,
    pin: (f64, f64),
) -> Macro {
    let name = src.name_u.as_deref().unwrap_or("-");
    match find_imported(doc, src) {
        Some(id) => Macro::new(
            format!("place master {name}"),
            vec![Box::new(AddMasterInstance::new(page, id, pin))],
        ),
        None => Macro::new(
            format!("place master {name}"),
            vec![
                Box::new(ImportMaster::new(src.clone())),
                Box::new(AddMasterInstance::new(page, next_master_id(doc), pin)),
            ],
        ),
    }
}

/// Place a new instance of a master onto a page at a pin position
/// (inches). Geometry/format inherit from the master, exactly like a
/// Visio drag-from-stencil drop. Lives in dc-core (not dc-ui) since
/// #84: it mutates the model like every other command here.
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

impl crate::Command for AddMasterInstance {
    fn touched(&self, _doc: &Document) -> crate::TouchList {
        // A fresh top-level instance (#93): membership only — its
        // layer renders on demand.
        match self.id {
            Some(id) => crate::TouchList {
                pages: vec![],
                shapes: vec![crate::Touched {
                    page: self.page,
                    shape: id,
                    touch: crate::Touch::Membership,
                }],
            },
            None => crate::TouchList::page(self.page),
        }
    }

    fn label(&self) -> String {
        format!(
            "add master {} instance at ({:.3}, {:.3})",
            self.master, self.pin.0, self.pin.1
        )
    }
    fn apply(&mut self, doc: &mut Document) -> Result<()> {
        let id = next_shape_id(doc, self.page);
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
    fn undo(&mut self, doc: &mut Document) -> Result<()> {
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

/// Remove one row from a sheet section (GitHub #28: deleting a
/// Hyperlink row). Undo reinserts it at its original position.
#[derive(Debug, Clone)]
pub struct RemoveSectionRow {
    pub scope: Scope,
    pub section: String,
    pub section_ix: i64,
    pub row: RowSel,
    removed: Option<(vsdx::model::Row, usize)>,
}

impl RemoveSectionRow {
    pub fn new(
        scope: Scope,
        section: impl Into<String>,
        section_ix: i64,
        row: RowSel,
    ) -> RemoveSectionRow {
        RemoveSectionRow {
            scope,
            section: section.into(),
            section_ix,
            row,
            removed: None,
        }
    }
}

impl crate::Command for RemoveSectionRow {

    fn touched(&self, doc: &Document) -> crate::TouchList {
        match &self.scope {
            Scope::Shape { page, shape } => match crate::scope::top_level_ancestor(doc, *page, *shape) {
                Some(root) => crate::TouchList {
                    pages: vec![],
                    shapes: vec![crate::Touched { page: *page, shape: root, touch: crate::Touch::Ink }],
                },
                None => crate::TouchList::page(*page),
            },
            Scope::PageSheet { page } => crate::TouchList::page(*page),
            _ => crate::TouchList::all_pages(doc),
        }
    }

    fn label(&self) -> String {
        format!(
            "remove {} row {:?} on {}",
            self.section, self.row, self.scope
        )
    }
    fn apply(&mut self, doc: &mut Document) -> Result<()> {
        let s = sheet_mut(doc, &self.scope)?;
        let sec = s
            .children
            .iter_mut()
            .find_map(|c| match c {
                vsdx::model::SheetChild::Section(sec)
                    if sec.name == self.section && sec.ix == self.section_ix =>
                {
                    Some(sec)
                }
                _ => None,
            })
            .ok_or_else(|| {
                anyhow::anyhow!("section {}:{} not found", self.section, self.section_ix)
            })?;
        let ri = row_index(&sec.rows, &self.row)
            .with_context(|| format!("row {:?} not found", self.row))?;
        self.removed = Some((sec.rows.remove(ri), ri));
        Ok(())
    }
    fn undo(&mut self, doc: &mut Document) -> Result<()> {
        let (row, pos) = self
            .removed
            .clone()
            .ok_or_else(|| anyhow::anyhow!("not yet applied"))?;
        let s = sheet_mut(doc, &self.scope)?;
        let sec = s
            .children
            .iter_mut()
            .find_map(|c| match c {
                vsdx::model::SheetChild::Section(sec)
                    if sec.name == self.section && sec.ix == self.section_ix =>
                {
                    Some(sec)
                }
                _ => None,
            })
            .ok_or_else(|| anyhow::anyhow!("section vanished before undo"))?;
        let at = pos.min(sec.rows.len());
        sec.rows.insert(at, row);
        Ok(())
    }
}

/// Remove a shape (any depth), storing it with its position for undo.
#[derive(Debug, Clone)]
pub struct RemoveShape {
    pub page: i64,
    pub shape: i64,
    removed: Option<(Shape, Vec<usize>)>, // (shape, index path)
}

impl RemoveShape {
    pub fn new(page: i64, shape: i64) -> RemoveShape {
        RemoveShape {
            page,
            shape,
            removed: None,
        }
    }
}

impl crate::Command for RemoveShape {

    fn touched(&self, doc: &Document) -> crate::TouchList {
        // `removed`'s index path says where the shape lived. shape_path
        // numbers EVERY level (its doc: "the last element is the
        // shape's slot"), so a top-level shape is path.len() == 1 and a
        // nested one is longer. A len-1 path used to fall into the
        // nested walk below, which read page.shapes[path[0]] AFTER the
        // removal — the slot now holds the successor — so a top-level
        // removal touched an unrelated sibling's Ink and never emitted
        // the removed shape's own Membership.
        let path = self.removed.as_ref().map(|(_, p)| p.clone()).unwrap_or_default();
        if path.len() == 1 {
            return crate::TouchList {
                pages: vec![],
                shapes: vec![crate::Touched { page: self.page, shape: self.shape, touch: crate::Touch::Membership }],
            };
        }
        // Empty path: nothing captured yet (touched before apply) —
        // say nothing precise, invalidate the page.
        if path.is_empty() {
            return crate::TouchList::page(self.page);
        }
        // Post-apply the shape is gone; walk to its former container
        // to confirm the path resolves, then roll up to the top-level
        // root at path[0].
        if let Some(page) = doc.pages.iter().find(|p| p.id == self.page) {
            let mut node = page.shapes.get(path[0]);
            for ix in path.iter().take(path.len().saturating_sub(1)).skip(1) {
                node = node.and_then(|n| n.sub_shapes.get(*ix));
            }
            if node.is_some() {
                return crate::TouchList {
                    pages: vec![],
                    shapes: vec![crate::Touched {
                        page: self.page,
                        shape: path_root(page, &path),
                        touch: crate::Touch::Ink,
                    }],
                };
            }
        }
        crate::TouchList::page(self.page)
    }

    fn label(&self) -> String {
        format!("remove shape {} from page:{}", self.shape, self.page)
    }
    fn apply(&mut self, doc: &mut Document) -> Result<()> {
        let pi = doc
            .pages
            .iter()
            .position(|p| p.id == self.page)
            .ok_or_else(|| anyhow::anyhow!("page {} not found", self.page))?;
        let shapes = &mut doc.pages[pi].shapes;
        let mut path = Vec::new();
        if !shape_path(shapes, self.shape, &mut path) {
            bail!("shape {} not found on page {}", self.shape, self.page);
        }
        let taken = take_shape(shapes, self.shape).expect("path found");
        self.removed = Some((taken, path));
        Ok(())
    }
    fn undo(&mut self, doc: &mut Document) -> Result<()> {
        let (shape, path) = self
            .removed
            .clone()
            .ok_or_else(|| anyhow::anyhow!("nothing to undo"))?;
        let pi = doc
            .pages
            .iter()
            .position(|p| p.id == self.page)
            .ok_or_else(|| anyhow::anyhow!("page {} not found", self.page))?;
        let (last, container) = path
            .split_last()
            .ok_or_else(|| anyhow::anyhow!("corrupt undo path"))?;
        let mut cur: &mut Vec<Shape> = &mut doc.pages[pi].shapes;
        for i in container {
            cur = &mut cur[*i].sub_shapes;
        }
        let at = (*last).min(cur.len());
        cur.insert(at, shape);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Duplicate / z-order commands
// ---------------------------------------------------------------------------

/// Duplicate a top-level shape on a page: a deep clone with a fresh ID,
/// offset by `offset` inches (Visio nudges duplicates right and down).
/// The clone lands at the end of the shape list (top of the z-order).
#[derive(Debug, Clone)]
pub struct DuplicateShape {
    pub page: i64,
    pub shape: i64,
    pub offset: (f64, f64),
    clone: Option<Shape>,
}

impl DuplicateShape {
    pub fn new(page: i64, shape: i64, offset: (f64, f64)) -> DuplicateShape {
        DuplicateShape {
            page,
            shape,
            offset,
            clone: None,
        }
    }
}

impl crate::Command for DuplicateShape {

    fn touched(&self, doc: &Document) -> crate::TouchList {
        match crate::scope::top_level_ancestor(doc, self.page, self.shape) {
            // Top-level original ⇒ top-level clone: new membership.
            Some(root) if root == self.shape => crate::TouchList {
                pages: vec![],
                shapes: vec![crate::Touched {
                    page: self.page,
                    shape: self.clone.as_ref().map(|c| c.id).unwrap_or(self.shape),
                    touch: crate::Touch::Membership,
                }],
            },
            // Sub-shape duplicate: the containing group's ink changed.
            Some(root) => crate::TouchList {
                pages: vec![],
                shapes: vec![crate::Touched { page: self.page, shape: root, touch: crate::Touch::Ink }],
            },
            None => crate::TouchList::page(self.page),
        }
    }

    fn label(&self) -> String {
        format!("duplicate shape {} on page:{}", self.shape, self.page)
    }
    fn apply(&mut self, doc: &mut Document) -> Result<()> {
        let new_id = next_shape_id(doc, self.page);
        let pi = doc
            .pages
            .iter()
            .position(|p| p.id == self.page)
            .ok_or_else(|| anyhow::anyhow!("page {} not found", self.page))?;
        let page = &mut doc.pages[pi];
        let mut path = Vec::new();
        if !shape_path(&page.shapes, self.shape, &mut path) {
            bail!("shape {} not found on page {}", self.shape, self.page);
        }
        let src = take_shape(&mut page.shapes, self.shape)
            .ok_or_else(|| anyhow::anyhow!("shape {} vanished", self.shape))?;
        let mut copy = src.clone();
        copy.id = new_id;
        // Offset the pin; strip the instance name suffix so the copy reads
        // as a fresh instance (NameU stays, Visio appends .N itself).
        for (name, dv) in [("PinX", self.offset.0), ("PinY", self.offset.1)] {
            let v = copy
                .sheet
                .cell(name)
                .and_then(|c| c.numeric())
                .unwrap_or(0.0)
                + dv;
            copy.sheet.set_cell(name, Cell::value(format!("{v}")));
        }
        // Restore the source first, then push the copy (top of z-order).
        let (last, container) = path
            .split_last()
            .ok_or_else(|| anyhow::anyhow!("corrupt path"))?;
        let mut cur: &mut Vec<Shape> = &mut page.shapes;
        for i in container {
            cur = &mut cur[*i].sub_shapes;
        }
        cur.insert((*last).min(cur.len()), src);
        page.shapes.push(copy.clone());
        self.clone = Some(copy);
        Ok(())
    }
    fn undo(&mut self, doc: &mut Document) -> Result<()> {
        let Some(clone) = self.clone.as_ref() else {
            bail!("nothing to undo");
        };
        let pi = doc
            .pages
            .iter()
            .position(|p| p.id == self.page)
            .ok_or_else(|| anyhow::anyhow!("page {} not found", self.page))?;
        let page = &mut doc.pages[pi];
        let removed = take_shape(&mut page.shapes, clone.id);
        if removed.is_none() {
            bail!("duplicate shape {} vanished", clone.id);
        }
        Ok(())
    }
}

/// A plain container around the given members (GitHub #27): a
/// rectangle frame shape with a top-left heading text block, inserted
/// at the BACK of the z-order, with membership recorded the Visio way
/// — each member carries `User.msvSDContainerShape` = the container's
/// shape ID. Returned as one Macro (one undo step); `id` comes from
/// [`next_shape_id`] at the call site.
pub fn add_container(
    page: i64,
    id: i64,
    members: &[i64],
    rect: (f64, f64, f64, f64), // x0, y0, w, h (page coords, y-up)
    heading: &str,
) -> Macro {
    let mut sheet = vsdx::model::Sheet::default();
    let set = |sh: &mut vsdx::model::Sheet, n: &str, v: &str| {
        sh.set_cell(n, vsdx::model::Cell::value(v.to_string()));
    };
    set(&mut sheet, "PinX", &fmt_num(rect.0));
    set(&mut sheet, "PinY", &fmt_num(rect.1));
    set(&mut sheet, "Width", &fmt_num(rect.2));
    set(&mut sheet, "Height", &fmt_num(rect.3));
    set(&mut sheet, "LocPinX", "0");
    set(&mut sheet, "LocPinY", "0");
    // Heading text: the standard text block, pinned into the top-left
    // corner strip (Visio's Plain container look).
    // #138 §2 — the styled heading band: the heading sits in a filled
    // strip across the container's top (the highlight blue, white
    // text), the body below is outline-only. Geometry rows are
    // FORMULA-authored (Width/Height references) so the band and body
    // track container resizes; the band's height is the member pad.
    set(&mut sheet, "TxtPinX", "0.2");
    set(&mut sheet, "TxtLocPinX", "0");
    sheet.set_cell(
        "TxtPinY",
        vsdx::model::Cell {
            v: None,
            // Band center: the top 0.25 in strip (PAD below).
            f: Some("Height-0.125".into()),
            ..vsdx::model::Cell::default()
        },
    );
    set(&mut sheet, "TxtLocPinY", "0");
    set(&mut sheet, "FillForegnd", "#2e74b5");
    // (Geometry rows are formula-authored below — the literal w/h the
    // old plain-frame rows used are gone.)
    // Heading text in the band is white.
    sheet.children.push(vsdx::model::SheetChild::Section(
        vsdx::model::Section {
            name: "Character".into(),
            ix: 0,
            rows: vec![vsdx::model::Row {
                cells: vec![(
                    "Color".into(),
                    vsdx::model::Cell::value("#FFFFFF".to_string()),
                )],
                ..vsdx::model::Row::default()
            }],
            ..vsdx::model::Section::default()
        },
    ));
    let container = Shape {
        id,
        name_u: Some(format!("Container.{id}")),
        type_: Some("Shape".into()),
        sheet,
        ..Shape::default()
    };
    let mut container = container;
    let heading_with_count = format!("{heading} ({})", members.len());
    container.sheet.text = Some(heading_with_count.clone());
    // The writer emits <Text> from text_raw (verbatim), so seed the
    // escaped form too — the SetText convention.
    container.sheet.text_raw = Some(
        heading_with_count
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;"),
    );
    // Section 0 — the heading band (filled). Formula rows keep it
    // glued to the container's live Width/Height.
    let fcell = |f: &str| {
        vsdx::model::Cell {
            v: None,
            f: Some(f.to_string()),
            ..vsdx::model::Cell::default()
        }
    };
    let frow = |ix: i64, t: &str, x: &str, y: &str| vsdx::model::Row {
        ix,
        t: Some(t.into()),
        cells: vec![
            ("X".into(), fcell(x)),
            ("Y".into(), fcell(y)),
        ],
        ..vsdx::model::Row::default()
    };
    container
        .sheet
        .children
        .push(vsdx::model::SheetChild::Section(vsdx::model::Section {
            name: "Geometry".into(),
            ix: 0,
            rows: vec![
                frow(1, "MoveTo", "0", "Height-0.25"),
                frow(2, "LineTo", "Width*1", "Height-0.25"),
                frow(3, "LineTo", "Width*1", "Height*1"),
                frow(4, "LineTo", "0", "Height*1"),
                frow(5, "LineTo", "0", "Height-0.25"),
            ],
            ..vsdx::model::Section::default()
        }));
    // Section 1 — the body: outline only (NoFill), same live frame.
    container
        .sheet
        .children
        .push(vsdx::model::SheetChild::Section(vsdx::model::Section {
            name: "Geometry".into(),
            ix: 1,
            cells: vec![("NoFill".into(), vsdx::model::Cell::value("1"))],
            rows: vec![
                frow(1, "MoveTo", "0", "0"),
                frow(2, "LineTo", "Width*1", "0"),
                frow(3, "LineTo", "Width*1", "Height*1"),
                frow(4, "LineTo", "0", "Height*1"),
                frow(5, "LineTo", "0", "0"),
            ],
            ..vsdx::model::Section::default()
        }));
    // The base title rides on the sheet (#138: the heading text is
    // re-derived as "{title} ({member count})" by membership flows.
    container.sheet.children.push(vsdx::model::SheetChild::Section(
        vsdx::model::Section {
            name: "User".into(),
            ix: 0,
            rows: vec![vsdx::model::Row {
                n: Some("msvSDContainerTitle".into()),
                cells: vec![(
                    "Value".into(),
                    vsdx::model::Cell::value(heading.to_string()),
                )],
                ..vsdx::model::Row::default()
            }],
            ..vsdx::model::Section::default()
        },
    ));
    let mut cmds: Vec<Box<dyn crate::Command>> = vec![
        Box::new(AddShape {
            page,
            parent: None,
            shape: container,
        }),
        Box::new(ReorderShape::new(page, id, false)),
    ];
    for m in members {
        cmds.push(Box::new(SetRowCell::new(
            Scope::Shape { page, shape: *m },
            "User",
            0,
            RowSel::Named("msvSDContainerShape".into()),
            "Value",
            vsdx::model::Cell::value(id.to_string()),
        )));
    }
    Macro::new(
        format!("add container around {} shape(s)", members.len()),
        cmds,
    )
}

/// #138: membership-row cleanup for shapes being removed — every
/// shape whose `User.msvSDContainerShape` names one of `removed` loses
/// that row, so a deleted container leaves no dangling rows in the
/// saved file. Pure command list; the caller folds it into the same
/// delete Macro (one undo restores container and rows together).
pub fn container_membership_cleanup(
    doc: &Document,
    page: i64,
    removed: &[i64],
) -> Vec<Box<dyn crate::Command>> {
    let gone: std::collections::HashSet<i64> = removed.iter().copied().collect();
    let Some(p) = doc.pages.iter().find(|p| p.id == page) else {
        return Vec::new();
    };
    let mut cmds = Vec::new();
    for s in &p.shapes {
        let hits = s.sheet.sections_named("User").iter().any(|sec| {
            sec.named_row("msvSDContainerShape")
                .and_then(|r| r.cell("Value"))
                .and_then(|c| c.numeric())
                .map(|v| gone.contains(&(v as i64)))
                .unwrap_or(false)
        });
        if hits {
            cmds.push(Box::new(RemoveSectionRow::new(
                Scope::Shape { page, shape: s.id },
                "User",
                0,
                RowSel::Named("msvSDContainerShape".into()),
            )) as Box<dyn crate::Command>);
        }
    }
    cmds
}

/// Undo payload for [`GroupShapes`]: member id, original z-position, and
/// the pre-group cells of every Xform cell grouping rewrote (PinX/PinY,
/// plus a 1D member's endpoints).
type GroupUndo = Vec<(
    i64,
    usize,
    Vec<(&'static str, Option<vsdx::model::Cell>)>,
)>;

/// Group top-level shapes into one `Type='Group'` shape (GitHub #26).
/// The group's XForm is the members' union bbox with `LocPin` at its
/// bottom-left, and each member's Pin is rewritten into the group's
/// local frame — the renderer places sub-shapes relative to the
/// group's LocPin origin, so the drawing renders identically and the
/// members follow the group's moves. A 1D member's endpoints
/// (BeginX/BeginY/EndX/EndY) are parent-frame points too and are
/// offset alongside the Pin; an endpoint a formula owns cannot be
/// offset safely, so grouping a member with formula-driven endpoints
/// is refused.
#[derive(Debug, Clone)]
pub struct GroupShapes {
    pub page: i64,
    /// Member ids, any order; the group preserves ascending z-order.
    pub members: Vec<i64>,
    /// Nesting target (#95): the new group is inserted into this
    /// top-level group's `sub_shapes` instead of the page, pinned at
    /// `parent_local_pin` (the bbox origin converted into the parent's
    /// local frame — computed by the UI through the renderer's affine,
    /// which dc-core cannot reach).
    pub parent: Option<i64>,
    parent_local_pin: (f64, f64),
    id: Option<i64>,
    /// (member id, original z-position, rewritten cells with their
    /// originals) for undo.
    undo: Option<GroupUndo>,
}

impl GroupShapes {
    pub fn new(page: i64, members: Vec<i64>) -> GroupShapes {
        GroupShapes {
            page,
            members,
            parent: None,
            parent_local_pin: (0.0, 0.0),
            id: None,
            undo: None,
        }
    }

    /// Nest the new group inside `parent` (a top-level Group), pinned
    /// at `parent_local_pin` — the members' page-space bbox origin
    /// mapped into the parent's local frame (#95). The members stay
    /// top-level-resolved and are re-pinned into the NEW group's
    /// frame exactly as the top-level variant; only where the new
    /// group lands differs.
    pub fn into_group(
        page: i64,
        members: Vec<i64>,
        parent: i64,
        parent_local_pin: (f64, f64),
    ) -> GroupShapes {
        GroupShapes {
            page,
            members,
            parent: Some(parent),
            parent_local_pin,
            id: None,
            undo: None,
        }
    }
}

impl crate::Command for GroupShapes {
    fn label(&self) -> String {
        format!("group {} shapes on page {}", self.members.len(), self.page)
    }
    fn apply(&mut self, doc: &mut Document) -> Result<()> {
        let pi = doc
            .pages
            .iter()
            .position(|p| p.id == self.page)
            .ok_or_else(|| anyhow::anyhow!("page {} not found", self.page))?;
        // Resolve members (top-level only) with their z-positions.
        let mut found: Vec<(usize, i64)> = self
            .members
            .iter()
            .filter_map(|id| {
                doc.pages[pi]
                    .shapes
                    .iter()
                    .position(|s| s.id == *id)
                    .map(|p| (p, *id))
            })
            .collect();
        // Top-level grouping needs two members (a lone group is a
        // no-op); NESTING accepts one — dropping a single shape into
        // an existing group as its own sub-group is the #95 flow.
        if found.len() < 2 && self.parent.is_none() {
            bail!("grouping needs at least two member shapes");
        }
        if found.is_empty() {
            bail!("grouping needs at least one member shape");
        }
        found.sort();
        // Union bbox from each member's absolute rect.
        let num = |s: &Shape, n: &str| -> f64 {
            doc.resolve_cell(s, &[], n)
                .and_then(|c| c.numeric())
                .unwrap_or(0.0)
        };
        let mut x0 = f64::MAX;
        let mut y0 = f64::MAX;
        let mut x1 = f64::MIN;
        let mut y1 = f64::MIN;
        for (_, id) in &found {
            let s = doc.pages[pi].shapes.iter().find(|s| s.id == *id).unwrap();
            let (px, py) = (num(s, "PinX"), num(s, "PinY"));
            let (w, h) = (num(s, "Width"), num(s, "Height"));
            let (lx, ly) = (num(s, "LocPinX"), num(s, "LocPinY"));
            x0 = x0.min(px - lx);
            y0 = y0.min(py - ly);
            x1 = x1.max(px - lx + w);
            y1 = y1.max(py - ly + h);
        }
        let gid = next_shape_id(doc, self.page);
        self.id = Some(gid);
        let mut undo = Vec::new();
        // Rewrite members' Pins into the group frame and detach. A 1D
        // member's endpoints are parent-frame points as well (Visio
        // keeps sub-shape cells in the group's coordinate system):
        // offset them alongside the Pin, or the next endpoint drag or
        // derivation would snap the shape back to its pre-grouping page
        // position. An endpoint a formula owns cannot be offset safely
        // — refuse grouping that member (nothing has been mutated yet).
        let mut members: Vec<Shape> = Vec::new();
        for (pos, id) in &found {
            let s = doc.pages[pi].shapes.iter().find(|s| s.id == *id).unwrap();
            let (px, py) = (num(s, "PinX"), num(s, "PinY"));
            let mut undo_cells: Vec<(&'static str, Option<vsdx::model::Cell>)> = vec![
                ("PinX", s.sheet.cell("PinX").cloned()),
                ("PinY", s.sheet.cell("PinY").cloned()),
            ];
            let mut m = s.clone();
            m.sheet
                .set_cell("PinX", vsdx::model::Cell::value(fmt_num(px - x0)));
            m.sheet
                .set_cell("PinY", vsdx::model::Cell::value(fmt_num(py - y0)));
            for name in ["BeginX", "BeginY", "EndX", "EndY"] {
                let Some(c) = doc.resolve_cell(s, &[], name) else {
                    continue;
                };
                if c
                    .f
                    .as_deref()
                    .is_some_and(|f| !f.is_empty() && f != "Inh" && f != "No Formula")
                {
                    bail!("cannot group shape {id}: {name} is formula-driven");
                }
                let Some(v) = c.numeric() else {
                    bail!("cannot group shape {id}: {name} has no numeric value");
                };
                undo_cells.push((name, s.sheet.cell(name).cloned()));
                let off = if name.ends_with('X') { x0 } else { y0 };
                m.sheet
                    .set_cell(name, vsdx::model::Cell::value(fmt_num(v - off)));
            }
            undo.push((*id, *pos, undo_cells));
            members.push(m);
        }
        self.undo = Some(undo);
        // Remove back-to-front so earlier positions stay valid.
        for (pos, _) in found.iter().rev() {
            doc.pages[pi].shapes.remove(*pos);
        }
        // #95: nested groups are pinned in the PARENT's local frame
        // (the converted bbox origin); the group stays axis-aligned in
        // page space at creation, so a rotated parent skews the frame
        // v1 accepts.
        let (gpx, gpy) = match self.parent {
            Some(_) => self.parent_local_pin,
            None => (x0, y0),
        };
        let group = Shape {
            id: gid,
            name_u: Some(format!("Group.{gid}")),
            type_: Some("Group".into()),
            sheet: {
                let mut sh = vsdx::model::Sheet::default();
                sh.set_cell("PinX", vsdx::model::Cell::value(fmt_num(gpx)));
                sh.set_cell("PinY", vsdx::model::Cell::value(fmt_num(gpy)));
                sh.set_cell("Width", vsdx::model::Cell::value(fmt_num(x1 - x0)));
                sh.set_cell("Height", vsdx::model::Cell::value(fmt_num(y1 - y0)));
                sh.set_cell("LocPinX", vsdx::model::Cell::value("0"));
                sh.set_cell("LocPinY", vsdx::model::Cell::value("0"));
                sh
            },
            sub_shapes: members,
            flow: vec![vsdx::model::ShapeFlow::Shapes],
            ..Shape::default()
        };
        match self.parent {
            Some(pid) => {
                let parent = doc.pages[pi]
                    .shapes
                    .iter_mut()
                    .find(|s| s.id == pid)
                    .ok_or_else(|| anyhow::anyhow!("parent group {pid} vanished"))?;
                if parent.type_.as_deref() != Some("Group") {
                    bail!("nest target {pid} is not a group");
                }
                parent.sub_shapes.push(group);
            }
            None => {
                let at = found.first().unwrap().0.min(doc.pages[pi].shapes.len());
                doc.pages[pi].shapes.insert(at, group);
            }
        }
        Ok(())
    }
    fn undo(&mut self, doc: &mut Document) -> Result<()> {
        let gid = self.id.ok_or_else(|| anyhow::anyhow!("not yet applied"))?;
        let undo = self
            .undo
            .take()
            .ok_or_else(|| anyhow::anyhow!("not yet applied"))?;
        let pi = doc
            .pages
            .iter()
            .position(|p| p.id == self.page)
            .ok_or_else(|| anyhow::anyhow!("page {} not found", self.page))?;
        let mut extracted = match self.parent {
            Some(pid) => {
                let parent = doc.pages[pi]
                    .shapes
                    .iter_mut()
                    .find(|s| s.id == pid)
                    .ok_or_else(|| anyhow::anyhow!("parent group {pid} vanished"))?;
                let gpos = parent
                    .sub_shapes
                    .iter()
                    .position(|s| s.id == gid)
                    .ok_or_else(|| anyhow::anyhow!("group {gid} vanished from parent"))?;
                parent.sub_shapes.remove(gpos).sub_shapes
            }
            None => {
                let gpos = doc.pages[pi]
                    .shapes
                    .iter()
                    .position(|s| s.id == gid)
                    .ok_or_else(|| anyhow::anyhow!("group {gid} vanished"))?;
                doc.pages[pi].shapes.remove(gpos).sub_shapes
            }
        };
        // Restore original absolute Xform cells and z-positions.
        for (id, pos, cells) in &undo {
            let m = extracted
                .iter_mut()
                .find(|s| s.id == *id)
                .ok_or_else(|| anyhow::anyhow!("member {id} vanished from group"))?;
            for (name, old) in cells {
                match old {
                    Some(c) => m.sheet.set_cell(name, c.clone()),
                    None => m.sheet.children.retain(|c| {
                        !matches!(c, vsdx::model::SheetChild::Cell(k, _) if k.as_str() == *name)
                    }),
                }
            }
            let at = (*pos).min(doc.pages[pi].shapes.len());
            doc.pages[pi].shapes.insert(at, m.clone());
        }
        Ok(())
    }
}

/// Dissolve a group one level (GitHub #26): members are promoted to
/// the group's z-position with their Pins — and a 1D member's
/// endpoints — translated back to absolute page coordinates using the
/// group's live Xform.
#[derive(Debug, Clone)]
pub struct UngroupShape {
    pub page: i64,
    pub group: i64,
    /// The removed group (members still in local-frame state) — undo
    /// reinserts it verbatim.
    removed: Option<Shape>,
    old_pos: Option<usize>,
}

impl UngroupShape {
    pub fn new(page: i64, group: i64) -> UngroupShape {
        UngroupShape {
            page,
            group,
            removed: None,
            old_pos: None,
        }
    }
}

impl crate::Command for UngroupShape {
    fn label(&self) -> String {
        format!("ungroup shape {} on page {}", self.group, self.page)
    }
    fn apply(&mut self, doc: &mut Document) -> Result<()> {
        let pi = doc
            .pages
            .iter()
            .position(|p| p.id == self.page)
            .ok_or_else(|| anyhow::anyhow!("page {} not found", self.page))?;
        let pos = doc.pages[pi]
            .shapes
            .iter()
            .position(|s| s.id == self.group)
            .ok_or_else(|| anyhow::anyhow!("shape {} not found", self.group))?;
        if doc.pages[pi].shapes[pos].sub_shapes.is_empty() {
            bail!("shape {} has no members to ungroup", self.group);
        }
        let num = |s: &Shape, n: &str| -> f64 {
            doc.resolve_cell(s, &[], n)
                .and_then(|c| c.numeric())
                .unwrap_or(0.0)
        };
        // The group's live Xform maps member-local points to the page:
        // Pin + R(Angle)·(p − LocPin). Angle 0 (every group
        // GroupShapes builds) reduces to the plain Pin−LocPin offset;
        // a rotated group promotes its members around the arc.
        let (gpx, gpy, glx, gly, ang) = {
            let g = &doc.pages[pi].shapes[pos];
            (
                num(g, "PinX"),
                num(g, "PinY"),
                num(g, "LocPinX"),
                num(g, "LocPinY"),
                num(g, "Angle"),
            )
        };
        let (sin, cos) = ang.sin_cos();
        let to_page = |x: f64, y: f64| -> (f64, f64) {
            let (dx, dy) = (x - glx, y - gly);
            (gpx + dx * cos - dy * sin, gpy + dx * sin + dy * cos)
        };
        let group_shape = doc.pages[pi].shapes.remove(pos);
        self.old_pos = Some(pos);
        self.removed = Some(group_shape.clone());
        let mut promoted = group_shape.sub_shapes;
        for m in promoted.iter_mut() {
            let (px, py) = (
                m.sheet
                    .cell("PinX")
                    .and_then(|c| c.numeric())
                    .unwrap_or(0.0),
                m.sheet
                    .cell("PinY")
                    .and_then(|c| c.numeric())
                    .unwrap_or(0.0),
            );
            let (ppx, ppy) = to_page(px, py);
            m.sheet
                .set_cell("PinX", vsdx::model::Cell::value(fmt_num(ppx)));
            m.sheet
                .set_cell("PinY", vsdx::model::Cell::value(fmt_num(ppy)));
            // A 1D member's endpoints are points in the group frame —
            // map them back with the same transform so they stay
            // consistent with the promoted Pin.
            for (xn, yn) in [("BeginX", "BeginY"), ("EndX", "EndY")] {
                if let (Some(x), Some(y)) = (
                    m.sheet.cell(xn).and_then(|c| c.numeric()),
                    m.sheet.cell(yn).and_then(|c| c.numeric()),
                ) {
                    let (ex, ey) = to_page(x, y);
                    m.sheet
                        .set_cell(xn, vsdx::model::Cell::value(fmt_num(ex)));
                    m.sheet
                        .set_cell(yn, vsdx::model::Cell::value(fmt_num(ey)));
                }
            }
            // Compose the group's rotation into the member's own Angle:
            // a member's page orientation is the group angle plus its
            // own (the nested render transforms compose), so skipping
            // this would orbit a rotated group's members into position
            // but leave them pointing the old way.
            if ang != 0.0 {
                let a = m.sheet.cell("Angle").and_then(|c| c.numeric()).unwrap_or(0.0);
                m.sheet
                    .set_cell("Angle", vsdx::model::Cell::value(fmt_num(a + ang)));
            }
        }
        let at = pos.min(doc.pages[pi].shapes.len());
        doc.pages[pi].shapes.splice(at..at, promoted);
        Ok(())
    }
    fn undo(&mut self, doc: &mut Document) -> Result<()> {
        let removed = self
            .removed
            .take()
            .ok_or_else(|| anyhow::anyhow!("not yet applied"))?;
        let pos = self
            .old_pos
            .ok_or_else(|| anyhow::anyhow!("not yet applied"))?;
        let pi = doc
            .pages
            .iter()
            .position(|p| p.id == self.page)
            .ok_or_else(|| anyhow::anyhow!("page {} not found", self.page))?;
        let ids: Vec<i64> = removed.sub_shapes.iter().map(|s| s.id).collect();
        doc.pages[pi].shapes.retain(|s| !ids.contains(&s.id));
        let at = pos.min(doc.pages[pi].shapes.len());
        doc.pages[pi].shapes.insert(at, removed);
        Ok(())
    }
}

/// Move a top-level shape to the front or back of its page's z-order
/// (Visio Bring to Front / Send to Back).
#[derive(Debug, Clone)]
pub struct ReorderShape {
    pub page: i64,
    pub shape: i64,
    pub to_front: bool,
    old_index: Option<usize>,
}

impl ReorderShape {
    pub fn new(page: i64, shape: i64, to_front: bool) -> ReorderShape {
        ReorderShape {
            page,
            shape,
            to_front,
            old_index: None,
        }
    }
}

impl crate::Command for ReorderShape {

    fn touched(&self, doc: &Document) -> crate::TouchList {
        match crate::scope::top_level_ancestor(doc, self.page, self.shape) {
            // Z-order among top-level shapes: nothing cached depends
            // on it. Within a group the subtree's paint order is baked
            // into the group's texture — ink.
            Some(root) if root == self.shape => crate::TouchList {
                pages: vec![],
                shapes: vec![crate::Touched { page: self.page, shape: root, touch: crate::Touch::Order }],
            },
            Some(root) => crate::TouchList {
                pages: vec![],
                shapes: vec![crate::Touched { page: self.page, shape: root, touch: crate::Touch::Ink }],
            },
            None => crate::TouchList::page(self.page),
        }
    }

    fn label(&self) -> String {
        format!(
            "{} shape {} on page:{}",
            if self.to_front {
                "bring to front"
            } else {
                "send to back"
            },
            self.shape,
            self.page
        )
    }
    fn apply(&mut self, doc: &mut Document) -> Result<()> {
        let pi = doc
            .pages
            .iter()
            .position(|p| p.id == self.page)
            .ok_or_else(|| anyhow::anyhow!("page {} not found", self.page))?;
        let shapes = &mut doc.pages[pi].shapes;
        let idx = shapes
            .iter()
            .position(|s| s.id == self.shape)
            .ok_or_else(|| {
                anyhow::anyhow!("shape {} not found on page {}", self.shape, self.page)
            })?;
        self.old_index = Some(idx);
        let s = shapes.remove(idx);
        if self.to_front {
            shapes.push(s);
        } else {
            shapes.insert(0, s);
        }
        Ok(())
    }
    fn undo(&mut self, doc: &mut Document) -> Result<()> {
        let old = self
            .old_index
            .ok_or_else(|| anyhow::anyhow!("nothing to undo"))?;
        let pi = doc
            .pages
            .iter()
            .position(|p| p.id == self.page)
            .ok_or_else(|| anyhow::anyhow!("page {} not found", self.page))?;
        let shapes = &mut doc.pages[pi].shapes;
        let cur = shapes
            .iter()
            .position(|s| s.id == self.shape)
            .ok_or_else(|| anyhow::anyhow!("shape {} vanished", self.shape))?;
        let s = shapes.remove(cur);
        let at = old.min(shapes.len());
        shapes.insert(at, s);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Page commands
// ---------------------------------------------------------------------------

/// Next free page ID (max existing + 1; Visio page IDs start at 0).
pub fn next_page_id(doc: &Document) -> i64 {
    doc.pages
        .iter()
        .map(|p| p.id)
        .max()
        .map(|m| m + 1)
        .unwrap_or(0)
}

/// Next free page part name (`visio/pages/pageN.xml`), considering both
/// modeled pages and parts already present in the retained package.
pub fn next_page_part(doc: &Document) -> String {
    let mut max = 0usize;
    let consider = |name: &str, max: &mut usize| {
        let stem = name
            .rsplit('/')
            .next()
            .unwrap_or(name)
            .strip_prefix("page")
            .and_then(|s| s.strip_suffix(".xml"))
            .and_then(|s| s.parse::<usize>().ok());
        if let Some(n) = stem {
            if n > *max {
                *max = n;
            }
        }
    };
    for p in &doc.pages {
        consider(&p.part, &mut max);
    }
    if let Some(pkg) = &doc.package {
        for name in pkg.part_names() {
            if name.starts_with("visio/pages/") && name.ends_with(".xml") && !name.contains("_rels")
            {
                consider(name, &mut max);
            }
        }
    }
    format!("visio/pages/page{}.xml", max + 1)
}

/// Next free relationship ID for the pages index (`rIdN`), considering
/// the index entries' `<Rel r:id>` children and the retained package's
/// pages rels part.
pub fn next_page_rid(doc: &Document) -> String {
    let mut max = 0usize;
    let bump = |v: &str, max: &mut usize| {
        if let Some(n) = v.strip_prefix("rId").and_then(|s| s.parse::<usize>().ok()) {
            if n > *max {
                *max = n;
            }
        }
    };
    for p in &doc.pages {
        for c in &p.children {
            if let IndexChild::Rel(attrs) = c {
                for (k, v) in attrs {
                    if k == "r:id" || k.ends_with(":id") {
                        bump(v, &mut max);
                    }
                }
            }
        }
    }
    if let Some(pkg) = &doc.package {
        let dir = match doc.pages_part.rfind('/') {
            Some(i) => &doc.pages_part[..i],
            None => "",
        };
        let file = &doc.pages_part[dir.len() + if dir.is_empty() { 0 } else { 1 }..];
        let rels = if dir.is_empty() {
            format!("_rels/{file}.rels")
        } else {
            format!("{dir}/_rels/{file}.rels")
        };
        if let Some(bytes) = pkg.part(&rels) {
            if let Ok(text) = std::str::from_utf8(bytes) {
                for (k, v) in scan_attrs(text, "Relationship") {
                    if k == "Id" {
                        bump(&v, &mut max);
                    }
                }
            }
        }
    }
    format!("rId{}", max + 1)
}

/// Minimal attribute scanner for writer- and corpus-shaped XML fragments
/// (`<Tag k='v' k2='v2'/>`): returns every attribute of every matching
/// element.
fn scan_attrs(xml: &str, tag: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let needle = format!("<{tag}");
    let mut rest = xml;
    while let Some(pos) = rest.find(&needle) {
        let after = &rest[pos + needle.len()..];
        if let Some(end) = after.find('>') {
            let body = &after[..end];
            let mut i = 0;
            let b = body.as_bytes();
            while i < b.len() {
                while i < b.len() && (b[i] as char).is_whitespace() {
                    i += 1;
                }
                let ks = i;
                while i < b.len() && b[i] != b'=' && !(b[i] as char).is_whitespace() {
                    i += 1;
                }
                if i >= b.len() || b[i] != b'=' {
                    break;
                }
                let key = body[ks..i].to_string();
                i += 1; // '='
                while i < b.len() && (b[i] as char).is_whitespace() {
                    i += 1;
                }
                if i >= b.len() {
                    break;
                }
                let quote = b[i];
                i += 1;
                let vs = i;
                while i < b.len() && b[i] != quote {
                    i += 1;
                }
                let val = body[vs..i].to_string();
                i += 1;
                out.push((key, val));
            }
        }
        rest = &rest[pos + needle.len()..];
    }
    out
}

/// A constructed page for [`AddPage`]: index entry with a fresh `r:id`,
/// an assigned part, a `PageSheet` (every real page has one; it is the
/// layer-section host), and standard attributes.
/// A4 landscape, Visio's inches — the no-source fallback for a page
/// added to a document with no pages (#68; a fresh metric drawing's
/// first sheet). 297×210 mm.
const A4_LANDSCAPE_IN: (f64, f64) = (297.0 / 25.4, 210.0 / 25.4);

/// The PageSheet setup cells a new page inherits from its source page
/// (#68 — Visio's Insert Page defaults to "Copy settings from
/// <current page>"): the sheet size always, the scale pair and its
/// type when the source carries them, so scaled drawings behave.
const PAGE_SEED_CELLS: [&str; 5] = [
    "PageWidth",
    "PageHeight",
    "PageScale",
    "DrawingScale",
    "DrawingSizeType",
];

/// The setup cells for a page with no usable source: the A4-landscape
/// size.
fn a4_page_cells() -> Vec<vsdx::model::SheetChild> {
    let (w, h) = A4_LANDSCAPE_IN;
    vec![
        vsdx::model::SheetChild::Cell("PageWidth".into(), Cell::value(format!("{w}"))),
        vsdx::model::SheetChild::Cell("PageHeight".into(), Cell::value(format!("{h}"))),
    ]
}

/// Build a new page whose PageSheet setup is seeded from `source`
/// (#68): the source page's size and scale cells, the document's last
/// page when `source` is None, A4 landscape when there is no usable
/// source at all (no pages yet, or a source lacking the size cells —
/// itself a pre-#68 inserted sliver). IDs, the relationship, and the
/// part name are allocated fresh.
pub fn new_page(doc: &Document, name: &str, source: Option<&Page>) -> Page {
    let setup = match source.or_else(|| doc.pages.last()).and_then(|p| p.sheet()) {
        Some(sheet) => {
            let cells: Vec<vsdx::model::SheetChild> = PAGE_SEED_CELLS
                .iter()
                .filter_map(|n| {
                    sheet
                        .cell(n)
                        .map(|c| vsdx::model::SheetChild::Cell(n.to_string(), c.clone()))
                })
                .collect();
            let sized = cells.iter().any(|c| {
                matches!(c, vsdx::model::SheetChild::Cell(n, _) if n == "PageWidth")
            }) && cells
                .iter()
                .any(|c| matches!(c, vsdx::model::SheetChild::Cell(n, _) if n == "PageHeight"));
            if sized {
                cells
            } else {
                a4_page_cells()
            }
        }
        None => a4_page_cells(),
    };
    Page {
        id: next_page_id(doc),
        name: name.to_string(),
        attrs: vec![
            ("ID".to_string(), next_page_id(doc).to_string()),
            ("NameU".to_string(), name.to_string()),
            ("Name".to_string(), name.to_string()),
        ],
        children: vec![
            IndexChild::Sheet {
                tag: "PageSheet".to_string(),
                sheet: vsdx::model::Sheet {
                    attrs: vec![
                        ("LineStyle".to_string(), "0".to_string()),
                        ("FillStyle".to_string(), "0".to_string()),
                        ("TextStyle".to_string(), "0".to_string()),
                    ],
                    children: setup,
                    ..vsdx::model::Sheet::default()
                },
            },
            IndexChild::Rel(vec![("r:id".to_string(), next_page_rid(doc))]),
        ],
        part: next_page_part(doc),
        ..Page::default()
    }
}

/// Append a page. The part name, relationship ID, and page ID are
/// allocated at apply time (and captured, so redo reuses them). The
/// new page's PageSheet setup is seeded from a source page (#68), and
/// the seeded cells are captured at the first apply so redo replays
/// them exactly even if the source's setup moved on in between.
#[derive(Debug, Clone)]
pub struct AddPage {
    pub name: String,
    /// Page whose setup seeds the new PageSheet (#68, Visio's "Copy
    /// settings from: <current page>"); None uses the document's last
    /// page at apply time (the CLI's append).
    pub source: Option<i64>,
    id: Option<i64>,
    /// The seeded setup cells captured at the first apply.
    seeded: Option<Vec<vsdx::model::SheetChild>>,
}

impl AddPage {
    pub fn new(name: impl Into<String>) -> AddPage {
        AddPage {
            name: name.into(),
            source: None,
            id: None,
            seeded: None,
        }
    }
    /// Seed the new page's setup from `source` (the GUI's active
    /// page).
    pub fn seeded_from(name: impl Into<String>, source: i64) -> AddPage {
        AddPage {
            name: name.into(),
            source: Some(source),
            id: None,
            seeded: None,
        }
    }
}

impl crate::Command for AddPage {
    fn label(&self) -> String {
        match self.id {
            Some(id) => format!("add page {id} {:?}", self.name),
            None => format!("add page {:?}", self.name),
        }
    }
    fn apply(&mut self, doc: &mut Document) -> Result<()> {
        // An EXPLICIT source must resolve — falling back to the last
        // page on a stale id would silently seed the wrong setup
        // (#68 review); the fallback belongs to `source: None` only.
        let src = match self.source {
            Some(id) => Some(
                doc.pages
                    .iter()
                    .find(|p| p.id == id)
                    .ok_or_else(|| anyhow::anyhow!("source page {id} not found"))?,
            ),
            None => None,
        };
        let mut page = new_page(doc, &self.name, src);
        if let Some(sheet) = page.children.iter_mut().find_map(|c| match c {
            IndexChild::Sheet { sheet, .. } => Some(sheet),
            _ => None,
        }) {
            match &self.seeded {
                // Redo: replay the first apply's setup exactly.
                Some(seeded) => sheet.children = seeded.clone(),
                None => self.seeded = Some(sheet.children.clone()),
            }
        }
        self.id = Some(page.id);
        doc.pages.push(page);
        Ok(())
    }
    fn undo(&mut self, doc: &mut Document) -> Result<()> {
        let id = self
            .id
            .ok_or_else(|| anyhow::anyhow!("page not yet applied"))?;
        let pos = doc.pages.iter().position(|p| p.id == id);
        match pos {
            Some(pos) => {
                doc.pages.remove(pos);
                Ok(())
            }
            None => bail!("page {id} vanished"),
        }
    }
}

/// Remove a page by ID, storing it (with its index position) for undo.
#[derive(Debug, Clone)]
pub struct RemovePage {
    pub page: i64,
    removed: Option<(Page, usize)>,
}

impl RemovePage {
    pub fn new(page: i64) -> RemovePage {
        RemovePage {
            page,
            removed: None,
        }
    }
}

impl crate::Command for RemovePage {
    fn label(&self) -> String {
        format!("remove page {}", self.page)
    }
    fn apply(&mut self, doc: &mut Document) -> Result<()> {
        let pos = doc
            .pages
            .iter()
            .position(|p| p.id == self.page)
            .ok_or_else(|| anyhow::anyhow!("page {} not found", self.page))?;
        self.removed = Some((doc.pages.remove(pos), pos));
        Ok(())
    }
    fn undo(&mut self, doc: &mut Document) -> Result<()> {
        let (page, pos) = self
            .removed
            .clone()
            .ok_or_else(|| anyhow::anyhow!("page not yet applied"))?;
        let at = pos.min(doc.pages.len());
        doc.pages.insert(at, page);
        Ok(())
    }
}

/// Rename a page (`NameU` and `Name` attributes plus the model field).
#[derive(Debug, Clone)]
pub struct RenamePage {
    pub page: i64,
    pub new_name: String,
    old: Option<(String, Attrs)>,
}

impl RenamePage {
    pub fn new(page: i64, new_name: impl Into<String>) -> RenamePage {
        RenamePage {
            page,
            new_name: new_name.into(),
            old: None,
        }
    }
}

impl crate::Command for RenamePage {
    fn label(&self) -> String {
        format!("rename page {} to {:?}", self.page, self.new_name)
    }
    fn apply(&mut self, doc: &mut Document) -> Result<()> {
        let p = doc
            .pages
            .iter_mut()
            .find(|p| p.id == self.page)
            .ok_or_else(|| anyhow::anyhow!("page {} not found", self.page))?;
        self.old = Some((p.name.clone(), p.attrs.clone()));
        for key in ["NameU", "Name"] {
            match p.attrs.iter_mut().find(|(k, _)| k == key) {
                Some(slot) => slot.1 = self.new_name.clone(),
                None => p.attrs.push((key.to_string(), self.new_name.clone())),
            }
        }
        p.name = self.new_name.clone();
        Ok(())
    }
    fn undo(&mut self, doc: &mut Document) -> Result<()> {
        let (name, attrs) = self
            .old
            .clone()
            .ok_or_else(|| anyhow::anyhow!("page not yet applied"))?;
        let p = doc
            .pages
            .iter_mut()
            .find(|p| p.id == self.page)
            .ok_or_else(|| anyhow::anyhow!("page {} not found", self.page))?;
        p.name = name;
        p.attrs = attrs;
        Ok(())
    }
}

/// Move a page (0-based positions; the pages index order is the only
/// thing that changes — relationships follow their pages).
#[derive(Debug, Clone)]
pub struct MovePage {
    pub from: usize,
    pub to: usize,
}

impl crate::Command for MovePage {
    fn label(&self) -> String {
        format!("move page {} → {}", self.from + 1, self.to + 1)
    }
    fn apply(&mut self, doc: &mut Document) -> Result<()> {
        move_page(doc, self.from, self.to)
    }
    fn undo(&mut self, doc: &mut Document) -> Result<()> {
        // remove(f).insert(t) is undone by the symmetric remove(t).insert(f):
        // the moved element sits at `to`, and re-inserting it at `from`
        // restores the original order.
        move_page(doc, self.to, self.from)
    }
}

fn move_page(doc: &mut Document, from: usize, to: usize) -> Result<()> {
    if from >= doc.pages.len() || to >= doc.pages.len() {
        bail!(
            "page position out of range (have {} pages): {from} → {to}",
            doc.pages.len()
        );
    }
    let page = doc.pages.remove(from);
    doc.pages.insert(to, page);
    Ok(())
}

/// Duplicate a page by ID (#54): a deep clone (shapes, sheet, index
/// children) inserted directly after the source, carrying a fresh page
/// ID, part name, and relationship ID so the writer materializes it as
/// its own part. The name is kept from the source; the caller renames
/// (Visio's UI assigns the next sequential name).
#[derive(Debug, Clone)]
pub struct DuplicatePage {
    pub page: i64,
    id: Option<i64>,
}

impl DuplicatePage {
    pub fn new(page: i64) -> DuplicatePage {
        DuplicatePage { page, id: None }
    }
}

impl crate::Command for DuplicatePage {
    fn label(&self) -> String {
        format!("duplicate page {}", self.page)
    }
    fn apply(&mut self, doc: &mut Document) -> Result<()> {
        let pos = doc
            .pages
            .iter()
            .position(|p| p.id == self.page)
            .ok_or_else(|| anyhow::anyhow!("page {} not found", self.page))?;
        // Fresh identity for the copy; allocations consider the doc
        // before insertion, exactly like AddPage.
        let id = next_page_id(doc);
        let rid = next_page_rid(doc);
        let mut copy = doc.pages[pos].clone();
        copy.id = id;
        copy.part = next_page_part(doc);
        match copy.attrs.iter_mut().find(|(k, _)| k == "ID") {
            Some(slot) => slot.1 = id.to_string(),
            None => copy.attrs.insert(0, ("ID".to_string(), id.to_string())),
        }
        for child in copy.children.iter_mut() {
            if let IndexChild::Rel(attrs) = child {
                for (k, v) in attrs.iter_mut() {
                    if k == "r:id" {
                        *v = rid.clone();
                    }
                }
            }
        }
        self.id = Some(id);
        doc.pages.insert(pos + 1, copy);
        Ok(())
    }
    fn undo(&mut self, doc: &mut Document) -> Result<()> {
        let id = self
            .id
            .ok_or_else(|| anyhow::anyhow!("page not yet applied"))?;
        match doc.pages.iter().position(|p| p.id == id) {
            Some(pos) => {
                doc.pages.remove(pos);
                Ok(())
            }
            None => bail!("duplicate page {id} vanished"),
        }
    }
}

// ---------------------------------------------------------------------------
// Connection commands
// ---------------------------------------------------------------------------

/// Add a connection record (glue) to a page or master's `<Connects>`.
#[derive(Debug, Clone)]
pub struct AddConnect {
    pub owner: ConnectsOwner,
    pub connect: Connect,
}

impl crate::Command for AddConnect {
    fn label(&self) -> String {
        format!(
            "connect {}:{} → {}:{} on {}",
            self.connect.from_sheet,
            self.connect.from_cell,
            self.connect.to_sheet,
            self.connect.to_cell,
            self.owner
        )
    }
    fn apply(&mut self, doc: &mut Document) -> Result<()> {
        let mut list = self.owner.read_connects_editable(doc)?;
        list.list.push(self.connect.clone());
        self.owner.host(doc)?.set_connects(&list);
        Ok(())
    }
    fn undo(&mut self, doc: &mut Document) -> Result<()> {
        let mut list = self.owner.read_connects_editable(doc)?;
        if let Some(pos) = list.list.iter().position(|c| c == &self.connect) {
            list.list.remove(pos);
        }
        self.owner.host(doc)?.set_connects(&list);
        Ok(())
    }
}

/// Remove the connection at `index` from a page or master's `<Connects>`.
#[derive(Debug, Clone)]
pub struct RemoveConnect {
    pub owner: ConnectsOwner,
    pub index: usize,
    removed: Option<Connect>,
}

impl RemoveConnect {
    pub fn new(owner: ConnectsOwner, index: usize) -> RemoveConnect {
        RemoveConnect {
            owner,
            index,
            removed: None,
        }
    }
}

impl crate::Command for RemoveConnect {
    fn label(&self) -> String {
        format!("remove connection {} from {}", self.index, self.owner)
    }
    fn apply(&mut self, doc: &mut Document) -> Result<()> {
        let mut list = self.owner.read_connects_editable(doc)?;
        if self.index >= list.list.len() {
            bail!(
                "connection index {} out of range ({} entries)",
                self.index,
                list.list.len()
            );
        }
        self.removed = Some(list.list.remove(self.index));
        self.owner.host(doc)?.set_connects(&list);
        Ok(())
    }
    fn undo(&mut self, doc: &mut Document) -> Result<()> {
        let removed = self
            .removed
            .clone()
            .ok_or_else(|| anyhow::anyhow!("not yet applied"))?;
        let mut list = self.owner.read_connects_editable(doc)?;
        let at = self.index.min(list.list.len());
        list.list.insert(at, removed);
        self.owner.host(doc)?.set_connects(&list);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// ShapeSheet Actions execution (GitHub #25)
// ---------------------------------------------------------------------------

/// Split a formula on top-level `&` (Visio's action chaining),
/// respecting parentheses and double-quoted strings.
fn split_top_level_amp(expr: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut in_str = false;
    let mut cur = String::new();
    for ch in expr.chars() {
        match ch {
            '"' => {
                in_str = !in_str;
                cur.push(ch);
            }
            '(' if !in_str => {
                depth += 1;
                cur.push(ch);
            }
            ')' if !in_str => {
                depth -= 1;
                cur.push(ch);
            }
            '&' if !in_str && depth == 0 => {
                out.push(cur.clone());
                cur.clear();
            }
            _ => cur.push(ch),
        }
    }
    out.push(cur);
    out
}

/// Format a numeric cell value the way Visio writes flags and small
/// numbers: integral → no decimal point.
fn fmt_num(v: f64) -> String {
    if (v - v.round()).abs() < 1e-9 {
        format!("{}", v.round() as i64)
    } else {
        format!("{v}")
    }
}

/// Build the commands for one ShapeSheet `Actions` row's `Action`
/// formula (GitHub #25): a `SETF(GetRef(target), value)` chain, each
/// term becoming one `SetCell`/`SetRowCell` against `scope` (the
/// instance — masters are never mutated). Values evaluate against
/// `env` (the instance's effective cells). `No Formula` and empty
/// actions return an empty vec (display-only rows); anything else
/// (RUNADDON, RUNMACRO, …) returns `Err` so the caller can defer with
/// a status note.
pub fn shape_action_commands(
    scope: Scope,
    formula: &str,
    env: &vsdx::formula::Env,
) -> Result<Vec<Box<dyn crate::Command>>, String> {
    let f = formula.trim();
    if f.is_empty() || f.eq_ignore_ascii_case("no formula") {
        return Ok(Vec::new());
    }
    let mut cmds: Vec<Box<dyn crate::Command>> = Vec::new();
    for term in split_top_level_amp(f) {
        let term = term.trim();
        let Some(inner) = term
            .strip_prefix("SETF(")
            .or_else(|| term.strip_prefix("setf("))
            .and_then(|rest| rest.strip_suffix(')'))
        else {
            return Err(format!("unsupported action term {term:?}"));
        };
        // First top-level comma splits target from value; the target
        // is GetRef(<cell>) or a bare cell name.
        let inner = inner.trim();
        let (first, rest) = split_top_level_comma(inner);
        let Some(value_str) = rest else {
            return Err(format!("action term has no value: {term:?}"));
        };
        let first = first.trim();
        let target = if let Some(g) = first
            .strip_prefix("GetRef(")
            .or_else(|| first.strip_prefix("getref("))
            .and_then(|rest| rest.strip_suffix(')'))
        {
            g.trim().to_string()
        } else {
            first.to_string()
        };
        let value = match vsdx::formula::eval(value_str.trim(), env) {
            Ok(vsdx::formula::Value::Num(n)) if n.is_finite() => fmt_num(n),
            Ok(vsdx::formula::Value::Str(s)) => s,
            _ => {
                // Only a plain quoted string literal may be stored as a
                // value (quotes stripped); anything else would persist
                // formula text into the cell's V — defer instead.
                let t = value_str.trim();
                t.strip_prefix('"')
                    .and_then(|r| r.strip_suffix('"'))
                    .map(str::to_string)
                    .ok_or_else(|| format!("unsupported action value {t:?}"))?
            }
        };
        let cell = vsdx::model::Cell::value(value.clone());
        // Map the reference spelling to a command. Named-row sections:
        // User/Prop/Scratch — the bare row name (`User.Style`) is the
        // row's Value cell; a trailing cell suffix (`User.Style.Prompt`)
        // targets that row's named cell. Row-0 sections: Para→Paragraph,
        // Char→Character. Anything else is a plain sheet cell
        // (VerticalAlign, Width, …); a dotted or sheet-qualified target
        // (`Actions.Menu`, `ThePage!PageWidth`) no branch maps defers
        // instead of writing a bogus cell name.
        let named_target = |rest: &str| -> Result<(String, String), String> {
            // Row names carry no dots: `Row.Cell`, defaulting the cell
            // to Value. Anything else is not a row we can name.
            match rest.rsplit_once('.') {
                Some((row, cellname)) => {
                    if row.is_empty() || cellname.is_empty() || row.contains('.') {
                        Err(format!("unsupported action target {target:?}"))
                    } else {
                        Ok((row.to_string(), cellname.to_string()))
                    }
                }
                None => Ok((rest.to_string(), "Value".to_string())),
            }
        };
        let cmd: Box<dyn crate::Command> = (|| -> Result<Box<dyn crate::Command>, String> {
            if let Some(rest) = target.strip_prefix("User.") {
                let (row, cellname) = named_target(rest)?;
                Ok(Box::new(SetRowCell::new(
                    scope.clone(),
                    "User",
                    0,
                    RowSel::Named(row),
                    cellname,
                    cell,
                )) as Box<dyn crate::Command>)
            } else if let Some(rest) = target.strip_prefix("Prop.") {
                let (row, cellname) = named_target(rest)?;
                Ok(Box::new(SetRowCell::new(
                    scope.clone(),
                    "Property",
                    0,
                    RowSel::Named(row),
                    cellname,
                    cell,
                )))
            } else if let Some(rest) = target.strip_prefix("Scratch.") {
                let (row, cellname) = named_target(rest)?;
                Ok(Box::new(SetRowCell::new(
                    scope.clone(),
                    "Scratch",
                    0,
                    RowSel::Named(row),
                    cellname,
                    cell,
                )))
            } else if let Some(cellname) = target.strip_prefix("Para.") {
                Ok(Box::new(SetRowCell::new(
                    scope.clone(),
                    "Paragraph",
                    0,
                    RowSel::Ix(0),
                    cellname.to_string(),
                    cell,
                )))
            } else if let Some(cellname) = target.strip_prefix("Char.") {
                Ok(Box::new(SetRowCell::new(
                    scope.clone(),
                    "Character",
                    0,
                    RowSel::Ix(0),
                    cellname.to_string(),
                    cell,
                )))
            } else if target.contains('.') || target.contains('!') {
                Err(format!("unsupported action target {target:?}"))
            } else {
                Ok(Box::new(SetCell::new(scope.clone(), target.clone(), cell)))
            }
        })()?;
        cmds.push(cmd);
    }
    Ok(cmds)
}

/// Split `a,b` on the first top-level comma (parens/quotes respected).
fn split_top_level_comma(s: &str) -> (String, Option<&str>) {
    let mut depth = 0i32;
    let mut in_str = false;
    for (i, ch) in s.char_indices() {
        match ch {
            '"' => in_str = !in_str,
            '(' if !in_str => depth += 1,
            ')' if !in_str => depth -= 1,
            ',' if !in_str && depth == 0 => {
                return (s[..i].to_string(), Some(&s[i + 1..]));
            }
            _ => {}
        }
    }
    (s.to_string(), None)
}

// ---------------------------------------------------------------------------
// Layer commands (live in layers.rs)
// ---------------------------------------------------------------------------

pub use crate::layers::{AddLayer, RemoveLayer, RenameLayer};

// ---------------------------------------------------------------------------
// Composite command
// ---------------------------------------------------------------------------

/// A named group of commands applied as one undo step (Visio's macro
/// analog): a multi-shape move, a resize (four cell edits), a marquee
/// delete.
pub struct Macro {
    pub label: String,
    pub cmds: Vec<Box<dyn crate::Command>>,
}

impl Macro {
    pub fn new(label: impl Into<String>, cmds: Vec<Box<dyn crate::Command>>) -> Macro {
        Macro {
            label: label.into(),
            cmds,
        }
    }
}

impl crate::Command for Macro {

    fn touched(&self, doc: &Document) -> crate::TouchList {
        let mut all = crate::TouchList::none();
        for cmd in &self.cmds {
            all.merge(cmd.touched(doc));
        }
        all
    }

    fn label(&self) -> String {
        self.label.clone()
    }
    fn apply(&mut self, doc: &mut Document) -> Result<()> {
        // Transactional: a failing child rolls back the children that
        // already ran (reverse order), so no partial changes stick
        // outside the undo history.
        for i in 0..self.cmds.len() {
            if let Err(e) = self.cmds[i].apply(doc) {
                for c in self.cmds[..i].iter_mut().rev() {
                    let _ = c.undo(doc);
                }
                return Err(e);
            }
        }
        Ok(())
    }
    fn undo(&mut self, doc: &mut Document) -> Result<()> {
        // Reverse order: later commands may depend on earlier state.
        for c in self.cmds.iter_mut().rev() {
            c.undo(doc)?;
        }
        Ok(())
    }
}


/// The top-level shape a sub-shape index path lives under: `path[0]`
/// indexes the page's top-level list (the same convention RemoveShape
/// captures).
fn path_root(page: &vsdx::model::Page, path: &[usize]) -> i64 {
    page.shapes
        .get(path[0])
        .map(|s| s.id)
        .unwrap_or(-1)
}
