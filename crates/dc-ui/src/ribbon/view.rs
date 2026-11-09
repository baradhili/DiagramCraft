//! View tab content (#252) — moved verbatim from `app.rs` (commit 1
//! scaffolding); the per-tab fidelity sweep reshapes this in its own
//! commit.

use super::{Action, Group, Icon, RibbonButton, TabCtx, big, grid, stack};

pub(crate) fn groups(ctx: &TabCtx) -> Vec<Group> {
    vec![
        Group::new(
            "Show",
            vec![grid(vec![
                vec![
                    RibbonButton::new(Icon::Rulers, "Rulers", Action::ToggleRulers)
                        .selected(ctx.show_rulers),
                    RibbonButton::new(Icon::Grid, "Grid", Action::ToggleGrid)
                        .selected(ctx.show_grid),
                ],
                vec![
                    RibbonButton::new(Icon::Comment, "Comments", Action::ToggleComments)
                        .selected(ctx.show_comments),
                    RibbonButton::new(Icon::Shapes, "Connection Points", Action::Macros)
                        .enabled(false)
                        .tip("Connection points — planned"),
                    RibbonButton::new(Icon::Shapes, "Guides", Action::Macros)
                        .enabled(false)
                        .tip("Guides — planned"),
                ],
            ])],
        ),
        Group::new(
            "Zoom",
            vec![
                stack(vec![
                    RibbonButton::new(Icon::ZoomIn, "Zoom In", Action::ZoomIn),
                    RibbonButton::new(Icon::ZoomOut, "Zoom Out", Action::ZoomOut),
                ]),
                big(RibbonButton::new(Icon::Fit, "Fit Window", Action::FitPage)),
                big(
                    RibbonButton::new(Icon::ZoomSelection, "Selection", Action::ZoomSelection)
                        .tip("Zoom to selection (Shift+Z)"),
                ),
                big(RibbonButton::new(Icon::ZoomIn, "100%", Action::Zoom100)),
            ],
        ),
        Group::new(
            "Visual Aids",
            vec![
                stack(vec![RibbonButton::new(
                    Icon::Grid,
                    "Dynamic Grid",
                    Action::ToggleDynamicGrid,
                )
                .selected(ctx.dynamic_grid)
                .tip("Alignment and spacing hints while dragging")]),
                big(
                    RibbonButton::new(Icon::Magnet, "Snap & Glue", Action::SnapGlue)
                        .selected(ctx.snap_dlg_open)
                        .tip("Configure snap and glue targets"),
                ),
            ],
        ),
        Group::new(
            "Macros",
            vec![big(RibbonButton::new(Icon::Macro, "Macros", Action::Macros))],
        ),
    ]
}
