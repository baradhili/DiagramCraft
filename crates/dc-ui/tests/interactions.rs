//! kittest coverage for the Visio 2016 shape paradigm: drag a master from
//! the stencil onto the page, right-click a shape for its context menu,
//! and act on it (delete / duplicate).

use dc_ui::app::App;
use egui::Pos2;
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;

fn harness_with_blank() -> Harness<'static, App> {
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

    assert_eq!(h.state().debug_shape_count(), 1, "drag from stencil placed a shape");
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
    assert!(h.state().debug_sel().is_some(), "right-click selects the shape");

    // The Visio context menu shows Delete; the row is the short node
    // (the menu container's aggregated label also contains the word).
    let row = h
        .get_all_by_label_contains("Delete")
        .into_iter()
        .find(|n| n.rect().height() < 30.0)
        .expect("Delete menu row");
    row.click();
    h.run();
    assert_eq!(h.state().debug_shape_count(), 0, "menu Delete removed the shape");
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
    assert_eq!(h.state().debug_shape_count(), 2, "menu Duplicate cloned the shape");
    // And it is undoable like every command-layer action.
    h.key_press_modifiers(egui::Modifiers::CTRL, egui::Key::Z);
    h.run();
    assert_eq!(h.state().debug_shape_count(), 1, "duplicate undoes");
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
    click_at(&mut h, c + egui::vec2(40.0, -40.0), egui::PointerButton::Primary);
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
