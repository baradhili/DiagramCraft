//! #226: the corpus Line-arc connector's endpoint drag, end to end,
//! against the owner-authored fixture `Sample/test files/utilisation.vsdx`
//! (untracked by design — the test skips when it is absent, like the
//! arrows-gallery fixture pins).
//!
//! The `Line-arc connector` master (master 5) carries NO ObjType — its
//! connector signature is `GlueType` + `BegTrigger`/`EndTrigger` — so
//! every gate here exercises the widened `is_connector_in`. Shape 1021
//! is the Bones → Available Graft File arc (both ends static-glued).

use dc_core::{Command, Scope};
use vsdx::model::find_shape_by_id;

const FIXTURE: &str = "../../../Sample/test files/utilisation.vsdx";

fn open_fixture() -> Option<dc_core::Editor> {
    match vsdx::parse::parse(std::path::Path::new(FIXTURE)) {
        Ok(doc) => Some(dc_core::Editor::new(doc)),
        Err(e) => {
            eprintln!("skipping: fixture not available: {e}");
            None
        }
    }
}

fn conn(ed: &dc_core::Editor, id: i64) -> &vsdx::model::Shape {
    find_shape_by_id(&ed.doc.pages[0].shapes, id).unwrap()
}

fn resolved(ed: &dc_core::Editor, id: i64, name: &str) -> f64 {
    let sh = conn(ed, id);
    let chain = dc_core::scope::shape_chain(&ed.doc, ed.doc.pages[0].id, id).unwrap();
    ed.doc
        .resolve_cell(sh, &chain, name)
        .and_then(|c| c.numeric())
        .unwrap_or_else(|| panic!("{name} resolves"))
}

fn geom_row(ed: &dc_core::Editor, id: i64, ix: i64) -> vsdx::model::Row {
    conn(ed, id)
        .sheet
        .sections_named("Geometry")[0]
        .rows
        .iter()
        .find(|r| r.ix == ix)
        .unwrap()
        .clone()
}

/// The arc's page-space end through the hit walk — the rendered ink's
/// terminus.
fn arc_page_end(ed: &dc_core::Editor, id: i64) -> (f64, f64) {
    let sh = conn(ed, id);
    let chain = dc_core::scope::shape_chain(&ed.doc, ed.doc.pages[0].id, id).unwrap();
    let msize = sh
        .master
        .and_then(|m| ed.doc.master(m))
        .and_then(|m| m.shapes.first())
        .map(|r| {
            (
                ed.doc.resolve_cell(r, &[r], "Width").and_then(|c| c.numeric()).unwrap_or(1.0),
                ed.doc.resolve_cell(r, &[r], "Height").and_then(|c| c.numeric()).unwrap_or(1.0),
            )
        })
        .unwrap_or((1.0, 1.0));
    let frame = dc::geom::hit_frame(&ed.doc, &chain, sh, msize);
    let segs = dc::geom::geometry_hit_segments(sh, None, &frame);
    segs.last().unwrap().1
}

/// The release commit: the dragged END drops off-shape — the endpoint
/// cells go literal (the anchored begin keeps its glue formula, the
/// move_endpoint_cmd ownership fix), the glue record is removed, and
/// the reroute lands the arc's end on the drop point.
#[test]
fn release_commit_lands_the_arc_end_on_the_drop_point() {
    let Some(mut ed) = open_fixture() else { return };
    let page_id = ed.doc.pages[0].id;
    let pre_begin_f = conn(&ed, 1021).sheet.cell("BeginX").unwrap().f.clone();
    assert!(
        pre_begin_f.as_deref().is_some_and(|f| f.contains("PAR(PNT")),
        "the fixture begin is static-glued"
    );

    // The drop: off-shape, below the old end.
    let drop = (7.6, 8.9);
    let scope = Scope::Shape { page: page_id, shape: 1021 };
    for (n, v) in [
        ("BeginX", format!("{:.6}", resolved(&ed, 1021, "BeginX"))),
        ("BeginY", format!("{:.6}", resolved(&ed, 1021, "BeginY"))),
        ("EndX", format!("{:.6}", drop.0)),
        ("EndY", format!("{:.6}", drop.1)),
    ] {
        // The anchored end keeps its formula (the glue engine updates
        // the cache behind it); the dragged end goes literal.
        let f = if n.starts_with("Begin") { pre_begin_f.clone() } else { None };
        dc_core::SetGluedCell::new(
            scope.clone(),
            n,
            vsdx::Cell {
                v: Some(v),
                f,
                ..vsdx::Cell::default()
            },
        )
        .apply(&mut ed.doc)
        .unwrap();
    }
    {
        use dc_core::connects::ConnectsHost;
        let list = ed.doc.pages.iter().find(|p| p.id == page_id).unwrap().connects();
        let idx = list
            .list
            .iter()
            .position(|c| c.from_sheet == 1021 && c.from_cell == "EndX")
            .expect("end glue record");
        dc_core::RemoveConnect::new(dc_core::ConnectsOwner::Page(page_id), idx)
            .apply(&mut ed.doc)
            .unwrap();
    }
    ed.apply(dc_ui::cmd::RerouteConnectors::new_including(page_id, vec![1021]))
        .unwrap();

    // The arc's end IS the drop point.
    let end = arc_page_end(&ed, 1021);
    assert!(
        (end.0 - drop.0).abs() < 1e-6 && (end.1 - drop.1).abs() < 1e-6,
        "the arc end lands on the drop point: ({:.4}, {:.4}) vs ({}, {})",
        end.0,
        end.1,
        drop.0,
        drop.1
    );
    // The chord frame re-derived: width = the new chord, angle = the
    // chord direction — behind their kept markers.
    let chord = (drop.0 - resolved(&ed, 1021, "BeginX")).hypot(drop.1 - resolved(&ed, 1021, "BeginY"));
    let width = conn(&ed, 1021).sheet.cell("Width").unwrap();
    assert!((width.numeric().unwrap() - chord).abs() < 1e-6, "Width {}", width.v.clone().unwrap());
    let angle = conn(&ed, 1021).sheet.cell("Angle").unwrap();
    assert_eq!(angle.f.as_deref(), Some("Inh"), "the Inh marker survives");
    // The anchored end's glue formula survived the drag.
    assert_eq!(
        conn(&ed, 1021).sheet.cell("BeginX").unwrap().f,
        pre_begin_f,
        "the anchored end keeps its glue formula"
    );
    // The sagitta controls stay frame-local.
    let row2 = geom_row(&ed, 1021, 2);
    assert!((row2.cell("A").unwrap().numeric().unwrap() - 1.134719188).abs() < 1e-6);
}

/// The #226 W4 witness on the corpus file: moving the BONES data store
/// drags its glued Line-arc connectors with it (the widened gate lets
/// the reroute see them at all).
#[test]
fn moving_the_target_drags_the_glued_arc() {
    let Some(mut ed) = open_fixture() else { return };
    let page_id = ed.doc.pages[0].id;
    let pre_begin = (resolved(&ed, 1021, "BeginX"), resolved(&ed, 1021, "BeginY"));

    // Move Bones (934) by (+1, -0.5) — a plain pin translation via the
    // move command (the app's group-move path carries the same pin).
    let pin = (resolved(&ed, 934, "PinX"), resolved(&ed, 934, "PinY"));
    ed.apply(dc_ui::cmd::MoveShape::new(page_id, 934, (pin.0 + 1.0, pin.1 - 0.5)))
        .unwrap();
    ed.apply(dc_ui::cmd::RerouteConnectors::new(page_id, vec![934]))
        .unwrap();

    let post_begin = (resolved(&ed, 1021, "BeginX"), resolved(&ed, 1021, "BeginY"));
    assert!(
        (post_begin.0 - pre_begin.0 - 1.0).abs() < 1e-6
            && (post_begin.1 - pre_begin.1 + 0.5).abs() < 1e-6,
        "the arc's begin followed its target: ({:.4}, {:.4}) → ({:.4}, {:.4})",
        pre_begin.0,
        pre_begin.1,
        post_begin.0,
        post_begin.1
    );
    // The glue formula survives the reroute.
    assert!(
        conn(&ed, 1021)
            .sheet
            .cell("BeginX")
            .unwrap()
            .f
            .as_deref()
            .is_some_and(|f| f.contains("PAR(PNT")),
        "the static glue formula stays"
    );
}

/// Preview ≡ commit: `plan_connector_frame` — what the live drag
/// raster paints — applied to a fresh copy of the connector produces
/// the same frame caches and row caches the commit-time reroute
/// writes for the same endpoint pair.
#[test]
fn preview_planner_matches_the_commit_reroute() {
    let Some(ed) = open_fixture() else { return };
    let page_id = ed.doc.pages[0].id;
    let begin = (resolved(&ed, 1021, "BeginX"), resolved(&ed, 1021, "BeginY"));
    let drop = (7.6, 8.9);

    // The commit path: literal endpoint writes (release) + reroute.
    let commit_ed = {
        let mut e = dc_core::Editor::new(ed.doc.clone());
        let scope = Scope::Shape { page: page_id, shape: 1021 };
        for (n, v) in [("EndX", drop.0), ("EndY", drop.1)] {
            dc_core::SetGluedCell::new(
                scope.clone(),
                n,
                vsdx::Cell::value(format!("{v:.6}")),
            )
            .apply(&mut e.doc)
            .unwrap();
        }
        {
            use dc_core::connects::ConnectsHost;
            let list = e.doc.pages.iter().find(|p| p.id == page_id).unwrap().connects();
            if let Some(idx) = list
                .list
                .iter()
                .position(|c| c.from_sheet == 1021 && c.from_cell == "EndX")
            {
                dc_core::RemoveConnect::new(dc_core::ConnectsOwner::Page(page_id), idx)
                    .apply(&mut e.doc)
                    .unwrap();
            }
        }
        e.apply(dc_ui::cmd::RerouteConnectors::new_including(page_id, vec![1021]))
            .unwrap();
        e
    };

    // The preview path: the shared planner applied to a SECOND fresh
    // copy — the same endpoint writes the gesture carries, then the
    // plan's frame/row cache writes. No commands, no reroute.
    let mut preview_ed = dc_core::Editor::new(ed.doc.clone());
    let pre_end = (resolved(&ed, 1021, "EndX"), resolved(&ed, 1021, "EndY"));
    let plan = {
        let page = ed.doc.pages.iter().find(|p| p.id == page_id).unwrap();
        let sh = find_shape_by_id(&page.shapes, 1021).unwrap();
        let chain = dc_core::scope::shape_chain(&ed.doc, page_id, 1021).unwrap();
        dc_ui::cmd::plan_connector_frame(&ed.doc, sh, &chain, begin, drop, pre_end)
    };
    {
        use dc_core::commands::{set_row_cell, RowSel};
        let page = preview_ed.doc.pages.iter_mut().find(|p| p.id == page_id).unwrap();
        let sh = vsdx::model::find_shape_by_id_mut(&mut page.shapes, 1021).unwrap();
        for (n, v) in [("EndX", drop.0), ("EndY", drop.1)] {
            sh.sheet.set_cell(n, vsdx::Cell::value(format!("{v:.6}")));
        }
        for (n, v, _) in &plan.cells {
            sh.sheet.set_cell(n, vsdx::Cell::value(format!("{v:.6}")));
        }
        for (sec_ix, row_ix, name, cell) in &plan.row_cells {
            set_row_cell(
                &mut sh.sheet,
                "Geometry",
                *sec_ix,
                &RowSel::Ix(*row_ix),
                name,
                cell.clone(),
            )
            .unwrap();
        }
    }

    // Frame + row caches agree.
    for n in ["Width", "PinX", "PinY", "LocPinX", "Angle"] {
        let a = conn(&commit_ed, 1021).sheet.cell(n).and_then(|c| c.numeric());
        let b = conn(&preview_ed, 1021).sheet.cell(n).and_then(|c| c.numeric());
        assert_eq!(a, b, "{n} agrees between the preview plan and the commit");
    }
    let commit_row = geom_row(&commit_ed, 1021, 2);
    let preview_row = geom_row(&preview_ed, 1021, 2);
    for n in ["X", "A", "B"] {
        assert_eq!(
            commit_row.cell(n).and_then(|c| c.numeric()),
            preview_row.cell(n).and_then(|c| c.numeric()),
            "row 2 {n} agrees"
        );
    }
}
