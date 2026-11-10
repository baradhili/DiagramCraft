//! Design tab content (#252 fidelity sweep). Visio group order:
//! **Page Setup · Themes · Variants · Layout** (+ Backgrounds kept as
//! the app's dialog placeholder). Page Setup leads (moved from
//! second); the Container gallery and Borders & Titles moved to
//! Insert ▸ Diagram Parts; Re-layout Page arrives from the removed
//! Process tab.

use super::{Action, Group, Icon, RibbonButton, TabCtx, big, grid, stack};

pub(crate) fn groups(ctx: &TabCtx) -> Vec<Group> {
    vec![
        Group::new("Page Setup", page_setup_grid(ctx)),
        Group::new("Themes", themes_cols()),
        Group::new("Variants", variants_cols()),
        Group::new("Layout", layout_cols()),
        // Real Visio 2016 Design carries a Backgrounds gallery; ours
        // stays the #113 dialog (placeholder per the disposition).
        Group::new("Backgrounds", vec![backgrounds_col(ctx)]),
    ]
}

fn page_setup_grid(ctx: &TabCtx) -> Vec<super::Col> {
    vec![grid(vec![
        vec![
            RibbonButton::new(Icon::Orientation, "Orientation", Action::PageOrientation)
                .tip("Swap portrait/landscape (swaps width and height)"),
            RibbonButton::new(Icon::NewPage, "Size", Action::PageSizeDialog)
                .selected(ctx.pagesetup_open)
                .tip("Page size — ISO/ANSI presets or custom"),
        ],
        vec![
            RibbonButton::new(Icon::Shapes, "Auto Size", Action::Macros)
                .enabled(false)
                .tip("Auto page size — planned"),
            RibbonButton::new(Icon::Fit, "Scale", Action::Macros)
                .enabled(false)
                .tip("Drawing scale — planned"),
        ],
    ])]
}

fn themes_cols() -> Vec<super::Col> {
    vec![
        big(
            RibbonButton::new(Icon::Theme, "Themes", Action::Macros)
                .enabled(false)
                .tip("Themes — planned (gallery folds Browse for Themes, Theme Colors/Effects/Connectors)"),
        ),
    ]
}

fn variants_cols() -> Vec<super::Col> {
    vec![grid(vec![
        vec![
            RibbonButton::new(Icon::Theme, "Variants", Action::Macros)
                .enabled(false)
                .tip("Theme variants — planned"),
            RibbonButton::new(Icon::Bucket, "Colors", Action::Macros)
                .enabled(false)
                .tip("Variant colors — planned"),
        ],
        vec![
            RibbonButton::new(Icon::Effects, "Effects", Action::Macros)
                .enabled(false)
                .tip("Variant effects — planned"),
            RibbonButton::new(Icon::Connector, "Connectors", Action::Macros)
                .enabled(false)
                .tip("Variant connectors — planned"),
        ],
    ])]
}

fn layout_cols() -> Vec<super::Col> {
    // Fed from the removed Process tab (owner 2026-11-09): Visio
    // paints Re-Layout/Layout Options/Spacing on Design ▸ Layout.
    vec![grid(vec![
        vec![
            RibbonButton::new(Icon::Shapes, "Re-layout Page", Action::Macros)
                .enabled(false)
                .tip("Auto layout — planned"),
            RibbonButton::new(Icon::Align, "Layout Options", Action::Macros)
                .enabled(false)
                .tip("Layout options — planned"),
        ],
        vec![
            RibbonButton::new(Icon::DistV, "Spacing", Action::Macros)
                .enabled(false)
                .tip("Layout spacing — planned"),
            RibbonButton::new(Icon::NewPage, "", Action::Macros)
                .alt("Layout Size")
                .enabled(false)
                .tip("Layout size — planned"),
        ],
    ])]
}

fn backgrounds_col(ctx: &TabCtx) -> super::Col {
    stack(vec![RibbonButton::new(
        Icon::Picture,
        "Backgrounds",
        Action::PageBackgrounds,
    )
    .selected(ctx.backgrounds_open)
    .tip("Assign a background page behind this one")])
}
