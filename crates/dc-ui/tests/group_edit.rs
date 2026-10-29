//! kittest coverage for the Group ▸ flyout and the group-edit mode
//! (GitHub #78): the flyout carries Group/Ungroup plus Open <name>;
//! opening a group isolates its members on the canvas, clicks and
//! edits target members directly, and Esc/Close Group returns to the
//! page with member edits intact (one undo each, like page edits).

use dc_ui::app::App;
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;

/// The cells of a plain rectangle at `pin` sized `w×h`, LocPin
/// centered.
fn rect_cells(id: usize, pin: (f64, f64), w: f64, h: f64, text: &str) -> String {
    format!(
        "<Shape ID='{id}' Type='Shape' LineStyle='0' FillStyle='0' TextStyle='0'>\n\
<Cell N='PinX' V='{px}'/><Cell N='PinY' V='{py}'/>\n\
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
</Shape>\n",
        px = pin.0,
        py = pin.1,
        lw = w / 2.0,
        lh = h / 2.0,
    )
}

/// A page with the group-edit shapes: outer group `Panel.9` (members
/// 10 and the nested group `Inner.20` holding 21), plus two loose
/// rectangles (12, 13) for the Group/Ungroup flyout actions, and one
/// stencil master so the pane matches the usual shell.
fn group_fixture(dir: &std::path::Path) -> std::path::PathBuf {
    let document = r#"<?xml version="1.0" encoding="utf-8"?>
<VisioDocument xmlns='http://schemas.microsoft.com/office/visio/2012/main' xmlns:r='http://schemas.openxmlformats.org/officeDocument/2006/relationships'>
<StyleSheets><StyleSheet ID='0' NameU='No Style'>
  <Cell N='LineWeight' V='0.01041666666666667'/><Cell N='LineColor' V='0'/>
  <Cell N='FillForegnd' V='1'/><Cell N='FillPattern' V='1'/>
</StyleSheet></StyleSheets>
</VisioDocument>"#;
    let pages = r#"<?xml version="1.0" encoding="utf-8"?>
<Pages xmlns='http://schemas.microsoft.com/office/visio/2012/main' xmlns:r='http://schemas.openxmlformats.org/officeDocument/2006/relationships'>
<Page ID='0' NameU='One'><PageSheet LineStyle='0' FillStyle='0' TextStyle='0'>
  <Cell N='PageWidth' V='8.5'/><Cell N='PageHeight' V='11'/>
</PageSheet><Rel r:id='rId1'/></Page>
</Pages>"#;
    let page1 = format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<PageContents xmlns='http://schemas.microsoft.com/office/visio/2012/main' xmlns:r='http://schemas.openxmlformats.org/officeDocument/2006/relationships'>
<Shapes>
<Shape ID='9' Type='Group' NameU='Panel.9' LineStyle='0' FillStyle='0' TextStyle='0'>
<Cell N='PinX' V='4'/><Cell N='PinY' V='5.5'/>
<Cell N='Width' V='3'/><Cell N='Height' V='2'/>
<Cell N='LocPinX' V='1.5'/><Cell N='LocPinY' V='1'/>
<Shapes>
{m10}
<Shape ID='20' Type='Group' NameU='Inner.20' LineStyle='0' FillStyle='0' TextStyle='0'>
<Cell N='PinX' V='2'/><Cell N='PinY' V='0.8'/>
<Cell N='Width' V='1'/><Cell N='Height' V='0.8'/>
<Cell N='LocPinX' V='0.5'/><Cell N='LocPinY' V='0.4'/>
<Shapes>
{m21}
</Shapes>
</Shape>
</Shapes>
</Shape>
{m12}
{m13}
</Shapes>
</PageContents>"#,
        m10 = rect_cells(10, (1.0, 1.2), 1.0, 0.8, "A"),
        m21 = rect_cells(21, (0.5, 0.4), 0.8, 0.5, "B"),
        m12 = rect_cells(12, (7.0, 9.0), 1.2, 0.8, "outside"),
        m13 = rect_cells(13, (1.0, 9.0), 1.2, 0.8, "solo"),
    );
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

    let path = dir.join("groups.vsdx");
    let file = std::fs::File::create(&path).unwrap();
    let mut w = zip::ZipWriter::new(file);
    let opts = zip::write::SimpleFileOptions::default();
    let parts: Vec<(String, String)> = vec![
        ("[Content_Types].xml".into(), content_types.into()),
        ("_rels/.rels".into(), root_rels.into()),
        ("visio/document.xml".into(), document.into()),
        ("visio/_rels/document.xml.rels".into(), doc_rels.into()),
        ("visio/pages/pages.xml".into(), pages.into()),
        ("visio/pages/_rels/pages.xml.rels".into(), page_rels.into()),
        ("visio/pages/page1.xml".into(), page1),
        ("visio/masters/masters.xml".into(), masters.into()),
        ("visio/masters/_rels/masters.xml.rels".into(), master_rels.into()),
        ("visio/masters/master1.xml".into(), master1.into()),
    ];
    for (name, data) in &parts {
        w.start_file(name.as_str(), opts).unwrap();
        std::io::Write::write_all(&mut w, data.as_bytes()).unwrap();
    }
    w.finish().unwrap();
    path
}

fn harness_with(dir: &std::path::Path, dark: bool) -> Harness<'static, App> {
    dc_ui::svgimg::pin_determinism_for_tests();
    Harness::builder()
        .with_size(egui::vec2(1280.0, 800.0))
        .build_ui_state(
            move |ui, app: &mut App| {
                dc_ui::theme::install_for(
                    ui.ctx(),
                    if dark {
                        egui::ThemePreference::Dark
                    } else {
                        egui::ThemePreference::Light
                    },
                );
                app.draw(ui);
            },
            App::with_doc(&group_fixture(dir)),
        )
}

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

/// Shift-click (multi-select, GitHub #26's idiom).
fn shift_click_at(h: &mut Harness<'_, App>, pos: egui::Pos2) {
    let shift = egui::Modifiers {
        shift: true,
        ..Default::default()
    };
    h.event(egui::Event::ModifiersChanged(shift));
    h.hover_at(pos);
    h.event(egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: shift,
    });
    h.event(egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: shift,
    });
    h.run();
    h.event(egui::Event::ModifiersChanged(egui::Modifiers::NONE));
}

/// Menu rows carry their label with leading padding (`ctx_item`
/// prefixes "  "), so queries match by containment. The Group ▸ flyout
/// row is the topmost chevron row (Group sits above Container ▸ in the
/// Visio menu order); the submenu opens below the outer menu, so a
/// submenu item is the matching node strictly under the flyout row.
/// Returns the context menu's bottom edge — the submenu opens below
/// it, which the item queries use to avoid identically-named texts
/// elsewhere (the breadcrumb's "›", ribbon buttons, the status line).
fn click_group_flyout(h: &mut Harness<'_, App>) -> egui::Rect {
    let menu = h
        .state()
        .debug_ctx_menu_rect()
        .expect("context menu open");
    let row = h
        .get_all_by_label_contains("›")
        .filter(|n| n.rect().height() < 30.0)
        .find(|n| menu.expand(2.0).contains_rect(n.rect()))
        .expect("Group ▸ flyout row inside the menu");
    row.click();
    h.run();
    menu
}

/// A node whose label contains `needle` and which renders below the
/// context menu — the submenu area.
fn flyout_item<'a>(
    h: &'a Harness<'a, App>,
    needle: &str,
    menu: egui::Rect,
) -> Option<egui_kittest::Node<'a>> {
    // The query ties the needle to the node lifetime; tests may leak
    // their few needles.
    let needle: &'a str = Box::leak(needle.to_string().into_boxed_str());
    let nodes: Vec<egui_kittest::Node<'a>> = h.get_all_by_label_contains(needle).collect();
    // The submenu opens beside the menu (vertically overlapping), so
    // an item is a matching node OUTSIDE the menu and below its top —
    // excluding the ribbon (above) and the in-menu rows themselves.
    nodes
        .into_iter()
        .find(|n| !menu.expand(2.0).contains_rect(n.rect()) && n.rect().top() > menu.top())
}

fn open_group_via_flyout(h: &mut Harness<'_, App>, open_label: &str) {
    let menu = click_group_flyout(h);
    flyout_item(h, open_label, menu)
        .unwrap_or_else(|| panic!("flyout item {open_label:?}"))
        .click();
    h.run();
}

/// A natural press-drag-release across frames (the #59 gesture shape;
/// see interactions.rs).
fn drag_naturally(h: &mut Harness<'_, App>, from: egui::Pos2, by: egui::Vec2) {
    let b = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    h.hover_at(from);
    h.event(b(from, true));
    h.run();
    for t in [0.25, 0.5, 0.75, 1.0] {
        h.hover_at(from + by * t);
        h.run();
    }
    h.event(b(from + by, false));
    h.run();
}

fn shape_center(h: &Harness<'_, App>, sid: i64) -> egui::Pos2 {
    h.state()
        .debug_shape_screen_rect_for_test(sid)
        .map(|r| r.center())
        .unwrap_or_else(|| panic!("screen rect for shape {sid}"))
}

#[test]
fn flyout_groups_and_ungroups() {
    let tmp = tempfile::tempdir().unwrap();
    let mut h = harness_with(tmp.path(), false);
    h.run();
    h.run();
    // Two loose rectangles → multi-select → Group through the flyout.
    let a = shape_center(&h, 12);
    let b = shape_center(&h, 13);
    click_at(&mut h, a, egui::PointerButton::Primary);
    shift_click_at(&mut h, b);
    assert_eq!(h.state().debug_sel_set_len(), 2);
    click_at(&mut h, a, egui::PointerButton::Secondary);
    h.run();
    let menu = click_group_flyout(&mut h);
    flyout_item(&h, "Group", menu).expect("Group item").click();
    h.run();
    // 12 and 13 became one group: 9 + the new group on the page.
    assert_eq!(h.state().debug_shape_count(), 2, "flyout Group grouped");
    // And back: Ungroup the new selection through the flyout.
    let sel = h.state().debug_sel().expect("new group selected").1;
    let c = shape_center(&h, sel);
    click_at(&mut h, c, egui::PointerButton::Secondary);
    h.run();
    let menu = click_group_flyout(&mut h);
    flyout_item(&h, "Ungroup", menu).expect("Ungroup item").click();
    h.run();
    assert_eq!(h.state().debug_shape_count(), 3, "flyout Ungroup restored");
    // Ungroup leaves the first member selected (a top-level shape).
    let member = h.state().debug_sel().expect("first member selected").1;

    // Undoing the ungroup re-nests the member: the selection must NOT
    // survive on the now-hidden nested shape (#136 review — handles
    // and Delete would act on it while the page shows only top-level
    // shapes).
    h.key_press_modifiers(egui::Modifiers::CTRL, egui::Key::Z);
    h.run();
    assert_eq!(h.state().debug_shape_count(), 2, "ungroup undone");
    assert!(
        h.state().debug_sel().is_none(),
        "the re-nested member {member} is not left selected"
    );
}

/// #95: resize handles work for members inside group-edit mode — the
/// member's own frame resizes (the group is untouched), and one undo
/// restores it. Handle positions and drag deltas route through the
/// renderer's XForm chain (`sel_page_rect` / the parent-local
/// conversion in `apply_resize`).
#[test]
fn member_resize_handles_work_in_group_edit_mode() {
    let tmp = tempfile::tempdir().unwrap();
    let mut h = harness_with(tmp.path(), false);
    h.run();
    h.run();
    // Open the group, select member 10.
    let g = shape_center(&h, 9);
    click_at(&mut h, g, egui::PointerButton::Secondary);
    h.run();
    open_group_via_flyout(&mut h, "Open Panel.9");
    let m = shape_center(&h, 10);
    click_at(&mut h, m, egui::PointerButton::Primary);
    assert_eq!(h.state().debug_sel().map(|s| s.1), Some(10));
    h.run();

    let (w0, h0) = (h.state().debug_sel_width(), h.state().debug_sel_height());
    assert!((w0 - 1.0).abs() < 1e-6, "member 10 starts 1 in wide");
    let group_rect_before = h.state().debug_shape_screen_rect(9).expect("group rect");

    // The corner handle exists in the mode (v1 suppressed all
    // handles) and dragging it grows ONLY the member.
    let br = h.state().debug_sel_br_corner().expect("member handles in the mode");
    drag_naturally(&mut h, br, egui::vec2(30.0, 20.0));
    let (w1, h1) = (h.state().debug_sel_width(), h.state().debug_sel_height());
    assert!(
        w1 > w0 * 1.2 && h1 > h0 * 1.2,
        "the member resized: {w0:.3}x{h0:.3} -> {w1:.3}x{h1:.3}"
    );
    let group_rect_after = h.state().debug_shape_screen_rect(9).expect("group rect");
    let grew = group_rect_after.width() > group_rect_before.width() + 1.0
        || group_rect_after.height() > group_rect_before.height() + 1.0;
    // The group's own Xform is untouched (its bbox may include the
    // member's growth through shape_ink_bounds-style unions? No —
    // the group's W/H are cells we never wrote).
    assert!(
        !grew,
        "the group's own Xform is untouched: {:?} -> {:?}",
        group_rect_before,
        group_rect_after
    );

    // One undo restores the member.
    h.key_press_modifiers(egui::Modifiers::CTRL, egui::Key::Z);
    h.run();
    let (w2, h2) = (h.state().debug_sel_width(), h.state().debug_sel_height());
    assert!(
        (w2 - w0).abs() < 1e-3 && (h2 - h0).abs() < 1e-3,
        "one undo restored the member: {w2:.3}x{h2:.3}"
    );
}

#[test]
fn open_group_edit_flow() {
    let tmp = tempfile::tempdir().unwrap();
    let mut h = harness_with(tmp.path(), false);
    h.run();
    h.run();
    // Right-click the group; the flyout's third row opens it.
    let g = shape_center(&h, 9);
    click_at(&mut h, g, egui::PointerButton::Secondary);
    h.run();
    open_group_via_flyout(&mut h, "Open Panel.9");
    assert_eq!(h.state().debug_group_edit(), vec![9], "group-edit entered");
    // The breadcrumb bar with its Close control is up.
    assert!(
        h.get_all_by_label_contains("Close Group").next().is_some(),
        "breadcrumb Close control"
    );
    assert!(
        h.get_all_by_label_contains("Page › Panel.9").next().is_some(),
        "breadcrumb path"
    );

    // Clicks resolve against members only.
    let m = shape_center(&h, 10);
    click_at(&mut h, m, egui::PointerButton::Primary);
    assert_eq!(h.state().debug_sel().map(|s| s.1), Some(10), "member selected");
    // The lone page shape is invisible to hits inside the mode.
    let out = shape_center(&h, 12);
    click_at(&mut h, out, egui::PointerButton::Primary);
    assert!(
        h.state().debug_sel().is_none(),
        "outside shape is not hittable in group-edit mode"
    );

    // Close Group returns to the page.
    h.get_all_by_label_contains("Close Group").next().unwrap().click();
    h.run();
    assert!(h.state().debug_group_edit().is_empty(), "closed via the button");
}

#[test]
fn member_edit_esc_and_one_undo() {
    let tmp = tempfile::tempdir().unwrap();
    let mut h = harness_with(tmp.path(), false);
    h.run();
    h.run();
    let g = shape_center(&h, 9);
    click_at(&mut h, g, egui::PointerButton::Secondary);
    h.run();
    open_group_via_flyout(&mut h, "Open Panel.9");

    // Drag member 10 a little (its Pin cells are group-local; the drag
    // converts through the group XForm).
    let m = shape_center(&h, 10);
    let pin_before = h.state().debug_cell_for_test(10, "PinX");
    h.hover_at(m);
    h.event(egui::Event::PointerButton {
        pos: m,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
    for k in 1..=4u32 {
        h.hover_at(m + egui::vec2(k as f32 * 10.0, -(k as f32 * 6.0)));
        h.run();
    }
    h.event(egui::Event::PointerButton {
        pos: m + egui::vec2(40.0, -24.0),
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    h.run();
    let pin_after = h.state().debug_cell_for_test(10, "PinX");
    assert_ne!(pin_before, pin_after, "member moved inside the group");

    // Esc leaves the mode; the edit is still there; ONE undo restores.
    h.event(egui::Event::Key {
        key: egui::Key::Escape,
        physical_key: Some(egui::Key::Escape),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    });
    h.run();
    assert!(h.state().debug_group_edit().is_empty(), "Esc closed the mode");
    assert_eq!(
        h.state().debug_cell_for_test(10, "PinX"),
        pin_after,
        "edit survives leaving the mode"
    );
    // Ctrl+Z as the platform integrations deliver it (command mirrors
    // ctrl on Linux/Windows).
    let undo = egui::Modifiers {
        ctrl: true,
        command: true,
        ..Default::default()
    };
    h.event(egui::Event::ModifiersChanged(undo));
    h.event(egui::Event::Key {
        key: egui::Key::Z,
        physical_key: Some(egui::Key::Z),
        pressed: true,
        repeat: false,
        modifiers: undo,
    });
    h.run();
    assert_eq!(
        h.state().debug_cell_for_test(10, "PinX"),
        pin_before,
        "one undo restored the member edit"
    );
}

#[test]
fn nested_groups_open_recursively() {
    let tmp = tempfile::tempdir().unwrap();
    let mut h = harness_with(tmp.path(), false);
    h.run();
    h.run();
    let g = shape_center(&h, 9);
    click_at(&mut h, g, egui::PointerButton::Secondary);
    h.run();
    open_group_via_flyout(&mut h, "Open Panel.9");
    assert_eq!(h.state().debug_group_edit(), vec![9]);

    // The nested group is a member here; open it in turn. A point on
    // 20 clear of its own child 21 (which renders on top of it): the
    // rect's top band, past the hit pad.
    let r20 = h
        .state()
        .debug_shape_screen_rect_for_test(20)
        .expect("inner group rect");
    let band = egui::pos2(r20.center().x, r20.top() + 3.0);
    click_at(&mut h, band, egui::PointerButton::Secondary);
    h.run();
    open_group_via_flyout(&mut h, "Open Inner.20");
    assert_eq!(h.state().debug_group_edit(), vec![9, 20], "breadcrumb grew");
    // Esc pops one level at a time.
    h.event(egui::Event::Key {
        key: egui::Key::Escape,
        physical_key: Some(egui::Key::Escape),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    });
    h.run();
    assert_eq!(h.state().debug_group_edit(), vec![9]);
    h.event(egui::Event::Key {
        key: egui::Key::Escape,
        physical_key: Some(egui::Key::Escape),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    });
    h.run();
    assert!(h.state().debug_group_edit().is_empty());
}

fn snap(h: &mut Harness<'_, App>, name: &str) {
    h.snapshot_options(
        name,
        &egui_kittest::SnapshotOptions::new()
            .threshold(3.0)
            .max_failed_pixels(10_000),
    );
}

/// Goldens (#78): the Group ▸ flyout open over the shape context menu,
/// one test per shell mode (a test owns exactly one snapshot-results
/// set).
#[test]
fn group_flyout_golden_light() {
    let tmp = tempfile::tempdir().unwrap();
    let mut h = harness_with(tmp.path(), false);
    flyout_open_snapshot(&mut h, "dc_ui_group_flyout_light");
}

#[test]
fn group_flyout_golden_dark() {
    let tmp = tempfile::tempdir().unwrap();
    let mut h = harness_with(tmp.path(), true);
    flyout_open_snapshot(&mut h, "dc_ui_group_flyout_dark");
}

fn flyout_open_snapshot(h: &mut Harness<'_, App>, name: &str) {
    h.run();
    h.run();
    let g = shape_center(h, 9);
    click_at(h, g, egui::PointerButton::Secondary);
    h.run();
    click_group_flyout(h);
    snap(h, name);
}

/// Golden (#78): the group-edit mode — isolated members, breadcrumb
/// bar with the Close control, member selection frame.
#[test]
fn group_edit_mode_golden() {
    let tmp = tempfile::tempdir().unwrap();
    let mut h = harness_with(tmp.path(), false);
    h.run();
    h.run();
    let g = shape_center(&h, 9);
    click_at(&mut h, g, egui::PointerButton::Secondary);
    h.run();
    open_group_via_flyout(&mut h, "Open Panel.9");
    // Select a member so the frame shows in the golden.
    let m = shape_center(&h, 10);
    click_at(&mut h, m, egui::PointerButton::Primary);
    h.run();
    snap(&mut h, "dc_ui_group_edit_mode");
}


