//! Design tab content (#252 fidelity sweep). Visio group order:
//! **Page Setup · Themes · Variants · Layout** (+ Backgrounds kept as
//! the app's dialog placeholder). Page Setup leads (moved from
//! second); the Container gallery and Borders & Titles moved to
//! Insert ▸ Diagram Parts; Re-layout Page arrives from the removed
//! Process tab.

use super::{Action, Group, Icon, RibbonButton, TabCtx, big, grid, stack};

pub(crate) fn groups(s: &'static crate::l10n::L10nLanguage, ctx: &TabCtx) -> Vec<Group> {
    vec![
        Group::new(s.msg_page_setup(), page_setup_grid(s, ctx)),
        Group::new(s.msg_themes(), themes_cols(s, )),
        Group::new(s.msg_variants(), variants_cols(s, )),
        Group::new(s.msg_layout(), layout_cols(s, )),
        // Real Visio 2016 Design carries a Backgrounds gallery; ours
        // stays the #113 dialog (placeholder per the disposition).
        Group::new(s.msg_backgrounds(), vec![backgrounds_col(s, ctx)]),
    ]
}

fn page_setup_grid(s: &'static crate::l10n::L10nLanguage, ctx: &TabCtx) -> Vec<super::Col> {
    vec![grid(vec![
        vec![
            RibbonButton::new(Icon::Orientation, s.msg_orientation(), Action::PageOrientation)
                .tip(s.msg_swap_portrait_landscape_swaps_wi()),
            RibbonButton::new(Icon::NewPage, s.msg_size(), Action::PageSizeDialog)
                .selected(ctx.pagesetup_open)
                .tip(s.msg_page_size_iso_ansi_presets_or_cu()),
        ],
        vec![
            RibbonButton::new(Icon::Shapes, s.msg_auto_size(), Action::Macros)
                .enabled(false)
                .tip(s.msg_auto_page_size_planned()),
            RibbonButton::new(Icon::Fit, s.msg_scale(), Action::Macros)
                .enabled(false)
                .tip(s.msg_drawing_scale_planned()),
        ],
    ])]
}

fn themes_cols(s: &'static crate::l10n::L10nLanguage) -> Vec<super::Col> {
    vec![
        big(
            RibbonButton::new(Icon::Theme, s.msg_themes(), Action::Macros)
                .enabled(false)
                .tip(s.msg_themes_planned_gallery_folds_bro()),
        ),
    ]
}

fn variants_cols(s: &'static crate::l10n::L10nLanguage) -> Vec<super::Col> {
    vec![grid(vec![
        vec![
            RibbonButton::new(Icon::Theme, s.msg_variants(), Action::Macros)
                .enabled(false)
                .tip(s.msg_theme_variants_planned()),
            RibbonButton::new(Icon::Bucket, s.msg_colors(), Action::Macros)
                .enabled(false)
                .tip(s.msg_variant_colors_planned()),
        ],
        vec![
            RibbonButton::new(Icon::Effects, s.msg_effects(), Action::Macros)
                .enabled(false)
                .tip(s.msg_variant_effects_planned()),
            RibbonButton::new(Icon::Connector, s.msg_connectors(), Action::Macros)
                .enabled(false)
                .tip(s.msg_variant_connectors_planned()),
        ],
    ])]
}

fn layout_cols(s: &'static crate::l10n::L10nLanguage) -> Vec<super::Col> {
    // Fed from the removed Process tab (owner 2026-11-09): Visio
    // paints Re-Layout/Layout Options/Spacing on Design ▸ Layout.
    vec![grid(vec![
        vec![
            RibbonButton::new(Icon::Shapes, s.msg_re_layout_page(), Action::Macros)
                .enabled(false)
                .tip(s.msg_auto_layout_planned()),
            RibbonButton::new(Icon::Align, s.msg_layout_options(), Action::Macros)
                .enabled(false)
                .tip(s.msg_layout_options_planned()),
        ],
        vec![
            RibbonButton::new(Icon::DistV, s.msg_spacing(), Action::Macros)
                .enabled(false)
                .tip(s.msg_layout_spacing_planned()),
            RibbonButton::new(Icon::NewPage, "", Action::Macros)
                .alt(s.msg_layout_size())
                .enabled(false)
                .tip(s.msg_layout_size_planned()),
        ],
    ])]
}

fn backgrounds_col(s: &'static crate::l10n::L10nLanguage, ctx: &TabCtx) -> super::Col {
    stack(vec![RibbonButton::new(
        Icon::Picture,
        s.msg_backgrounds(),
        Action::PageBackgrounds,
    )
    .selected(ctx.backgrounds_open)
    .tip(s.msg_assign_a_background_page_behind())])
}
