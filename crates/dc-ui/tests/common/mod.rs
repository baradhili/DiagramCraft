//! Shared integration-test fixtures (#97 section 4 dedup): the
//! many-shape .vsdx builder that raster_async and render_perf carried
//! as byte-identical copies (modulo comments and a filename prefix —
//! each suite writes into its own tempdir, so one filename serves).
pub fn many_shape_fixture(dir: &std::path::Path, n_shapes: usize, n_masters: usize) -> std::path::PathBuf {
    let rect = |id: usize, pin_x: f64, pin_y: f64, w: f64, h: f64, text: &str| {
        let (lw, lh) = (w / 2.0, h / 2.0);
        format!(
            "<Shape ID='{id}' Type='Shape' LineStyle='0' FillStyle='0' TextStyle='0'>\n\
<Cell N='PinX' V='{pin_x}'/><Cell N='PinY' V='{pin_y}'/>\n\
<Cell N='Width' V='{w}'/><Cell N='Height' V='{h}'/>\n\
<Cell N='LocPinX' V='{lw}'/><Cell N='LocPinY' V='{lh}'/>\n\
<Section N='Geometry' IX='0'>\n\
<Row T='MoveTo' IX='1'><Cell N='X' V='0'/><Cell N='Y' V='0'/></Row>\n\
<Row T='LineTo' IX='2'><Cell N='X' V='{w}'/><Cell N='Y' V='0'/></Row>\n\
<Row T='LineTo' IX='3'><Cell N='X' V='{w}'/><Cell N='Y' V='{h}'/></Row>\n\
<Row T='LineTo' IX='4'><Cell N='X' V='0'/><Cell N='Y' V='{h}'/></Row>\n\
<Row T='LineTo' IX='5'><Cell N='X' V='0'/><Cell N='Y' V='0'/></Row>\n\
</Section>\n\
<Text>{text}</Text>\n\
</Shape>\n"
        )
    };

    // Shape 1 at the page center (a fat click target); the rest on a
    // 20-column grid filling the 8.5×11 page.
    let mut shapes = String::new();
    shapes.push_str(&rect(1, 4.25, 5.5, 2.0, 1.0, "center"));
    let (cols, cw, ch) = (20usize, 0.4, 0.107);
    for i in 1..n_shapes {
        let (row, col) = (i / cols, i % cols);
        shapes.push_str(&rect(
            i + 1,
            0.25 + col as f64 * cw + cw / 2.0,
            10.7 - row as f64 * ch - ch / 2.0,
            0.3,
            0.08,
            &format!("S{i}"),
        ));
    }

    let document = r#"<?xml version="1.0" encoding="utf-8"?>
<VisioDocument xmlns='http://schemas.microsoft.com/office/visio/2012/main' xmlns:r='http://schemas.openxmlformats.org/officeDocument/2006/relationships'>
<StyleSheets><StyleSheet ID='0' NameU='No Style'>
  <Cell N='LineWeight' V='0.01041666666666667'/><Cell N='LineColor' V='0'/>
  <Cell N='FillForegnd' V='1'/><Cell N='FillPattern' V='1'/>
</StyleSheet></StyleSheets>
</VisioDocument>"#;
    let pages = r#"<?xml version="1.0" encoding="utf-8"?>
<Pages xmlns='http://schemas.microsoft.com/office/visio/2012/main' xmlns:r='http://schemas.openxmlformats.org/officeDocument/2006/relationships'>
<Page ID='0' NameU='Bench'><PageSheet LineStyle='0' FillStyle='0' TextStyle='0'>
  <Cell N='PageWidth' V='8.5'/><Cell N='PageHeight' V='11'/>
</PageSheet><Rel r:id='rId1'/></Page>
</Pages>"#;
    let page1 = format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<PageContents xmlns='http://schemas.microsoft.com/office/visio/2012/main' xmlns:r='http://schemas.openxmlformats.org/officeDocument/2006/relationships'>
<Shapes>
{shapes}</Shapes>
</PageContents>"#
    );
    let master_entries: String = (0..n_masters)
        .map(|i| {
            format!(
                "<Master ID='{}' NameU='B{i}'><PageSheet LineStyle='0' FillStyle='0' TextStyle='0'>\
<Cell N='PageHeight' V='1'/></PageSheet><Rel r:id='rId{}'/></Master>\n",
                i + 1,
                i + 1
            )
        })
        .collect();
    let masters = format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<Masters xmlns='http://schemas.microsoft.com/office/visio/2012/main' xmlns:r='http://schemas.openxmlformats.org/officeDocument/2006/relationships'>
{master_entries}</Masters>"#
    );
    // One shared geometry for every master body; ids inside a master
    // part are master-local, so a single template serves them all.
    let master_body = |i: usize| {
        format!(
            r#"<?xml version="1.0" encoding="utf-8"?>
<MasterContents xmlns='http://schemas.microsoft.com/office/visio/2012/main' xmlns:r='http://schemas.openxmlformats.org/officeDocument/2006/relationships'>
<Shapes>{}</Shapes>
</MasterContents>"#,
            rect(i, 0.5, 0.5, 0.8, 0.6, &format!("M{i}"))
        )
    };
    let master_rels: String = (0..n_masters)
        .map(|i| {
            format!(
                "<Relationship Id='rId{}' Type='http://schemas.microsoft.com/visio/2010/relationships/master' Target='master{}.xml'/>",
                i + 1,
                i + 1
            )
        })
        .collect();
    let master_rels = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">{master_rels}</Relationships>"#
    );
    let master_overrides: String = (0..n_masters)
        .map(|i| {
            format!(
                "<Override PartName=\"/visio/masters/master{}.xml\" ContentType=\"application/vnd.ms-visio.master+xml\"/>\n",
                i + 1
            )
        })
        .collect();
    let content_types = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
<Default Extension="xml" ContentType="application/xml"/>
<Override PartName="/visio/document.xml" ContentType="application/vnd.ms-visio.drawing.main+xml"/>
<Override PartName="/visio/pages/pages.xml" ContentType="application/vnd.ms-visio.pages+xml"/>
<Override PartName="/visio/pages/page1.xml" ContentType="application/vnd.ms-visio.pages+xml"/>
<Override PartName="/visio/masters/masters.xml" ContentType="application/vnd.ms-visio.masters+xml"/>
{master_overrides}</Types>"#
    );
    let root_rels = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.microsoft.com/visio/2010/relationships/document" Target="visio/document.xml"/>
</Relationships>"#;
    let doc_rels = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.microsoft.com/visio/2010/relationships/pages" Target="pages/pages.xml"/>
<Relationship Id="rId2" Type="http://schemas.microsoft.com/visio/2010/relationships/masters" Target="masters/masters.xml"/>
</Relationships>"#;
    let page_rels = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.microsoft.com/visio/2010/relationships/page" Target="page1.xml"/>
</Relationships>"#;

    let path = dir.join(format!("bench-{n_shapes}.vsdx"));
    let file = std::fs::File::create(&path).unwrap();
    let mut w = zip::ZipWriter::new(file);
    let opts = zip::write::SimpleFileOptions::default();
    let mut parts: Vec<(String, String)> = vec![
        ("[Content_Types].xml".into(), content_types),
        ("_rels/.rels".into(), root_rels.into()),
        ("visio/document.xml".into(), document.into()),
        ("visio/_rels/document.xml.rels".into(), doc_rels.into()),
        ("visio/pages/pages.xml".into(), pages.into()),
        ("visio/pages/_rels/pages.xml.rels".into(), page_rels.into()),
        ("visio/pages/page1.xml".into(), page1),
        ("visio/masters/masters.xml".into(), masters),
        ("visio/masters/_rels/masters.xml.rels".into(), master_rels),
    ];
    for i in 0..n_masters {
        parts.push((format!("visio/masters/master{}.xml", i + 1), master_body(i + 1)));
    }
    for (name, data) in &parts {
        w.start_file(name.as_str(), opts).unwrap();
        std::io::Write::write_all(&mut w, data.as_bytes()).unwrap();
    }
    w.finish().unwrap();
    path
}
