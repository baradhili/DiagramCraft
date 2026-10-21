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
    let idx = sheet.children.iter().position(|c| matches!(c, vsdx::model::SheetChild::Cell(n, _) if n == name))?;
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
            vsdx::model::SheetChild::Section(s) if s.name == section && s.ix == section_ix => Some(s),
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
            vsdx::model::SheetChild::Section(s) if s.name == section && s.ix == section_ix => Some(s),
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
            vsdx::model::SheetChild::Section(s) if s.name == section && s.ix == section_ix => Some(s),
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
        SetCell { scope, name: name.into(), new, old: None }
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
            let Scope::Shape { page: page_id, shape: shape_id } = &self.scope else {
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
        set_row_cell(s, &self.section, self.section_ix, &self.row, &self.name, self.new.clone())?;
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
                    RowSel::Named(n) => {
                        sec.rows.iter().position(|r| r.n.as_deref() == Some(n.as_str()))
                    }
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
        SetText { scope, text: text.into(), old: None }
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
            self.parent.map(|p| format!(" group:{p}")).unwrap_or_default()
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
                let group = find_shape_mut(&mut page.shapes, pid)
                    .ok_or_else(|| anyhow::anyhow!("group shape {pid} not found on page {}", self.page))?;
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

/// Remove a shape (any depth), storing it with its position for undo.
#[derive(Debug, Clone)]
pub struct RemoveShape {
    pub page: i64,
    pub shape: i64,
    removed: Option<(Shape, Vec<usize>)>, // (shape, index path)
}

impl RemoveShape {
    pub fn new(page: i64, shape: i64) -> RemoveShape {
        RemoveShape { page, shape, removed: None }
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
        DuplicateShape { page, shape, offset, clone: None }
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
        ReorderShape { page, shape, to_front, old_index: None }
    }
}

impl crate::Command for ReorderShape {
    fn label(&self) -> String {
        format!(
            "{} shape {} on page:{}",
            if self.to_front { "bring to front" } else { "send to back" },
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
            .ok_or_else(|| anyhow::anyhow!("shape {} not found on page {}", self.shape, self.page))?;
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
    doc.pages.iter().map(|p| p.id).max().map(|m| m + 1).unwrap_or(0)
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
            if name.starts_with("visio/pages/") && name.ends_with(".xml") && !name.contains("_rels") {
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
            IndexChild::Rel(vec![(
                "r:id".to_string(),
                next_page_rid(doc),
            )]),
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
        AddPage { name: name.into(), id: None }
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
        let id = self.id.ok_or_else(|| anyhow::anyhow!("page not yet applied"))?;
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
        RemovePage { page, removed: None }
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
        RenamePage { page, new_name: new_name.into(), old: None }
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
        let (name, attrs) = self.old.clone().ok_or_else(|| anyhow::anyhow!("page not yet applied"))?;
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
        RemoveConnect { owner, index, removed: None }
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
        let removed = self.removed.clone().ok_or_else(|| anyhow::anyhow!("not yet applied"))?;
        let mut list = self.owner.read_connects_editable(doc)?;
        let at = self.index.min(list.list.len());
        list.list.insert(at, removed);
        self.owner.host(doc)?.set_connects(&list);
        Ok(())
    }
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
        Macro { label: label.into(), cmds }
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
