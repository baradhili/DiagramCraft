//! Integration tests for the M4 editing core: command layer, undo/redo,
//! raw-preservation guarantees, and page-set reconciliation in the writer.
//!
//! Fixtures are synthetic (no Microsoft content), written to a temp
//! package and parsed back so edits run against a retained source
//! package — the real editing scenario.

use dc_core::commands::{
    AddConnect, AddLayer, AddPage, AddShape, DuplicatePage, GroupShapes, MovePage, RemoveConnect,
    RemoveLayer, RemovePage, RemoveShape, RenameLayer, RenamePage, RowSel, SetCell, SetRowCell,
    SetText, UngroupShape,
};
use dc_core::connects::{Connect, ConnectsHost, ConnectsOwner};
use dc_core::layers::{layer_name, layer_rows};
use dc_core::scope::{sheet as scope_sheet, Scope};
use dc_core::{Editor, History};
use vsdx::model::{Cell, Document, IndexChild, Shape};

// ---------------------------------------------------------------------------
// Synthetic fixture
// ---------------------------------------------------------------------------

const CONTENT_TYPES: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
<Default Extension="xml" ContentType="application/xml"/>
<Override PartName="/visio/document.xml" ContentType="application/vnd.ms-visio.drawing.main+xml"/>
<Override PartName="/visio/pages/pages.xml" ContentType="application/vnd.ms-visio.pages+xml"/>
<Override PartName="/visio/pages/page1.xml" ContentType="application/vnd.ms-visio.pages+xml"/>
<Override PartName="/visio/masters/masters.xml" ContentType="application/vnd.ms-visio.masters+xml"/>
<Override PartName="/visio/masters/master1.xml" ContentType="application/vnd.ms-visio.master+xml"/>
</Types>"#;

const ROOT_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.microsoft.com/visio/2010/relationships/document" Target="visio/document.xml"/>
</Relationships>"#;

const DOCUMENT: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<VisioDocument xmlns='http://schemas.microsoft.com/office/visio/2012/main' xmlns:r='http://schemas.openxmlformats.org/officeDocument/2006/relationships'>
<StyleSheets>
<StyleSheet ID='0' NameU='No Style'>
  <Cell N='LineWeight' V='0.01041666666666667'/>
  <Cell N='LineColor' V='0'/>
  <Cell N='FillForegnd' V='1'/>
  <Cell N='FillPattern' V='1'/>
</StyleSheet>
</StyleSheets>
</VisioDocument>"#;

const DOC_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.microsoft.com/visio/2010/relationships/pages" Target="pages/pages.xml"/>
<Relationship Id="rId2" Type="http://schemas.microsoft.com/visio/2010/relationships/masters" Target="masters/masters.xml"/>
</Relationships>"#;

const PAGES: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<Pages xmlns='http://schemas.microsoft.com/office/visio/2012/main' xmlns:r='http://schemas.openxmlformats.org/officeDocument/2006/relationships'>
<Page ID='0' NameU='Page-1' Name='Page-1'>
<PageSheet LineStyle='0' FillStyle='0' TextStyle='0'>
  <Cell N='PageWidth' V='8.5'/>
  <Cell N='PageHeight' V='11'/>
  <Section N='Layer'>
    <Row IX='0'>
      <Cell N='Name' V='Connector'/><Cell N='Color' V='255'/><Cell N='Status' V='0'/>
      <Cell N='Visible' V='1'/><Cell N='Print' V='1'/><Cell N='Active' V='0'/>
      <Cell N='Lock' V='0'/><Cell N='Snap' V='1'/><Cell N='Glue' V='1'/>
      <Cell N='NameUniv' V='Connector'/><Cell N='ColorTrans' V='0'/>
    </Row>
  </Section>
</PageSheet>
<Rel r:id='rId1'/>
</Page>
</Pages>"#;

const PAGES_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.microsoft.com/visio/2010/relationships/page" Target="page1.xml"/>
</Relationships>"#;

const PAGE1: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<PageContents xmlns='http://schemas.microsoft.com/office/visio/2012/main' xmlns:r='http://schemas.openxmlformats.org/officeDocument/2006/relationships'>
<Shapes>
<Shape ID='1' Type='Shape' LineStyle='0' FillStyle='0' TextStyle='0'>
  <Cell N='PinX' V='4.25'/><Cell N='PinY' V='5.5'/>
  <Cell N='Width' V='2'/><Cell N='Height' V='1'/>
  <Cell N='LocPinX' V='1' F='Width*0.5'/><Cell N='LocPinY' V='0.5' F='Height*0.5'/>
  <Cell N='LineWeight' V='0.02083333333333333'/>
  <Section N='Geometry' IX='0'>
    <Row T='MoveTo' IX='1'><Cell N='X' V='0' F='Width*0'/><Cell N='Y' V='0' F='Height*0'/></Row>
    <Row T='LineTo' IX='2'><Cell N='X' V='2' F='Width*1'/><Cell N='Y' V='0' F='Height*0'/></Row>
    <Row T='LineTo' IX='3'><Cell N='X' V='2' F='Width*1'/><Cell N='Y' V='1' F='Height*1'/></Row>
    <Row T='LineTo' IX='4'><Cell N='X' V='0' F='Width*0'/><Cell N='Y' V='1' F='Height*1'/></Row>
    <Row T='LineTo' IX='5'><Cell N='X' V='0' F='Width*0'/><Cell N='Y' V='0' F='Height*0'/></Row>
  </Section>
  <Text>hello world</Text>
</Shape>
<Shape ID='2' NameU='BoxFromMaster' Type='Shape' Master='7'>
  <Cell N='PinX' V='2'/><Cell N='PinY' V='2'/>
</Shape>
</Shapes>
<Connects>
<Connect FromSheet='1' FromCell='BeginX' FromPart='9' ToSheet='2' ToCell='PinX' ToPart='0'/>
</Connects>
</PageContents>"#;

const MASTERS: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<Masters xmlns='http://schemas.microsoft.com/office/visio/2012/main' xmlns:r='http://schemas.openxmlformats.org/officeDocument/2006/relationships'>
<Master ID='7' NameU='Box'>
<PageSheet LineStyle='0' FillStyle='0' TextStyle='0'>
  <Cell N='LineWeight' V='0.015625'/>
</PageSheet>
<Rel r:id='rId1'/>
</Master>
</Masters>"#;

const MASTERS_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.microsoft.com/visio/2010/relationships/master" Target="master1.xml"/>
</Relationships>"#;

const MASTER1: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<MasterContents xmlns='http://schemas.microsoft.com/office/visio/2012/main' xmlns:r='http://schemas.openxmlformats.org/officeDocument/2006/relationships'>
<Shapes>
<Shape ID='1' Type='Shape' LineStyle='0' FillStyle='0' TextStyle='0'>
  <Cell N='Width' V='1'/><Cell N='Height' V='1'/>
  <Cell N='LineWeight' V='0.015625'/>
  <Section N='Geometry' IX='0'>
    <Row T='MoveTo' IX='1'><Cell N='X' V='0'/><Cell N='Y' V='0'/></Row>
    <Row T='LineTo' IX='2'><Cell N='X' V='1'/><Cell N='Y' V='0'/></Row>
    <Row T='LineTo' IX='3'><Cell N='X' V='1'/><Cell N='Y' V='1'/></Row>
    <Row T='LineTo' IX='4'><Cell N='X' V='0'/><Cell N='Y' V='1'/></Row>
    <Row T='LineTo' IX='5'><Cell N='X' V='0'/><Cell N='Y' V='0'/></Row>
  </Section>
</Shape>
</Shapes>
<Connects>
<Connect FromSheet='60' FromCell='EndX' FromPart='12' ToSheet='51' ToCell='Connections.X1' ToPart='100'/>
<Connect FromSheet='60' FromCell='BeginX' FromPart='9' ToSheet='59' ToCell='Connections.X1' ToPart='100'/>
</Connects>
</MasterContents>"#;

/// Write the fixture package and return its path (inside `tmp`).
fn fixture(tmp: &std::path::Path) -> std::path::PathBuf {
    let path = tmp.join("fixture.vsdx");
    let file = std::fs::File::create(&path).unwrap();
    let mut w = zip::ZipWriter::new(file);
    let opts = zip::write::SimpleFileOptions::default();
    for (name, data) in [
        ("[Content_Types].xml", CONTENT_TYPES),
        ("_rels/.rels", ROOT_RELS),
        ("visio/document.xml", DOCUMENT),
        ("visio/_rels/document.xml.rels", DOC_RELS),
        ("visio/pages/pages.xml", PAGES),
        ("visio/pages/_rels/pages.xml.rels", PAGES_RELS),
        ("visio/pages/page1.xml", PAGE1),
        ("visio/masters/masters.xml", MASTERS),
        ("visio/masters/_rels/masters.xml.rels", MASTERS_RELS),
        ("visio/masters/master1.xml", MASTER1),
    ] {
        w.start_file(name, opts).unwrap();
        std::io::Write::write_all(&mut w, data.as_bytes()).unwrap();
    }
    w.finish().unwrap();
    path
}

/// Parsed fixture document.
fn open_fixture() -> (Editor, tempfile::TempDir) {
    let tmp = tempfile::tempdir().unwrap();
    let path = fixture(tmp.path());
    let ed = Editor::open(&path).unwrap();
    (ed, tmp)
}

fn part_bytes(path: &std::path::Path, name: &str) -> Vec<u8> {
    vsdx::Package::open(path)
        .unwrap()
        .part(name)
        .unwrap()
        .to_vec()
}

// ---------------------------------------------------------------------------
// History semantics (code-built document, no package)
// ---------------------------------------------------------------------------

fn bare_doc() -> Document {
    let mut sheet = vsdx::model::Sheet::default();
    sheet.set_cell("LineWeight", Cell::value("0.01"));
    let mut shape = Shape {
        id: 1,
        type_: Some("Shape".into()),
        ..Shape::default()
    };
    shape.sheet.set_cell("PinX", Cell::value("1"));
    let page = vsdx::model::Page {
        id: 0,
        name: "P1".into(),
        attrs: vec![("ID".into(), "0".into()), ("NameU".into(), "P1".into())],
        children: vec![IndexChild::Sheet {
            tag: "PageSheet".into(),
            sheet,
        }],
        shapes: vec![shape],
        ..vsdx::model::Page::default()
    };
    Document {
        pages: vec![page],
        ..Document::default()
    }
}

#[test]
fn history_undo_redo_and_labels() {
    let mut doc = bare_doc();
    let mut h = History::new(10);
    h.apply(
        Box::new(SetCell::new(
            Scope::PageSheet { page: 0 },
            "LineWeight",
            Cell::value("0.05"),
        )),
        &mut doc,
    )
    .unwrap();
    h.apply(
        Box::new(SetCell::new(
            Scope::Shape { page: 0, shape: 1 },
            "PinX",
            Cell::value("9"),
        )),
        &mut doc,
    )
    .unwrap();
    assert_eq!(h.undo_labels().len(), 2);
    assert!(!h.can_redo());

    let label = h.undo(&mut doc).unwrap().unwrap();
    assert!(label.0.contains("PinX"));
    assert_eq!(
        doc.pages[0].shapes[0]
            .sheet
            .cell("PinX")
            .unwrap()
            .v
            .as_deref(),
        Some("1")
    );
    let _ = h.redo(&mut doc).unwrap().unwrap();
    assert_eq!(
        doc.pages[0].shapes[0]
            .sheet
            .cell("PinX")
            .unwrap()
            .v
            .as_deref(),
        Some("9")
    );

    // A new apply clears redo.
    h.apply(
        Box::new(SetCell::new(
            Scope::Shape { page: 0, shape: 1 },
            "PinX",
            Cell::value("8"),
        )),
        &mut doc,
    )
    .unwrap();
    assert!(!h.can_redo());
    // Three applied commands → three undos back to the pristine sheet.
    for _ in 0..3 {
        assert!(h.undo(&mut doc).unwrap().is_some());
    }
    assert!(!h.can_undo());
    assert_eq!(
        doc.pages[0]
            .sheet()
            .unwrap()
            .cell("LineWeight")
            .unwrap()
            .v
            .as_deref(),
        Some("0.01")
    );
    assert_eq!(
        doc.pages[0].shapes[0]
            .sheet
            .cell("PinX")
            .unwrap()
            .v
            .as_deref(),
        Some("1")
    );
}

#[test]
fn history_limit_truncates_oldest() {
    let mut doc = bare_doc();
    let mut h = History::new(2);
    for v in ["0.1", "0.2", "0.3"] {
        h.apply(
            Box::new(SetCell::new(
                Scope::PageSheet { page: 0 },
                "LineWeight",
                Cell::value(v),
            )),
            &mut doc,
        )
        .unwrap();
    }
    assert_eq!(h.undo_labels().len(), 2);
    assert!(h.undo(&mut doc).unwrap().is_some());
    assert!(h.undo(&mut doc).unwrap().is_some());
    assert!(h.undo(&mut doc).unwrap().is_none());
    // After two undos we are back at 0.1 (the first edit fell off).
    assert_eq!(
        doc.pages[0]
            .sheet()
            .unwrap()
            .cell("LineWeight")
            .unwrap()
            .v
            .as_deref(),
        Some("0.1")
    );
}

// ---------------------------------------------------------------------------
// Raw-preservation guarantee: an undone edit is byte-invisible
// ---------------------------------------------------------------------------

#[test]
fn set_cell_is_surgical_and_undo_is_byte_exact() {
    let (ed, tmp) = open_fixture();
    let mut ed = ed;
    let base = tmp.path().join("base.vsdx");
    ed.save_as(&base).unwrap();

    ed.apply(SetCell::new(
        Scope::Shape { page: 0, shape: 1 },
        "LineWeight",
        Cell::value("0.05"),
    ))
    .unwrap();
    let edited = tmp.path().join("edited.vsdx");
    ed.save_as(&edited).unwrap();

    // Only the shape's page part differs (canonically); everything else is byte-identical.
    for name in [
        "[Content_Types].xml",
        "_rels/.rels",
        "visio/document.xml",
        "visio/_rels/document.xml.rels",
        "visio/pages/pages.xml",
        "visio/pages/_rels/pages.xml.rels",
        "visio/masters/masters.xml",
        "visio/masters/_rels/masters.xml.rels",
        "visio/masters/master1.xml",
    ] {
        assert_eq!(
            part_bytes(&base, name),
            part_bytes(&edited, name),
            "{name} changed"
        );
    }
    assert_ne!(
        vsdx::canonical_form(&part_bytes(&base, "visio/pages/page1.xml")),
        vsdx::canonical_form(&part_bytes(&edited, "visio/pages/page1.xml"))
    );

    // Re-parsed edit shows the new value.
    let reparsed = vsdx::parse::parse(&edited).unwrap();
    let shape1 = vsdx::model::find_shape_by_id(&reparsed.pages[0].shapes, 1).unwrap();
    assert_eq!(
        shape1.sheet.cell("LineWeight").unwrap().v.as_deref(),
        Some("0.05")
    );
    // Untouched cell keeps formula + attrs.
    assert_eq!(
        shape1.sheet.cell("LocPinX").unwrap().f.as_deref(),
        Some("Width*0.5")
    );

    // Undo → byte-identical package to the baseline.
    ed.undo().unwrap();
    let undone = tmp.path().join("undone.vsdx");
    ed.save_as(&undone).unwrap();
    let names = vsdx::Package::open(&base).unwrap();
    let out = vsdx::Package::open(&undone).unwrap();
    assert_eq!(names.part_names(), out.part_names());
    for name in names.part_names() {
        assert_eq!(
            names.part(name).unwrap(),
            out.part(name).unwrap(),
            "{name} differs after undo"
        );
    }
}

#[test]
fn set_cell_on_absent_cell_undo_removes_it() {
    let (ed, tmp) = open_fixture();
    let mut ed = ed;
    ed.apply(SetCell::new(
        Scope::Shape { page: 0, shape: 1 },
        "RotateCmd",
        Cell::value("1"),
    ))
    .unwrap();
    let shape = vsdx::model::find_shape_by_id(&ed.doc.pages[0].shapes, 1).unwrap();
    assert!(shape.sheet.cell("RotateCmd").is_some());
    ed.undo().unwrap();
    let shape = vsdx::model::find_shape_by_id(&ed.doc.pages[0].shapes, 1).unwrap();
    assert!(shape.sheet.cell("RotateCmd").is_none());
    let _ = tmp;
}

#[test]
fn set_row_cell_and_undo() {
    let (ed, _tmp) = open_fixture();
    let mut ed = ed;
    let scope = Scope::Shape { page: 0, shape: 1 };
    let old = {
        let s = scope_sheet(&ed.doc, &scope).unwrap();
        s.sections_named("Geometry")[0]
            .row_by_ix(2)
            .unwrap()
            .cell("X")
            .unwrap()
            .clone()
    };
    ed.apply(SetRowCell::new(
        scope.clone(),
        "Geometry",
        0,
        RowSel::Ix(2),
        "X",
        Cell::value("1.5"),
    ))
    .unwrap();
    let s = scope_sheet(&ed.doc, &scope).unwrap();
    assert_eq!(
        s.sections_named("Geometry")[0]
            .row_by_ix(2)
            .unwrap()
            .cell("X")
            .unwrap()
            .v
            .as_deref(),
        Some("1.5")
    );
    // formula preserved by replace? set semantics: new cell has no formula → V only.
    // undo restores the original cell exactly (including F and attrs).
    ed.undo().unwrap();
    let s = scope_sheet(&ed.doc, &scope).unwrap();
    assert_eq!(
        s.sections_named("Geometry")[0]
            .row_by_ix(2)
            .unwrap()
            .cell("X")
            .unwrap(),
        &old
    );
}

#[test]
fn set_text_escapes_and_round_trips() {
    let (ed, tmp) = open_fixture();
    let mut ed = ed;
    ed.apply(SetText::new(
        Scope::Shape { page: 0, shape: 1 },
        "a & b <c>",
    ))
    .unwrap();
    let out = tmp.path().join("text.vsdx");
    ed.save_as(&out).unwrap();
    let reparsed = vsdx::parse::parse(&out).unwrap();
    let shape = vsdx::model::find_shape_by_id(&reparsed.pages[0].shapes, 1).unwrap();
    assert_eq!(shape.sheet.text.as_deref(), Some("a & b <c>"));
    // undo restores the original raw text exactly
    ed.undo().unwrap();
    assert_eq!(
        ed.doc.pages[0].shapes[0].sheet.text_raw.as_deref(),
        Some("hello world")
    );
}

// ---------------------------------------------------------------------------
// Page set: add / remove / rename / move through the writer
// ---------------------------------------------------------------------------

#[test]
fn add_page_materializes_part_rels_and_content_type() {
    let (ed, tmp) = open_fixture();
    let mut ed = ed;
    let base = tmp.path().join("base.vsdx");
    ed.save_as(&base).unwrap();

    ed.apply(AddPage::new("Page-2")).unwrap();
    let new_id = ed.doc.pages[1].id;
    assert_eq!(new_id, 1);
    assert_eq!(ed.doc.pages[1].part, "visio/pages/page2.xml");
    let out = tmp.path().join("added.vsdx");
    ed.save_as(&out).unwrap();

    let pkg = vsdx::Package::open(&out).unwrap();
    let names: Vec<&str> = pkg.part_names();
    assert!(names.contains(&"visio/pages/page2.xml"));
    let rels = String::from_utf8(
        pkg.part("visio/pages/_rels/pages.xml.rels")
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(rels.contains("page2.xml"));
    let ct = String::from_utf8(pkg.part("[Content_Types].xml").unwrap().to_vec()).unwrap();
    assert!(ct.contains("/visio/pages/page2.xml"));

    // Untouched parts stay byte-identical.
    for name in [
        "visio/document.xml",
        "visio/pages/page1.xml",
        "visio/masters/master1.xml",
    ] {
        assert_eq!(
            part_bytes(&base, name),
            part_bytes(&out, name),
            "{name} changed"
        );
    }

    // Re-parse: two pages, the new one empty with its name.
    let reparsed = vsdx::parse::parse(&out).unwrap();
    assert_eq!(reparsed.pages.len(), 2);
    assert_eq!(reparsed.pages[1].name, "Page-2");

    // Removing the added page returns the package to the original part set,
    // with the index canonically equal and the original parts byte-equal.
    ed.apply(RemovePage::new(new_id)).unwrap();
    let reverted = tmp.path().join("reverted.vsdx");
    ed.save_as(&reverted).unwrap();
    let a = vsdx::Package::open(&base).unwrap();
    let b = vsdx::Package::open(&reverted).unwrap();
    assert_eq!(a.part_names(), b.part_names());
    assert_eq!(
        vsdx::canonical_form(a.part("visio/pages/pages.xml").unwrap()),
        vsdx::canonical_form(b.part("visio/pages/pages.xml").unwrap())
    );
    for name in [
        "visio/document.xml",
        "visio/pages/page1.xml",
        "visio/masters/masters.xml",
    ] {
        assert_eq!(
            a.part(name).unwrap(),
            b.part(name).unwrap(),
            "{name} changed"
        );
    }
}

#[test]
fn remove_page_undo_restores_it() {
    let (ed, tmp) = open_fixture();
    let mut ed = ed;
    ed.apply(RemovePage::new(0)).unwrap();
    assert_eq!(ed.doc.pages.len(), 0);
    let out = tmp.path().join("nopages.vsdx");
    ed.save_as(&out).unwrap();
    let pkg = vsdx::Package::open(&out).unwrap();
    assert!(!pkg.part_names().contains(&"visio/pages/page1.xml"));
    let ct = String::from_utf8(pkg.part("[Content_Types].xml").unwrap().to_vec()).unwrap();
    assert!(!ct.contains("page1.xml"));

    ed.undo().unwrap();
    assert_eq!(ed.doc.pages.len(), 1);
    assert_eq!(ed.doc.pages[0].name, "Page-1");
    assert_eq!(ed.doc.pages[0].part, "visio/pages/page1.xml");
}

/// #68: a new page's PageSheet is seeded from its source — size
/// always, the scale cells only when the source carries them — with
/// the document's last page as the default source, an explicit page
/// via `seeded_from`, and the capture replaying exactly across redo.
#[test]
fn add_page_seeds_sheet_setup_from_source() {
    let (ed, tmp) = open_fixture();
    let mut ed = ed;
    let cell = |ed: &Editor, page: usize, n: &str| -> Option<f64> {
        ed.doc.pages[page]
            .sheet()
            .and_then(|s| s.cell(n))
            .and_then(|c| c.numeric())
    };

    // Default source = the document's last page: the fixture's letter
    // sheet. Scale cells the source lacks are not invented.
    ed.apply(AddPage::new("Page-2")).unwrap();
    assert_eq!(
        cell(&ed, 1, "PageWidth"),
        Some(8.5),
        "width seeded from the source page"
    );
    assert_eq!(
        cell(&ed, 1, "PageHeight"),
        Some(11.0),
        "height seeded from the source page"
    );
    assert_eq!(cell(&ed, 1, "PageScale"), None, "no scale cells invented");

    // Save → reparse: the seeded cells persist in the written package.
    let out = tmp.path().join("seeded.vsdx");
    ed.save_as(&out).unwrap();
    let back = Editor::open(&out).unwrap();
    assert_eq!(cell(&back, 1, "PageWidth"), Some(8.5));
    assert_eq!(cell(&back, 1, "PageHeight"), Some(11.0));

    // An explicit source beats the last-page default: grow page 2,
    // then add from page 0.
    ed.apply(SetCell::new(
        Scope::PageSheet { page: 1 },
        "PageWidth",
        Cell::value("5"),
    ))
    .unwrap();
    ed.apply(AddPage::seeded_from("Page-3", 0)).unwrap();
    assert_eq!(
        cell(&ed, 2, "PageWidth"),
        Some(8.5),
        "seeded_from pins the source page"
    );
    ed.undo().unwrap(); // drop Page-3

    // The default source follows the last page (now 5 in wide), and
    // undo/redo replay the captured setup exactly.
    ed.apply(AddPage::new("Page-4")).unwrap();
    assert_eq!(
        cell(&ed, 2, "PageWidth"),
        Some(5.0),
        "the default source is the last page"
    );
    ed.undo().unwrap();
    assert_eq!(ed.doc.pages.len(), 2, "undo removes the seeded page cleanly");
    ed.redo().unwrap();
    assert_eq!(ed.doc.pages.len(), 3, "redo brings it back");
    assert_eq!(
        cell(&ed, 2, "PageWidth"),
        Some(5.0),
        "redo replays the captured setup"
    );
}

#[test]
fn rename_and_move_page() {
    let (ed, _tmp) = open_fixture();
    let mut ed = ed;
    ed.apply(AddPage::new("Page-2")).unwrap();
    ed.apply(AddPage::new("Page-3")).unwrap();
    ed.apply(RenamePage::new(1, "Middle")).unwrap();
    assert_eq!(ed.doc.pages[1].name, "Middle");
    assert!(ed.doc.pages[1]
        .attrs
        .iter()
        .any(|(k, v)| k == "NameU" && v == "Middle"));
    ed.undo().unwrap();
    assert_eq!(ed.doc.pages[1].name, "Page-2");
    assert!(ed.doc.pages[1]
        .attrs
        .iter()
        .any(|(k, v)| k == "NameU" && v == "Page-2"));

    ed.apply(MovePage { from: 2, to: 0 }).unwrap();
    let names: Vec<&str> = ed.doc.pages.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, vec!["Page-3", "Page-1", "Page-2"]);
    ed.undo().unwrap();
    let names: Vec<&str> = ed.doc.pages.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, vec!["Page-1", "Page-2", "Page-3"]);
}

#[test]
fn duplicate_page_clones_content_with_fresh_identity() {
    let (ed, tmp) = open_fixture();
    let mut ed = ed;
    ed.apply(DuplicatePage::new(0)).unwrap();
    assert_eq!(ed.doc.pages.len(), 2);
    let src = &ed.doc.pages[0];
    let copy = &ed.doc.pages[1];
    assert_ne!(copy.id, src.id, "fresh page id");
    assert_eq!(copy.part, "visio/pages/page2.xml", "own part");
    assert_eq!(copy.name, src.name, "name kept; the UI renames");
    assert_eq!(copy.shapes.len(), src.shapes.len(), "shapes deep-cloned");
    assert_eq!(
        copy.shapes[0].id, src.shapes[0].id,
        "shape ids are page-local"
    );
    let rid_of = |p: &vsdx::model::Page| {
        p.children.iter().find_map(|c| match c {
            vsdx::model::IndexChild::Rel(attrs) => attrs
                .iter()
                .find(|(k, _)| k == "r:id")
                .map(|(_, v)| v.clone()),
            _ => None,
        })
    };
    assert_ne!(rid_of(copy).unwrap(), rid_of(src).unwrap(), "fresh r:id");

    // Save → reparse: the duplicate is materialized as its own part
    // with the cloned shapes.
    let out = tmp.path().join("dup.vsdx");
    ed.save_as(&out).unwrap();
    let pkg = vsdx::Package::open(&out).unwrap();
    assert!(pkg.part_names().contains(&"visio/pages/page2.xml"));
    let reparsed = vsdx::parse::parse(&out).unwrap();
    assert_eq!(reparsed.pages.len(), 2);
    assert_eq!(
        reparsed.pages[1].shapes.len(),
        reparsed.pages[0].shapes.len(),
        "clone keeps every shape"
    );

    // Undo returns to one page; redo re-materializes the copy.
    ed.undo().unwrap();
    assert_eq!(ed.doc.pages.len(), 1);
    ed.redo().unwrap();
    assert_eq!(ed.doc.pages.len(), 2);
}

#[test]
fn multi_page_save_reload_preserves_names_order_and_edits() {
    let (ed, tmp) = open_fixture();
    let mut ed = ed;
    ed.apply(AddPage::new("Middle")).unwrap();
    // Edit page 2 through its own duplicate of the source shape.
    ed.apply(DuplicatePage::new(0)).unwrap();
    let page2 = ed.doc.pages[1].id;
    let sid = ed.doc.pages[1].shapes[0].id;
    // Scope addresses pages by ID, not index.
    ed.apply(SetText::new(
        Scope::Shape {
            page: page2,
            shape: sid,
        },
        "page two edit",
    ))
    .unwrap();
    ed.apply(RenamePage::new(page2, "Renamed-2")).unwrap();
    ed.apply(MovePage { from: 1, to: 0 }).unwrap();

    let out = tmp.path().join("multi.vsdx");
    ed.save_as(&out).unwrap();
    let re = vsdx::parse::parse(&out).unwrap();
    let names: Vec<&str> = re.pages.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(
        names,
        vec!["Renamed-2", "Page-1", "Middle"],
        "order survives"
    );
    let text = |p: &vsdx::model::Page| {
        p.shapes[0]
            .sheet
            .text
            .as_deref()
            .unwrap_or_default()
            .to_string()
    };
    assert_eq!(text(&re.pages[0]), "page two edit", "edit follows its page");
    assert_ne!(text(&re.pages[1]), "page two edit", "source untouched");
}

// ---------------------------------------------------------------------------
// Shapes
// ---------------------------------------------------------------------------

#[test]
fn add_and_remove_shape_undo() {
    let (ed, _tmp) = open_fixture();
    let mut ed = ed;
    let id = dc_core::commands::next_shape_id(&ed.doc, 0);
    assert_eq!(id, 3);
    let mut shape = Shape {
        id,
        type_: Some("Shape".into()),
        ..Shape::default()
    };
    shape.sheet.set_cell("PinX", Cell::value("1"));
    ed.apply(AddShape {
        page: 0,
        parent: None,
        shape,
    })
    .unwrap();
    assert!(vsdx::model::find_shape_by_id(&ed.doc.pages[0].shapes, 3).is_some());
    ed.undo().unwrap();
    assert!(vsdx::model::find_shape_by_id(&ed.doc.pages[0].shapes, 3).is_none());

    ed.apply(RemoveShape::new(0, 2)).unwrap();
    assert!(vsdx::model::find_shape_by_id(&ed.doc.pages[0].shapes, 2).is_none());
    ed.undo().unwrap();
    let restored = vsdx::model::find_shape_by_id(&ed.doc.pages[0].shapes, 2).unwrap();
    assert_eq!(restored.name_u.as_deref(), Some("BoxFromMaster"));
    assert_eq!(restored.master, Some(7));
}

// ---------------------------------------------------------------------------
// Connections
// ---------------------------------------------------------------------------

#[test]
fn connects_parse_store_idempotent_and_editable() {
    let (ed, _tmp) = open_fixture();
    let mut ed = ed;

    // Page-level: one connection from the fixture.
    let conns = ConnectsOwner::Page(0).read_connects(&ed.doc).unwrap();
    assert_eq!(conns.list.len(), 1);
    assert_eq!(conns.list[0].from_cell, "BeginX");
    assert_eq!(conns.list[0].from_part, 9);
    assert_eq!(conns.list[0].to_cell, "PinX");

    // Master-level: two connections, corpus-shaped.
    let mconns = ConnectsOwner::Master(7).read_connects(&ed.doc).unwrap();
    assert_eq!(mconns.list.len(), 2);
    assert_eq!(mconns.list[0].from_sheet, 60);
    assert_eq!(mconns.list[0].to_part, 100);
    assert_eq!(mconns.list[1].from_part, 9);

    // Unmodified parse → store is byte-stable (inner XML unchanged).
    let before_inner = ed.doc.pages[0]
        .contents_raw
        .iter()
        .find(|r| r.name == "Connects")
        .map(|r| r.inner.clone())
        .unwrap();
    let c = ConnectsOwner::Page(0).read_connects(&ed.doc).unwrap();
    ConnectsOwner::Page(0)
        .host(&mut ed.doc)
        .unwrap()
        .set_connects(&c);
    let after_inner = ed.doc.pages[0]
        .contents_raw
        .iter()
        .find(|r| r.name == "Connects")
        .map(|r| r.inner.clone())
        .unwrap();
    assert_eq!(before_inner, after_inner);

    // Add + undo round the package.
    ed.apply(AddConnect {
        owner: ConnectsOwner::Page(0),
        connect: Connect::new(2, "EndX", 12, 1, "PinX", 0),
    })
    .unwrap();
    assert_eq!(
        ConnectsOwner::Page(0)
            .read_connects(&ed.doc)
            .unwrap()
            .list
            .len(),
        2
    );
    ed.undo().unwrap();
    assert_eq!(
        ConnectsOwner::Page(0)
            .read_connects(&ed.doc)
            .unwrap()
            .list
            .len(),
        1
    );

    // RemoveConnect by index.
    ed.apply(RemoveConnect::new(ConnectsOwner::Page(0), 0))
        .unwrap();
    assert!(ConnectsOwner::Page(0)
        .read_connects(&ed.doc)
        .unwrap()
        .list
        .is_empty());
    // An empty Connects is dropped entirely.
    assert!(!ed.doc.pages[0]
        .contents_raw
        .iter()
        .any(|r| r.name == "Connects"));
    ed.undo().unwrap();
    assert_eq!(
        ConnectsOwner::Page(0)
            .read_connects(&ed.doc)
            .unwrap()
            .list
            .len(),
        1
    );
}

#[test]
fn connects_edit_survives_save_and_reparse() {
    let (ed, tmp) = open_fixture();
    let mut ed = ed;
    ed.apply(AddConnect {
        owner: ConnectsOwner::Page(0),
        connect: Connect::new(2, "EndX", 12, 1, "PinX", 0),
    })
    .unwrap();
    let out = tmp.path().join("conn.vsdx");
    ed.save_as(&out).unwrap();
    let reparsed = vsdx::parse::parse(&out).unwrap();
    let conns = reparsed.pages[0].connects();
    assert_eq!(conns.list.len(), 2);
    assert_eq!(conns.list[1].from_cell, "EndX");
    assert_eq!(conns.list[1].from_part, 12);
}

// ---------------------------------------------------------------------------
// Layers
// ---------------------------------------------------------------------------

#[test]
fn layers_add_remove_rename() {
    let (ed, _tmp) = open_fixture();
    let mut ed = ed;
    let rows = layer_rows(&ed.doc, 0).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(layer_name(&rows[0]), Some("Connector"));

    ed.apply(AddLayer::new(0, "Background")).unwrap();
    let rows = layer_rows(&ed.doc, 0).unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[1].ix, 1);
    assert_eq!(layer_name(&rows[1]), Some("Background"));
    ed.undo().unwrap();
    assert_eq!(layer_rows(&ed.doc, 0).unwrap().len(), 1);

    ed.apply(RenameLayer::new(0, 0, "FG")).unwrap();
    let rows = layer_rows(&ed.doc, 0).unwrap();
    assert_eq!(layer_name(&rows[0]), Some("FG"));
    assert_eq!(rows[0].cell("NameUniv").unwrap().v.as_deref(), Some("FG"));
    ed.undo().unwrap();
    let rows = layer_rows(&ed.doc, 0).unwrap();
    assert_eq!(layer_name(&rows[0]), Some("Connector"));

    ed.apply(RemoveLayer::new(0, 0)).unwrap();
    assert!(layer_rows(&ed.doc, 0).unwrap().is_empty());
    // Removing the last row drops the section? (implementation keeps the
    // section; only AddLayer-created sections are removed on undo.)
    ed.undo().unwrap();
    assert_eq!(layer_rows(&ed.doc, 0).unwrap().len(), 1);
}

#[test]
fn add_layer_creates_section_when_absent_and_undo_removes_it() {
    let (ed, tmp) = open_fixture();
    let mut ed = ed;
    ed.apply(AddPage::new("P2")).unwrap();
    let new_page_id = ed.doc.pages[1].id;
    assert!(layer_rows(&ed.doc, new_page_id).unwrap().is_empty());
    ed.apply(AddLayer::new(new_page_id, "First")).unwrap();
    assert_eq!(layer_rows(&ed.doc, new_page_id).unwrap().len(), 1);
    ed.undo().unwrap();
    assert!(layer_rows(&ed.doc, new_page_id).unwrap().is_empty());
    // section fully removed → page sheet has no Layer section child
    let sheet = ed.doc.pages[1].sheet().unwrap();
    assert!(!sheet.has_sections() || sheet.sections_named("Layer").is_empty());
    let _ = tmp;
}

// ---------------------------------------------------------------------------
// Scopes / inspector
// ---------------------------------------------------------------------------

#[test]
fn scopes_parse_and_resolve() {
    let (ed, _tmp) = open_fixture();
    let ed = ed;
    assert!(Scope::parse("doc").unwrap() == Scope::Document);
    assert_eq!(
        Scope::parse("page:0").unwrap(),
        Scope::PageSheet { page: 0 }
    );
    assert_eq!(
        Scope::parse("page:0/shape:1").unwrap(),
        Scope::Shape { page: 0, shape: 1 }
    );
    assert_eq!(
        Scope::parse("master:7/shape:1").unwrap(),
        Scope::Master {
            master: 7,
            shape: Some(1)
        }
    );
    assert!(Scope::parse("bogus").is_err());
    assert!(Scope::parse("page:x").is_err());

    for s in [
        "doc",
        "page:0",
        "page:0/shape:1",
        "page:0/shape:2",
        "style:0",
        "master:7",
        "master:7/shape:1",
    ] {
        let scope = Scope::parse(s).unwrap();
        assert!(scope_sheet(&ed.doc, &scope).is_ok(), "{s} should resolve");
    }
    assert!(scope_sheet(&ed.doc, &Scope::parse("page:9").unwrap()).is_err());
    assert!(scope_sheet(&ed.doc, &Scope::parse("page:0/shape:9").unwrap()).is_err());

    // Inheritance-aware cell resolution: shape 2 (master-backed, no
    // LineWeight of its own) resolves through the master's PageSheet.
    let chain = dc_core::scope::shape_chain(&ed.doc, 0, 2).unwrap();
    let cell = ed
        .doc
        .resolve_cell(chain[chain.len() - 1], &chain, "LineWeight")
        .unwrap();
    assert_eq!(cell.v.as_deref(), Some("0.015625"));
}

#[test]
fn new_blank_roundtrips() {
    let mut ed = Editor::new_blank();
    assert_eq!(ed.doc.pages.len(), 1);
    // Starter masters ship with the blank drawing (Basic Shapes set,
    // plus the 1D Line master — GitHub #59). The exact count is the
    // template's business, not a contract: adding a starter master
    // must not fail this test.
    assert!(ed.doc.masters.len() >= 8);
    assert!(ed
        .doc
        .masters
        .iter()
        .any(|m| m.name_u.as_deref() == Some("Rectangle")));
    assert!(ed
        .doc
        .masters
        .iter()
        .any(|m| m.name_u.as_deref() == Some("Line")));
    let page = &ed.doc.pages[0];
    assert_eq!(page.name, "Page-1");
    let w = page
        .sheet()
        .and_then(|s| s.cell("PageWidth"))
        .and_then(|c| c.numeric());
    let h = page
        .sheet()
        .and_then(|s| s.cell("PageHeight"))
        .and_then(|c| c.numeric());
    assert_eq!((w, h), (Some(8.5), Some(11.0)));
    assert!(page.shapes.is_empty());

    // Save the blank doc and reopen: still one empty Letter page.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("blank.vsdx");
    ed.save_as(&path).unwrap();
    let reopened = Editor::open(&path).unwrap();
    assert_eq!(reopened.doc.pages.len(), 1);
    assert!(reopened.doc.pages[0].shapes.is_empty());

    // And an edit on the fresh doc works + undoes.
    let page_id = ed.doc.pages[0].id;
    let label = ed
        .apply(dc_core::commands::SetCell::new(
            dc_core::Scope::PageSheet { page: page_id },
            "PageWidth",
            vsdx::model::Cell::value("6"),
        ))
        .unwrap();
    assert!(label.0.contains("PageWidth"));
    ed.undo().unwrap();
}

#[test]
fn resave_is_idempotent_and_leaves_no_staging_files() {
    // What is observable here: saving over an existing file repeatedly
    // keeps the destination a complete, re-openable package, and no
    // staging leftovers remain beside it. (Crash atomicity itself —
    // stage + rename never truncating in place — is craft-atomic's own
    // contract and cannot be observed without simulating a crash.)
    let mut ed = Editor::new_blank();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("atomic.vsdx");
    let page_id = ed.doc.pages[0].id;
    let _ = ed
        .apply(dc_core::commands::SetCell::new(
            dc_core::Scope::PageSheet { page: page_id },
            "PageWidth",
            vsdx::model::Cell::value("6"),
        ))
        .unwrap();
    for round in 0..2 {
        ed.save_as(&path).unwrap();
        let reopened = Editor::open(&path).unwrap();
        assert_eq!(
            reopened.doc.pages.len(),
            1,
            "round {round}: package complete"
        );
        assert!(
            std::fs::read(&path).unwrap().len() > 1000,
            "round {round}: non-trivial package"
        );
    }
    let leftovers: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy() != "atomic.vsdx")
        .collect();
    assert!(leftovers.is_empty(), "staging files left: {leftovers:?}");
}

#[test]
fn duplicate_shape_clones_offsets_and_undoes() {
    let mut ed = Editor::new_blank();
    let page = ed.doc.pages[0].id;
    let first = dc_core::commands::next_shape_id(&ed.doc, page);
    ed.apply(dc_core::AddShape {
        page,
        parent: None,
        shape: {
            let mut s = vsdx::model::Shape {
                id: first,
                ..Default::default()
            };
            s.sheet.set_cell("PinX", vsdx::model::Cell::value("2"));
            s.sheet.set_cell("PinY", vsdx::model::Cell::value("3"));
            s
        },
    })
    .unwrap();

    let new_id = dc_core::commands::next_shape_id(&ed.doc, page);
    ed.apply(dc_core::DuplicateShape::new(page, first, (0.25, -0.25)))
        .unwrap();
    let shapes = &ed.doc.pages.iter().find(|p| p.id == page).unwrap().shapes;
    assert_eq!(shapes.len(), 2);
    let dup = shapes.iter().find(|s| s.id == new_id).unwrap();
    let px = dup.sheet.cell("PinX").and_then(|c| c.numeric()).unwrap();
    let py = dup.sheet.cell("PinY").and_then(|c| c.numeric()).unwrap();
    assert!(
        (px - 2.25).abs() < 1e-9 && (py - 2.75).abs() < 1e-9,
        "nudged pin, got ({px},{py})"
    );

    ed.undo().unwrap();
    let shapes = &ed.doc.pages.iter().find(|p| p.id == page).unwrap().shapes;
    assert_eq!(shapes.len(), 1, "undo removes the duplicate only");
    assert_eq!(shapes[0].id, first);
}

#[test]
fn reorder_shape_moves_zorder_and_restores() {
    let mut ed = Editor::new_blank();
    let page = ed.doc.pages[0].id;
    let mut ids = [0i64; 3];
    for id in &mut ids {
        *id = dc_core::commands::next_shape_id(&ed.doc, page);
        let s = vsdx::model::Shape {
            id: *id,
            ..Default::default()
        };
        ed.apply(dc_core::AddShape {
            page,
            parent: None,
            shape: s,
        })
        .unwrap();
    }
    let order = |ed: &Editor| -> Vec<i64> {
        ed.doc
            .pages
            .iter()
            .find(|p| p.id == page)
            .unwrap()
            .shapes
            .iter()
            .map(|s| s.id)
            .collect()
    };
    assert_eq!(order(&ed), ids.to_vec());

    ed.apply(dc_core::ReorderShape::new(page, ids[0], true))
        .unwrap(); // bring to front
    assert_eq!(order(&ed), vec![ids[1], ids[2], ids[0]]);
    ed.apply(dc_core::ReorderShape::new(page, ids[0], false))
        .unwrap(); // send to back
    assert_eq!(order(&ed), vec![ids[0], ids[1], ids[2]]);

    ed.undo().unwrap();
    ed.undo().unwrap();
    assert_eq!(order(&ed), ids.to_vec(), "both reorder steps undo");
}

#[test]
fn set_row_cell_materializes_missing_character_section_with_undo() {
    let mut ed = Editor::new_blank();
    let page = ed.doc.pages[0].id;
    let sid = dc_core::commands::next_shape_id(&ed.doc, page);
    // A master-backed instance whose sheet carries NO local Character
    // section (the common case for placed shapes).
    let s = vsdx::model::Shape {
        id: sid,
        master: Some(7), // blank template master with Character rows
        ..Default::default()
    };
    ed.apply(dc_core::AddShape {
        page,
        parent: None,
        shape: s,
    })
    .unwrap();

    ed.apply(dc_core::SetRowCell::new(
        dc_core::Scope::Shape { page, shape: sid },
        "Character",
        0,
        dc_core::commands::RowSel::Ix(0),
        "Size",
        vsdx::model::Cell::value("0.1666"),
    ))
    .unwrap();

    // The local section/row now exist with the written value.
    let sh = vsdx::model::find_shape_by_id(&ed.doc.pages[0].shapes, sid).unwrap();
    let secs = sh.sheet.sections_named("Character");
    let sec = secs.first().expect("Character section materialized");
    let size = sec.rows[0].cell("Size").and_then(|c| c.numeric()).unwrap();
    assert!((size - 0.1666).abs() < 1e-9);

    // Undo removes the materialized row/section again.
    ed.undo().unwrap();
    let sh = vsdx::model::find_shape_by_id(&ed.doc.pages[0].shapes, sid).unwrap();
    assert!(
        sh.sheet.sections_named("Character").is_empty(),
        "materialized section removed on undo"
    );
}

/// A Macro whose child fails must leave the document untouched.
#[derive(Debug)]
struct Failing;

impl dc_core::Command for Failing {
    fn label(&self) -> String {
        "always fails".into()
    }
    fn apply(&mut self, _doc: &mut Document) -> anyhow::Result<()> {
        anyhow::bail!("boom")
    }
    fn undo(&mut self, _doc: &mut Document) -> anyhow::Result<()> {
        Ok(())
    }
}

#[test]
fn macro_rolls_back_applied_children_on_failure() {
    let mut ed = Editor::new_blank();
    let page = ed.doc.pages[0].id;
    let a = dc_core::commands::next_shape_id(&ed.doc, page);
    let sa = vsdx::model::Shape {
        id: a,
        ..Default::default()
    };
    ed.apply(dc_core::AddShape {
        page,
        parent: None,
        shape: sa,
    })
    .unwrap();

    let sb = vsdx::model::Shape {
        id: a + 1,
        ..Default::default()
    };
    let cmds: Vec<Box<dyn dc_core::Command>> = vec![
        Box::new(dc_core::RemoveShape::new(page, a)),
        Box::new(dc_core::AddShape {
            page,
            parent: None,
            shape: sb,
        }),
        Box::new(Failing),
    ];
    let undo_depth_before = ed.history.undo_labels().len();
    let result = ed.apply(dc_core::Macro::new("doomed", cmds));
    assert!(result.is_err(), "macro propagates the child error");

    // The removal rolled back: the original shape is back, nothing
    // half-applied remains, and no undo entry was pushed.
    let shapes = &ed.doc.pages.iter().find(|p| p.id == page).unwrap().shapes;
    assert_eq!(shapes.len(), 1, "rolled back to the single original shape");
    assert_eq!(shapes[0].id, a);
    assert_eq!(
        ed.history.undo_labels().len(),
        undo_depth_before,
        "failed macro left no undo entry"
    );
}

// ---------------------------------------------------------------------------
// ShapeSheet Actions execution (GitHub #25)
// ---------------------------------------------------------------------------

#[test]
fn shape_action_setf_chain_applies_and_one_undo_rolls_it_back() {
    use dc_core::commands::shape_action_commands;
    use std::collections::HashMap;

    let (ed, _tmp) = open_fixture();
    let mut ed = ed;
    let mut env = HashMap::new();
    env.insert("User.Style".to_string(), 0.0);
    let cmds = shape_action_commands(
        Scope::Shape { page: 0, shape: 1 },
        "SETF(GetRef(User.Style),1)&SETF(GetRef(Para.HorzAlign),1)&SETF(GetRef(VerticalAlign),1)",
        &env,
    )
    .unwrap();
    assert_eq!(cmds.len(), 3);
    ed.apply(dc_core::Macro::new("run shape action", cmds))
        .unwrap();

    // All three writes land on the INSTANCE sheet (row 1's shape).
    let sh = &ed.doc.pages[0].shapes[0];
    let user = sh
        .sheet
        .sections_named("User")
        .iter()
        .find_map(|s| s.named_row("Style"))
        .expect("User.Style materialized");
    assert_eq!(
        user.cell("Value").and_then(|c| c.v.clone()).as_deref(),
        Some("1")
    );
    let para = sh
        .sheet
        .sections_named("Paragraph")
        .iter()
        .find_map(|s| s.row_by_ix(0))
        .expect("Paragraph row materialized");
    assert_eq!(
        para.cell("HorzAlign").and_then(|c| c.v.clone()).as_deref(),
        Some("1")
    );
    assert_eq!(
        sh.sheet
            .cell("VerticalAlign")
            .and_then(|c| c.v.clone())
            .as_deref(),
        Some("1")
    );

    // One undo rolls the whole chain back (materialized rows removed).
    ed.undo().unwrap();
    let sh = &ed.doc.pages[0].shapes[0];
    assert!(sh
        .sheet
        .sections_named("User")
        .iter()
        .find_map(|s| s.named_row("Style"))
        .is_none());
}

#[test]
fn shape_action_values_expressions_strings_and_row_cell_suffixes() {
    use dc_core::commands::shape_action_commands;
    use std::collections::HashMap;

    let (ed, _tmp) = open_fixture();
    let mut ed = ed;
    // Chained values may be expressions.
    let mut env = HashMap::new();
    env.insert("User.N".to_string(), 3.0);
    let cmds = shape_action_commands(
        Scope::Shape { page: 0, shape: 1 },
        "SETF(GetRef(User.Style),User.N+1)",
        &env,
    )
    .unwrap();
    ed.apply(dc_core::Macro::new("expr", cmds)).unwrap();
    let sh = &ed.doc.pages[0].shapes[0];
    let user = sh
        .sheet
        .sections_named("User")
        .iter()
        .find_map(|s| s.named_row("Style"))
        .unwrap();
    assert_eq!(
        user.cell("Value").and_then(|c| c.v.clone()).as_deref(),
        Some("4")
    );
    ed.undo().unwrap();
    // A quoted string value is stored unquoted.
    let cmds = shape_action_commands(
        Scope::Shape { page: 0, shape: 1 },
        "SETF(GetRef(User.Style),\"Hello\")",
        &HashMap::new(),
    )
    .unwrap();
    ed.apply(dc_core::Macro::new("str", cmds)).unwrap();
    let sh = &ed.doc.pages[0].shapes[0];
    let user = sh
        .sheet
        .sections_named("User")
        .iter()
        .find_map(|s| s.named_row("Style"))
        .unwrap();
    assert_eq!(
        user.cell("Value").and_then(|c| c.v.clone()).as_deref(),
        Some("Hello"),
        "quoted string value stored unquoted"
    );
    ed.undo().unwrap();
    // A trailing cell suffix targets the row's named cell (not a row
    // literally named "Style.Prompt").
    let cmds = shape_action_commands(
        Scope::Shape { page: 0, shape: 1 },
        "SETF(GetRef(User.Style.Prompt),\"hint\")",
        &HashMap::new(),
    )
    .unwrap();
    ed.apply(dc_core::Macro::new("prompt", cmds)).unwrap();
    let sh = &ed.doc.pages[0].shapes[0];
    let user = sh
        .sheet
        .sections_named("User")
        .iter()
        .find_map(|s| s.named_row("Style"))
        .unwrap();
    assert_eq!(
        user.cell("Prompt").and_then(|c| c.v.clone()).as_deref(),
        Some("hint"),
        "Row.Cell suffix writes the row's named cell"
    );
}

#[test]
fn shape_action_unsupported_targets_defer_instead_of_writing() {
    use dc_core::commands::shape_action_commands;
    use std::collections::HashMap;

    let (_ed, _tmp) = open_fixture();
    let mut env = HashMap::new();
    env.insert("User.N".to_string(), 3.0);
    // RUNADDON is not a SETF chain: defer (Err).
    assert!(shape_action_commands(
        Scope::Shape { page: 0, shape: 1 },
        "RUNADDON(\"mystery\")",
        &env
    )
    .is_err());
    // A value that cannot be evaluated defers instead of persisting
    // formula text in the cell's V.
    assert!(shape_action_commands(
        Scope::Shape { page: 0, shape: 1 },
        "SETF(GetRef(User.Style),Width*2)",
        &HashMap::new(),
    )
    .is_err());
    // Unmapped dotted or sheet-qualified targets defer instead of
    // writing a bogus cell name.
    for bad in [
        "SETF(GetRef(Actions.Menu),\"x\")",
        "SETF(GetRef(ThePage!PageWidth),1)",
        "SETF(GetRef(User.A.B.C),1)",
    ] {
        assert!(
            shape_action_commands(Scope::Shape { page: 0, shape: 1 }, bad, &env).is_err(),
            "{bad} defers"
        );
    }
    // No Formula / empty: display-only, no commands.
    assert!(
        shape_action_commands(Scope::Shape { page: 0, shape: 1 }, "No Formula", &env)
            .unwrap()
            .is_empty()
    );
}

// ---------------------------------------------------------------------------
// Group / Ungroup (GitHub #26)
// ---------------------------------------------------------------------------

#[test]
fn group_and_ungroup_round_trip_with_undo() {
    use dc_core::commands::{GroupShapes, UngroupShape};

    let (ed, _tmp) = open_fixture();
    let mut ed = ed;
    // A second top-level shape to group with.
    let mut second = ed.doc.pages[0].shapes[0].clone();
    second.id = 5;
    second.sheet.set_cell("PinX", vsdx::model::Cell::value("6"));
    second.sheet.set_cell("PinY", vsdx::model::Cell::value("6"));
    ed.doc.pages[0].shapes.push(second);
    // The fixture page also carries shape 2 (a master instance) which
    // stays outside the group.
    let before = ed.doc.pages[0].shapes.len();
    assert_eq!(before, 3);

    // Shape 1 sits at PinX 4.25/PinY 5.5, W2 H1, LocPin (1, 0.5) →
    // rect (3.25, 5)-(5.25, 6). Shape 5 at (6,6) same size → (5, 5.5)-
    // (7, 6.5). Union: (3.25, 5)-(7, 6.5) → group Pin (3.25, 5),
    // W 3.75, H 1.5.
    ed.apply(GroupShapes::new(0, vec![5, 1])).unwrap();
    assert_eq!(
        ed.doc.pages[0].shapes.len(),
        2,
        "group + the ungrouped shape 2"
    );
    let g = &ed.doc.pages[0].shapes[0];
    assert_eq!(g.type_.as_deref(), Some("Group"));
    assert_eq!(g.sub_shapes.len(), 2);
    let n = |s: &vsdx::model::Sheet, k: &str| s.cell(k).and_then(|c| c.numeric()).unwrap_or(0.0);
    assert!((n(&g.sheet, "PinX") - 3.25).abs() < 1e-9);
    assert!((n(&g.sheet, "PinY") - 5.0).abs() < 1e-9);
    assert!((n(&g.sheet, "Width") - 3.75).abs() < 1e-9);
    assert!((n(&g.sheet, "Height") - 1.5).abs() < 1e-9);
    // Members in the group frame: shape 1 (4.25, 5.5) → (1, 0.5);
    // shape 5 (6, 6) → (2.75, 1).
    let m1 = g.sub_shapes.iter().find(|s| s.id == 1).unwrap();
    assert!((n(&m1.sheet, "PinX") - 1.0).abs() < 1e-9);
    assert!((n(&m1.sheet, "PinY") - 0.5).abs() < 1e-9);
    let m5 = g.sub_shapes.iter().find(|s| s.id == 5).unwrap();
    assert!((n(&m5.sheet, "PinX") - 2.75).abs() < 1e-9);
    assert!((n(&m5.sheet, "PinY") - 1.0).abs() < 1e-9);

    // Ungroup promotes with absolute Pins at the group's z-position.
    let gid = g.id;
    ed.apply(UngroupShape::new(0, gid)).unwrap();
    assert_eq!(ed.doc.pages[0].shapes.len(), 3);
    let s1 = ed.doc.pages[0].shapes.iter().find(|s| s.id == 1).unwrap();
    assert!((n(&s1.sheet, "PinX") - 4.25).abs() < 1e-9);
    assert!((n(&s1.sheet, "PinY") - 5.5).abs() < 1e-9);
    // Undo both directions.
    ed.undo().unwrap(); // undo ungroup → group back
    assert_eq!(ed.doc.pages[0].shapes.len(), 2);
    ed.undo().unwrap(); // undo group → original three, original order
    assert_eq!(ed.doc.pages[0].shapes.len(), 3);
    assert_eq!(ed.doc.pages[0].shapes[0].id, 1);
    let s1 = &ed.doc.pages[0].shapes[0];
    assert!((n(&s1.sheet, "PinX") - 4.25).abs() < 1e-9);

    // Save → reparse keeps the nested <Shapes> tree.
    ed.apply(GroupShapes::new(0, vec![1, 5])).unwrap();
    let out = _tmp.path().join("grouped.vsdx");
    ed.save_as(&out).unwrap();
    let re = vsdx::parse::parse(&out).unwrap();
    let g = &re.pages[0].shapes[0];
    assert_eq!(g.type_.as_deref(), Some("Group"));
    assert_eq!(g.sub_shapes.len(), 2, "nested tree survives save/reload");
}

/// A 1D member's endpoints are parent-frame points: grouping offsets
/// them into the group frame, ungroup maps them back through the
/// group's live Xform (a moved group moves them with it), and a
/// formula-driven endpoint refuses the grouping.
/// Append a 1D line (id 7) to page 0: endpoints (2, 5.25)→(6, 6.75)
/// with consistent derived Xform cells (Width/Angle/Pin/LocPin), the
/// way placement writes them. Returns its screen width.
fn push_1d_line(ed: &mut Editor) -> f64 {
    let w = (4.0f64).hypot(1.5);
    let mut line = ed.doc.pages[0].shapes[0].clone();
    line.id = 7;
    let cells: &[(&str, String)] = &[
        ("BeginX", "2".into()),
        ("BeginY", "5.25".into()),
        ("EndX", "6".into()),
        ("EndY", "6.75".into()),
        ("Width", format!("{w}")),
        ("Height", "0".into()),
        ("Angle", format!("{}", 1.5f64.atan2(4.0))),
        ("PinX", "4".into()),
        ("PinY", "6".into()),
        ("LocPinX", format!("{}", w / 2.0)),
        ("LocPinY", "0".into()),
    ];
    for (n, v) in cells {
        line.sheet.set_cell(n, vsdx::model::Cell::value(v.clone()));
    }
    ed.doc.pages[0].shapes.push(line);
    w
}

fn sheet_num(s: &vsdx::model::Shape, k: &str) -> f64 {
    s.sheet.cell(k).and_then(|c| c.numeric()).unwrap_or(f64::NAN)
}

#[test]
fn group_converts_1d_endpoints_to_the_group_frame_and_back() {
    use dc_core::commands::{GroupShapes, UngroupShape};

    let (ed, _tmp) = open_fixture();
    let mut ed = ed;
    let w = push_1d_line(&mut ed);

    // Union bbox with shape 1's rect (3.25, 5)-(5.25, 6): the line's
    // rect runs (4 − w/2, 6)-(4 + w/2, 6) → x0 = 4 − w/2, y0 = 5.
    let x0 = 4.0 - w / 2.0;
    ed.apply(GroupShapes::new(0, vec![1, 7])).unwrap();
    let g = ed.doc.pages[0].shapes.iter().find(|s| s.sub_shapes.len() == 2).unwrap();
    let gid = g.id;
    let line = g.sub_shapes.iter().find(|s| s.id == 7).unwrap();
    for (cell, page_val) in [("BeginX", 2.0), ("BeginY", 5.25), ("EndX", 6.0), ("EndY", 6.75)] {
        let off = if cell.ends_with('X') { x0 } else { 5.0 };
        assert!(
            (sheet_num(line, cell) - (page_val - off)).abs() < 1e-9,
            "{cell} converted to the group frame: {} vs {}",
            sheet_num(line, cell),
            page_val - off
        );
    }

    // Ungroup (group unmoved): endpoints return to page values.
    ed.apply(UngroupShape::new(0, gid)).unwrap();
    let line = ed.doc.pages[0].shapes.iter().find(|s| s.id == 7).unwrap();
    for (cell, page_val) in [("BeginX", 2.0), ("BeginY", 5.25), ("EndX", 6.0), ("EndY", 6.75)] {
        assert!((sheet_num(line, cell) - page_val).abs() < 1e-9, "{cell} restored");
    }
}

#[test]
fn ungroup_restores_endpoints_moved_with_the_group() {
    use dc_core::commands::{GroupShapes, SetCell, UngroupShape};

    let (ed, _tmp) = open_fixture();
    let mut ed = ed;
    push_1d_line(&mut ed);
    ed.apply(GroupShapes::new(0, vec![1, 7])).unwrap();
    let g = ed.doc.pages[0].shapes.iter().find(|s| s.sub_shapes.len() == 2).unwrap();
    let gid = g.id;
    let (gpx, gpy) = (sheet_num(g, "PinX"), sheet_num(g, "PinY"));
    // Move the group through the command layer, not by poking its sheet.
    ed.apply(SetCell::new(
        Scope::Shape { page: 0, shape: gid },
        "PinX",
        vsdx::model::Cell::value(format!("{}", gpx + 2.0)),
    ))
    .unwrap();
    ed.apply(SetCell::new(
        Scope::Shape { page: 0, shape: gid },
        "PinY",
        vsdx::model::Cell::value(format!("{}", gpy + 1.0)),
    ))
    .unwrap();
    ed.apply(UngroupShape::new(0, gid)).unwrap();
    let line = ed.doc.pages[0].shapes.iter().find(|s| s.id == 7).unwrap();
    for (cell, moved_val) in [
        ("BeginX", 4.0),
        ("BeginY", 6.25),
        ("EndX", 8.0),
        ("EndY", 7.75),
    ] {
        assert!(
            (sheet_num(line, cell) - moved_val).abs() < 1e-9,
            "{cell} followed the moved group: {} vs {moved_val}",
            sheet_num(line, cell)
        );
    }
}

#[test]
fn ungroup_composes_a_rotated_group_angle_into_members() {
    use dc_core::commands::{GroupShapes, SetCell, UngroupShape};

    let (ed, _tmp) = open_fixture();
    let mut ed = ed;
    push_1d_line(&mut ed);
    // A rotated group composes its Angle into the members: positions
    // orbit around the group's pin AND orientations rotate with it.
    ed.apply(GroupShapes::new(0, vec![1, 7])).unwrap();
    let g = ed.doc.pages[0]
        .shapes
        .iter_mut()
        .find(|s| s.sub_shapes.len() == 2)
        .unwrap();
    let gid = g.id;
    let (gpx, gpy, glx, gly) = (
        sheet_num(g, "PinX"),
        sheet_num(g, "PinY"),
        sheet_num(g, "LocPinX"),
        sheet_num(g, "LocPinY"),
    );
    // Member-local values BEFORE the rotation lands (they were
    // converted into the group frame by grouping).
    let m7 = g.sub_shapes.iter().find(|s| s.id == 7).unwrap();
    let (b_local, begin_y_local) = (sheet_num(m7, "BeginX"), sheet_num(m7, "BeginY"));
    ed.apply(SetCell::new(
        Scope::Shape { page: 0, shape: gid },
        "Angle",
        vsdx::model::Cell::value(format!("{}", std::f64::consts::FRAC_PI_2)),
    ))
    .unwrap();
    ed.apply(UngroupShape::new(0, gid)).unwrap();
    let line = ed.doc.pages[0].shapes.iter().find(|s| s.id == 7).unwrap();
    let rect = ed.doc.pages[0].shapes.iter().find(|s| s.id == 1).unwrap();
    // R(90°): page = Pin + R·(p − LocPin), (dx,dy) → (−dy,dx).
    assert!(
        (sheet_num(line, "BeginX") - (gpx - (begin_y_local - gly))).abs() < 1e-9,
        "BeginX rotated with the group"
    );
    assert!(
        (sheet_num(line, "BeginY") - (gpy + (b_local - glx))).abs() < 1e-9,
        "BeginY rotated with the group"
    );
    let line_ang0 = 1.5f64.atan2(4.0);
    assert!(
        (sheet_num(line, "Angle") - (line_ang0 + std::f64::consts::FRAC_PI_2)).abs() < 1e-9,
        "line Angle composes group + own: {} vs {}",
        sheet_num(line, "Angle"),
        line_ang0 + std::f64::consts::FRAC_PI_2
    );
    assert!((sheet_num(rect, "Angle") - std::f64::consts::FRAC_PI_2).abs() < 1e-9);
}

#[test]
fn grouping_refuses_a_formula_driven_endpoint_without_mutating() {
    use dc_core::commands::GroupShapes;

    let (ed, _tmp) = open_fixture();
    let mut ed = ed;
    push_1d_line(&mut ed);
    // A formula-driven endpoint cannot be offset: grouping refuses and
    // the document is untouched.
    let line = ed
        .doc
        .pages[0]
        .shapes
        .iter_mut()
        .find(|s| s.id == 7)
        .unwrap();
    let mut f_cell = vsdx::model::Cell::value("2");
    f_cell.f = Some("Width*0.5".into());
    line.sheet.set_cell("BeginX", f_cell);
    let before = ed.doc.pages[0].shapes.len();
    let err = ed.apply(GroupShapes::new(0, vec![1, 7]));
    assert!(err.is_err(), "formula endpoint refuses grouping");
    assert_eq!(
        ed.doc.pages[0].shapes.len(),
        before,
        "refused grouping mutated nothing"
    );
}

// ---------------------------------------------------------------------------
// Containers (GitHub #27)
// ---------------------------------------------------------------------------

#[test]
fn add_container_records_membership_and_survives_reload() {
    use dc_core::commands::{add_container, next_shape_id};

    let (ed, tmp) = open_fixture();
    let mut ed = ed;
    let id = next_shape_id(&ed.doc, 0);
    // Wrap shapes 1 and 2 (fixture rect at (3.25,5)-(5.25,6) and the
    // master instance around (2,2)) with a rect of our own.
    ed.apply(add_container(
        0,
        id,
        &[1, 2],
        (1.0, 1.0, 6.0, 6.0),
        "Legend",
    ))
    .unwrap();

    // Container at the BACK of the z-order, members still top-level.
    assert_eq!(ed.doc.pages[0].shapes[0].id, id);
    assert_eq!(ed.doc.pages[0].shapes.len(), 3);
    let member_row = |ed: &dc_core::Editor, sid: i64| {
        ed.doc.pages[0]
            .shapes
            .iter()
            .find(|s| s.id == sid)
            .unwrap()
            .sheet
            .sections_named("User")
            .iter()
            .find_map(|sec| sec.named_row("msvSDContainerShape"))
            .and_then(|r| r.cell("Value"))
            .and_then(|c| c.v.clone())
    };
    assert_eq!(member_row(&ed, 1).as_deref(), Some(id.to_string().as_str()));
    assert_eq!(member_row(&ed, 2).as_deref(), Some(id.to_string().as_str()));

    // One undo removes container AND membership rows.
    ed.undo().unwrap();
    assert_eq!(ed.doc.pages[0].shapes.len(), 2);
    assert!(
        member_row(&ed, 1).is_none(),
        "membership row removed with the container"
    );

    // Re-apply, save, reparse: membership rides along.
    ed.redo().unwrap();
    let out = tmp.path().join("container.vsdx");
    ed.save_as(&out).unwrap();
    let re = vsdx::parse::parse(&out).unwrap();
    let c = &re.pages[0].shapes[0];
    assert_eq!(c.id, id);
    assert_eq!(c.sheet.text.as_deref(), Some("Legend"));
    let m = re.pages[0]
        .shapes
        .iter()
        .find(|s| s.id == 1)
        .unwrap()
        .sheet
        .sections_named("User")
        .iter()
        .find_map(|sec| sec.named_row("msvSDContainerShape"))
        .and_then(|r| r.cell("Value"))
        .and_then(|c| c.v.clone());
    assert_eq!(m.as_deref(), Some(id.to_string().as_str()));
}

// ---------------------------------------------------------------------------
// Hyperlink rows (GitHub #28)
// ---------------------------------------------------------------------------

#[test]
fn hyperlink_row_add_edit_remove_round_trip() {
    use dc_core::commands::{RemoveSectionRow, RowSel};
    use dc_core::{Macro, SetRowCell};

    let (ed, _tmp) = open_fixture();
    let mut ed = ed;
    let scope = Scope::Shape { page: 0, shape: 1 };
    let row = |ix: i64, name: &str, v: &str| {
        SetRowCell::new(
            scope.clone(),
            "Hyperlink",
            0,
            RowSel::Ix(ix),
            name,
            vsdx::model::Cell::value(v.to_string()),
        )
    };
    ed.apply(Macro::new(
        "add hyperlink",
        vec![
            Box::new(row(0, "Address", "https://example.test/")),
            Box::new(row(0, "Description", "Docs")),
        ],
    ))
    .unwrap();
    let link = |ed: &dc_core::Editor| {
        ed.doc.pages[0].shapes[0]
            .sheet
            .sections_named("Hyperlink")
            .iter()
            .flat_map(|s| s.rows.iter())
            .map(|r| {
                (
                    r.cell("Address").and_then(|c| c.v.clone()),
                    r.cell("Description").and_then(|c| c.v.clone()),
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(
        link(&ed),
        vec![(Some("https://example.test/".into()), Some("Docs".into()))]
    );
    // One undo removes the whole row materialization.
    ed.undo().unwrap();
    assert!(link(&ed).is_empty());
    ed.redo().unwrap();

    // RemoveSectionRow + undo.
    ed.apply(RemoveSectionRow::new(
        scope.clone(),
        "Hyperlink",
        0,
        RowSel::Ix(0),
    ))
    .unwrap();
    assert!(link(&ed).is_empty());
    ed.undo().unwrap();
    assert_eq!(link(&ed).len(), 1);
}

#[test]
fn remove_shape_touch_targets_the_removed_shape_not_its_successor() {
    let mut ed = Editor::new_blank();
    let page = ed.doc.pages[0].id;
    let shape = |id: i64, ty: &str| {
        let mut s = Shape {
            id,
            type_: Some(ty.into()),
            ..Shape::default()
        };
        s.sheet.set_cell("PinX", Cell::value("1"));
        s
    };
    for (id, parent) in [(1, None), (2, None), (3, Some(2)), (4, None)] {
        ed.apply(AddShape {
            page,
            parent,
            shape: shape(id, if id == 2 { "Group" } else { "Shape" }),
        })
        .unwrap();
    }
    // page.shapes = [1, Group(2)[3], 4].
    //
    // Top-level removal from the MIDDLE: the touch is the removed
    // shape's own Membership. (The old classification ran the nested
    // walk, read the post-removal slot 0 — now the group — and
    // touched the group's Ink instead.)
    let (_, t) = ed.apply(RemoveShape::new(page, 1)).unwrap();
    assert_eq!(t.shapes.len(), 1, "one touched shape: {t:?}");
    assert_eq!(t.shapes[0].shape, 1, "the removed shape, not its successor");
    assert!(matches!(t.shapes[0].touch, dc_core::Touch::Membership));

    // Nested removal: the top-level ROOT's Ink (the whole subtree
    // layer re-rasters), not the member itself.
    let (_, t) = ed.apply(RemoveShape::new(page, 3)).unwrap();
    assert_eq!(t.shapes.len(), 1, "one touched shape: {t:?}");
    assert_eq!(t.shapes[0].shape, 2, "the member's top-level root");
    assert!(matches!(t.shapes[0].touch, dc_core::Touch::Ink));

    // Pre-apply (no captured path): nothing precise is known — the
    // whole page invalidates.
    let cmd = RemoveShape::new(page, 4);
    let t = <RemoveShape as dc_core::Command>::touched(&cmd, &ed.doc);
    assert_eq!(t.pages, vec![page]);
    assert!(t.shapes.is_empty());
}

// ---------------------------------------------------------------------------
// Place-from-stencil: ImportMaster + AddMasterInstance (#84)
// ---------------------------------------------------------------------------

/// A synthetic STENCIL master like one parsed from an open `.vssx`:
/// its ids, parts, and rIds belong to the stencil's own namespace and
/// deliberately collide with the document's (master 7, master1.xml,
/// rId1) — the import must re-key all of them. `BaseID` is the
/// cross-document identity Visio dedupes on.
fn stencil_master(base_id: &str, name: &str) -> vsdx::model::Master {
    use vsdx::model::*;
    let mut sheet = Sheet::default();
    for (n, v) in [("Width", "1"), ("Height", "1")] {
        sheet.set_cell(n, Cell::value(v));
    }
    let row = |t: &str, ix: i64, x: &str, y: &str| Row {
        t: Some(t.into()),
        ix,
        n: None,
        attrs: vec![("T".into(), t.into()), ("IX".into(), ix.to_string())],
        cells: vec![("X".into(), Cell::value(x)), ("Y".into(), Cell::value(y))],
    };
    sheet.children.push(SheetChild::Section(Section {
        name: "Geometry".into(),
        ix: 0,
        attrs: vec![("N".into(), "Geometry".into()), ("IX".into(), "0".into())],
        cells: vec![],
        rows: vec![
            row("MoveTo", 1, "0", "0"),
            row("LineTo", 2, "1", "0"),
            row("LineTo", 3, "1", "1"),
            row("LineTo", 4, "0", "1"),
        ],
    }));
    let mut m = Master {
        id: 7, // collides with the document's own master 7
        name_u: Some(name.into()),
        attrs: vec![
            ("ID".into(), "7".into()),
            ("NameU".into(), name.into()),
            ("Name".into(), name.into()),
            ("BaseID".into(), base_id.into()),
            ("MatchByName".into(), "1".into()),
        ],
        // The stencil's rId1 belongs to the stencil's rels; the import
        // must drop it (the writer assigns a fresh document rId).
        children: vec![IndexChild::Rel(vec![("r:id".into(), "rId1".into())])],
        shapes: vec![Shape {
            id: 1,
            type_: Some("Shape".into()),
            line_style: Some(0),
            fill_style: Some(0),
            text_style: Some(0),
            attrs: vec![
                ("ID".into(), "1".into()),
                ("Type".into(), "Shape".into()),
                ("LineStyle".into(), "0".into()),
                ("FillStyle".into(), "0".into()),
                ("TextStyle".into(), "0".into()),
            ],
            sheet,
            ..Shape::default()
        }],
        part: "visio/masters/master1.xml".into(), // collides too
        ..Master::default()
    };
    let _ = &mut m;
    m
}

#[test]
fn place_from_stencil_imports_dedupes_and_undoes_as_one() {
    let (mut ed, tmp) = open_fixture();
    assert_eq!(ed.doc.masters.len(), 1, "fixture carries one master (id 7)");
    let page_shapes = |ed: &Editor| ed.doc.pages[0].shapes.len();
    assert_eq!(page_shapes(&ed), 2);

    // 1) First drop imports a re-keyed copy (+1 master) and places the
    //    instance referencing the document-local copy.
    let src = stencil_master("{S1}", "Star");
    ed.apply(dc_core::place_master_from_stencil(&ed.doc, 0, &src, (3.0, 3.0)))
        .unwrap();
    assert_eq!(ed.doc.masters.len(), 2, "first drop imports the master");
    assert_eq!(page_shapes(&ed), 3);
    {
        let imported = ed.doc.masters.last().unwrap();
        assert_ne!(imported.id, 7, "fresh document-local id (stencil id 7 collides)");
        assert_eq!(imported.id, 8);
        assert_eq!(imported.part, "visio/masters/master2.xml", "document-unique part");
        assert!(
            imported
                .children
                .iter()
                .all(|c| !matches!(c, IndexChild::Rel(_))),
            "the stencil's borrowed Rel child is dropped"
        );
        assert_eq!(imported.name_u.as_deref(), Some("Star"), "NameU travels verbatim");
        assert_eq!(
            imported
                .attrs
                .iter()
                .find(|(k, _)| k == "BaseID")
                .map(|(_, v)| v.as_str()),
            Some("{S1}"),
            "BaseID travels verbatim (the dedupe key)"
        );
    }
    let imported_id = ed.doc.masters.last().unwrap().id;
    let placed = ed.doc.pages[0].shapes.last().unwrap();
    assert_eq!(placed.master, Some(imported_id), "the instance references the local copy");

    // 2) A second drop of the same master dedupes: reuse, no new master.
    ed.apply(dc_core::place_master_from_stencil(&ed.doc, 0, &src, (5.0, 5.0)))
        .unwrap();
    assert_eq!(ed.doc.masters.len(), 2, "re-drop reuses the imported copy");
    assert_eq!(page_shapes(&ed), 4);
    let second = ed.doc.pages[0].shapes.last().unwrap();
    assert_eq!(second.master, Some(imported_id));

    // 3) One undo reverts exactly one placement step; the second
    //    placement's undo removes only its shape, the first's removes
    //    shape AND imported master together.
    ed.undo().unwrap();
    assert_eq!(page_shapes(&ed), 3, "second placement undone alone");
    assert_eq!(ed.doc.masters.len(), 2, "imported master still in use");
    ed.undo().unwrap();
    assert_eq!(page_shapes(&ed), 2, "first placement undone");
    assert_eq!(ed.doc.masters.len(), 1, "…including its imported master");

    // 4) Redo restores both steps exactly.
    ed.redo().unwrap();
    ed.redo().unwrap();
    assert_eq!(ed.doc.masters.len(), 2);
    assert_eq!(page_shapes(&ed), 4);
    assert_eq!(ed.doc.masters.last().unwrap().id, 8, "redo re-imports with the same id");

    // 5) Save → reload keeps the imported master and its part.
    let out = tmp.path().join("placed.vsdx");
    ed.save_as(&out).unwrap();
    let pkg = vsdx::Package::open(&out).unwrap();
    let names = pkg.part_names();
    assert!(
        names.contains(&"visio/masters/master2.xml"),
        "the imported master's part was materialized: {names:?}"
    );
    let rels = String::from_utf8(
        pkg.part("visio/masters/_rels/masters.xml.rels").unwrap().to_vec(),
    )
    .unwrap();
    assert!(
        rels.contains("master2.xml"),
        "masters.xml.rels gained the imported master's relationship: {rels}"
    );
    let reloaded = Editor::open(&out).unwrap();
    assert_eq!(reloaded.doc.masters.len(), 2, "reload keeps the imported master");
    assert_eq!(reloaded.doc.masters.last().unwrap().name_u.as_deref(), Some("Star"));
    assert_eq!(reloaded.doc.pages[0].shapes.len(), 4, "placed shapes survive");
}

// ---------------------------------------------------------------------------
// #97 §6 item 5: the apply error contract — every refused command
// leaves the document untouched and pushes no history entry.
// ---------------------------------------------------------------------------

#[test]
fn refused_commands_never_mutate_or_push_history() {
    let (ed, _tmp) = open_fixture();
    let mut ed = ed;
    let page0 = ed.doc.pages[0].id;
    let shape_ids = |ed: &Editor| {
        format!(
            "{:?}",
            ed.doc.pages[0]
                .shapes
                .iter()
                .map(|s| (s.id, s.sub_shapes.len()))
                .collect::<Vec<_>>()
        )
    };
    let before = shape_ids(&ed);

    // Unknown shape scope.
    assert!(ed
        .apply(dc_core::commands::SetCell::new(
            dc_core::Scope::Shape { page: page0, shape: 99 },
            "PinX",
            vsdx::model::Cell::value("1"),
        ))
        .is_err());
    // Page position out of range.
    assert!(ed.apply(MovePage { from: 0, to: 9 }).is_err());
    // GroupShapes needs at least two members.
    assert!(ed.apply(GroupShapes::new(page0, vec![1])).is_err());
    // Ungrouping a shape with no members (a non-group) refuses.
    assert!(ed.apply(UngroupShape::new(page0, 1)).is_err());
    // Connection index out of range.
    assert!(ed
        .apply(RemoveConnect::new(ConnectsOwner::Page(page0), 9))
        .is_err());

    assert_eq!(
        shape_ids(&ed),
        before,
        "a refused command mutated the document"
    );
    assert!(
        ed.undo().unwrap().is_none(),
        "a refused command pushed a history entry"
    );
}

#[test]
fn add_connect_on_a_partless_page_refuses() {
    // A stencil-like page with no contents part would silently lose the
    // connection on save; the command refuses instead.
    let mut ed = Editor::new(bare_doc());
    let page0 = ed.doc.pages[0].id;
    let r = ed.apply(AddConnect {
        owner: ConnectsOwner::Page(page0),
        connect: Connect {
            from_sheet: 1,
            from_cell: "Begin".into(),
            from_part: 3,
            to_sheet: 2,
            to_cell: "End".into(),
            to_part: 3,
            extra: Vec::new(),
        },
    });
    assert!(r.is_err(), "partless page must refuse connection edits");
    assert!(ed.undo().unwrap().is_none());
}

// ---------------------------------------------------------------------------
// #97 §6 item 6: GUARD/Protection refusals live in the command layer —
// the GUI gestures AND `dc edit` inherit the same semantics. A plain
// value write is refused; a write carrying a formula (ShapeSheet edit,
// `--formula`, the cache refresh's F='Inh' snapshots) is the escape
// hatch.
// ---------------------------------------------------------------------------

#[test]
fn set_cell_refuses_plain_value_writes_to_guarded_cells() {
    let (ed, _tmp) = open_fixture();
    let mut ed = ed;
    let page0 = ed.doc.pages[0].id;
    // Author a GUARDed Width the way shipped files carry it.
    {
        let sh = &mut ed.doc.pages[0].shapes[0];
        let mut g = vsdx::model::Cell::value("2");
        g.f = Some("GUARD(2 in)".into());
        sh.sheet.set_cell("Width", g);
    }

    // Plain value write: refused, cell untouched, no history entry.
    let err = ed.apply(dc_core::commands::SetCell::new(
        dc_core::Scope::Shape { page: page0, shape: 1 },
        "Width",
        vsdx::model::Cell::value("3"),
    ));
    assert!(err.is_err(), "plain write to a GUARDed cell must refuse");
    assert_eq!(
        ed.doc.pages[0].shapes[0]
            .sheet
            .cell("Width")
            .and_then(|c| c.numeric()),
        Some(2.0),
        "refused write changed the guarded cell"
    );
    assert!(ed.undo().unwrap().is_none());

    // A formula write is the escape hatch (ShapeSheet edit / --formula).
    let mut f = vsdx::model::Cell::default();
    f.f = Some("Width*2".into());
    ed.apply(dc_core::commands::SetCell::new(
        dc_core::Scope::Shape { page: page0, shape: 1 },
        "Width",
        f,
    ))
    .unwrap();

    // An F='Inh' cache-refresh write lands under the guard too (the
    // flattened-group refresh re-evaluates guarded master formulas).
    let mut inh = vsdx::model::Cell::default();
    inh.v = Some("2".into());
    inh.f = Some("Inh".into());
    ed.apply(dc_core::commands::SetCell::new(
        dc_core::Scope::Shape { page: page0, shape: 1 },
        "Width",
        inh,
    ))
    .unwrap();
}

#[test]
fn set_cell_refuses_through_an_inh_snapshot_behind_a_master_guard() {
    // resolve_cell prefers an instance xform cell's cached V under
    // F='Inh' — the master's GUARD behind the snapshot must still
    // protect the cell (a plain `dc edit --cell` write used to clobber
    // it; found by the #125 review).
    let (ed, _tmp) = open_fixture();
    let mut ed = ed;
    let page0 = ed.doc.pages[0].id;
    {
        // Instance shape 2 (master-backed): a cached Width snapshot.
        let sh = vsdx::model::find_shape_by_id_mut(&mut ed.doc.pages.iter_mut().next().unwrap().shapes, 2);
        let mut snap = vsdx::model::Cell::value("1");
        snap.f = Some("Inh".into());
        sh.unwrap().sheet.set_cell("Width", snap);
        // And the master guards the same cell.
        let m = ed.doc.masters.iter_mut().find(|m| m.id == 7).unwrap();
        let root = m.shapes.first_mut().unwrap();
        let mut g = vsdx::model::Cell::value("2");
        g.f = Some("GUARD(2 in)".into());
        root.sheet.set_cell("Width", g);
    }
    let err = ed.apply(dc_core::commands::SetCell::new(
        dc_core::Scope::Shape { page: page0, shape: 2 },
        "Width",
        vsdx::model::Cell::value("9"),
    ));
    assert!(err.is_err(), "the master GUARD behind an Inh snapshot must refuse");
    assert_eq!(
        ed.doc.pages[0]
            .shapes
            .iter()
            .find(|s| s.id == 2)
            .unwrap()
            .sheet
            .cell("Width")
            .and_then(|c| c.numeric()),
        Some(1.0),
        "the snapshot value is untouched"
    );
}

#[test]
fn set_cell_refuses_locked_axes_via_protection_rows() {
    let (ed, _tmp) = open_fixture();
    let mut ed = ed;
    let page0 = ed.doc.pages[0].id;
    // Author LockHeight in the Protection section (instance side).
    {
        let sh = &mut ed.doc.pages[0].shapes[0];
        sh.sheet.children.push(vsdx::model::SheetChild::Section(
            vsdx::model::Section {
                name: "Protection".into(),
                rows: vec![vsdx::model::Row {
                    cells: vec![(
                        "LockHeight".into(),
                        vsdx::model::Cell::value("1"),
                    )],
                    ..vsdx::model::Row::default()
                }],
                ..vsdx::model::Section::default()
            },
        ));
    }

    let scope = dc_core::Scope::Shape { page: page0, shape: 1 };
    // The locked axis refuses.
    assert!(ed
        .apply(dc_core::commands::SetCell::new(
            scope.clone(),
            "Height",
            vsdx::model::Cell::value("3"),
        ))
        .is_err());
    // The unlocked axis still writes.
    ed.apply(dc_core::commands::SetCell::new(
        scope.clone(),
        "Width",
        vsdx::model::Cell::value("4"),
    ))
    .unwrap();
    // Writing the lock cell itself is allowed — that is how a shape is
    // unlocked.
    ed.apply(dc_core::commands::SetCell::new(
        scope,
        "LockHeight",
        vsdx::model::Cell::value("0"),
    ))
    .unwrap();
}

#[test]
fn master_side_guards_and_locks_protect_instances() {
    let (ed, _tmp) = open_fixture();
    let mut ed = ed;
    let page0 = ed.doc.pages[0].id;
    // Shape 2 (BoxFromMaster) inherits from master 7: a GUARD in the
    // master's Width protects the instance the same way the GUI's
    // gesture-level check treats it.
    {
        let m = ed.doc.masters.iter_mut().find(|m| m.id == 7).unwrap();
        let root = m.shapes.first_mut().unwrap();
        let mut g = vsdx::model::Cell::value("1");
        g.f = Some("GUARD(1 in)".into());
        root.sheet.set_cell("Width", g);
    }
    assert!(ed
        .apply(dc_core::commands::SetCell::new(
            dc_core::Scope::Shape { page: page0, shape: 2 },
            "Width",
            vsdx::model::Cell::value("9"),
        ))
        .is_err());
    // LockWidth on the master locks the instance's axis as well.
    {
        let m = ed.doc.masters.iter_mut().find(|m| m.id == 7).unwrap();
        let root = m.shapes.first_mut().unwrap();
        root.sheet.set_cell("LockWidth", vsdx::model::Cell::value("1"));
    }
    assert!(ed
        .apply(dc_core::commands::SetCell::new(
            dc_core::Scope::Shape { page: page0, shape: 2 },
            "Width",
            vsdx::model::Cell::value("9"),
        ))
        .is_err());
}
