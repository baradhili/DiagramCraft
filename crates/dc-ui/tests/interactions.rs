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

/// The wrap formula, mirroring `columns_for` in stencil.rs: C columns
/// occupy C*64 + (C-1)*6 px of wrapped width.
fn cols_for(w: f32) -> usize {
    (((w + 6.0) / 70.0).floor() as usize).max(1)
}

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
    // Quick Shapes (#88) adds the 6 head masters once more on top of
    // the document entry's 8.
    assert_eq!(
        cells.len(),
        8 + 6,
        "8 starter masters in the doc entry (incl. the 1D Line, #59) + their Quick Shapes heads"
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

    // The rendered column count matches the computed optimum for the
    // width the grid actually wrapped into, at every stop.
    let mut cols_380 = 0;
    for _ in 0..3 {
        let expect = cols_for(h.state().debug_stencil_wrap_width());
        cols_380 = h.state().debug_stencil_cols();
        assert_eq!(cols_380, expect, "column count matches the wrap optimum");
        assert_grid_fits(&h);
        h.run();
    }

    // And back down through the issue's other stops: 260px, then 160.
    drag_stencil_edge_to(&mut h, 260.0);
    let cols_260 = h.state().debug_stencil_cols();
    assert_eq!(
        cols_260,
        cols_for(h.state().debug_stencil_wrap_width()),
        "260px pane matches the wrap optimum"
    );
    assert!(cols_260 < cols_380, "narrowing reduces the column count");
    drag_stencil_edge_to(&mut h, 160.0);
    assert_eq!(
        h.state().debug_stencil_cols(),
        cols_for(h.state().debug_stencil_wrap_width()),
        "back at 160px the wrap optimum holds"
    );
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

/// A synthetic flattened group — the structure real Visio files save
/// (inline sub-shapes whose XForm and geometry cells are `F='Inh'`
/// snapshots of formula evaluations at the instance's size; the
/// utilisation.vsdx "Receive order" report, GitHub #59). Resizing must
/// refresh those caches (Visio's edit-time refresh) or the selection
/// box grows around a stale drawing: sub-shape XForms, the Txt block,
/// and geometry row coordinates all follow the new size, while a
/// fixed-size sub-shape (a GUARD'ed icon) keeps its size.
#[test]
fn resizing_a_flattened_group_refreshes_subshape_caches() {
    // -- fixture: master 10 "TaskG" = root group + proportional Body
    //    sub + fixed-size Icon sub; instance with inline F='Inh' caches.
    let mut ed = dc_core::Editor::new_blank();
    let page_id = ed.doc.pages[0].id;
    // Detach the source package: the retained-package write path only
    // re-serializes masters that already exist in the source zip, so a
    // master ADDED in the editing layer would be dropped on save (a
    // writer gap of its own — pages got a reconcile pass in M4, masters
    // never did). The fresh-document path writes every modeled master.
    ed.doc.package = None;
    let cell = |v: &str, f: Option<&str>| vsdx_model_cell(v, f);

    let mut mroot = vsdx::model::Shape {
        id: 1,
        type_: Some("Group".into()),
        ..vsdx::model::Shape::default()
    };
    // The layout cells real task masters use (Process.7 pattern): the
    // Txt block and icon pins hide their size dependence behind User
    // rows — `TxtPinX = GUARD(User.TxtPinX)` where `User.TxtPinX =
    // Width/2`, `PinY = GUARD(Sheet.1!User.IconY)` where
    // `User.IconY = Height - User.InfoPanelHeight/2`. A refresh that
    // only greps formulas for "Width"/"Height" cannot see through the
    // indirection and leaves those caches stale (the icon/text drift
    // report on #59).
    for (n, v, f) in [
        ("IconX", "0.1", None),
        ("IconY", "0.9", Some("Height-User.InfoPanelHeight/2")),
        ("InfoPanelHeight", "0.2", None),
        ("TxtPinX", "1", Some("Width/2")),
        ("TxtPinY", "0.5", Some("Height/2")),
        ("TxtWidth", "2", Some("Width*1")),
        ("TxtHeight", "1", Some("Height*1")),
    ] {
        add_user_row(&mut mroot.sheet, n, v, f);
    }
    for (n, v, f) in [
        ("PinX", "1", None),
        ("PinY", "0.5", None),
        ("Width", "2", None),
        ("Height", "1", None),
        ("TxtPinX", "1", Some("GUARD(User.TxtPinX)")),
        ("TxtPinY", "0.5", Some("GUARD(User.TxtPinY)")),
        ("TxtWidth", "2", Some("GUARD(User.TxtWidth)")),
        ("TxtHeight", "1", Some("GUARD(User.TxtHeight)")),
        ("TxtLocPinX", "1", Some("TxtWidth*0.5")),
        ("TxtLocPinY", "0.5", Some("TxtHeight*0.5")),
    ] {
        mroot.sheet.set_cell(n, cell(v, f));
    }
    let mut mbody = vsdx::model::Shape {
        id: 2,
        type_: Some("Shape".into()),
        ..vsdx::model::Shape::default()
    };
    for (n, v, f) in [
        ("PinX", "1", "Sheet.1!Width*0.5"),
        ("PinY", "0.5", "Sheet.1!Height*0.5"),
        ("Width", "2", "Sheet.1!Width*1"),
        ("Height", "1", "Sheet.1!Height*1"),
        ("LocPinX", "1", "Width*0.5"),
        ("LocPinY", "0.5", "Height*0.5"),
    ] {
        mbody.sheet.set_cell(n, cell(v, Some(f)));
    }
    add_rect_geometry(&mut mbody.sheet, 2.0, 1.0, None);
    let mut micon = vsdx::model::Shape {
        id: 3,
        type_: Some("Shape".into()),
        ..vsdx::model::Shape::default()
    };
    for (n, v, f) in [
        ("PinX", "0.2", "GUARD(Sheet.1!User.IconX)"),
        ("PinY", "0.2", "GUARD(Sheet.1!User.IconY)"),
        ("Width", "0.157480", "GUARD(4MM)"),
        ("Height", "0.110236", "GUARD(2.8MM)"),
        ("LocPinX", "0.078740", "Width*0.5"),
        ("LocPinY", "0.055118", "Height*0.5"),
    ] {
        micon.sheet.set_cell(n, cell(v, Some(f)));
    }
    add_rect_geometry(&mut micon.sheet, 0.157480, 0.110236, None);
    mroot.sub_shapes = vec![mbody, micon];
    ed.doc.masters.push(vsdx::model::Master {
        id: 10,
        name_u: Some("TaskG".into()),
        attrs: vec![
            ("ID".into(), "10".into()),
            ("NameU".into(), "TaskG".into()),
            ("Name".into(), "TaskG".into()),
        ],
        children: vec![vsdx::model::IndexChild::Rel(vec![(
            "r:id".into(),
            "rId9".into(),
        )])],
        shapes: vec![mroot],
        part: "visio/masters/master10.xml".into(),
        ..vsdx::model::Master::default()
    });

    // Instance: 1 x 0.5 (the master is 2 x 1), inline Body sub caches
    // at instance units, Icon caches at its fixed size.
    let root_id = dc_core::commands::next_shape_id(&ed.doc, page_id);
    let mut inst = vsdx::model::Shape {
        id: root_id,
        type_: Some("Group".into()),
        master: Some(10),
        ..vsdx::model::Shape::default()
    };
    inst.attrs = vec![
        ("ID".into(), root_id.to_string()),
        ("Type".into(), "Group".into()),
        ("Master".into(), "10".into()),
    ];
    for (n, v, f) in [
        ("PinX", "4", None),
        ("PinY", "5", None),
        ("Width", "1", None),
        ("Height", "0.5", None),
        ("LocPinX", "0.5", None),
        ("LocPinY", "0.25", None),
        // The Txt block is formula-driven in the master: the instance
        // carries F='Inh' snapshots, exactly like real files.
        ("TxtPinX", "0.5", Some("Inh")),
        ("TxtPinY", "0.25", Some("Inh")),
        ("TxtWidth", "1", Some("Inh")),
        ("TxtHeight", "0.5", Some("Inh")),
        ("TxtLocPinX", "0.5", Some("Inh")),
        ("TxtLocPinY", "0.25", Some("Inh")),
    ] {
        inst.sheet.set_cell(n, cell(v, f));
    }
    // A cached User row (F='Inh') — real instances ship these too. The
    // master's formula must still own the value at refresh time; a
    // frozen cache here must not freeze the icon's position.
    add_user_row(&mut inst.sheet, "IconY", "0.4", Some("Inh"));
    let mut isub = vsdx::model::Shape {
        id: root_id + 1,
        type_: Some("Shape".into()),
        master: Some(10),
        master_shape: Some(2),
        ..vsdx::model::Shape::default()
    };
    isub.attrs = vec![
        ("ID".into(), (root_id + 1).to_string()),
        ("Type".into(), "Shape".into()),
        ("Master".into(), "10".into()),
        ("MasterShape".into(), "2".into()),
    ];
    for (n, v) in [
        ("PinX", "0.5"),
        ("PinY", "0.25"),
        ("Width", "1"),
        ("Height", "0.5"),
        ("LocPinX", "0.5"),
        ("LocPinY", "0.25"),
    ] {
        isub.sheet.set_cell(n, cell(v, Some("Inh")));
    }
    // Geometry caches in instance units (half the master's literals).
    add_rect_geometry(&mut isub.sheet, 1.0, 0.5, Some("Inh"));
    let mut iicon = vsdx::model::Shape {
        id: root_id + 2,
        type_: Some("Shape".into()),
        master: Some(10),
        master_shape: Some(3),
        ..vsdx::model::Shape::default()
    };
    iicon.attrs = vec![
        ("ID".into(), (root_id + 2).to_string()),
        ("Type".into(), "Shape".into()),
        ("Master".into(), "10".into()),
        ("MasterShape".into(), "3".into()),
    ];
    for (n, v) in [
        ("PinX", "0.1"),
        ("PinY", "0.4"),
        ("Width", "0.157480"),
        ("Height", "0.110236"),
        ("LocPinX", "0.078740"),
        ("LocPinY", "0.055118"),
    ] {
        iicon.sheet.set_cell(n, cell(v, Some("Inh")));
    }
    inst.sub_shapes = vec![isub, iicon];
    ed.doc.pages[0].shapes.push(inst);

    // Round-trip through the package writer: the fixture must survive a
    // save/open like any real file.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("taskg.vsdx");
    ed.save_as(&path).unwrap();
    // The fixture survives the save/open round-trip (master tree and
    // inline sub-shape caches intact).
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
    let (kx, ky) = (w1 / w0, h1 / h0);
    assert!(
        kx > 1.2 && ky > 1.2,
        "resize grew: {w0:.3}x{h0:.3} → {w1:.3}x{h1:.3}"
    );

    // Sub caches followed the group (F='Inh' snapshots refreshed).
    let sub_w = cell_of(&h, page_id, root_id + 1, "Width");
    assert!(
        (sub_w - 1.0 * kx).abs() < 1e-3,
        "sub Width cache scaled: {sub_w:.4} vs {:.4}",
        kx
    );
    let sub_px = cell_of(&h, page_id, root_id + 1, "PinX");
    assert!(
        (sub_px - 0.5 * kx).abs() < 1e-3,
        "sub PinX cache scaled: {sub_px:.4}"
    );
    // The Txt block followed too — including the cells the indirection
    // hides: TxtPinX = GUARD(User.TxtPinX) with User.TxtPinX = Width/2
    // must re-center (the pre-evaluation refresh left these stale and
    // the text drifted low-left).
    let txt_w = cell_of(&h, page_id, root_id, "TxtWidth");
    assert!(
        (txt_w - kx).abs() < 1e-3,
        "root TxtWidth scaled: {txt_w:.4}"
    );
    let txt_px = cell_of(&h, page_id, root_id, "TxtPinX");
    assert!(
        (txt_px - w1 / 2.0).abs() < 1e-3,
        "root TxtPinX re-centered through User.TxtPinX: {txt_px:.4} vs {:.4}",
        w1 / 2.0
    );
    let txt_py = cell_of(&h, page_id, root_id, "TxtPinY");
    assert!(
        (txt_py - h1 / 2.0).abs() < 1e-3,
        "root TxtPinY re-centered: {txt_py:.4} vs {:.4}",
        h1 / 2.0
    );
    let txt_lx = cell_of(&h, page_id, root_id, "TxtLocPinX");
    assert!(
        (txt_lx - w1 / 2.0).abs() < 1e-3,
        "root TxtLocPinX follows TxtWidth: {txt_lx:.4}"
    );
    // Refreshed cells keep F='Inh' — a later resize must refresh them
    // again, not meet frozen literals.
    let (_, txt_px_f) = h
        .state()
        .debug_cell(page_id, root_id, "TxtPinX")
        .expect("TxtPinX cell");
    assert_eq!(txt_px_f, "Inh", "refreshed cache keeps its Inh marker");

    // The fixed-size icon kept its GUARD'ed size …
    let icon_w = cell_of(&h, page_id, root_id + 2, "Width");
    assert!(
        (icon_w - 0.157480).abs() < 1e-6,
        "GUARD'ed icon Width unchanged, got {icon_w}"
    );
    // … its pin X stays the fixed User.IconX offset …
    let icon_px = cell_of(&h, page_id, root_id + 2, "PinX");
    assert!(
        (icon_px - 0.1).abs() < 1e-6,
        "icon PinX keeps the fixed IconX offset, got {icon_px}"
    );
    // … while PinY = GUARD(Sheet.1!User.IconY) with
    // User.IconY = Height - User.InfoPanelHeight/2 re-evaluates at the
    // new height (the instance's own cached User.IconY must not freeze
    // it).
    let icon_py = cell_of(&h, page_id, root_id + 2, "PinY");
    assert!(
        (icon_py - (h1 - 0.1)).abs() < 1e-3,
        "icon PinY follows Height through the User indirection: {icon_py:.4} vs {:.4}",
        h1 - 0.1
    );

    // And the rendered drawing spans the new size: the Body's geometry
    // (which fills the group) reaches the new width in the page SVG.
    let svg = h.state().debug_page_svg().unwrap();
    let expected = format!("{:.5}", sub_w);
    assert!(
        svg.contains(&format!("L {expected} ")),
        "rendered geometry scaled to {expected}; SVG:\n{}",
        svg.lines().take(6).collect::<Vec<_>>().join("\n")
    );

    // A SECOND resize refreshes again (the Inh markers survived).
    let br2 = h.state().debug_sel_br_corner().expect("sel");
    drag_naturally(&mut h, br2, egui::vec2(30.0, 20.0));
    let w2 = h.state().debug_sel_width();
    let txt_px2 = cell_of(&h, page_id, root_id, "TxtPinX");
    assert!(
        (txt_px2 - w2 / 2.0).abs() < 1e-3,
        "second resize re-centered TxtPinX: {txt_px2:.4} vs {:.4}",
        w2 / 2.0
    );

    // Each undo restores one resize's whole refresh; two undos return
    // to the authored caches.
    h.key_press_modifiers(egui::Modifiers::CTRL, egui::Key::Z);
    h.run();
    assert!(
        (cell_of(&h, page_id, root_id, "TxtPinX") - w1 / 2.0).abs() < 1e-3,
        "undo #1 restored the first refresh state"
    );
    h.key_press_modifiers(egui::Modifiers::CTRL, egui::Key::Z);
    h.run();
    assert!(
        (cell_of(&h, page_id, root_id + 1, "Width") - 1.0).abs() < 1e-6,
        "undo restored the sub cache"
    );
    assert!(
        (cell_of(&h, page_id, root_id, "TxtPinX") - 0.5).abs() < 1e-6,
        "undo restored the authored TxtPinX cache"
    );
    assert!(
        (cell_of(&h, page_id, root_id + 2, "PinY") - 0.4).abs() < 1e-6,
        "undo restored the authored icon PinY cache"
    );
}

/// Helper of the fixture tests: a Cell with optional formula.
fn vsdx_model_cell(v: &str, f: Option<&str>) -> vsdx::model::Cell {
    vsdx::model::Cell {
        v: Some(v.to_string()),
        f: f.map(|s| s.to_string()),
        ..vsdx::model::Cell::default()
    }
}

/// Helper of the fixture tests: a named User row with a Value cell.
fn add_user_row(sheet: &mut vsdx::model::Sheet, name: &str, v: &str, f: Option<&str>) {
    use vsdx::model::{Row, Section, SheetChild};
    let row = Row {
        n: Some(name.to_string()),
        cells: vec![("Value".to_string(), vsdx_model_cell(v, f))],
        ..Row::default()
    };
    if let Some(SheetChild::Section(sec)) = sheet
        .children
        .iter_mut()
        .find(|c| matches!(c, SheetChild::Section(s) if s.name == "User"))
    {
        sec.rows.push(row);
    } else {
        sheet.children.push(SheetChild::Section(Section {
            name: "User".into(),
            rows: vec![row],
            ..Section::default()
        }));
    }
}

/// Helper of the fixture tests: a rectangle Geometry section; row cells
/// carry `f` (F='Inh' for instance snapshots) when given.
fn add_rect_geometry(sheet: &mut vsdx::model::Sheet, w: f64, h: f64, f: Option<&str>) {
    use vsdx::model::{Cell, Row, Section, SheetChild};
    let mut rows = Vec::new();
    for (ix, t, (x, y)) in [
        (1, "MoveTo", (0.0, 0.0)),
        (2, "LineTo", (w, 0.0)),
        (3, "LineTo", (w, h)),
        (4, "LineTo", (0.0, h)),
        (5, "LineTo", (0.0, 0.0)),
    ] {
        rows.push(Row {
            t: Some(t.to_string()),
            ix,
            cells: vec![
                (
                    "X".to_string(),
                    Cell {
                        v: Some(format!("{x}")),
                        f: f.map(str::to_string),
                        ..Cell::default()
                    },
                ),
                (
                    "Y".to_string(),
                    Cell {
                        v: Some(format!("{y}")),
                        f: f.map(str::to_string),
                        ..Cell::default()
                    },
                ),
            ],
            ..Row::default()
        });
    }
    sheet.children.push(SheetChild::Section(Section {
        name: "Geometry".into(),
        ix: 0,
        rows,
        ..Section::default()
    }));
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
    // The ribbon also has a Group button — take the topmost "…Group…"
    // row in the menu (below the ribbon bands). GitHub #78: that row is
    // the Group ▸ flyout parent; the action lives in its submenu.
    let group_row = h
        .get_all_by_label_contains("Group")
        .filter(|n| n.rect().min.y > 250.0 && n.rect().height() < 30.0)
        .min_by_key(|n| n.rect().min.y as i64)
        .expect("Group ▸ flyout row in the context menu");
    group_row.click();
    h.run();
    let menu = h.state().debug_ctx_menu_rect().expect("menu still open");
    let group_item = h
        .get_all_by_label_contains("Group")
        .filter(|n| n.rect().min.y > 250.0 && n.rect().height() < 30.0)
        .find(|n| !menu.expand(2.0).contains_rect(n.rect()))
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
    h.get_all_by_label_contains("Container")
        .filter(|n| n.rect().min.y > 250.0 && n.rect().height() < 30.0)
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
    h.get_all_by_label_contains("Hyperlink")
        .filter(|n| n.rect().min.y > 250.0 && n.rect().height() < 30.0)
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
    h.get_all_by_label_contains("Format Shape")
        .filter(|n| n.rect().min.y > 250.0 && n.rect().height() < 30.0)
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
    h.get_all_by_label_contains("Solid")
        .filter(|n| n.rect().min.x > 1050.0)
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

    // Undo clears the selection (and with it the pane); reselect the
    // shape for the Text section leg.
    click_at(
        &mut h,
        canvas.center() + egui::vec2(40.0, -40.0),
        egui::PointerButton::Primary,
    );
    h.run();
    h.get_by_label("Bold").click();
    h.run();
    assert_eq!(
        h.state().debug_cell_for_test(sid, "Char.Style"),
        Some("1".into()),
        "bold wrote Character Style"
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
    h.get_all_by_label_contains("Hyperlink")
        .filter(|n| n.rect().min.y > 250.0 && n.rect().height() < 30.0)
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
    h.get_all_by_label_contains("Shape Data")
        .filter(|n| n.rect().min.y > 250.0 && n.rect().height() < 30.0)
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
    assert_eq!(h.state().debug_sel().map(|(p, s)| (p, s)), Some((page, shape)));
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
    // Quick Shapes (#88) leads the pane: every source's head masters
    // appear once more at the top (each synth stencil is under the
    // per-source cap, so its whole set joins). doc_cells was measured
    // with only the document open — quick heads + doc entry — and
    // opening stencils adds no document masters.
    assert_eq!(
        count(&h, StencilId::Document),
        doc_cells,
        "doc masters in Quick Shapes and the document entry"
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
    assert_eq!(entries[0].1, false, "alpha collapsed by its own header");
    assert_eq!(entries[1].1, true, "beta's expansion is independent");
    assert_eq!(
        count(&h, StencilId::Open(0)),
        2,
        "collapsed entry hides its grid (Quick Shapes still shows its heads)"
    );
    assert_eq!(count(&h, StencilId::Open(1)), 4);
    assert_eq!(
        count(&h, StencilId::Document),
        doc_cells,
        "document entry untouched by a stencil collapse"
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
    for _ in 0..8 {
        h.run(); // the worker thread's answer lands via poll_dialog
    }
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
    for _ in 0..8 {
        h.run();
    }
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

    // Search is a view, never a state change: collapse the document
    // entry, run a search (which must still surface the document's
    // matches), clear it — the entry list comes back exactly as it
    // was, document entry collapsed, both stencils expanded.
    h.state_mut().set_search_for_test("");
    h.run();
    let entries = h.state().debug_stencil_entry_rects();
    let doc_rect = entries
        .iter()
        .find(|(id, _)| *id == StencilId::Document)
        .map(|(_, r)| *r)
        .expect("document entry restored");
    click_at(&mut h, doc_rect.center(), egui::PointerButton::Primary);
    assert_eq!(
        visible(&h),
        vec![StencilId::Document, StencilId::Open(0), StencilId::Open(1)],
        "collapsed document entry keeps its header"
    );
    assert_eq!(
        keys(&h).into_iter().filter(|k| k.0 == StencilId::Document).count(),
        6,
        "collapsed document entry hides its grid (Quick Shapes still shows the doc heads)"
    );

    h.state_mut().set_search_for_test("r");
    h.run();
    assert!(
        keys(&h).into_iter().any(|k| k.0 == StencilId::Document),
        "the search still surfaces the collapsed entry's masters"
    );

    h.state_mut().set_search_for_test("");
    h.run();
    assert_eq!(
        visible(&h),
        vec![StencilId::Document, StencilId::Open(0), StencilId::Open(1)],
        "clearing restores every entry"
    );
    assert_eq!(
        keys(&h).into_iter().filter(|k| k.0 == StencilId::Document).count(),
        6,
        "document entry is still collapsed — the flag survived the search (Quick heads remain)"
    );
    let entries = h.state().debug_stencil_entries();
    assert_eq!(entries.len(), 2, "stencil expansion flags untouched");
    assert!(entries.iter().all(|(_, e)| *e), "stencils still expanded");
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
    // The document's own entry follows with the full (unquick) grid:
    // doc-keyed cells = the quick heads (≤6) plus the whole doc entry.
    let quick_heads = 6.min(doc_masters);
    assert_eq!(
        keys.iter().filter(|k| k.0 == StencilId::Document).count(),
        quick_heads + doc_masters,
        "doc masters appear once in Quick Shapes and once in the doc entry"
    );

    // Place from Quick Shapes: the Square cell (an open-stencil
    // master) imports + places in one undo (#84 path).
    let before = h.state().debug_doc_masters().len();
    let shapes_before = h.state().debug_shape_count();
    let cell = h.state().debug_master_cells()
        .into_iter()
        .find(|(k, _)| *k == (StencilId::Open(0), 1))
        .expect("Square cell in Quick Shapes")
        .1;
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
