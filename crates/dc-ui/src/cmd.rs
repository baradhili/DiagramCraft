//! GUI-authored editing commands (implementing [`dc_core::Command`]) —
//! the pieces a canvas needs beyond the dc-core basics. All values are
//! in Visio's internal inches; the mm/in distinction is display-only
//! (see `units.rs`).

use dc_core::commands::{remove_cell, SetCell};
use dc_core::{Command, Scope};
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
