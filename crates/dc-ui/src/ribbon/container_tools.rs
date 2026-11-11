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
        Group::new(crate::i18n::strs().msg_size(), size_cols()),
        Group::new(crate::i18n::strs().msg_container_styles(), styles_grid()),
        Group::new(crate::i18n::strs().msg_heading_style(), heading_cols()),
        Group::new(crate::i18n::strs().msg_membership(), membership_cols(ctx)),
    ]
}

fn size_cols() -> Vec<super::Col> {
    vec![
        big(
            RibbonButton::new(Icon::Grid, crate::i18n::strs().msg_margins(), Action::Macros)
                .enabled(false)
                .split()
                .tip(crate::i18n::strs().msg_container_margins_planned()),
        ),
        stack(vec![
            RibbonButton::new(Icon::Fit, crate::i18n::strs().msg_fit_to_contents(), Action::Macros)
                .enabled(false)
                .tip(crate::i18n::strs().msg_size_the_container_to_its_member()),
            RibbonButton::new(Icon::Fit, crate::i18n::strs().msg_automatic_resize(), Action::Macros)
                .enabled(false)
                .split()
                .tip(crate::i18n::strs().msg_grow_with_the_members_planned()),
        ]),
    ]
}

fn styles_grid() -> Vec<super::Col> {
    vec![grid(vec![
        CONTAINER_STYLES[..7]
            .iter()
            .map(|n| {
                RibbonButton::new(Icon::Container, *n, Action::ContainerStyle(n))
                    .tip(crate::i18n::strs().msg_wrap_the_selection_in_this_conta())
            })
            .collect::<Vec<_>>(),
        CONTAINER_STYLES[7..]
            .iter()
            .map(|n| {
                RibbonButton::new(Icon::Container, *n, Action::ContainerStyle(n))
                    .tip(crate::i18n::strs().msg_wrap_the_selection_in_this_conta())
            })
            .collect::<Vec<_>>(),
    ])]
}

fn heading_cols() -> Vec<super::Col> {
    vec![big(
        RibbonButton::new(Icon::FontA, crate::i18n::strs().msg_heading_style(), Action::Macros)
            .enabled(false)
            .split()
            .tip(crate::i18n::strs().msg_heading_style_gallery_planned()),
    )]
}

fn membership_cols(ctx: &TabCtx) -> Vec<super::Col> {
    vec![
        grid(vec![
            vec![
                RibbonButton::new(Icon::Container, crate::i18n::strs().msg_highlight_members(), Action::ContainerHighlight)
                    .selected(ctx.container_highlight)
                    .tip(crate::i18n::strs().msg_tint_the_container_s_members_whi()),
                RibbonButton::new(Icon::Lock, "", Action::Macros)
                    .alt(crate::i18n::strs().msg_lock_container())
                    .enabled(false)
                    .tip(crate::i18n::strs().msg_lock_the_container_against_membe()),
            ],
            vec![
                RibbonButton::new(Icon::Shapes, "", Action::Macros)
                    .alt(crate::i18n::strs().msg_select_contents())
                    .enabled(false)
                    .tip(crate::i18n::strs().msg_select_the_container_s_members_p()),
                RibbonButton::new(Icon::Delete, "", Action::ContainerDisband)
                    .alt(crate::i18n::strs().msg_disband_container())
                    .enabled(ctx.sel_container)
                    .tip(crate::i18n::strs().msg_release_the_members_and_delete_t()),
            ],
        ]),
    ]
}
