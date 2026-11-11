//! Developer tab content (#252 fidelity sweep). Visio group order:
//! **Code · Add-ins · Controls · Shape Design · Stencil · Show/Hide**.
//! ShapeSheet paints once (Visio double-lists it); Macros stays on
//! View only; Add-ins/Controls/Stencil land as planned groups.

use super::{Action, Group, Icon, RibbonButton, TabCtx, big, grid, stack};

pub(crate) fn groups(ctx: &TabCtx) -> Vec<Group> {
    vec![
        Group::new(crate::i18n::strs().msg_code(), code_cols(ctx)),
        Group::new(crate::i18n::strs().msg_add_ins(), addins_cols()),
        Group::new(crate::i18n::strs().msg_controls(), controls_cols()),
        Group::new(crate::i18n::strs().msg_shape_design(), shape_design_grid()),
        Group::new(crate::i18n::strs().msg_stencil(), stencil_cols()),
        Group::new(crate::i18n::strs().msg_show_hide(), show_hide_grid(ctx)),
    ]
}

fn code_cols(ctx: &TabCtx) -> Vec<super::Col> {
    vec![
        big(
            RibbonButton::new(Icon::ShapeSheet, crate::i18n::strs().msg_shapesheet(), Action::ShowShapeSheet)
                .selected(ctx.shapesheet_open)
                .tip(crate::i18n::strs().msg_inspect_the_selected_shape_s_cel()),
        ),
        grid(vec![
            vec![
                RibbonButton::new(Icon::Macro, "", Action::Macros)
                    .alt(crate::i18n::strs().msg_visual_basic())
                    .enabled(false)
                    .tip(crate::i18n::strs().msg_vba_arrives_with_m7_vsdm()),
                RibbonButton::new(Icon::Check, "", Action::Macros)
                    .alt(crate::i18n::strs().msg_macro_security())
                    .enabled(false)
                    .tip(crate::i18n::strs().msg_macro_security_m7()),
            ],
        ]),
    ]
}

fn addins_cols() -> Vec<super::Col> {
    vec![stack(vec![
        RibbonButton::new(Icon::Link, crate::i18n::strs().msg_com_add_ins(), Action::Macros)
            .enabled(false)
            .tip(crate::i18n::strs().msg_add_in_hosting_arrives_with_m8()),
        RibbonButton::new(Icon::Shapes, crate::i18n::strs().msg_add_in_manager(), Action::Macros)
            .enabled(false)
            .tip(crate::i18n::strs().msg_add_in_manager_m8()),
    ])]
}

fn controls_cols() -> Vec<super::Col> {
    vec![stack(vec![
        RibbonButton::new(Icon::Shapes, crate::i18n::strs().msg_insert_control(), Action::Macros)
            .enabled(false)
            .tip(crate::i18n::strs().msg_activex_era_likely_cut_scope_cal()),
        RibbonButton::new(Icon::Check, crate::i18n::strs().msg_design_mode(), Action::Macros)
            .enabled(false)
            .tip(crate::i18n::strs().msg_design_mode_likely_cut_scope_cal()),
    ])]
}

fn shape_design_grid() -> Vec<super::Col> {
    vec![grid(vec![
        vec![
            RibbonButton::new(Icon::Shapes, crate::i18n::strs().msg_operations(), Action::Macros)
                .enabled(false)
                .tip(crate::i18n::strs().msg_boolean_shape_operations_planned()),
            RibbonButton::new(Icon::Group, crate::i18n::strs().msg_behavior(), Action::Macros)
                .enabled(false)
                .tip(crate::i18n::strs().msg_shape_behavior_planned()),
        ],
        vec![
            RibbonButton::new(Icon::Check, crate::i18n::strs().msg_protection(), Action::Macros)
                .enabled(false)
                .tip(crate::i18n::strs().msg_shape_protection_planned()),
        ],
    ])]
}

fn stencil_cols() -> Vec<super::Col> {
    vec![grid(vec![
        vec![
            RibbonButton::new(Icon::Shapes, crate::i18n::strs().msg_edit_stencil(), Action::Macros)
                .enabled(false)
                .tip(crate::i18n::strs().msg_stencil_editing_planned_scope_ca()),
            RibbonButton::new(Icon::New, crate::i18n::strs().msg_new_stencil(), Action::Macros)
                .enabled(false)
                .tip(crate::i18n::strs().msg_create_a_stencil_planned()),
        ],
        vec![
            RibbonButton::new(Icon::Save, crate::i18n::strs().msg_save_stencil(), Action::Macros)
                .enabled(false)
                .tip(crate::i18n::strs().msg_save_the_stencil_planned()),
            RibbonButton::new(Icon::Units, crate::i18n::strs().msg_stencil_settings(), Action::Macros)
                .enabled(false)
                .tip(crate::i18n::strs().msg_stencil_settings_planned()),
        ],
    ])]
}

fn show_hide_grid(ctx: &TabCtx) -> Vec<super::Col> {
    vec![grid(vec![
        vec![
            // Visio legitimately double-lists ShapeSheet (Code and
            // Show/Hide) — the Show/Hide copy stays the wired toggle.
            RibbonButton::new(Icon::ShapeSheet, crate::i18n::strs().msg_show_shapesheet(), Action::ShowShapeSheet)
                .selected(ctx.shapesheet_open)
                .tip(crate::i18n::strs().msg_inspect_the_selected_shape_s_cel()),
            RibbonButton::new(Icon::Units, crate::i18n::strs().msg_mm_in(), Action::ToggleUnits)
                .tip(crate::i18n::strs().msg_toggle_display_units_model_stays()),
        ],
        vec![
            RibbonButton::new(Icon::Shapes, crate::i18n::strs().msg_drawing_explorer(), Action::Macros)
                .enabled(false)
                .tip(crate::i18n::strs().msg_drawing_explorer_planned()),
            RibbonButton::new(Icon::Shapes, crate::i18n::strs().msg_show_document_stencil(), Action::Macros)
                .enabled(false)
                .tip(crate::i18n::strs().msg_document_stencil_planned()),
        ],
    ])]
}
