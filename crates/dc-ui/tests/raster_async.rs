//! Async rasterization behavior (GitHub #92) — the one test binary
//! that does NOT set the sync determinism seam, so the raster pool
//! really runs workers. What is provable headlessly without timing
//! flakiness: completions only ever land at the START of a frame's
//! drain, so (a) after the first `run` the page texture is absent even
//! if a worker already finished, (b) once it appears, exactly one page
//! parse happened (#91 tree reuse held off-thread), and (c) the
//! stencil pane populates progressively — the per-frame post cap means
//! far fewer than all masters are textured after the first run.
//! Fixtures are generated in code (repo policy: no Microsoft content).

use dc_ui::app::App;
use dc_ui::perf;
use egui_kittest::Harness;
use std::time::{Duration, Instant};

/// A synthetic document: `n_shapes` text-bearing rectangles on a grid,
/// `n_masters` stencil masters — the same minimal .vsdx layout the
/// other harnesses use (see render_perf.rs).
fn fixture(dir: &std::path::Path, n_shapes: usize, n_masters: usize) -> std::path::PathBuf {
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

    let path = dir.join(format!("async-{n_shapes}x{n_masters}.vsdx"));
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

fn mount(app: App) -> Harness<'static, App> {
    Harness::builder()
        .with_size(egui::vec2(1280.0, 800.0))
        .build_ui_state(
            |ui, app: &mut App| {
                dc_ui::theme::install(ui.ctx());
                app.draw(ui);
            },
            app,
        )
}

/// Pump frames until `f` holds (workers are async; the drain at each
/// frame's start is what lands their results).
fn pump_until(h: &mut Harness<'_, App>, mut f: impl FnMut(&App) -> bool, what: &str) {
    let deadline = Instant::now() + Duration::from_secs(60);
    while !f(h.state()) {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        h.step();
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// The async contract of #92, in one sequential flow (one fn: the perf
/// counters are process-global, so parallel tests would smear them).
#[test]
fn rasters_run_off_thread_and_land_on_later_frames() {
    let tmp = tempfile::tempdir().unwrap();

    // --- Phase 1: a large drawing's open does not block on the page
    //     raster. 1200 shapes make the parse take seconds — far beyond
    //     the first run's layout passes — so the absence below is a
    //     structural guarantee (completions land at frame start), not
    //     a timing race. ---
    let path = fixture(tmp.path(), 1200, 4);
    let mut h = mount(App::with_doc(&path));
    h.step();
    assert!(
        h.state().debug_page_tex_id_for_test().is_none(),
        "first paint must not carry the page raster (#92)"
    );
    pump_until(
        &mut h,
        |a| a.debug_page_tex_id_for_test().is_some(),
        "the page texture to land",
    );
    // Exactly one page parse across open + zoom-settle: the tree came
    // back from the worker and bucket re-renders reused it (#91).
    let parses = perf::snapshot()
        .into_iter()
        .find(|s| s.stage == "page/parse")
        .map(|s| s.count)
        .unwrap_or(0);
    assert_eq!(parses, 1, "one parse even though buckets changed");
    // The stencil pane (4 masters here) also filled in off-thread.
    pump_until(
        &mut h,
        |a| a.debug_stencil_thumb_count_for_test() == 4,
        "all stencil thumbnails",
    );
    drop(h); // joins the pool's workers

    // --- Phase 2: a 40-master stencil pane populates progressively —
    //     the per-frame post cap keeps the first run from rasterizing
    //     every master at once (the pre-#92 serial burst). ---
    perf::reset();
    let path = fixture(tmp.path(), 20, 40);
    let mut h = mount(App::with_doc(&path));
    h.step();
    let after_first = h.state().debug_stencil_thumb_count_for_test();
    assert!(
        after_first < 40,
        "first run textured {after_first}/40 masters — the cap should stagger them"
    );
    pump_until(
        &mut h,
        |a| a.debug_stencil_thumb_count_for_test() == 40,
        "all 40 thumbnails",
    );
}
