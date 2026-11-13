//! Insert tab content (#252 fidelity sweep). Visio group order:
//! **Pages · Illustrations · Diagram Parts · Links · Text**. The
//! Comments group is gone (Review owns New Comment); the Container
//! gallery and Borders & Titles move in from Design, Legend from Data
//! (Design/Data drop them in their own commits). Page Setup's launcher
//! moves to Design ▸ Page Setup.

use super::{Action, Group, Icon, RibbonButton, TabCtx, big, grid, stack};

pub(crate) fn groups(s: &'static crate::l10n::L10nLanguage, ctx: &TabCtx) -> Vec<Group> {
    let has_doc = ctx.has_doc;
    let has_sel = ctx.has_sel;
    vec![
        Group::new(s.msg_insert_pages(), pages_cols(s, has_doc)),
        Group::new(s.msg_illustrations(), illustrations_cols(s, )),
        Group::new(s.msg_diagram_parts(), diagram_parts_cols(s, ctx)),
        Group::new(
            s.msg_links(),
            vec![big(
                RibbonButton::new(Icon::Link, s.msg_hyperlink(), Action::Hyperlink)
                    .enabled(has_sel)
                    .tip(s.msg_edit_the_selected_shape_s_hyperl()),
            )],
        ),
        Group::new(s.msg_text(), text_cols(s, )),
    ]
}

fn pages_cols(s: &'static crate::l10n::L10nLanguage, has_doc: bool) -> Vec<super::Col> {
    vec![
        big(RibbonButton::new(Icon::NewPage, s.msg_new_page(), Action::NewPage).enabled(has_doc)),
        // Visio's New Page split: plain page / background page (the
        // background variant is planned; assignment lives in
        // Design ▸ Backgrounds today).
        stack(vec![RibbonButton::new(
            Icon::Picture,
            s.msg_new_background_page(),
            Action::Macros,
        )
        .enabled(false)
        .tip(s.msg_create_a_background_page_planned())]),
    ]
}

fn illustrations_cols(s: &'static crate::l10n::L10nLanguage) -> Vec<super::Col> {
    vec![
        big(
            RibbonButton::new(Icon::Picture, s.msg_pictures(), Action::InsertPicture)
                .tip(s.msg_insert_a_png_or_jpeg_picture_fro()),
        ),
        grid(vec![
            vec![
                RibbonButton::new(Icon::OnlinePic, "", Action::Macros)
                    .alt(s.msg_online_pictures())
                    .enabled(false)
                    .tip(s.msg_online_pictures_planned()),
                RibbonButton::new(Icon::DataGraphic, "", Action::Macros)
                    .alt(s.msg_chart())
                    .enabled(false)
                    .tip(s.msg_charts_planned_later_milestone()),
            ],
            vec![
                RibbonButton::new(Icon::Shapes, "", Action::Macros)
                    .alt(s.msg_cad_drawing())
                    .enabled(false)
                    .tip(s.msg_cad_drawing_import_planned_later()),
            ],
        ]),
    ]
}

fn diagram_parts_cols(s: &'static crate::l10n::L10nLanguage, ctx: &TabCtx) -> Vec<super::Col> {
    vec![
        big(
            RibbonButton::new(Icon::Container, s.msg_container(), Action::Macros)
                .enabled(false)
                .tip(s.msg_containers_planned()),
        ),
        big(
            RibbonButton::new(Icon::Callout, s.msg_callout(), Action::Macros)
                .enabled(false)
                .tip(s.msg_callouts_planned()),
        ),
        grid(vec![
            vec![
                // #156: the SDCONT gallery (moved from Design).
                RibbonButton::new(Icon::Container, "", Action::ContainerGallery)
                    .alt(s.msg_containers())
                    .selected(ctx.contgal_open)
                    .tip(s.msg_containers_wrap_tip()),
                RibbonButton::new(Icon::Connector, "", Action::ConnectorTool)
                    .alt(s.msg_connector())
                    .selected(ctx.tool == crate::app::Tool::Connector)
                    .tip(s.msg_connector_tool_tip()),
            ],
            vec![
                RibbonButton::new(Icon::Shapes, "", Action::Macros)
                    .alt(s.msg_legend())
                    .enabled(false)
                    .tip(s.msg_legends_m6_visio_paints_legend_o()),
                RibbonButton::new(Icon::Title, "", Action::Macros)
                    .alt(s.msg_title())
                    .enabled(false)
                    .tip(s.msg_title_planned()),
            ],
            vec![
                RibbonButton::new(Icon::Picture, "", Action::Macros)
                    .alt(s.msg_border_title())
                    .enabled(false)
                    .tip(s.msg_borders_titles_planned()),
            ],
        ]),
    ]
}

fn text_cols(s: &'static crate::l10n::L10nLanguage) -> Vec<super::Col> {
    vec![
        big(
            RibbonButton::new(Icon::TextBox, s.msg_text_box(), Action::Macros)
                .enabled(false)
                .tip(s.msg_text_shapes_planned()),
        ),
        grid(vec![
            vec![
                RibbonButton::new(Icon::Field, "", Action::Macros)
                    .alt(s.msg_text_field())
                    .enabled(false)
                    .tip(s.msg_fields_planned()),
                RibbonButton::new(Icon::DateTime, "", Action::Macros)
                    .alt(s.msg_date_time())
                    .enabled(false)
                    .tip(s.msg_date_time_field_planned()),
            ],
            vec![
                RibbonButton::new(Icon::NewPage, "", Action::Macros)
                    .alt(s.msg_page_number())
                    .enabled(false)
                    .tip(s.msg_page_number_field_planned()),
                RibbonButton::new(Icon::FontA, "", Action::Macros)
                    .alt(s.msg_symbol())
                    .enabled(false)
                    .tip(s.msg_symbol_planned()),
            ],
        ]),
    ]
}
