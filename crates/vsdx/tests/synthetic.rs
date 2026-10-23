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
    let names: Vec<&str> = pkg.part_names().into();
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

#[test]
fn formula_subset() {
    let mut env = vsdx::formula::Env::new();
    env.insert("Width".to_string(), 2.0);
    env.insert("Height".to_string(), 1.0);
    assert_eq!(
        vsdx::formula::eval("Width*0.5", &env).unwrap().as_f64(),
        Some(1.0)
    );
    // Unit suffixes are absorbed.
    assert_eq!(
        vsdx::formula::eval("Width-0.5DL", &env).unwrap().as_f64(),
        Some(1.5)
    );
}
