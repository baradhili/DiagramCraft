//! `TouchList` classification and merge units (#97 §6 item 4): the #93
//! invalidation surface the canvas composites on. A wrong class or a
//! broken merge means stale layer textures — silent visual corruption
//! no golden reliably catches — so the lattice is pinned directly at
//! the unit layer. (The aggregate behavior — a move commit re-renders
//! zero layers — is guarded in dc-ui's render_perf/raster_async gates.)

use dc_core::commands::{AddShape, SetCell, SetRowCell, SetText};
use dc_core::commands::RowSel;
use dc_core::{Editor, Scope, Touched, Touch, TouchList};

fn doc_with_shape() -> Editor {
    let mut shape = vsdx::model::Shape {
        id: 1,
        type_: Some("Shape".into()),
        ..vsdx::model::Shape::default()
    };
    shape.sheet.set_cell("PinX", vsdx::model::Cell::value("1.5"));
    shape.sheet.set_cell("PinY", vsdx::model::Cell::value("2.5"));
    shape.sheet.set_cell("Width", vsdx::model::Cell::value("2"));
    let page = vsdx::model::Page {
        id: 0,
        name: "P1".into(),
        attrs: vec![("ID".into(), "0".into()), ("NameU".into(), "P1".into())],
        shapes: vec![shape],
        ..vsdx::model::Page::default()
    };
    Editor::new(vsdx::Document {
        pages: vec![page],
        ..vsdx::Document::default()
    })
}

fn touched_of<C: dc_core::Command>(ed: &Editor, cmd: &C) -> TouchList {
    cmd.touched(&ed.doc)
}

#[test]
fn top_level_pin_writes_are_pure_moves_everything_else_is_ink() {
    let ed = doc_with_shape();
    let shape_scope = Scope::Shape { page: 0, shape: 1 };

    // PinX/PinY on the top-level shape: the texture is
    // translation-invariant — only the quad moves.
    for cell in ["PinX", "PinY"] {
        let t = touched_of(
            &ed,
            &SetCell::new(
                shape_scope.clone(),
                cell,
                vsdx::model::Cell::value("3"),
            ),
        );
        assert_eq!(t.shapes, vec![Touched { page: 0, shape: 1, touch: Touch::Move }], "{cell}");
    }

    // Any other cell on the same shape changes ink within its bounds.
    for cell in ["Width", "FillForegnd", "Angle"] {
        let t = touched_of(
            &ed,
            &SetCell::new(
                shape_scope.clone(),
                cell,
                vsdx::model::Cell::value("1"),
            ),
        );
        assert_eq!(t.shapes[0].touch, Touch::Ink, "{cell}");
    }

    // Text edits are ink.
    let t = touched_of(&ed, &SetText::new(shape_scope.clone(), "hello"));
    assert_eq!(t.shapes[0].touch, Touch::Ink);

    // A geometry-row cell is ink.
    let t = touched_of(
        &ed,
        &SetRowCell::new(shape_scope, "Geometry", 0, RowSel::Ix(1), "X", vsdx::model::Cell::value("2")),
    );
    assert_eq!(t.shapes[0].touch, Touch::Ink);
}

#[test]
fn page_sheet_and_structural_edits_fall_back_to_whole_page_dirty() {
    let ed = doc_with_shape();
    // Page-sheet geometry dirties the whole page conservatively.
    let t = touched_of(
        &ed,
        &SetCell::new(
            Scope::PageSheet { page: 0 },
            "PageWidth",
            vsdx::model::Cell::value("11"),
        ),
    );
    assert_eq!(t, TouchList::page(0));

    // Membership: a shape entering the page is a precise layer-set
    // change on that shape — not ink, not a page fallback.
    let mut shape = vsdx::model::Shape {
        id: 2,
        type_: Some("Shape".into()),
        ..vsdx::model::Shape::default()
    };
    shape.sheet.set_cell("PinX", vsdx::model::Cell::value("1"));
    let t = touched_of(&ed, &AddShape { page: 0, parent: None, shape });
    assert_eq!(
        t,
        TouchList {
            pages: vec![],
            shapes: vec![Touched { page: 0, shape: 2, touch: Touch::Membership }],
        }
    );
}

#[test]
fn merge_unions_per_shape_with_the_strongest_touch_winning() {
    let mut base = TouchList {
        pages: vec![0],
        shapes: vec![Touched { page: 0, shape: 1, touch: Touch::Move }],
    };
    // Move + Ink on the same shape → Ink (an ink re-render subsumes).
    base.merge(TouchList {
        pages: vec![],
        shapes: vec![Touched { page: 0, shape: 1, touch: Touch::Ink }],
    });
    assert_eq!(base.shapes[0].touch, Touch::Ink);

    // Ink then Move stays Ink — a pure move never DOWNGRADES to skip a
    // needed re-render.
    base.merge(TouchList {
        pages: vec![],
        shapes: vec![Touched { page: 0, shape: 1, touch: Touch::Move }],
    });
    assert_eq!(base.shapes[0].touch, Touch::Ink);

    // Different shapes accumulate; pages union + dedup.
    base.merge(TouchList {
        pages: vec![0, 1],
        shapes: vec![Touched { page: 0, shape: 2, touch: Touch::Move }],
    });
    assert_eq!(base.shapes.len(), 2);
    assert_eq!(base.pages, vec![0, 1], "page ids sort + dedup");
    base.merge(TouchList::page(0));
    assert_eq!(base.pages, vec![0, 1], "duplicate page id dropped");
}

#[test]
fn editor_apply_and_undo_route_their_touches() {
    // The wiring contract: apply/undo/redo return the command's
    // TouchList so the canvas can invalidate precisely (#93's seam).
    let mut ed = doc_with_shape();
    let (_, touched) = ed
        .apply(SetCell::new(
            Scope::Shape { page: 0, shape: 1 },
            "PinX",
            vsdx::model::Cell::value("9"),
        ))
        .unwrap();
    assert_eq!(touched.shapes, vec![Touched { page: 0, shape: 1, touch: Touch::Move }]);

    // Undo restores the old pin — itself a pure translation, so the
    // reverse touch is also a Move (undo routes the command's own
    // classification, not a blanket ink).
    let (_, touched) = ed.undo().unwrap().unwrap();
    assert_eq!(touched.shapes[0].touch, Touch::Move);

    let (_, touched) = ed.redo().unwrap().unwrap();
    assert_eq!(touched.shapes[0].touch, Touch::Move);
}
