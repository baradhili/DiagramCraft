//! The flattened-group fixture of the #59 cache-refresh tests: master
//! 10 "TaskG" (root group + proportional Body sub + fixed-size GUARD'ed
//! Icon sub) instanced with inline `F='Inh'` snapshot caches, the shape
//! real task masters produce. Shared by the `group_cache_refresh` unit
//! test in `cmd.rs` and the kittest wiring test in
//! `tests/interactions.rs` (#97 §4 relocation: the refresh semantics
//! are tested at the command layer; the harness only proves the resize
//! gesture commits them).

use dc_core::Editor;
use vsdx::model::{Cell, IndexChild, Master, Row, Section, Shape, Sheet, SheetChild};

/// A Cell with optional formula.
pub fn cell(v: &str, f: Option<&str>) -> Cell {
    Cell {
        v: Some(v.to_string()),
        f: f.map(|s| s.to_string()),
        ..Cell::default()
    }
}

/// A named User row with a Value cell.
fn add_user_row(sheet: &mut Sheet, name: &str, v: &str, f: Option<&str>) {
    let row = Row {
        n: Some(name.to_string()),
        cells: vec![("Value".to_string(), cell(v, f))],
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

/// A rectangle Geometry section; row cells carry `f` (F='Inh' for
/// instance snapshots) when given.
fn add_rect_geometry(sheet: &mut Sheet, w: f64, h: f64, f: Option<&str>) {
    let mut rows = Vec::new();
    for (ix, t, (x, y)) in [
        (1, "MoveTo", (0.0, 0.0)),
        (2, "LineTo", (w, 0.0)),
        (3, "LineTo", (w, h)),
        (4, "LineTo", (0.0, h)),
        (5, "LineTo", (0.0, 0.0)),
    ] {
        rows.push(Row {
            t: Some(t.into()),
            ix,
            attrs: vec![("T".into(), t.into()), ("IX".into(), ix.to_string())],
            cells: vec![
                ("X".into(), cell(&format!("{x:.6}"), f)),
                ("Y".into(), cell(&format!("{y:.6}"), f)),
            ],
            ..Row::default()
        });
    }
    sheet
        .children
        .push(SheetChild::Section(Section {
            name: "Geometry".into(),
            ix: 0,
            attrs: vec![("N".into(), "Geometry".into()), ("IX".into(), "0".into())],
            rows,
            ..Section::default()
        }));
}

/// Build the editor: blank document (detached from its source package —
/// the fresh-document write path materializes added masters), master 10
/// instanced once at 1 × 0.5 with inline caches. Returns the editor,
/// the page id and the instance's root/sub/icon shape ids.
pub fn taskg_editor() -> (Editor, i64, [i64; 3]) {
    let mut ed = Editor::new_blank();
    let page_id = ed.doc.pages[0].id;
    ed.doc.package = None;

    let mut mroot = Shape {
        id: 1,
        type_: Some("Group".into()),
        ..Shape::default()
    };
    // The layout cells real task masters use (Process.7 pattern): the
    // Txt block and icon pins hide their size dependence behind User
    // rows — `TxtPinX = GUARD(User.TxtPinX)` where `User.TxtPinX =
    // Width/2`, `PinY = GUARD(Sheet.1!User.IconY)` where `User.IconY =
    // Height - User.InfoPanelHeight/2`. A refresh that only greps
    // formulas for "Width"/"Height" cannot see through the indirection.
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
    let mut mbody = Shape {
        id: 2,
        type_: Some("Shape".into()),
        ..Shape::default()
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
    let mut micon = Shape {
        id: 3,
        type_: Some("Shape".into()),
        ..Shape::default()
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
    ed.doc.masters.push(Master {
        id: 10,
        name_u: Some("TaskG".into()),
        attrs: vec![
            ("ID".into(), "10".into()),
            ("NameU".into(), "TaskG".into()),
            ("Name".into(), "TaskG".into()),
        ],
        children: vec![IndexChild::Rel(vec![(
            "r:id".into(),
            "rId9".into(),
        )])],
        shapes: vec![mroot],
        part: "visio/masters/master10.xml".into(),
        ..Master::default()
    });

    // Instance: 1 x 0.5 (the master is 2 x 1), inline Body sub caches
    // at instance units, Icon caches at its fixed size.
    let root_id = dc_core::commands::next_shape_id(&ed.doc, page_id);
    let mut inst = Shape {
        id: root_id,
        type_: Some("Group".into()),
        master: Some(10),
        ..Shape::default()
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
    // master's formula must still own the value at refresh time.
    add_user_row(&mut inst.sheet, "IconY", "0.4", Some("Inh"));
    let mut isub = Shape {
        id: root_id + 1,
        type_: Some("Shape".into()),
        master: Some(10),
        master_shape: Some(2),
        ..Shape::default()
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
    add_rect_geometry(&mut isub.sheet, 1.0, 0.5, Some("Inh"));
    let mut iicon = Shape {
        id: root_id + 2,
        type_: Some("Shape".into()),
        master: Some(10),
        master_shape: Some(3),
        ..Shape::default()
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
    (ed, page_id, [root_id, root_id + 1, root_id + 2])
}
