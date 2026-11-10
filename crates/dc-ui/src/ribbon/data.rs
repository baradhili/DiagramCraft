//! Data tab content (#252 fidelity sweep). Visio group order:
//! **External Data · Shape Data · Data Graphics · Display Options**
//! (everything M6-planned). Legend left for Insert ▸ Diagram Parts;
//! the two window toggles fold into their owning groups.

use super::{Action, Group, Icon, RibbonButton, TabCtx, big, grid, stack};

pub(crate) fn groups(_ctx: &TabCtx) -> Vec<Group> {
    vec![
        Group::new("External Data", external_data_cols()),
        Group::new("Shape Data", shape_data_cols()),
        Group::new("Data Graphics", data_graphics_cols()),
        Group::new("Display Options", display_options_cols()),
    ]
}

fn external_data_cols() -> Vec<super::Col> {
    vec![
        big(
            RibbonButton::new(Icon::Open, "Quick Import", Action::Macros)
                .enabled(false)
                .tip("Data linking (M6)"),
        ),
        grid(vec![
            vec![
                RibbonButton::new(Icon::Link, "", Action::Macros)
                    .alt("Link Data to Shapes")
                    .enabled(false)
                    .tip("Data linking (M6)"),
                RibbonButton::new(Icon::Refresh, "", Action::Macros)
                    .alt("Refresh All")
                    .enabled(false)
                    .tip("Data linking (M6)"),
            ],
            vec![
                RibbonButton::new(Icon::Shapes, "", Action::Macros)
                    .alt("Custom Import")
                    .enabled(false)
                    .tip("Data linking (M6)"),
                RibbonButton::new(Icon::Magnet, "", Action::Macros)
                    .alt("Automatically Link")
                    .enabled(false)
                    .tip("Data linking (M6)"),
            ],
        ]),
        stack(vec![RibbonButton::new(
            Icon::Units,
            "External Data Window",
            Action::Macros,
        )
        .enabled(false)
        .tip("External data window (M6)")]),
    ]
}

fn shape_data_cols() -> Vec<super::Col> {
    vec![
        big(
            RibbonButton::new(Icon::ShapeSheet, "Shape Data Window", Action::Macros)
                .enabled(false)
                .tip("Shape data window (M6)"),
        ),
        stack(vec![RibbonButton::new(
            Icon::Field,
            "Define Shape Data",
            Action::Macros,
        )
        .enabled(false)
        .tip("Define shape data (M6)")]),
    ]
}

fn data_graphics_cols() -> Vec<super::Col> {
    vec![
        big(
            RibbonButton::new(Icon::DataGraphic, "Data Graphics", Action::Macros)
                .enabled(false)
                .tip("Data graphics (M6)"),
        ),
        grid(vec![
            vec![
                RibbonButton::new(Icon::Effects, "", Action::Macros)
                    .alt("Create New Data Graphic")
                    .enabled(false)
                    .tip("Data graphics (M6)"),
                RibbonButton::new(Icon::Pen, "", Action::Macros)
                    .alt("Edit Data Graphic")
                    .enabled(false)
                    .tip("Data graphics (M6)"),
            ],
        ]),
    ]
}

fn display_options_cols() -> Vec<super::Col> {
    vec![stack(vec![RibbonButton::new(
        Icon::Check,
        "Display Data Graphics",
        Action::Macros,
    )
    .enabled(false)
    .tip("Show/hide data graphics on the page (M6)")])]
}
