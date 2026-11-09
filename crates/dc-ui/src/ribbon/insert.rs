//! Insert tab content (#252) — moved verbatim from `app.rs` (commit 1
//! scaffolding); the per-tab fidelity sweep reshapes this in its own
//! commit.

use super::{Action, Group, Icon, RibbonButton, TabCtx, big, stack};

pub(crate) fn groups(ctx: &TabCtx) -> Vec<Group> {
    let has_doc = ctx.has_doc;
    vec![
        Group::new(
            "Blocks",
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
            ],
        ),
        Group::new(
            "Illustrations",
            vec![
                big(
                    RibbonButton::new(Icon::Picture, "Pictures", Action::InsertPicture)
                        .tip("Insert a PNG or JPEG picture from a file"),
                ),
                stack(vec![RibbonButton::new(
                    Icon::OnlinePic,
                    "Online Pictures",
                    Action::Macros,
                )
                .enabled(false)
                .tip("Picture shapes — planned")]),
            ],
        ),
        Group::new(
            "Parts",
            vec![
                big(
                    RibbonButton::new(Icon::TextBox, "Text Box", Action::Macros)
                        .enabled(false)
                        .tip("Text shapes — planned"),
                ),
                stack(vec![RibbonButton::new(Icon::Field, "Field", Action::Macros)
                    .enabled(false)
                    .tip("Fields — planned")]),
            ],
        ),
        Group::new(
            "Pages",
            vec![big(
                RibbonButton::new(Icon::NewPage, "New Page", Action::NewPage).enabled(has_doc),
            )],
        ),
        Group::new(
            "Comments",
            vec![big(
                RibbonButton::new(Icon::Comment, "New Comment", Action::Macros)
                    .enabled(false)
                    .tip("Comments — planned"),
            )],
        ),
    ]
}
