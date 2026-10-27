//! Headless render-path bench (GitHub #89): drives the full shell over
//! synthetic documents and prints the per-stage cost split recorded by
//! `dc_ui::perf` — serialize, font resolve, parse, render, upload, and
//! whole frames — for the scenarios that matter on #66: document open,
//! idle cached frames, a plain shape drag, a resize-handle drag (both
//! preview-only during the gesture; the commit frame after release
//! pays the full-page re-raster), and a fresh raster at a higher zoom
//! bucket.
//!
//! Informational, not a pass/fail gate: run with
//! `cargo test -p dc-ui --release --test render_perf -- --ignored --nocapture`.
//! Fixtures are generated in code (repo policy: no Microsoft content).
//! Fonts are NOT pinned to the embedded set here — the bench measures
//! the production path, whose font-chain resolve scans installed
//! families; numbers are host-specific.

use dc_ui::app::App;
use dc_ui::perf::{self, Stat};
use egui_kittest::Harness;
use std::time::Instant;

/// A synthetic document: `n_shapes` text-bearing rectangles on a grid
/// (shape 1 sits at the page center so a center click selects it) and
/// `n_masters` stencil masters, packaged as a minimal .vsdx the same
/// way the shell fixtures are.
fn many_shape_fixture(dir: &std::path::Path, n_shapes: usize, n_masters: usize) -> std::path::PathBuf {
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

fn mount(app: App) -> Harness<'static, App> {
    // Sync rasters (fonts NOT pinned — the bench measures the
    // production font path): stage attribution stays deterministic,
    // and `idle_frames_skip_the_page_pipeline` never depends on
    // worker timing (#92).
    dc_ui::svgimg::force_sync_raster_for_tests();
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

/// Print one scenario's stage deltas as a fixed-width table row block.
fn report(label: &str, d: &[Stat], wall: std::time::Duration) {
    let frames = d.iter().find(|s| s.stage == "frame").map(|s| s.count).unwrap_or(0);
    println!("-- {label} (frames: {frames}, wall: {wall:.0?})");
    for s in d {
        if s.count == 0 || s.stage == "frame" {
            continue;
        }
        let per_call_us = s.ns as f64 / s.count as f64 / 1000.0;
        let per_frame_us = if frames > 0 { s.ns as f64 / frames as f64 / 1000.0 } else { 0.0 };
        println!(
            "   {:<14} n={:<4} total={:>9.1}ms  per-call={:>8.1}µs  per-frame={:>8.1}µs",
            s.stage,
            s.count,
            s.ns as f64 / 1e6,
            per_call_us,
            per_frame_us
        );
    }
}

fn press(h: &mut Harness<'_, App>, pos: egui::Pos2) {
    h.hover_at(pos);
    h.event(egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
}

fn release(h: &mut Harness<'_, App>, pos: egui::Pos2) {
    h.event(egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    h.run();
}

#[test]
#[ignore = "informational perf bench (GitHub #89); run with --release --ignored --nocapture"]
fn render_perf_stage_split() {
    let host = std::env::var("HOSTNAME").unwrap_or_else(|_| "unknown".into());
    println!("== render-path stage split (GitHub #89) — host: {host} ==");

    for &n in &[500usize, 2000] {
        println!("\n===== {n} shapes =====");
        let tmp = tempfile::tempdir().unwrap();
        let path = many_shape_fixture(tmp.path(), n, 6);

        // --- open: first frames pay page raster + thumbnail warm-up ---
        perf::reset();
        let mut h = mount(App::with_doc(&path));
        let t = Instant::now();
        h.run();
        h.run(); // second pass: textures and fit settle
        let open = perf::snapshot();
        report("open (2 frames)", &open, t.elapsed());

        // --- idle cached frames: nothing changes ---
        let base = perf::snapshot();
        let t = Instant::now();
        for _ in 0..20 {
            h.run();
        }
        report("idle x20", &perf::diff(&base, &perf::snapshot()), t.elapsed());

        // --- plain shape drag: preview-only move frames + one
        //     mutation frame on release ---
        let center = h.state().debug_canvas_rect().center();
        press(&mut h, center); // press on the center shape arms a move-drag
        let base = perf::snapshot();
        let t = Instant::now();
        for k in 1..=8u32 {
            h.hover_at(center + egui::vec2(k as f32 * 4.0, -(k as f32 * 3.0)));
            h.run();
        }
        let mid = perf::snapshot();
        report("shape drag x8 (preview frames)", &perf::diff(&base, &mid), t.elapsed());
        let t = Instant::now();
        release(&mut h, center + egui::vec2(32.0, -24.0)); // commits MoveShape
        report("drag release (mutation frame)", &perf::diff(&mid, &perf::snapshot()), t.elapsed());

        // --- resize-handle drag: preview-only move frames (the resize
        //     commits once on release, like the move drag — verified
        //     against the handle_drag_done handler), then the commit
        //     frame pays the full-page re-raster ---
        h.state().debug_sel().expect("shape selected after drag");
        let corners = h.state().debug_sel_corners().expect("handles for selection");
        press(&mut h, corners[1]); // top-right corner
        let base = perf::snapshot();
        let t = Instant::now();
        for k in 1..=4u32 {
            h.hover_at(corners[1] + egui::vec2(k as f32 * 8.0, -(k as f32 * 5.0)));
            h.run();
        }
        let mid = perf::snapshot();
        report("resize-handle drag x4 (preview frames)", &perf::diff(&base, &mid), t.elapsed());
        let t = Instant::now();
        release(&mut h, corners[1] + egui::vec2(32.0, -20.0));
        report("resize commit (mutation frame)", &perf::diff(&mid, &perf::snapshot()), t.elapsed());

        // --- zoom bucket: a fresh harness mounted at 200% pays the new
        // bucket's raster (same work a mid-session bucket change
        // pays: same version, new bucket → render+upload only, the
        // parsed tree is reused) ---
        let mut app = App::with_doc(&path);
        app.set_zoom_for_test(2.0);
        perf::reset();
        let mut h2 = mount(app);
        let t = Instant::now();
        h2.run();
        h2.run();
        report("open at 200% zoom (new bucket raster)", &perf::snapshot(), t.elapsed());

        // --- zoom sweep (#91/#93): one harness, several bucket
        //     crossings without a model change — layers re-RENDER at
        //     the new width and never re-serialize (the strings are
        //     retained per version) ---
        let app = App::with_doc(&path);
        let mut h3 = mount(app);
        h3.run();
        h3.run(); // warm the tree at the fit zoom
        let base = perf::snapshot();
        let t = Instant::now();
        for &z in &[1.25f32, 1.5, 1.75, 2.0] {
            h3.state_mut().set_zoom_for_test(z);
            h3.run();
        }
        report("zoom sweep x4 buckets (tree reuse)", &perf::diff(&base, &perf::snapshot()), t.elapsed());
    }
    println!("\n(done)");
}

/// #90 + #93 regression, in one sequential fn (the perf counters are
/// process-global — parallel test threads would smear them): idle
/// The per-n leg of the flatness gate: a committed MOVE re-renders
/// nothing and a resize-handle commit re-renders exactly one layer
/// (n=1 per stage).
fn move_and_resize_commits_are_flat(n: usize, tmp: &std::path::Path) {
    let path = many_shape_fixture(tmp, n, 4);
    let mut h = mount(App::with_doc(&path));
    h.run();
    h.run(); // fill + settle (sync seam: everything lands inline)

    // --- move commit: O(0) ---
    let center = h.state().debug_canvas_rect().center();
    press(&mut h, center);
    let base = perf::snapshot();
    h.hover_at(center + egui::vec2(24.0, -18.0));
    h.run();
    release(&mut h, center + egui::vec2(24.0, -18.0));
    let d = perf::diff(&base, &perf::snapshot());
    for s in &d {
        if s.stage.starts_with("page/") {
            assert_eq!(s.count, 0, "{} ran {}× for a move (n={})", s.stage, s.count, n);
        }
    }

    // --- resize commit: exactly one layer ---
    h.state().debug_sel().expect("shape selected after move");
    let corners = h.state().debug_sel_corners().expect("handles");
    press(&mut h, corners[1]);
    let base = perf::snapshot();
    h.hover_at(corners[1] + egui::vec2(16.0, -12.0));
    h.run();
    release(&mut h, corners[1] + egui::vec2(16.0, -12.0));
    let d = perf::diff(&base, &perf::snapshot());
    for s in &d {
        for stage in ["page/svg", "page/resolve", "page/parse", "page/render", "page/upload"] {
            if s.stage == stage {
                assert_eq!(
                    s.count, 1,
                    "{} ran {}× for a one-shape resize (n={}) — must be flat in page size",
                    s.stage, s.count, n
                );
            }
        }
    }
}

/// #90/#93 acceptance gate: idle cached frames add ZERO page work, a
/// drag-release MOVE re-renders nothing (the layer texture is
/// translation-invariant), and a resize-handle commit re-renders
/// exactly ONE layer (n=1 per stage) — the flatness #93's verification
/// asks for, guarded headlessly under the sync seam. Runs at n=500 in
/// the default (debug) suite; the n=2000 leg is the ignored twin below
/// (debug builds pay superlinearly for the 4× shapes — #97 §4 split).
#[test]
fn idle_frames_are_free_and_commits_scale_with_the_shape() {
    // --- #90: idle frames skip the page pipeline entirely ---
    {
        let tmp = tempfile::tempdir().unwrap();
        let path = many_shape_fixture(tmp.path(), 40, 4);
        let mut h = mount(App::with_doc(&path));
        h.run();
        h.run(); // let the fit/zoom and layers settle
        let base = perf::snapshot();
        for _ in 0..20 {
            h.run();
        }
        let d = perf::diff(&base, &perf::snapshot());
        for s in &d {
            if s.stage.starts_with("page/") {
                assert_eq!(
                    s.count, 0,
                    "{} ran {}× across 20 idle cached frames",
                    s.stage, s.count
                );
            }
        }
        let frames = d.iter().find(|s| s.stage == "frame").map(|s| s.count).unwrap();
        assert_eq!(frames, 20);
    }

    let tmp = tempfile::tempdir().unwrap();
    move_and_resize_commits_are_flat(500, tmp.path());
}

/// The n=2000 leg of the flatness gate — same asserts, off the default
/// path. Run with the release recipe (alongside the stage-split bench):
/// `cargo test -p dc-ui --release --test render_perf -- --ignored
/// --nocapture`.
#[test]
#[ignore = "n=2000 twin of the flatness gate (release recipe; the n=500 leg runs by default)"]
fn commits_scale_with_the_shape_at_2000() {
    let tmp = tempfile::tempdir().unwrap();
    move_and_resize_commits_are_flat(2000, tmp.path());
}
