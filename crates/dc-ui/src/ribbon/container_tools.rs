//! Container Tools contextual tab (#156, #252) — moved verbatim from
//! `app.rs` (commit 1 scaffolding); the per-tab fidelity sweep
//! reshapes this in its own commit against the reference shot that
//! landed on #252 (2026-11-09).

use super::{Action, Group, Icon, RibbonButton, TabCtx, grid, stack};

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
        Group::new(
            "Container Styles",
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
            ])],
        ),
        Group::new(
            "Members",
            vec![stack(vec![RibbonButton::new(
                Icon::Container,
                "Highlight Members",
                Action::ContainerHighlight,
            )
            .selected(ctx.container_highlight)
            .tip("Tint the container's members while it is selected")])],
        ),
    ]
}
