//! Design tab content (#252) — moved verbatim from `app.rs` (commit 1
//! scaffolding); the per-tab fidelity sweep reshapes this in its own
//! commit.

use super::{Action, Group, Icon, RibbonButton, TabCtx, big, grid, stack};

pub(crate) fn groups(ctx: &TabCtx) -> Vec<Group> {
    vec![
        Group::new(
            "Themes",
            vec![
                big(
                    RibbonButton::new(Icon::Theme, "Themes", Action::Macros)
                        .enabled(false)
                        .tip("Themes — planned"),
                ),
                stack(vec![RibbonButton::new(
                    Icon::Theme,
                    "Variants",
                    Action::Macros,
                )
                .enabled(false)
                .tip("Theme variants — planned")]),
            ],
        ),
        Group::new(
            "Page Setup",
            vec![grid(vec![
                vec![
                    RibbonButton::new(Icon::Orientation, "Orientation", Action::PageOrientation)
                        .tip("Swap portrait/landscape (swaps width and height)"),
                    RibbonButton::new(Icon::NewPage, "Size", Action::PageSizeDialog)
                        .selected(ctx.pagesetup_open)
                        .tip("Page size — ISO/ANSI presets or custom"),
                ],
                vec![RibbonButton::new(Icon::Shapes, "Auto Size", Action::Macros)
                    .enabled(false)
                    .tip("Auto page size — planned")],
            ])],
        ),
        Group::new(
            "Backgrounds",
            vec![grid(vec![
                vec![RibbonButton::new(
                    Icon::Picture,
                    "Backgrounds",
                    Action::PageBackgrounds,
                )
                .selected(ctx.backgrounds_open)
                .tip("Assign a background page behind this one")],
                vec![
                    RibbonButton::new(Icon::Shapes, "Borders & Titles", Action::Macros)
                        .enabled(false)
                        .tip("Borders & titles — planned"),
                ],
            ])],
        ),
        // #156: the Container Styles gallery — the SDCONT
        // masters (also on the Container Tools contextual
        // tab once a container exists).
        Group::new(
            "Containers",
            vec![stack(vec![RibbonButton::new(
                Icon::Container,
                "Containers…",
                Action::ContainerGallery,
            )
            .selected(ctx.contgal_open)
            .tip("Wrap the selection in a container style")])],
        ),
    ]
}
