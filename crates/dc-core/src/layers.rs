//! Layers — typed access to a page sheet's `<Section N='Layer'>` rows
//! and edit commands over them.
//!
//! Corpus shape (stencil page sheets): rows indexed by `IX`, each with
//! cells `Name`, `Color`, `Status`, `Visible`, `Print`, `Active`, `Lock`,
//! `Snap`, `Glue`, `NameUniv`, `ColorTrans`.

use anyhow::{Context, Result};
use vsdx::model::{Cell, Document, Row, Section};

use crate::scope::{sheet_mut, Scope};

/// Read the layer rows of a page sheet (by page ID), in section order.
pub fn layer_rows(doc: &Document, page: i64) -> Result<Vec<Row>> {
    let p = doc
        .pages
        .iter()
        .find(|p| p.id == page)
        .ok_or_else(|| anyhow::anyhow!("page {page} not found"))?;
    let sheet = p.sheet().ok_or_else(|| anyhow::anyhow!("page {page} has no PageSheet"))?;
    Ok(sheet
        .sections_named("Layer")
        .into_iter()
        .flat_map(|s| s.rows.iter().cloned())
        .collect())
}

/// The layer's display name (the `Name` cell).
pub fn layer_name(row: &Row) -> Option<&str> {
    row.cell("Name").and_then(|c| c.v.as_deref())
}

fn layer_section_mut<'a>(
    doc: &'a mut Document,
    page: i64,
) -> Result<(&'a mut Section, bool)> {
    let sheet = sheet_mut(doc, &Scope::PageSheet { page })?;
    if let Some(i) = sheet.children.iter().position(|c| {
        matches!(c, vsdx::model::SheetChild::Section(s) if s.name == "Layer")
    }) {
        if let vsdx::model::SheetChild::Section(s) = &mut sheet.children[i] {
            return Ok((s, false));
        }
        unreachable!();
    }
    // Create the Layer section after existing cells/sections (end of body).
    sheet.children.push(vsdx::model::SheetChild::Section(Section {
        name: "Layer".to_string(),
        ix: 0,
        attrs: vec![("N".to_string(), "Layer".to_string())],
        cells: Vec::new(),
        rows: Vec::new(),
    }));
    let last = sheet.children.len() - 1;
    if let vsdx::model::SheetChild::Section(s) = &mut sheet.children[last] {
        Ok((s, true))
    } else {
        unreachable!()
    }
}

fn layer_row(ix: i64, name: &str) -> Row {
    let mut cells = Vec::new();
    let mut push = |n: &str, v: &str| cells.push((n.to_string(), Cell::value(v)));
    push("Name", name);
    push("Color", "255");
    push("Status", "0");
    push("Visible", "1");
    push("Print", "1");
    push("Active", "0");
    push("Lock", "0");
    push("Snap", "1");
    push("Glue", "1");
    push("NameUniv", name);
    push("ColorTrans", "0");
    Row {
        t: None,
        ix,
        n: None,
        attrs: if ix != 0 { vec![("IX".to_string(), ix.to_string())] } else { Vec::new() },
        cells,
    }
}

/// Append a layer row to a page sheet (undo removes it, and the section
/// too when the command created it).
#[derive(Debug, Clone)]
pub struct AddLayer {
    pub page: i64,
    pub name: String,
    ix: Option<i64>,
    created_section: bool,
}

impl AddLayer {
    pub fn new(page: i64, name: impl Into<String>) -> AddLayer {
        AddLayer { page, name: name.into(), ix: None, created_section: false }
    }
}

impl crate::Command for AddLayer {
    fn label(&self) -> String {
        format!("add layer {:?} to page:{}", self.name, self.page)
    }
    fn apply(&mut self, doc: &mut Document) -> Result<()> {
        let (sec, created) = layer_section_mut(doc, self.page)?;
        self.created_section = created;
        let ix = sec.rows.iter().map(|r| r.ix).max().map(|m| m + 1).unwrap_or(0);
        sec.rows.push(layer_row(ix, &self.name));
        self.ix = Some(ix);
        Ok(())
    }
    fn undo(&mut self, doc: &mut Document) -> Result<()> {
        let ix = self.ix.context("layer not yet applied")?;
        let (sec, _) = layer_section_mut(doc, self.page)?;
        if let Some(pos) = sec.rows.iter().position(|r| r.ix == ix) {
            sec.rows.remove(pos);
        }
        if self.created_section && sec.rows.is_empty() {
            let sheet = sheet_mut(doc, &Scope::PageSheet { page: self.page })?;
            sheet
                .children
                .retain(|c| !matches!(c, vsdx::model::SheetChild::Section(s) if s.name == "Layer"));
        }
        Ok(())
    }
}

/// Remove the layer row with the given `IX`.
#[derive(Debug, Clone)]
pub struct RemoveLayer {
    pub page: i64,
    pub ix: i64,
    removed: Option<(Row, usize)>,
}

impl RemoveLayer {
    pub fn new(page: i64, ix: i64) -> RemoveLayer {
        RemoveLayer { page, ix, removed: None }
    }
}

impl crate::Command for RemoveLayer {
    fn label(&self) -> String {
        format!("remove layer {} from page:{}", self.ix, self.page)
    }
    fn apply(&mut self, doc: &mut Document) -> Result<()> {
        let (sec, _) = layer_section_mut(doc, self.page)?;
        let pos = sec
            .rows
            .iter()
            .position(|r| r.ix == self.ix)
            .ok_or_else(|| anyhow::anyhow!("layer {0} not found on page:{1}", self.ix, self.page))?;
        self.removed = Some((sec.rows.remove(pos), pos));
        Ok(())
    }
    fn undo(&mut self, doc: &mut Document) -> Result<()> {
        let (row, pos) = self.removed.clone().context("layer not yet applied")?;
        let (sec, _) = layer_section_mut(doc, self.page)?;
        let at = pos.min(sec.rows.len());
        sec.rows.insert(at, row);
        Ok(())
    }
}

/// Rename a layer (sets both `Name` and `NameUniv`).
#[derive(Debug, Clone)]
pub struct RenameLayer {
    pub page: i64,
    pub ix: i64,
    pub new_name: String,
    old: Option<(Option<String>, Option<String>)>,
}

impl RenameLayer {
    pub fn new(page: i64, ix: i64, new_name: impl Into<String>) -> RenameLayer {
        RenameLayer { page, ix, new_name: new_name.into(), old: None }
    }
}

impl crate::Command for RenameLayer {
    fn label(&self) -> String {
        format!("rename layer {} on page:{} to {:?}", self.ix, self.page, self.new_name)
    }
    fn apply(&mut self, doc: &mut Document) -> Result<()> {
        let (sec, _) = layer_section_mut(doc, self.page)?;
        let row = sec
            .rows
            .iter_mut()
            .find(|r| r.ix == self.ix)
            .ok_or_else(|| anyhow::anyhow!("layer {0} not found on page:{1}", self.ix, self.page))?;
        self.old = Some((
            row.cell("Name").and_then(|c| c.v.clone()),
            row.cell("NameUniv").and_then(|c| c.v.clone()),
        ));
        for key in ["Name", "NameUniv"] {
            match row.cells.iter_mut().find(|(n, _)| n == key) {
                Some((_, slot)) => *slot = Cell::value(self.new_name.clone()),
                None => row.cells.push((key.to_string(), Cell::value(self.new_name.clone()))),
            }
        }
        Ok(())
    }
    fn undo(&mut self, doc: &mut Document) -> Result<()> {
        let (name, name_univ) = self.old.clone().context("layer not yet applied")?;
        let (sec, _) = layer_section_mut(doc, self.page)?;
        let row = sec
            .rows
            .iter_mut()
            .find(|r| r.ix == self.ix)
            .ok_or_else(|| anyhow::anyhow!("layer {0} vanished", self.ix))?;
        for (key, val) in [("Name", name), ("NameUniv", name_univ)] {
            let pos = row.cells.iter().position(|(n, _)| n == key);
            match (pos, val) {
                (Some(p), Some(v)) => row.cells[p].1 = Cell::value(v),
                (Some(p), None) => {
                    row.cells.remove(p);
                }
                (None, Some(v)) => row.cells.push((key.to_string(), Cell::value(v))),
                (None, None) => {}
            }
        }
        Ok(())
    }
}
