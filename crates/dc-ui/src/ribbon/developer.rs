//! Developer tab content (#252) — moved verbatim from `app.rs`
//! (commit 1 scaffolding); the per-tab fidelity sweep reshapes this
//! in its own commit.

use super::{Action, Group, Icon, RibbonButton, TabCtx, big, grid, stack};

pub(crate) fn groups(ctx: &TabCtx) -> Vec<Group> {
    vec![
        Group::new(
            "Show/Hide",
            vec![grid(vec![
                vec![
                    RibbonButton::new(Icon::ShapeSheet, "ShapeSheet", Action::ShowShapeSheet)
                        .selected(ctx.shapesheet_open)
                        .tip("Inspect the selected shape's cells"),
                    RibbonButton::new(Icon::Units, "mm/in", Action::ToggleUnits)
                        .tip("Toggle display units (model stays inches)"),
                ],
                vec![
                    RibbonButton::new(Icon::Shapes, "Drawing Explorer", Action::Macros)
                        .enabled(false)
                        .tip("Drawing explorer — planned"),
                ],
            ])],
        ),
        Group::new(
            "Shape Design",
            vec![grid(vec![
                vec![
                    RibbonButton::new(Icon::Shapes, "Operations", Action::Macros)
                        .enabled(false)
                        .tip("Boolean shape operations — planned"),
                    RibbonButton::new(Icon::Group, "Behavior", Action::Macros)
                        .enabled(false)
                        .tip("Shape behavior — planned"),
                ],
                vec![RibbonButton::new(Icon::Check, "Protection", Action::Macros)
                    .enabled(false)
                    .tip("Shape protection — planned")],
            ])],
        ),
        Group::new(
            "Code",
            vec![
                big(
                    RibbonButton::new(Icon::Macro, "Macros", Action::Macros)
                        .enabled(false)
                        .tip("VBA macros arrive with M7 (.vsdm)"),
                ),
                stack(vec![RibbonButton::new(Icon::Link, "Add-ins", Action::Macros)
                    .enabled(false)
                    .tip("Real Visio add-in hosting arrives with M8")]),
            ],
        ),
    ]
}
