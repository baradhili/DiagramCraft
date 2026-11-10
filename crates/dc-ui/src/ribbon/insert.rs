//! Insert tab content (#252 fidelity sweep). Visio group order:
//! **Pages · Illustrations · Diagram Parts · Links · Text**. The
//! Comments group is gone (Review owns New Comment); the Container
//! gallery and Borders & Titles move in from Design, Legend from Data
//! (Design/Data drop them in their own commits). Page Setup's launcher
//! moves to Design ▸ Page Setup.

use super::{Action, Group, Icon, RibbonButton, TabCtx, big, grid, stack};

pub(crate) fn groups(ctx: &TabCtx) -> Vec<Group> {
    let has_doc = ctx.has_doc;
    let has_sel = ctx.has_sel;
    vec![
        Group::new("Pages", pages_cols(has_doc)),
        Group::new("Illustrations", illustrations_cols()),
        Group::new("Diagram Parts", diagram_parts_cols(ctx)),
        Group::new(
            "Links",
            vec![big(
                RibbonButton::new(Icon::Link, "Hyperlink", Action::Hyperlink)
                    .enabled(has_sel)
                    .tip("Edit the selected shape's hyperlinks (Ctrl+K)"),
            )],
        ),
        Group::new("Text", text_cols()),
    ]
}

fn pages_cols(has_doc: bool) -> Vec<super::Col> {
    vec![
        big(RibbonButton::new(Icon::NewPage, "New Page", Action::NewPage).enabled(has_doc)),
        // Visio's New Page split: plain page / background page (the
        // background variant is planned; assignment lives in
        // Design ▸ Backgrounds today).
        stack(vec![RibbonButton::new(
            Icon::Picture,
            "New Background Page",
            Action::Macros,
        )
        .enabled(false)
        .tip("Create a background page — planned (Design ▸ Backgrounds assigns today)")]),
    ]
}

fn illustrations_cols() -> Vec<super::Col> {
    vec![
        big(
            RibbonButton::new(Icon::Picture, "Pictures", Action::InsertPicture)
                .tip("Insert a PNG or JPEG picture from a file"),
        ),
        grid(vec![
            vec![
                RibbonButton::new(Icon::OnlinePic, "", Action::Macros)
                    .alt("Online Pictures")
                    .enabled(false)
                    .tip("Online pictures — planned"),
                RibbonButton::new(Icon::DataGraphic, "", Action::Macros)
                    .alt("Chart")
                    .enabled(false)
                    .tip("Charts — planned (later milestone)"),
            ],
            vec![
                RibbonButton::new(Icon::Shapes, "", Action::Macros)
                    .alt("CAD Drawing")
                    .enabled(false)
                    .tip("CAD drawing import — planned (later milestone)"),
            ],
        ]),
    ]
}

fn diagram_parts_cols(ctx: &TabCtx) -> Vec<super::Col> {
    vec![
        big(
            RibbonButton::new(Icon::Container, "Container", Action::Macros)
                .enabled(false)
                .tip("Containers — planned"),
        ),
        big(
            RibbonButton::new(Icon::Callout, "Callout", Action::Macros)
                .enabled(false)
                .tip("Callouts — planned"),
        ),
        grid(vec![
            vec![
                // #156: the SDCONT gallery (moved from Design).
                RibbonButton::new(Icon::Container, "", Action::ContainerGallery)
                    .alt("Containers…")
                    .selected(ctx.contgal_open)
                    .tip("Wrap the selection in a container style"),
                RibbonButton::new(Icon::Connector, "", Action::ConnectorTool)
                    .alt("Connector")
                    .selected(ctx.tool == crate::app::Tool::Connector)
                    .tip("Connector tool — draw a connector between shapes or points"),
            ],
            vec![
                RibbonButton::new(Icon::Shapes, "", Action::Macros)
                    .alt("Legend")
                    .enabled(false)
                    .tip("Legends (M6; Visio paints Legend on Insert, not Data)"),
                RibbonButton::new(Icon::Title, "", Action::Macros)
                    .alt("Title")
                    .enabled(false)
                    .tip("Title — planned"),
            ],
            vec![
                RibbonButton::new(Icon::Picture, "", Action::Macros)
                    .alt("Border & Title")
                    .enabled(false)
                    .tip("Borders & titles — planned"),
            ],
        ]),
    ]
}

fn text_cols() -> Vec<super::Col> {
    vec![
        big(
            RibbonButton::new(Icon::TextBox, "Text Box", Action::Macros)
                .enabled(false)
                .tip("Text shapes — planned"),
        ),
        grid(vec![
            vec![
                RibbonButton::new(Icon::Field, "", Action::Macros)
                    .alt("Text Field")
                    .enabled(false)
                    .tip("Fields — planned"),
                RibbonButton::new(Icon::DateTime, "", Action::Macros)
                    .alt("Date & Time")
                    .enabled(false)
                    .tip("Date & time field — planned"),
            ],
            vec![
                RibbonButton::new(Icon::NewPage, "", Action::Macros)
                    .alt("Page Number")
                    .enabled(false)
                    .tip("Page-number field — planned"),
                RibbonButton::new(Icon::FontA, "", Action::Macros)
                    .alt("Symbol")
                    .enabled(false)
                    .tip("Symbol — planned"),
            ],
        ]),
    ]
}
