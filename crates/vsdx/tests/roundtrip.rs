//! Round-trip tests (v0.2): a deliberately messy synthetic package (no
//! Microsoft content) must survive parse → write → parse with canonical
//! XML equality on modeled parts, byte equality on everything else, and an
//! unchanged model. Plus: fresh documents built in code, and mutation via
//! `Sheet::set_cell` (the seed of the editing API).

use std::io::Write;
use std::path::{Path, PathBuf};

/// A package exercising every preservation path: unknown document children,
/// interleaved `<Trigger>`s, `<RefBy>` inside cells, empty-form rows and
/// sections, raw `<Text>` markup with entities, `<ForeignData>` before
/// `<Text>`, deleted (empty-form) shapes, `<Icon>` in the masters index,
/// `<Connects>` in a page part, and unmodeled parts including binary data.
fn write_messy_package(dir: &Path) -> PathBuf {
    let path = dir.join("messy.vstx");
    let file = std::fs::File::create(&path).unwrap();
    let mut w = zip::ZipWriter::new(file);
    let opts = zip::write::SimpleFileOptions::default();

    let content_types = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
<Default Extension="xml" ContentType="application/xml"/>
<Default Extension="bin" ContentType="application/octet-stream"/>
<Override PartName="/visio/document.xml" ContentType="application/vnd.ms-visio.drawing.main+xml"/>
<Override PartName="/visio/pages/pages.xml" ContentType="application/vnd.ms-visio.pages+xml"/>
<Override PartName="/visio/pages/page1.xml" ContentType="application/vnd.ms-visio.pages+xml"/>
<Override PartName="/visio/masters/masters.xml" ContentType="application/vnd.ms-visio.masters+xml"/>
<Override PartName="/visio/masters/master1.xml" ContentType="application/vnd.ms-visio.master+xml"/>
</Types>"#;

    let root_rels = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.microsoft.com/visio/2010/relationships/document" Target="visio/document.xml"/>
<Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/>
</Relationships>"#;

    // Unknown children (DocumentSettings, FaceNames) around modeled ones;
    // stylesheet with attrs; colors; document sheet with a Trigger.
    let document = r#"<?xml version="1.0" encoding="utf-8"?>
<VisioDocument xmlns='http://schemas.microsoft.com/office/visio/2012/main' xmlns:r='http://schemas.openxmlformats.org/officeDocument/2006/relationships'>
<DocumentSettings><Cell N='DocLockReplace' V='0'/></DocumentSettings>
<StyleSheets>
<StyleSheet ID='0' NameU='No Style' Name='No Style' IsCustomNameU='0'>
  <Cell N='LineWeight' V='0.01041666666666667'/>
  <Cell N='LineColor' V='0'/>
  <Trigger N='RecalcTextWidth'><RefBy ID='0' T='Document'/></Trigger>
  <Cell N='FillForegnd' V='1'/>
</StyleSheet>
</StyleSheets>
<FaceNames><FaceName NameU='Arial'/></FaceNames>
<Colors>
<ColorEntry IX='24' RGB='#7F7F7F'/>
<ColorEntry IX='25' RGB='#123456'/>
</Colors>
<DocumentSheet NameU='TheDoc' LineStyle='0' FillStyle='0' TextStyle='0'>
<Cell N='LineWeight' V='0.01'/>
</DocumentSheet>
</VisioDocument>"#;

    let document_rels = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.microsoft.com/visio/2010/relationships/pages" Target="pages/pages.xml"/>
<Relationship Id="rId2" Type="http://schemas.microsoft.com/visio/2010/relationships/masters" Target="masters/masters.xml"/>
</Relationships>"#;

    // Page index with attrs, inline PageSheet with an interleaved Trigger,
    // and a second (stencil-style) page without a part.
    let pages = r#"<?xml version="1.0" encoding="utf-8"?>
<Pages xmlns='http://schemas.microsoft.com/office/visio/2012/main' xmlns:r='http://schemas.openxmlformats.org/officeDocument/2006/relationships'>
<Page ID='0' NameU='Page-1' Name='Page-1' ViewScale='1' ViewCenterX='4.25' ViewCenterY='5.5'>
<PageSheet LineStyle='0' FillStyle='0' TextStyle='0'>
  <Cell N='PageWidth' V='8.5'/>
  <Trigger N='RecalcColor'><RefBy ID='0' T='Page'/></Trigger>
  <Cell N='PageHeight' V='11.0'/>
</PageSheet>
<Rel r:id='rId1'/>
</Page>
<Page ID='1' NameU='Stencil-2'>
<PageSheet LineStyle='0' FillStyle='0' TextStyle='0'>
  <Cell N='PageWidth' V='4'/>
  <Cell N='PageHeight' V='3'/>
</PageSheet>
</Page>
</Pages>"#;

    let pages_rels = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.microsoft.com/visio/2010/relationships/page" Target="page1.xml"/>
</Relationships>"#;

    // Page part: mixed quoting, empty-form row/section, named User rows
    // with RefBy inside a cell, Text markup with entities, ForeignData
    // BEFORE Text, a deleted empty-form shape, and trailing Connects.
    let page1 = r#"<?xml version="1.0" encoding="utf-8"?>
<PageContents xmlns='http://schemas.microsoft.com/office/visio/2012/main' xmlns:r='http://schemas.openxmlformats.org/officeDocument/2006/relationships'>
<Shapes>
<Shape ID='1' Type='Shape' UniqueID='{11111111-2222-3333-4444-555555555555}' LineStyle='0' FillStyle='0' TextStyle='0'>
  <Cell N='PinX' V='4.25'/><Cell N='PinY' V='5.5'/>
  <Cell N='Width' V='2'/><Cell N='Height' V='1'/>
  <Section N='Geometry' IX='0'>
    <Cell N='NoFill' V='0'/>
    <Row T='MoveTo' IX='1'><Cell N='X' V='0'/><Cell N='Y' V='0'/></Row>
    <Row T='LineTo' IX='2'><Cell N='X' V='2'/><Cell N='Y' V='0'/></Row>
    <Row T='LineTo' IX='3'><Cell N='X' V='2'/><Cell N='Y' V='1'/></Row>
    <Row T='LineTo' IX='4'><Cell N='X' V='0'/><Cell N='Y' V='1'/></Row>
    <Row T='LineTo' IX='5'><Cell N='X' V='0'/><Cell N='Y' V='0'/></Row>
  </Section>
  <Section N='User'>
    <Row N='Count'><Cell N='Value' V='7'><RefBy ID='1' T='Shape'/></Cell><Cell N='Prompt' V=''/></Row>
  </Section>
  <Section N='Tabs'><Row IX='0'/></Section>
  <ForeignData CompressionType='PNG' ForeignType='Bitmap'><Rel r:id='rId1'/></ForeignData>
  <Text>a &amp; b &lt;tag&gt; <cp IX='0'/>line&#65;</Text>
</Shape>
<Shape ID='2' Type='Group' Master='7' NameU='G'>
  <Cell N='PinX' V='2'/>
  <Shapes>
    <Shape ID='3' Type='Shape' MasterShape='5'><Cell N='PinX' V='0.5'/></Shape>
    <Shape Del='1' ID='4294967295' MasterShape='9'/>
  </Shapes>
</Shape>
</Shapes>
<Connects><Connect FromCell='BeginX' FromPart='9' FromSheet='2' ToCell='Connections.X1' ToPart='100' ToSheet='1'/></Connects>
</PageContents>"#;

    let page1_rels = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.microsoft.com/visio/2010/relationships/image" Target="../media/image1.png" TargetMode="External"/>
</Relationships>"#;

    let masters = r#"<?xml version="1.0" encoding="utf-8"?>
<Masters xmlns='http://schemas.microsoft.com/office/visio/2012/main' xmlns:r='http://schemas.openxmlformats.org/officeDocument/2006/relationships'>
<Master ID='7' NameU='Master box' Name='Master box' PatternFlags='0' Hidden='0'>
<PageSheet LineStyle='0' FillStyle='0' TextStyle='0'>
  <Cell N='PageWidth' V='1.5'/>
  <Cell N='PageHeight' V='0.75'/>
</PageSheet>
<Rel r:id='rId1'/>
<Icon>AAECAwQFBgcICQ==</Icon>
</Master>
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
  <Section N='Geometry' IX='0'>
    <Row T='MoveTo' IX='1'><Cell N='X' V='0'/><Cell N='Y' V='0'/></Row>
    <Row T='LineTo' IX='2'><Cell N='X' V='1.5'/><Cell N='Y' V='0'/></Row>
    <Row T='LineTo' IX='3'><Cell N='X' V='1.5'/><Cell N='Y' V='0.75'/></Row>
    <Row T='LineTo' IX='4'><Cell N='X' V='0'/><Cell N='Y' V='0'/></Row>
  </Section>
</Shape>
</Shapes>
</MasterContents>"#;

    // Unmodeled parts: XML, properties, and raw binary.
    let core = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/">
<dc:title>messy fixture</dc:title>
</cp:coreProperties>"#;

    let mut entries: Vec<(&str, Vec<u8>)> = vec![
        ("[Content_Types].xml", content_types.as_bytes().to_vec()),
        ("_rels/.rels", root_rels.as_bytes().to_vec()),
        ("visio/document.xml", document.as_bytes().to_vec()),
        (
            "visio/_rels/document.xml.rels",
            document_rels.as_bytes().to_vec(),
        ),
        ("visio/pages/pages.xml", pages.as_bytes().to_vec()),
        (
            "visio/pages/_rels/pages.xml.rels",
            pages_rels.as_bytes().to_vec(),
        ),
        ("visio/pages/page1.xml", page1.as_bytes().to_vec()),
        (
            "visio/pages/_rels/page1.xml.rels",
            page1_rels.as_bytes().to_vec(),
        ),
        ("visio/masters/masters.xml", masters.as_bytes().to_vec()),
        (
            "visio/masters/_rels/masters.xml.rels",
            masters_rels.as_bytes().to_vec(),
        ),
        ("visio/masters/master1.xml", master1.as_bytes().to_vec()),
        ("docProps/core.xml", core.as_bytes().to_vec()),
        ("custom/blob.bin", vec![0x00, 0x01, 0xFE, 0xFF, 0x7F, 0x80]),
    ];
    // Slight compression variety for the binary part.
    for (name, data) in entries.drain(..) {
        if name.ends_with(".bin") {
            w.start_file(
                name,
                zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Stored),
            )
            .unwrap();
        } else {
            w.start_file(name, opts).unwrap();
        }
        w.write_all(&data).unwrap();
    }
    w.finish().unwrap();
    path
}

#[test]
fn roundtrip_preserves_messy_package() {
    let tmp = tempfile::tempdir().unwrap();
    let src = write_messy_package(tmp.path());
    let out = tmp.path().join("rewritten.vstx");

    let doc = vsdx::parse::parse(&src).unwrap();
    vsdx::write::save(&doc, &out).unwrap();

    let src_pkg = doc.package.as_ref().unwrap();
    let out_pkg = vsdx::Package::open(&out).unwrap();

    // 1. Identical part inventory.
    let src_names: Vec<&str> = src_pkg.part_names();
    let out_names: Vec<&str> = out_pkg.part_names();
    assert_eq!(src_names, out_names, "part inventory differs");

    // 2. Modeled parts: canonical-XML equal. Everything else: byte-equal.
    let modeled = vsdx::write::modeled_parts(&doc);
    for name in &src_names {
        let (a, b) = (src_pkg.part(name).unwrap(), out_pkg.part(name).unwrap());
        if modeled.iter().any(|m| m == name) {
            assert_eq!(
                vsdx::canonical_form(a),
                vsdx::canonical_form(b),
                "canonical form of {name} differs"
            );
        } else {
            assert_eq!(a, b, "{name} should be byte-identical");
        }
    }

    // 3. Re-parsed model is unchanged.
    let doc2 = vsdx::parse::parse(&out).unwrap();
    assert!(doc.same_model(&doc2), "re-parsed model differs");

    // Spot-check that the messy constructs actually survived the model.
    let page = &doc.pages[0];
    assert_eq!(
        page.attrs
            .iter()
            .find(|(k, _)| k == "ViewScale")
            .map(|(_, v)| v.as_str()),
        Some("1")
    );
    assert_eq!(
        page.sheet()
            .and_then(|s| s.cell("PageWidth"))
            .and_then(|c| c.numeric()),
        Some(8.5)
    );
    // Stencil-style page (no part) kept.
    assert_eq!(doc.pages.len(), 2);
    assert_eq!(doc.pages[1].part, "");
    // Interleaved Trigger survived inside the PageSheet.
    let sheet = page.sheet().unwrap();
    let has_trigger = sheet.children.iter().any(|c| {
        matches!(
            c,
            vsdx::model::SheetChild::Raw(r) if r.name == "Trigger"
        )
    });
    assert!(has_trigger, "PageSheet Trigger lost");
    // RefBy survived inside the User row cell (on the shape's sheet).
    let user = doc.pages[0].shapes[0].sheet.section("User", 0).unwrap();
    let val = user.named_row("Count").unwrap().cell("Value").unwrap();
    assert_eq!(val.v.as_deref(), Some("7"));
    assert!(val.raw_inner.as_deref().unwrap().contains("RefBy"));
    // Text markup and entities survive verbatim.
    let text_raw = doc.pages[0].shapes[0].sheet.text_raw.as_deref().unwrap();
    assert!(
        text_raw.contains("<cp IX=\"0\"/>"),
        "cp markup lost: {text_raw}"
    );
    assert!(text_raw.contains("&amp;"), "entity lost: {text_raw}");
    assert!(text_raw.contains("&#65;"), "numeric ref lost: {text_raw}");
    // Plain text decodes entities.
    assert_eq!(
        doc.pages[0].shapes[0].sheet.text.as_deref(),
        Some("a & b <tag> lineA")
    );
    // Deleted empty-form shape survived.
    let g = &doc.pages[0].shapes[1];
    assert_eq!(g.sub_shapes.len(), 2);
    assert_eq!(g.sub_shapes[1].id, 4294967295);
    assert!(g.sub_shapes[1].attrs.iter().any(|(k, _)| k == "Del"));
    // ForeignData before Text order preserved.
    let flow = &doc.pages[0].shapes[0].flow;
    let fi = flow
        .iter()
        .position(|f| matches!(f, vsdx::model::ShapeFlow::Raw(_)));
    let ti = flow
        .iter()
        .position(|f| matches!(f, vsdx::model::ShapeFlow::Text));
    assert!(fi.is_some_and(|fi| ti.is_some_and(|ti| fi < ti)));
    // Icon survived in the masters index.
    let m = doc.master(7).unwrap();
    assert!(m.children.iter().any(|c| matches!(
        c,
        vsdx::model::IndexChild::Raw(r) if r.name == "Icon"
    )));
}

#[test]
fn roundtrip_keeps_edits_and_verbatim_parts() {
    let tmp = tempfile::tempdir().unwrap();
    let src = write_messy_package(tmp.path());
    let out = tmp.path().join("edited.vstx");

    let mut doc = vsdx::parse::parse(&src).unwrap();
    doc.pages[0].shapes[0]
        .sheet
        .set_cell("PinX", vsdx::Cell::value("9.75"));
    vsdx::write::save(&doc, &out).unwrap();

    let doc2 = vsdx::parse::parse(&out).unwrap();
    assert_eq!(
        doc2.pages[0].shapes[0]
            .sheet
            .cell("PinX")
            .unwrap()
            .numeric(),
        Some(9.75),
        "edited cell did not survive"
    );

    // The binary blob is still byte-identical.
    let out_pkg = vsdx::Package::open(&out).unwrap();
    assert_eq!(
        out_pkg.part("custom/blob.bin").unwrap(),
        &[0x00, 0x01, 0xFE, 0xFF, 0x7F, 0x80]
    );

    // The re-serialized page part contains the new value and the raw
    // constructs that were never edited.
    let page_xml = String::from_utf8(out_pkg.part(&doc2.pages[0].part).unwrap().to_vec()).unwrap();
    assert!(page_xml.contains(r#"N="PinX" V="9.75""#));
    assert!(page_xml.contains("RefBy"));
    assert!(page_xml.contains("<cp IX=\"0\"/>"));
}

#[test]
fn writes_fresh_document_from_model() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("fresh.vsdx");

    use vsdx::model::*;
    let xmlns = vec![
        (
            "xmlns".to_string(),
            "http://schemas.microsoft.com/office/visio/2012/main".to_string(),
        ),
        (
            "xmlns:r".to_string(),
            "http://schemas.openxmlformats.org/officeDocument/2006/relationships".to_string(),
        ),
    ];
    let mut doc = Document {
        root_attrs: xmlns.clone(),
        pages_attrs: xmlns.clone(),
        doc_part: "visio/document.xml".into(),
        pages_part: "visio/pages/pages.xml".into(),
        items: vec![DocItem::StyleSheets],
        ..Default::default()
    };
    let ss_attrs: Vec<(String, String)> = vec![
        ("ID".into(), "0".into()),
        ("NameU".into(), "No Style".into()),
    ];
    doc.stylesheets.push(StyleSheet {
        id: 0,
        name_u: Some("No Style".into()),
        attrs: ss_attrs.clone(),
        sheet: Sheet {
            attrs: ss_attrs,
            ..Sheet::default()
        },
    });

    let mut page_sheet = Sheet {
        attrs: vec![
            ("LineStyle".into(), "0".into()),
            ("FillStyle".into(), "0".into()),
            ("TextStyle".into(), "0".into()),
        ],
        ..Default::default()
    };
    page_sheet.set_cell("PageWidth", Cell::value("8.5"));
    page_sheet.set_cell("PageHeight", Cell::value("11"));

    let mut shape_sheet = Sheet::default();
    shape_sheet.set_cell("PinX", Cell::value("4.25"));
    shape_sheet.set_cell("PinY", Cell::value("5.5"));
    shape_sheet.set_cell("Width", Cell::value("2"));
    shape_sheet.set_cell("Height", Cell::value("1"));
    let geometry = Section {
        name: "Geometry".into(),
        ix: 0,
        attrs: vec![("N".into(), "Geometry".into()), ("IX".into(), "0".into())],
        cells: vec![],
        rows: vec![
            Row {
                t: Some("MoveTo".into()),
                ix: 1,
                n: None,
                attrs: vec![("T".into(), "MoveTo".into()), ("IX".into(), "1".into())],
                cells: vec![
                    ("X".into(), Cell::value("0")),
                    ("Y".into(), Cell::value("0")),
                ],
            },
            Row {
                t: Some("LineTo".into()),
                ix: 2,
                n: None,
                attrs: vec![("T".into(), "LineTo".into()), ("IX".into(), "2".into())],
                cells: vec![
                    ("X".into(), Cell::value("2")),
                    ("Y".into(), Cell::value("0")),
                ],
            },
        ],
    };
    shape_sheet.children.push(SheetChild::Section(geometry));
    let shape = Shape {
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
        sheet: shape_sheet,
        ..Default::default()
    };

    doc.pages.push(Page {
        id: 0,
        name: "Page-1".into(),
        attrs: vec![("ID".into(), "0".into()), ("NameU".into(), "Page-1".into())],
        children: vec![
            IndexChild::Sheet {
                tag: "PageSheet".into(),
                sheet: page_sheet,
            },
            IndexChild::Rel(vec![("r:id".into(), "rId1".into())]),
        ],
        shapes: vec![shape],
        part: "visio/pages/page1.xml".into(),
        contents_attrs: xmlns.clone(),
        ..Default::default()
    });

    vsdx::write::save(&doc, &out).unwrap();

    // The generated package must be a valid visio package.
    let pkg = vsdx::Package::open(&out).unwrap();
    let names = pkg.part_names();
    for expected in [
        "[Content_Types].xml",
        "_rels/.rels",
        "visio/document.xml",
        "visio/_rels/document.xml.rels",
        "visio/pages/pages.xml",
        "visio/pages/_rels/pages.xml.rels",
        "visio/pages/page1.xml",
    ] {
        assert!(names.contains(&expected), "missing {expected} in {names:?}");
    }
    assert_eq!(
        pkg.content_type_of("visio/document.xml").unwrap(),
        "application/vnd.ms-visio.drawing.main+xml"
    );

    // And it must re-parse to the same model.
    let doc2 = vsdx::parse::parse(&out).unwrap();
    assert_eq!(doc2.pages.len(), 1);
    assert_eq!(doc2.pages[0].name, "Page-1");
    assert_eq!(
        doc2.pages[0]
            .sheet()
            .and_then(|s| s.cell("PageWidth"))
            .and_then(|c| c.numeric()),
        Some(8.5)
    );
    assert_eq!(
        doc2.pages[0].shapes[0]
            .sheet
            .cell("PinX")
            .unwrap()
            .numeric(),
        Some(4.25)
    );
    assert_eq!(
        doc2.pages[0].shapes[0]
            .sheet
            .sections_named("Geometry")
            .len(),
        1
    );
    // Geometry row values survive the round trip (row order and X/Y).
    let geo = &doc2.pages[0].shapes[0].sheet.sections_named("Geometry")[0];
    assert_eq!(geo.rows.len(), 2);
    let mv = geo.rows.iter().find(|r| r.t.as_deref() == Some("MoveTo")).unwrap();
    assert_eq!(
        (mv.cell("X").and_then(|c| c.numeric()), mv.cell("Y").and_then(|c| c.numeric())),
        (Some(0.0), Some(0.0))
    );
    let lt = geo.rows.iter().find(|r| r.t.as_deref() == Some("LineTo")).unwrap();
    assert_eq!(
        (lt.cell("X").and_then(|c| c.numeric()), lt.cell("Y").and_then(|c| c.numeric())),
        (Some(2.0), Some(0.0))
    );
    assert_eq!(doc2.stylesheets.len(), 1);
    // No same_model leg here by design: it would require constructing
    // cells with the writer's exact attribute list (the old cell_nv
    // helper), pinning serializer conventions the model API does not
    // guarantee. Attribute fidelity is already proven where it is the
    // spec — the messy-package canonical-XML oracle above.
}
