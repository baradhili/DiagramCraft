//! Headless shell snapshot (egui_kittest): renders the full desktop
//! shell — title band, ribbon, stencil pane, canvas, page tabs, status
//! bar — over a synthetic package (no Microsoft content) and writes
//! PNGs for review under target/kittest-snapshots/.

use dc_ui::app::App;
use egui_kittest::Harness;

fn fixture(dir: &std::path::Path) -> std::path::PathBuf {
    fixture_pages(dir, &["Page-1"])
}

/// The shell fixture with N named pages (#54): identical page bodies,
/// sequential rIds and parts, all through the same zip layout.
fn fixture_pages(dir: &std::path::Path, names: &[&str]) -> std::path::PathBuf {
    let document = r#"<?xml version="1.0" encoding="utf-8"?>
<VisioDocument xmlns='http://schemas.microsoft.com/office/visio/2012/main' xmlns:r='http://schemas.openxmlformats.org/officeDocument/2006/relationships'>
<StyleSheets><StyleSheet ID='0' NameU='No Style'>
  <Cell N='LineWeight' V='0.01041666666666667'/><Cell N='LineColor' V='0'/>
  <Cell N='FillForegnd' V='1'/><Cell N='FillPattern' V='1'/>
</StyleSheet></StyleSheets>
</VisioDocument>"#;
    let entries: String = names
        .iter()
        .enumerate()
        .map(|(i, name)| {
            format!(
                "<Page ID='{i}' NameU='{name}' Name='{name}'>\n\
<PageSheet LineStyle='0' FillStyle='0' TextStyle='0'>\n  \
<Cell N='PageWidth' V='8.5'/><Cell N='PageHeight' V='11'/>\n\
</PageSheet>\n\
<Rel r:id='rId{}'/>\n\
</Page>\n",
                i + 1
            )
        })
        .collect();
    let pages = format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<Pages xmlns='http://schemas.microsoft.com/office/visio/2012/main' xmlns:r='http://schemas.openxmlformats.org/officeDocument/2006/relationships'>
{entries}</Pages>"#
    );
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
    let page_rels = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
{}</Relationships>"#,
        names
            .iter()
            .enumerate()
            .map(|(i, _)| format!(
                "<Relationship Id='rId{}' Type='http://schemas.microsoft.com/visio/2010/relationships/page' Target='page{}.xml'/>",
                i + 1,
                i + 1
            ))
            .collect::<String>()
    );
    let master_rels = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.microsoft.com/visio/2010/relationships/master" Target="master1.xml"/>
</Relationships>"#;
    let page_overrides: String = names
        .iter()
        .enumerate()
        .map(|(i, _)| {
            format!(
                "<Override PartName=\"/visio/pages/page{}.xml\" ContentType=\"application/vnd.ms-visio.pages+xml\"/>\n",
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
{page_overrides}<Override PartName="/visio/masters/masters.xml" ContentType="application/vnd.ms-visio.masters+xml"/>
<Override PartName="/visio/masters/master1.xml" ContentType="application/vnd.ms-visio.master+xml"/>
</Types>"#
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

    let path = dir.join("shell.vsdx");
    let file = std::fs::File::create(&path).unwrap();
    let mut w = zip::ZipWriter::new(file);
    let opts = zip::write::SimpleFileOptions::default();
    let mut parts: Vec<(String, String)> = [
        ("[Content_Types].xml".to_string(), content_types),
        ("_rels/.rels".to_string(), root_rels.to_string()),
        ("visio/document.xml".to_string(), document.to_string()),
        (
            "visio/_rels/document.xml.rels".to_string(),
            doc_rels.to_string(),
        ),
        ("visio/pages/pages.xml".to_string(), pages),
        ("visio/pages/_rels/pages.xml.rels".to_string(), page_rels),
        ("visio/masters/masters.xml".to_string(), masters.to_string()),
        (
            "visio/masters/_rels/masters.xml.rels".to_string(),
            master_rels.to_string(),
        ),
        ("visio/masters/master1.xml".to_string(), master1.to_string()),
    ]
    .into_iter()
    .collect();
    for i in 0..names.len() {
        parts.insert(
            6 + i,
            (format!("visio/pages/page{}.xml", i + 1), page1.to_string()),
        );
    }
    for (name, data) in &parts {
        w.start_file(name.as_str(), opts).unwrap();
        std::io::Write::write_all(&mut w, data.as_bytes()).unwrap();
    }
    w.finish().unwrap();
    path
}

fn run_shell(name: &str, app: App) {
    run_shell_mode(name, app, false);
}

/// Snapshot the shell in one visual mode. Light keeps the exact
/// existing goldens; dark installs the same both-slot theme and
/// selects Dark, exercising the OS-dark path end-to-end (GitHub #52).
fn run_shell_mode(name: &str, app: App, dark: bool) {
    let mut app = app;
    // Goldens must render identically on every machine: pin text to
    // the embedded Carlito rather than this machine's installed fonts.
    dc_ui::svgimg::pin_determinism_for_tests();
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1280.0, 800.0))
        .build_ui(move |ui| {
            dc_ui::theme::install_for(
                ui.ctx(),
                if dark {
                    egui::ThemePreference::Dark
                } else {
                    egui::ThemePreference::Light
                },
            );
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

/// #62: tiered ruler ticks at a second density — zoomed in 2×, the
/// major steps halve and quarter subdivisions stay legible (the fit-
/// mode goldens above cover the default density).
#[test]
fn shell_snapshot_rulers_zoomed() {
    let tmp = tempfile::tempdir().unwrap();
    let mut app = App::with_doc(&fixture(tmp.path()));
    app.set_zoom_for_test(2.0);
    run_shell("dc_ui_shell_rulers_zoom", app);
}

/// GitHub #52: the same home shell under the dark mode the OS watcher
/// selects when the desktop is dark — dark chrome (ribbon, rulers,
/// desk, tabs, status), white page paper, brand-green bands.
#[test]
fn shell_snapshot_home_dark() {
    let tmp = tempfile::tempdir().unwrap();
    run_shell_mode(
        "dc_ui_shell_home_dark",
        App::with_doc(&fixture(tmp.path())),
        true,
    );
}

/// GitHub #52: the blank drawing (stencil pane + empty page) in dark
/// mode — covers the stencil pane and page-tab strip counterparts.
#[test]
fn shell_snapshot_blank_dark() {
    run_shell_mode("dc_ui_shell_blank_dark", App::with_blank(), true);
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
    // The #55 landing rule: with a document the File screen opens on
    // Info (2010 three-band rail: commands, destinations, Options).
    // The path is pinned: the Info page shows the file's Location and
    // goldens must not embed this run's random tempdir (that made the
    // snapshot differ per OS/runner — the Windows/macOS CI failures).
    app.set_path_for_test(std::path::PathBuf::from("/golden/shell.vssx"));
    app.open_backstage_for_test();
    run_shell("dc_ui_shell_backstage", app);
}

#[test]
fn info_thumbnail_refreshes_after_edit() {
    // #58: the Info page carries a live page thumbnail keyed by
    // (page, version) — an edit must re-rasterize it (new texture id).
    let tmp = tempfile::tempdir().unwrap();
    let mut app = App::with_doc(&fixture(tmp.path()));
    app.set_path_for_test(std::path::PathBuf::from("/golden/shell.vssx"));
    app.open_backstage_for_test();
    dc_ui::svgimg::pin_determinism_for_tests();
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
    harness.run(); // second pass: the raster settles
    let id1 = harness.state().info_thumb_tex_id_for_test();
    assert!(
        id1.is_some(),
        "Info page should have rasterized a thumbnail"
    );
    // What every editing command does: dirty + version bump.
    harness.state_mut().simulate_edit_for_test();
    harness.run();
    let id2 = harness.state().info_thumb_tex_id_for_test();
    assert!(id2.is_some());
    assert_ne!(id1, id2, "an edit must invalidate the thumbnail cache");
}

#[test]
fn info_thumbnail_placeholder_on_render_failure() {
    // #58: a failed render leaves the neutral card (no error text, no
    // texture) and must not panic.
    let tmp = tempfile::tempdir().unwrap();
    let mut app = App::with_doc(&fixture(tmp.path()));
    app.set_path_for_test(std::path::PathBuf::from("/golden/shell.vssx"));
    app.force_info_render_failure_for_test(true);
    app.open_backstage_for_test();
    dc_ui::svgimg::pin_determinism_for_tests();
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
    harness.run();
    assert!(harness.state().info_thumb_tex_id_for_test().is_none());
}

#[test]
fn shell_snapshot_backstage_recent() {
    // No document → the Recent page, with its recents list.
    let tmp = tempfile::tempdir().unwrap();
    let doc = fixture(tmp.path());
    let ghost = tmp.path().join("moved-away.vsdx");
    let mut app = App::with_doc_opt(None);
    app.set_recent_for_test(vec![doc, ghost]);
    app.open_backstage_for_test();
    run_shell("dc_ui_shell_backstage_recent", app);
}

#[test]
fn shell_snapshot_backstage_new() {
    // The search-first New page (gallery + search box, #55).
    let mut app = App::with_blank();
    app.open_backstage_for_test();
    app.set_backstage_page_for_test("new");
    run_shell("dc_ui_shell_backstage_new", app);
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
    dc_ui::svgimg::pin_determinism_for_tests();
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
    harness.get_by_label_contains("shell.vsdx").click();
    harness.run();
    assert_eq!(
        harness.state().debug_shape_count(),
        1,
        "clicking a recent entry reopened the document"
    );
    assert_eq!(harness.state().debug_recent(), &[doc]);
}

/// GitHub #56: the painted controls — the File tab block, the Tell-Me
/// box, the QAT icon buttons, the ribbon galley captions — carry
/// accessible names on their responses, so kittest finds them by label
/// (and screen readers announce them).
#[test]
fn painted_shell_controls_carry_accessible_labels() {
    use egui_kittest::kittest::Queryable;
    dc_ui::svgimg::pin_determinism_for_tests();
    let mut harness = egui_kittest::Harness::builder()
        .with_size(egui::vec2(1280.0, 800.0))
        .build_ui_state(
            |ui, app: &mut App| {
                dc_ui::theme::install(ui.ctx());
                app.draw(ui);
            },
            App::with_blank(),
        );
    harness.run();

    // File tab: a painted green block; the label must exist and the
    // click must still open the backstage.
    let file = harness.get_by_label("File");
    assert!(
        file.rect().height() > 20.0,
        "the File tab block carries the label"
    );
    file.click();
    harness.run();
    assert!(
        harness.state().backstage,
        "clicking the labeled File tab opens the backstage"
    );
    harness.state_mut().backstage = false;
    harness.run();

    // Tell-Me box: painted placeholder + lightbulb icon.
    harness.get_by_label("Tell Me");

    // QAT buttons: painted icons, names without the shortcut hint.
    for name in ["Save", "Undo", "Redo"] {
        assert!(
            harness.query_by_label(name).is_some(),
            "QAT button {name} is labeled"
        );
    }

    // Ribbon hero caption: painted galley text.
    assert!(
        harness.query_by_label("Paste").is_some(),
        "the ribbon Paste button is labeled"
    );
}

// ---------------------------------------------------------------------------
// Multi-page tabs (GitHub #54)
// ---------------------------------------------------------------------------

/// Pointer click at a position (the interactions.rs convention: press +
/// release queued together, then settle).
fn click_at(h: &mut Harness<'_, App>, pos: egui::Pos2, button: egui::PointerButton) {
    let b = |pressed| egui::Event::PointerButton {
        pos,
        button,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    h.hover_at(pos);
    h.event(b(true));
    h.event(b(false));
    h.run();
}

/// Right-click the named tab and pick a context-menu row (the short
/// node — the menu container's aggregated label also matches).
fn tab_menu_pick(h: &mut Harness<'_, App>, tab: &str, item: &str) {
    use egui_kittest::kittest::Queryable;
    let pos = h.get_by_label(tab).rect().center();
    click_at(h, pos, egui::PointerButton::Secondary);
    let row = h
        .get_all_by_label_contains(item)
        .into_iter()
        .find(|n| n.rect().height() < 30.0)
        .unwrap_or_else(|| panic!("no {item} row in the tab context menu"));
    row.click();
    h.run();
}

#[test]
fn shell_snapshot_multi_page_tabs() {
    // #54: three tabs in the shipped strip look, the second one active.
    let tmp = tempfile::tempdir().unwrap();
    let mut app = App::with_doc(&fixture_pages(tmp.path(), &["Overview", "Detail", "Notes"]));
    app.select_shape_for_test(0, 1); // exercise a non-empty selection render
    run_shell("dc_ui_shell_pages", app);
}

#[test]
fn page_tabs_switch_restore_selection_and_name_status() {
    use egui_kittest::kittest::Queryable;
    let tmp = tempfile::tempdir().unwrap();
    let doc = fixture_pages(tmp.path(), &["One", "Two", "Three"]);
    dc_ui::svgimg::pin_determinism_for_tests();
    let mut h = egui_kittest::Harness::builder()
        .with_size(egui::vec2(1280.0, 800.0))
        .build_ui_state(
            |ui, app: &mut App| {
                dc_ui::theme::install(ui.ctx());
                app.draw(ui);
            },
            App::with_doc(&doc),
        );
    h.run();
    h.run();
    assert_eq!(h.state().debug_page_names(), vec!["One", "Two", "Three"]);
    assert_eq!(h.state().debug_active_page_name(), "One");

    // Selection is per-page (#54): set on One, cleared on Two, restored
    // when One comes back.
    h.state_mut().select_shape_for_test(0, 1);
    h.run();
    assert!(h.state().debug_sel().is_some());
    h.get_by_label("Two").click();
    h.run();
    assert_eq!(h.state().debug_active_page_name(), "Two");
    assert!(h.state().debug_sel().is_none(), "selection is per-page");
    assert!(
        h.query_all_by_label_contains("Two of 3").count() > 0,
        "status bar names the active page"
    );
    h.get_by_label("One").click();
    h.run();
    assert!(
        h.state().debug_sel().is_some(),
        "switching back restores the page's selection"
    );

    // The Visio 2010 nav cluster (#54, owner-confirmed): >> last,
    // << first, < > step.
    h.get_by_label(">>").click();
    h.run();
    assert_eq!(h.state().debug_active_page_name(), "Three");
    h.get_by_label("<<").click();
    h.run();
    assert_eq!(h.state().debug_active_page_name(), "One");
    h.get_by_label(">").click();
    h.run();
    assert_eq!(h.state().debug_active_page_name(), "Two");
    h.get_by_label("<").click();
    h.run();
    assert_eq!(h.state().debug_active_page_name(), "One");
}

#[test]
fn page_tab_context_menu_manages_pages() {
    use egui_kittest::kittest::Queryable;
    let tmp = tempfile::tempdir().unwrap();
    let doc = fixture_pages(tmp.path(), &["One", "Two", "Three"]);
    dc_ui::svgimg::pin_determinism_for_tests();
    let mut h = egui_kittest::Harness::builder()
        .with_size(egui::vec2(1280.0, 800.0))
        .build_ui_state(
            |ui, app: &mut App| {
                dc_ui::theme::install(ui.ctx());
                app.draw(ui);
            },
            App::with_doc(&doc),
        );
    h.run();
    h.run();

    // Duplicate lands the copy right after the source and activates it.
    tab_menu_pick(&mut h, "Two", "Duplicate Page");
    assert_eq!(
        h.state().debug_page_names(),
        vec!["One", "Two", "Page-4", "Three"]
    );
    assert_eq!(h.state().debug_active_page_name(), "Page-4");

    // Delete removes the duplicate and leaves the active page valid.
    tab_menu_pick(&mut h, "Page-4", "Delete Page");
    assert_eq!(h.state().debug_page_names(), vec!["One", "Two", "Three"]);

    // Reorder Pages… is Visio's dialog mechanism: Move Down on row 2
    // (Two), then Close.
    tab_menu_pick(&mut h, "Three", "Reorder Pages");
    assert!(
        h.query_all_by_label_contains("Reorder Pages").count() > 0,
        "the Reorder Pages dialog opened"
    );
    h.get_all_by_label("Move Down")
        .nth(1)
        .expect("row 2 Move Down")
        .click();
    h.run();
    assert_eq!(h.state().debug_page_names(), vec!["One", "Three", "Two"]);
    assert_eq!(
        h.state().debug_active_page_name(),
        "Three",
        "the active page follows its position, not the moved row"
    );
    h.get_by_label("Close").click();
    h.run();

    // Undo rolls the reorder back (everything went through the
    // command layer).
    h.key_press_modifiers(egui::Modifiers::CTRL, egui::Key::Z);
    h.run();
    assert_eq!(h.state().debug_page_names(), vec!["One", "Two", "Three"]);

    // The last page can never be deleted.
    tab_menu_pick(&mut h, "One", "Delete Page");
    assert_eq!(h.state().debug_page_names(), vec!["Two", "Three"]);
    tab_menu_pick(&mut h, "Two", "Delete Page");
    assert_eq!(h.state().debug_page_names(), vec!["Three"]);
    tab_menu_pick(&mut h, "Three", "Delete Page");
    assert_eq!(
        h.state().debug_page_names(),
        vec!["Three"],
        "the last page cannot be deleted"
    );

    // Insert adds a page after the active one and activates it.
    tab_menu_pick(&mut h, "Three", "Insert Page");
    assert_eq!(h.state().debug_page_names(), vec!["Three", "Page-2"]);
    assert_eq!(h.state().debug_active_page_name(), "Page-2");
}

/// #68: an inserted page carries the source page's sheet setup — the
/// tab menu's Insert Page on a letter blank produces a full letter
/// page, not the 0.1-in missing-cell sliver `page_geom` clamps to.
#[test]
fn inserted_page_carries_the_source_page_size() {
    use egui_kittest::kittest::Queryable;
    dc_ui::svgimg::pin_determinism_for_tests();
    let mut h = egui_kittest::Harness::builder()
        .with_size(egui::vec2(1280.0, 800.0))
        .build_ui_state(
            |ui, app: &mut App| {
                dc_ui::theme::install(ui.ctx());
                app.draw(ui);
            },
            App::with_blank(),
        );
    h.run();
    h.run();
    let (w0, h0) = h.state().debug_page_geom_for_test();
    assert!(
        (w0 - 8.5).abs() < 1e-9 && (h0 - 11.0).abs() < 1e-9,
        "the blank's page is letter: {w0}×{h0}"
    );

    tab_menu_pick(&mut h, "Page-1", "Insert Page");
    h.run();
    assert_eq!(
        h.state().debug_active_page_name(),
        "Page-2",
        "insert activates the new page"
    );
    let (w1, h1) = h.state().debug_page_geom_for_test();
    assert!(
        (w1 - w0).abs() < 1e-9 && (h1 - h0).abs() < 1e-9,
        "#68: the inserted page keeps the source's sheet size: {w1}×{h1}"
    );

    // The ribbon's New Page button (Action::NewPage, on the Insert
    // tab) seeds the same way — from the now-active Page-2.
    h.state_mut().set_tab_for_test(dc_ui::ribbon::Tab::Insert);
    h.run();
    let add = h.get(egui_kittest::kittest::By::default()
        .role(egui::Role::Button)
        .label("New Page"));
    add.click();
    h.run();
    assert_eq!(h.state().debug_active_page_name(), "Page-3");
    let (w2, h2) = h.state().debug_page_geom_for_test();
    assert!(
        (w2 - w0).abs() < 1e-9 && (h2 - h0).abs() < 1e-9,
        "#68: the New Page button's page also carries the source size: {w2}×{h2}"
    );
}

#[test]
fn page_tab_rename_edits_inline() {
    use egui_kittest::kittest::Queryable;
    let tmp = tempfile::tempdir().unwrap();
    let doc = fixture_pages(tmp.path(), &["One", "Two"]);
    dc_ui::svgimg::pin_determinism_for_tests();
    let mut h = egui_kittest::Harness::builder()
        .with_size(egui::vec2(1280.0, 800.0))
        .build_ui_state(
            |ui, app: &mut App| {
                dc_ui::theme::install(ui.ctx());
                app.draw(ui);
            },
            App::with_doc(&doc),
        );
    h.run();
    h.run();

    tab_menu_pick(&mut h, "One", "Rename Page");
    // The tab becomes an inline editor (the only TextInput whose value
    // is the old name — the shell carries other text fields); type a
    // new name and Enter.
    let editor = h
        .query_all_by_role(egui::Role::TextInput)
        .into_iter()
        .find(|n| n.value().as_deref() == Some("One"))
        .expect("rename editor holds the old name");
    editor.set_text_value_accesskit("Alpha");
    h.key_press(egui::Key::Enter);
    h.run();
    assert_eq!(h.state().debug_page_names(), vec!["Alpha", "Two"]);
    // The rename is one undoable command.
    h.key_press_modifiers(egui::Modifiers::CTRL, egui::Key::Z);
    h.run();
    assert_eq!(h.state().debug_page_names(), vec!["One", "Two"]);
}
