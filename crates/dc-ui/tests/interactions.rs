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
    dc_ui::svgimg::pin_determinism_for_tests();
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
    dc_ui::svgimg::pin_determinism_for_tests();
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


/// GitHub #77: the icon action bar heads the shape context menu —
/// icons dispatch the same commands as the text rows, and enablement
/// mirrors them. (The bar's accessible labels are exact: the text
/// rows carry leading-space layout text, so `get_by_label` picks the
/// icon and `…_contains` sees both.)
#[test]
fn ctx_menu_icon_bar_dispatches_and_mirrors_enablement() {
    use egui_kittest::kittest::Queryable;
    let mut h = harness_with_blank();
    h.run();
    let (_page, a) = place_master(&mut h, 2);

    // Open the menu: the bar renders above the rows. Two runs settle
    // the popup's screen-edge re-anchoring before rects are read.
    let to = h.state().debug_shape_screen_rect(a).unwrap().center();
    click_at(&mut h, to, egui::PointerButton::Secondary);
    h.run();
    h.run();
    assert!(
        h.get_all_by_label_contains("Edit Text…").count() >= 2,
        "the bar icon and the text row are both present"
    );
    let bar_icon = h.get_by_label("Edit Text…").rect();

    // The Group icon mirrors the row's enablement: one selected shape
    // cannot group — the click does nothing and the menu stays open.
    // (Icon vs row vs the ribbon's own Group button: the icon is the
    // narrow node INSIDE the menu rect.)
    let menu = h.state().debug_ctx_menu_rect().expect("menu open");
    let group_icon = h
        .get_all_by_label("Group")
        .into_iter()
        .find(|n| n.rect().width() < 40.0 && menu.expand(2.0).contains_rect(n.rect()))
        .expect("the Group bar icon");
    group_icon.click();
    h.run();
    assert!(h.state().debug_ctx_menu_rect().is_some(), "no dispatch, menu open");
    assert_eq!(h.state().debug_shape_count(), 1, "nothing grouped");

    // The Edit-Text icon opens the in-place editor (#20 path).
    click_at(&mut h, bar_icon.center(), egui::PointerButton::Primary);
    h.run();
    assert!(
        h.state().debug_text_edit().is_some(),
        "the bar icon opened the text editor"
    );
    h.key_press(egui::Key::Escape);
    h.run();

    // A multi-selection enables the Group icon: it groups through the
    // same Act::Group dispatch as the text row. (The two placements
    // overlap at the canvas center, so a marquee is the reliable
    // multi-select — a shift-click at a's center would hit the
    // topmost shape and toggle it off.)
    let (_p2, b) = place_master(&mut h, 2);
    let _ = b;
    let c = h.state().debug_canvas_rect().center();
    let s0 = c + egui::vec2(-120.0, -120.0);
    let s1 = c + egui::vec2(120.0, 120.0);
    h.hover_at(s0);
    h.event(egui::Event::PointerButton {
        pos: s0,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
    h.hover_at(s0 + (s1 - s0) * 0.5);
    h.run();
    h.hover_at(s1);
    h.event(egui::Event::PointerButton {
        pos: s1,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    h.run();
    assert_eq!(h.state().debug_sel_set_len(), 2, "both shapes selected");
    let ca = h.state().debug_shape_screen_rect(a).unwrap().center();
    click_at(&mut h, ca, egui::PointerButton::Secondary);
    h.run();
    h.run();
    let menu = h.state().debug_ctx_menu_rect().expect("menu open");
    let group_icon = h
        .get_all_by_label("Group")
        .into_iter()
        .find(|n| n.rect().width() < 40.0 && menu.expand(2.0).contains_rect(n.rect()))
        .expect("the Group bar icon");
    group_icon.click();
    h.run();
    assert_eq!(
        h.state().debug_shape_count(),
        1,
        "the enabled Group icon grouped the selection"
    );

    // The Rotate icon matches its label's direction: the text row
    // "Rotate Right 90°" is Rot(−π/2) — page coordinates are y-up, so
    // clockwise is NEGATIVE radians (branch review: the bar had it
    // backwards).
    let (_, gid) = h.state().debug_sel().expect("group selected");
    let cg = h.state().debug_shape_screen_rect(gid).unwrap().center();
    click_at(&mut h, cg, egui::PointerButton::Secondary);
    h.run();
    h.run();
    h.get_by_label("Rotate Right 90°").click();
    h.run();
    let angle = h
        .state()
        .debug_cell_for_test(gid, "Angle")
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(999.0);
    assert!(
        (angle + std::f64::consts::FRAC_PI_2).abs() < 1e-6,
        "Rotate Right is clockwise: −π/2, got {angle}"
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
/// The blank harness at a taller viewport (#25): the context menu grew
/// by its master-Actions rows, and the beside-the-shape anchoring
/// tests need room for the full menu below the anchor.
fn tall_harness_with_blank() -> Harness<'static, App> {
    dc_ui::svgimg::pin_determinism_for_tests();
    Harness::builder()
        .with_size(egui::vec2(1280.0, 1100.0))
        .build_ui_state(
            |ui, app: &mut App| {
                dc_ui::theme::install(ui.ctx());
                app.draw(ui);
            },
            App::with_blank(),
        )
}

#[test]
fn right_click_menu_opens_beside_shape() {
    let mut h = tall_harness_with_blank();
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
    let mut h = tall_harness_with_blank();
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

/// A Ctrl/Cmd press starts a pan (canvas.rs pan_drag): it must not arm
/// a marquee or a move-drag, or panning clobbers the selection with an
/// empty marquee band and drags the shape under the press along.
#[test]
fn ctrl_drag_pans_without_marquee_or_move() {
    let mut h = harness_with_blank();
    h.run();
    let from = h.state().debug_master_cells()[0].1.center();
    let canvas = h.state().debug_canvas_rect();
    let to = canvas.center();
    click_at(&mut h, from, egui::PointerButton::Primary);
    click_at(&mut h, to, egui::PointerButton::Primary);
    let (_, sid) = h.state().debug_sel().expect("placed shape selected");
    let pin_before = h.state().debug_cell_for_test(sid, "PinX");

    // Ctrl as the platform integrations deliver it: on Linux/Windows
    // `command` mirrors `ctrl` (egui's Modifiers::CTRL leaves the
    // command field false), and the canvas keys the pan off `command`.
    let ctrl = egui::Modifiers {
        ctrl: true,
        command: true,
        ..Default::default()
    };
    let ctrl_drag = |h: &mut Harness<'_, App>, a: egui::Pos2, b: egui::Pos2| {
        // Hold Ctrl across the gesture: ModifiersChanged persists in
        // the input state until cleared.
        h.event(egui::Event::ModifiersChanged(ctrl));
        h.hover_at(a);
        h.event(egui::Event::PointerButton {
            pos: a,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: ctrl,
        });
        h.hover_at(a + (b - a) * 0.5);
        h.run();
        h.hover_at(b);
        h.event(egui::Event::PointerButton {
            pos: b,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: ctrl,
        });
        h.run();
        h.event(egui::Event::ModifiersChanged(egui::Modifiers::NONE));
    };

    // Ctrl+drag over empty page: a pan — no marquee commits, so the
    // selection survives.
    let a = egui::pos2(canvas.left() + 15.0, canvas.bottom() - 15.0);
    ctrl_drag(&mut h, a, canvas.center() + egui::vec2(60.0, -30.0));
    assert!(
        h.state().debug_sel().is_some(),
        "pan did not commit a selection-clobbering marquee"
    );

    // Ctrl+drag ON the shape: still a pan — no move-drag armed. The
    // first drag panned the view, so re-locate the shape on screen;
    // a short drag keeps it on the canvas.
    let on_shape = h
        .state()
        .debug_shape_screen_rect_for_test(sid)
        .expect("shape screen rect after pan")
        .center();
    ctrl_drag(&mut h, on_shape, on_shape + egui::vec2(20.0, -10.0));
    assert_eq!(
        h.state().debug_cell_for_test(sid, "PinX"),
        pin_before,
        "pan did not move the shape under the press"
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

/// Redo wiring (#97 item 7): undo was exercised ~20x across the
/// suites, redo never. Keyboard (Ctrl+Y) and the QAT Redo button must
/// both restore a move.
#[test]
fn redo_restores_an_undone_move_via_keyboard_and_the_qat_button() {
    let mut h = harness_with_blank();
    h.run();
    place_master(&mut h, 2);
    let (page, shape) = h.state().debug_sel().expect("placed shape selected");
    let x0 = cell_of(&h, page, shape, "PinX");

    // Move it right.
    let c = h.state().debug_shape_screen_rect(shape).expect("shape rect");
    drag_naturally(&mut h, c.center(), egui::vec2(60.0, 0.0));
    let x1 = cell_of(&h, page, shape, "PinX");
    assert!(x1 > x0 + 0.1, "drag moved the shape: {x0} -> {x1}");

    // Undo, then redo via Ctrl+Y: the move returns.
    h.key_press_modifiers(egui::Modifiers::CTRL, egui::Key::Z);
    h.run();
    assert!(
        (cell_of(&h, page, shape, "PinX") - x0).abs() < 1e-6,
        "undo restored the original pin"
    );
    h.key_press_modifiers(egui::Modifiers::CTRL, egui::Key::Y);
    h.run();
    assert!(
        (cell_of(&h, page, shape, "PinX") - x1).abs() < 1e-6,
        "Ctrl+Y redid the move"
    );

    // And via the QAT Redo button after another undo.
    h.key_press_modifiers(egui::Modifiers::CTRL, egui::Key::Z);
    h.run();
    h.get_by_label_contains("Redo").click();
    h.run();
    assert!(
        (cell_of(&h, page, shape, "PinX") - x1).abs() < 1e-6,
        "QAT Redo redid the move"
    );
}

/// GitHub #136: undo/redo restores the shapes the command touched
/// instead of clearing the selection. Move undo/redo keeps the shape
/// selected (handles stay visible); undoing a placement clears (its id
/// no longer resolves); undoing a delete restores *and selects* the
/// shape (observed Visio 2016 behavior).
#[test]
fn undo_redo_restores_the_touched_selection() {
    let mut h = harness_with_blank();
    h.run();
    let (page, shape) = place_master(&mut h, 2);
    let x0 = cell_of(&h, page, shape, "PinX");

    // Move it right; the drag keeps it selected.
    let c = h.state().debug_shape_screen_rect(shape).expect("shape rect");
    drag_naturally(&mut h, c.center(), egui::vec2(60.0, 0.0));
    assert_eq!(
        h.state().debug_sel(),
        Some((page, shape)),
        "the moved shape is still selected"
    );

    // Ctrl+Z: the move reverts AND the shape stays selected, handles
    // live.
    h.key_press_modifiers(egui::Modifiers::CTRL, egui::Key::Z);
    h.run();
    assert!(
        (cell_of(&h, page, shape, "PinX") - x0).abs() < 1e-6,
        "undo restored the original pin"
    );
    assert_eq!(
        h.state().debug_sel(),
        Some((page, shape)),
        "#136: undo keeps the moved shape selected"
    );
    assert!(
        h.state().debug_sel_br_corner().is_some(),
        "the selection still resolves (handles visible) after undo"
    );

    // Ctrl+Shift+Z: the move returns and the shape stays selected.
    h.key_press_modifiers(egui::Modifiers::SHIFT | egui::Modifiers::CTRL, egui::Key::Z);
    h.run();
    assert_eq!(
        h.state().debug_sel(),
        Some((page, shape)),
        "#136: redo keeps the moved shape selected"
    );

    // Undo the move, then the placement: the shape leaves the page and
    // the selection clears naturally.
    h.key_press_modifiers(egui::Modifiers::CTRL, egui::Key::Z);
    h.run();
    h.key_press_modifiers(egui::Modifiers::CTRL, egui::Key::Z);
    h.run();
    assert_eq!(h.state().debug_shape_count(), 0, "the placement is undone");
    assert_eq!(
        h.state().debug_sel(),
        None,
        "#136: undoing a placement clears the selection"
    );

    // Redo the placement: the shape comes back selected (a placed
    // shape lands selected in Visio).
    h.key_press_modifiers(egui::Modifiers::SHIFT | egui::Modifiers::CTRL, egui::Key::Z);
    h.run();
    assert_eq!(h.state().debug_shape_count(), 1, "the placement is redone");
    assert_eq!(
        h.state().debug_sel(),
        Some((page, shape)),
        "#136: redoing a placement selects the restored shape"
    );

    // Delete, then undo the delete: the shape returns SELECTED.
    h.key_press(egui::Key::Delete);
    h.run();
    assert_eq!(h.state().debug_shape_count(), 0, "deleted");
    assert_eq!(h.state().debug_sel(), None, "delete cleared the selection");
    h.key_press_modifiers(egui::Modifiers::CTRL, egui::Key::Z);
    h.run();
    assert_eq!(h.state().debug_shape_count(), 1, "the delete is undone");
    assert_eq!(
        h.state().debug_sel(),
        Some((page, shape)),
        "#136: undoing a delete restores AND selects the shape"
    );
    assert_eq!(
        h.state().debug_sel_set(),
        vec![(page, shape)],
        "the restored selection set is exactly the touched shape"
    );
    assert!(
        h.state().debug_sel_br_corner().is_some(),
        "handles are live on the restored shape"
    );
}

// ------------------------------------------------------------------
// GitHub #59: reliable handles, shapesheet guards, 1D line handles.

/// The id of the blank template's 1D Line master (stencil cells expose
/// (id, rect) pairs; see blank.vsdx).
const LINE_MASTER: i64 = 9;

/// One numeric cell of a shape, parsed (avoids closure-borrow fights
/// with the harness in the line tests).
fn cell_of(h: &Harness<'_, App>, page: i64, shape: i64, n: &str) -> f64 {
    h.state()
        .debug_cell(page, shape, n)
        .unwrap()
        .0
        .parse::<f64>()
        .unwrap()
}

/// A natural mouse gesture — press, several small movements across
/// frames, release — exactly the shape of drag that egui's
/// click-vs-drag disambiguation used to swallow (GitHub #59).
fn drag_naturally(h: &mut Harness<'_, App>, from: Pos2, by: egui::Vec2) {
    let b = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    h.hover_at(from);
    h.event(b(from, true));
    h.run(); // the press frame arms the drag
    for t in [0.25, 0.5, 0.75, 1.0] {
        h.hover_at(from + by * t);
        h.run();
    }
    h.event(b(from + by, false));
    h.run();
}

/// Place a master through the stencil's click-to-arm, click-to-place
/// path and return the (page, shape) of the placed instance.
fn place_master(h: &mut Harness<'_, App>, master: i64) -> (i64, i64) {
    // Quick Shapes (#88) added a first entry, so a target master's
    // cell may sit below the fold — wheel the pane until the cell is
    // in the viewport before clicking it.
    let mut wheels = 0;
    let from = loop {
        let cell = h
            .state()
            .debug_master_cells()
            .into_iter()
            .find(|(id, _)| *id == (dc_ui::stencil::StencilId::Document, master))
            .map(|(_, r)| r)
            .unwrap_or_else(|| panic!("master {master} in the stencil"));
        if cell.center().y < 760.0 {
            break cell.center();
        }
        wheels += 1;
        assert!(wheels < 40, "pane never scrolled master {master} into view");
        h.hover_at(egui::pos2(80.0, 400.0));
        h.event(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, -120.0),
            modifiers: egui::Modifiers::NONE,
            phase: egui::TouchPhase::Move,
            source: egui::MouseWheelSource::Wheel,
        });
        h.run_steps(3);
    };
    let to = h.state().debug_canvas_rect().center();
    click_at(h, from, egui::PointerButton::Primary);
    click_at(h, to, egui::PointerButton::Primary);
    h.run();
    h.state().debug_sel().expect("placed shape is selected")
}

/// The core #59 regression: a natural press-drag-release on a shape's
/// body MOVES it. The old plumbing armed the move on the click
/// (release) and dropped it the next frame, so real gestures no-opped.
#[test]
fn shape_drag_moves_shape_natural_gesture() {
    let mut h = harness_with_blank();
    h.run();
    let (page, shape) = place_master(&mut h, 2); // Rectangle
    assert_eq!(h.state().debug_shape_count(), 1);

    let (pin0x, pin0y) = {
        let c = h.state().debug_cell(page, shape, "PinX").unwrap();
        let d = h.state().debug_cell(page, shape, "PinY").unwrap();
        (c.0.parse::<f64>().unwrap(), d.0.parse::<f64>().unwrap())
    };
    // Press inside the shape (center), drag like a mouse: small moves
    // across frames, release.
    let center = h.state().debug_shape_screen_rect(shape).unwrap().center();
    drag_naturally(&mut h, center, egui::vec2(60.0, 40.0));

    let (pin1x, pin1y) = {
        let c = h.state().debug_cell(page, shape, "PinX").unwrap();
        let d = h.state().debug_cell(page, shape, "PinY").unwrap();
        (c.0.parse::<f64>().unwrap(), d.0.parse::<f64>().unwrap())
    };
    let zoom = h.state().debug_zoom();
    let ppi = 96.0 * zoom;
    assert!(
        (pin1x - pin0x - 60.0 / ppi as f64).abs() < 0.05,
        "pin moved right by the drag: {pin0x:.3} → {pin1x:.3}"
    );
    assert!(
        (pin1y - (pin0y - 40.0 / ppi as f64)).abs() < 0.05,
        "pin moved down (y-up decreases) by the drag: {pin0y:.3} → {pin1y:.3}"
    );

    // One undo step returns the shape.
    h.key_press_modifiers(egui::Modifiers::CTRL, egui::Key::Z);
    h.run();
    let back = h.state().debug_cell(page, shape, "PinX").unwrap();
    assert!(
        (back.0.parse::<f64>().unwrap() - pin0x).abs() < 1e-6,
        "undo restored the move"
    );
}

/// Every one of the four corners grabs and resizes outward, anchoring
/// the opposite corner (#59: handles were unreliable/no-op).
#[test]
fn all_four_corner_drags_resize() {
    for (i, delta) in [
        egui::vec2(-40.0, -30.0), // TL outward (up-left)
        egui::vec2(40.0, -30.0),  // TR outward (up-right)
        egui::vec2(-40.0, 30.0),  // BL outward (down-left)
        egui::vec2(40.0, 30.0),   // BR outward (down-right)
    ]
    .into_iter()
    .enumerate()
    {
        let mut h = harness_with_blank();
        h.run();
        place_master(&mut h, 2);
        let corners = h.state().debug_sel_corners().expect("selection");
        let (w0, h0) = (h.state().debug_sel_width(), h.state().debug_sel_height());
        // Corner order is [tl, tr, bl, br]: the opposite corner is the
        // mirror index (0↔3, 1↔2).
        let opposite = 3 - i;
        let opposite_before = corners[opposite];
        drag_naturally(&mut h, corners[i], delta);
        let (w1, h1) = (h.state().debug_sel_width(), h.state().debug_sel_height());
        assert!(
            w1 > w0 + 0.1 && h1 > h0 + 0.08,
            "corner {i} drag grew the shape: {w0:.3}×{h0:.3} → {w1:.3}×{h1:.3}"
        );
        let opposite_after = h.state().debug_sel_corners().expect("selection")[opposite];
        assert!(
            (opposite_after.x - opposite_before.x).abs() <= 1.0
                && (opposite_after.y - opposite_before.y).abs() <= 1.0,
            "corner {i}: opposite corner must anchor: {opposite_before:?} → {opposite_after:?}"
        );
    }
}

/// A `GUARD(...)`-driven Width is never silently overwritten: the drag
/// refuses that axis with a status note and the cell is untouched
/// (#59). The height axis (unguarded) still resizes.
#[test]
fn guarded_width_refuses_that_axis() {
    let mut h = harness_with_blank();
    h.run();
    let (page, shape) = place_master(&mut h, 2);
    h.state_mut()
        .set_cell_for_test(page, shape, "Width", "1.5", Some("GUARD(1.5 in)"));
    h.run();

    let br = h.state().debug_sel_br_corner().expect("selection");
    let (h0,) = (h.state().debug_sel_height(),);
    drag_naturally(&mut h, br, egui::vec2(50.0, 40.0));

    let width = h.state().debug_cell(page, shape, "Width").unwrap();
    assert_eq!(
        width,
        ("1.5".to_string(), "GUARD(1.5 in)".to_string()),
        "guarded Width cell untouched"
    );
    assert!(
        h.state().debug_sel_width() < 1.6,
        "visible width did not grow"
    );
    assert!(
        h.state().debug_sel_height() > h0 + 0.1,
        "free axis (height) still resized"
    );
    let status = h.state().debug_status();
    assert!(
        status.contains("width not resized") && status.contains("GUARD"),
        "status explains the refusal, got: {status}"
    );
}

/// LockWidth in the Protection section guards the axis exactly like a
/// formula (#59); locking BOTH axes refuses the whole resize.
#[test]
fn protection_locks_refuse_resize() {
    let mut h = harness_with_blank();
    h.run();
    let (page, shape) = place_master(&mut h, 2);
    h.state_mut()
        .set_protection_for_test(page, shape, "LockWidth", true);
    h.run();

    let br = h.state().debug_sel_br_corner().expect("selection");
    let (w0, h0) = (h.state().debug_sel_width(), h.state().debug_sel_height());
    drag_naturally(&mut h, br, egui::vec2(50.0, 40.0));
    assert!(
        (h.state().debug_sel_width() - w0).abs() < 1e-6,
        "LockWidth kept the width"
    );
    assert!(
        h.state().debug_sel_height() > h0 + 0.1,
        "height still resized"
    );
    assert!(
        h.state().debug_status().contains("LockWidth"),
        "status names the lock, got: {}",
        h.state().debug_status()
    );

    // Both axes locked → the whole resize is refused, nothing changes.
    h.state_mut()
        .set_protection_for_test(page, shape, "LockHeight", true);
    h.run();
    let br = h.state().debug_sel_br_corner().expect("selection");
    let (w1, h1) = (h.state().debug_sel_width(), h.state().debug_sel_height());
    drag_naturally(&mut h, br, egui::vec2(60.0, 50.0));
    assert!(
        (h.state().debug_sel_width() - w1).abs() < 1e-6
            && (h.state().debug_sel_height() - h1).abs() < 1e-6,
        "fully locked shape does not resize"
    );
    assert!(
        h.state().debug_status().contains("resize refused"),
        "status refuses, got: {}",
        h.state().debug_status()
    );
}

/// Body-dragging a 1D line (press away from the handles) must translate
/// Begin/End/Pin TOGETHER — a pin-only move leaves the endpoint cells
/// stale, so the handles sit at the old positions and the next endpoint
/// drag snaps the line back (CodeRabbit review of df4a562).
#[test]
fn line_body_drag_translates_endpoints() {
    let mut h = harness_with_blank();
    h.run();
    // The Line master sits outside Quick Shapes' head set — reveal the
    // document stencil entry (#102) to place it.
    h.state_mut().show_doc_stencil_for_test();
    // Pure translation semantics: turn the (default-on) snap engine
    // off — #109's snap tests cover the snapped landings separately.
    let mut snap_off = h.state().debug_snap_settings();
    snap_off.snap_active = false;
    snap_off.glue_active = false;
    h.state_mut().debug_set_snap_for_test(snap_off);
    h.run();
    let (page, shape) = place_master(&mut h, LINE_MASTER);

    // Stretch the line first so its body has room clear of every
    // handle's 12 px grab radius.
    let ex_pre = cell_of(&h, page, shape, "EndX");
    let end_handle = h
        .state()
        .debug_handle_screen(dc_ui::canvas::HandleId::LineEnd)
        .expect("line endpoint handle");
    drag_naturally(&mut h, end_handle, egui::vec2(180.0, 0.0));
    let (bx0, by0, ex0, ey0) = (
        cell_of(&h, page, shape, "BeginX"),
        cell_of(&h, page, shape, "BeginY"),
        cell_of(&h, page, shape, "EndX"),
        cell_of(&h, page, shape, "EndY"),
    );
    assert!(
        (ex0 - ex_pre - 180.0 / (96.0 * h.state().debug_zoom()) as f64).abs() < 0.05,
        "endpoint stretch landed: {ex_pre:.3} → {ex0:.3}"
    );

    // Press a quarter of the way along the body: on the line, well
    // outside any handle's grab radius.
    let beg = h
        .state()
        .debug_handle_screen(dc_ui::canvas::HandleId::LineBegin)
        .unwrap();
    let end = h
        .state()
        .debug_handle_screen(dc_ui::canvas::HandleId::LineEnd)
        .unwrap();
    let quarter = egui::pos2(
        beg.x + (end.x - beg.x) * 0.25,
        beg.y + (end.y - beg.y) * 0.25,
    );
    drag_naturally(&mut h, quarter, egui::vec2(50.0, 30.0));

    let zoom = h.state().debug_zoom();
    let ppi = 96.0 * zoom;
    let (bx1, by1, ex1, ey1) = (
        cell_of(&h, page, shape, "BeginX"),
        cell_of(&h, page, shape, "BeginY"),
        cell_of(&h, page, shape, "EndX"),
        cell_of(&h, page, shape, "EndY"),
    );
    assert!(
        (bx1 - bx0 - 50.0 / ppi as f64).abs() < 0.05,
        "begin translated with the body drag: {bx0:.3} → {bx1:.3}"
    );
    assert!(
        (ex1 - ex0 - 50.0 / ppi as f64).abs() < 0.05,
        "end translated with the body drag: {ex0:.3} → {ex1:.3}"
    );
    assert!(
        ((by1 - by0) + 30.0 / ppi as f64).abs() < 0.05
            && ((ey1 - ey0) + 30.0 / ppi as f64).abs() < 0.05,
        "both ends moved down (y-up decreases) by the drag"
    );
    assert!(
        ((ex1 - bx1) - (ex0 - bx0)).abs() < 1e-6,
        "body drag translated, did not stretch"
    );
}

/// Dropping the 1D Line master snapshots XForm1D cells on the instance
/// (a horizontal 1-inch line centered on the drop), and dragging one
/// endpoint moves only that end, re-deriving Width/Angle/Pin (#59).
#[test]
fn line_endpoint_drag_moves_only_that_end() {
    let mut h = harness_with_blank();
    h.run();
    // The Line master sits outside Quick Shapes' head set — reveal the
    // document stencil entry (#102) to place it.
    h.state_mut().show_doc_stencil_for_test();
    h.run();
    let (page, shape) = place_master(&mut h, LINE_MASTER);

    // Instance snapshot: horizontal 1-inch line centered on the drop.
    let beg = h.state().debug_cell(page, shape, "BeginX").unwrap();
    let end = h.state().debug_cell(page, shape, "EndX").unwrap();
    let (bx0, ex0) = (beg.0.parse::<f64>().unwrap(), end.0.parse::<f64>().unwrap());
    let ey0 = cell_of(&h, page, shape, "EndY");
    assert!(
        (ex0 - bx0 - 1.0).abs() < 1e-6,
        "drop snapshots a 1in line: {bx0:.3} → {ex0:.3}"
    );
    assert_eq!(beg.1, "", "BeginX is a literal, not a formula");

    // Selection handles switch to the 1D set: endpoints + midpoint.
    let end_handle = h
        .state()
        .debug_handle_screen(dc_ui::canvas::HandleId::LineEnd)
        .expect("line endpoint handle");

    drag_naturally(&mut h, end_handle, egui::vec2(50.0, -35.0));

    let (bx1, by1, ex1, ey1) = (
        cell_of(&h, page, shape, "BeginX"),
        cell_of(&h, page, shape, "BeginY"),
        cell_of(&h, page, shape, "EndX"),
        cell_of(&h, page, shape, "EndY"),
    );
    // Begin anchored.
    assert!(
        (bx1 - bx0).abs() < 1e-6,
        "begin x anchored: {bx0:.3} → {bx1:.3}"
    );
    // End moved by the drag (page inches; screen +50x is +x, screen
    // -35y is +y in page terms).
    let zoom = h.state().debug_zoom();
    let ppi = 96.0 * zoom;
    assert!(
        (ex1 - ex0 - 50.0 / ppi as f64).abs() < 0.05,
        "end x moved by the drag: {ex0:.3} → {ex1:.3}"
    );
    assert!(
        (ey1 - ey0 - 35.0 / ppi as f64).abs() < 0.05,
        "end y moved up by the drag: {ey0:.3} → {ey1:.3}"
    );
    // Derived cells follow: Width is the new length, not the stale 1.0.
    let w = cell_of(&h, page, shape, "Width");
    let expected = ((ex1 - bx1).hypot(ey1 - by1) * 1e6).round() / 1e6;
    assert!(
        (w - expected).abs() < 2e-3,
        "Width re-derived as the endpoint distance: {w} vs {expected}"
    );
}

/// The 1D midpoint handle translates the whole line — both endpoints
/// move by the same delta, their separation untouched (#59).
#[test]
fn line_midpoint_drag_translates_both_ends() {
    let mut h = harness_with_blank();
    h.run();
    // The Line master sits outside Quick Shapes' head set — reveal the
    // document stencil entry (#102) to place it.
    h.state_mut().show_doc_stencil_for_test();
    h.run();
    let (page, shape) = place_master(&mut h, LINE_MASTER);
    let (bx0, by0, ex0, ey0) = (
        cell_of(&h, page, shape, "BeginX"),
        cell_of(&h, page, shape, "BeginY"),
        cell_of(&h, page, shape, "EndX"),
        cell_of(&h, page, shape, "EndY"),
    );

    let mid = h
        .state()
        .debug_handle_screen(dc_ui::canvas::HandleId::LineMid)
        .expect("line midpoint handle");
    drag_naturally(&mut h, mid, egui::vec2(40.0, 25.0));

    let zoom = h.state().debug_zoom();
    let ppi = 96.0 * zoom;
    let (bx1, by1, ex1, ey1) = (
        cell_of(&h, page, shape, "BeginX"),
        cell_of(&h, page, shape, "BeginY"),
        cell_of(&h, page, shape, "EndX"),
        cell_of(&h, page, shape, "EndY"),
    );
    assert!(
        (bx1 - bx0 - 40.0 / ppi as f64).abs() < 0.05,
        "begin moved with the midpoint drag: {bx0:.3} → {bx1:.3}"
    );
    assert!(
        (ex1 - ex0 - 40.0 / ppi as f64).abs() < 0.05,
        "end moved with the midpoint drag: {ex0:.3} → {ex1:.3}"
    );
    // Separation preserved.
    assert!(
        ((ex1 - bx1) - (ex0 - bx0)).abs() < 1e-6 && ((ey1 - by1) - (ey0 - by0)).abs() < 1e-6,
        "midpoint drag translated, did not stretch"
    );

    // One undo returns the whole translation.
    h.key_press_modifiers(egui::Modifiers::CTRL, egui::Key::Z);
    h.run();
    assert!(
        (cell_of(&h, page, shape, "BeginX") - bx0).abs() < 1e-6
            && (cell_of(&h, page, shape, "EndX") - ex0).abs() < 1e-6,
        "undo restored both endpoints"
    );
}

/// Ribbon zoom wiring (GitHub #18): the View tab's Zoom In button
/// zooms ×1.25 and Fit Window returns to the fit zoom. The fit-state
/// machine itself (resize-follows until manual override) is unit-tested
/// in canvas.rs `refit_follows_content_changes_until_manual_intervention`.
#[test]
fn ribbon_zoom_buttons_zoom_and_refit() {
    let mut h = harness_with_blank();
    h.run();
    h.run(); // fit settles
    let z_fit = h.state().debug_zoom();

    h.get_by_label_contains("View").click();
    h.run();
    h.get_by_label_contains("Zoom In").click();
    h.run();
    let z_manual = h.state().debug_zoom();
    assert!(
        (z_manual - (z_fit * 1.25)).abs() < 0.01,
        "Zoom In: {z_fit} × 1.25 expected, got {z_manual}"
    );

    h.get_by_label_contains("Fit Window").click();
    h.run();
    let z_refit = h.state().debug_zoom();
    assert!(
        (z_refit - z_fit).abs() / z_fit < 0.05,
        "Fit Window returns to the fit zoom: {z_manual} → {z_refit} (fit was {z_fit})"
    );
}

/// GitHub #20/#22: double-clicking a shape opens its linked text for
/// IN-PLACE editing (honoring EventDblClick — a placed shape has no
/// formula, so Visio's default: open the editor). The overlay — not a
/// modal window — sits on the shape's text-block rect, the view zooms
/// into the block and back out on exit, and the committed text lands
/// through SetText (one undo step restores the pre-edit text).
#[test]
fn double_click_opens_inplace_text_editor_and_edits_shape_text() {
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
    let zoom_before = h.state().debug_view_zoom();

    dbl_click_at(&mut h, to);
    h.run();
    assert_eq!(
        h.state().debug_text_edit(),
        Some(h.state().debug_sel().unwrap()),
        "double-click opened the in-place editor on the shape"
    );
    // The overlay — not a modal window — is up, at the shape's block.
    let editor = h.get_by_role(egui::Role::MultilineTextInput);
    let block = h
        .state()
        .debug_text_edit_rect()
        .expect("text block resolves on screen");
    let er = editor.rect();
    assert!(
        block.expand(6.0).contains_rect(er),
        "editor {er:?} sits on the text block {block:?}"
    );
    // The edited text stays where it renders (owner report on #22):
    // content vertically centered in the block per the default
    // VertAlign — no jump to the block's top-left on entry.
    assert!(
        (er.center().y - block.center().y).abs() < 4.0,
        "editor content anchored mid-block: editor {er:?} vs block {block:?}"
    );
    assert!(
        h.state().debug_view_zoom() > zoom_before,
        "entry zoomed into the block: {} -> {}",
        zoom_before,
        h.state().debug_view_zoom()
    );

    // Type the accesskit way, then click EMPTY canvas: the click
    // outside ends the edit and commits (Visio semantics; one undo
    // step — the retired dialog's commit path).
    h.get_by_role(egui::Role::MultilineTextInput)
        .set_text_value_accesskit("Server");
    h.run();
    click_at(&mut h, to + egui::vec2(200.0, 200.0), egui::PointerButton::Primary);
    h.run();
    assert_eq!(
        h.state().debug_shape_text(sid).as_deref(),
        Some("Server"),
        "click-outside committed the typed text"
    );
    assert_eq!(h.state().debug_text_edit(), None, "editor closed");
    assert!(
        (h.state().debug_view_zoom() - zoom_before).abs() < 1e-4,
        "view restored to the pre-edit zoom: {} vs {}",
        h.state().debug_view_zoom(),
        zoom_before
    );

    // One Ctrl+Z restores the shape's original (empty) text.
    h.key_press_modifiers(egui::Modifiers::CTRL, egui::Key::Z);
    h.run();
    assert_eq!(
        h.state().debug_shape_text(sid),
        None,
        "undo restores the pre-edit text"
    );

    // Esc DISCARDS: the shape's text reverts to its pre-edit value
    // (owner review, 2026-10-27).
    dbl_click_at(&mut h, to);
    h.run();
    assert!(h.state().debug_text_edit().is_some());
    h.get_by_role(egui::Role::MultilineTextInput)
        .set_text_value_accesskit("Router");
    h.run();
    h.key_press(egui::Key::Escape);
    h.run();
    assert_eq!(h.state().debug_text_edit(), None, "Esc closed the editor");
    assert_eq!(
        h.state().debug_shape_text(sid),
        None,
        "Esc reverted the typed text (the pre-edit value was empty — the earlier undo)"
    );
    // And a following edit still commits normally.
    dbl_click_at(&mut h, to);
    h.run();
    h.get_by_role(egui::Role::MultilineTextInput)
        .set_text_value_accesskit("Router");
    h.run();
    click_at(&mut h, to + egui::vec2(200.0, 200.0), egui::PointerButton::Primary);
    h.run();
    assert_eq!(
        h.state().debug_shape_text(sid).as_deref(),
        Some("Router"),
        "editing still commits after an Esc revert"
    );

    // A double-click on empty page area (well off the shape) opens
    // nothing — folded in from the former no-op test; same harness.
    dbl_click_at(&mut h, to + egui::vec2(200.0, 200.0));
    assert_eq!(
        h.state().debug_text_edit(),
        None,
        "double-click on empty page must not open an editor"
    );
    assert_eq!(h.state().debug_shape_count(), 1);
}

/// #22 review: the text-suppression toggle bumps only the previously
/// and newly skipped shapes' content versions — another shape's layer
/// entry survives an edit session uncached-free (the toggle used to
/// clear the whole store). Probed by CONTENT VERSION, not texture id:
/// the editor's entry zoom changes the ppi bucket, which legitimately
/// re-rasters every on-screen entry (new texture ids) without
/// invalidating anything.
#[test]
fn text_edit_skip_toggle_keeps_other_layers_cached() {
    let mut h = harness_with_blank();
    h.run();
    let (_page, bystander) = place_master(&mut h, 2);
    // The second placement lands on the same spot and stays selected
    // (the topmost shape) — F2 edits the SELECTION, so the bystander
    // underneath is the cache probe.
    let (_, edited) = place_master(&mut h, 2);
    h.run_steps(3);
    assert!(
        h.state().debug_layer_tex_id_for_test(bystander).is_some()
            && h.state().debug_layer_tex_id_for_test(edited).is_some(),
        "both layers landed (sync raster)"
    );
    let (v_by0, v_ed0) = (
        h.state().debug_layer_version_for_test(bystander),
        h.state().debug_layer_version_for_test(edited),
    );

    // F2 opens the in-place editor on the selection: its entry is
    // invalidated (re-render textless); the bystander's version must
    // not move.
    h.key_press(egui::Key::F2);
    h.run_steps(3);
    assert!(h.state().debug_text_edit().is_some(), "editor open");
    assert_ne!(
        h.state().debug_layer_version_for_test(edited),
        v_ed0,
        "the edited shape's entry was invalidated"
    );
    assert_eq!(
        h.state().debug_layer_version_for_test(bystander),
        v_by0,
        "the unedited shape's content version survived the toggle"
    );

    // Esc ends the edit: the text returns (another bump); the
    // bystander untouched.
    h.key_press(egui::Key::Escape);
    h.run_steps(3);
    assert!(h.state().debug_text_edit().is_none(), "editor closed");
    assert_ne!(
        h.state().debug_layer_version_for_test(edited),
        v_ed0,
        "the edited shape re-rendered with its text back"
    );
    assert_eq!(
        h.state().debug_layer_version_for_test(bystander),
        v_by0,
        "the unedited shape's content version is still unchanged"
    );
}

/// #22 branch review: switching pages mid-edit ends the in-place
/// editor AND commits the draft (Visio commits on the focus change).
#[test]
fn page_switch_commits_the_text_edit() {
    use egui_kittest::kittest::Queryable;
    let mut h = harness_with_blank();
    h.run();
    // Two pages (insert activates Page-2).
    let tab = h.get_by_label("Page-1").rect().center();
    click_at(&mut h, tab, egui::PointerButton::Secondary);
    h.run();
    h.get_all_by_label_contains("Insert Page")
        .into_iter()
        .find(|n| n.rect().height() < 30.0)
        .expect("Insert Page… row")
        .click();
    h.run();
    assert_eq!(h.state().debug_active_page_name(), "Page-2");

    // Back to Page-1: place a shape and open its editor with a draft.
    h.get_by_label("Page-1").click();
    h.run();
    let (_p, sid) = place_master(&mut h, 2);
    h.key_press(egui::Key::F2);
    h.run();
    h.get_by_role(egui::Role::MultilineTextInput)
        .set_text_value_accesskit("committed");
    h.run();
    assert!(h.state().debug_text_edit().is_some(), "editor open");

    // Switching to Page-2 ends the edit and commits the draft.
    h.get_by_label("Page-2").click();
    h.run();
    assert_eq!(
        h.state().debug_text_edit(),
        None,
        "the edit ended on the page switch"
    );
    // The commit landed on PAGE-1's shape — shape reads are
    // active-page-scoped, so switch back before asserting.
    h.get_by_label("Page-1").click();
    h.run();
    assert_eq!(
        h.state().debug_shape_text(sid).as_deref(),
        Some("committed"),
        "the draft committed through SetText"
    );

    // Same-frame edge (branch review): a typing event and the tab
    // click queued in ONE harness pass — the click's switch_page runs
    // before the editor widget sees the text. The pending input must
    // ride along into the commit, not vanish.
    let (_p2, sid2) = place_master(&mut h, 2);
    h.key_press(egui::Key::F2);
    h.run();
    h.get_by_role(egui::Role::MultilineTextInput)
        .set_text_value_accesskit("edge");
    h.run();
    assert!(h.state().debug_text_edit().is_some());
    // Deliver a keystroke AND the Page-2 tab click together.
    let tab2 = h.get_by_label("Page-2").rect().center();
    h.event(egui::Event::Text("!".into()));
    h.event(egui::Event::PointerButton {
        pos: tab2,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
    h.event(egui::Event::PointerButton {
        pos: tab2,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    h.run();
    h.get_by_label("Page-1").click();
    h.run();
    assert_eq!(
        h.state().debug_shape_text(sid2).as_deref(),
        Some("edge!"),
        "the same-frame keystroke survived into the commit"
    );
}

/// GitHub #48: the context menu must derive every color it paints from
/// the theme mapping over the live visuals — hover fill, label,
/// disabled label, shortcut hint, separators and icon tints — so the
/// menu stays readable when the shell runs under dark visuals, not
/// just the installed Visio-light ones.
fn open_menu_harness(dark: bool) -> Harness<'static, App> {
    dc_ui::svgimg::pin_determinism_for_tests();
    let mut h = Harness::builder()
        .with_size(egui::vec2(1280.0, 800.0))
        .build_ui_state(
            move |ui, app: &mut App| {
                dc_ui::theme::install(ui.ctx());
                if dark {
                    // install() pins Light (the app is a light app; see
                    // the #48 review — eframe on Linux reports no system
                    // theme and egui falls back to Dark). The dark test
                    // re-switches AFTER it, per frame, to keep the menu's
                    // dark token path exercised.
                    ui.ctx().set_theme(egui::ThemePreference::Dark);
                }
                app.draw(ui);
            },
            App::with_blank(),
        );
    h.run();
    let from = h.state().debug_master_cells()[0].1.center();
    let canvas = h.state().debug_canvas_rect();
    // High on the canvas so the tall menu fits beside the shape.
    let to = egui::pos2(canvas.center().x, canvas.top() + 80.0);
    click_at(&mut h, from, egui::PointerButton::Primary);
    click_at(&mut h, to, egui::PointerButton::Primary);
    click_at(&mut h, to, egui::PointerButton::Secondary);
    h.run();
    assert!(
        h.state().debug_ctx_menu_rect().is_some(),
        "context menu opened{}",
        if dark { " (dark visuals)" } else { "" }
    );
    h
}

fn ctx_menu_snapshot(h: &mut Harness<'_, App>, name: &str) {
    // Tolerances mirror shell.rs: anti-aliasing differs per GPU/driver.
    h.snapshot_options(
        name,
        &egui_kittest::SnapshotOptions::new()
            .threshold(3.0)
            .max_failed_pixels(10_000),
    );
}

// ------------------------------------------------------------------
// GitHub #53: the stencil grid re-wraps to the live pane width.

/// Drag the stencil panel's resize handle so the panel's outer width
/// becomes `target_x`, moving through intermediate widths with a
/// layout pass at each (the live-resize path, not a jump).
fn drag_stencil_edge_to(h: &mut Harness<'_, App>, target_x: f32) {
    let inner = h.state().debug_stencil_panel_inner_rect();
    // The handle's sense zone is the panel's outer edge (inner right +
    // the 6px frame margin) ±3px. Press 1.5px INSIDE the edge: at the
    // edge itself the canvas (shown after the panel, so on top) wins
    // the pointer and the drag becomes a canvas marquee.
    let start = egui::pos2(inner.right() + 6.0 - 1.5, inner.center().y);
    let b = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    h.hover_at(start);
    h.event(b(start, true));
    for t in [0.34, 0.67, 1.0] {
        let x = start.x + (target_x - start.x) * t;
        h.hover_at(egui::pos2(x, start.y));
        h.run();
    }
    h.event(b(egui::pos2(target_x, start.y), false));
    h.run();
}

/// All cells lie inside the pane content rect (nothing clips) and
/// every master has a cell.
fn assert_grid_fits(h: &Harness<'_, App>) {
    let inner = h.state().debug_stencil_panel_inner_rect();
    let cells = h.state().debug_master_cells();
    // #102: the document entry is not pinned — the starter masters
    // appear as Quick Shapes' head cells only.
    assert_eq!(
        cells.len(),
        6,
        "the starter head masters in Quick Shapes only (#102)"
    );
    for (id, r) in &cells {
        assert!(
            r.right() <= inner.right() + 0.5,
            "master {id:?} cell {r:?} exceeds pane content {inner:?}"
        );
        assert!(
            r.left() >= inner.left() - 0.5,
            "master {id:?} clipped at the left edge"
        );
    }
}

#[test]
fn stencil_rewraps_to_pane_width_during_resize() {
    let mut h = harness_with_blank();
    h.run();
    h.run(); // settle
    let cols0 = h.state().debug_stencil_cols();
    assert_eq!(cols0, 2, "default 160px pane starts as 2 columns");

    // Widen to ~380px in one live drag; the layout passes at the
    // intermediate widths must already show the wider wraps (masters
    // re-arrange during the drag itself — no settle step).
    let inner = h.state().debug_stencil_panel_inner_rect();
    let start = egui::pos2(inner.right() + 6.0 - 1.5, inner.center().y);
    let target = 380.0f32;
    let mid = start.x + (target - start.x) * 0.5;
    h.hover_at(start);
    h.event(egui::Event::PointerButton {
        pos: start,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
    h.hover_at(egui::pos2(mid, start.y));
    h.run();
    let cols_mid = h.state().debug_stencil_cols();
    assert!(
        cols_mid > cols0,
        "mid-drag (≈{mid:.0}px pane) still wraps at {cols0} columns"
    );
    h.event(egui::Event::PointerButton {
        pos: egui::pos2(target, start.y),
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    h.run();
    h.run();

    // At the 380px stop the wrap is strictly wider than the 2-column
    // default and everything fits the pane (no mirror of the layout
    // formula here on purpose: a hand-copied twin of columns_for can
    // pass while both copies are wrong — behavior anchors instead).
    let mut cols_380 = 0;
    for _ in 0..3 {
        cols_380 = h.state().debug_stencil_cols();
        assert!(cols_380 >= 4, "380px pane wraps to 4+ columns, got {cols_380}");
        assert_grid_fits(&h);
        h.run();
    }

    // And back down through the issue's other stops: monotonic
    // narrowing, everything still fitting.
    drag_stencil_edge_to(&mut h, 260.0);
    let cols_260 = h.state().debug_stencil_cols();
    assert!(
        cols_260 > 2 && cols_260 < cols_380,
        "260px pane narrows between the default and 380px: {cols_260}"
    );
    assert_grid_fits(&h);
    drag_stencil_edge_to(&mut h, 160.0);
    let cols_160 = h.state().debug_stencil_cols();
    assert!(
        cols_160 <= cols_260,
        "nominally-160px pane narrows to at most the 260px wrap: {cols_160}"
    );
    // (No exact count here on purpose: dragging the outer edge to
    // 160px lands marginally narrower than the pristine default pane,
    // so the wrap may legitimately be 1 column where the fresh app
    // shows 2. The pristine default is pinned by the cols0 assert.)
    assert_grid_fits(&h);
}

/// The drag-to-place flow reads cell rects from the live layout: right
/// after a resize, dragging a master still places a shape (GitHub #53
/// verification: the drag ghost originates from the new cell).
#[test]
fn stencil_drag_works_right_after_resize() {
    let mut h = harness_with_blank();
    h.run();
    h.run();
    drag_stencil_edge_to(&mut h, 380.0);
    h.run();

    let cells = h.state().debug_master_cells();
    assert!(
        h.state().debug_stencil_cols() >= 4,
        "the 380px pane re-wrapped to 4+ columns"
    );
    let from = cells[0].1.center();
    let to = h.state().debug_canvas_rect().center();
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
        "drag from the re-wrapped stencil placed a shape"
    );
}

/// The pane aligned with the Visio 2016 reference (GitHub #53 review):
/// the stencil list entry sits above the grid, and the pane at two
/// widths shows the re-wrapped arrangement.
#[test]
fn stencil_pane_snapshots_two_widths() {
    let mut h = harness_with_blank();
    h.run();
    h.run();
    ctx_menu_snapshot(&mut h, "dc_ui_stencil_default_width");
    drag_stencil_edge_to(&mut h, 380.0);
    h.run();
    h.run();
    assert!(
        h.state().debug_stencil_cols() >= 4,
        "wide pane shows 4+ columns"
    );
    ctx_menu_snapshot(&mut h, "dc_ui_stencil_wide");
}

#[test]
fn context_menu_snapshot_light_visuals() {
    let mut h = open_menu_harness(false);
    ctx_menu_snapshot(&mut h, "dc_ui_ctx_menu_light");
}

#[test]
fn context_menu_snapshot_dark_visuals() {
    let mut h = open_menu_harness(true);
    ctx_menu_snapshot(&mut h, "dc_ui_ctx_menu_dark");
}

/// Harness for the File backstage (#55): same shell builder, app state
/// supplied by the caller.
fn backstage_harness(app: App) -> Harness<'static, App> {
    dc_ui::svgimg::pin_determinism_for_tests();
    let mut h = Harness::builder()
        .with_size(egui::vec2(1280.0, 800.0))
        .build_ui_state(
            |ui, app: &mut App| {
                dc_ui::theme::install(ui.ctx());
                app.draw(ui);
            },
            app,
        );
    h.run();
    h
}

/// #55 landing rule: File with a document opens Info; without one (and
/// no recents) it opens the Recent page's welcome state.
#[test]
fn backstage_landing_rule() {
    use egui_kittest::kittest::Queryable;
    // with_blank carries a document (starter masters in the pane).
    let mut with_doc = App::with_blank();
    with_doc.open_backstage_for_test();
    let h = backstage_harness(with_doc);
    assert!(
        h.query_all_by_label_contains("Protect Drawing").count() > 0,
        "with a document the backstage lands on Info"
    );

    let mut no_doc = App::with_doc_opt(None);
    no_doc.open_backstage_for_test();
    let h = backstage_harness(no_doc);
    assert!(
        h.query_all_by_label_contains("New blank drawing").count() > 0,
        "no document + no recents shows the Recent welcome state"
    );
    assert_eq!(
        h.query_all_by_label_contains("Protect Drawing").count(),
        0,
        "Info-only content must not show without a document"
    );
}

/// #55: Esc always returns from the File screen to the document.
#[test]
fn backstage_esc_returns_to_document() {
    use egui_kittest::kittest::Queryable;
    let mut app = App::with_blank();
    app.open_backstage_for_test();
    let mut h = backstage_harness(app);
    assert!(h.state().debug_backstage(), "backstage is open");
    h.key_press(egui::Key::Escape);
    h.run();
    assert!(
        !h.state().debug_backstage(),
        "Esc closed the backstage back to the document"
    );
    assert!(
        h.query_all_by_label_contains("Developer").count() > 0,
        "the shell (tab strip) is back after Esc"
    );
}

/// #55: the New page's search filters the template gallery instantly
/// (elementary/Apple: results as you type).
#[test]
fn backstage_search_filters_templates() {
    use egui_kittest::kittest::Queryable;
    let mut app = App::with_blank();
    app.open_backstage_for_test();
    app.set_backstage_page_for_test("new");
    let mut h = backstage_harness(app);
    assert!(
        h.query_all_by_label_contains("Network Diagram").count() > 0,
        "gallery shows the placeholder templates unfiltered"
    );

    let edit = h.get_by_role(egui::Role::TextInput);
    edit.set_text_value_accesskit("flow");
    h.run();
    assert!(
        h.query_all_by_label_contains("Flowchart").count() > 0,
        "flowchart survives the “flow” query"
    );
    assert_eq!(
        h.query_all_by_label_contains("Network Diagram").count(),
        0,
        "network is filtered out by “flow”"
    );

    h.get_by_role(egui::Role::TextInput)
        .set_text_value_accesskit("net");
    h.run();
    assert_eq!(
        h.query_all_by_label_contains("Flowchart").count(),
        0,
        "flowchart is filtered out by “net”"
    );
    assert!(
        h.query_all_by_label_contains("Network Diagram").count() > 0,
        "network survives the “net” query"
    );
}

/// Wiring leg of the #59 flattened-group cache-refresh coverage
/// (#97 §4 relocation): the semantic facts — sub-shape/Txt/geometry
/// caches re-evaluate through User indirections, GUARD'ed sizes stay,
/// one undo per refresh — are unit-tested in cmd.rs
/// (group_cache_refresh_reevaluates_inh_caches_through_user_indirections)
/// against the shared taskg fixture. What needs the real shell is only
/// this: the corner-drag GESTURE commits that refresh Macro, and one
/// undo reverts it.
#[test]
fn resizing_a_flattened_group_commits_the_cache_refresh() {
    let (ed, page_id, [root_id, _, _]) = dc_ui::group_fixture::taskg_editor();

    // Round-trip through the package writer: the fixture must survive a
    // save/open like any real file.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("taskg.vsdx");
    ed.save_as(&path).unwrap();
    {
        let back = dc_core::Editor::open(&path).unwrap();
        let m = back.doc.master(10).expect("master10");
        assert_eq!(
            m.shapes.first().map(|r| r.sub_shapes.len()),
            Some(2),
            "master sub-shapes survived the round-trip"
        );
    }

    dc_ui::svgimg::pin_determinism_for_tests();
    let mut h = Harness::builder()
        .with_size(egui::vec2(1280.0, 800.0))
        .build_ui_state(
            |ui, app: &mut App| {
                dc_ui::theme::install(ui.ctx());
                app.draw(ui);
            },
            App::with_doc(&path),
        );
    h.run();
    h.state_mut().select_shape_for_test(page_id, root_id);
    h.run();
    let (w0, h0) = (h.state().debug_sel_width(), h.state().debug_sel_height());
    assert!((w0 - 1.0).abs() < 1e-6 && (h0 - 0.5).abs() < 1e-6);

    let br = h.state().debug_sel_br_corner().expect("sel");
    drag_naturally(&mut h, br, egui::vec2(40.0, 30.0));
    let (w1, h1) = (h.state().debug_sel_width(), h.state().debug_sel_height());
    assert!(w1 > 1.2 && h1 > 1.2 * 0.5, "resize grew: {w0:.3}x{h0:.3} -> {w1:.3}x{h1:.3}");

    // Spot-check that the gesture ran the refresh (semantics live in the
    // unit test): the Txt block re-centered through the User
    // indirection, and the refreshed cell kept F='Inh'.
    let txt_px = cell_of(&h, page_id, root_id, "TxtPinX");
    assert!(
        (txt_px - w1 / 2.0).abs() < 1e-3,
        "TxtPinX re-centered through the gesture: {txt_px:.4} vs {:.4}",
        w1 / 2.0
    );
    let (_, f) = h
        .state()
        .debug_cell(page_id, root_id, "TxtPinX")
        .expect("TxtPinX cell");
    assert_eq!(f, "Inh", "refreshed cache keeps its Inh marker");

    // One undo reverts the whole resize Macro. Since #136 the undo
    // also restores the touched selection: the group stays selected
    // (this test used to dodge the selection because undo cleared it).
    h.key_press_modifiers(egui::Modifiers::CTRL, egui::Key::Z);
    h.run();
    assert_eq!(
        h.state().debug_sel(),
        Some((page_id, root_id)),
        "#136: undoing the resize keeps the resized group selected"
    );
    assert!(
        (cell_of(&h, page_id, root_id, "Width") - 1.0).abs() < 1e-6,
        "undo restored the pre-resize width"
    );
    assert!(
        (cell_of(&h, page_id, root_id, "TxtPinX") - 0.5).abs() < 1e-6,
        "undo restored the authored TxtPinX cache"
    );
}

/// GitHub #25: master ShapeSheet Actions rows execute through the
/// command layer — the blank Rectangle master carries a synthetic
/// Actions section (User.Style layouts; ours, not Microsoft's).
#[test]
fn master_action_row_executes_setf_chain() {
    use egui_kittest::kittest::Queryable;
    let mut h = harness_with_blank();
    h.run();
    let from = h.state().debug_master_cells()[0].1.center();
    let to = h.state().debug_canvas_rect().center();
    click_at(&mut h, from, egui::PointerButton::Primary);
    click_at(&mut h, to, egui::PointerButton::Primary);
    assert_eq!(h.state().debug_shape_count(), 1);
    let (_, sid) = h.state().debug_sel().expect("placed shape selected");
    assert_eq!(
        h.state().debug_user_value_for_test(sid, "Style"),
        None,
        "User.Style is inherited from the master before any action"
    );

    // The master's Actions rows head the context menu; the labels live
    // in Menu text content (multi-line cells).
    click_at(&mut h, to, egui::PointerButton::Secondary);
    h.run();
    assert!(
        h.get_all_by_label_contains("Rectangle text style").count() > 0,
        "flyout header row present"
    );
    h.get_all_by_label_contains("2. Centered")
        .into_iter()
        .find(|n| n.rect().height() < 30.0)
        .expect("Centered action row")
        .click();
    h.run();

    // The SETF chain wrote instance-locally: User.Style = 1.
    assert_eq!(
        h.state().debug_user_value_for_test(sid, "Style"),
        Some(1.0),
        "the action executed its SETF chain on the instance"
    );
    // One undo restores the inherited row.
    h.key_press_modifiers(egui::Modifiers::CTRL, egui::Key::Z);
    h.run();
    assert_eq!(
        h.state().debug_user_value_for_test(sid, "Style"),
        None,
        "undo removed the materialized row (inheritance restored)"
    );
}

/// GitHub #26: marquee-select two shapes, Group from the context menu,
/// move the group (members follow), then undo both.
#[test]
fn context_menu_group_creates_one_shape_that_moves_as_one() {
    use egui_kittest::kittest::Queryable;
    let mut h = harness_with_blank();
    h.run();
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

    // Marquee both shapes.
    let c = canvas.center();
    let a = c + egui::vec2(-120.0, -120.0);
    let b = c + egui::vec2(120.0, 120.0);
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

    // Right-click one of them and Group.
    click_at(
        &mut h,
        c + egui::vec2(-40.0, 40.0),
        egui::PointerButton::Secondary,
    );
    h.run();
    h.run(); // the popup's screen-edge re-anchor settles after a frame
    // The ribbon also has a Group button and the icon bar has a Group
    // icon (#77) — scope the pick to the ctx menu's own rect (#97 §4:
    // no hardcoded band) and to full-width ROWS (the 22 px bar icon
    // dispatches immediately instead of opening the flyout). GitHub
    // #78: that row is the Group ▸ flyout parent; the action lives in
    // its submenu.
    let menu = h.state().debug_ctx_menu_rect().expect("menu still open");
    let group_row = h
        .get_all_by_label_contains("Group")
        .filter(|n| menu.expand(2.0).contains_rect(n.rect()) && n.rect().width() > 100.0)
        .min_by_key(|n| n.rect().min.y as i64)
        .expect("Group ▸ flyout row in the context menu");
    group_row.click();
    h.run();
    let menu = h.state().debug_ctx_menu_rect().expect("menu still open");
    let group_item = h
        .get_all_by_label_contains("Group")
        .find(|n| {
            let r = n.rect();
            !menu.expand(2.0).contains_rect(r) && r.min.y > menu.min.y - 5.0
        })
        .expect("Group item in the flyout submenu");
    group_item.click();
    h.run();
    assert_eq!(h.state().debug_shape_count(), 1, "one top-level group");
    let (_, gid) = h.state().debug_sel().expect("group selected");
    let g0 = h.state().debug_shape_screen_rect(gid).expect("group rect");

    // Moving the group moves the whole composition (members live in
    // its frame).
    let before_svg = h.state().debug_page_svg().unwrap_or_default();
    assert!(before_svg.contains("<path"), "the grouped members render");
    drag_naturally(&mut h, g0.center(), egui::vec2(60.0, 40.0));
    let g1 = h.state().debug_shape_screen_rect(gid).expect("group rect");
    assert!(
        (g1.min.x - g0.min.x - 60.0).abs() < 1.0 && (g1.min.y - g0.min.y - 40.0).abs() < 1.0,
        "group moved: {:?} → {:?}",
        g0,
        g1
    );

    // Undo the move, then the group itself.
    h.key_press_modifiers(egui::Modifiers::CTRL, egui::Key::Z);
    h.run();
    let g2 = h.state().debug_shape_screen_rect(gid).expect("group rect");
    assert!((g2.min.x - g0.min.x).abs() < 0.5, "move undone");
    h.key_press_modifiers(egui::Modifiers::CTRL, egui::Key::Z);
    h.run();
    assert_eq!(h.state().debug_shape_count(), 2, "ungroup via undo");
}

/// #138: deleting a container strips its members' membership rows in
/// the same undo step — members stay put, no dangling rows, one
/// Ctrl+Z restores everything — and the styled heading band renders
/// (band fill + count heading).
#[test]
fn container_delete_strips_membership_rows() {
    use egui_kittest::kittest::Queryable;
    let mut h = harness_with_blank();
    h.run();
    // Two shapes wrapped in a container (#27 flow).
    let cells = h.state().debug_master_cells();
    let from = cells[0].1.center();
    let canvas = h.state().debug_canvas_rect();
    for off in [-40.0f32, 40.0f32] {
        let to = canvas.center() + egui::vec2(off, -off);
        click_at(&mut h, from, egui::PointerButton::Primary);
        click_at(&mut h, to, egui::PointerButton::Primary);
    }
    h.run();
    let c = canvas.center();
    let s0 = c + egui::vec2(-120.0, -120.0);
    let s1 = c + egui::vec2(120.0, 120.0);
    h.hover_at(s0);
    h.event(egui::Event::PointerButton {
        pos: s0,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
    h.hover_at(s0 + (s1 - s0) * 0.5);
    h.run();
    h.hover_at(s1);
    h.event(egui::Event::PointerButton {
        pos: s1,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    h.run();
    click_at(&mut h, c + egui::vec2(-40.0, 40.0), egui::PointerButton::Secondary);
    h.run();
    let menu = h.state().debug_ctx_menu_rect().expect("context menu open");
    h.get_all_by_label_contains("Container")
        .filter(|n| menu.expand(2.0).contains_rect(n.rect()))
        .min_by_key(|n| n.rect().min.y as i64)
        .expect("Container flyout parent")
        .click();
    h.run();
    h.run();
    h.get_all_by_label_contains("Add to New Container")
        .into_iter()
        .find(|n| {
            let r = n.rect();
            !menu.expand(2.0).contains_rect(r) && r.min.y > menu.min.y - 5.0
        })
        .expect("Add to New Container row")
        .click();
    h.run();
    let (_, cont) = h.state().debug_sel().expect("container selected");

    // Styled band (#138 §2): the heading carries the member count,
    // the page SVG renders the band fill, and the heading GLYPHS sit
    // INSIDE the band (branch review: the text block defaulted to the
    // full container height and towered invisibly above it).
    assert_eq!(
        h.state().debug_shape_text(cont).as_deref(),
        Some("Container (2)"),
        "the heading band shows the count"
    );
    let svg = h.state().debug_page_svg().unwrap_or_default();
    assert!(
        svg.contains("#2e74b5"),
        "the band fill (the highlight blue) renders"
    );
    {
        // The heading tspan's local y-down y ≈ -(H - 0.125) — the band
        // center of a 0.25-in band at the container's top.
        let hgt = h
            .state()
            .debug_cell_for_test(cont, "Height")
            .and_then(|v| v.parse::<f64>().ok())
            .expect("container Height");
        let band_center = -(hgt - 0.125);
        let tspan = svg
            .split("<tspan")
            .find(|t| t.contains("Container (2)"))
            .expect("the heading tspan");
        let y: f64 = tspan
            .split("y=\"")
            .nth(1)
            .and_then(|rest| rest.split('"').next())
            .and_then(|v| v.parse().ok())
            .expect("tspan y");
        assert!(
            (y - band_center).abs() < 0.06,
            "the heading renders in the band: y={y} vs band center {band_center}"
        );
    }

    // Delete the container: members stay put, rows stripped.
    assert_eq!(h.state().debug_membership_rows_for_test(), 2);
    let m1_before = h.state().debug_shape_screen_rect(1).unwrap();
    h.key_press(egui::Key::Delete);
    h.run();
    assert_eq!(h.state().debug_shape_count(), 2, "members orphaned in place");
    assert_eq!(
        h.state().debug_membership_rows_for_test(),
        0,
        "no dangling membership rows"
    );
    let m1_after = h.state().debug_shape_screen_rect(1).unwrap();
    assert!(
        (m1_after.min.x - m1_before.min.x).abs() < 0.5,
        "the member stayed put"
    );

    // One Ctrl+Z restores the container AND both rows.
    h.key_press_modifiers(egui::Modifiers::CTRL, egui::Key::Z);
    h.run();
    assert_eq!(h.state().debug_shape_count(), 3, "the container is back");
    assert_eq!(
        h.state().debug_membership_rows_for_test(),
        2,
        "one undo restored the membership rows"
    );
}

/// #75: Add to Underlying Container — a shape placed over an existing
/// container joins it through the flyout item, moves with it, and one
/// Ctrl+Z removes only the membership write.
#[test]
fn add_to_underlying_container_members_and_carries() {
    use egui_kittest::kittest::Queryable;
    let mut h = harness_with_blank();
    h.run();
    // Two shapes, wrapped in a container (#27 flow).
    let cells = h.state().debug_master_cells();
    let from = cells[0].1.center();
    let canvas = h.state().debug_canvas_rect();
    for off in [-40.0f32, 40.0f32] {
        let to = canvas.center() + egui::vec2(off, -off);
        click_at(&mut h, from, egui::PointerButton::Primary);
        click_at(&mut h, to, egui::PointerButton::Primary);
    }
    h.run();
    let c = canvas.center();
    let s0 = c + egui::vec2(-120.0, -120.0);
    let s1 = c + egui::vec2(120.0, 120.0);
    h.hover_at(s0);
    h.event(egui::Event::PointerButton {
        pos: s0,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
    h.hover_at(s0 + (s1 - s0) * 0.5);
    h.run();
    h.hover_at(s1);
    h.event(egui::Event::PointerButton {
        pos: s1,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    h.run();
    click_at(&mut h, c + egui::vec2(-40.0, 40.0), egui::PointerButton::Secondary);
    h.run();
    let menu = h.state().debug_ctx_menu_rect().expect("context menu open");
    h.get_all_by_label_contains("Container")
        .filter(|n| menu.expand(2.0).contains_rect(n.rect()))
        .min_by_key(|n| n.rect().min.y as i64)
        .expect("Container flyout parent")
        .click();
    h.run();
    h.run();
    h.get_all_by_label_contains("Add to New Container")
        .into_iter()
        .find(|n| {
            let r = n.rect();
            !menu.expand(2.0).contains_rect(r) && r.min.y > menu.min.y - 5.0
        })
        .expect("Add to New Container row")
        .click();
    h.run();
    let (_, cont) = h.state().debug_sel().expect("container selected");
    assert_eq!(h.state().debug_shape_count(), 3, "two members + container");
    assert_eq!(
        h.state().debug_container_members_for_test(cont),
        vec![1, 2],
        "#27 membership"
    );

    // A third shape placed over the container, selected by placement.
    click_at(&mut h, from, egui::PointerButton::Primary);
    click_at(&mut h, c, egui::PointerButton::Primary);
    h.run();
    let (_, third) = h.state().debug_sel().expect("third placed + selected");
    assert!(
        !h.state()
            .debug_container_members_for_test(cont)
            .contains(&third),
        "no membership yet"
    );

    // Right-click → Container ▸ → Add to Underlying Container (enabled
    // now — a container sits under the selection and the shape is not
    // its member).
    click_at(&mut h, c, egui::PointerButton::Secondary);
    h.run();
    let menu = h.state().debug_ctx_menu_rect().expect("context menu open");
    h.get_all_by_label_contains("Container")
        .filter(|n| menu.expand(2.0).contains_rect(n.rect()))
        .min_by_key(|n| n.rect().min.y as i64)
        .expect("Container flyout parent")
        .click();
    h.run();
    h.run();
    h.get_all_by_label_contains("Add to Underlying Container")
        .into_iter()
        .find(|n| {
            let r = n.rect();
            !menu.expand(2.0).contains_rect(r) && r.min.y > menu.min.y - 5.0
        })
        .expect("Add to Underlying row")
        .click();
    h.run();
    assert_eq!(
        h.state().debug_container_members_for_test(cont),
        vec![1, 2, third],
        "the third joined the underlying container"
    );

    // The third now moves with the container (#27 carry semantics).
    // Press the CONTAINER, not its center — the third sits on top
    // there and a center press would drag the shape directly. The
    // left-edge margin is container-only; members 1 and 2 moving too
    // proves the container itself was dragged.
    let cont_grab = {
        let r = h.state().debug_shape_screen_rect(cont).unwrap();
        egui::pos2(r.left() + 8.0, r.center().y)
    };
    let third_before = h.state().debug_shape_screen_rect(third).unwrap();
    let m1_before = h.state().debug_shape_screen_rect(1).unwrap();
    drag_naturally(&mut h, cont_grab, egui::vec2(60.0, 0.0));
    let third_after = h.state().debug_shape_screen_rect(third).unwrap();
    let m1_after = h.state().debug_shape_screen_rect(1).unwrap();
    assert!(
        (third_after.min.x - third_before.min.x - 60.0).abs() < 1.5,
        "the third carried with the container: {:?} -> {:?}",
        third_before,
        third_after
    );
    assert!(
        (m1_after.min.x - m1_before.min.x - 60.0).abs() < 1.5,
        "the container itself moved (member 1 carried)"
    );

    // Undo the container move, then undo ONLY the membership write:
    // the third stays put on the next container drag.
    h.key_press_modifiers(egui::Modifiers::CTRL, egui::Key::Z);
    h.run();
    h.key_press_modifiers(egui::Modifiers::CTRL, egui::Key::Z);
    h.run();
    assert_eq!(
        h.state().debug_container_members_for_test(cont),
        vec![1, 2],
        "one undo removed only the membership write"
    );
    let third_parked = h.state().debug_shape_screen_rect(third).unwrap();
    let cont_grab = {
        let r = h.state().debug_shape_screen_rect(cont).unwrap();
        egui::pos2(r.left() + 8.0, r.center().y)
    };
    drag_naturally(&mut h, cont_grab, egui::vec2(40.0, 0.0));
    let third_now = h.state().debug_shape_screen_rect(third).unwrap();
    assert!(
        (third_now.min.x - third_parked.min.x).abs() < 1.0,
        "the third no longer carries after the membership undo"
    );
}

/// #95 §1: grouping a selection of {one group + other shapes} nests a
/// new group INSIDE the existing one — members keep their page-space
/// positions through the local-frame conversion, and one undo
/// restores the top-level layout.
#[test]
fn group_into_existing_group_nests() {
    use egui_kittest::kittest::Queryable;
    let mut h = harness_with_blank();
    h.run();
    // Three shapes: a pair to group first, plus a loner.
    let cells = h.state().debug_master_cells();
    let from = cells[0].1.center();
    let canvas = h.state().debug_canvas_rect();
    for off in [-40.0f32, 40.0f32, 0.0f32] {
        let to = canvas.center() + egui::vec2(off, -off);
        click_at(&mut h, from, egui::PointerButton::Primary);
        click_at(&mut h, to, egui::PointerButton::Primary);
    }
    h.run();
    assert_eq!(h.state().debug_shape_count(), 3);
    // Marquee all three, right-click, Group — with THREE plain shapes
    // this makes a top-level group (the pre-#95 behavior): G1 holds
    // members 1, 2, 3 (placement ids are sequential).
    let c = canvas.center();
    let s0 = c + egui::vec2(-120.0, -120.0);
    let s1 = c + egui::vec2(120.0, 120.0);
    h.hover_at(s0);
    h.event(egui::Event::PointerButton {
        pos: s0,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
    h.hover_at(s0 + (s1 - s0) * 0.5);
    h.run();
    h.hover_at(s1);
    h.event(egui::Event::PointerButton {
        pos: s1,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    h.run();
    click_at(&mut h, c + egui::vec2(-40.0, 40.0), egui::PointerButton::Secondary);
    h.run();
    h.run();
    let menu = h.state().debug_ctx_menu_rect().expect("menu open");
    let group_row = h
        .get_all_by_label_contains("Group")
        .filter(|n| menu.expand(2.0).contains_rect(n.rect()) && n.rect().width() > 100.0)
        .min_by_key(|n| n.rect().min.y as i64)
        .expect("Group ▸ flyout row");
    group_row.click();
    h.run();
    let menu = h.state().debug_ctx_menu_rect().expect("menu still open");
    let group_item = h
        .get_all_by_label_contains("Group")
        .find(|n| {
            let r = n.rect();
            !menu.expand(2.0).contains_rect(r) && r.min.y > menu.min.y - 5.0
        })
        .expect("Group item");
    group_item.click();
    h.run();
    assert_eq!(h.state().debug_shape_count(), 1, "one top-level group");
    let (_, g1) = h.state().debug_sel().expect("group selected");

    // Place a fourth shape, then select {G1, shape4} and Group: with
    // exactly one GROUP in the selection this NESTS (#95) — shape4
    // lands inside a new sub-group of G1, page-space stable.
    click_at(&mut h, from, egui::PointerButton::Primary);
    click_at(&mut h, canvas.center() + egui::vec2(150.0, 0.0), egui::PointerButton::Primary);
    h.run();
    assert_eq!(h.state().debug_shape_count(), 2, "group plus the new shape");
    let s4 = h.state().debug_sel().expect("placed").1;
    let s4_before = h.state().debug_shape_screen_rect(s4).expect("shape4 rect");

    // Multi-select G1 + shape4: click G1, shift-click shape4.
    let g1c = h.state().debug_shape_screen_rect(g1).unwrap().center();
    let s4c = s4_before.center();
    click_at(&mut h, g1c, egui::PointerButton::Primary);
    let shift = egui::Modifiers {
        shift: true,
        ..Default::default()
    };
    h.event(egui::Event::ModifiersChanged(shift));
    h.hover_at(s4c);
    h.event(egui::Event::PointerButton {
        pos: s4c,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: shift,
    });
    h.event(egui::Event::PointerButton {
        pos: s4c,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: shift,
    });
    h.run();
    h.event(egui::Event::ModifiersChanged(egui::Modifiers::NONE));
    h.run();
    assert_eq!(h.state().debug_sel_set_len(), 2, "group + shape selected");

    // Right-click the selection, Group.
    click_at(&mut h, g1c, egui::PointerButton::Secondary);
    h.run();
    h.run();
    let menu = h.state().debug_ctx_menu_rect().expect("menu open");
    let group_row = h
        .get_all_by_label_contains("Group")
        .filter(|n| menu.expand(2.0).contains_rect(n.rect()) && n.rect().width() > 100.0)
        .min_by_key(|n| n.rect().min.y as i64)
        .expect("Group ▸ flyout row");
    group_row.click();
    h.run();
    let menu = h.state().debug_ctx_menu_rect().expect("menu still open");
    let group_item = h
        .get_all_by_label_contains("Group")
        .find(|n| {
            let r = n.rect();
            !menu.expand(2.0).contains_rect(r) && r.min.y > menu.min.y - 5.0
        })
        .expect("Group item");
    group_item.click();
    h.run();

    // Nested: still ONE top-level shape; the new group (selected) is a
    // MEMBER of G1, holding shape4.
    assert_eq!(
        h.state().debug_shape_count(),
        1,
        "nesting kept one top-level group"
    );
    let (_, g2) = h.state().debug_sel().expect("the new sub-group is selected");
    assert_ne!(g2, g1, "a new group was created");
    assert_eq!(
        h.state().debug_group_members_for_test(g1),
        vec![1, 2, 3, g2],
        "the new group joined G1's members"
    );
    assert_eq!(
        h.state().debug_group_members_for_test(g2),
        vec![s4],
        "the sub-group holds shape4"
    );
    // Page-space stability through the local-frame conversion.
    let s4_after = h.state().debug_shape_screen_rect(s4).expect("shape4 rect");
    assert!(
        (s4_after.min.x - s4_before.min.x).abs() < 1.0
            && (s4_after.min.y - s4_before.min.y).abs() < 1.0
            && (s4_after.width() - s4_before.width()).abs() < 1.0,
        "shape4 renders where it was: {:?} -> {:?}",
        s4_before,
        s4_after
    );

    // One undo restores the top-level layout: G1 (with 1,2) + shape4.
    h.key_press_modifiers(egui::Modifiers::CTRL, egui::Key::Z);
    h.run();
    assert_eq!(h.state().debug_shape_count(), 2, "undo restored the layout");
    assert_eq!(
        h.state().debug_group_members_for_test(g1),
        vec![1, 2, 3],
        "G1 back to its original members"
    );
}

/// #69: resizing a group SCALES its members — both member screen
/// rects grow with the group's frame, and one Ctrl+Z restores the
/// members and the group in a single step.
#[test]
fn group_resize_scales_its_members() {
    use egui_kittest::kittest::Queryable;
    let mut h = harness_with_blank();
    h.run();
    let cells = h.state().debug_master_cells();
    let from = cells[0].1.center();
    let canvas = h.state().debug_canvas_rect();
    for off in [-40.0f32, 40.0f32] {
        let to = canvas.center() + egui::vec2(off, -off);
        click_at(&mut h, from, egui::PointerButton::Primary);
        click_at(&mut h, to, egui::PointerButton::Primary);
    }
    h.run();
    // Marquee both, right-click, Group (the flyout row — full-width,
    // the icon bar's 22 px Group icon dispatches instead).
    let c = canvas.center();
    let s0 = c + egui::vec2(-120.0, -120.0);
    let s1 = c + egui::vec2(120.0, 120.0);
    h.hover_at(s0);
    h.event(egui::Event::PointerButton {
        pos: s0,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
    h.hover_at(s0 + (s1 - s0) * 0.5);
    h.run();
    h.hover_at(s1);
    h.event(egui::Event::PointerButton {
        pos: s1,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    h.run();
    click_at(&mut h, c + egui::vec2(-40.0, 40.0), egui::PointerButton::Secondary);
    h.run();
    h.run();
    let menu = h.state().debug_ctx_menu_rect().expect("menu open");
    let group_row = h
        .get_all_by_label_contains("Group")
        .filter(|n| menu.expand(2.0).contains_rect(n.rect()) && n.rect().width() > 100.0)
        .min_by_key(|n| n.rect().min.y as i64)
        .expect("Group ▸ flyout row");
    group_row.click();
    h.run();
    let menu = h.state().debug_ctx_menu_rect().expect("menu still open");
    let group_item = h
        .get_all_by_label_contains("Group")
        .find(|n| {
            let r = n.rect();
            !menu.expand(2.0).contains_rect(r) && r.min.y > menu.min.y - 5.0
        })
        .expect("Group item in the flyout submenu");
    group_item.click();
    h.run();
    let (_, gid) = h.state().debug_sel().expect("group selected");
    let members = h.state().debug_group_members_for_test(gid);
    assert_eq!(members.len(), 2, "the group holds both shapes");
    let rect_of = |h: &egui_kittest::Harness<'_, App>, m: i64| {
        h.state().debug_shape_screen_rect_for_test(m).expect("member rect")
    };
    let before: Vec<_> = members.iter().map(|&m| rect_of(&h, m)).collect();
    let (gw0, gh0) = {
        let r = h.state().debug_shape_screen_rect(gid).unwrap();
        (r.width(), r.height())
    };

    // Drag the group's bottom-right corner out: the frame grows and
    // BOTH members scale with it (#69).
    let br = h.state().debug_sel_br_corner().expect("group handles");
    drag_naturally(&mut h, br, egui::vec2(40.0, 30.0));
    let (gw1, gh1) = {
        let r = h.state().debug_shape_screen_rect(gid).unwrap();
        (r.width(), r.height())
    };
    assert!(gw1 > gw0 * 1.2 && gh1 > gh0 * 1.2, "the group grew");
    let (fx, fy) = (gw1 / gw0, gh1 / gh0);
    for (i, &m) in members.iter().enumerate() {
        let after = rect_of(&h, m);
        let want_w = before[i].width() * fx;
        let want_h = before[i].height() * fy;
        assert!(
            (after.width() - want_w).abs() < 1.5 && (after.height() - want_h).abs() < 1.5,
            "member {m} scaled with the group: {:?} → {:?} (fx {fx:.2}, fy {fy:.2})",
            before[i],
            after
        );
    }

    // One Ctrl+Z restores the members and the group together.
    h.key_press_modifiers(egui::Modifiers::CTRL, egui::Key::Z);
    h.run();
    for (i, &m) in members.iter().enumerate() {
        let back = rect_of(&h, m);
        assert!(
            (back.width() - before[i].width()).abs() < 0.5
                && (back.height() - before[i].height()).abs() < 0.5,
            "member {m} restored: {:?} vs {:?}",
            back,
            before[i]
        );
    }
}

/// GitHub #27: Container from the context menu wraps the selection;
/// dragging the container carries its members; delete orphans them in
/// place.
#[test]
fn context_menu_container_wraps_and_carries_members() {
    use egui_kittest::kittest::Queryable;
    let mut h = harness_with_blank();
    h.run();
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
    let (_, s1) = h.state().debug_sel().expect("second placed shape selected");
    let r1_before = h.state().debug_shape_screen_rect(s1).expect("member rect");

    // Marquee both, right-click, Container.
    let c = canvas.center();
    let a = c + egui::vec2(-120.0, -120.0);
    let b = c + egui::vec2(120.0, 120.0);
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
    click_at(
        &mut h,
        c + egui::vec2(-40.0, 40.0),
        egui::PointerButton::Secondary,
    );
    h.run();
    // Container opens a flyout (Visio 2016); the action is in it.
    let menu = h.state().debug_ctx_menu_rect().expect("context menu open");
    h.get_all_by_label_contains("Container")
        .filter(|n| menu.expand(2.0).contains_rect(n.rect()))
        .min_by_key(|n| n.rect().min.y as i64)
        .expect("Container flyout parent")
        .click();
    h.run();
    h.run(); // the flyout settles a frame later
             // ctx_item rows carry leading spaces in their labels — match by
             // contains and click at the row's center (get_by_label is exact).
    let rows = h
        .query_all_by_label_contains("Add to New Container")
        .collect::<Vec<_>>();
    let r = rows[0].rect();
    click_at(&mut h, r.center(), egui::PointerButton::Primary);
    h.run();
    assert_eq!(h.state().debug_shape_count(), 3, "members stay top-level");
    let (_, cid) = h.state().debug_sel().expect("container selected");

    // Drag the container by (60, 0): both members move with it.
    let cr = h
        .state()
        .debug_shape_screen_rect(cid)
        .expect("container rect");
    drag_naturally(&mut h, cr.center(), egui::vec2(60.0, 0.0));
    let r1_after = h.state().debug_shape_screen_rect(s1).expect("member rect");
    assert!(
        (r1_after.min.x - r1_before.min.x - 60.0).abs() < 1.5,
        "member followed the container: {:?} → {:?}",
        r1_before,
        r1_after
    );

    // Undo restores the member (one step for the whole move).
    h.key_press_modifiers(egui::Modifiers::CTRL, egui::Key::Z);
    h.run();
    let r1_back = h.state().debug_shape_screen_rect(s1).expect("member rect");
    assert!((r1_back.min.x - r1_before.min.x).abs() < 0.5, "move undone");

    // Deleting the container leaves the members in place (Visio).
    click_at(&mut h, cr.center(), egui::PointerButton::Primary);
    h.run();
    h.key_press(egui::Key::Delete);
    h.run();
    assert_eq!(
        h.state().debug_shape_count(),
        2,
        "members orphaned in place"
    );
    assert!(h.state().debug_shape_screen_rect(s1).is_some());
}

/// GitHub #28: Hyperlink… dialog adds a row through the command layer,
/// and Ctrl+Click follows a SubAddress to its page.
#[test]
fn hyperlink_dialog_edits_rows_and_ctrl_click_navigates() {
    use egui_kittest::kittest::Queryable;
    let mut h = harness_with_blank();
    h.run();
    // Two pages so SubAddress navigation has somewhere to go: insert
    // via the tab context menu (the + button's rect straddles panels
    // and its center misses the click target — separate wart).
    let tab = h.get_by_label("Page-1").rect().center();
    click_at(&mut h, tab, egui::PointerButton::Secondary);
    h.run();
    h.get_all_by_label_contains("Insert Page")
        .into_iter()
        .find(|n| n.rect().height() < 30.0)
        .expect("Insert Page… row")
        .click();
    h.run();
    assert_eq!(h.state().debug_page_names(), vec!["Page-1", "Page-2"]);

    let from = h.state().debug_master_cells()[0].1.center();
    let to = h.state().debug_canvas_rect().center();
    click_at(&mut h, from, egui::PointerButton::Primary);
    click_at(&mut h, to, egui::PointerButton::Primary);
    assert_eq!(h.state().debug_shape_count(), 1);
    let (_, sid) = h.state().debug_sel().expect("placed shape selected");

    // Open Hyperlink… from the context menu, add a row.
    click_at(&mut h, to, egui::PointerButton::Secondary);
    h.run();
    let menu = h.state().debug_ctx_menu_rect().expect("context menu open");
    h.get_all_by_label_contains("Hyperlink")
        .filter(|n| menu.expand(2.0).contains_rect(n.rect()))
        .min_by_key(|n| n.rect().min.y as i64)
        .expect("Hyperlink… row in the context menu")
        .click();
    h.run();
    h.get_by_label("New…").click();
    h.run();
    // Fill the draft's three fields: the dialog's inputs are the
    // TextInputs near the top-left (the stencil search sits lower in
    // the left pane; order = Address, Sub-address, Description).
    // The draft inputs carry accessible names (#56 pattern).
    h.get(
        egui_kittest::kittest::By::default()
            .role(egui::Role::TextInput)
            .label("Address"),
    )
    .set_text_value_accesskit("https://example.test/");
    h.run();
    h.get(
        egui_kittest::kittest::By::default()
            .role(egui::Role::TextInput)
            .label("Sub-address"),
    )
    .set_text_value_accesskit("Page-2");
    h.run();
    h.get_by_label("OK").click();
    h.run();

    // The row landed on the instance and undo removes it.
    let links = h.state().debug_hyperlinks_for_test(sid);
    assert_eq!(
        links,
        vec![("https://example.test/".into(), "Page-2".into())],
        "hyperlink row added through the dialog"
    );
    // Ctrl+Click the shape: SubAddress navigation to Page-2.
    let b = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::CTRL,
    };
    h.hover_at(to);
    h.event(b(to, true));
    h.event(b(to, false));
    h.run();
    assert_eq!(
        h.state().debug_active_page_name(),
        "Page-2",
        "Ctrl+Click followed the SubAddress"
    );
    // Back to page 1 and undo removes the row.
    h.get_by_label("Page-1").click();
    h.run();
    h.key_press_modifiers(egui::Modifiers::CTRL, egui::Key::Z);
    h.run();
    assert!(
        h.state().debug_hyperlinks_for_test(sid).is_empty(),
        "undo removed the hyperlink row"
    );

    // The dialog auto-closed when the page switched (the alive check);
    // the explicit cancel path is covered by its own test below.
}

/// A blank-vsdx copy whose Rectangle master (2) carries a Hyperlink
/// row (#139): instances inherit it into the follow list and the
/// dialog. Synthetic content — no Microsoft bits beyond the blank
/// template this repo already ships.
fn hyperlink_master_fixture(dir: &std::path::Path) -> std::path::PathBuf {
    let src =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../dc-core/assets/blank.vsdx");
    let injection = "  <Section N='Hyperlink'>\
<Row IX='1'><Cell N='Address' V='https://master.example/doc'/><Cell N='Description' V='Inherited link'/></Row>\
</Section>\n</Shape>";
    let path = dir.join("hyperlink-master.vsdx");
    let in_pkg = vsdx::Package::open(&src).unwrap();
    let file = std::fs::File::create(&path).unwrap();
    let mut w = zip::ZipWriter::new(file);
    let opts = zip::write::SimpleFileOptions::default();
    for name in in_pkg.part_names() {
        let data = in_pkg.part(name).unwrap().to_vec();
        let data = if name == "visio/masters/master2.xml" {
            String::from_utf8(data)
                .unwrap()
                .replacen("</Shape>", injection, 1)
                .into_bytes()
        } else {
            data
        };
        w.start_file(name, opts).unwrap();
        std::io::Write::write_all(&mut w, &data).unwrap();
    }
    w.finish().unwrap();
    path
}

/// GitHub #139: the Hyperlink dialog edits master-inherited rows —
/// Edit materializes the row onto the instance sheet (one undo Macro,
/// master untouched), the override shadows the inherited row in the
/// list, and Remove stays read-only on inherited rows (Visio keeps
/// inherited row sets read-only until overridden).
#[test]
fn hyperlink_dialog_edits_master_inherited_rows() {
    use egui_kittest::kittest::Queryable;
    let tmp = tempfile::tempdir().unwrap();
    let doc = hyperlink_master_fixture(tmp.path());
    dc_ui::svgimg::pin_determinism_for_tests();
    let mut h = Harness::builder()
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

    // Place a Rectangle instance (master 2): the inherited row shows
    // in the follow list, nothing on the instance sheet yet.
    let (_page, sid) = place_master(&mut h, 2);
    let inherited = ("https://master.example/doc".to_string(), String::new());
    assert!(
        h.state().debug_instance_hyperlinks_for_test(sid).is_empty(),
        "nothing materialized before any edit"
    );
    assert_eq!(
        h.state().debug_hyperlinks_for_test(sid),
        vec![inherited.clone()],
        "the follow list carries the inherited row"
    );

    // Open Hyperlink… — the dialog lists the row with its origin.
    let to = h.state().debug_shape_screen_rect(sid).unwrap().center();
    click_at(&mut h, to, egui::PointerButton::Secondary);
    h.run();
    let menu = h.state().debug_ctx_menu_rect().expect("context menu open");
    h.get_all_by_label_contains("Hyperlink")
        .filter(|n| menu.expand(2.0).contains_rect(n.rect()))
        .min_by_key(|n| n.rect().min.y as i64)
        .expect("Hyperlink… row in the context menu")
        .click();
    h.run();
    assert!(
        h.query_all_by_label_contains("Inherited link").count() > 0,
        "the dialog lists the inherited row"
    );
    assert!(
        h.query_all_by_label_contains("inherited").count() > 0,
        "the inherited origin is visible"
    );

    // Remove is read-only on the inherited row: clicking it applies
    // nothing.
    h.get_by_label("Remove").click();
    h.run();
    assert!(
        h.state().debug_instance_hyperlinks_for_test(sid).is_empty(),
        "Remove on an inherited row changed nothing"
    );
    assert_eq!(
        h.state().debug_master_hyperlinks_for_test(sid),
        vec![inherited.clone()],
        "the master's row is still there"
    );

    // Edit the inherited row: the draft is prefilled with its values;
    // change the address and OK.
    h.get_by_label("Edit").click();
    h.run();
    h.get(
        egui_kittest::kittest::By::default()
            .role(egui::Role::TextInput)
            .label("Address"),
    )
    .set_text_value_accesskit("https://instance.example/override");
    h.run();
    h.get_by_label("OK").click();
    h.run();

    let overridden = ("https://instance.example/override".to_string(), String::new());
    assert_eq!(
        h.state().debug_instance_hyperlinks_for_test(sid),
        vec![(1, overridden.0.clone(), overridden.1.clone(), "Inherited link".into())],
        "the row materialized on the instance sheet with the new address"
    );
    assert_eq!(
        h.state().debug_master_hyperlinks_for_test(sid),
        vec![inherited.clone()],
        "the master's row is unchanged by the edit"
    );
    assert_eq!(
        h.state().debug_hyperlinks_for_test(sid),
        vec![overridden.clone()],
        "the override shadows the inherited row — no duplicate"
    );

    // One undo removes the materialized row; the inherited row
    // returns unshadowed.
    h.key_press_modifiers(egui::Modifiers::CTRL, egui::Key::Z);
    h.run();
    assert!(
        h.state().debug_instance_hyperlinks_for_test(sid).is_empty(),
        "one undo removed the materialized row"
    );
    assert_eq!(
        h.state().debug_hyperlinks_for_test(sid),
        vec![inherited.clone()],
        "the inherited row is back and the master untouched"
    );

    // Clearing an inherited field sticks: the materialized row is
    // seeded with the master's cells, so a skipped write would
    // silently keep the inherited description (#139 review).
    h.get_by_label("Edit").click();
    h.run();
    h.get(
        egui_kittest::kittest::By::default()
            .role(egui::Role::TextInput)
            .label("Description"),
    )
    .set_text_value_accesskit("");
    h.run();
    h.get_by_label("OK").click();
    h.run();
    assert_eq!(
        h.state().debug_instance_hyperlinks_for_test(sid),
        vec![(
            1,
            "https://master.example/doc".into(),
            String::new(),
            String::new()
        )],
        "the cleared description stayed cleared (address seeded from the master)"
    );
    assert_eq!(
        h.state().debug_master_hyperlinks_for_test(sid),
        vec![inherited.clone()],
        "the master is still untouched"
    );
}

/// GitHub #29: Add Comment creates an annotation shape on the
/// Annotation layer; the Review toggle hides it; undo removes it.
#[test]
fn add_comment_creates_layer_annotation_and_toggles() {
    use egui_kittest::kittest::Queryable;
    let mut h = harness_with_blank();
    h.run();
    let from = h.state().debug_master_cells()[0].1.center();
    let to = h.state().debug_canvas_rect().center();
    click_at(&mut h, from, egui::PointerButton::Primary);
    click_at(&mut h, to, egui::PointerButton::Primary);
    assert_eq!(h.state().debug_shape_count(), 1);
    // Right-click → Add Comment → type → OK.
    click_at(&mut h, to, egui::PointerButton::Secondary);
    h.run();
    h.get_all_by_label_contains("Add Comment")
        .filter(|n| n.rect().height() < 30.0)
        .find(|n| n.rect().min.y > 250.0)
        .expect("Add Comment row")
        .click();
    h.run();
    // The popup's input is labeled "New comment" for an empty thread.
    h.get(
        egui_kittest::kittest::By::default()
            .role(egui::Role::TextInput)
            .label("New comment"),
    )
    .set_text_value_accesskit("needs review");
    h.run();
    h.get_by_label("OK").click();
    h.run();

    let ann = h.state().debug_annotations_for_test();
    assert_eq!(ann.len(), 1, "one annotation on the Annotation layer");
    assert!(
        ann[0].1.contains("needs review"),
        "author/date/body in the text block: {:?}",
        ann[0].1
    );

    // The Review toggle (View tab → Comments) hides it. The SVG wraps
    // text into per-line <tspan>s, and where the wrap falls depends on
    // the author name (runneradmin on Windows CI vs a short $USER
    // locally) — join the tspan lines before asserting on the phrase.
    let svg_with = h.state().debug_page_svg().unwrap_or_default();
    let joined: String = svg_with
        .split("<tspan")
        .skip(1)
        .filter_map(|seg| seg.split_once('>'))
        .filter_map(|(_, rest)| rest.split_once("</tspan>"))
        .map(|(t, _)| t)
        .collect::<Vec<_>>()
        .join(" ");
    assert!(
        joined.contains("needs review"),
        "comment text renders (wrapped): {joined:?}"
    );
    // The popup WINDOW is also titled "Comments" — the ribbon toggle
    // is the tall hero button.
    // Hide comments directly — the ribbon toggle rides the standard
    // toggle dispatch (the popup overlays the ribbon in this harness).
    h.state_mut().set_show_comments_for_test(false);
    h.run();
    assert!(!h.state().show_comments_for_test());

    // Undo removes the annotation (+ its layer).
    h.get_by_label("Home").click();
    h.run();
    h.key_press_modifiers(egui::Modifiers::CTRL, egui::Key::Z);
    h.run();
    assert_eq!(
        h.state().debug_annotations_for_test().len(),
        0,
        "undo removed the comment"
    );
}

/// GitHub #30: Shape Data… edits Prop rows through the command layer.
#[test]
fn shape_data_dialog_edits_prop_rows() {
    use egui_kittest::kittest::Queryable;
    let mut h = harness_with_blank();
    h.run();
    let from = h.state().debug_master_cells()[0].1.center();
    let to = h.state().debug_canvas_rect().center();
    click_at(&mut h, from, egui::PointerButton::Primary);
    click_at(&mut h, to, egui::PointerButton::Primary);
    let (_, sid) = h.state().debug_sel().expect("placed shape selected");
    assert_eq!(
        h.state().debug_prop_value_for_test(sid, "Department"),
        Some("Engineering".into()),
        "master Prop row resolves"
    );

    // Open Shape Data… — the dialog lists Department (a list field)
    // with its current choice; pick another from the combo.
    click_at(&mut h, to, egui::PointerButton::Secondary);
    h.run();
    h.get_all_by_label_contains("Shape Data")
        .filter(|n| n.rect().height() < 30.0)
        .find(|n| n.rect().min.y > 250.0)
        .expect("Shape Data… row")
        .click();
    h.run();
    // The combo is labeled by its field name; open it and pick
    // "Sales".
    // The grid label "Department" and the combo share the name — take
    // the clickable one (the combo on the value column).
    let combo = h
        .get_all_by_label("Department")
        .filter(|n| n.rect().min.x > 90.0)
        .collect::<Vec<_>>();
    combo[combo.len() - 1].click();
    h.run();
    h.get_by_label("Sales").click();
    h.run();
    assert_eq!(
        h.state().debug_prop_value_for_test(sid, "Department"),
        Some("Sales".into()),
        "the combo wrote the row's Value"
    );
    // One Ctrl+Z restores the previous value.
    h.key_press_modifiers(egui::Modifiers::CTRL, egui::Key::Z);
    h.run();
    assert_eq!(
        h.state().debug_prop_value_for_test(sid, "Department"),
        Some("Engineering".into()),
        "undo restored the Prop value"
    );
}

/// GitHub #30: a variable-list (Type 4) Prop field can grow — the "＋"
/// reveals an add row whose commit writes the row's Value AND appends
/// the choice to Format, as one undo step. Fixed lists (Type 1) keep
/// the plain dropdown.
#[test]
fn shape_data_variable_list_adds_choice() {
    use egui_kittest::kittest::Queryable;
    let mut h = harness_with_blank();
    h.run();
    let from = h.state().debug_master_cells()[0].1.center();
    let to = h.state().debug_canvas_rect().center();
    click_at(&mut h, from, egui::PointerButton::Primary);
    click_at(&mut h, to, egui::PointerButton::Primary);
    let (_, sid) = h.state().debug_sel().expect("placed shape selected");
    h.state_mut()
        .debug_add_prop_row_for_test(sid, "Shift", "4", "Day;Night", "Day");
    h.run();

    // Open Shape Data… — the variable-list field shows the "＋".
    click_at(&mut h, to, egui::PointerButton::Secondary);
    h.run();
    h.get_all_by_label_contains("Shape Data")
        .filter(|n| n.rect().height() < 30.0)
        .find(|n| n.rect().min.y > 250.0)
        .expect("Shape Data… row")
        .click();
    h.run();
    // The dialog's "＋" — the status bar carries another "+"-labeled
    // control, so take the upper one inside the Shape Data window.
    h.get_all_by_label("+")
        .find(|n| n.rect().min.y < 500.0)
        .expect("variable-list add button")
        .click();
    h.run();
    // Type the new choice and commit.
    h.get(
        egui_kittest::kittest::By::default()
            .role(egui::Role::TextInput)
            .label("Shift"),
    )
    .set_text_value_accesskit("Swing");
    h.run();
    h.get_by_label("Add").click();
    h.run();
    assert_eq!(
        h.state().debug_prop_cell_for_test(sid, "Shift", "Value"),
        Some("Swing".into()),
        "adding a choice writes the Value"
    );
    assert_eq!(
        h.state().debug_prop_cell_for_test(sid, "Shift", "Format"),
        Some("Day;Night;Swing".into()),
        "adding a choice appends to Format"
    );
    // The add row closed after committing (its Add button is gone).
    assert!(
        h.query_all_by_label("Add").count() == 0,
        "the add row closed after committing"
    );
    // One Ctrl+Z rolls back both writes.
    h.key_press_modifiers(egui::Modifiers::CTRL, egui::Key::Z);
    h.run();
    assert_eq!(
        h.state().debug_prop_cell_for_test(sid, "Shift", "Value"),
        Some("Day".into()),
        "undo restored the previous value"
    );
    assert_eq!(
        h.state().debug_prop_cell_for_test(sid, "Shift", "Format"),
        Some("Day;Night".into()),
        "undo restored the previous format"
    );

    // An EXISTING choice selects instead of duplicating in Format.
    let open_add = |h: &mut Harness<'_, App>| {
        h.get_all_by_label("+")
            .find(|n| n.rect().min.y < 500.0)
            .expect("variable-list add button")
            .click();
        h.run();
    };
    let set_choice = |h: &mut Harness<'_, App>, text: &str| {
        h.get(
            egui_kittest::kittest::By::default()
                .role(egui::Role::TextInput)
                .label("Shift"),
        )
        .set_text_value_accesskit(text);
        h.run();
        h.get_by_label("Add").click();
        h.run();
    };
    open_add(&mut h);
    set_choice(&mut h, "Night");
    assert_eq!(
        h.state().debug_prop_cell_for_test(sid, "Shift", "Value"),
        Some("Night".into()),
        "an existing choice selects"
    );
    assert_eq!(
        h.state().debug_prop_cell_for_test(sid, "Shift", "Format"),
        Some("Day;Night".into()),
        "an existing choice does not duplicate in Format"
    );

    // A choice containing ';' is refused with a status note; the add
    // row stays open and nothing is written.
    open_add(&mut h);
    set_choice(&mut h, "a;b");
    assert_eq!(
        h.state().debug_prop_cell_for_test(sid, "Shift", "Value"),
        Some("Night".into()),
        "a ';' choice writes nothing"
    );
    assert_eq!(
        h.state().debug_prop_cell_for_test(sid, "Shift", "Format"),
        Some("Day;Night".into()),
        "a ';' choice leaves Format alone"
    );
    assert!(
        h.query_all_by_label("Add").count() > 0,
        "the add row stayed open for correction"
    );
    h.get_by_label("Cancel").click();
    h.run();
}

/// GitHub #31: Format Shape pane writes the renderer's cells; every
/// change is one undo step; multi-selection applies to all members.
#[test]
fn format_pane_edits_cells_for_whole_selection() {
    use egui_kittest::kittest::Queryable;
    let mut h = harness_with_blank();
    h.run();
    // Two shapes for the multi-selection leg.
    let cells = h.state().debug_master_cells();
    let from = cells[0].1.center();
    let canvas = h.state().debug_canvas_rect();
    for off in [-40.0f32, 40.0f32] {
        let to = canvas.center() + egui::vec2(off, -off);
        click_at(&mut h, from, egui::PointerButton::Primary);
        click_at(&mut h, to, egui::PointerButton::Primary);
    }
    h.run();
    let (_, sid) = h.state().debug_sel().expect("second placed shape selected");

    // Right-click → Format Shape….
    click_at(
        &mut h,
        canvas.center() + egui::vec2(40.0, -40.0),
        egui::PointerButton::Secondary,
    );
    h.run();
    let menu = h.state().debug_ctx_menu_rect().expect("context menu open");
    h.get_all_by_label_contains("Format Shape")
        .filter(|n| menu.expand(2.0).contains_rect(n.rect()))
        .min_by_key(|n| n.rect().min.y as i64)
        .expect("Format Shape… row")
        .click();
    h.run();
    assert!(
        h.query_all_by_label_contains("Format Shape").count() > 0,
        "the pane opened"
    );

    // Line weight: the slider writes LineWeight (0.75pt default → drag
    // is awkward headless; use the Fill pattern dropdown instead for a
    // precise assert, then weight via its slider API).
    // FillPattern: Solid → None (0).
    // The FillPattern dropdown button reads "Solid  ›" (a pane label
    // "Solid" alone would be ambiguous) — take the button on the pane's
    // right column.
    // Scope to the pane via its title node's relative geometry (the
    // title strip itself does not contain the dropdown; #97 section 4
    // replaces the old absolute min.x > 1050 band).
    let pane = h.get_by_label_contains("Format Shape").rect();
    h.get_all_by_label_contains("Solid")
        .filter(|n| {
            let r = n.rect();
            r.min.x >= pane.min.x - 20.0
                && r.max.x <= pane.max.x + 20.0
                && r.min.y > pane.max.y
                && r.min.y < pane.max.y + 300.0
        })
        .min_by_key(|n| n.rect().min.y as i64)
        .expect("FillPattern dropdown")
        .click();
    h.run();
    h.get_by_label("None").click();
    h.run();
    assert_eq!(
        h.state().debug_cell_for_test(sid, "FillPattern"),
        Some("0".into()),
        "FillPattern wrote through"
    );
    h.key_press_modifiers(egui::Modifiers::CTRL, egui::Key::Z);
    h.run();
    assert_eq!(
        h.state().debug_cell_for_test(sid, "FillPattern"),
        Some("1".into()),
        "one undo restored Solid"
    );

    // Since #136 the undo keeps the selection (and with it the pane) —
    // this leg used to reselect the shape by clicking its start-of-test
    // screen position, which only worked while the closed pane kept the
    // canvas at full width. The Text section leg goes straight on.
    assert_eq!(
        h.state().debug_sel().map(|(_, s)| s),
        Some(sid),
        "#136: undo keeps the formatted shape selected"
    );
    h.get_by_label("Bold").click();
    h.run();
    assert_eq!(
        h.state().debug_cell_for_test(sid, "Char.Style"),
        Some("1".into()),
        "bold wrote Character Style"
    );

    // #73: the Text section's Color button opens the palette; a pick
    // writes the Character row-0 Color cell through the same
    // whole-selection macro (first-edit materialization), the canvas
    // renders it (WYSIWYG by construction — emit_text reads the
    // cell), and one undo removes the materialized row.
    h.key_press(egui::Key::F2);
    h.run();
    h.get_by_role(egui::Role::MultilineTextInput)
        .set_text_value_accesskit("Red");
    h.run();
    let off = h.state().debug_canvas_rect().right_bottom() - egui::vec2(10.0, 10.0);
    click_at(&mut h, off, egui::PointerButton::Primary);
    h.run();
    assert_eq!(
        h.state().debug_shape_text(sid).as_deref(),
        Some("Red"),
        "the shape carries text for the color to render on"
    );

    // The pane carries two "Color" rows (Line above, Text below); the
    // labels are static text — the BUTTON beside the label carries
    // the value. Take the lowest "Color" label (Text's) and click the
    // button in its row.
    let color_label = h
        .get_all_by_label("Color")
        .map(|n| n.rect())
        .max_by_key(|r| r.min.y as i64)
        .expect("the Text section's Color label");
    let text_color_btn = h
        .query_all_by_role(egui::Role::Button)
        .map(|n| n.rect())
        .find(|r| {
            r.min.x > color_label.right()
                && r.center().y > color_label.min.y - 4.0
                && r.center().y < color_label.max.y + 4.0
        })
        .expect("the Color value button beside its label");
    click_at(&mut h, text_color_btn.center(), egui::PointerButton::Primary);
    h.run();
    h.get_by_label("#FFC000").click();
    h.run();
    assert_eq!(
        h.state().debug_cell_for_test(sid, "Char.Color"),
        Some("#FFC000".into()),
        "the palette wrote the Character row-0 Color cell"
    );
    assert!(
        h.state()
            .debug_page_svg()
            .unwrap_or_default()
            .contains("fill=\"#FFC000\""),
        "the text renders in the picked color"
    );
    h.key_press_modifiers(egui::Modifiers::CTRL, egui::Key::Z);
    h.run();
    assert_eq!(
        h.state().debug_cell_for_test(sid, "Char.Color"),
        None,
        "one undo removed the materialized Color cell"
    );

    // #72: the Line section's Begin/End Arrow pickers write through,
    // the canvas renders the head, and one Ctrl+Z restores.
    // (The placed Rectangle is 2-D, but Arrow cells ride any line
    // geometry; assert the write + the marker in the page SVG.)
    let end_cur = h
        .state()
        .debug_cell_for_test(sid, "EndArrow")
        .unwrap_or_else(|| "0".into());
    assert_eq!(end_cur, "0", "no arrow yet");
    // The End Arrow dropdown: the fmt_dropdown button shows the
    // current name ("None") in the pane's Line section.
    // Begin and End both show "None  ›" — the End row sits BELOW the
    // Begin row, so take the lowest pane dropdown.
    let btn = h
        .get_all_by_label_contains("None")
        .filter(|n| {
            let r = n.rect();
            r.min.x > 900.0 && r.width() < 120.0
        })
        .max_by_key(|n| n.rect().min.y as i64)
        .expect("the End Arrow dropdown (shows None)");
    btn.click();
    h.run();
    h.get_by_label("Arrow").click();
    h.run();
    assert_eq!(
        h.state().debug_cell_for_test(sid, "EndArrow"),
        Some("1".into()),
        "the picker wrote the cell through the whole-selection macro"
    );
    assert!(
        h.state()
            .debug_page_svg()
            .unwrap_or_default()
            .contains("dc-ahek"),
        "the canvas renders the arrowhead marker"
    );
    h.key_press_modifiers(egui::Modifiers::CTRL, egui::Key::Z);
    h.run();
    assert_eq!(
        h.state().debug_cell_for_test(sid, "EndArrow"),
        Some("0".into()),
        "one undo restored the arrow state (the master snapshot's 0)"
    );
}

/// GitHub #29 rework: the badge indicates a comment; the popup edits,
/// replies, and deletes — one undo per action.
#[test]
fn comment_badge_opens_thread_with_edit_reply_delete() {
    use egui_kittest::kittest::Queryable;
    let mut h = harness_with_blank();
    h.run();
    let from = h.state().debug_master_cells()[0].1.center();
    let to = h.state().debug_canvas_rect().center();
    click_at(&mut h, from, egui::PointerButton::Primary);
    click_at(&mut h, to, egui::PointerButton::Primary);
    // First comment through the context menu.
    click_at(&mut h, to, egui::PointerButton::Secondary);
    h.run();
    h.get_all_by_label_contains("Add Comment")
        .filter(|n| n.rect().height() < 30.0)
        .find(|n| n.rect().min.y > 250.0)
        .expect("Add Comment row")
        .click();
    h.run();
    h.get(
        egui_kittest::kittest::By::default()
            .role(egui::Role::TextInput)
            .label("New comment"),
    )
    .set_text_value_accesskit("first");
    h.run();
    h.get_by_label("OK").click();
    h.run();
    assert_eq!(h.state().debug_annotations_for_test().len(), 1);

    // The badge shows on the shape; clicking it opens the thread.
    h.get_by_label_contains("Comments on shape").click();
    h.run();
    assert!(
        h.query_all_by_label_contains("first").count() > 0,
        "the thread card shows the comment"
    );

    // Reply: a second annotation stacks under the first.
    h.get(
        egui_kittest::kittest::By::default()
            .role(egui::Role::TextInput)
            .label("Reply"),
    )
    .set_text_value_accesskit("second");
    h.run();
    // "Reply" matches the input AND the button — take the button.
    h.get(
        egui_kittest::kittest::By::default()
            .role(egui::Role::Button)
            .label("Reply"),
    )
    .click();
    h.run();
    let ann = h.state().debug_annotations_for_test();
    assert_eq!(ann.len(), 2, "reply appended to the thread");
    assert!(ann.iter().any(|(_, t)| t.contains("second")));

    // Edit the first comment: its card's Edit → buffer → Save writes
    // SetText (author/date preserved).
    h.get_all_by_label("Edit")
        .into_iter()
        .find(|n| n.rect().min.y < 500.0)
        .expect("Edit button")
        .click();
    h.run();
    h.get(
        egui_kittest::kittest::By::default()
            .role(egui::Role::TextInput)
            .label("Edit comment"),
    )
    .set_text_value_accesskit("first (edited)");
    h.run();
    // The QAT also has a Save — take the LOWEST match (the popup
    // card sits well below the title band).
    let saves = h
        .get_all_by_label("Save")
        .map(|n| n.rect())
        .collect::<Vec<_>>();
    let save_rect = *saves.last().expect("popup Save");
    click_at(&mut h, save_rect.center(), egui::PointerButton::Primary);
    h.run();
    let ann = h.state().debug_annotations_for_test();
    assert!(
        ann.iter().any(|(_, t)| t.contains("first (edited)")),
        "edit rewrote the body: {ann:?}"
    );

    // Delete the SECOND card's comment (the lowest Delete button).
    // The ribbon Home tab also has a Delete — take the popup card's
    // (the lowest match).
    let del_rects = h
        .get_all_by_label("Delete")
        .map(|n| n.rect())
        .collect::<Vec<_>>();
    click_at(
        &mut h,
        del_rects.last().unwrap().center(),
        egui::PointerButton::Primary,
    );
    h.run();
    let ann = h.state().debug_annotations_for_test();
    assert_eq!(ann.len(), 1, "delete removed one comment");
    assert!(ann[0].1.contains("first (edited)"));

    // Close actually closes (live report: it didn't): the popup's
    // labeled inputs disappear with it (the stencil search remains).
    h.get_by_label("Close").click();
    h.run();
    let reply_inputs = h.query_all_by_label_contains("Reply").count();
    assert_eq!(reply_inputs, 0, "the Comments popup closed on Close");
}

/// Live report follow-up (#28): the Hyperlinks window must have a
/// cancel mechanism — Close (and the title-bar X) dismiss it and
/// discard any draft.
#[test]
fn hyperlink_dialog_close_dismisses_and_discards_draft() {
    use egui_kittest::kittest::Queryable;
    let mut h = harness_with_blank();
    h.run();
    let from = h.state().debug_master_cells()[0].1.center();
    let to = h.state().debug_canvas_rect().center();
    click_at(&mut h, from, egui::PointerButton::Primary);
    click_at(&mut h, to, egui::PointerButton::Primary);
    let (_, sid) = h.state().debug_sel().expect("placed shape selected");

    click_at(&mut h, to, egui::PointerButton::Secondary);
    h.run();
    let menu = h.state().debug_ctx_menu_rect().expect("context menu open");
    h.get_all_by_label_contains("Hyperlink")
        .filter(|n| menu.expand(2.0).contains_rect(n.rect()))
        .min_by_key(|n| n.rect().min.y as i64)
        .expect("Hyperlink… row")
        .click();
    h.run();
    // Start a draft, then cancel the whole window: no row lands.
    h.get_by_label("New…").click();
    h.run();
    h.get(
        egui_kittest::kittest::By::default()
            .role(egui::Role::TextInput)
            .label("Address"),
    )
    .set_text_value_accesskit("https://example.test/cancelled");
    h.run();
    // The window's Close is ALWAYS visible — even with a draft open
    // (elementary/Apple HIG: the dismiss control is obvious).
    h.get_by_label("Close").click();
    h.run();
    assert_eq!(
        h.query_all_by_label_contains("New…").count(),
        0,
        "the Hyperlinks window closed on Close"
    );
    assert!(
        h.state().debug_hyperlinks_for_test(sid).is_empty(),
        "the discarded draft wrote nothing"
    );
}

/// Live report on #30: the Shape Data window must be closable — Close
/// (always visible) and the title-bar X dismiss it.
#[test]
fn shape_data_dialog_close_dismisses() {
    use egui_kittest::kittest::Queryable;
    let mut h = harness_with_blank();
    h.run();
    let from = h.state().debug_master_cells()[0].1.center();
    let to = h.state().debug_canvas_rect().center();
    click_at(&mut h, from, egui::PointerButton::Primary);
    click_at(&mut h, to, egui::PointerButton::Primary);
    click_at(&mut h, to, egui::PointerButton::Secondary);
    h.run();
    let menu = h.state().debug_ctx_menu_rect().expect("context menu open");
    h.get_all_by_label_contains("Shape Data")
        .filter(|n| menu.expand(2.0).contains_rect(n.rect()))
        .min_by_key(|n| n.rect().min.y as i64)
        .expect("Shape Data… row")
        .click();
    h.run();
    assert!(
        h.query_all_by_label_contains("Department").count() > 0,
        "the Shape Data window opened"
    );
    h.get_by_label("Close").click();
    h.run();
    assert_eq!(
        h.query_all_by_label_contains("Department").count(),
        0,
        "the Shape Data window closed on Close"
    );
}

// ------------------------------------------------------------------
// GitHub #96: live drag feedback.

/// During a move drag the shape itself follows the cursor — live
/// layer ghosts, never a re-raster — replacing the old dashed-only
/// preview (which drew at inch coordinates as pixels and was
/// invisible). The ghost must disappear when the release commits the
/// move exactly where the ghost showed.
#[test]
fn drag_moves_the_shape_live_under_the_cursor() {
    let mut h = harness_with_blank();
    h.run();
    let (master, _) = h.state().debug_master_cells()[0];
    let (page, shape) = place_master(&mut h, master.1);
    h.run();
    // Select the placed shape (placement may not carry selection).
    let r = h.state().debug_shape_screen_rect(shape).expect("placed");
    click_at(&mut h, r.center(), egui::PointerButton::Primary);
    assert_eq!(h.state().debug_sel(), Some((page, shape)));
    let from = r.center();
    let b = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    h.hover_at(from);
    h.event(b(from, true));
    h.run(); // press frame arms the drag
    h.hover_at(from + egui::vec2(60.0, -40.0));
    h.run(); // mid-gesture frame
    assert_eq!(
        h.state().debug_drag_ghosts_for_test(),
        1,
        "the dragged shape must ghost live under the cursor (#96)"
    );
    ctx_menu_snapshot(&mut h, "dc_ui_drag_live_feedback");
    h.event(b(from + egui::vec2(60.0, -40.0), false));
    h.run();
    assert_eq!(
        h.state().debug_drag_ghosts_for_test(),
        0,
        "ghosts vanish once the release commits"
    );
    let r2 = h.state().debug_shape_screen_rect(shape).expect("after move");
    assert!(
        (r2.center().x - (from.x + 60.0)).abs() <= 1.5
            && (r2.center().y - (from.y - 40.0)).abs() <= 1.5,
        "the shape must land where the ghost showed: {:?} vs {:?}",
        r2.center(),
        from + egui::vec2(60.0, -40.0)
    );
}

// ---------------------------------------------------------------------------
// Multiple open stencils (GitHub #63 sub-issue 1 / #82)
// ---------------------------------------------------------------------------

/// A synthetic standalone stencil (.vssx) written from an in-code
/// model — DiagramCraft's own fixture, no Microsoft content. Master
/// ids are caller-chosen so tests can make two stencils collide.
/// Pump the harness until the dialog worker's answer lands in the
/// stencil pane (a FIXED frame count races the worker thread on slow
/// CI runners — the Windows job of PR #159 saw 8 frames not suffice);
/// bounded, and yields a wall-clock slice periodically so the worker
/// can actually finish.
fn wait_for_stencil_entry(h: &mut egui_kittest::Harness<'_, App>) {
    let mut waited = 0;
    while h.state().debug_stencil_entries().is_empty() {
        waited += 1;
        assert!(waited < 20_000, "dialog worker never answered");
        h.run();
        if waited % 500 == 0 {
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }
}

fn synth_stencil(dir: &std::path::Path, stem: &str, masters: &[(&str, i64)]) -> std::path::PathBuf {
    synth_stencil_opts(dir, stem, masters, &[])
}

/// [`synth_stencil`] with extra `Hidden='1'` masters (#88: Quick
/// Shapes excludes them; the pane entry itself still lists them).
fn synth_stencil_opts(
    dir: &std::path::Path,
    stem: &str,
    masters: &[(&str, i64)],
    hidden: &[(&str, i64)],
) -> std::path::PathBuf {
    use vsdx::model::*;
    let xmlns = || {
        vec![
            (
                "xmlns".to_string(),
                "http://schemas.microsoft.com/office/visio/2012/main".to_string(),
            ),
            (
                "xmlns:r".to_string(),
                "http://schemas.openxmlformats.org/officeDocument/2006/relationships"
                    .to_string(),
            ),
        ]
    };
    let mut doc = vsdx::Document {
        root_attrs: xmlns(),
        pages_attrs: xmlns(),
        masters_attrs: xmlns(),
        doc_part: "visio/document.xml".into(),
        pages_part: "visio/pages/pages.xml".into(),
        items: vec![DocItem::StyleSheets],
        ..Document::default()
    };
    let ss_attrs = vec![("ID".into(), "0".into()), ("NameU".into(), "No Style".into())];
    doc.stylesheets.push(StyleSheet {
        id: 0,
        name_u: Some("No Style".into()),
        attrs: ss_attrs.clone(),
        sheet: Sheet {
            attrs: ss_attrs,
            ..Sheet::default()
        },
    });
    let all: Vec<(&str, i64, bool)> = masters
        .iter()
        .map(|(n, i)| (*n, *i, false))
        .chain(hidden.iter().map(|(n, i)| (*n, *i, true)))
        .collect();
    for (i, (name, id, is_hidden)) in all.iter().enumerate() {
        let mut sheet = Sheet::default();
        for (n, v) in [("PinX", "1"), ("PinY", "1"), ("Width", "1"), ("Height", "1")] {
            sheet.set_cell(n, Cell::value(v));
        }
        let line = |t: &str, ix: i64, x: &str, y: &str| Row {
            t: Some(t.into()),
            ix,
            n: None,
            attrs: vec![("T".into(), t.into()), ("IX".into(), ix.to_string())],
            cells: vec![("X".into(), Cell::value(x)), ("Y".into(), Cell::value(y))],
        };
        sheet.children.push(SheetChild::Section(Section {
            name: "Geometry".into(),
            ix: 0,
            attrs: vec![("N".into(), "Geometry".into()), ("IX".into(), "0".into())],
            cells: vec![],
            rows: vec![
                line("MoveTo", 1, "0", "0"),
                line("LineTo", 2, "1", "0"),
                line("LineTo", 3, "1", "1"),
                line("LineTo", 4, "0", "1"),
                line("LineTo", 5, "0", "0"),
            ],
        }));
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
            sheet,
            ..Shape::default()
        };
        let mut attrs = vec![
            ("ID".into(), id.to_string()),
            ("NameU".into(), name.to_string()),
            ("Name".into(), name.to_string()),
            ("BaseID".into(), format!("{{SYNTH-{stem}-{id}}}")),
        ];
        if *is_hidden {
            attrs.push(("Hidden".into(), "1".into()));
        }
        doc.masters.push(Master {
            id: *id,
            name_u: Some(name.to_string()),
            attrs,
            shapes: vec![shape],
            part: format!("visio/masters/master{}.xml", i + 1),
            contents_attrs: xmlns(),
            ..Master::default()
        });
    }
    let mut page_sheet = Sheet::default();
    page_sheet.set_cell("PageWidth", Cell::value("8.5"));
    page_sheet.set_cell("PageHeight", Cell::value("11"));
    doc.pages.push(Page {
        id: 0,
        name: "Page-1".into(),
        attrs: vec![("ID".into(), "0".into()), ("NameU".into(), "Page-1".into())],
        children: vec![IndexChild::Sheet {
            tag: "PageSheet".into(),
            sheet: page_sheet,
        }],
        part: "visio/pages/page1.xml".into(),
        contents_attrs: xmlns(),
        ..Page::default()
    });
    let path = dir.join(format!("{stem}.vssx"));
    vsdx::write::save(&doc, &path).unwrap();
    path
}

#[test]
fn stencil_pane_lists_multiple_open_stencils_independently() {
    use dc_ui::stencil::StencilId;
    let mut h = harness_with_blank();
    h.run();
    let doc_cells = h.state().debug_master_cells().len();
    assert!(doc_cells > 0, "blank template exposes starter masters");
    let tmp = tempfile::tempdir().unwrap();
    // Two synthetic stencils whose master ids deliberately collide —
    // pane identity must be (stencil, master), never a bare id (#82).
    let a = synth_stencil(tmp.path(), "alpha", &[("Square", 1), ("Circle", 2)]);
    let b = synth_stencil(tmp.path(), "beta", &[("Star", 1), ("Arrow", 2)]);
    h.state_mut().open_stencil_for_test(&a);
    h.state_mut().open_stencil_for_test(&b);
    h.run();

    let entries = h.state().debug_stencil_entries();
    assert_eq!(entries.len(), 2, "two open stencils listed");
    assert_eq!(entries[0], ("alpha".to_string(), true), "opened expanded");
    assert_eq!(entries[1], ("beta".to_string(), true), "second entry below the first");

    let count = |h: &Harness<'_, App>, which: StencilId| {
        h.state()
            .debug_master_cells()
            .iter()
            .filter(|(k, _)| k.0 == which)
            .count()
    };
    // Quick Shapes (#88) leads the pane and — with the document entry
    // unpinned (#102) — is the only place the document's masters
    // appear: its head masters, once. Each synth stencil is under the
    // per-source cap, so its whole set joins quick as well as its
    // own entry.
    assert_eq!(
        count(&h, StencilId::Document),
        6,
        "doc head masters in Quick Shapes only (no pinned document entry, #102)"
    );
    assert_eq!(count(&h, StencilId::Open(0)), 4, "alpha twice (quick + entry)");
    assert_eq!(count(&h, StencilId::Open(1)), 4, "beta twice (quick + entry)");

    // Collapse the first open stencil via its entry header: only its
    // grid disappears; the document entry and the other stencil stay.
    let header = h
        .state()
        .debug_stencil_entry_rects()
        .into_iter()
        .find(|(id, _)| *id == StencilId::Open(0))
        .expect("alpha header laid out")
        .1;
    click_at(&mut h, header.center(), egui::PointerButton::Primary);
    let entries = h.state().debug_stencil_entries();
    assert!(!entries[0].1, "alpha collapsed by its own header");
    assert!(entries[1].1, "beta's expansion is independent");
    assert_eq!(
        count(&h, StencilId::Open(0)),
        2,
        "collapsed entry hides its grid (Quick Shapes still shows its heads)"
    );
    assert_eq!(count(&h, StencilId::Open(1)), 4);
    assert_eq!(
        count(&h, StencilId::Document),
        6,
        "document head masters untouched by a stencil collapse"
    );

    // Re-expand, then a canvas interaction leaves the entries
    // unchanged. The collapse re-wrapped the pane (the scrollbar
    // appeared, the doc grid re-wrapped one row longer), so the
    // header moved — always click the FRESH rect.
    let header2 = h
        .state()
        .debug_stencil_entry_rects()
        .into_iter()
        .find(|(id, _)| *id == StencilId::Open(0))
        .expect("alpha header laid out")
        .1;
    click_at(&mut h, header2.center(), egui::PointerButton::Primary);
    let canvas_center = h.state().debug_canvas_rect().center();
    click_at(&mut h, canvas_center, egui::PointerButton::Primary);
    assert_eq!(
        count(&h, StencilId::Open(0)),
        4,
        "entries survive a canvas interaction unchanged"
    );
}

// ---------------------------------------------------------------------------
// Opening .vssx stencils into the pane (GitHub #83, #63 sub-issue 2)
// ---------------------------------------------------------------------------

#[test]
fn more_shapes_menu_opens_a_stencil_into_the_pane() {
    use egui_kittest::kittest::Queryable;
    let mut h = harness_with_blank();
    h.run();
    let tmp = tempfile::tempdir().unwrap();
    let s = synth_stencil(tmp.path(), "gamma", &[("Box", 3)]);
    // The dialog-worker test seam answers the native dialog (never
    // opens rfd under kittest).
    h.state_mut().preset_save_as_for_test(Some(s.clone()));
    h.get_by_label("More Shapes").click();
    h.run();
    h.get_by_label("Open Existing Stencil...").click();
    h.run();
    wait_for_stencil_entry(&mut h);
    let entries = h.state().debug_stencil_entries();
    assert_eq!(entries, vec![("gamma".to_string(), true)], "stencil opened expanded");
    assert!(
        h.state()
            .debug_master_cells()
            .iter()
            .any(|(k, _)| matches!(k.0, dc_ui::stencil::StencilId::Open(0))),
        "its masters are listed"
    );
    assert!(h.state().debug_status().contains("opened stencil"));

    // Re-opening the same path dedupes: expands the entry, adds nothing.
    h.state_mut().open_stencil_for_test(&s);
    let entries = h.state().debug_stencil_entries();
    assert_eq!(entries.len(), 1, "re-open dedupes by source path");
}

#[test]
fn file_open_of_a_stencil_lands_in_the_pane_not_the_editor() {
    let mut h = harness_with_blank();
    h.run();
    let shapes_before = h.state().debug_shape_count();
    let tmp = tempfile::tempdir().unwrap();
    let s = synth_stencil(tmp.path(), "delta", &[("Blob", 5)]);
    // File→Open through the real dialog-worker seam, preset-answered.
    h.state_mut().open_file_for_test(s);
    h.run();
    wait_for_stencil_entry(&mut h);
    assert_eq!(
        h.state().debug_stencil_entries().len(),
        1,
        "the .vssx opened into the Shapes pane"
    );
    assert!(
        h.state().debug_master_cells().iter().any(|(k, _)| matches!(k.0, dc_ui::stencil::StencilId::Open(0))),
        "its masters are listed"
    );
    assert_eq!(
        h.state().debug_shape_count(),
        shapes_before,
        "the edited document is untouched by a stencil open"
    );
    assert!(
        h.state().debug_status().contains("opened stencil"),
        "status notes the stencil open: {}",
        h.state().debug_status()
    );
}

/// Dropping a master from an open stencil copies it into the drawing
/// and places the instance in one undo step (#84).
#[test]
fn stencil_master_drop_imports_and_places_as_one_undo() {
    use dc_ui::stencil::StencilId;
    let mut h = harness_with_blank();
    h.run();
    let masters_before = h.state().debug_doc_masters().len();
    let tmp = tempfile::tempdir().unwrap();
    // The stencil master's id (1) collides with nothing here, but its
    // BaseID identity is what the import dedupes on.
    let s = synth_stencil(tmp.path(), "kit", &[("Kite", 1)]);
    h.state_mut().open_stencil_for_test(&s);
    h.run();
    // The open-reveal scrolled the pane; come back to the top so the
    // Quick Shapes copy of the cell is fully in the viewport (its
    // clipped center can sit under the search box otherwise). Dragging
    // from the Quick Shapes copy exercises #88's promise: placing from
    // Quick Shapes imports exactly like the source entry.
    h.state_mut().scroll_pane_to_top_for_test();
    h.run();
    let from = h
        .state()
        .debug_master_cells()
        .into_iter()
        .find(|(k, _)| *k == (StencilId::Open(0), 1))
        .expect("kite master cell")
        .1
        .center();
    let to = h.state().debug_canvas_rect().center();

    // The drag-drop paradigm: press on the cell, cross the canvas,
    // release.
    h.hover_at(from);
    h.event(egui::Event::PointerButton {
        pos: from,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
    h.hover_at(from + (to - from) * 0.5);
    h.run();
    h.hover_at(to);
    h.run();
    h.event(egui::Event::PointerButton {
        pos: to,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    h.run();

    assert_eq!(h.state().debug_shape_count(), 1, "the drop placed a shape");
    let masters = h.state().debug_doc_masters();
    assert_eq!(
        masters.len(),
        masters_before + 1,
        "first use imported a copy into the drawing"
    );
    let imported_id = masters.last().unwrap().0;
    assert_eq!(
        masters.last().unwrap().1, "Kite",
        "the imported master keeps its name"
    );

    // One undo removes the shape AND the imported master together.
    h.key_press_modifiers(egui::Modifiers::CTRL, egui::Key::Z);
    h.run();
    assert_eq!(h.state().debug_shape_count(), 0, "undo removed the instance");
    assert_eq!(
        h.state().debug_doc_masters().len(),
        masters_before,
        "the same undo removed the imported master"
    );
    let _ = imported_id;
}

/// Thumbnails are cached by (stencil serial, master id) — never a bare
/// master id — and evicted when their stencil closes (#85).
#[test]
fn stencil_thumbnails_key_by_source_and_evict_on_close() {
    use dc_ui::stencil::StencilId;
    use egui_kittest::kittest::Queryable;
    let mut h = harness_with_blank();
    h.run();
    let doc_thumbs = h.state().debug_stencil_thumb_count_for_test();
    let tmp = tempfile::tempdir().unwrap();
    let a = synth_stencil(tmp.path(), "alpha", &[("Square", 1), ("Circle", 2)]);
    let b = synth_stencil(tmp.path(), "beta", &[("Star", 1), ("Arrow", 2)]);
    h.state_mut().open_stencil_for_test(&a);
    h.state_mut().open_stencil_for_test(&b);
    h.run();
    h.run();

    // Master id 1 exists in BOTH stencils — the two cells must never
    // share a texture.
    let a1 = h.state().debug_thumb_id_for_test(StencilId::Open(0), 1);
    let b1 = h.state().debug_thumb_id_for_test(StencilId::Open(1), 1);
    assert!(a1.is_some() && b1.is_some(), "both thumbnails landed (sync seam)");
    assert_ne!(a1, b1, "colliding master ids never share a texture");
    assert_eq!(
        h.state().debug_stencil_thumb_count_for_test(),
        doc_thumbs + 4,
        "document set + both stencils cached independently"
    );

    // Golden: the pane with two stencils open, thumbnails rendered from
    // each stencil's own document.
    ctx_menu_snapshot(&mut h, "dc_ui_stencil_two_open");

    // Close alpha via its entry header's context menu.
    let header = h
        .state()
        .debug_stencil_entry_rects()
        .into_iter()
        .find(|(id, _)| *id == StencilId::Open(0))
        .expect("alpha header")
        .1;
    click_at(&mut h, header.center(), egui::PointerButton::Secondary);
    h.run();
    h.get_by_label("Close Stencil...").click();
    h.run();

    let entries = h.state().debug_stencil_entries();
    assert_eq!(entries.len(), 1, "alpha closed");
    assert_eq!(entries[0].0, "beta", "the surviving entry keeps its identity");
    assert_eq!(
        h.state().debug_stencil_thumb_count_for_test(),
        doc_thumbs + 2,
        "alpha's textures were evicted with its serial"
    );
    assert_eq!(
        h.state().debug_thumb_id_for_test(StencilId::Open(0), 1),
        Some(b1.unwrap()),
        "beta (now entry 0) kept its own art — not a stale alpha texture"
    );
}

/// #86: an active search replaces the entry list with one virtual
/// "Search Results" entry aggregating matches from the document set
/// and every open stencil (observed Visio 2016 behavior); clearing the
/// box restores the list with each entry's own expansion — search is a
/// view, never a state change.
#[test]
fn search_aggregates_matches_into_a_results_entry() {
    use dc_ui::stencil::StencilId;
    let mut h = harness_with_blank();
    h.run();
    let tmp = tempfile::tempdir().unwrap();
    // "r" matches Square/Circle (alpha), Star/Arrow (beta) and some of
    // the blank template's document masters; "square" is alpha-only.
    let a = synth_stencil(tmp.path(), "alpha", &[("Square", 1), ("Circle", 2)]);
    let b = synth_stencil(tmp.path(), "beta", &[("Star", 1), ("Arrow", 2)]);
    h.state_mut().open_stencil_for_test(&a);
    h.state_mut().open_stencil_for_test(&b);
    h.run();

    let keys = |h: &Harness<'_, App>| {
        h.state()
            .debug_master_cells()
            .iter()
            .map(|(k, _)| *k)
            .collect::<Vec<_>>()
    };
    let visible = |h: &Harness<'_, App>| h.state().debug_visible_entry_ids();
    let full_doc = {
        let doc: Vec<i64> = keys(&h)
            .into_iter()
            .filter(|k| k.0 == StencilId::Document)
            .map(|k| k.1)
            .collect();
        doc.len()
    };
    assert!(full_doc > 0, "blank template exposes starter masters");

    // One query matching masters in every source: the stencil entries
    // are replaced by a single aggregated results grid holding matches
    // from the document set, then each open stencil in pane order.
    h.state_mut().set_search_for_test("r");
    h.run();
    assert!(
        visible(&h).is_empty(),
        "no stencil entries under an active search"
    );
    let got = keys(&h);
    let srcs: Vec<dc_ui::stencil::StencilId> = {
        let mut v = Vec::new();
        for k in &got {
            if v.last() != Some(&k.0) {
                v.push(k.0);
            }
        }
        v
    };
    assert_eq!(
        srcs,
        vec![
            StencilId::Document,
            StencilId::Open(0),
            StencilId::Open(1)
        ],
        "results aggregate the document set then each stencil in order"
    );
    let doc_hits = got.iter().filter(|k| k.0 == StencilId::Document).count();
    assert!(
        doc_hits > 0 && doc_hits < full_doc,
        "document matches narrowed: {doc_hits} of {full_doc}"
    );
    assert_eq!(
        got.iter()
            .filter(|k| k.0 == StencilId::Open(0) || k.0 == StencilId::Open(1))
            .count(),
        4,
        "both stencils' matching masters present"
    );

    // A stencil-only match still shows the results entry with just
    // that one master.
    h.state_mut().set_search_for_test("square");
    h.run();
    let got = keys(&h);
    assert_eq!(
        got,
        vec![(StencilId::Open(0), 1)],
        "the one matching master, addressed by its own source"
    );

    // Search is a view, never a state change: collapse an open
    // stencil entry, run a search (which must still surface its
    // matches), clear it — the entry list comes back exactly as it
    // was, alpha collapsed, beta expanded.
    h.state_mut().set_search_for_test("");
    h.run();
    let entries = h.state().debug_stencil_entry_rects();
    let alpha_rect = entries
        .iter()
        .find(|(id, _)| *id == StencilId::Open(0))
        .map(|(_, r)| *r)
        .expect("alpha entry restored");
    click_at(&mut h, alpha_rect.center(), egui::PointerButton::Primary);
    assert_eq!(
        visible(&h),
        vec![StencilId::Open(0), StencilId::Open(1)],
        "collapsed alpha keeps its header"
    );
    assert_eq!(
        keys(&h).into_iter().filter(|k| k.0 == StencilId::Open(0)).count(),
        2,
        "collapsed alpha hides its grid (Quick Shapes still shows its heads)"
    );

    h.state_mut().set_search_for_test("square");
    h.run();
    assert!(
        keys(&h).into_iter().any(|k| k.0 == StencilId::Open(0)),
        "the search still surfaces the collapsed entry's masters"
    );

    h.state_mut().set_search_for_test("");
    h.run();
    assert_eq!(
        visible(&h),
        vec![StencilId::Open(0), StencilId::Open(1)],
        "clearing restores every entry"
    );
    assert_eq!(
        keys(&h).into_iter().filter(|k| k.0 == StencilId::Open(0)).count(),
        2,
        "alpha is still collapsed — the flag survived the search (Quick Shapes keeps its heads)"
    );
    let entries = h.state().debug_stencil_entries();
    assert_eq!(entries.len(), 2, "stencil expansion flags untouched");
    assert_eq!(entries[0], ("alpha".to_string(), false), "alpha still collapsed");
    assert_eq!(entries[1], ("beta".to_string(), true), "beta still expanded");
}

/// #87: the More Shapes menu lists the built-in catalog by category;
/// opening a built-in lands in the pane like a file stencil, and the
/// same menu path toggles it closed.
#[test]
fn builtin_catalog_menu_opens_and_toggles_stencils() {
    use dc_ui::stencil::StencilId;
    use egui_kittest::kittest::Queryable;
    let mut h = harness_with_blank();
    h.run();

    // General ▸ Basic Shapes: 48 masters land as an expanded entry.
    h.get_by_label("More Shapes").click();
    h.run();
    h.get_by_label("General").click();
    h.run();
    h.get_by_label("Basic Shapes").click();
    for _ in 0..4 {
        h.run(); // the pane request is handled on a later frame
    }
    let entries = h.state().debug_stencil_entries();
    assert_eq!(entries, vec![("Basic Shapes".to_string(), true)]);
    assert_eq!(
        h.state()
            .debug_master_cells()
            .iter()
            .filter(|(k, _)| k.0 == StencilId::Open(0))
            .count(),
        48 + 6,
        "all 48 masters in the entry plus its head 6 in Quick Shapes (#88)"
    );
    assert!(h.state().debug_status().contains("opened Basic Shapes"));
    ctx_menu_snapshot(&mut h, "dc_ui_builtin_basic_shapes");

    // Accessories ▸ Containers: a second category, a second entry.
    h.get_by_label("More Shapes").click();
    h.run();
    h.get_by_label("Accessories").click();
    h.run();
    h.get_by_label("Containers").click();
    for _ in 0..4 {
        h.run();
    }
    let entries = h.state().debug_stencil_entries();
    assert_eq!(
        entries,
        vec![
            ("Basic Shapes".to_string(), true),
            ("Containers".to_string(), true)
        ]
    );

    // The same menu path toggles: Basic Shapes closes (Containers
    // shifts down a pane position), the status names the close.
    h.get_by_label("More Shapes").click();
    h.run();
    h.get_by_label("General").click();
    h.run();
    h.get_by_label("Basic Shapes").click();
    for _ in 0..4 {
        h.run();
    }
    let entries = h.state().debug_stencil_entries();
    assert_eq!(entries, vec![("Containers".to_string(), true)]);
    assert!(h.state().debug_status().contains("closed Basic Shapes"));
    assert!(
        !h.state()
            .debug_master_cells()
            .iter()
            .any(|(k, _)| matches!(k.0, StencilId::Open(1))),
        "containers shifted to pane position 0 (no stale Open(1) cells)"
    );
    assert!(
        h.state()
            .debug_master_cells()
            .iter()
            .any(|(k, _)| k.0 == StencilId::Open(0)),
        "containers' masters are listed at pane position 0"
    );
}

/// #88: Quick Shapes — the pane's virtual first entry aggregating the
/// head masters of every source (document set first, then each open
/// stencil in pane order), hidden masters excluded. Placing from it
/// imports through the source identity exactly like the source entry,
/// and closing a stencil removes its contributions.
#[test]
fn quick_shapes_aggregates_places_and_tracks_closures() {
    use dc_ui::stencil::StencilId;
    let mut h = harness_with_blank();
    h.run();
    let tmp = tempfile::tempdir().unwrap();
    let a = synth_stencil_opts(tmp.path(), "alpha", &[("Square", 1), ("Circle", 2)], &[("Ghost", 3)]);
    let b = synth_stencil_opts(tmp.path(), "beta", &[("Star", 1)], &[]);
    h.state_mut().open_stencil_for_test(&a);
    h.state_mut().open_stencil_for_test(&b);
    h.run();

    // The first cells in the pane belong to Quick Shapes: the doc
    // set's masters first, then alpha's visible heads (Ghost hidden
    // masters never contribute), then beta's.
    let keys: Vec<(dc_ui::stencil::StencilId, i64)> = h
        .state()
        .debug_master_cells()
        .iter()
        .map(|(k, _)| *k)
        .collect();
    let doc_masters = h.state().debug_doc_masters().len();
    let mut expect: Vec<(dc_ui::stencil::StencilId, i64)> = h
        .state()
        .debug_doc_masters()
        .iter()
        .take(6)
        .map(|(id, _)| (StencilId::Document, *id))
        .collect();
    expect.extend([(StencilId::Open(0), 1), (StencilId::Open(0), 2)]);
    expect.push((StencilId::Open(1), 1));
    assert_eq!(
        &keys[..expect.len()],
        &expect[..],
        "Quick Shapes leads with every source's visible head masters"
    );
    assert!(
        !keys[..expect.len()].contains(&(StencilId::Open(0), 3)),
        "hidden masters never join Quick Shapes"
    );
    // #102: the document entry is not pinned — the document's masters
    // appear only as Quick Shapes' head cells.
    let quick_heads = 6.min(doc_masters);
    assert_eq!(
        keys.iter().filter(|k| k.0 == StencilId::Document).count(),
        quick_heads,
        "doc head masters appear in Quick Shapes only"
    );

    // Place from Quick Shapes: the Square cell (an open-stencil
    // master) imports + places in one undo (#84 path).
    let before = h.state().debug_doc_masters().len();
    let shapes_before = h.state().debug_shape_count();
    // A reveal-on-open may have scrolled the pane; come back to the
    // top so the Quick Shapes cells are in the viewport.
    h.state_mut().scroll_pane_to_top_for_test();
    h.run();
    let cell = h.state().debug_master_cells()
        .into_iter()
        .find(|(k, _)| *k == (StencilId::Open(0), 1))
        .expect("Square cell in Quick Shapes")
        .1;
    click_at(&mut h, cell.center(), egui::PointerButton::Primary);
    h.run();
    let canvas = h.state().debug_canvas_rect().center();
    click_at(&mut h, canvas, egui::PointerButton::Primary);
    for _ in 0..2 {
        h.run();
    }
    assert_eq!(h.state().debug_shape_count(), shapes_before + 1, "shape placed");
    assert_eq!(
        h.state().debug_doc_masters().len(),
        before + 1,
        "placing an open-stencil master from Quick Shapes imported it"
    );

    // Closing a stencil removes its contributions from Quick Shapes:
    // right-click alpha's entry header → Close Stencil...
    // Quick Shapes makes the pane content taller than the viewport —
    // wheel down to bring alpha's entry header into view.
    let mut wheels = 0;
    let alpha = loop {
        let entries = h.state().debug_stencil_entry_rects();
        let alpha = entries
            .iter()
            .find(|(id, _)| *id == StencilId::Open(0))
            .map(|(_, r)| *r)
            .expect("alpha entry listed");
        if alpha.center().y < 700.0 {
            break alpha;
        }
        wheels += 1;
        assert!(wheels < 40, "pane never scrolled alpha into view");
        h.hover_at(egui::pos2(80.0, 400.0));
        h.event(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, -120.0),
            modifiers: egui::Modifiers::NONE,
            phase: egui::TouchPhase::Move,
            source: egui::MouseWheelSource::Wheel,
        });
        // Hovering pane cells arms tooltips (constant repaint), so
        // step instead of settling.
        h.run_steps(3);
    };
    click_at(&mut h, alpha.center(), egui::PointerButton::Secondary);
    h.run();
    h.get_by_label("Close Stencil...").click();
    for _ in 0..3 {
        h.run();
    }
    let keys: Vec<(dc_ui::stencil::StencilId, i64)> = h
        .state()
        .debug_master_cells()
        .iter()
        .map(|(k, _)| *k)
        .collect();
    // Alpha is gone; beta (one master, Star id 1 — colliding with
    // alpha's Square id, deliberately) shifted to pane position 0, so
    // the pane holds exactly beta's two cells: one in Quick Shapes,
    // one in beta's own entry.
    let open_cells: Vec<(dc_ui::stencil::StencilId, i64)> = keys
        .iter()
        .filter(|k| !matches!(k.0, StencilId::Document))
        .copied()
        .collect();
    assert_eq!(
        open_cells,
        vec![(StencilId::Open(0), 1), (StencilId::Open(0), 1)],
        "alpha's masters left the pane with the stencil; beta's Star is all that remains"
    );
}

/// #102: the drawing's own masters are not pinned as a pane entry —
/// Quick Shapes aggregates them for placement — and "More Shapes ▸
/// Document Stencil" reveals the entry for the session.
#[test]
fn document_stencil_entry_is_opt_in() {
    use dc_ui::stencil::StencilId;
    use egui_kittest::kittest::Queryable;
    let mut h = harness_with_blank();
    h.run();

    // Default: no Document entry — Quick Shapes' head cells only.
    let entries = h.state().debug_stencil_entry_rects();
    assert!(
        entries.iter().all(|(id, _)| *id != StencilId::Document),
        "the document entry is not pinned (#102)"
    );
    let doc_cells = h
        .state()
        .debug_master_cells()
        .iter()
        .filter(|(k, _)| k.0 == StencilId::Document)
        .count();
    assert_eq!(doc_cells, 6, "its head masters live in Quick Shapes");

    // More Shapes ▸ Document Stencil reveals the entry with the full
    // document set.
    h.get_by_label("More Shapes").click();
    h.run();
    h.get_by_label("Document Stencil").click();
    for _ in 0..3 {
        h.run();
    }
    let entries = h.state().debug_stencil_entry_rects();
    assert!(
        entries.iter().any(|(id, _)| *id == StencilId::Document),
        "the toggle reveals the document entry"
    );
    let doc_cells = h
        .state()
        .debug_master_cells()
        .iter()
        .filter(|(k, _)| k.0 == StencilId::Document)
        .count();
    assert_eq!(
        doc_cells,
        6 + 8,
        "Quick Shapes heads plus the full document set in the revealed entry"
    );

    // Untick: the entry leaves again; Quick Shapes is unaffected.
    h.get_by_label("More Shapes").click();
    h.run();
    h.get_by_label("Document Stencil").click();
    for _ in 0..3 {
        h.run();
    }
    let entries = h.state().debug_stencil_entry_rects();
    assert!(
        entries.iter().all(|(id, _)| *id != StencilId::Document),
        "unticking hides the entry again"
    );
    let doc_cells = h
        .state()
        .debug_master_cells()
        .iter()
        .filter(|(k, _)| k.0 == StencilId::Document)
        .count();
    assert_eq!(doc_cells, 6, "Quick Shapes still aggregates the heads");
}


/// Scroll the stencil pane until `key`'s cell is on screen, returning
/// its current rect (#143: deep grids — the document entry's full set,
/// a stencil's non-head masters — sit below the fold).
fn reveal_stencil_cell(
    h: &mut egui_kittest::Harness<'_, App>,
    key: (dc_ui::stencil::StencilId, i64),
) -> egui::Rect {
    let mut wheels = 0;
    loop {
        let cell = h
            .state()
            .debug_master_cells()
            .into_iter()
            .find(|(k, _)| *k == key)
            .map(|(_, r)| r)
            .unwrap_or_else(|| panic!("cell {key:?} in the stencil pane"));
        if cell.center().y < 760.0 {
            return cell;
        }
        wheels += 1;
        assert!(wheels < 80, "pane never scrolled {key:?} into view");
        h.hover_at(egui::pos2(80.0, 400.0));
        h.event(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, -120.0),
            modifiers: egui::Modifiers::NONE,
            phase: egui::TouchPhase::Move,
            source: egui::MouseWheelSource::Wheel,
        });
        h.run_steps(3);
    }
}

/// GitHub #143: per-user Quick Shapes membership, persisted app-side.
/// Add a non-head master from the revealed Document Stencil entry →
/// it joins Quick Shapes and places; a fresh app loading the same
/// store keeps it (the restart simulation); removing restores the
/// default membership. An open built-in's addition survives closing
/// and reopening the stencil (identity = its source path).
#[test]
fn quick_shapes_user_membership_persists_and_toggles() {
    use dc_ui::stencil::StencilId;
    use egui_kittest::kittest::Queryable;
    let tmp = tempfile::tempdir().unwrap();
    let store = tmp.path().join("quickshapes.json");
    let mut h = harness_with_blank();
    h.state_mut().set_quick_shapes_store_for_test(&store);
    h.run();

    // Blank carries masters 2–9; the default head set is 2–7, so
    // master 8 (Cross) is not in Quick Shapes.
    let quick = |h: &egui_kittest::Harness<'_, App>| {
        h.state()
            .debug_quick_cells_for_test()
            .into_iter()
            .filter(|(id, _)| *id == StencilId::Document)
            .map(|(_, m)| m)
            .collect::<Vec<_>>()
    };
    assert_eq!(quick(&h), vec![2, 3, 4, 5, 6, 7], "the default head set");

    // Reveal the Document Stencil entry (the full master grid), then
    // right-click master 8's cell → Add to Quick Shapes.
    h.get_by_label("More Shapes").click();
    h.run();
    h.get_by_label("Document Stencil").click();
    for _ in 0..3 {
        h.run();
    }
    let cell = reveal_stencil_cell(&mut h, (StencilId::Document, 8));
    click_at(&mut h, cell.center(), egui::PointerButton::Secondary);
    h.run();
    h.get_by_label("Add to Quick Shapes").click();
    h.run();
    assert_eq!(
        quick(&h),
        vec![2, 3, 4, 5, 6, 7, 8],
        "the addition joined Quick Shapes after the defaults"
    );
    assert_eq!(h.state().debug_status(), "added to Quick Shapes");

    // The added cell places through the normal quick-cell wiring
    // (its key is the source key, #88's placement path).
    let quick_cell = reveal_stencil_cell(&mut h, (StencilId::Document, 8));
    let to = h.state().debug_canvas_rect().center();
    click_at(&mut h, quick_cell.center(), egui::PointerButton::Primary);
    click_at(&mut h, to, egui::PointerButton::Primary);
    h.run();
    assert_eq!(
        h.state().debug_shape_count(),
        1,
        "the added quick cell places like any other"
    );

    // Restart simulation: a fresh app loading the same store keeps
    // the addition.
    let mut h2 = harness_with_blank();
    h2.state_mut().set_quick_shapes_store_for_test(&store);
    h2.run();
    assert_eq!(
        quick(&h2),
        vec![2, 3, 4, 5, 6, 7, 8],
        "the addition survived the restart (config reload)"
    );

    // Removing the added master restores the default membership.
    let cell = reveal_stencil_cell(&mut h2, (StencilId::Document, 8));
    click_at(&mut h2, cell.center(), egui::PointerButton::Secondary);
    h2.run();
    h2.get_by_label("Remove from Quick Shapes").click();
    h2.run();
    assert_eq!(
        quick(&h2),
        vec![2, 3, 4, 5, 6, 7],
        "removing the addition restores the default membership"
    );

    // Removing a DEFAULT head works too, and re-adding restores it.
    let cell = reveal_stencil_cell(&mut h2, (StencilId::Document, 2));
    click_at(&mut h2, cell.center(), egui::PointerButton::Secondary);
    h2.run();
    h2.get_by_label("Remove from Quick Shapes").click();
    h2.run();
    assert_eq!(quick(&h2), vec![3, 4, 5, 6, 7], "a default head can leave");
    // Re-add through the Document Stencil entry (with 2 out of Quick
    // Shapes, its only cell lives in the full document grid).
    h2.get_by_label("More Shapes").click();
    h2.run();
    h2.get_by_label("Document Stencil").click();
    for _ in 0..3 {
        h2.run();
    }
    let cell = reveal_stencil_cell(&mut h2, (StencilId::Document, 2));
    click_at(&mut h2, cell.center(), egui::PointerButton::Secondary);
    h2.run();
    h2.get_by_label("Add to Quick Shapes").click();
    h2.run();
    assert_eq!(
        quick(&h2),
        vec![2, 3, 4, 5, 6, 7],
        "re-adding restores the default member (cells stay id-ordered)"
    );
}

/// #143: an open stencil's additions key by its source path, so they
/// survive closing and reopening the stencil (pane index shifts do
/// not matter).
#[test]
fn quick_shapes_additions_survive_stencil_close_and_reopen() {
    use dc_ui::stencil::StencilId;
    use egui_kittest::kittest::Queryable;
    let tmp = tempfile::tempdir().unwrap();
    let store = tmp.path().join("quickshapes.json");
    let mut h = harness_with_blank();
    h.state_mut().set_quick_shapes_store_for_test(&store);
    h.run();

    // More Shapes ▸ General ▸ Basic Shapes opens a built-in stencil.
    h.get_by_label("More Shapes").click();
    h.run();
    h.get_by_label("General").click();
    h.run();
    h.get_by_label("Basic Shapes").click();
    for _ in 0..4 {
        h.run();
    }
    // A non-head master of the stencil: its id is absent from Quick
    // Shapes' open-stencil cells (the default head set).
    let head_ids: Vec<i64> = h
        .state()
        .debug_quick_cells_for_test()
        .into_iter()
        .filter(|(id, _)| matches!(id, StencilId::Open(_)))
        .map(|(_, m)| m)
        .collect();
    let target = h
        .state()
        .debug_master_cells()
        .into_iter()
        .find(|(k, _)| matches!(k.0, StencilId::Open(_)) && !head_ids.contains(&k.1))
        .map(|(k, _)| k)
        .expect("a non-head master cell in the open stencil");
    let cell = reveal_stencil_cell(&mut h, target);
    click_at(&mut h, cell.center(), egui::PointerButton::Secondary);
    h.run();
    h.get_by_label("Add to Quick Shapes").click();
    h.run();
    let quick_has = |h: &egui_kittest::Harness<'_, App>| {
        h.state()
            .debug_quick_cells_for_test()
            .iter()
            .any(|k| k.1 == target.1 && matches!(k.0, StencilId::Open(_)))
    };
    assert!(quick_has(&h), "the stencil master joined Quick Shapes");

    // Close the stencil (the menu checkbox toggles it off), reopen
    // it — the addition must survive (identity is the source path,
    // not the pane slot).
    for _ in 0..2 {
        h.get_by_label("More Shapes").click();
        h.run();
        h.get_by_label("General").click();
        h.run();
        h.get_by_label("Basic Shapes").click();
        for _ in 0..4 {
            h.run();
        }
    }
    assert!(
        quick_has(&h),
        "the addition survived close and reopen (pane slot changed)"
    );
}

/// #85 follow-up: the More Shapes "Metric Stencils" session toggle —
/// built-ins open from their metric twins when ticked (the shipped U/M
/// pairs differ in master geometry only), and the status names the
/// variant.
#[test]
fn metric_toggle_opens_the_metric_variant() {
    use egui_kittest::kittest::Queryable;
    let mut h = harness_with_blank();
    h.run();

    // Default: the US variant.
    h.get_by_label("More Shapes").click();
    h.run();
    h.get_by_label("General").click();
    h.run();
    h.get_by_label("Basic Shapes").click();
    for _ in 0..4 {
        h.run();
    }
    assert_eq!(
        h.state().debug_stencil_entries(),
        vec![("Basic Shapes".to_string(), true)]
    );
    assert_eq!(h.state().debug_status(), "opened Basic Shapes");

    // Tick Metric Stencils, close, reopen: the metric twin opened.
    h.get_by_label("More Shapes").click();
    h.run();
    h.get_by_label("Metric Stencils").click();
    for _ in 0..3 {
        h.run();
    }
    h.get_by_label("More Shapes").click();
    h.run();
    h.get_by_label("General").click();
    h.run();
    h.get_by_label("Basic Shapes").click(); // untick closes the US entry
    for _ in 0..4 {
        h.run();
    }
    assert_eq!(h.state().debug_stencil_entries(), Vec::new(), "US entry closed");
    h.get_by_label("More Shapes").click();
    h.run();
    h.get_by_label("General").click();
    h.run();
    h.get_by_label("Basic Shapes").click(); // reopen — now metric
    for _ in 0..4 {
        h.run();
    }
    assert_eq!(
        h.state().debug_stencil_entries(),
        vec![("Basic Shapes".to_string(), true)]
    );
    assert_eq!(
        h.state().debug_status(),
        "opened Basic Shapes (metric)",
        "the metric twin opened under the session preference"
    );
}

// ---- GitHub #109: the snap & glue engine ---------------------------------

/// The snap engine lands a dragged shape on a neighbor's alignment
/// box (URS-1100): after a real drag whose release point is a few px
/// off perfect alignment, the shape lands exactly aligned — and the
/// dynamic grid (URS-1088) reports alignment hints while the drag is
/// live and none after release. (A drag that never leaves the snap
/// threshold stays a no-op: no movement, no undo entry — Visio's
/// feel, and why the shape is first dragged well clear.)
#[test]
fn drag_snaps_to_neighbors_alignment_box() {
    let mut h = harness_with_blank();
    h.run();
    h.run(); // fit settles
    // Alignment-box targets only — a deterministic landing that does
    // not depend on the grid or ruler spacings.
    let mut s = h.state().debug_snap_settings();
    s.snap_active = true;
    s.snap_grid = false;
    s.snap_ruler_subdivisions = false;
    s.snap_guides = false;
    s.snap_shape_extensions = false;
    s.snap_shape_intersections = false;
    s.snap_connection_points = false;
    s.snap_alignment_boxes = true;
    s.dynamic_grid = true;
    h.state_mut().debug_set_snap_for_test(s);
    h.run();

    // Two instances of the Rectangle master (both land at the canvas
    // center; the second is on top and is the one a press grabs).
    let (page, a) = place_master(&mut h, 2);
    let (page2, b) = place_master(&mut h, 2);
    assert_eq!(page, page2);
    let ar = h.state().debug_shape_page_rect(a).expect("A rect");

    // Drag B well clear of A (beyond the snap threshold the whole
    // way, so this is a real move), then read its live position for
    // the gesture below.
    let bs = h
        .state()
        .debug_shape_screen_rect_for_test(b)
        .expect("B screen rect");
    drag_naturally(&mut h, bs.center(), egui::vec2(120.0, 90.0));
    let bp = h.state().debug_shape_page_rect(b).expect("B rect after moving away");
    assert!(
        (bp.0 - ar.0).abs() > 0.3,
        "B moved clear of A first: {bp:?} vs {ar:?}"
    );
    let bs = h
        .state()
        .debug_shape_screen_rect_for_test(b)
        .expect("B screen rect (moved)");

    // page (y-up) → screen affine derived from B's two rects.
    let k = bs.width() as f64 / bp.2;
    let p2s = |x: f64, y: f64| {
        egui::pos2(
            bs.left() + ((x - bp.0) * k) as f32,
            bs.top() + (((bp.1 + bp.3) - y) * k) as f32,
        )
    };
    // Drag B back to within 3 px (both axes) of A's center: a real
    // travel, with the release point inside the snap strength, so the
    // engine must land it exactly aligned.
    let ppi = 96.0 * h.state().debug_view_zoom();
    let target = p2s(
        ar.0 + ar.2 / 2.0 + 3.0 / ppi as f64,
        ar.1 + ar.3 / 2.0 + 3.0 / ppi as f64,
    );
    let from = bs.center();

    let press = |pos| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    };
    let release = |pos| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    };
    h.hover_at(from);
    h.event(press(from));
    h.run();
    for t in [0.25, 0.5, 0.75] {
        h.hover_at(from + (target - from) * t);
        h.run();
    }
    // Mid-drag: the dynamic grid reports alignment hints (x and y)
    // against the neighbor.
    assert!(
        h.state().debug_snap_hint_count() >= 2,
        "alignment hints live mid-drag"
    );
    h.hover_at(target);
    h.run();
    h.event(release(target));
    h.run();

    let br = h.state().debug_shape_page_rect(b).expect("B rect after");
    assert!(
        (br.0 - ar.0).abs() < 1e-6,
        "B snapped back onto A's left edge: {} vs {}",
        br.0,
        ar.0
    );
    assert!(
        (br.1 - ar.1).abs() < 1e-6,
        "B snapped onto A's bottom edge: {} vs {} ({:?} vs {:?})",
        br.1,
        ar.1,
        br,
        ar
    );
    assert_eq!(h.state().debug_snap_hint_count(), 0, "hints clear on release");
}

/// Glue settings decide where a 1D endpoint sticks (URS-1112): with
/// shape-handle glue enabled, dropping the endpoint 3 px off a
/// rectangle's corner lands exactly on the handle. The sticky Connect
/// itself is #108's connector work — #109 delivers the settings and
/// the landing.
#[test]
fn endpoint_glues_to_a_shape_handle() {
    let mut h = harness_with_blank();
    h.run();
    h.run();
    let mut s = h.state().debug_snap_settings();
    s.snap_active = false; // the snap side must not interfere
    s.glue_active = true;
    s.glue_guides = false;
    s.glue_shape_handles = true;
    s.glue_shape_vertices = false;
    s.glue_connection_points = false;
    h.state_mut().debug_set_snap_for_test(s);
    h.run();

    let (page, rect) = place_master(&mut h, 2);
    // The Line master sits outside Quick Shapes' head set — reveal the
    // document stencil entry (#102) to place it.
    h.state_mut().show_doc_stencil_for_test();
    h.run();
    let (page2, line) = place_master(&mut h, LINE_MASTER);
    assert_eq!(page, page2);
    let r = h.state().debug_shape_page_rect(rect).expect("rect");
    let corner = (r.0 + r.2, r.1 + r.3); // top-right, page y-up

    let bp = h.state().debug_shape_page_rect(line).expect("line rect");
    let bs = h
        .state()
        .debug_shape_screen_rect_for_test(line)
        .expect("line screen rect");
    let k = bs.width() as f64 / bp.2;
    let p2s = |x: f64, y: f64| {
        egui::pos2(
            bs.left() + ((x - bp.0) * k) as f32,
            bs.top() + (((bp.1 + bp.3) - y) * k) as f32,
        )
    };
    // The line rect's y extent is its stroke bbox; for p2s's y the
    // rect height may be ~0 — fall back to B's screen rect height.
    let k_y = (bs.height() as f64 / bp.3).max(k);

    let end = h
        .state()
        .debug_handle_screen(dc_ui::canvas::HandleId::LineEnd)
        .expect("endpoint handle");
    let ppi = 96.0 * h.state().debug_view_zoom();
    let target = p2s(corner.0 - 3.0 / ppi as f64 * (k / k_y), corner.1);
    drag_naturally(&mut h, end, target - end);

    let (ex, ey) = (cell_of(&h, page, line, "EndX"), cell_of(&h, page, line, "EndY"));
    assert!(
        (ex - corner.0).abs() < 1e-5 && (ey - corner.1).abs() < 1e-5,
        "endpoint glued to the handle: ({ex:.4}, {ey:.4}) vs ({:.4}, {:.4})",
        corner.0,
        corner.1
    );
    let status = h.state().debug_status();
    assert!(
        status.contains("glued"),
        "the landing notes the glue target: {status}"
    );
}

/// The Snap & Glue dialog writes every toggle straight into the
/// document's DocumentSettings (GitHub #109) — a blank drawing grows
/// the element on its first change, and the ribbon's Dynamic Grid
/// toggle persists too.
#[test]
fn snap_glue_dialog_toggles_persist_to_the_document() {
    let mut h = harness_with_blank();
    h.run();
    // The blank template carries no DocumentSettings — none reads as
    // the shipped Visio 2016 defaults.
    assert!(h.state().debug_doc_snap_glue().is_none());
    assert_eq!(
        h.state().debug_snap_settings().to_doc(),
        vsdx::model::DocSnapGlue::CORPUS_DEFAULT
    );

    h.get_by_label_contains("View").click();
    h.run();
    h.get_by_label_contains("Snap & Glue").click();
    h.run();
    // "Alignment boxes" is off in the corpus default (295) — turn it
    // on: 295 + 128 = 423 must land in the document.
    h.get_by_label_contains("Alignment boxes").click();
    h.run();
    let d = h.state().debug_doc_snap_glue().expect("DocumentSettings written");
    assert_eq!(d.snap, 423, "alignment-boxes bit persisted");
    assert_eq!(d.glue, 9, "glue untouched");
    assert!(h.state().debug_dirty(), "a settings change marks the doc dirty");

    // The ribbon's Dynamic Grid toggle is the same write path.
    h.get_by_label_contains("Dynamic Grid").click();
    h.run();
    let d = h.state().debug_doc_snap_glue().expect("DocumentSettings present");
    assert!(!d.dynamic_grid, "dynamic grid toggled off in the doc");
    assert!(!h.state().debug_snap_settings().dynamic_grid);
}
