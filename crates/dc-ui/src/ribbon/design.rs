//! Design tab content (#252 fidelity sweep). Visio group order:
//! **Page Setup · Themes · Variants · Layout** (+ Backgrounds kept as
//! the app's dialog placeholder). Page Setup leads (moved from
//! second); the Container gallery and Borders & Titles moved to
//! Insert ▸ Diagram Parts; Re-layout Page arrives from the removed
//! Process tab.

use super::{Action, Group, Icon, RibbonButton, TabCtx, big, grid, stack};

pub(crate) fn groups(ctx: &TabCtx) -> Vec<Group> {
    vec![
        Group::new(crate::i18n::strs().msg_page_setup(), page_setup_grid(ctx)),
        Group::new(crate::i18n::strs().msg_themes(), themes_cols()),
        Group::new(crate::i18n::strs().msg_variants(), variants_cols()),
        Group::new(crate::i18n::strs().msg_layout(), layout_cols()),
        // Real Visio 2016 Design carries a Backgrounds gallery; ours
        // stays the #113 dialog (placeholder per the disposition).
        Group::new(crate::i18n::strs().msg_backgrounds(), vec![backgrounds_col(ctx)]),
    ]
}

fn page_setup_grid(ctx: &TabCtx) -> Vec<super::Col> {
    vec![grid(vec![
        vec![
            RibbonButton::new(Icon::Orientation, crate::i18n::strs().msg_orientation(), Action::PageOrientation)
                .tip(crate::i18n::strs().msg_swap_portrait_landscape_swaps_wi()),
            RibbonButton::new(Icon::NewPage, crate::i18n::strs().msg_size(), Action::PageSizeDialog)
                .selected(ctx.pagesetup_open)
                .tip(crate::i18n::strs().msg_page_size_iso_ansi_presets_or_cu()),
        ],
        vec![
            RibbonButton::new(Icon::Shapes, crate::i18n::strs().msg_auto_size(), Action::Macros)
                .enabled(false)
                .tip(crate::i18n::strs().msg_auto_page_size_planned()),
            RibbonButton::new(Icon::Fit, crate::i18n::strs().msg_scale(), Action::Macros)
                .enabled(false)
                .tip(crate::i18n::strs().msg_drawing_scale_planned()),
        ],
    ])]
}

fn themes_cols() -> Vec<super::Col> {
    vec![
        big(
            RibbonButton::new(Icon::Theme, crate::i18n::strs().msg_themes(), Action::Macros)
                .enabled(false)
                .tip(crate::i18n::strs().msg_themes_planned_gallery_folds_bro()),
        ),
    ]
}

fn variants_cols() -> Vec<super::Col> {
    vec![grid(vec![
        vec![
            RibbonButton::new(Icon::Theme, crate::i18n::strs().msg_variants(), Action::Macros)
                .enabled(false)
                .tip(crate::i18n::strs().msg_theme_variants_planned()),
            RibbonButton::new(Icon::Bucket, crate::i18n::strs().msg_colors(), Action::Macros)
                .enabled(false)
                .tip(crate::i18n::strs().msg_variant_colors_planned()),
        ],
        vec![
            RibbonButton::new(Icon::Effects, crate::i18n::strs().msg_effects(), Action::Macros)
                .enabled(false)
                .tip(crate::i18n::strs().msg_variant_effects_planned()),
            RibbonButton::new(Icon::Connector, crate::i18n::strs().msg_connectors(), Action::Macros)
                .enabled(false)
                .tip(crate::i18n::strs().msg_variant_connectors_planned()),
        ],
    ])]
}

fn layout_cols() -> Vec<super::Col> {
    // Fed from the removed Process tab (owner 2026-11-09): Visio
    // paints Re-Layout/Layout Options/Spacing on Design ▸ Layout.
    vec![grid(vec![
        vec![
            RibbonButton::new(Icon::Shapes, crate::i18n::strs().msg_re_layout_page(), Action::Macros)
                .enabled(false)
                .tip(crate::i18n::strs().msg_auto_layout_planned()),
            RibbonButton::new(Icon::Align, crate::i18n::strs().msg_layout_options(), Action::Macros)
                .enabled(false)
                .tip(crate::i18n::strs().msg_layout_options_planned()),
        ],
        vec![
            RibbonButton::new(Icon::DistV, crate::i18n::strs().msg_spacing(), Action::Macros)
                .enabled(false)
                .tip(crate::i18n::strs().msg_layout_spacing_planned()),
            RibbonButton::new(Icon::NewPage, "", Action::Macros)
                .alt(crate::i18n::strs().msg_layout_size())
                .enabled(false)
                .tip(crate::i18n::strs().msg_layout_size_planned()),
        ],
    ])]
}

fn backgrounds_col(ctx: &TabCtx) -> super::Col {
    stack(vec![RibbonButton::new(
        Icon::Picture,
        crate::i18n::strs().msg_backgrounds(),
        Action::PageBackgrounds,
    )
    .selected(ctx.backgrounds_open)
    .tip(crate::i18n::strs().msg_assign_a_background_page_behind())])
}
