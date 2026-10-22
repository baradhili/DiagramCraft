//! kittest coverage for the Visio 2016 shape paradigm: drag a master from
//! the stencil onto the page, right-click a shape for its context menu,
//! act on it (delete / duplicate), and double-click into text editing
//! (EventDblClick, GitHub #20).

use dc_ui::app::App;
use egui::Pos2;
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;

fn harness_with_blank() -> Harness<'static, App> {
    // Goldens/steps must be machine-independent: pin text to the
    // embedded Carlito rather than this machine's installed fonts.
    dc_ui::svgimg::pin_embedded_fonts_for_tests();
    Harness::builder()
        .with_size(egui::vec2(1280.0, 800.0))
        .build_ui_state(
            |ui, app: &mut App| {
                dc_ui::theme::install(ui.ctx());
                app.draw(ui);
            },
            App::with_blank(),
        )
}

/// egui groups a click pair only when the two releases land inside its
/// double-click window (0.3 s of input time), and each kittest step
/// advances that clock by `step_dt` — the default 0.25 s pushes the
/// pair's releases 0.5 s apart, so these tests step at 0.05 s (and may
/// then need more steps to settle the same frames).
fn dblclick_harness_with_blank() -> Harness<'static, App> {
    dc_ui::svgimg::pin_embedded_fonts_for_tests();
    Harness::builder()
        .with_size(egui::vec2(1280.0, 800.0))
        .with_step_dt(0.05)
        .with_max_steps(30)
        .build_ui_state(
            |ui, app: &mut App| {
                dc_ui::theme::install(ui.ctx());
                app.draw(ui);
            },
            App::with_blank(),
        )
}

/// Click (press + release queued together, then settle — the kittest
/// convention; settling between press and release cancels the click).
fn click_at(h: &mut Harness<'_, App>, pos: Pos2, button: egui::PointerButton) {
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

/// Double-click: both press/release pairs queued back-to-back so the
/// releases stay inside egui's double-click window (see the harness
/// builder above for the step timing that makes this deterministic).
fn dbl_click_at(h: &mut Harness<'_, App>, pos: Pos2) {
    let b = |pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    h.hover_at(pos);
    h.event(b(true));
    h.event(b(false));
    h.event(b(true));
    h.event(b(false));
    h.run();
}

#[test]
fn stencil_drag_places_shape() {
    let mut h = harness_with_blank();
    h.run();
    let cells = h.state().debug_master_cells();
    assert!(!cells.is_empty(), "blank template exposes starter masters");
    let from = cells[0].1.center();
    let to = h.state().debug_canvas_rect().center();

    // Press on the stencil cell, then move across frames (so
    // drag_started fires on the cell), then release over the canvas.
    h.hover_at(from);
    h.event(egui::Event::PointerButton {
        pos: from,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
    h.hover_at(from + (to - from) * 0.25);
    h.run();
    for t in [0.5, 0.75, 1.0] {
        h.hover_at(from + (to - from) * t);
        h.run();
    }
    h.event(egui::Event::PointerButton {
        pos: to,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    h.run();

    assert_eq!(
        h.state().debug_shape_count(),
        1,
        "drag from stencil placed a shape"
    );
}

#[test]
fn right_click_menu_delete_removes_shape() {
    let mut h = harness_with_blank();
    h.run();
    let from = h.state().debug_master_cells()[0].1.center();
    let to = h.state().debug_canvas_rect().center();
    // click-to-arm then click-to-place (the other placement path)
    click_at(&mut h, from, egui::PointerButton::Primary);
    click_at(&mut h, to, egui::PointerButton::Primary);
    assert_eq!(h.state().debug_shape_count(), 1);

    // Right-click the placed shape.
    click_at(&mut h, to, egui::PointerButton::Secondary);
    h.run();
    assert!(
        h.state().debug_sel().is_some(),
        "right-click selects the shape"
    );

    // The Visio context menu shows Delete; the row is the short node
    // (the menu container's aggregated label also contains the word).
    let row = h
        .get_all_by_label_contains("Delete")
        .into_iter()
        .find(|n| n.rect().height() < 30.0)
        .expect("Delete menu row");
    row.click();
    h.run();
    assert_eq!(
        h.state().debug_shape_count(),
        0,
        "menu Delete removed the shape"
    );
}

#[test]
fn right_click_menu_duplicate_creates_shape() {
    let mut h = harness_with_blank();
    h.run();
    let from = h.state().debug_master_cells()[0].1.center();
    let to = h.state().debug_canvas_rect().center();
    click_at(&mut h, from, egui::PointerButton::Primary);
    click_at(&mut h, to, egui::PointerButton::Primary);
    assert_eq!(h.state().debug_shape_count(), 1);

    click_at(&mut h, to, egui::PointerButton::Secondary);
    h.run();

    h.get_by_label_contains("Duplicate").click();
    h.run();
    assert_eq!(
        h.state().debug_shape_count(),
        2,
        "menu Duplicate cloned the shape"
    );
    // And it is undoable like every command-layer action.
    h.key_press_modifiers(egui::Modifiers::CTRL, egui::Key::Z);
    h.run();
    assert_eq!(h.state().debug_shape_count(), 1, "duplicate undoes");
}

// Visio anchors the context menu just outside the shape's top-right
// corner (menu growing right/down); near the right screen edge it
// mirrors to just outside the top-left corner (GitHub #49).
#[test]
fn right_click_menu_opens_beside_shape() {
    let mut h = harness_with_blank();
    h.run();
    let from = h.state().debug_master_cells()[0].1.center();
    // High on the canvas so the tall menu fits below the shape (egui
    // clamps popups into the viewport; tops stay level when it fits).
    let canvas = h.state().debug_canvas_rect();
    let to = egui::pos2(canvas.center().x, canvas.top() + 80.0);
    click_at(&mut h, from, egui::PointerButton::Primary);
    click_at(&mut h, to, egui::PointerButton::Primary);
    click_at(&mut h, to, egui::PointerButton::Secondary);
    h.run();

    let (_, shape) = h.state().debug_sel().expect("right-click selects");
    let s = h
        .state()
        .debug_shape_screen_rect(shape)
        .expect("shape rect");
    let m = h.state().debug_ctx_menu_rect().expect("menu shown");
    // Menu's top-left sits ~2 px right of the shape's top-right corner,
    // tops level: the menu never covers the shape.
    assert!(
        (m.left() - (s.right() + 2.0)).abs() <= 0.5,
        "menu left {} should sit 2px right of shape right {}",
        m.left(),
        s.right()
    );
    assert!(
        (m.top() - s.top()).abs() <= 0.5,
        "menu top {} should align with shape top {}",
        m.top(),
        s.top()
    );
}

#[test]
fn right_click_menu_flips_near_right_edge() {
    let mut h = harness_with_blank();
    h.run();
    let from = h.state().debug_master_cells()[0].1.center();
    let canvas = h.state().debug_canvas_rect();
    // The flip decision compares against the window's right edge; the
    // canvas must reach it for this placement to force the mirror. High
    // on the canvas so the menu fits below (see test above).
    assert!(canvas.right() >= 1270.0, "canvas reaches window edge");
    let to = egui::pos2(canvas.right() - 6.0, canvas.top() + 80.0);
    click_at(&mut h, from, egui::PointerButton::Primary);
    click_at(&mut h, to, egui::PointerButton::Primary);
    click_at(&mut h, to, egui::PointerButton::Secondary);
    h.run();

    let (_, shape) = h.state().debug_sel().expect("right-click selects");
    let s = h
        .state()
        .debug_shape_screen_rect(shape)
        .expect("shape rect");
    let m = h.state().debug_ctx_menu_rect().expect("menu shown");
    // Mirrored: the menu's top-right sits ~2 px LEFT of the shape's
    // top-left corner, tops still level.
    assert!(
        (m.right() - (s.left() - 2.0)).abs() <= 0.5,
        "menu right {} should sit 2px left of shape left {}",
        m.right(),
        s.left()
    );
    assert!(
        (m.top() - s.top()).abs() <= 0.5,
        "menu top {} should align with shape top {}",
        m.top(),
        s.top()
    );
}

#[test]
fn marquee_selects_multiple_and_delete_removes_all() {
    let mut h = harness_with_blank();
    h.run();
    // Two shapes at DIFFERENT heights, so a mirrored (y-flipped)
    // selection band would pick the wrong one.
    let cells = h.state().debug_master_cells();
    let from = cells[0].1.center();
    let canvas = h.state().debug_canvas_rect();
    for off in [-40.0f32, 40.0f32] {
        let to = canvas.center() + egui::vec2(off, -off);
        click_at(&mut h, from, egui::PointerButton::Primary);
        click_at(&mut h, to, egui::PointerButton::Primary);
    }
    h.run();
    assert_eq!(h.state().debug_shape_count(), 2);

    // Upper shape only: a band around the upper-left placement picks
    // exactly one — this fails if the band is mirrored vertically.
    let c = canvas.center();
    let a = c + egui::vec2(10.0, -80.0);
    let b = c + egui::vec2(110.0, -20.0);
    h.hover_at(a);
    h.event(egui::Event::PointerButton {
        pos: a,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
    h.hover_at(a + (b - a) * 0.5);
    h.run();
    h.hover_at(b);
    h.event(egui::Event::PointerButton {
        pos: b,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    h.run();
    assert_eq!(
        h.state().debug_sel_set().len(),
        1,
        "upper band selects exactly the upper shape (mirroring would pick the lower)"
    );

    // Full marquee over both.
    let a = egui::pos2(c.x - 120.0, c.y - 90.0);
    let b = egui::pos2(c.x + 120.0, c.y + 90.0);
    h.hover_at(a);
    h.event(egui::Event::PointerButton {
        pos: a,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
    h.hover_at(a + (b - a) * 0.5);
    h.run();
    h.hover_at(b);
    h.event(egui::Event::PointerButton {
        pos: b,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    h.run();
    assert!(
        h.state().debug_sel_set().len() >= 2,
        "marquee selected multiple shapes, got {:?}",
        h.state().debug_sel_set()
    );

    // Clicking a member keeps the multi-selection alive (Visio: dragging
    // a selected shape moves the whole selection).
    let c = canvas.center();
    click_at(
        &mut h,
        c + egui::vec2(40.0, -40.0),
        egui::PointerButton::Primary,
    );
    assert_eq!(
        h.state().debug_sel_set().len(),
        2,
        "clicking a member preserves the multi-selection"
    );

    // One Delete removes the whole selection (single undo step).
    h.key_press(egui::Key::Delete);
    h.run();
    assert_eq!(h.state().debug_shape_count(), 0);
}

#[test]
fn corner_drag_resizes_shape() {
    let mut h = harness_with_blank();
    h.run();
    let from = h.state().debug_master_cells()[0].1.center();
    let to = h.state().debug_canvas_rect().center();
    click_at(&mut h, from, egui::PointerButton::Primary);
    click_at(&mut h, to, egui::PointerButton::Primary);
    h.run();
    assert_eq!(h.state().debug_shape_count(), 1);

    // Selection rect in screen coords comes from the app; approximate:
    // the shape is ~1in at ~fit zoom centered where we dropped. Grab the
    // bottom-right corner via the app's debug accessor.
    let br = h.state().debug_sel_br_corner().expect("selection rect");
    let tl_before = h.state().debug_sel_tl_corner().expect("selection rect");
    let before = h.state().debug_sel_width();
    let h_before = h.state().debug_sel_height();
    h.hover_at(br);
    h.event(egui::Event::PointerButton {
        pos: br,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
    h.hover_at(br + egui::vec2(40.0, 30.0));
    h.run();
    h.event(egui::Event::PointerButton {
        pos: br + egui::vec2(40.0, 30.0),
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    h.run();
    let after = h.state().debug_sel_width();
    let h_after = h.state().debug_sel_height();
    assert!(
        after > before + 0.15,
        "corner drag grew the shape: {before} → {after}"
    );
    assert!(
        h_after > h_before + 0.1,
        "corner drag grew the shape height: {h_before} → {h_after}"
    );
    // Dragging the bottom-right screen corner must anchor the opposite
    // (top-left) corner: it stays put within a pixel.
    let tl_after = h.state().debug_sel_tl_corner().expect("selection rect");
    assert!(
        (tl_after.x - tl_before.x).abs() <= 1.0 && (tl_after.y - tl_before.y).abs() <= 1.0,
        "top-left corner moved during bottom-right drag: {tl_before:?} → {tl_after:?}"
    );
}

/// Fit-mode zoom (GitHub #18): the document opens fit-to-window and the
/// fit follows window resizes; once the user zooms manually, resizes
/// must not clobber the chosen view. Fit Page re-arms it.
#[test]
fn zoom_follows_window_resize_in_fit_mode() {
    let mut h = harness_with_blank();
    h.run();
    h.run(); // fit settles
    let z_fit = h.state().debug_zoom();

    // Make the window taller: still in fit mode → the page re-fits
    // (bigger zoom — the fit is height-limited in this window shape).
    // This is the opening bug — nothing watched the content rect, so
    // the view stayed frozen at the old size.
    h.set_size(egui::vec2(1280.0, 1200.0));
    h.run();
    let z_tall = h.state().debug_zoom();
    assert!(
        z_tall > z_fit * 1.05,
        "resize while in fit mode must re-fit: {z_fit} → {z_tall}"
    );

    // Manual zoom (ribbon View → Zoom In) leaves fit mode.
    h.get_by_label_contains("View").click();
    h.run();
    h.get_by_label_contains("Zoom In").click();
    h.run();
    let z_manual = h.state().debug_zoom();
    assert!(
        (z_manual - (z_tall * 1.25)).abs() < 0.01,
        "manual zoom: {z_tall} × 1.25 expected, got {z_manual}"
    );

    // Resize again: the manual view survives untouched.
    h.set_size(egui::vec2(1280.0, 700.0));
    h.run();
    assert_eq!(
        h.state().debug_zoom(),
        z_manual,
        "a resize must not clobber the user's manual zoom"
    );

    // Fit Window re-arms fit mode: the view fits the (now shorter)
    // window again.
    h.get_by_label_contains("Fit Window").click();
    h.run();
    let z_refit = h.state().debug_zoom();
    assert!(
        (z_refit - z_manual).abs() / z_manual > 0.05,
        "Fit Page should re-fit away from the manual zoom: {z_manual} → {z_refit}"
    );
    // And following resumes.
    h.set_size(egui::vec2(1280.0, 1200.0));
    h.run();
    assert!(
        h.state().debug_zoom() > z_refit * 1.05,
        "fit mode follows resizes again after Fit Page"
    );
}

/// GitHub #20: double-clicking a shape opens its linked text for
/// editing (the F2 window reached by mouse, honoring EventDblClick — a
/// placed shape has no formula, so Visio's default: open the editor).
#[test]
fn double_click_opens_text_editor_and_edits_shape_text() {
    let mut h = dblclick_harness_with_blank();
    h.run();
    let from = h.state().debug_master_cells()[0].1.center();
    let to = h.state().debug_canvas_rect().center();
    click_at(&mut h, from, egui::PointerButton::Primary);
    click_at(&mut h, to, egui::PointerButton::Primary);
    h.run();
    assert_eq!(h.state().debug_shape_count(), 1);
    let (_, sid) = h.state().debug_sel().expect("placed shape selected");
    assert_eq!(
        h.state().debug_shape_text(sid),
        None,
        "a fresh master instance carries no text"
    );

    dbl_click_at(&mut h, to);
    assert!(
        h.query_all_by_label_contains("Edit Text").count() > 0,
        "double-click opened the Edit Text window"
    );

    // Set the field's value the accesskit way (the stencil pane's
    // search box is another TextInput, so scope the query to the
    // window), then OK: the text lands through SetText (one undo step).
    h.get_by_label_contains("Edit Text")
        .get_by_role(egui::Role::MultilineTextInput)
        .set_text_value_accesskit("Server");
    h.run();
    assert_eq!(
        h.get_by_label_contains("Edit Text")
            .get_by_role(egui::Role::MultilineTextInput)
            .value()
            .as_deref(),
        Some("Server"),
        "typed text visible in the field"
    );
    h.get_by_label_contains("OK").click();
    h.run();
    assert_eq!(
        h.state().debug_shape_text(sid).as_deref(),
        Some("Server"),
        "OK committed the typed text"
    );

    // Ctrl+Z restores the shape's original (empty) text.
    h.key_press_modifiers(egui::Modifiers::CTRL, egui::Key::Z);
    h.run();
    assert_eq!(
        h.state().debug_shape_text(sid),
        None,
        "undo restores the pre-edit text"
    );
}

/// GitHub #20: a double-click on empty page area opens nothing.
#[test]
fn double_click_on_empty_page_is_a_no_op() {
    let mut h = dblclick_harness_with_blank();
    h.run();
    let from = h.state().debug_master_cells()[0].1.center();
    let to = h.state().debug_canvas_rect().center();
    click_at(&mut h, from, egui::PointerButton::Primary);
    click_at(&mut h, to, egui::PointerButton::Primary);
    h.run();
    assert_eq!(h.state().debug_shape_count(), 1);

    // Well off the placed shape (~3 in at fit zoom), still on the page.
    dbl_click_at(&mut h, to + egui::vec2(200.0, 200.0));
    assert!(
        h.query_all_by_label_contains("Edit Text").count() == 0,
        "double-click on empty page must not open an editor"
    );
    assert_eq!(h.state().debug_shape_count(), 1);
}
