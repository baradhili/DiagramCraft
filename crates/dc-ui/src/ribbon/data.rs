//! Data tab content (#252 fidelity sweep). Visio group order:
//! **External Data · Shape Data · Data Graphics · Display Options**
//! (everything M6-planned). Legend left for Insert ▸ Diagram Parts;
//! the two window toggles fold into their owning groups.

use super::{Action, Group, Icon, RibbonButton, TabCtx, big, grid, stack};

pub(crate) fn groups(s: &'static crate::l10n::L10nLanguage, _ctx: &TabCtx) -> Vec<Group> {
    vec![
        Group::new(s.msg_external_data(), external_data_cols(s, )),
        Group::new(s.msg_shape_data(), shape_data_cols(s, )),
        Group::new(s.msg_data_graphics(), data_graphics_cols(s, )),
        Group::new(s.msg_display_options(), display_options_cols(s, )),
    ]
}

fn external_data_cols(s: &'static crate::l10n::L10nLanguage) -> Vec<super::Col> {
    vec![
        big(
            RibbonButton::new(Icon::Open, s.msg_quick_import(), Action::Macros)
                .enabled(false)
                .tip(s.msg_data_linking_m6()),
        ),
        grid(vec![
            vec![
                RibbonButton::new(Icon::Link, "", Action::Macros)
                    .alt(s.msg_link_data_to_shapes())
                    .enabled(false)
                    .tip(s.msg_data_linking_m6()),
                RibbonButton::new(Icon::Refresh, "", Action::Macros)
                    .alt(s.msg_refresh_all())
                    .enabled(false)
                    .tip(s.msg_data_linking_m6()),
            ],
            vec![
                RibbonButton::new(Icon::Shapes, "", Action::Macros)
                    .alt(s.msg_custom_import())
                    .enabled(false)
                    .tip(s.msg_data_linking_m6()),
                RibbonButton::new(Icon::Magnet, "", Action::Macros)
                    .alt(s.msg_automatically_link())
                    .enabled(false)
                    .tip(s.msg_data_linking_m6()),
            ],
        ]),
        stack(vec![RibbonButton::new(
            Icon::Units,
            s.msg_external_data_window(),
            Action::Macros,
        )
        .enabled(false)
        .tip(s.msg_external_data_window_m6())]),
    ]
}

fn shape_data_cols(s: &'static crate::l10n::L10nLanguage) -> Vec<super::Col> {
    vec![
        big(
            RibbonButton::new(Icon::ShapeSheet, s.msg_shape_data_window(), Action::Macros)
                .enabled(false)
                .tip(s.msg_shape_data_window_m6()),
        ),
        stack(vec![RibbonButton::new(
            Icon::Field,
            s.msg_define_shape_data(),
            Action::Macros,
        )
        .enabled(false)
        .tip(s.msg_define_shape_data_m6())]),
    ]
}

fn data_graphics_cols(s: &'static crate::l10n::L10nLanguage) -> Vec<super::Col> {
    vec![
        big(
            RibbonButton::new(Icon::DataGraphic, s.msg_data_graphics(), Action::Macros)
                .enabled(false)
                .tip(s.msg_data_graphics_m6()),
        ),
        grid(vec![
            vec![
                RibbonButton::new(Icon::Effects, "", Action::Macros)
                    .alt(s.msg_create_new_data_graphic())
                    .enabled(false)
                    .tip(s.msg_data_graphics_m6()),
                RibbonButton::new(Icon::Pen, "", Action::Macros)
                    .alt(s.msg_edit_data_graphic())
                    .enabled(false)
                    .tip(s.msg_data_graphics_m6()),
            ],
        ]),
    ]
}

fn display_options_cols(s: &'static crate::l10n::L10nLanguage) -> Vec<super::Col> {
    vec![stack(vec![RibbonButton::new(
        Icon::Check,
        s.msg_display_data_graphics(),
        Action::Macros,
    )
    .enabled(false)
    .tip(s.msg_show_hide_data_graphics_on_the_p())])]
}
