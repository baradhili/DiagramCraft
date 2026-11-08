//! #238 investigation harness: exercise EVERY master of a connector
//! stencil through the GUI's drag & glue paths headlessly and report
//! per-master pass/fail with reasons, so failures split into
//! per-family fix issues.
//!
//! Per master (US and metric stencils both):
//!  1. drop — [`dc_core::AddMasterInstance`] (the GUI's stencil drag)
//!  2. glue — the endpoint re-glue write set (static PAR(PNT) glue to
//!     a target's Connection row on shape A; dynamic GUARD(_WALKGLUE)
//!     glue to shape B; AddConnect records; the #179 reroute by ID)
//!  3. move — MoveShape(B) + RerouteConnectors (the deferred reroute)
//!  4. undo every phase, byte-comparing the connector's sheet cells
//!
//! At every phase the harness checks: the connector's resolved
//! endpoints match the glue targets, its ink bounds are non-empty and
//! track the moved shape, and no local Geometry rows were
//! materialized over the master's formula rows.
//!
//! Usage (from `diagramcraft/`):
//!
//! ```text
//! cargo run -p dc-ui --example connec_exercise -- \
//!     crates/dc-ui/assets/stencils/connec.vssx
//! ```

use dc_core::{Command, Editor, Scope};
use dc_ui::cmd::MoveShape;
use dc_ui::cmd::RerouteConnectors;
use vsdx::model::{Cell, Row, Section, Shape, SheetChild};


struct Fail {
    phase: &'static str,
    why: String,
}

static DEBUG: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn dump(ed: &Editor, conn_id: i64, phase: &str) {
    if !DEBUG.load(std::sync::atomic::Ordering::Relaxed) {
        return;
    }
    let Some(conn) = vsdx::model::find_shape_by_id(&ed.doc.pages[0].shapes, conn_id) else {
        return;
    };
    let chain = dc_core::scope::shape_chain(&ed.doc, 0, conn_id).unwrap_or_default();
    let g = |n: &str| {
        ed.doc
            .resolve_cell(conn, &chain, n)
            .map(|c| (c.v.clone(), c.f.clone()))
    };
    println!("  -- {phase}");
    for n in ["BeginX", "BeginY", "EndX", "EndY", "PinX", "PinY", "Width", "Height", "LocPinX", "LocPinY", "Angle"] {
        if let Some((v, f)) = g(n) {
            println!("     {n:8} v={v:?} f={f:?}");
        } else {
            println!("     {n:8} (inherited)");
        }
    }
    let src_ = ed.doc.instance_source(conn, &chain);
    if let Some(m) = src_ {
        let gm = |n: &str| m.sheet.cell(n).map(|c| (c.v.clone(), c.f.clone()));
        for n in ["Width", "Height", "LocPinX", "LocPinY", "Angle"] {
            if let Some((v, f)) = gm(n) {
                println!("     master.{n:8} v={v:?} f={f:?}");
            }
        }
    }
    println!("     ink={:?}", ink(ed, conn_id));
}

fn main() -> anyhow::Result<()> {
    let path = std::env::args_os()
        .nth(1)
        .expect("usage: connec_exercise <stencil.vssx>");
    let stencil = vsdx::parse::parse(std::path::Path::new(&path))?;
    let mut pass = 0usize;
    let mut fails = 0usize;
    let debug: Vec<String> = std::env::var("DEBUG_MASTERS")
        .unwrap_or_default()
        .split(',')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();
    DEBUG.store(!debug.is_empty(), std::sync::atomic::Ordering::Relaxed);
    for m in &stencil.masters {
        let name = m.name_u.clone().unwrap_or_else(|| "?".into());
        match exercise(m) {
            Ok(()) => pass += 1,
            Err(fails_) => {
                fails += 1;
                println!("FAIL {:40} {}", name, fails_.iter().map(|f| format!("[{}] {}", f.phase, f.why)).collect::<Vec<_>>().join(" "));
            }
        }
    }
    println!("\n== {pass} pass, {fails} fail ==");
    if !debug.is_empty() {
        for name in &debug {
            if let Some(m) = stencil.masters.iter().find(|m| m.name_u.as_deref() == Some(name.as_str())) {
                println!("--- debug {name} ---");
                if let Err(fs) = exercise(m) {
                    for f_ in fs { println!("  [{}] {}", f_.phase, f_.why); }
                } else {
                    println!("  all phases pass");
                }
            }
        }
    }
    Ok(())
}

/// Two glue targets: shape A (id 1) with a Connection row at its right
/// edge middle, shape B (id 2) plain. The connector instance drops at
/// (5, 3), glues begin→A's point statically, end→B dynamically; then
/// B moves by (+2, +1).
fn exercise(m: &vsdx::Master) -> Result<(), Vec<Fail>> {
    let doc = fresh_doc(m);
    let mut ed = Editor::new(doc);
    let mut f: Vec<Fail> = Vec::new();

    // Phase 1: drop. Non-1D masters (the Star) are not glue
    // connectors — skipped with a note, not a failure.
    if m.shapes.first().is_some_and(|s| s.sheet.cell("BeginX").is_none()) {
        return Ok(());
    }
    ed.apply(dc_core::AddMasterInstance::new(0, 1, (5.0, 3.0)))
        .map_err(|e| vec![Fail { phase: "drop", why: format!("apply: {e}") }])?;
    let before_drop = snapshot(&ed, 3);
    let conn_id = vsdx::model::find_shape_by_id(&ed.doc.pages[0].shapes, 3)
        .map(|s| s.id)
        .ok_or_else(|| vec![Fail { phase: "drop", why: "instance missing".into() }])?;
    let (Some(bx), Some(by), Some(ex), Some(ey)) = resolved(&ed, conn_id)
    else {
        f.push(Fail {
            phase: "drop",
            why: "BeginX/BeginY/EndX/EndY do not resolve on the instance".into(),
        });
        return Err(f);
    };
    if ((ex - bx) - 1.0).abs() > 1e-6 || (ey - by).abs() > 1e-6 {
        f.push(Fail {
            phase: "drop",
            why: format!("1D snapshot not the horizontal 1-inch line: begin ({bx},{by}) end ({ex},{ey})"),
        });
    }
    if ink(&ed, conn_id).is_none() {
        f.push(Fail {
            phase: "drop",
            why: "no ink rendered after the drop".into(),
        });
    }
    dump(&ed, conn_id, "drop");

    // Phase 2: glue — the endpoint-drop write set (app.rs glue commit).
    // Begin → shape A's Connection row (static), End → shape B (dynamic).
    // STATIC_ONLY=1 probes the #238 comment-4 gap: both ends static
    // (PAR(PNT)), NO trigger cells — does a move still reroute?
    let static_only = std::env::var("STATIC_ONLY").is_ok();
    let static_f = dc_ui::connectors::static_glue_formula(1, 2);
    let walk_f = dc_ui::connectors::walkglue_formula(dc_ui::connectors::End::End);
    let mut cmds: Vec<Box<dyn Command>> = Vec::new();
    for (cell, v, f_) in [
        ("BeginX", 3.0, static_f.clone()),
        ("BeginY", 2.5, static_f.clone()),
        ("EndX", 8.0, walk_f.clone()),
        ("EndY", 5.5, walk_f.clone()),
    ] {
        cmds.push(Box::new(dc_core::SetGluedCell::new(
            Scope::Shape { page: 0, shape: conn_id },
            cell,
            Cell {
                v: Some(format!("{v:.6}")),
                f: Some(f_),
                ..Cell::default()
            },
        )));
    }
    // Triggers: the dynamic end needs one; write both like the GUI
    // does when the glue kind is dynamic (static ends carry none).
    if !static_only {
        cmds.push(Box::new(dc_core::SetCell::new(
            Scope::Shape { page: 0, shape: conn_id },
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
    if static_only {
        // Static end: PAR(PNT) onto B's own connection row 2.
        cmds.push(Box::new(dc_core::SetGluedCell::new(
            Scope::Shape { page: 0, shape: conn_id },
            "EndX",
            Cell {
                v: Some("9.500000".into()),
                f: Some(dc_ui::connectors::static_glue_formula(2, 2)),
                ..Cell::default()
            },
        )));
        cmds.push(Box::new(dc_core::SetGluedCell::new(
            Scope::Shape { page: 0, shape: conn_id },
            "EndY",
            Cell {
                v: Some("6.500000".into()),
                f: Some(dc_ui::connectors::static_glue_formula(2, 2)),
                ..Cell::default()
            },
        )));
    }
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
    // Give B a connection row so the static end has a target.
    {
        let page = ed.doc.pages.iter_mut().find(|p| p.id == 0).unwrap();
        let b = vsdx::model::find_shape_by_id_mut(&mut page.shapes, 2).unwrap();
        let mut sec = Section { name: "Connection".into(), ix: 0, ..Section::default() };
        sec.rows.push(Row {
            ix: 2,
            cells: vec![
                ("X".to_string(), Cell::value("0.5")),
                ("Y".to_string(), Cell::value("0.5")),
            ],
            ..Row::default()
        });
        b.sheet.children.push(SheetChild::Section(sec));
    }
    cmds.push(Box::new(RerouteConnectors::new_including(0, vec![conn_id])));
    ed.apply(dc_core::Macro::new("glue", cmds))
        .map_err(|e| vec![Fail { phase: "glue", why: format!("apply: {e}") }])?;
    check_glue(&ed, conn_id, &mut f);
    dump(&ed, conn_id, "glue");
    let after_glue = snapshot(&ed, conn_id);

    // Phase 3: move shape B by (+2, +1) — the deferred reroute.
    ed.apply(dc_core::Macro::new(
        "move B",
        vec![
            Box::new(MoveShape::new(0, 2, (10.0, 6.5))),
            Box::new(RerouteConnectors::new(0, vec![2])),
        ],
    ))
    .map_err(|e| vec![Fail { phase: "move", why: format!("apply: {e}") }])?;
    check_move(&ed, conn_id, &mut f);
    dump(&ed, conn_id, "move");

    // Phase 4: undo — the glue and move phases must restore exactly.
    ed.undo().map_err(|e| vec![Fail { phase: "undo", why: format!("move undo: {e}") }])?;
    let after_move_undo = snapshot(&ed, conn_id);
    if after_move_undo != after_glue {
        f.push(Fail {
            phase: "undo",
            why: "the move phase's undo does not byte-restore the connector cells".into(),
        });
    }
    ed.undo().map_err(|e| vec![Fail { phase: "undo", why: format!("glue undo: {e}") }])?;
    let after_glue_undo = snapshot(&ed, conn_id);
    if after_glue_undo != before_drop {
        let d = first_diff(&after_glue_undo, &before_drop);
        f.push(Fail {
            phase: "undo",
            why: format!("the glue phase's undo does not byte-restore the connector cells: {d}"),
        });
    }

    if f.is_empty() {
        Ok(())
    } else {
        Err(f)
    }
}

/// The endpoint expectations after glue: begin on A's right edge
/// middle (3, 2.5), end on B's facing border (8, 5.5).
fn check_glue(ed: &Editor, conn_id: i64, f: &mut Vec<Fail>) {
    let (Some(bx), Some(by), Some(ex), Some(ey)) = resolved(ed, conn_id) else {
        f.push(Fail { phase: "glue", why: "endpoints stopped resolving".into() });
        return;
    };
    if ((bx - 3.0).abs() > 1e-6) || ((by - 2.5).abs() > 1e-6) {
        f.push(Fail { phase: "glue", why: format!("begin did not land on A's point: ({bx:.3},{by:.3}) ≠ (3,2.5)") });
    }
    // B's rect (7.5,5,1,1), center (8,5.5); the facing border point
    // toward the begin (3,2.5).
    let want = dc_ui::connectors::attach_point((7.5, 5.0, 1.0, 1.0), (3.0, 2.5));
    if ((ex - want.0).abs() > 0.05) || ((ey - want.1).abs() > 0.05) {
        f.push(Fail { phase: "glue", why: format!("end did not land on B's facing side: ({ex:.3},{ey:.3}) ≉ ({:.3},{:.3})", want.0, want.1) });
    }
    if ink(ed, conn_id).is_none() {
        f.push(Fail { phase: "glue", why: "no ink rendered after glue".into() });
    }
    if let Some(b) = ink(ed, conn_id) {
        // The ink must COVER both glue sites: A's point (3,2.5) and
        // B's facing side (8,5.5) — y-up inches vs the y-down raster
        // box; page height 11.
        let cov = |x: f64, y: f64| b.0 - 0.15 <= x && x <= b.0 + b.2 + 0.15 && (11.0 - b.1 - b.3) - 0.15 <= y && y <= 11.0 - b.1 + 0.15;
        let want_end = dc_ui::connectors::attach_point((7.5, 5.0, 1.0, 1.0), (3.0, 2.5));
        if !cov(3.0, 2.5) || !cov(want_end.0, want_end.1) {
            f.push(Fail { phase: "glue", why: format!("ink bounds {b:?} (y-down px) do not cover both glue sites") });
        }
    }
    // Route preservation: no LOCAL geometry rows materialized by the
    // glue phase — formula rows live on the master; a local copy means
    // the reroute wrote literals over inherited formulas.
    let conn = vsdx::model::find_shape_by_id(&ed.doc.pages[0].shapes, conn_id).unwrap();
    let local_rows: Vec<String> = conn
        .sheet
        .sections_named("Geometry")
        .iter()
        .map(|g| g.rows.iter().filter_map(|r| r.t.clone()).collect::<Vec<_>>().join(","))
        .collect();
    if !local_rows.is_empty() {
        f.push(Fail { phase: "glue", why: format!("local geometry rows materialized: {local_rows:?}") });
    }
}

/// After the move: the dynamic end walks with B (+2,+1 → 8,5.5 →
/// 10,6.5 facing side), the ink tracks it.
fn check_move(ed: &Editor, conn_id: i64, f: &mut Vec<Fail>) {
    let (Some(_), Some(_), Some(ex), Some(ey)) = resolved(ed, conn_id) else {
        f.push(Fail { phase: "move", why: "endpoints stopped resolving after move".into() });
        return;
    };
    // B moved by (+2,+1): rect (9.5,6,1,1), center (10,6.5).
    let want = if std::env::var("STATIC_ONLY").is_ok() {
        // Static glue: B's row sits at pin-relative (0.5, 0.5) → page (10, 7).
        (10.0, 7.0)
    } else {
        dc_ui::connectors::attach_point((9.5, 6.0, 1.0, 1.0), (3.0, 2.5))
    };
    if ((ex - want.0).abs() > 0.05) || ((ey - want.1).abs() > 0.05) {
        f.push(Fail { phase: "move", why: format!("dynamic end did not walk with B: ({ex:.3},{ey:.3}) ≉ ({:.3},{:.3})", want.0, want.1) });
    }
    if ink(ed, conn_id).is_none() {
        f.push(Fail { phase: "move", why: "no ink rendered after move".into() });
    } else if let Some(b) = ink(ed, conn_id) {
        let cov = |x: f64, y: f64| b.0 - 0.15 <= x && x <= b.0 + b.2 + 0.15 && (11.0 - b.1 - b.3) - 0.15 <= y && y <= 11.0 - b.1 + 0.15;
        let want = dc_ui::connectors::attach_point((9.5, 6.0, 1.0, 1.0), (3.0, 2.5));
        if !cov(want.0, want.1) {
            f.push(Fail { phase: "move", why: format!("ink bounds {b:?} do not follow the moved end") });
        }
    }
}

// ---------------------------------------------------------------------
// Fixture plumbing
// ---------------------------------------------------------------------

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
        let mut sec = Section { name: "Connection".into(), ix: 0, ..Section::default() };
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

fn fresh_doc(m: &vsdx::Master) -> vsdx::Document {
    let mut master = m.clone();
    master.id = 1;
    let mut doc = vsdx::Document {
        pages: vec![vsdx::model::Page {
            id: 0,
            name: "P1".into(),
            part: "pages/page1.xml".into(),
            attrs: vec![
                ("ID".into(), "0".into()),
                ("NameU".into(), "P1".into()),
                // Letter page so the y-down conversion is fixed.
                ("PageWidth".into(), "8.5".into()),
                ("PageHeight".into(), "11".into()),
            ],
            shapes: vec![rect(1, 2.0, 2.0, true), rect(2, 7.5, 5.0, false)],
            ..vsdx::model::Page::default()
        }],
        masters: vec![master],
        ..vsdx::Document::default()
    };
    doc.masters[0].id = 1;
    doc
}

/// The connector's resolved endpoint cells.
fn resolved(ed: &Editor, conn_id: i64) -> (Option<f64>, Option<f64>, Option<f64>, Option<f64>) {
    let Some(conn) = vsdx::model::find_shape_by_id(&ed.doc.pages[0].shapes, conn_id) else {
        return (None, None, None, None);
    };
    let Ok(chain) = dc_core::scope::shape_chain(&ed.doc, 0, conn_id) else {
        return (None, None, None, None);
    };
    let g = |n: &str| ed.doc.resolve_cell(conn, &chain, n).and_then(|c| c.numeric());
    (g("BeginX"), g("BeginY"), g("EndX"), g("EndY"))
}

/// The connector's ink bounds in page pixels (y-down), if any ink.
fn ink(ed: &Editor, conn_id: i64) -> Option<(f64, f64, f64, f64)> {
    let page = &ed.doc.pages[0];
    dc::svg::shape_ink_bounds(&ed.doc, page, conn_id)
}

/// Byte snapshot of the connector's own sheet cells.
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

fn first_diff(a: &[(String, Option<String>, Option<String>)], b: &[(String, Option<String>, Option<String>)]) -> String {
    for (x, y) in a.iter().zip(b.iter()) {
        if x != y {
            return format!("{x:?} vs {y:?}");
        }
    }
    format!("length {} vs {}", a.len(), b.len())
}
