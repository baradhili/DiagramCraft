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
    fn label(&self) -> String {
        format!("set {}:{} = {:?}", self.scope, self.name, self.new.v)
    }
    fn apply(&mut self, doc: &mut Document) -> Result<()> {
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
    set(&mut sheet, "TxtPinX", "0.2");
    set(&mut sheet, "TxtLocPinX", "0");
    sheet.set_cell(
        "TxtPinY",
        vsdx::model::Cell {
            v: None,
            f: Some("Height-TxtHeight".into()),
            ..vsdx::model::Cell::default()
        },
    );
    set(&mut sheet, "TxtLocPinY", "0");
    set(&mut sheet, "FillForegnd", "#FFFFFF");
    let row = |ix: i64, t: &str, x: &str, y: &str| vsdx::model::Row {
        ix,
        t: Some(t.into()),
        cells: vec![
            ("X".into(), vsdx::model::Cell::value(x.to_string())),
            ("Y".into(), vsdx::model::Cell::value(y.to_string())),
        ],
        ..vsdx::model::Row::default()
    };
    let (w, h) = (fmt_num(rect.2), fmt_num(rect.3));
    let container = Shape {
        id,
        name_u: Some(format!("Container.{id}")),
        type_: Some("Shape".into()),
        sheet,
        ..Shape::default()
    };
    let mut container = container;
    container.sheet.text = Some(heading.to_string());
    // The writer emits <Text> from text_raw (verbatim), so seed the
    // escaped form too — the SetText convention.
    container.sheet.text_raw = Some(
        heading
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;"),
    );
    container
        .sheet
        .children
        .push(vsdx::model::SheetChild::Section(vsdx::model::Section {
            name: "Geometry".into(),
            ix: 0,
            rows: vec![
                row(1, "MoveTo", "0", "0"),
                row(2, "LineTo", &w, "0"),
                row(3, "LineTo", &w, &h),
                row(4, "LineTo", "0", &h),
                row(5, "LineTo", "0", "0"),
            ],
            ..vsdx::model::Section::default()
        }));
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

/// Undo payload for [`GroupShapes`]: member id, original z-position,
/// and the pre-group PinX/PinY cells.
type GroupUndo = Vec<(
    i64,
    usize,
    Option<vsdx::model::Cell>,
    Option<vsdx::model::Cell>,
)>;

/// Group top-level shapes into one `Type='Group'` shape (GitHub #26).
/// The group's XForm is the members' union bbox with `LocPin` at its
/// bottom-left, and each member's Pin is rewritten into the group's
/// local frame — the renderer places sub-shapes relative to the
/// group's LocPin origin, so the drawing renders identically and the
/// members follow the group's moves.
#[derive(Debug, Clone)]
pub struct GroupShapes {
    pub page: i64,
    /// Member ids, any order; the group preserves ascending z-order.
    pub members: Vec<i64>,
    id: Option<i64>,
    /// (member id, original z-position, old PinX/PinY cells) for undo.
    undo: Option<GroupUndo>,
}

impl GroupShapes {
    pub fn new(page: i64, members: Vec<i64>) -> GroupShapes {
        GroupShapes {
            page,
            members,
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
        if found.len() < 2 {
            bail!("grouping needs at least two member shapes");
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
        // Rewrite members' Pins into the group frame and detach.
        let mut members: Vec<Shape> = Vec::new();
        for (pos, id) in &found {
            let s = doc.pages[pi].shapes.iter().find(|s| s.id == *id).unwrap();
            let (px, py) = (num(s, "PinX"), num(s, "PinY"));
            undo.push((
                *id,
                *pos,
                s.sheet.cell("PinX").cloned(),
                s.sheet.cell("PinY").cloned(),
            ));
            let mut m = s.clone();
            m.sheet
                .set_cell("PinX", vsdx::model::Cell::value(fmt_num(px - x0)));
            m.sheet
                .set_cell("PinY", vsdx::model::Cell::value(fmt_num(py - y0)));
            members.push(m);
        }
        self.undo = Some(undo);
        // Remove back-to-front so earlier positions stay valid.
        for (pos, _) in found.iter().rev() {
            doc.pages[pi].shapes.remove(*pos);
        }
        let group = Shape {
            id: gid,
            name_u: Some(format!("Group.{gid}")),
            type_: Some("Group".into()),
            sheet: {
                let mut sh = vsdx::model::Sheet::default();
                sh.set_cell("PinX", vsdx::model::Cell::value(fmt_num(x0)));
                sh.set_cell("PinY", vsdx::model::Cell::value(fmt_num(y0)));
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
        let at = found.first().unwrap().0.min(doc.pages[pi].shapes.len());
        doc.pages[pi].shapes.insert(at, group);
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
        let gpos = doc.pages[pi]
            .shapes
            .iter()
            .position(|s| s.id == gid)
            .ok_or_else(|| anyhow::anyhow!("group {gid} vanished"))?;
        let mut extracted = doc.pages[pi].shapes.remove(gpos).sub_shapes;
        // Restore original absolute Pins and z-positions.
        for (id, pos, old_x, old_y) in &undo {
            let m = extracted
                .iter_mut()
                .find(|s| s.id == *id)
                .ok_or_else(|| anyhow::anyhow!("member {id} vanished from group"))?;
            let restore = |m: &mut Shape, n: &str, old: &Option<vsdx::model::Cell>| match old {
                Some(c) => m.sheet.set_cell(n, c.clone()),
                None => m
                    .sheet
                    .children
                    .retain(|c| !matches!(c, vsdx::model::SheetChild::Cell(k, _) if k == n)),
            };
            restore(m, "PinX", old_x);
            restore(m, "PinY", old_y);
            let at = (*pos).min(doc.pages[pi].shapes.len());
            doc.pages[pi].shapes.insert(at, m.clone());
        }
        Ok(())
    }
}

/// Dissolve a group one level (GitHub #26): members are promoted to
/// the group's z-position with their Pins translated back to absolute
/// page coordinates using the group's live Xform.
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
        let (gx, gy) = {
            let g = &doc.pages[pi].shapes[pos];
            (
                num(g, "PinX") - num(g, "LocPinX"),
                num(g, "PinY") - num(g, "LocPinY"),
            )
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
            m.sheet
                .set_cell("PinX", vsdx::model::Cell::value(fmt_num(px + gx)));
            m.sheet
                .set_cell("PinY", vsdx::model::Cell::value(fmt_num(py + gy)));
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
pub fn new_page(doc: &Document, name: &str) -> Page {
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
/// allocated at apply time (and captured, so redo reuses them).
#[derive(Debug, Clone)]
pub struct AddPage {
    pub name: String,
    id: Option<i64>,
}

impl AddPage {
    pub fn new(name: impl Into<String>) -> AddPage {
        AddPage {
            name: name.into(),
            id: None,
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
        let page = new_page(doc, &self.name);
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
