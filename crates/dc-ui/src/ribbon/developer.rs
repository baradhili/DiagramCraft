//! Developer tab content (#252 fidelity sweep). Visio group order:
//! **Code · Add-ins · Controls · Shape Design · Stencil · Show/Hide**.
//! ShapeSheet paints once (Visio double-lists it); Macros stays on
//! View only; Add-ins/Controls/Stencil land as planned groups.

use super::{Action, Group, Icon, RibbonButton, TabCtx, big, grid, stack};

pub(crate) fn groups(ctx: &TabCtx) -> Vec<Group> {
    vec![
        Group::new("Code", code_cols(ctx)),
        Group::new("Add-ins", addins_cols()),
        Group::new("Controls", controls_cols()),
        Group::new("Shape Design", shape_design_grid()),
        Group::new("Stencil", stencil_cols()),
        Group::new("Show/Hide", show_hide_grid(ctx)),
    ]
}

fn code_cols(ctx: &TabCtx) -> Vec<super::Col> {
    vec![
        big(
            RibbonButton::new(Icon::ShapeSheet, "ShapeSheet", Action::ShowShapeSheet)
                .selected(ctx.shapesheet_open)
                .tip("Inspect the selected shape's cells"),
        ),
        grid(vec![
            vec![
                RibbonButton::new(Icon::Macro, "", Action::Macros)
                    .alt("Visual Basic")
                    .enabled(false)
                    .tip("VBA arrives with M7 (.vsdm)"),
                RibbonButton::new(Icon::Check, "", Action::Macros)
                    .alt("Macro Security")
                    .enabled(false)
                    .tip("Macro security — M7"),
            ],
        ]),
    ]
}

fn addins_cols() -> Vec<super::Col> {
    vec![stack(vec![
        RibbonButton::new(Icon::Link, "COM Add-Ins", Action::Macros)
            .enabled(false)
            .tip("Add-in hosting arrives with M8"),
        RibbonButton::new(Icon::Shapes, "Add-in Manager", Action::Macros)
            .enabled(false)
            .tip("Add-in manager — M8"),
    ])]
}

fn controls_cols() -> Vec<super::Col> {
    vec![stack(vec![
        RibbonButton::new(Icon::Shapes, "Insert Control", Action::Macros)
            .enabled(false)
            .tip("ActiveX-era — likely cut (scope call on #252)"),
        RibbonButton::new(Icon::Check, "Design Mode", Action::Macros)
            .enabled(false)
            .tip("Design mode — likely cut (scope call on #252)"),
    ])]
}

fn shape_design_grid() -> Vec<super::Col> {
    vec![grid(vec![
        vec![
            RibbonButton::new(Icon::Shapes, "Operations", Action::Macros)
                .enabled(false)
                .tip("Boolean shape operations — planned"),
            RibbonButton::new(Icon::Group, "Behavior", Action::Macros)
                .enabled(false)
                .tip("Shape behavior — planned"),
        ],
        vec![
            RibbonButton::new(Icon::Check, "Protection", Action::Macros)
                .enabled(false)
                .tip("Shape protection — planned"),
        ],
    ])]
}

fn stencil_cols() -> Vec<super::Col> {
    vec![grid(vec![
        vec![
            RibbonButton::new(Icon::Shapes, "Edit Stencil", Action::Macros)
                .enabled(false)
                .tip("Stencil editing — planned (scope call on #252)"),
            RibbonButton::new(Icon::New, "New Stencil", Action::Macros)
                .enabled(false)
                .tip("Create a stencil — planned"),
        ],
        vec![
            RibbonButton::new(Icon::Save, "Save Stencil", Action::Macros)
                .enabled(false)
                .tip("Save the stencil — planned"),
            RibbonButton::new(Icon::Units, "Stencil Settings", Action::Macros)
                .enabled(false)
                .tip("Stencil settings — planned"),
        ],
    ])]
}

fn show_hide_grid(ctx: &TabCtx) -> Vec<super::Col> {
    vec![grid(vec![
        vec![
            // Visio legitimately double-lists ShapeSheet (Code and
            // Show/Hide) — the Show/Hide copy stays the wired toggle.
            RibbonButton::new(Icon::ShapeSheet, "Show ShapeSheet", Action::ShowShapeSheet)
                .selected(ctx.shapesheet_open)
                .tip("Inspect the selected shape's cells"),
            RibbonButton::new(Icon::Units, "mm/in", Action::ToggleUnits)
                .tip("Toggle display units (model stays inches)"),
        ],
        vec![
            RibbonButton::new(Icon::Shapes, "Drawing Explorer", Action::Macros)
                .enabled(false)
                .tip("Drawing explorer — planned"),
            RibbonButton::new(Icon::Shapes, "Show Document Stencil", Action::Macros)
                .enabled(false)
                .tip("Document stencil — planned"),
        ],
    ])]
}
