//! Data tab content (#252) — moved verbatim from `app.rs` (commit 1
//! scaffolding); the per-tab fidelity sweep reshapes this in its own
//! commit. Everything here is M6-planned.

use super::{Action, Group, Icon, RibbonButton, TabCtx, big, grid, stack};

pub(crate) fn groups(_ctx: &TabCtx) -> Vec<Group> {
    vec![
        Group::new(
            "Display Data",
            vec![
                big(
                    RibbonButton::new(Icon::DataGraphic, "Data Graphic", Action::Macros)
                        .enabled(false)
                        .tip("Data graphics (M6)"),
                ),
                stack(vec![RibbonButton::new(Icon::Shapes, "Legend", Action::Macros)
                    .enabled(false)
                    .tip("Legends (M6)")]),
            ],
        ),
        Group::new(
            "External Data",
            vec![grid(vec![
                vec![
                    RibbonButton::new(Icon::Link, "Link Data to Shapes", Action::Macros)
                        .enabled(false)
                        .tip("Data linking (M6)"),
                    RibbonButton::new(Icon::Refresh, "Refresh Data", Action::Macros)
                        .enabled(false)
                        .tip("Data linking (M6)"),
                ],
                vec![
                    RibbonButton::new(Icon::Open, "Quick Import", Action::Macros)
                        .enabled(false)
                        .tip("Data linking (M6)"),
                ],
            ])],
        ),
        Group::new(
            "Show/Hide",
            vec![stack(vec![
                RibbonButton::new(Icon::ShapeSheet, "Shape Data Window", Action::Macros)
                    .enabled(false)
                    .tip("Shape data window (M6)"),
                RibbonButton::new(Icon::Units, "External Data Window", Action::Macros)
                    .enabled(false)
                    .tip("External data window (M6)"),
            ])],
        ),
    ]
}
