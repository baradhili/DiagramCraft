//! Drawing guides and guide points — the #113 guides slice
//! (URS-1076, FS-019): page shapes with `Type='Guide'` dragged out of
//! the rulers, repositioned and deleted, snap and glue targets for
//! the #109 engine. Shape→guide glue persists as Visio's pin
//! formulas (`PinX = Sheet.N!PinX`): the dragged shape writes the
//! formula with a cached value, and moving the guide rewrites every
//! dependent's cached value in the same undo step — a real Visio
//! re-evaluates the formula, and the relationship survives the file
//! round-trip either way.
//!
//! Format notes are clean-room from the shipped corpus (a `Type=
//! 'Guide'` shape with an `InfiniteLine` geometry row in a DWGCNV
//! master): guides are ordinary shapes the model already round-trips;
//! the shapes built here are synthetic (no Microsoft content) — line
//! guides carry Begin/End spanning the page (the XForm1D marker the
//! move path understands), guide points carry pins only.

use vsdx::model::{Cell, Shape};

/// Which ruler a guide came from — and its orientation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuideKind {
    /// From the left (vertical) ruler: constrains x.
    Vertical,
    /// From the top (horizontal) ruler: constrains y.
    Horizontal,
    /// From the ruler corner: constrains both axes.
    Point,
}

/// One guide as the UI sees it, in page inches (y-up).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GuideInfo {
    pub id: i64,
    pub kind: GuideKind,
    /// Vertical guides: the x line coordinate; horizontal: unused
    /// (the y field carries it); points: both.
    pub x: f64,
    pub y: f64,
}

/// `Type='Guide'` — the only shapes the guide machinery treats
/// specially.
pub fn is_guide(s: &Shape) -> bool {
    s.type_.as_deref() == Some("Guide")
}

/// A guide shape's resolved geometry (page inches, y-up): resolved
/// Begin/End when the shape carries them, else its pins (a guide
/// point). `None` for anything unresolvable.
fn endpoints(get: impl Fn(&str) -> Option<f64>) -> Option<((f64, f64), (f64, f64))> {
    match (
        get("BeginX"),
        get("BeginY"),
        get("EndX"),
        get("EndY"),
    ) {
        (Some(bx), Some(by), Some(ex), Some(ey)) => Some(((bx, by), (ex, ey))),
        _ => get("PinX")
            .zip(get("PinY"))
            .map(|(px, py)| ((px, py), (px, py))),
    }
}

/// Classify a guide's geometry (both `GuideKind` discriminators use
/// the 1e-6 the drag layer already treats as degenerate).
pub fn classify(begin: (f64, f64), end: (f64, f64)) -> GuideKind {
    let (dx, dy) = (end.0 - begin.0, end.1 - begin.1);
    if dx.abs() < 1e-6 && dy.abs() < 1e-6 {
        GuideKind::Point
    } else if dx.abs() <= dy.abs() {
        GuideKind::Vertical
    } else {
        GuideKind::Horizontal
    }
}

/// Read one guide shape into [`GuideInfo`] through the caller's cell
/// resolver (the app resolves through the inheritance chain). The
/// stored coordinates are the guide's Begin point: the constrained
/// axis for line guides (the other field is the line's start on the
/// free axis — unused), both for points.
pub fn guide_info(id: i64, s: &Shape, get: impl Fn(&str) -> Option<f64> + Copy) -> Option<GuideInfo> {
    if !is_guide(s) {
        return None;
    }
    let (begin, end) = endpoints(get)?;
    Some(GuideInfo {
        id,
        kind: classify(begin, end),
        x: begin.0,
        y: begin.1,
    })
}

/// The guide closest to `p` within `threshold_in` page inches — the
/// guide's line for line guides (distance on the constrained axis),
/// Euclidean for points. Guides sit above shapes in hit priority
/// (Visio: clicking on a guide grabs it, over any shape).
pub fn guide_hit(
    guides: &[GuideInfo],
    p: (f64, f64),
    threshold_in: f64,
) -> Option<&GuideInfo> {
    let mut best: Option<(&GuideInfo, f64)> = None;
    for g in guides {
        let d = match g.kind {
            GuideKind::Vertical => (g.x - p.0).abs(),
            GuideKind::Horizontal => (g.y - p.1).abs(),
            GuideKind::Point => (g.x - p.0).hypot(g.y - p.1),
        };
        if d <= threshold_in && best.is_none_or(|(_, c)| d < c) {
            best = Some((g, d));
        }
    }
    best.map(|(g, _)| g)
}

/// A synthetic guide shape (clean-room format: ordinary cells, no
/// Microsoft content). Line guides span the page through Begin/End —
/// the XForm1D marker the move path rewrites; guide points carry
/// pins only so they move through the plain pin path.
pub fn guide_shape(id: i64, kind: GuideKind, at: (f64, f64), page: (f64, f64)) -> Shape {
    let mut s = Shape {
        id,
        type_: Some("Guide".into()),
        ..Shape::default()
    };
    let set = |sheet_cell: &mut Vec<vsdx::model::SheetChild>, n: &str, v: f64| {
        sheet_cell.push(vsdx::model::SheetChild::Cell(
            n.to_string(),
            Cell::value(format!("{v:.6}")),
        ));
    };
    let cells = &mut s.sheet.children;
    match kind {
        GuideKind::Vertical => {
            set(cells, "PinX", at.0);
            set(cells, "PinY", page.1 / 2.0);
            set(cells, "Width", 0.0);
            set(cells, "Height", page.1);
            set(cells, "LocPinX", 0.0);
            set(cells, "LocPinY", page.1 / 2.0);
            set(cells, "BeginX", at.0);
            set(cells, "BeginY", 0.0);
            set(cells, "EndX", at.0);
            set(cells, "EndY", page.1);
        }
        GuideKind::Horizontal => {
            set(cells, "PinX", page.0 / 2.0);
            set(cells, "PinY", at.1);
            set(cells, "Width", page.0);
            set(cells, "Height", 0.0);
            set(cells, "LocPinX", page.0 / 2.0);
            set(cells, "LocPinY", 0.0);
            set(cells, "BeginX", 0.0);
            set(cells, "BeginY", at.1);
            set(cells, "EndX", page.0);
            set(cells, "EndY", at.1);
        }
        GuideKind::Point => {
            set(cells, "PinX", at.0);
            set(cells, "PinY", at.1);
            set(cells, "Width", 0.0);
            set(cells, "Height", 0.0);
            set(cells, "LocPinX", 0.0);
            set(cells, "LocPinY", 0.0);
        }
    }
    set(cells, "Angle", 0.0);
    set(cells, "FlipX", 0.0);
    set(cells, "FlipY", 0.0);
    set(cells, "ResizeMode", 0.0);
    s
}

/// The Visio glue formula for one axis of a guide shape.
pub fn glue_formula(guide_id: i64, axis: &str) -> String {
    format!("Sheet.{guide_id}!{axis}")
}

/// Shapes whose pin formulas reference `guide_id` — the guide's glue
/// dependents (moved with the guide, released when it deletes).
/// Returns (shape id, [(cell name, guide axis)]).
pub fn glue_dependents(
    shapes: &[Shape],
    guide_id: i64,
) -> Vec<(i64, Vec<( &'static str, &'static str)>)> {
    let pin_x = format!("Sheet.{guide_id}!PinX");
    let pin_y = format!("Sheet.{guide_id}!PinY");
    let mut out = Vec::new();
    for s in shapes {
        if is_guide(s) {
            continue;
        }
        let mut axes: Vec<(&'static str, &'static str)> = Vec::new();
        for (cell, formula) in [("PinX", &pin_x), ("PinY", &pin_y)] {
            if s
                .sheet
                .cell(cell)
                .and_then(|c| c.f.as_deref())
                .is_some_and(|f| f.contains(formula.as_str()))
            {
                let guide_axis = if cell == "PinX" { "PinX" } else { "PinY" };
                axes.push((cell, guide_axis));
            }
        }
        if !axes.is_empty() {
            out.push((s.id, axes));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get<'s>(s: &'s Shape) -> impl Fn(&str) -> Option<f64> + Copy + 's {
        move |n| s.sheet.cell(n).and_then(|c| c.numeric())
    }

    #[test]
    fn vertical_guide_shape_classifies_and_hits() {
        let s = guide_shape(5, GuideKind::Vertical, (2.5, 0.0), (8.5, 11.0));
        assert!(is_guide(&s));
        let g = guide_info(5, &s, get(&s)).expect("guide info");
        assert_eq!(g.kind, GuideKind::Vertical);
        assert!((g.x - 2.5).abs() < 1e-9);
        let guides = [g];
        // On the line, 0.04 in off, beyond the threshold.
        assert!(guide_hit(&guides, (2.5, 5.0), 0.08).is_some());
        assert!(guide_hit(&guides, (2.54, 5.0), 0.08).is_some());
        assert!(guide_hit(&guides, (2.6, 5.0), 0.08).is_none());
    }

    #[test]
    fn horizontal_and_point_guides_classify() {
        let h = guide_shape(6, GuideKind::Horizontal, (0.0, 3.0), (8.5, 11.0));
        let g = guide_info(6, &h, get(&h)).unwrap();
        assert_eq!(g.kind, GuideKind::Horizontal);
        assert!((g.y - 3.0).abs() < 1e-9);
        // Horizontal guides ignore x when hitting.
        let guides = [g];
        assert!(guide_hit(&guides, (7.9, 3.02), 0.08).is_some());

        let p = guide_shape(7, GuideKind::Point, (4.0, 6.0), (8.5, 11.0));
        let g = guide_info(7, &p, get(&p)).unwrap();
        assert_eq!(g.kind, GuideKind::Point);
        assert!((g.x - 4.0).abs() < 1e-9 && (g.y - 6.0).abs() < 1e-9);
        let guides = [g];
        assert!(guide_hit(&guides, (4.05, 6.05), 0.08).is_some());
        assert!(guide_hit(&guides, (4.05, 6.05), 0.04).is_none());
    }

    #[test]
    fn nearest_guide_wins() {
        let mk = |id, x| GuideInfo {
            id,
            kind: GuideKind::Vertical,
            x,
            y: 0.0,
        };
        let guides = [mk(1, 1.0), mk(2, 2.0)];
        assert_eq!(guide_hit(&guides, (1.9, 0.0), 0.5).map(|g| g.id), Some(2));
    }

    #[test]
    fn glue_dependents_scan_pin_formulas() {
        let mut glued = Shape {
            id: 9,
            ..Shape::default()
        };
        glued.sheet.set_cell(
            "PinX",
            Cell {
                v: Some("2.5".into()),
                f: Some("Sheet.5!PinX".into()),
                ..Cell::default()
            },
        );
        let mut free = Shape {
            id: 10,
            ..Shape::default()
        };
        free.sheet.set_cell("PinX", Cell::value("2.5"));
        let deps = glue_dependents(&[glued, free.clone()], 5);
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0].0, 9);
        assert_eq!(deps[0].1, vec![("PinX", "PinX")]);
        // The wrong guide id matches nothing.
        assert!(glue_dependents(&[free], 6).is_empty());
    }

    #[test]
    fn guide_shape_has_no_geometry_so_layers_skip_it() {
        // Layers cull shapes without ink bounds; a guide carries none.
        let s = guide_shape(5, GuideKind::Vertical, (2.5, 0.0), (8.5, 11.0));
        assert!(s.sheet.sections_named("Geometry").is_empty());
        assert!(s.sheet.text_raw.is_none());
    }
}
