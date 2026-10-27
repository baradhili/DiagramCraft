//! Async rasterization behavior (GitHub #92) — the one test binary
//! that does NOT set the sync determinism seam, so the raster pool
//! really runs workers. What is provable headlessly without timing
//! flakiness: completions only ever land at the START of a frame's
//! drain, so (a) after the first `run` the page texture is absent even
//! if a worker already finished, (b) once it appears, exactly one page
//! parse happened (#91 tree reuse held off-thread), and (c) the
//! stencil pane populates progressively — no single frame may ever
//! queue more than the per-frame post cap (a count-after-N-frames
//! assert would race `mount`'s run-to-stable pump, whose pass count
//! is machine-speed-dependent: on a slow runner every pass is long
//! enough for the workers to finish, so ALL masters can legitimately
//! be textured by the first explicit step — observed on the 2-core
//! CI runner).
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

/// The async contract of #92/#93, in one sequential flow (one fn: the
/// perf counters are process-global, so parallel tests would smear
/// them).
#[test]
fn rasters_run_off_thread_and_land_on_later_frames() {
    let tmp = tempfile::tempdir().unwrap();

    // --- Phase 1: a large drawing's open does not block on any page
    //     raster (#92). Completions only land at a frame's drain, so
    //     after the first step the layers are absent regardless of
    //     worker speed — a structural guarantee, not a timing race. ---
    let path = fixture(tmp.path(), 1200, 4);
    let mut h = mount(App::with_doc(&path));
    h.step();
    // The post cap makes this deterministic: at most a few dozen small
    // layer jobs can land inside one step (a pre-#92 inline render
    // would have the WHOLE page done).
    let landed = h.state().debug_layer_count_for_test();
    assert!(
        landed < 1200,
        "first step rasterized all {landed} layers — nothing should run inline/serially"
    );
    pump_until(
        &mut h,
        |a| a.debug_page_tex_id_for_test().is_some(),
        "the first layer to land",
    );
    // No whole-page parse ever runs (#93): every page/parse call is a
    // per-shape layer — a monolith parse of this document costs ~half
    // a second; nothing here may come close.
    let parse = perf::snapshot()
        .into_iter()
        .find(|s| s.stage == "page/parse")
        .map(|s| (s.count, s.ns))
        .unwrap_or((0, 0));
    assert!(parse.0 > 0, "layers did rasterize");
    let mean_ms = parse.1 as f64 / parse.0 as f64 / 1e6;
    assert!(
        mean_ms < 50.0,
        "mean page/parse {mean_ms:.1}ms — a whole-page parse ran"
    );
    // The stencil pane (4 masters here) also filled in off-thread.
    pump_until(
        &mut h,
        |a| a.debug_stencil_thumb_count_for_test() == 4,
        "all stencil thumbnails",
    );
    // Quiesce the page fill before the move measurement: the O(0)
    // claim below is about the MOVE, not the tail of the fill.
    pump_until(
        &mut h,
        |a| a.debug_layer_count_for_test() == 1200,
        "all 1200 layers to land",
    );

    // --- #93 acceptance: a committed MOVE re-renders nothing. Drag
    //     shape 1 (page center) and drop it elsewhere; the layer
    //     texture is translation-invariant, so the SAME texture id
    //     keeps showing and no serialize/parse/render runs. ---
    let tex_before = h.state().debug_layer_tex_id_for_test(1).expect("layer 1");
    let center = h.state().debug_canvas_rect().center();
    let before = perf::snapshot();
    h.hover_at(center);
    h.event(egui::Event::PointerButton {
        pos: center,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
    h.step();
    h.hover_at(center + egui::vec2(40.0, -30.0));
    h.step();
    h.event(egui::Event::PointerButton {
        pos: center + egui::vec2(40.0, -30.0),
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    h.step();
    let d = perf::diff(&before, &perf::snapshot());
    for s in &d {
        if s.stage == "page/svg" || s.stage == "page/parse" || s.stage == "page/render" {
            assert_eq!(
                s.count, 0,
                "{} ran {}× for a pure move — moves must be O(0) (#93)",
                s.stage, s.count
            );
        }
    }
    let tex_after = h
        .state()
        .debug_layer_tex_id_for_test(1)
        .expect("layer 1 after move");
    assert_eq!(
        tex_before, tex_after,
        "a move must keep the layer texture (translation-invariant ink)"
    );
    drop(h); // joins the pool's workers

    // --- Phase 2: the #92 guarantee is the per-frame POST CAP, not a
    //     landed count after a fixed number of frames. `mount` pumps
    //     run-to-stable internally: how many passes that takes (and
    //     how many worker completions land inside them) depends on
    //     machine speed — on the 2-core CI runner all 100 masters were
    //     legitimately textured by the first explicit step, which is
    //     fine: the cap still spread the WORK across frames. What must
    //     hold everywhere: no single frame ever queued more than the
    //     cap — the pre-#92 serial burst queued all 121 at once. ---
    perf::reset();
    let path = fixture(tmp.path(), 20, 100);
    let mut h = mount(App::with_doc(&path));
    // #102: the document entry is opt-in — reveal it so all 100 masters
    // (not just Quick Shapes' head 6) get pane thumbnails.
    h.state_mut().show_doc_stencil_for_test();
    h.step();
    let peak = h.state().debug_pool_peak_posts_per_frame_for_test();
    assert!(peak > 0, "no raster job was ever queued");
    let cap = h.state().debug_pool_post_cap_for_test();
    assert!(
        peak <= cap,
        "one frame queued {peak} jobs — the per-frame cap ({cap}) must stagger posts (#92)"
    );
    pump_until(
        &mut h,
        |a| a.debug_stencil_thumb_count_for_test() == 100,
        "all 100 thumbnails",
    );
    // Every master went through the pool (a texture only materializes
    // from its own posted job; re-posts of still-missing entries only
    // push the count higher).
    assert!(
        h.state().debug_pool_total_posts_for_test() >= 100,
        "masters were textured without pool posts — impossible; hook is wrong"
    );
}

/// #93's memory bound: culling must not just stop RASTERIZING
/// off-screen shapes — entries scrolled far off the viewport release
/// their GPU textures (svg/version retained), and scrolling back
/// restores them. Deterministic: wheel-zoom is applied step by step
/// and eviction happens synchronously in composite; only the
/// re-rasters on the way back need the pump.
#[test]
fn far_off_screen_layers_release_their_textures() {
    let tmp = tempfile::tempdir().unwrap();
    // 40 shapes: the center rectangle (id 1) plus a 2-row grid along
    // the page bottom (ids 2..=40).
    let path = fixture(tmp.path(), 40, 1);
    let mut h = mount(App::with_doc(&path));
    pump_until(&mut h, |a| a.debug_layer_count_for_test() == 40, "all 40 layers");
    let center = h.state().debug_canvas_rect().center();
    h.hover_at(center);
    h.step();
    // Zoom in hard — the canvas wheel applies one 1.1x step per frame
    // (clamped at 8x). At 8x the visible window plus the 2in evict
    // margin is a couple of inches tall around the page center: the
    // bottom-row grid (5in below the anchor) must release, the center
    // shape must keep its texture.
    for _ in 0..30 {
        h.event(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, 120.0),
            phase: egui::TouchPhase::Move,
            source: egui::MouseWheelSource::Unknown,
            modifiers: egui::Modifiers::NONE,
        });
        h.step();
    }
    let kept = h.state().debug_layer_count_for_test();
    assert!(kept < 40, "off-screen layers must release their textures (kept {kept}/40)");
    assert!(
        h.state().debug_layer_tex_id_for_test(1).is_some(),
        "the zoom-focus shape keeps its texture"
    );
    // Zoom back out through fit and wait: every layer re-rasterizes
    // (the bucket swept) and the count returns to all 40.
    for _ in 0..35 {
        h.event(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, -120.0),
            phase: egui::TouchPhase::Move,
            source: egui::MouseWheelSource::Unknown,
            modifiers: egui::Modifiers::NONE,
        });
        h.step();
    }
    pump_until(&mut h, |a| a.debug_layer_count_for_test() == 40, "all 40 layers back");
}
