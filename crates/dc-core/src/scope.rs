//! Sheet addressing — the ShapeSheet inspector surface (the
//! `IVSection`/`CCell` analog's navigation layer).
//!
//! A [`Scope`] names one sheet of the document. Its string form is used by
//! the CLI and tests:
//! - `doc` — the document sheet
//! - `page:3` — page 3's `PageSheet` (page **ID**, not position)
//! - `page:3/shape:7` — shape 7 on page 3 (nested shapes resolve by ID)
//! - `style:2` — stylesheet 2
//! - `master:7` — master 7's index sheet
//! - `master:7/shape:2` — shape 2 of master 7's shape tree

use anyhow::{bail, Result};
use vsdx::model::{Document, IndexChild, Sheet, Shape};

/// One addressed sheet.
#[derive(Debug, Clone, PartialEq)]
pub enum Scope {
    Document,
    PageSheet { page: i64 },
    Shape { page: i64, shape: i64 },
    StyleSheet { id: i64 },
    Master { master: i64, shape: Option<i64> },
}

impl Scope {
    pub fn parse(s: &str) -> Result<Scope> {
        let s = s.trim();
        let bad = || anyhow::anyhow!("unrecognized target {s:?} — use doc | page:N | page:N/shape:M | style:N | master:N[/shape:M]");
        let (head, tail) = match s.split_once('/') {
            Some((h, t)) => (h, Some(t)),
            None => (s, None),
        };
        let (kind, id) = match head.split_once(':') {
            Some((k, v)) => {
                let id: i64 = v.parse().map_err(|_| bad())?;
                (k, id)
            }
            None => {
                return if s == "doc" || s == "document" {
                    Ok(Scope::Document)
                } else {
                    Err(bad())
                }
            }
        };
        let shape_of = |tail: &str| -> Result<i64> {
            let (k, v) = tail.split_once(':').ok_or_else(|| bad())?;
            if k != "shape" {
                return Err(bad());
            }
            v.parse::<i64>().map_err(|_| bad())
        };
        match (kind, tail) {
            ("page", Some(t)) => Ok(Scope::Shape { page: id, shape: shape_of(t)? }),
            ("page", None) => Ok(Scope::PageSheet { page: id }),
            ("style", None) => Ok(Scope::StyleSheet { id }),
            ("master", Some(t)) => Ok(Scope::Master { master: id, shape: Some(shape_of(t)?) }),
            ("master", None) => Ok(Scope::Master { master: id, shape: None }),
            _ => Err(bad()),
        }
    }
}

impl std::fmt::Display for Scope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Scope::Document => write!(f, "doc"),
            Scope::PageSheet { page } => write!(f, "page:{page}"),
            Scope::Shape { page, shape } => write!(f, "page:{page}/shape:{shape}"),
            Scope::StyleSheet { id } => write!(f, "style:{id}"),
            Scope::Master { master, shape: None } => write!(f, "master:{master}"),
            Scope::Master { master, shape: Some(s) } => write!(f, "master:{master}/shape:{s}"),
        }
    }
}

/// Borrow the sheet a scope addresses.
pub fn sheet<'a>(doc: &'a Document, scope: &Scope) -> Result<&'a Sheet> {
    match scope {
        Scope::Document => Ok(&doc.sheet),
        Scope::PageSheet { page } => {
            let p = page_by_id(doc, *page)?;
            p.sheet().ok_or_else(|| anyhow::anyhow!("page {page} has no PageSheet"))
        }
        Scope::Shape { page, shape } => {
            let p = page_by_id(doc, *page)?;
            let s = vsdx::model::find_shape_by_id(&p.shapes, *shape);
            match s {
                Some(s) => Ok(&s.sheet),
                None => bail!("shape {shape} not found on page {page}"),
            }
        }
        Scope::StyleSheet { id } => {
            doc.stylesheet(*id).map(|s| &s.sheet).ok_or_else(|| anyhow::anyhow!("stylesheet {id} not found"))
        }
        Scope::Master { master, shape } => {
            let m = doc.master(*master).ok_or_else(|| anyhow::anyhow!("master {master} not found"))?;
            match shape {
                None => m.sheet().ok_or_else(|| anyhow::anyhow!("master {master} has no index sheet")),
                Some(sid) => match vsdx::model::find_shape_by_id(&m.shapes, *sid) {
                    Some(s) => Ok(&s.sheet),
                    None => bail!("shape {sid} not found in master {master}"),
                },
            }
        }
    }
}

/// Mutably borrow the sheet a scope addresses.
pub fn sheet_mut<'a>(doc: &'a mut Document, scope: &Scope) -> Result<&'a mut Sheet> {
    let page_idx = |doc: &Document, id: i64| -> Result<usize> {
        doc.pages.iter().position(|p| p.id == id).ok_or_else(|| anyhow::anyhow!("page {id} not found"))
    };
    let master_idx = |doc: &Document, id: i64| -> Result<usize> {
        doc.masters.iter().position(|m| m.id == id).ok_or_else(|| anyhow::anyhow!("master {id} not found"))
    };
    match scope {
        Scope::Document => Ok(&mut doc.sheet),
        Scope::PageSheet { page } => {
            let i = page_idx(doc, *page)?;
            let p = &mut doc.pages[i];
            let sheet = p.children.iter_mut().find_map(|c| match c {
                IndexChild::Sheet { sheet, .. } => Some(sheet),
                _ => None,
            });
            sheet.ok_or_else(|| anyhow::anyhow!("page {page} has no PageSheet"))
        }
        Scope::Shape { page, shape } => {
            let i = page_idx(doc, *page)?;
            let p = &mut doc.pages[i];
            match find_shape_mut(&mut p.shapes, *shape) {
                Some(s) => Ok(&mut s.sheet),
                None => bail!("shape {shape} not found on page {page}"),
            }
        }
        Scope::StyleSheet { id } => {
            let i = doc.stylesheets.iter().position(|s| s.id == *id);
            match i {
                Some(i) => Ok(&mut doc.stylesheets[i].sheet),
                None => bail!("stylesheet {id} not found"),
            }
        }
        Scope::Master { master, shape } => {
            let i = master_idx(doc, *master)?;
            let m = &mut doc.masters[i];
            match shape {
                None => {
                    let sheet = m.children.iter_mut().find_map(|c| match c {
                        IndexChild::Sheet { sheet, .. } => Some(sheet),
                        _ => None,
                    });
                    sheet.ok_or_else(|| anyhow::anyhow!("master {master} has no index sheet"))
                }
                Some(sid) => match find_shape_mut(&mut m.shapes, *sid) {
                    Some(s) => Ok(&mut s.sheet),
                    None => bail!("shape {sid} not found in master {master}"),
                },
            }
        }
    }
}

/// The inheritance chain for a shape scope (ancestors outermost, target
/// last) — input to [`Document::resolve_cell`].
pub fn shape_chain<'a>(doc: &'a Document, page: i64, shape: i64) -> Result<Vec<&'a Shape>> {
    let p = page_by_id(doc, page)?;
    let mut chain = Vec::new();
    if !collect_chain(&p.shapes, shape, &mut chain) {
        bail!("shape {shape} not found on page {page}");
    }
    Ok(chain)
}

fn collect_chain<'a>(shapes: &'a [Shape], id: i64, out: &mut Vec<&'a Shape>) -> bool {
    for s in shapes {
        out.push(s);
        if s.id == id {
            return true;
        }
        if collect_chain(&s.sub_shapes, id, out) {
            return true;
        }
        out.pop();
    }
    false
}

pub fn page_by_id<'a>(doc: &'a Document, id: i64) -> Result<&'a vsdx::Page> {
    doc.pages.iter().find(|p| p.id == id).ok_or_else(|| anyhow::anyhow!("page {id} not found"))
}

/// Find a shape by ID anywhere in a nested tree, mutably.
fn find_mut_recursive<'a>(shapes: &'a mut [Shape], id: i64) -> Option<&'a mut Shape> {
    for s in shapes {
        if s.id == id {
            return Some(s);
        }
        if let Some(found) = find_mut_recursive(&mut s.sub_shapes, id) {
            return Some(found);
        }
    }
    None
}

/// Find a shape by ID anywhere in a nested tree, mutably (public form).
pub fn find_shape_mut<'a>(shapes: &'a mut Vec<Shape>, id: i64) -> Option<&'a mut Shape> {
    find_mut_recursive(shapes, id)
}
