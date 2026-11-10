//! Container Tools contextual tab (#156, #252). Reshaped against the
//! reference shot that landed on #252 (2026-11-09): Visio paints
//! **Size · Container Styles · Heading Style · Membership** — Margins
//! / Fit to Contents / Automatic Resize placeholders, the SDCONT
//! gallery, the heading-style gallery, and the membership set with a
//! wired Disband Container.

use super::{Action, Group, Icon, RibbonButton, TabCtx, big, grid, stack};

/// #156: the SDCONT container style gallery — the shipped Visio 2016
/// set, in gallery order (Analysis/container-stencil-sdcont.md; the
/// masters live in the embedded `sdcont` stencil). Shared with the
/// Container Styles gallery popup in `app.rs`.
pub(crate) const CONTAINER_STYLES: [&str; 14] = [
    "Plain",
    "Notch",
    "Waves",
    "Wire",
    "Belt",
    "Classic",
    "Full sail",
    "Corners",
    "Alternating",
    "Banner",
    "Picture Frame",
    "Snapshot",
    "Intersection",
    "Translucent",
];

pub(crate) fn groups(ctx: &TabCtx) -> Vec<Group> {
    vec![
        Group::new("Size", size_cols()),
        Group::new("Container Styles", styles_grid()),
        Group::new("Heading Style", heading_cols()),
        Group::new("Membership", membership_cols(ctx)),
    ]
}

fn size_cols() -> Vec<super::Col> {
    vec![
        big(
            RibbonButton::new(Icon::Grid, "Margins", Action::Macros)
                .enabled(false)
                .split()
                .tip("Container margins — planned"),
        ),
        stack(vec![
            RibbonButton::new(Icon::Fit, "Fit to Contents", Action::Macros)
                .enabled(false)
                .tip("Size the container to its members — planned"),
            RibbonButton::new(Icon::Fit, "Automatic Resize", Action::Macros)
                .enabled(false)
                .split()
                .tip("Grow with the members — planned"),
        ]),
    ]
}

fn styles_grid() -> Vec<super::Col> {
    vec![grid(vec![
        CONTAINER_STYLES[..7]
            .iter()
            .map(|n| {
                RibbonButton::new(Icon::Container, n, Action::ContainerStyle(n))
                    .tip("Wrap the selection in this container style")
            })
            .collect::<Vec<_>>(),
        CONTAINER_STYLES[7..]
            .iter()
            .map(|n| {
                RibbonButton::new(Icon::Container, n, Action::ContainerStyle(n))
                    .tip("Wrap the selection in this container style")
            })
            .collect::<Vec<_>>(),
    ])]
}

fn heading_cols() -> Vec<super::Col> {
    vec![big(
        RibbonButton::new(Icon::FontA, "Heading Style", Action::Macros)
            .enabled(false)
            .split()
            .tip("Heading style gallery — planned"),
    )]
}

fn membership_cols(ctx: &TabCtx) -> Vec<super::Col> {
    vec![
        grid(vec![
            vec![
                RibbonButton::new(Icon::Container, "Highlight Members", Action::ContainerHighlight)
                    .selected(ctx.container_highlight)
                    .tip("Tint the container's members while it is selected"),
                RibbonButton::new(Icon::Lock, "", Action::Macros)
                    .alt("Lock Container")
                    .enabled(false)
                    .tip("Lock the container against membership edits — planned"),
            ],
            vec![
                RibbonButton::new(Icon::Shapes, "", Action::Macros)
                    .alt("Select Contents")
                    .enabled(false)
                    .tip("Select the container's members — planned"),
                RibbonButton::new(Icon::Delete, "", Action::ContainerDisband)
                    .alt("Disband Container")
                    .enabled(ctx.sel_container)
                    .tip("Release the members and delete the container (one undo step)"),
            ],
        ]),
    ]
}
