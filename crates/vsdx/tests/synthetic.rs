//! Integration test: build a synthetic VSDX package in a temp file (no
//! Microsoft content), parse it, and check the model + inheritance.

use std::io::Write;

fn write_package(dir: &std::path::Path) -> std::path::PathBuf {
    let path = dir.join("synthetic.vsdx");
    let file = std::fs::File::create(&path).unwrap();
    let mut w = zip::ZipWriter::new(file);
    let opts = zip::write::SimpleFileOptions::default();

    let content_types = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
<Default Extension="xml" ContentType="application/xml"/>
<Override PartName="/visio/document.xml" ContentType="application/vnd.ms-visio.drawing.main+xml"/>
<Override PartName="/visio/pages/pages.xml" ContentType="application/vnd.ms-visio.pages+xml"/>
<Override PartName="/visio/pages/page1.xml" ContentType="application/vnd.ms-visio.pages+xml"/>
<Override PartName="/visio/masters/masters.xml" ContentType="application/vnd.ms-visio.masters+xml"/>
<Override PartName="/visio/masters/master1.xml" ContentType="application/vnd.ms-visio.master+xml"/>
</Types>"#;

    let root_rels = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.microsoft.com/visio/2010/relationships/document" Target="visio/document.xml"/>
</Relationships>"#;

    let document = r#"<?xml version="1.0" encoding="utf-8"?>
<VisioDocument xmlns='http://schemas.microsoft.com/office/visio/2012/main' xmlns:r='http://schemas.openxmlformats.org/officeDocument/2006/relationships'>
<Colors><ColorEntry IX='24' RGB='#7F7F7F'/></Colors>
<StyleSheets>
<StyleSheet ID='0' NameU='No Style'>
  <Cell N='LineWeight' V='0.01041666666666667'/>
  <Cell N='LineColor' V='0'/>
  <Cell N='FillForegnd' V='1'/>
  <Cell N='FillPattern' V='1'/>
</StyleSheet>
</StyleSheets>
</VisioDocument>"#;

    let document_rels = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.microsoft.com/visio/2010/relationships/pages" Target="pages/pages.xml"/>
<Relationship Id="rId2" Type="http://schemas.microsoft.com/visio/2010/relationships/masters" Target="masters/masters.xml"/>
</Relationships>"#;

    let pages = r#"<?xml version="1.0" encoding="utf-8"?>
<Pages xmlns='http://schemas.microsoft.com/office/visio/2012/main' xmlns:r='http://schemas.openxmlformats.org/officeDocument/2006/relationships'>
<Page ID='0' NameU='Page-1' Name='Page-1'>
<PageSheet LineStyle='0' FillStyle='0' TextStyle='0'>
  <Cell N='PageWidth' V='8.5'/>
  <Cell N='PageHeight' V='11.0'/>
</PageSheet>
<Rel r:id='rId1'/>
</Page>
</Pages>"#;

    let pages_rels = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.microsoft.com/visio/2010/relationships/page" Target="page1.xml"/>
</Relationships>"#;

    let page1 = r#"<?xml version="1.0" encoding="utf-8"?>
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
  <Cell N='PinX' V='2.0'/><Cell N='PinY' V='2.0'/>
  <Cell N='LineWeight' V='0.03125' F='Inh'/>
  <Shapes>
    <Shape ID='4' Type='Shape' MasterShape='5'>
      <Cell N='PinX' V='0.5'/>
    </Shape>
  </Shapes>
</Shape>
</Shapes>
</PageContents>"#;

    let masters = r#"<?xml version="1.0" encoding="utf-8"?>
<Masters xmlns='http://schemas.microsoft.com/office/visio/2012/main' xmlns:r='http://schemas.openxmlformats.org/officeDocument/2006/relationships'>
<Master ID='7' NameU='Master box' r:id='rId1'/>
</Masters>"#;

    let masters_rels = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.microsoft.com/visio/2010/relationships/master" Target="master1.xml"/>
</Relationships>"#;

    let master1 = r#"<?xml version="1.0" encoding="utf-8"?>
<MasterContents xmlns='http://schemas.microsoft.com/office/visio/2012/main' xmlns:r='http://schemas.openxmlformats.org/officeDocument/2006/relationships'>
<Shapes>
<Shape ID='5' Type='Shape' LineStyle='0' FillStyle='0' TextStyle='0'>
  <Cell N='Width' V='1.5'/><Cell N='Height' V='0.75'/>
  <Cell N='LineWeight' V='0.015625'/>
  <Section N='Geometry' IX='0'>
    <Row T='MoveTo' IX='1'><Cell N='X' V='0'/><Cell N='Y' V='0'/></Row>
    <Row T='LineTo' IX='2'><Cell N='X' V='1.5'/><Cell N='Y' V='0'/></Row>
    <Row T='LineTo' IX='3'><Cell N='X' V='1.5'/><Cell N='Y' V='0.75'/></Row>
    <Row T='LineTo' IX='4'><Cell N='X' V='0'/><Cell N='Y' V='0.75'/></Row>
    <Row T='LineTo' IX='5'><Cell N='X' V='0'/><Cell N='Y' V='0'/></Row>
  </Section>
</Shape>
</Shapes>
</MasterContents>"#;

    for (name, data) in [
        ("[Content_Types].xml", content_types),
        ("_rels/.rels", root_rels),
        ("visio/document.xml", document),
        ("visio/_rels/document.xml.rels", document_rels),
        ("visio/pages/pages.xml", pages),
        ("visio/pages/_rels/pages.xml.rels", pages_rels),
        ("visio/pages/page1.xml", page1),
        ("visio/masters/masters.xml", masters),
        ("visio/masters/_rels/masters.xml.rels", masters_rels),
        ("visio/masters/master1.xml", master1),
    ] {
        w.start_file(name, opts).unwrap();
        w.write_all(data.as_bytes()).unwrap();
    }
    w.finish().unwrap();
    path
}

#[test]
fn parses_synthetic_package() {
    let tmp = tempfile::tempdir().unwrap();
    let path = write_package(tmp.path());

    let pkg = vsdx::Package::open(&path).unwrap();
    let names: Vec<&str> = pkg.part_names();
    assert!(names.contains(&"visio/document.xml"));
    assert_eq!(
        pkg.content_type_of("visio/document.xml").unwrap(),
        "application/vnd.ms-visio.drawing.main+xml"
    );

    let doc = vsdx::parse::parse(&path).unwrap();
    assert_eq!(doc.pages.len(), 1);
    assert_eq!(doc.pages[0].name, "Page-1");
    assert_eq!(
        doc.pages[0]
            .sheet()
            .and_then(|s| s.cell("PageWidth"))
            .and_then(|c| c.numeric()),
        Some(8.5)
    );
    assert_eq!(doc.color(24).unwrap(), "#7F7F7F");

    // Direct shape.
    let s1 = &doc.pages[0].shapes[0];
    assert_eq!(s1.id, 1);
    assert_eq!(s1.sheet.text.as_deref(), Some("hello world"));
    assert_eq!(
        doc.resolve_cell(s1, &[], "PinX").unwrap().numeric(),
        Some(4.25)
    );

    // Master shape inherits Width/Height and geometry from master 7.
    let s2 = &doc.pages[0].shapes[1];
    assert_eq!(s2.master, Some(7));
    assert_eq!(
        doc.resolve_cell(s2, &[], "Width").unwrap().numeric(),
        Some(1.5)
    );
    // LineWeight overridden on the instance (F=Inh means inherit — the
    // instance value is a pass-through, so resolution goes to the master).
    assert_eq!(
        doc.resolve_cell(s2, &[], "LineWeight").unwrap().numeric(),
        Some(0.015625)
    );
    let sheet = doc.effective_sheet(s2, &[]);
    assert_eq!(sheet.sections_named("Geometry").len(), 1);

    // Sub-shape with only MasterShape='5' inherits from the enclosing
    // group's master sub-shape (the standard group-instance layout).
    let s4 = &s2.sub_shapes[0];
    assert_eq!(s4.master, None);
    assert_eq!(s4.master_shape, Some(5));
    assert_eq!(
        doc.resolve_cell(s4, &[s2], "PinX").unwrap().numeric(),
        Some(0.5)
    );
    assert_eq!(
        doc.resolve_cell(s4, &[s2], "Width").unwrap().numeric(),
        Some(1.5)
    );
    assert_eq!(
        doc.effective_sheet(s4, &[s2])
            .sections_named("Geometry")
            .len(),
        1
    );

    // Formula evaluation of LocPinX (Width*0.5) via env fallback is
    // exercised in the dc renderer; here check raw formula text survives.
    assert_eq!(
        doc.pages[0].shapes[0]
            .sheet
            .cell("LocPinX")
            .unwrap()
            .f
            .as_deref(),
        Some("Width*0.5")
    );
}

/// A model-built document (fresh `write::save` path) whose pages and
/// masters carry no `<Rel>` children — parse binds contents parts
/// strictly through each entry's rId, so the writer must synthesize
/// `<Rel r:id>` children (and matching rels entries) for such entries
/// or the shapes silently vanish on reload (#85).
#[test]
fn fresh_save_binds_contents_without_rel_children() {
    use vsdx::model::*;

    let xmlns = || {
        vec![
            (
                "xmlns".to_string(),
                "http://schemas.microsoft.com/office/visio/2012/main".to_string(),
            ),
            (
                "xmlns:r".to_string(),
                "http://schemas.openxmlformats.org/officeDocument/2006/relationships"
                    .to_string(),
            ),
        ]
    };
    let mut doc = vsdx::Document {
        root_attrs: xmlns(),
        pages_attrs: xmlns(),
        masters_attrs: xmlns(),
        doc_part: "visio/document.xml".into(),
        pages_part: "visio/pages/pages.xml".into(),
        items: vec![DocItem::StyleSheets],
        ..vsdx::Document::default()
    };

    // One master, one page, both built in code: no IndexChild::Rel.
    let mut sheet = Sheet::default();
    for (n, v) in [("PinX", "1"), ("PinY", "1"), ("Width", "1"), ("Height", "1")] {
        sheet.set_cell(n, Cell::value(v));
    }
    sheet.children.push(SheetChild::Section(Section {
        name: "Geometry".into(),
        ix: 0,
        attrs: vec![("N".into(), "Geometry".into()), ("IX".into(), "0".into())],
        cells: vec![],
        rows: vec![Row {
            t: Some("MoveTo".into()),
            ix: 1,
            n: None,
            attrs: vec![("T".into(), "MoveTo".into()), ("IX".into(), "1".into())],
            cells: vec![("X".into(), Cell::value("0")), ("Y".into(), Cell::value("0"))],
        }],
    }));
    doc.masters.push(Master {
        id: 1,
        name_u: Some("Square".into()),
        attrs: vec![("ID".into(), "1".into()), ("NameU".into(), "Square".into())],
        shapes: vec![Shape {
            id: 1,
            type_: Some("Shape".into()),
            sheet,
            ..Shape::default()
        }],
        part: "visio/masters/master1.xml".into(),
        contents_attrs: xmlns(),
        ..Master::default()
    });
    doc.pages.push(Page {
        id: 0,
        name: "Page-1".into(),
        attrs: vec![("ID".into(), "0".into()), ("NameU".into(), "Page-1".into())],
        children: vec![],
        part: "visio/pages/page1.xml".into(),
        contents_attrs: xmlns(),
        shapes: vec![Shape {
            id: 2,
            type_: Some("Shape".into()),
            master: Some(1),
            ..Shape::default()
        }],
        ..Page::default()
    });

    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("fresh.vsdx");
    vsdx::write::save(&doc, &path).unwrap();

    // The written index carries a synthesized Rel per entry, and the
    // rels parts bind them to the contents parts.
    let pkg = vsdx::Package::open(&path).unwrap();
    let masters_index = String::from_utf8(
        pkg.part("visio/masters/masters.xml").unwrap().to_vec(),
    )
    .unwrap();
    assert!(
        masters_index.contains("<Rel r:id=\"rId1\"/>"),
        "master entry synthesized its Rel binding: {masters_index}"
    );
    let pages_index = String::from_utf8(
        pkg.part("visio/pages/pages.xml").unwrap().to_vec(),
    )
    .unwrap();
    assert!(
        pages_index.contains("<Rel r:id=\"rId1\"/>"),
        "page entry synthesized its Rel binding: {pages_index}"
    );

    // Reload: both keep their shapes.
    let back = vsdx::parse::parse(&path).unwrap();
    assert_eq!(back.masters.len(), 1);
    assert_eq!(
        back.masters[0].shapes.len(),
        1,
        "master contents survive the round-trip"
    );
    assert_eq!(back.masters[0].part, "visio/masters/master1.xml");
    assert_eq!(back.pages.len(), 1);
    assert_eq!(
        back.pages[0].shapes.len(),
        1,
        "page contents survive the round-trip"
    );
    assert_eq!(back.pages[0].shapes[0].master, Some(1));
}

/// CodeRabbit (PR #106): a retained package that ships WITHOUT a
/// masters part — no masters relationship, no masters.xml — must not
/// lose a master added afterwards (an ImportMaster from a stencil
/// drop): the save synthesizes the masters index, its parts, the
/// document relationship, and the content-type override.
#[test]
fn retained_package_without_masters_keeps_added_master() {
    use std::io::Write;
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("no-masters.vsdx");
    let file = std::fs::File::create(&path).unwrap();
    let mut w = zip::ZipWriter::new(file);
    let opts = zip::write::SimpleFileOptions::default();
    // Same minimal package as `write_package`, minus every masters
    // part/relationship/override and minus the master-instance shape.
    let content_types = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
<Default Extension="xml" ContentType="application/xml"/>
<Override PartName="/visio/document.xml" ContentType="application/vnd.ms-visio.drawing.main+xml"/>
<Override PartName="/visio/pages/pages.xml" ContentType="application/vnd.ms-visio.pages+xml"/>
<Override PartName="/visio/pages/page1.xml" ContentType="application/vnd.ms-visio.pages+xml"/>
</Types>"#;
    let root_rels = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.microsoft.com/visio/2010/relationships/document" Target="visio/document.xml"/>
</Relationships>"#;
    let document = r#"<?xml version="1.0" encoding="utf-8"?>
<VisioDocument xmlns='http://schemas.microsoft.com/office/visio/2012/main' xmlns:r='http://schemas.openxmlformats.org/officeDocument/2006/relationships'>
<StyleSheets>
<StyleSheet ID='0' NameU='No Style'><Cell N='LineWeight' V='0.01041666666666667'/><Cell N='LineColor' V='0'/></StyleSheet>
</StyleSheets>
</VisioDocument>"#;
    let document_rels = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.microsoft.com/visio/2010/relationships/pages" Target="pages/pages.xml"/>
</Relationships>"#;
    let pages = r#"<?xml version="1.0" encoding="utf-8"?>
<Pages xmlns='http://schemas.microsoft.com/office/visio/2012/main' xmlns:r='http://schemas.openxmlformats.org/officeDocument/2006/relationships'>
<Page ID='0' NameU='Page-1' Name='Page-1'>
<PageSheet LineStyle='0' FillStyle='0' TextStyle='0'><Cell N='PageWidth' V='8.5'/><Cell N='PageHeight' V='11.0'/></PageSheet>
<Rel r:id='rId1'/>
</Page>
</Pages>"#;
    let pages_rels = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.microsoft.com/visio/2010/relationships/page" Target="page1.xml"/>
</Relationships>"#;
    let page1 = r#"<?xml version="1.0" encoding="utf-8"?>
<PageContents xmlns='http://schemas.microsoft.com/office/visio/2012/main' xmlns:r='http://schemas.openxmlformats.org/officeDocument/2006/relationships'><Shapes/></PageContents>"#;
    for (name, data) in [
        ("[Content_Types].xml", content_types),
        ("_rels/.rels", root_rels),
        ("visio/document.xml", document),
        ("visio/_rels/document.xml.rels", document_rels),
        ("visio/pages/pages.xml", pages),
        ("visio/pages/_rels/pages.xml.rels", pages_rels),
        ("visio/pages/page1.xml", page1),
    ] {
        w.start_file(name, opts).unwrap();
        w.write_all(data.as_bytes()).unwrap();
    }
    w.finish().unwrap();

    // Parse: no masters anywhere.
    let mut doc = vsdx::parse::parse(&path).unwrap();
    assert!(doc.masters.is_empty());
    assert!(doc.masters_part.is_none(), "the package ships no masters part");

    // Import a master the way ImportMaster does (fresh id, its own
    // part, no Rel child — the writer must bind it).
    doc.masters.push(vsdx::model::Master {
        id: 1,
        name_u: Some("Imported".into()),
        attrs: vec![
            ("ID".into(), "1".into()),
            ("NameU".into(), "Imported".into()),
            ("BaseID".into(), "{NO-MASTERS-1}".into()),
        ],
        shapes: vec![vsdx::model::Shape {
            id: 1,
            type_: Some("Shape".into()),
            sheet: {
                let mut s = vsdx::model::Sheet::default();
                s.set_cell("Width", vsdx::model::Cell::value("1"));
                s.set_cell("Height", vsdx::model::Cell::value("1"));
                s
            },
            ..vsdx::model::Shape::default()
        }],
        part: "visio/masters/master1.xml".into(),
        ..vsdx::model::Master::default()
    });

    let out = tmp.path().join("saved.vsdx");
    vsdx::write::save(&doc, &out).unwrap();
    let pkg = vsdx::Package::open(&out).unwrap();
    let names = pkg.part_names();
    assert!(
        names.contains(&"visio/masters/masters.xml"),
        "the masters index was synthesized: {names:?}"
    );
    assert!(
        names.contains(&"visio/masters/master1.xml"),
        "the imported master's part was materialized"
    );
    let doc_rels = String::from_utf8(
        pkg.part("visio/_rels/document.xml.rels").unwrap().to_vec(),
    )
    .unwrap();
    assert!(
        doc_rels.contains("masters/masters.xml"),
        "the document relationship was added: {doc_rels}"
    );
    let ct = String::from_utf8(pkg.part("[Content_Types].xml").unwrap().to_vec()).unwrap();
    assert!(
        ct.contains("/visio/masters/masters.xml"),
        "the content-type override was added"
    );

    // Reload: the imported master survived, contents bound.
    let back = vsdx::parse::parse(&out).unwrap();
    assert_eq!(back.masters.len(), 1);
    assert_eq!(back.masters[0].name_u.as_deref(), Some("Imported"));
    assert_eq!(back.masters[0].shapes.len(), 1, "contents bound, not lost");
    assert_eq!(back.masters[0].part, "visio/masters/master1.xml");
}
