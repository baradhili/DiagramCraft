//! Data tab content (#252 fidelity sweep). Visio group order:
//! **External Data · Shape Data · Data Graphics · Display Options**
//! (everything M6-planned). Legend left for Insert ▸ Diagram Parts;
//! the two window toggles fold into their owning groups.

use super::{Action, Group, Icon, RibbonButton, TabCtx, big, grid, stack};

pub(crate) fn groups(_ctx: &TabCtx) -> Vec<Group> {
    vec![
        Group::new(crate::i18n::strs().msg_external_data(), external_data_cols()),
        Group::new(crate::i18n::strs().msg_shape_data(), shape_data_cols()),
        Group::new(crate::i18n::strs().msg_data_graphics(), data_graphics_cols()),
        Group::new(crate::i18n::strs().msg_display_options(), display_options_cols()),
    ]
}

fn external_data_cols() -> Vec<super::Col> {
    vec![
        big(
            RibbonButton::new(Icon::Open, crate::i18n::strs().msg_quick_import(), Action::Macros)
                .enabled(false)
                .tip(crate::i18n::strs().msg_data_linking_m6()),
        ),
        grid(vec![
            vec![
                RibbonButton::new(Icon::Link, "", Action::Macros)
                    .alt(crate::i18n::strs().msg_link_data_to_shapes())
                    .enabled(false)
                    .tip(crate::i18n::strs().msg_data_linking_m6()),
                RibbonButton::new(Icon::Refresh, "", Action::Macros)
                    .alt(crate::i18n::strs().msg_refresh_all())
                    .enabled(false)
                    .tip(crate::i18n::strs().msg_data_linking_m6()),
            ],
            vec![
                RibbonButton::new(Icon::Shapes, "", Action::Macros)
                    .alt(crate::i18n::strs().msg_custom_import())
                    .enabled(false)
                    .tip(crate::i18n::strs().msg_data_linking_m6()),
                RibbonButton::new(Icon::Magnet, "", Action::Macros)
                    .alt(crate::i18n::strs().msg_automatically_link())
                    .enabled(false)
                    .tip(crate::i18n::strs().msg_data_linking_m6()),
            ],
        ]),
        stack(vec![RibbonButton::new(
            Icon::Units,
            crate::i18n::strs().msg_external_data_window(),
            Action::Macros,
        )
        .enabled(false)
        .tip(crate::i18n::strs().msg_external_data_window_m6())]),
    ]
}

fn shape_data_cols() -> Vec<super::Col> {
    vec![
        big(
            RibbonButton::new(Icon::ShapeSheet, crate::i18n::strs().msg_shape_data_window(), Action::Macros)
                .enabled(false)
                .tip(crate::i18n::strs().msg_shape_data_window_m6()),
        ),
        stack(vec![RibbonButton::new(
            Icon::Field,
            crate::i18n::strs().msg_define_shape_data(),
            Action::Macros,
        )
        .enabled(false)
        .tip(crate::i18n::strs().msg_define_shape_data_m6())]),
    ]
}

fn data_graphics_cols() -> Vec<super::Col> {
    vec![
        big(
            RibbonButton::new(Icon::DataGraphic, crate::i18n::strs().msg_data_graphics(), Action::Macros)
                .enabled(false)
                .tip(crate::i18n::strs().msg_data_graphics_m6()),
        ),
        grid(vec![
            vec![
                RibbonButton::new(Icon::Effects, "", Action::Macros)
                    .alt(crate::i18n::strs().msg_create_new_data_graphic())
                    .enabled(false)
                    .tip(crate::i18n::strs().msg_data_graphics_m6()),
                RibbonButton::new(Icon::Pen, "", Action::Macros)
                    .alt(crate::i18n::strs().msg_edit_data_graphic())
                    .enabled(false)
                    .tip(crate::i18n::strs().msg_data_graphics_m6()),
            ],
        ]),
    ]
}

fn display_options_cols() -> Vec<super::Col> {
    vec![stack(vec![RibbonButton::new(
        Icon::Check,
        crate::i18n::strs().msg_display_data_graphics(),
        Action::Macros,
    )
    .enabled(false)
    .tip(crate::i18n::strs().msg_show_hide_data_graphics_on_the_p())])]
}
