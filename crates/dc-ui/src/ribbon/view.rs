//! View tab content (#252 fidelity sweep). Visio group order:
//! **Views · Show · Visual Aids · Zoom · Window · Macros**. Visual
//! Aids moves ahead of Zoom and absorbs Connection Points from Show;
//! "Fit Window" renames to Visio's Whole Page; the new Window group
//! is placeholder (Switch Windows maps to the strip's #192 switcher
//! eventually).

use super::{Action, Group, Icon, RibbonButton, TabCtx, big, grid, stack};

pub(crate) fn groups(s: &'static crate::l10n::L10nLanguage, ctx: &TabCtx) -> Vec<Group> {
    vec![
        Group::new(s.msg_views(), views_cols(s, )),
        Group::new(s.msg_show(), show_grid(s, ctx)),
        Group::new(s.msg_visual_aids(), visual_aids_cols(s, ctx)),
        Group::new(s.msg_zoom(), zoom_cols(s, )),
        Group::new(s.msg_window(), window_cols(s, )),
        Group::new(
            s.msg_macros(),
            vec![big(RibbonButton::new(Icon::Macro, s.msg_macros(), Action::Macros))],
        ),
    ]
}

fn views_cols(s: &'static crate::l10n::L10nLanguage) -> Vec<super::Col> {
    vec![big(
        RibbonButton::new(Icon::Fit, s.msg_presentation_mode(), Action::Macros)
            .enabled(false)
            .tip(s.msg_presentation_mode_planned()),
    )]
}

fn show_grid(s: &'static crate::l10n::L10nLanguage, ctx: &TabCtx) -> Vec<super::Col> {
    vec![grid(vec![
        vec![
            RibbonButton::new(Icon::Rulers, s.msg_rulers(), Action::ToggleRulers)
                .selected(ctx.show_rulers),
            RibbonButton::new(Icon::Grid, s.msg_grid(), Action::ToggleGrid)
                .selected(ctx.show_grid),
        ],
        vec![
            RibbonButton::new(Icon::Shapes, s.msg_guides(), Action::Macros)
                .enabled(false)
                .tip(s.msg_guides_planned_113_s_objects_exi()),
            RibbonButton::new(Icon::Delete, s.msg_page_breaks(), Action::Macros)
                .enabled(false)
                .tip(s.msg_page_breaks_planned()),
        ],
        vec![
            // Q4 default: the comments pane toggle stays on View (the
            // pane itself; Review owns comment authoring).
            RibbonButton::new(Icon::Comment, s.msg_comments(), Action::ToggleComments)
                .selected(ctx.show_comments),
            RibbonButton::new(Icon::Shapes, s.msg_task_panes(), Action::Macros)
                .enabled(false)
                .tip(s.msg_task_panes_shapes_pan_zoom_plann()),
        ],
    ])]
}

fn visual_aids_cols(s: &'static crate::l10n::L10nLanguage, ctx: &TabCtx) -> Vec<super::Col> {
    vec![
        stack(vec![
            RibbonButton::new(Icon::Grid, s.msg_dynamic_grid(), Action::ToggleDynamicGrid)
                .selected(ctx.dynamic_grid)
                .tip(s.msg_alignment_and_spacing_hints_whil()),
            RibbonButton::new(Icon::Shapes, s.msg_conn_points(), Action::Macros)
                .enabled(false)
                .tip(s.msg_show_connection_points_planned_m()),
        ]),
        stack(vec![
            RibbonButton::new(Icon::Magnet, s.msg_snap_glue(), Action::SnapGlue)
                .selected(ctx.snap_dlg_open)
                .tip(s.msg_configure_snap_and_glue_targets()),
            RibbonButton::new(Icon::Connector, s.msg_autoconnect(), Action::Macros)
                .enabled(false)
                .tip(s.msg_autoconnect_arrows_the_108_remai()),
        ]),
    ]
}

fn zoom_cols(s: &'static crate::l10n::L10nLanguage) -> Vec<super::Col> {
    vec![
        stack(vec![
            RibbonButton::new(Icon::ZoomIn, "", Action::ZoomIn)
                .alt(s.msg_zoom_in())
                .tip(s.msg_zoom_in_tip()),
            RibbonButton::new(Icon::ZoomOut, "", Action::ZoomOut)
                .alt(s.msg_zoom_out())
                .tip(s.msg_zoom_out_tip()),
        ]),
        grid(vec![
            vec![
                RibbonButton::new(Icon::ZoomIn, "100%", Action::Zoom100),
                // Visio's Whole Page (was "Fit Window").
                RibbonButton::new(Icon::Fit, s.msg_whole_page(), Action::FitPage)
                    .tip(s.msg_fit_the_whole_page_in_the_window()),
            ],
            vec![
                RibbonButton::new(Icon::Fit, s.msg_page_width(), Action::Macros)
                    .enabled(false)
                    .tip(s.msg_fit_the_page_width_planned()),
                RibbonButton::new(Icon::Shapes, s.msg_zoom_menu(), Action::Macros)
                    .enabled(false)
                    .tip(s.msg_zoom_dialog_planned()),
            ],
        ]),
        stack(vec![
            // Q5 default: kept (Shift+Z exists).
            RibbonButton::new(Icon::ZoomSelection, s.msg_selection(), Action::ZoomSelection)
                .tip(s.msg_zoom_to_selection_shift_z()),
        ]),
    ]
}

fn window_cols(s: &'static crate::l10n::L10nLanguage) -> Vec<super::Col> {
    vec![grid(vec![
        vec![
            // #246: the Language dialog — applies on selection, no
            // restart; the next frame re-renders from the catalog.
            RibbonButton::new(
                Icon::Translate,
                s.msg_language_menu(),
                Action::Language,
            )
            .tip(s.msg_choose_the_display_language_for()),
            RibbonButton::new(Icon::New, s.msg_new_window(), Action::Macros)
                .enabled(false)
                .tip(s.msg_new_window_planned_single_window()),
            RibbonButton::new(Icon::Shapes, s.msg_arrange_all(), Action::Macros)
                .enabled(false)
                .tip(s.msg_arrange_windows_planned()),
        ],
        vec![
            RibbonButton::new(Icon::Shapes, s.msg_split(), Action::Macros)
                .enabled(false)
                .tip(s.msg_split_view_planned()),
            RibbonButton::new(Icon::Fit, s.msg_switch_windows(), Action::Macros)
                .enabled(false)
                .tip(s.msg_switch_documents_the_tab_strip_s()),
        ],
    ])]
}
