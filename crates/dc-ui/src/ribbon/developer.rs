//! Developer tab content (#252 fidelity sweep). Visio group order:
//! **Code · Add-ins · Controls · Shape Design · Stencil · Show/Hide**.
//! ShapeSheet paints once (Visio double-lists it); Macros stays on
//! View only; Add-ins/Controls/Stencil land as planned groups.

use super::{Action, Group, Icon, RibbonButton, TabCtx, big, grid, stack};

pub(crate) fn groups(s: &'static crate::l10n::L10nLanguage, ctx: &TabCtx) -> Vec<Group> {
    vec![
        Group::new(s.msg_code(), code_cols(s, ctx)),
        Group::new(s.msg_add_ins(), addins_cols(s, )),
        Group::new(s.msg_controls(), controls_cols(s, )),
        Group::new(s.msg_shape_design(), shape_design_grid(s, )),
        Group::new(s.msg_stencil(), stencil_cols(s, )),
        Group::new(s.msg_show_hide(), show_hide_grid(s, ctx)),
    ]
}

fn code_cols(s: &'static crate::l10n::L10nLanguage, ctx: &TabCtx) -> Vec<super::Col> {
    vec![
        big(
            RibbonButton::new(Icon::ShapeSheet, s.msg_shapesheet(), Action::ShowShapeSheet)
                .selected(ctx.shapesheet_open)
                .tip(s.msg_inspect_the_selected_shape_s_cel()),
        ),
        grid(vec![
            vec![
                RibbonButton::new(Icon::Macro, "", Action::Macros)
                    .alt(s.msg_visual_basic())
                    .enabled(false)
                    .tip(s.msg_vba_arrives_with_m7_vsdm()),
                RibbonButton::new(Icon::Check, "", Action::Macros)
                    .alt(s.msg_macro_security())
                    .enabled(false)
                    .tip(s.msg_macro_security_m7()),
            ],
        ]),
    ]
}

fn addins_cols(s: &'static crate::l10n::L10nLanguage) -> Vec<super::Col> {
    vec![stack(vec![
        RibbonButton::new(Icon::Link, s.msg_com_add_ins(), Action::Macros)
            .enabled(false)
            .tip(s.msg_add_in_hosting_arrives_with_m8()),
        RibbonButton::new(Icon::Shapes, s.msg_add_in_manager(), Action::Macros)
            .enabled(false)
            .tip(s.msg_add_in_manager_m8()),
    ])]
}

fn controls_cols(s: &'static crate::l10n::L10nLanguage) -> Vec<super::Col> {
    vec![stack(vec![
        RibbonButton::new(Icon::Shapes, s.msg_insert_control(), Action::Macros)
            .enabled(false)
            .tip(s.msg_activex_era_likely_cut_scope_cal()),
        RibbonButton::new(Icon::Check, s.msg_design_mode(), Action::Macros)
            .enabled(false)
            .tip(s.msg_design_mode_likely_cut_scope_cal()),
    ])]
}

fn shape_design_grid(s: &'static crate::l10n::L10nLanguage) -> Vec<super::Col> {
    vec![grid(vec![
        vec![
            RibbonButton::new(Icon::Shapes, s.msg_operations(), Action::Macros)
                .enabled(false)
                .tip(s.msg_boolean_shape_operations_planned()),
            RibbonButton::new(Icon::Group, s.msg_behavior(), Action::Macros)
                .enabled(false)
                .tip(s.msg_shape_behavior_planned()),
        ],
        vec![
            RibbonButton::new(Icon::Check, s.msg_protection(), Action::Macros)
                .enabled(false)
                .tip(s.msg_shape_protection_planned()),
        ],
    ])]
}

fn stencil_cols(s: &'static crate::l10n::L10nLanguage) -> Vec<super::Col> {
    vec![grid(vec![
        vec![
            RibbonButton::new(Icon::Shapes, s.msg_edit_stencil(), Action::Macros)
                .enabled(false)
                .tip(s.msg_stencil_editing_planned_scope_ca()),
            RibbonButton::new(Icon::New, s.msg_new_stencil(), Action::Macros)
                .enabled(false)
                .tip(s.msg_create_a_stencil_planned()),
        ],
        vec![
            RibbonButton::new(Icon::Save, s.msg_save_stencil(), Action::Macros)
                .enabled(false)
                .tip(s.msg_save_the_stencil_planned()),
            RibbonButton::new(Icon::Units, s.msg_stencil_settings(), Action::Macros)
                .enabled(false)
                .tip(s.msg_stencil_settings_planned()),
        ],
    ])]
}

fn show_hide_grid(s: &'static crate::l10n::L10nLanguage, ctx: &TabCtx) -> Vec<super::Col> {
    vec![grid(vec![
        vec![
            // Visio legitimately double-lists ShapeSheet (Code and
            // Show/Hide) — the Show/Hide copy stays the wired toggle.
            RibbonButton::new(Icon::ShapeSheet, s.msg_show_shapesheet(), Action::ShowShapeSheet)
                .selected(ctx.shapesheet_open)
                .tip(s.msg_inspect_the_selected_shape_s_cel()),
            RibbonButton::new(Icon::Units, s.msg_mm_in(), Action::ToggleUnits)
                .tip(s.msg_toggle_display_units_model_stays()),
        ],
        vec![
            RibbonButton::new(Icon::Shapes, s.msg_drawing_explorer(), Action::Macros)
                .enabled(false)
                .tip(s.msg_drawing_explorer_planned()),
            RibbonButton::new(Icon::Shapes, s.msg_show_document_stencil(), Action::Macros)
                .enabled(false)
                .tip(s.msg_document_stencil_planned()),
        ],
    ])]
}
