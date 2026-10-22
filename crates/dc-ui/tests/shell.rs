//! Headless shell snapshot (egui_kittest): renders the full desktop
//! shell — title band, ribbon, stencil pane, canvas, page tabs, status
//! bar — over a synthetic package (no Microsoft content) and writes
//! PNGs for review under target/kittest-snapshots/.

use dc_ui::app::App;
use egui_kittest::Harness;

fn fixture(dir: &std::path::Path) -> std::path::PathBuf {
    let document = r#"<?xml version="1.0" encoding="utf-8"?>
<VisioDocument xmlns='http://schemas.microsoft.com/office/visio/2012/main' xmlns:r='http://schemas.openxmlformats.org/officeDocument/2006/relationships'>
<StyleSheets><StyleSheet ID='0' NameU='No Style'>
  <Cell N='LineWeight' V='0.01041666666666667'/><Cell N='LineColor' V='0'/>
  <Cell N='FillForegnd' V='1'/><Cell N='FillPattern' V='1'/>
</StyleSheet></StyleSheets>
</VisioDocument>"#;
    let pages = r#"<?xml version="1.0" encoding="utf-8"?>
<Pages xmlns='http://schemas.microsoft.com/office/visio/2012/main' xmlns:r='http://schemas.openxmlformats.org/officeDocument/2006/relationships'>
<Page ID='0' NameU='Page-1' Name='Page-1'>
<PageSheet LineStyle='0' FillStyle='0' TextStyle='0'>
  <Cell N='PageWidth' V='8.5'/><Cell N='PageHeight' V='11'/>
</PageSheet>
<Rel r:id='rId1'/>
</Page>
</Pages>"#;
    let page1 = r#"<?xml version="1.0" encoding="utf-8"?>
<PageContents xmlns='http://schemas.microsoft.com/office/visio/2012/main' xmlns:r='http://schemas.openxmlformats.org/officeDocument/2006/relationships'>
<Shapes>
<Shape ID='1' Type='Shape' LineStyle='0' FillStyle='0' TextStyle='0'>
  <Cell N='PinX' V='4.25'/><Cell N='PinY' V='8'/>
  <Cell N='Width' V='2'/><Cell N='Height' V='1'/>
  <Cell N='LocPinX' V='1'/><Cell N='LocPinY' V='0.5'/>
  <Section N='Geometry' IX='0'>
    <Row T='MoveTo' IX='1'><Cell N='X' V='0'/><Cell N='Y' V='0'/></Row>
    <Row T='LineTo' IX='2'><Cell N='X' V='2'/><Cell N='Y' V='0'/></Row>
    <Row T='LineTo' IX='3'><Cell N='X' V='2'/><Cell N='Y' V='1'/></Row>
    <Row T='LineTo' IX='4'><Cell N='X' V='0'/><Cell N='Y' V='1'/></Row>
    <Row T='LineTo' IX='5'><Cell N='X' V='0'/><Cell N='Y' V='0'/></Row>
  </Section>
  <Text>hello visio</Text>
</Shape>
</Shapes>
</PageContents>"#;
    let masters = r#"<?xml version="1.0" encoding="utf-8"?>
<Masters xmlns='http://schemas.microsoft.com/office/visio/2012/main' xmlns:r='http://schemas.openxmlformats.org/officeDocument/2006/relationships'>
<Master ID='7' NameU='Box'>
<PageSheet LineStyle='0' FillStyle='0' TextStyle='0'><Cell N='PageHeight' V='1'/></PageSheet>
<Rel r:id='rId1'/>
</Master>
</Masters>"#;
    let master1 = r#"<?xml version="1.0" encoding="utf-8"?>
<MasterContents xmlns='http://schemas.microsoft.com/office/visio/2012/main' xmlns:r='http://schemas.openxmlformats.org/officeDocument/2006/relationships'>
<Shapes>
<Shape ID='1' Type='Shape' LineStyle='0' FillStyle='0' TextStyle='0'>
  <Cell N='PinX' V='0.5'/><Cell N='PinY' V='0.5'/>
  <Cell N='Width' V='0.8'/><Cell N='Height' V='0.6'/>
  <Cell N='LocPinX' V='0.4'/><Cell N='LocPinY' V='0.3'/>
  <Section N='Geometry' IX='0'>
    <Row T='MoveTo' IX='1'><Cell N='X' V='0'/><Cell N='Y' V='0'/></Row>
    <Row T='LineTo' IX='2'><Cell N='X' V='0.8'/><Cell N='Y' V='0'/></Row>
    <Row T='LineTo' IX='3'><Cell N='X' V='0.8'/><Cell N='Y' V='0.6'/></Row>
    <Row T='LineTo' IX='4'><Cell N='X' V='0'/><Cell N='Y' V='0.6'/></Row>
    <Row T='LineTo' IX='5'><Cell N='X' V='0'/><Cell N='Y' V='0'/></Row>
  </Section>
</Shape>
</Shapes>
</MasterContents>"#;
    let page_rels = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.microsoft.com/visio/2010/relationships/page" Target="page1.xml"/>
</Relationships>"#;
    let master_rels = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.microsoft.com/visio/2010/relationships/master" Target="master1.xml"/>
</Relationships>"#;
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
    let doc_rels = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.microsoft.com/visio/2010/relationships/pages" Target="pages/pages.xml"/>
<Relationship Id="rId2" Type="http://schemas.microsoft.com/visio/2010/relationships/masters" Target="masters/masters.xml"/>
</Relationships>"#;

    let path = dir.join("shell.vssx");
    let file = std::fs::File::create(&path).unwrap();
    let mut w = zip::ZipWriter::new(file);
    let opts = zip::write::SimpleFileOptions::default();
    for (name, data) in [
        ("[Content_Types].xml", content_types),
        ("_rels/.rels", root_rels),
        ("visio/document.xml", document),
        ("visio/_rels/document.xml.rels", doc_rels),
        ("visio/pages/pages.xml", pages),
        ("visio/pages/_rels/pages.xml.rels", page_rels),
        ("visio/pages/page1.xml", page1),
        ("visio/masters/masters.xml", masters),
        ("visio/masters/_rels/masters.xml.rels", master_rels),
        ("visio/masters/master1.xml", master1),
    ] {
        w.start_file(name, opts).unwrap();
        std::io::Write::write_all(&mut w, data.as_bytes()).unwrap();
    }
    w.finish().unwrap();
    path
}

fn run_shell(name: &str, app: App) {
    let mut app = app;
    // Goldens must render identically on every machine: pin text to
    // the embedded Carlito rather than this machine's installed fonts.
    dc_ui::svgimg::pin_embedded_fonts_for_tests();
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1280.0, 800.0))
        .build_ui(move |ui| {
            dc_ui::theme::install(ui.ctx());
            app.draw(ui);
        });
    harness.run();
    harness.run(); // second pass: textures and fit settle
    // Tolerances: anti-aliasing differs per GPU/driver (CI runners use
    // software adapters), which kittest's own README calls out — accept
    // small per-pixel deltas and a small budget of failed pixels while
    // still catching real layout regressions.
    harness.snapshot_options(
        name,
        &egui_kittest::SnapshotOptions::new()
            .threshold(3.0)
            .max_failed_pixels(10_000),
    );
}

#[test]
fn shell_snapshot_home() {
    let tmp = tempfile::tempdir().unwrap();
    run_shell("dc_ui_shell_home", App::with_doc(&fixture(tmp.path())));
}

#[test]
fn shell_snapshot_view_tab() {
    let tmp = tempfile::tempdir().unwrap();
    let mut app = App::with_doc(&fixture(tmp.path()));
    app.set_tab_for_test(dc_ui::ribbon::Tab::View);
    run_shell("dc_ui_shell_view", app);
}

#[test]
fn shell_snapshot_selected_shapesheet() {
    let tmp = tempfile::tempdir().unwrap();
    let mut app = App::with_doc(&fixture(tmp.path()));
    app.select_shape_for_test(0, 1);
    app.set_tab_for_test(dc_ui::ribbon::Tab::Developer);
    run_shell("dc_ui_shell_shapesheet", app);
}

#[test]
fn shell_snapshot_backstage() {
    let tmp = tempfile::tempdir().unwrap();
    let mut app = App::with_doc(&fixture(tmp.path()));
    app.backstage = true;
    run_shell("dc_ui_shell_backstage", app);
}

#[test]
fn shell_snapshot_blank_drawing() {
    // File → New: empty page with the starter shape masters in the pane.
    run_shell("dc_ui_shell_blank", App::with_blank());
}

#[test]
fn shell_snapshot_start_recent() {
    // Start screen with a populated MRU: one live entry, one ghosted
    // (its file is gone — Visio keeps it listed, grayed).
    let tmp = tempfile::tempdir().unwrap();
    let doc = fixture(tmp.path());
    let ghost = tmp.path().join("moved-away.vsdx");
    let mut app = App::with_doc_opt(None);
    app.set_recent_for_test(vec![doc, ghost]);
    run_shell("dc_ui_shell_start_recent", app);
}

#[test]
fn recent_entry_click_reopens() {
    use egui_kittest::kittest::Queryable;
    let tmp = tempfile::tempdir().unwrap();
    let doc = fixture(tmp.path());
    let mut app = App::with_doc_opt(None);
    app.set_recent_for_test(vec![doc.clone()]);
    dc_ui::svgimg::pin_embedded_fonts_for_tests();
    let mut harness = egui_kittest::Harness::builder()
        .with_size(egui::vec2(1280.0, 800.0))
        .build_ui_state(
            |ui, app: &mut App| {
                dc_ui::theme::install(ui.ctx());
                app.draw(ui);
            },
            app,
        );
    harness.run();
    assert_eq!(harness.state().debug_shape_count(), 0, "no document yet");
    // Click the MRU entry (labeled with the file name) on the start
    // screen; the document opens.
    harness.get_by_label_contains("shell.vssx").click();
    harness.run();
    assert_eq!(
        harness.state().debug_shape_count(),
        1,
        "clicking a recent entry reopened the document"
    );
    assert_eq!(harness.state().debug_recent(), &[doc]);
}
