//! View tab content (#252 fidelity sweep). Visio group order:
//! **Views · Show · Visual Aids · Zoom · Window · Macros**. Visual
//! Aids moves ahead of Zoom and absorbs Connection Points from Show;
//! "Fit Window" renames to Visio's Whole Page; the new Window group
//! is placeholder (Switch Windows maps to the strip's #192 switcher
//! eventually).

use super::{Action, Group, Icon, RibbonButton, TabCtx, big, grid, stack};

pub(crate) fn groups(ctx: &TabCtx) -> Vec<Group> {
    vec![
        Group::new("Views", views_cols()),
        Group::new("Show", show_grid(ctx)),
        Group::new("Visual Aids", visual_aids_cols(ctx)),
        Group::new("Zoom", zoom_cols()),
        Group::new("Window", window_cols()),
        Group::new(
            "Macros",
            vec![big(RibbonButton::new(Icon::Macro, "Macros", Action::Macros))],
        ),
    ]
}

fn views_cols() -> Vec<super::Col> {
    vec![big(
        RibbonButton::new(Icon::Fit, "Presentation Mode", Action::Macros)
            .enabled(false)
            .tip("Presentation mode — planned"),
    )]
}

fn show_grid(ctx: &TabCtx) -> Vec<super::Col> {
    vec![grid(vec![
        vec![
            RibbonButton::new(Icon::Rulers, "Rulers", Action::ToggleRulers)
                .selected(ctx.show_rulers),
            RibbonButton::new(Icon::Grid, "Grid", Action::ToggleGrid)
                .selected(ctx.show_grid),
        ],
        vec![
            RibbonButton::new(Icon::Shapes, "Guides", Action::Macros)
                .enabled(false)
                .tip("Guides — planned (#113's objects exist; the toggle is planned)"),
            RibbonButton::new(Icon::Delete, "Page Breaks", Action::Macros)
                .enabled(false)
                .tip("Page breaks — planned"),
        ],
        vec![
            // Q4 default: the comments pane toggle stays on View (the
            // pane itself; Review owns comment authoring).
            RibbonButton::new(Icon::Comment, "Comments", Action::ToggleComments)
                .selected(ctx.show_comments),
            RibbonButton::new(Icon::Shapes, "Task Panes", Action::Macros)
                .enabled(false)
                .tip("Task panes (Shapes, Pan & Zoom…) — planned"),
        ],
    ])]
}

fn visual_aids_cols(ctx: &TabCtx) -> Vec<super::Col> {
    vec![
        stack(vec![
            RibbonButton::new(Icon::Grid, "Dynamic Grid", Action::ToggleDynamicGrid)
                .selected(ctx.dynamic_grid)
                .tip("Alignment and spacing hints while dragging"),
            RibbonButton::new(Icon::Shapes, "Connection Points", Action::Macros)
                .enabled(false)
                .tip("Show connection points — planned (moved from Show; Visio lists it here)"),
        ]),
        stack(vec![
            RibbonButton::new(Icon::Magnet, "Snap & Glue", Action::SnapGlue)
                .selected(ctx.snap_dlg_open)
                .tip("Configure snap and glue targets"),
            RibbonButton::new(Icon::Connector, "AutoConnect", Action::Macros)
                .enabled(false)
                .tip("AutoConnect arrows — the #108 remainder"),
        ]),
    ]
}

fn zoom_cols() -> Vec<super::Col> {
    vec![
        stack(vec![
            RibbonButton::new(Icon::ZoomIn, "", Action::ZoomIn)
                .alt("Zoom In")
                .tip("Zoom in"),
            RibbonButton::new(Icon::ZoomOut, "", Action::ZoomOut)
                .alt("Zoom Out")
                .tip("Zoom out"),
        ]),
        grid(vec![
            vec![
                RibbonButton::new(Icon::ZoomIn, "100%", Action::Zoom100),
                // Visio's Whole Page (was "Fit Window").
                RibbonButton::new(Icon::Fit, "Whole Page", Action::FitPage)
                    .tip("Fit the whole page in the window"),
            ],
            vec![
                RibbonButton::new(Icon::Fit, "Page Width", Action::Macros)
                    .enabled(false)
                    .tip("Fit the page width — planned"),
                RibbonButton::new(Icon::Shapes, "Zoom…", Action::Macros)
                    .enabled(false)
                    .tip("Zoom dialog — planned"),
            ],
        ]),
        stack(vec![
            // Q5 default: kept (Shift+Z exists).
            RibbonButton::new(Icon::ZoomSelection, "Selection", Action::ZoomSelection)
                .tip("Zoom to selection (Shift+Z)"),
        ]),
    ]
}

fn window_cols() -> Vec<super::Col> {
    vec![grid(vec![
        vec![
            RibbonButton::new(Icon::New, "New Window", Action::Macros)
                .enabled(false)
                .tip("New window — planned (single-window today)"),
            RibbonButton::new(Icon::Shapes, "Arrange All", Action::Macros)
                .enabled(false)
                .tip("Arrange windows — planned"),
        ],
        vec![
            RibbonButton::new(Icon::Shapes, "Split", Action::Macros)
                .enabled(false)
                .tip("Split view — planned"),
            RibbonButton::new(Icon::Fit, "Switch Windows", Action::Macros)
                .enabled(false)
                .tip("Switch documents — the tab strip's Window menu does this today (#192)"),
        ],
    ])]
}
