//! `dc edit` — edit a document through the dc-core command layer.
//!
//! Every edit flag maps to one undoable command; they apply in a fixed
//! order (cells → formulas → text → shapes → pages → layers → connects),
//! then the result is saved through the lossless write path. `--undo`
//! undoes the most recent command before saving (proof the history is
//! live); `--history` prints the applied labels.

use anyhow::{bail, Context as _, Result};
use dc_core::commands::{
    AddConnect, AddLayer, AddPage, AddShape, MovePage, RemoveConnect, RemoveLayer, RemovePage,
    RemoveShape, RenameLayer, RenamePage, SetCell, SetText,
};
use dc_core::connects::Connect;
use dc_core::{Editor, Scope};
use std::path::Path;
use vsdx::model::{Attrs, Cell, Shape};
use vsdx::Document;

pub struct EditArgs {
    pub cell: Vec<(String, String, String)>, // target, name, value
    pub formula: Vec<(String, String, String)>, // target, name, formula
    pub text: Vec<(String, String)>,         // target(shape), text
    pub add_shape: Vec<i64>,                 // page ids
    pub remove_shape: Vec<(i64, i64)>,       // page, shape
    pub add_page: Vec<String>,               // names
    pub remove_page: Vec<i64>,               // ids
    pub rename_page: Vec<(i64, String)>,     // id, new name
    pub move_page: Vec<(usize, usize)>,      // 1-based positions
    pub add_layer: Vec<(i64, String)>,       // page, name
    pub remove_layer: Vec<(i64, i64)>,       // page, ix
    pub rename_layer: Vec<(i64, i64, String)>, // page, ix, name
    pub connect: Vec<ConnectSpec>,
    pub disconnect: Vec<(dc_core::connects::ConnectsOwner, i64)>, // owner, index
    pub undo_count: u8,
    pub print_history: bool,
}

pub struct ConnectSpec {
    pub owner: dc_core::connects::ConnectsOwner,
    pub from_sheet: i64,
    pub from_cell: String,
    pub from_part: i64,
    pub to_sheet: i64,
    pub to_cell: String,
    pub to_part: i64,
}

/// `page:3`, `master:7`, or a bare page ID.
pub fn connect_owner(s: &str) -> Result<dc_core::connects::ConnectsOwner> {
    if let Some(id) = s.strip_prefix("master:") {
        return Ok(dc_core::connects::ConnectsOwner::Master(id.parse()?));
    }
    if let Some(id) = s.strip_prefix("page:") {
        return Ok(dc_core::connects::ConnectsOwner::Page(id.parse()?));
    }
    Ok(dc_core::connects::ConnectsOwner::Page(s.parse()?))
}

pub fn run(file: &Path, out: &Path, args: EditArgs) -> Result<()> {
    let mut ed = Editor::open(file).with_context(|| format!("opening {}", file.display()))?;
    apply_all(&mut ed, &args)?;
    for _ in 0..args.undo_count {
        match ed.undo()? {
            Some((label, _touched)) => println!("undid: {label}"),
            None => bail!("--undo: nothing left to undo"),
        }
    }
    ed.save_as(out)?;
    if args.print_history {
        println!("history (oldest first):");
        for l in ed.history.undo_labels() {
            println!("  ✓ {l}");
        }
        for l in ed.history.redo_labels() {
            println!("  ↺ {l}");
        }
    }
    println!("wrote {}", out.display());
    Ok(())
}

fn apply_all(ed: &mut Editor, args: &EditArgs) -> Result<()> {
    for (target, name, value) in &args.cell {
        let scope = Scope::parse(target)?;
        ed.apply(SetCell::new(
            scope,
            name.clone(),
            Cell::value(value.clone()),
        ))?;
    }
    for (target, name, formula) in &args.formula {
        let scope = Scope::parse(target)?;
        ed.apply(SetCell::new(
            scope,
            name.clone(),
            Cell {
                f: Some(formula.clone()),
                ..Cell::default()
            },
        ))?;
    }
    for (target, text) in &args.text {
        let scope = Scope::parse(target)?;
        ed.apply(SetText::new(scope, text.clone()))?;
    }
    for page in &args.add_shape {
        let shape = placeholder_shape(&ed.doc, *page)?;
        ed.apply(AddShape {
            page: *page,
            parent: None,
            shape,
        })?;
    }
    for (page, shape) in &args.remove_shape {
        ed.apply(RemoveShape::new(*page, *shape))?;
    }
    for name in &args.add_page {
        ed.apply(AddPage::new(name.clone()))?;
    }
    for id in &args.remove_page {
        ed.apply(RemovePage::new(*id))?;
    }
    for (id, name) in &args.rename_page {
        ed.apply(RenamePage::new(*id, name.clone()))?;
    }
    for (from, to) in &args.move_page {
        ed.apply(MovePage {
            from: from - 1,
            to: to - 1,
        })?;
    }
    for (page, name) in &args.add_layer {
        ed.apply(AddLayer::new(*page, name.clone()))?;
    }
    for (page, ix) in &args.remove_layer {
        ed.apply(RemoveLayer::new(*page, *ix))?;
    }
    for (page, ix, name) in &args.rename_layer {
        ed.apply(RenameLayer::new(*page, *ix, name.clone()))?;
    }
    for c in &args.connect {
        ed.apply(AddConnect {
            owner: c.owner.clone(),
            connect: Connect::new(
                c.from_sheet,
                c.from_cell.clone(),
                c.from_part,
                c.to_sheet,
                c.to_cell.clone(),
                c.to_part,
            ),
        })?;
    }
    for (owner, index) in &args.disconnect {
        ed.apply(RemoveConnect::new(owner.clone(), *index as usize))?;
    }
    Ok(())
}

/// A plain rectangle-less shape pinned at the page center, half-page
/// size — enough to demonstrate add/remove and to appear in `inspect`.
fn placeholder_shape(doc: &Document, page: i64) -> Result<Shape> {
    let p = doc
        .pages
        .iter()
        .find(|p| p.id == page)
        .ok_or_else(|| anyhow::anyhow!("page {page} not found"))?;
    let cell = |n: &str| -> f64 {
        p.sheet()
            .and_then(|s| s.cell(n))
            .and_then(|c| c.numeric())
            .unwrap_or(4.25)
    };
    let (w, h) = (cell("PageWidth"), cell("PageHeight"));
    let id = dc_core::commands::next_shape_id(doc, page);
    let mut sheet = vsdx::model::Sheet::default();
    for (n, v) in [
        ("PinX", format!("{:.6}", w / 2.0)),
        ("PinY", format!("{:.6}", h / 2.0)),
        ("Width", format!("{:.6}", w / 2.0)),
        ("Height", format!("{:.6}", h / 2.0)),
        ("LocPinX", format!("{:.6}", w / 4.0)),
        ("LocPinY", format!("{:.6}", h / 4.0)),
    ] {
        sheet.set_cell(n, Cell::value(v));
    }
    let attrs: Attrs = vec![
        ("ID".to_string(), id.to_string()),
        ("Type".to_string(), "Shape".to_string()),
        ("LineStyle".to_string(), "0".to_string()),
        ("FillStyle".to_string(), "0".to_string()),
        ("TextStyle".to_string(), "0".to_string()),
    ];
    Ok(Shape {
        id,
        type_: Some("Shape".to_string()),
        line_style: Some(0),
        fill_style: Some(0),
        text_style: Some(0),
        attrs,
        sheet,
        ..Shape::default()
    })
}
