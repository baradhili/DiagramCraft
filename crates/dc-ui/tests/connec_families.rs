//! #238: every master of BOTH built-in connector stencils (US
//! `connec.vssx`, metric `connec-m.vssx`) exercised through the
//! drag & glue paths headlessly — the same write set the GUI's
//! endpoint-drop and shape-move commits run (glue formulas +
//! `AddConnect` records + the #179 reroute), verified for plan
//! (resolved endpoints, no local rows materialized) and render (ink
//! bounds covering the glue sites), with every phase undone
//! byte-exact.
//!
//! The masters classify by FRAME at runtime (the sweep's families):
//! chord frames (`Width=SQRT((EndX-BeginX)^2+…)`, `Angle=ATAN2`),
//! signed frames (`Width=GUARD(EndX-BeginX)`), and everything else.
//! #248/#249 landed: every 1D master glues, tracks its targets and
//! undoes byte-exact; the chord acceptance and the band/knee net
//! below pin it. #250's static-only reroute gap has its own probe.

use dc_core::{Command, Editor, Scope};
use dc_ui::cmd::{MoveShape, RerouteConnectors};
use vsdx::model::{Cell, Row, Section, Shape, SheetChild};

// -- kittest (the #108 GUI pattern) for the surviving family: the
// -- ObjType=2 Dynamic connector master placed from the real stencil.

use dc_ui::app::App;
use dc_ui::stencil::StencilId;
use egui::Pos2;
use egui_kittest::Harness;

const DYN_CONNECTOR_MASTER: &str = "Dynamic connector";

fn harness_with_blank() -> Harness<'static, App> {
    dc_ui::svgimg::pin_determinism_for_tests();
    Harness::builder()
        .with_size(egui::vec2(1280.0, 800.0))
        .with_max_steps(16)
        .build_ui_state(
            |ui, app: &mut App| {
                dc_ui::theme::install(ui.ctx());
                app.draw(ui);
            },
            App::with_blank(),
        )
}

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

fn cell_of(h: &Harness<'_, App>, page: i64, shape: i64, n: &str) -> f64 {
    h.state()
        .debug_cell(page, shape, n)
        .unwrap()
        .0
        .parse::<f64>()
        .unwrap()
}

/// Place from an OPEN stencil's master cell (click the cell, click the
/// canvas — the pane paradigm), wheeling the pane when the cell sits
/// below the fold.
fn place_from_open(
    h: &mut Harness<'_, App>,
    source: StencilId,
    master: i64,
) -> (i64, i64) {
    let mut wheels = 0;
    let from = loop {
        // The pane repeats a stencil's Quick-Shapes block above its
        // full master list under the same StencilId — the LAST match
        // is the real list's cell.
        let cell = h
            .state()
            .debug_master_cells()
            .into_iter()
            .rev()
            .find(|(id, _)| *id == (source, master))
            .map(|(_, r)| r)
            .unwrap_or_else(|| panic!("master {master} visible in the stencil pane"));
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

/// The Dynamic connector master's id inside the built-in US stencil.
fn dyn_connector_master_id() -> i64 {
    let doc = stencil(US);
    doc.masters
        .iter()
        .find(|m| m.name_u.as_deref() == Some(DYN_CONNECTOR_MASTER))
        .map(|m| m.id)
        .expect("Dynamic connector master in the built-in stencil")
}

const US: &[u8] = include_bytes!("../assets/stencils/connec.vssx");
const METRIC: &[u8] = include_bytes!("../assets/stencils/connec-m.vssx");

/// Masters the #238 sweep flagged for ink that skipped the far end —
/// the #248/#249 fixes healed them; they stay listed as the
/// regression net for the band/knee classes (chord + signed).
const KNOWN_FLAT_BAND: &[(&str, &str)] = &[
    ("2 lines", "chord"),
    ("4 lines", "chord"),
    ("Arced arrow", "chord"),
    ("2-line elbow", "signed"),
    ("4-line elbow", "signed"),
];

struct Exercised {
    lock_refused: bool,
    flat_band: bool,
    glue_ok: bool,
}

fn stencil(bytes: &[u8]) -> vsdx::Document {
    vsdx::parse::parse_bytes(bytes).expect("stencil parses")
}

// -- the fixture (two glue targets; A carries a Connection row) -------

fn rect(id: i64, x: f64, y: f64, with_conn: bool) -> Shape {
    let mut s = Shape {
        id,
        type_: Some("Shape".into()),
        ..Shape::default()
    };
    let cells = &mut s.sheet.children;
    for (n, v) in [
        ("PinX", x + 0.5),
        ("PinY", y + 0.5),
        ("Width", 1.0),
        ("Height", 1.0),
        ("LocPinX", 0.5),
        ("LocPinY", 0.5),
    ] {
        cells.push(SheetChild::Cell(
            n.to_string(),
            Cell::value(format!("{v:.6}")),
        ));
    }
    if with_conn {
        let mut sec = Section {
            name: "Connection".into(),
            ix: 0,
            ..Section::default()
        };
        sec.rows.push(Row {
            ix: 2,
            cells: vec![
                ("X".to_string(), Cell::value("1.0")),
                ("Y".to_string(), Cell::value("0.5")),
            ],
            ..Row::default()
        });
        s.sheet.children.push(SheetChild::Section(sec));
    }
    s
}

fn fresh_doc(master: &vsdx::Master) -> vsdx::Document {
    let mut m = master.clone();
    m.id = 1;
    vsdx::Document {
        pages: vec![vsdx::model::Page {
            id: 0,
            name: "P1".into(),
            part: "pages/page1.xml".into(),
            attrs: vec![
                ("ID".into(), "0".into()),
                ("NameU".into(), "P1".into()),
            ],
            shapes: vec![rect(1, 2.0, 2.0, true), rect(2, 7.5, 5.0, true)],
            ..vsdx::model::Page::default()
        }],
        masters: vec![m],
        ..vsdx::Document::default()
    }
}

// -- the exercise ------------------------------------------------------

fn snapshot(ed: &Editor, conn_id: i64) -> Vec<(String, Option<String>, Option<String>)> {
    let Some(conn) = vsdx::model::find_shape_by_id(&ed.doc.pages[0].shapes, conn_id) else {
        return Vec::new();
    };
    let mut out: Vec<(String, Option<String>, Option<String>)> = Vec::new();
    for c in &conn.sheet.children {
        if let SheetChild::Cell(n, cell) = c {
            out.push((n.clone(), cell.v.clone(), cell.f.clone()));
        }
    }
    out.sort();
    out
}

fn resolved(ed: &Editor, conn_id: i64) -> (f64, f64, f64, f64) {
    let conn = vsdx::model::find_shape_by_id(&ed.doc.pages[0].shapes, conn_id).unwrap();
    let chain = dc_core::scope::shape_chain(&ed.doc, 0, conn_id).unwrap();
    let g = |n: &str| {
        ed.doc
            .resolve_cell(conn, &chain, n)
            .and_then(|c| c.numeric())
            .unwrap_or(f64::NAN)
    };
    (g("BeginX"), g("BeginY"), g("EndX"), g("EndY"))
}

fn ink(ed: &Editor, conn_id: i64) -> Option<(f64, f64, f64, f64)> {
    let page = &ed.doc.pages[0];
    dc::svg::shape_ink_bounds(&ed.doc, page, conn_id)
}

/// Page-inches → the y-down ink box's coverage test (the page is 11
/// tall in the fixture; the stroke pad is 0.15in).
fn covers(b: (f64, f64, f64, f64), x: f64, y: f64) -> bool {
    let (top, bottom) = (11.0 - b.1 - b.3, 11.0 - b.1);
    b.0 - 0.15 <= x && x <= b.0 + b.2 + 0.15 && top - 0.15 <= y && y <= bottom + 0.15
}

/// The full drop → glue → move → undo cycle for ONE master. Returns
/// the per-master outcome flags the assertions read.
fn exercise(master: &vsdx::Master) -> Exercised {
    exercise_mode(master, false)
}

/// `static_only` glues BOTH ends statically (`PAR(PNT(...))`, no
/// trigger cells) — the #250 probe: glue-formula-owned masters must
/// still reroute.
fn exercise_mode(master: &vsdx::Master, static_only: bool) -> Exercised {
    let name = master.name_u.clone().unwrap_or_default();
    let root = master.shapes.first().expect("master root");
    let out = Exercised {
        lock_refused: false,
        flat_band: false,
        glue_ok: false,
    };
    if root.sheet.cell("BeginX").is_none() {
        // 2D masters in the stencil (the Star) are not glue
        // connectors — skip.
        return out;
    }

    let mut ed = Editor::new(fresh_doc(master));
    // Phase 1: drop (the stencil drag).
    ed.apply(dc_core::AddMasterInstance::new(0, 1, (5.0, 3.0)))
        .expect("drop applies");
    let conn_id = 3;
    let before_glue = snapshot(&ed, conn_id);
    assert!(
        ink(&ed, conn_id).is_some(),
        "{name}: no ink rendered after the drop"
    );

    // Phase 2: glue — the endpoint-drop write set: static glue to A's
    // connection point, dynamic glue to B, AddConnect records, the
    // #179 reroute by ID.
    let static_f = dc_ui::connectors::static_glue_formula(1, 2);
    let walk_f = dc_ui::connectors::walkglue_formula(dc_ui::connectors::End::End);
    let mut cmds: Vec<Box<dyn Command>> = Vec::new();
    for (cell, v, f) in [
        ("BeginX", 3.0, static_f.clone()),
        ("BeginY", 2.5, static_f.clone()),
        ("EndX", 8.0, walk_f.clone()),
        ("EndY", 5.5, walk_f.clone()),
    ] {
        cmds.push(Box::new(dc_core::SetGluedCell::new(
            Scope::Shape {
                page: 0,
                shape: conn_id,
            },
            cell,
            Cell {
                v: Some(format!("{v:.6}")),
                f: Some(f),
                ..Cell::default()
            },
        )));
    }
    if static_only {
        // Static end: PAR(PNT) onto B's own connection row — no
        // trigger cells anywhere (the #250 detection surface).
        let f = dc_ui::connectors::static_glue_formula(2, 2);
        for (cell, v) in [("EndX", 8.5), ("EndY", 6.0)] {
            cmds.push(Box::new(dc_core::SetGluedCell::new(
                Scope::Shape {
                    page: 0,
                    shape: conn_id,
                },
                cell,
                Cell {
                    v: Some(format!("{v:.6}")),
                    f: Some(f.clone()),
                    ..Cell::default()
                },
            )));
        }
    } else {
        cmds.push(Box::new(dc_core::SetCell::new(
            Scope::Shape {
                page: 0,
                shape: conn_id,
            },
            "EndTrigger",
            Cell {
                v: Some("2".into()),
                f: Some("GUARD(_XFTRIGGER(Sheet.2!EventXFMod))".into()),
                ..Cell::default()
            },
        )));
    }
    cmds.push(Box::new(dc_core::AddConnect {
        owner: dc_core::ConnectsOwner::Page(0),
        connect: dc_core::Connect::new(conn_id, "BeginX", 9, 1, "Connections.X2", 102),
    }));
    cmds.push(Box::new(dc_core::AddConnect {
        owner: dc_core::ConnectsOwner::Page(0),
        connect: dc_core::Connect::new(
            conn_id,
            "EndX",
            12,
            2,
            (if static_only { "Connections.X2" } else { "PinX" }).to_string(),
            if static_only { 102 } else { 3 },
        ),
    }));
    cmds.push(Box::new(RerouteConnectors::new_including(0, vec![conn_id])));
    let glue = ed.apply(dc_core::Macro::new("glue", cmds));
    let mut out = Exercised {
        lock_refused: glue.is_err(),
        glue_ok: glue.is_ok(),
        ..out
    };
    if glue.is_err() {
        // The refusal rolls the Macro back (transactional) — the
        // connector must be untouched and undo of the drop phase
        // still restores exactly.
        assert_eq!(
            snapshot(&ed, conn_id),
            before_glue,
            "{name}: the refused glue Macro left partial writes"
        );
        return out;
    }

    // Glue-phase plan checks: endpoints on the glue sites, no local
    // geometry rows materialized over the master's formulas.
    let (bx, by, ex, ey) = resolved(&ed, conn_id);
    assert!(
        (bx - 3.0).abs() < 1e-6 && (by - 2.5).abs() < 1e-6,
        "{name}: begin did not land on A's point: ({bx},{by})"
    );
    // Static-only: the end sits on B's row (rect-relative (1.0, 0.5)
    // of the pre-move rect origin (7.5, 5.0)).
    let want = if static_only {
        (8.5, 5.5)
    } else {
        dc_ui::connectors::attach_point((7.5, 5.0, 1.0, 1.0), (3.0, 2.5))
    };
    assert!(
        (ex - want.0).abs() < 0.05 && (ey - want.1).abs() < 0.05,
        "{name}: end did not land on its glue site: ({ex},{ey})"
    );
    let conn = vsdx::model::find_shape_by_id(&ed.doc.pages[0].shapes, conn_id).unwrap();
    assert!(
        conn.sheet.sections_named("Geometry").is_empty(),
        "{name}: the glue phase materialized local geometry rows"
    );
    let after_glue = snapshot(&ed, conn_id);

    // Phase 3: move B by (+2,+1) — the deferred reroute.
    ed.apply(dc_core::Macro::new(
        "move B",
        vec![
            Box::new(MoveShape::new(0, 2, (10.0, 6.5))),
            Box::new(RerouteConnectors::new(0, vec![2])),
        ],
    ))
    .unwrap_or_else(|e| panic!("{name}: the move phase applies: {e}"));
    let (_, _, ex, ey) = resolved(&ed, conn_id);
    // B moved by (+2,+1): rect (9.5,6,1,1), center (10,6.5). The
    // static end lands on B's connection row (pin-relative
    // (0.5, 0.5)).
    let want = if static_only {
        (10.5, 6.5)
    } else {
        dc_ui::connectors::attach_point((9.5, 6.0, 1.0, 1.0), (3.0, 2.5))
    };
    assert!(
        (ex - want.0).abs() < 0.05 && (ey - want.1).abs() < 0.05,
        "{name}: the end did not walk with B: ({ex},{ey})"
    );
    let tracked = ink(&ed, conn_id)
        .map(|b| covers(b, want.0, want.1))
        .unwrap_or(false);
    out.flat_band = !tracked;

    // Phase 4: undo — every phase restores byte-exact.
    ed.undo().expect("move undo");
    assert_eq!(
        snapshot(&ed, conn_id),
        after_glue,
        "{name}: the move phase's undo does not byte-restore"
    );
    ed.undo().expect("glue undo");
    assert_eq!(
        snapshot(&ed, conn_id),
        before_glue,
        "{name}: the glue phase's undo does not byte-restore"
    );
    out
}

fn both_stencils() -> Vec<(String, vsdx::Document)> {
    vec![
        ("US".to_string(), stencil(US)),
        ("metric".to_string(), stencil(METRIC)),
    ]
}

// -- the green assertions ----------------------------------------------

/// Every 1D master drops and renders ink in both stencils (#238's
/// baseline: the stencil surface itself is healthy).
#[test]
fn every_1d_master_drops_and_renders() {
    let mut checked = 0;
    for (label, doc) in both_stencils() {
        for m in &doc.masters {
            let Some(root) = m.shapes.first() else {
                continue;
            };
            if root.sheet.cell("BeginX").is_none() {
                continue;
            }
            let mut ed = Editor::new(fresh_doc(m));
            ed.apply(dc_core::AddMasterInstance::new(0, 1, (5.0, 3.0)))
                .unwrap_or_else(|e| panic!("{label} {}: drop: {e}", m.name_u.clone().unwrap_or_default()));
            assert!(
                ink(&ed, 3).is_some(),
                "{label} {}: no ink after the drop",
                m.name_u.clone().unwrap_or_default()
            );
            checked += 1;
        }
    }
    assert!(checked >= 130, "both stencils swept: {checked}");
}

/// The signed-frame masters + the ObjType=2 Dynamic connector glue,
/// track their targets through a move, and undo byte-exact — the
/// dynamic-connector model working across the whole stencil surface.
/// Chord-frame masters run in their own acceptance test below
/// (#248): the frame classification differs, so the classes stay
/// separate.
#[test]
fn signed_frame_masters_glue_track_and_undo() {
    for (label, doc) in both_stencils() {
        for m in &doc.masters {
            let name = m.name_u.clone().unwrap_or_default();
            let Some(root) = m.shapes.first() else {
                continue;
            };
            if root.sheet.cell("BeginX").is_none() {
                continue;
            }
            let frames = root
                .sheet
                .cell("Width")
                .and_then(|c| c.f.clone())
                .unwrap_or_default();
            if frames.contains("SQRT(") {
                continue; // chord frame: #248's acceptance covers it
            }
            let out = exercise(m);
            assert!(
                out.glue_ok,
                "{label} {name}: glue refused ({})",
                if out.lock_refused { "LockHeight Macro abort" } else { "?" }
            );
            assert!(
                !out.flat_band || KNOWN_FLAT_BAND.iter().any(|(n, _)| *n == name),
                "{label} {name}: ink did not track the moved target"
            );
        }
    }
}

// -- #250: the static-only detection surface --------------------------

/// A trigger-less master glued STATICALLY (`PAR(PNT(...))` on both
/// ends, no ObjType=2, no triggers) is glue-formula-owned: moving the
/// target must reroute it (the caches follow), not strand the
/// endpoint at the pre-move pose (#250).
#[test]
fn static_only_glued_masters_reroute_on_a_move() {
    // Spot masters across the families; Elbow 1 carries no triggers.
    for name in ["Elbow 1", "Side to side 1", "Line-arc connector", "2-line elbow"] {
        let (_, doc) = both_stencils().remove(0);
        let Some(m) = doc.masters.iter().find(|m| m.name_u.as_deref() == Some(name)) else {
            panic!("{name} in the US stencil");
        };
        let out = exercise_mode(m, true);
        assert!(out.glue_ok, "{name}: static-only glue refused");
        assert!(
            !out.flat_band,
            "{name}: the static-only end did not follow the moved target"
        );
    }
}

// -- the class acceptance tests (#248/#249 landed; #250's probe above)

/// #248: the chord-frame masters — every one of them glue (no
/// LockHeight Macro abort) and track the moved target. The drop's
/// derived XForm cells are `F="Inh"` instancing caches (the snapshot
/// fix), the chord frame engages from the master's own formulas, and
/// master-owned Height is never written. Previously 25 refused.
/// refuse outright and the rest render stale/flat ink.
#[test]
fn chord_frame_masters_glue_and_track() {
    let mut refused = Vec::new();
    let mut flat = Vec::new();
    for (label, doc) in both_stencils() {
        for m in &doc.masters {
            let name = m.name_u.clone().unwrap_or_default();
            let Some(root) = m.shapes.first() else {
                continue;
            };
            if root.sheet.cell("BeginX").is_none() {
                continue;
            }
            let frames = root
                .sheet
                .cell("Width")
                .and_then(|c| c.f.clone())
                .or_else(|| root.sheet.cell("Height").and_then(|c| c.f.clone()))
                .unwrap_or_default();
            if !frames.contains("SQRT(") {
                continue;
            }
            let out = exercise(m);
            if out.lock_refused {
                refused.push(format!("{label}/{name}"));
            }
            if out.glue_ok && out.flat_band {
                flat.push(format!("{label}/{name}"));
            }
        }
    }
    assert!(
        refused.is_empty() && flat.is_empty(),
        "chord masters still broken — refuse: {refused:?}, flat ink: {flat:?}"
    );
}

/// #249: the band/knee masters — their Scratch/Controls formulas
/// re-evaluate against the glued endpoints with live cross-row
/// feeds (`Scratch.Y2=ABS(BeginY-EndY)` spans the ink), so they
/// follow the moved target.
#[test]
fn flat_band_masters_track_their_targets() {
    for (label, doc) in both_stencils() {
        for m in &doc.masters {
            let name = m.name_u.clone().unwrap_or_default();
            if !KNOWN_FLAT_BAND.iter().any(|(n, _)| *n == name) {
                continue;
            }
            let out = exercise(m);
            assert!(!out.flat_band, "{label} {name}: ink still does not track");
        }
    }
}


/// #238 (kittest, the #108 GUI pattern): the SURVIVING family — the
/// ObjType=2 Dynamic connector master dropped from the real built-in
/// stencil, both endpoints glued through the endpoint-drag gesture
/// (static to a shape handle, then a shape), the target moved, and
/// the connector's cells following in the same undo steps.
#[test]
fn stencil_master_connector_glues_and_follows_a_move() {
    use dc_ui::canvas::HandleId;

    let mut h = harness_with_blank();
    h.run();
    h.run();
    // The glue side must not interfere; handles glue on.
    let mut s = h.state().debug_snap_settings();
    s.snap_active = false;
    s.glue_active = true;
    s.glue_guides = false;
    s.glue_shape_handles = true;
    s.glue_shape_vertices = false;
    s.glue_connection_points = false;
    h.state_mut().debug_set_snap_for_test(s);
    // The Dynamic connector master is 1D — its placed instance needs
    // endpoint handles, so the XForm1D gesture set must be on too (the
    // Line master's default set).
    h.run();

    // Two rect targets, dragged clear of the canvas center so the
    // connector's drop point stays empty.
    let rect_master = 2;
    let (_, a) = place_from_open(&mut h, StencilId::Document, rect_master);
    let ac = h
        .state()
        .debug_shape_screen_rect_for_test(a)
        .expect("A screen rect")
        .center();
    drag_naturally(&mut h, ac, egui::vec2(-280.0, 0.0));
    h.run();
    let (_, b) = place_from_open(&mut h, StencilId::Document, rect_master);
    let bc = h
        .state()
        .debug_shape_screen_rect_for_test(b)
        .expect("B screen rect")
        .center();
    drag_naturally(&mut h, bc, egui::vec2(280.0, 0.0));
    h.run();
    let ar = h.state().debug_shape_page_rect(a).expect("A rect");

    // Open the built-in Connector stencil from the repo asset (the
    // same file the pane's embedded fallback carries).
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("assets/stencils/connec.vssx");
    h.state_mut().open_stencil_for_test(&path);
    h.run();
    let entries = h.state().debug_stencil_entries();
    let open_ix = entries.len().checked_sub(1).expect("the stencil opened");
    let mid = dyn_connector_master_id();
    let (_, conn) = place_from_open(&mut h, StencilId::Open(open_ix), mid);
    h.run();


    // The placed master instance carries the 1D endpoint handles
    // (ObjType=2 resolved through the chain, BeginX/EndX snapshots).
    let page = h.state().debug_sel().map(|(p, _)| p).expect("selected");
    let (which, handle) = h
        .state()
        .debug_handle_screen(HandleId::LineBegin)
        .map(|p| (("BeginX", "BeginY"), p))
        .or_else(|| {
            h.state()
                .debug_handle_screen(HandleId::LineEnd)
                .map(|p| (("EndX", "EndY"), p))
        })
        .expect("endpoint handle on the placed connector master");

    // Glue that end onto A's right edge middle (page y-up → screen).
    let asr = h
        .state()
        .debug_shape_screen_rect_for_test(a)
        .expect("A screen rect");
    let a_target = egui::pos2(asr.right(), asr.center().y);
    drag_naturally(&mut h, handle, a_target - handle);
    let (ex, ey) = (cell_of(&h, page, conn, which.0), cell_of(&h, page, conn, which.1));
    let want_x = ar.0 + ar.2;
    let want_y = ar.1 + ar.3 / 2.0;
    assert!(
        (ex - want_x).abs() < 0.06 && (ey - want_y).abs() < 0.06,
        "the connector {} glued to A's right edge: ({ex:.4},{ey:.4}) vs ({want_x:.4},{want_y:.4})",
        which.0
    );

    // Glue the OTHER end onto B's left edge middle.
    let other = if which.0 == "BeginX" { "EndX" } else { "BeginX" };
    let other_y = if which.0 == "BeginX" { "EndY" } else { "BeginY" };
    let handle2 = h
        .state()
        .debug_handle_screen(if other == "BeginX" {
            HandleId::LineBegin
        } else {
            HandleId::LineEnd
        })
        .expect("the other endpoint handle");
    let bsr = h
        .state()
        .debug_shape_screen_rect_for_test(b)
        .expect("B screen rect");
    // 2px INSIDE B: the endpoint-drop commit glues a dragged endpoint
    // to a shape only when the drop lands ON it (the #202 halo-off
    // rule) — an epsilon-outside drop releases instead.
    let b_target = egui::pos2(bsr.left() + 2.0, bsr.center().y);
    drag_naturally(&mut h, handle2, b_target - handle2);
    let br = h.state().debug_shape_page_rect(b).expect("B rect");
    let (ox, oy) = (cell_of(&h, page, conn, other), cell_of(&h, page, conn, other_y));
    assert!(
        (ox - br.0).abs() < 0.06 && (oy - (br.1 + br.3 / 2.0)).abs() < 0.06,
        "the connector {other} glued to B's left edge: ({ox:.4},{oy:.4})"
    );

    // Move B down-right; the glued endpoint follows in the same undo
    // step (the deferred RerouteConnectors).
    let bc = h
        .state()
        .debug_shape_screen_rect_for_test(b)
        .expect("B screen rect")
        .center();
    drag_naturally(&mut h, bc, egui::vec2(120.0, 90.0));
    let br2 = h.state().debug_shape_page_rect(b).expect("B rect after move");
    let (mx, my) = (cell_of(&h, page, conn, other), cell_of(&h, page, conn, other_y));
    // The dynamic attach is the facing-border point toward the OTHER
    // endpoint — compute it, don't assume the edge middle.
    let begin = (
        cell_of(&h, page, conn, "BeginX"),
        cell_of(&h, page, conn, "BeginY"),
    );
    let want = dc_ui::connectors::attach_point(br2, begin);
    assert!(
        (mx - want.0).abs() < 0.1 && (my - want.1).abs() < 0.1,
        "the glued endpoint followed B's move: ({mx:.4},{my:.4}) vs attach ({:.4},{:.4})",
        want.0,
        want.1
    );

    // One undo restores B; the endpoint un-follows with it.
    h.key_press_modifiers(egui::Modifiers::CTRL, egui::Key::Z);
    h.run();
    let (ux, uy) = (cell_of(&h, page, conn, other), cell_of(&h, page, conn, other_y));
    assert!(
        (ux - br.0).abs() < 0.1 && (uy - (br.1 + br.3 / 2.0)).abs() < 0.1,
        "undo restores the glued endpoint pose: ({ux:.4},{uy:.4}) vs ({:.4},{:.4})",
        br.0,
        br.1 + br.3 / 2.0
    );
}

/// #251: the page's line-jump switch persists through a save/load
/// round trip (the PageSheet's `LineJumpCode` — 6 = no jumps, 1 =
/// Visio's shown default — and `LineJumpFactorX`), and the default
/// page shows jumps (the shipped corpus carries no jump cells).
#[test]
fn line_jump_settings_persist_through_a_round_trip() {
    let mut ed = Editor::new(vsdx::Document {
        pages: vec![vsdx::model::Page {
            id: 0,
            name: "P".into(),
            part: "pages/page1.xml".into(),
            attrs: vec![("ID".into(), "0".into())],
            children: vec![vsdx::model::IndexChild::Sheet {
                tag: "PageSheet".into(),
                sheet: vsdx::model::Sheet::default(),
            }],
            ..vsdx::model::Page::default()
        }],
        ..vsdx::Document::default()
    });
    assert!(ed.doc.pages[0].line_jumps_shown(), "the default shows");
    ed.apply(dc_core::SetCell::new(
        dc_core::Scope::PageSheet { page: 0 },
        "LineJumpCode",
        vsdx::Cell::value("6"),
    ))
    .unwrap();
    ed.apply(dc_core::SetCell::new(
        dc_core::Scope::PageSheet { page: 0 },
        "LineJumpFactorX",
        vsdx::Cell::value("1.75"),
    ))
    .unwrap();
    assert!(!ed.doc.pages[0].line_jumps_shown());
    assert!((ed.doc.pages[0].line_jump_factor() - 1.75).abs() < 1e-9);
    // One undo restores the shown default.
    ed.undo().unwrap();
    ed.undo().unwrap();
    assert!(ed.doc.pages[0].line_jumps_shown());
}
