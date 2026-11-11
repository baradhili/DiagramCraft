//! Home tab content (#252 fidelity sweep). Visio group order:
//! **Clipboard · Font · Paragraph · Tools · Shape Styles · Arrange ·
//! Editing**. The Select cluster (Select All / Select by Type /
//! Lasso) rides beside Clipboard pending Visio's Select split-button
//! dropdown; Sub/Superscript, Clear Formatting and Line Spacing moved
//! to the text dialog's tabs per the #252 disposition. Compact tiers
//! (icon-only tool grid, 2-wide Arrange clusters) keep the WIRED
//! groups inside a 1280 px window; the all-placeholder Editing group
//! is last and may clip on narrow windows, as Visio's own collapsed
//! tier does.

use super::{Action, Group, Icon, RibbonButton, TabCtx, big, grid, stack};
use crate::app::Tool;

pub(crate) fn groups(ctx: &TabCtx) -> Vec<Group> {
    let has_sel = ctx.has_sel;
    let multi_sel = ctx.multi_sel;
    let tri_sel = ctx.tri_sel;
    vec![
        Group::new(crate::i18n::strs().msg_clipboard(), clipboard_cols(ctx, has_sel)),
        Group::new(crate::i18n::strs().msg_font(), vec![font_grid(has_sel)]).with_launcher(),
        Group::new(crate::i18n::strs().msg_paragraph(), vec![paragraph_grid()]).with_launcher(),
        Group::new(crate::i18n::strs().msg_tools(), tools_cols(ctx)),
        Group::new(crate::i18n::strs().msg_shape_styles(), shape_styles_cols(has_sel)),
        Group::new(crate::i18n::strs().msg_arrange(), arrange_cols(ctx, multi_sel, tri_sel)),
        // #252: Visio's Editing group — Find/Replace/Clear are
        // placeholders (no backend yet); Select stays beside
        // Clipboard until the Select dropdown lands.
        Group::new(
            crate::i18n::strs().msg_editing(),
            vec![grid(vec![
                vec![
                    RibbonButton::new(Icon::Find, "", Action::Macros)
                        .alt(crate::i18n::strs().msg_find())
                        .enabled(false)
                        .tip(crate::i18n::strs().msg_find_planned_later_milestone()),
                    RibbonButton::new(Icon::FindReplace, "", Action::Macros)
                        .alt(crate::i18n::strs().msg_replace())
                        .enabled(false)
                        .tip(crate::i18n::strs().msg_replace_planned_later_milestone()),
                ],
                vec![
                    RibbonButton::new(Icon::Eraser, "", Action::Macros)
                        .alt(crate::i18n::strs().msg_clear())
                        .enabled(false)
                        .tip(crate::i18n::strs().msg_clear_planned_later_milestone()),
                ],
            ])],
        ),
    ]
}

fn clipboard_cols(ctx: &TabCtx, has_sel: bool) -> Vec<super::Col> {
    vec![
        big(
            RibbonButton::new(Icon::Paste, crate::i18n::strs().msg_paste(), Action::Macros)
                .enabled(false)
                .split()
                .tip(crate::i18n::strs().msg_clipboard_planned_for_a_later_mi()),
        ),
        grid(vec![
            vec![
                RibbonButton::new(Icon::Cut, "", Action::Macros)
                    .alt(crate::i18n::strs().msg_cut())
                    .enabled(false)
                    .tip(crate::i18n::strs().msg_cut_planned()),
                RibbonButton::new(Icon::Copy, "", Action::Macros)
                    .alt(crate::i18n::strs().msg_copy())
                    .enabled(false)
                    .tip(crate::i18n::strs().msg_copy_planned()),
            ],
            vec![RibbonButton::new(
                Icon::Painter,
                crate::i18n::strs().msg_format_painter(),
                Action::FormatPainter,
            )
            .enabled(has_sel)
            .selected(ctx.painter_armed)
            .tip(
                "Copy the selection's formatting to the next shape you click; \
             double-click to keep painting until Esc",
            )],
        ]),
        // Visio folds these into the Editing group's Select dropdown;
        // they stay a painted cluster beside Clipboard until that
        // dropdown lands (#110: Select All + Select by Type +
        // URS-1160, plus the freehand-trail Lasso).
        grid(vec![
            vec![
                RibbonButton::new(Icon::Shapes, "", Action::SelectAll)
                    .alt(crate::i18n::strs().msg_select_all())
                    .tip(crate::i18n::strs().msg_select_every_shape_on_the_page_c()),
                RibbonButton::new(Icon::Shapes, "", Action::SelectByType)
                    .alt(crate::i18n::strs().msg_select_by_type_menu())
                    .selected(ctx.selby_open)
                    .tip(crate::i18n::strs().msg_select_shapes_by_type_master_or()),
            ],
            vec![RibbonButton::new(Icon::Lasso, "", Action::LassoTool)
                .alt(crate::i18n::strs().msg_lasso_select())
                .selected(ctx.tool == Tool::Lasso)
                .tip(
                    "Draw a freehand trail; shapes the trail touches or encloses \
                 are selected (Esc to finish)",
                )],
        ]),
    ]
}

fn font_grid(has_sel: bool) -> super::Col {
    grid(vec![
        vec![
            RibbonButton::new(Icon::FontA, crate::i18n::strs().msg_calibri(), Action::Macros)
                .combo()
                .width(96.0)
                .enabled(false)
                .tip(crate::i18n::strs().msg_font_family_planned()),
            RibbonButton::new(Icon::FontA, "11", Action::Macros)
                .combo()
                .width(40.0)
                .enabled(false)
                .tip(crate::i18n::strs().msg_font_size_planned()),
            RibbonButton::new(Icon::Grow, "", Action::FontGrow)
                .alt(crate::i18n::strs().msg_font_increase())
                .enabled(has_sel)
                .tip(crate::i18n::strs().msg_font_increase_tip()),
            RibbonButton::new(Icon::Shrink, "", Action::FontShrink)
                .alt(crate::i18n::strs().msg_font_decrease())
                .enabled(has_sel)
                .tip(crate::i18n::strs().msg_font_decrease_tip()),
        ],
        vec![
            RibbonButton::new(Icon::Bold, "", Action::Macros)
                .alt(crate::i18n::strs().msg_bold())
                .enabled(false)
                .tip(crate::i18n::strs().msg_bold_planned()),
            RibbonButton::new(Icon::Italic, "", Action::Macros)
                .alt(crate::i18n::strs().msg_italic())
                .enabled(false)
                .tip(crate::i18n::strs().msg_italic_planned()),
            RibbonButton::new(Icon::Underline, "", Action::Macros)
                .alt(crate::i18n::strs().msg_underline())
                .enabled(false)
                .tip(crate::i18n::strs().msg_underline_planned()),
            RibbonButton::new(Icon::Strike, "", Action::Macros)
                .alt(crate::i18n::strs().msg_strikethrough())
                .enabled(false)
                .tip(crate::i18n::strs().msg_strikethrough_planned()),
            RibbonButton::new(Icon::TextColor, "", Action::Macros)
                .alt(crate::i18n::strs().msg_text_color())
                .enabled(false)
                .tip(crate::i18n::strs().msg_text_color_planned()),
        ],
    ])
}

fn paragraph_grid() -> super::Col {
    grid(vec![
        vec![
            RibbonButton::new(Icon::Align, "", Action::Macros)
                .alt(crate::i18n::strs().msg_text_align_left())
                .enabled(false)
                .tip(crate::i18n::strs().msg_align_left_planned()),
            RibbonButton::new(Icon::AlignCenter, "", Action::Macros)
                .alt(crate::i18n::strs().msg_align_center())
                .enabled(false)
                .tip(crate::i18n::strs().msg_align_center_planned()),
            RibbonButton::new(Icon::AlignRight, "", Action::Macros)
                .alt(crate::i18n::strs().msg_text_align_right())
                .enabled(false)
                .tip(crate::i18n::strs().msg_align_right_planned()),
            RibbonButton::new(Icon::Justify, "", Action::Macros)
                .alt(crate::i18n::strs().msg_justify())
                .enabled(false)
                .tip(crate::i18n::strs().msg_justify_planned()),
        ],
        vec![
            RibbonButton::new(Icon::Bullets, "", Action::Macros)
                .alt(crate::i18n::strs().msg_bullets())
                .enabled(false)
                .tip(crate::i18n::strs().msg_bullets_planned()),
            RibbonButton::new(Icon::Numbering, "", Action::Macros)
                .alt(crate::i18n::strs().msg_numbering())
                .enabled(false)
                .tip(crate::i18n::strs().msg_numbering_planned()),
            RibbonButton::new(Icon::IndentLess, "", Action::Macros)
                .alt(crate::i18n::strs().msg_decrease_indent())
                .enabled(false)
                .tip(crate::i18n::strs().msg_decrease_indent_planned()),
            RibbonButton::new(Icon::IndentMore, "", Action::Macros)
                .alt(crate::i18n::strs().msg_increase_indent())
                .enabled(false)
                .tip(crate::i18n::strs().msg_increase_indent_planned()),
        ],
    ])
}

fn tools_cols(ctx: &TabCtx) -> Vec<super::Col> {
    vec![
        stack(vec![
            RibbonButton::new(Icon::Pointer, "", Action::PointerTool)
                .alt(crate::i18n::strs().msg_pointer_tool())
                .selected(ctx.tool == Tool::Pointer)
                .tip(crate::i18n::strs().msg_pointer_tool_select_and_drag_esc()),
            RibbonButton::new(Icon::Connector, "", Action::ConnectorTool)
                .alt(crate::i18n::strs().msg_connector())
                .selected(ctx.tool == Tool::Connector)
                .tip(crate::i18n::strs().msg_connector_tool_status()),
        ]),
        stack(vec![
            RibbonButton::new(Icon::ConnPoint, "", Action::ConnPointTool)
                .alt(crate::i18n::strs().msg_conn_point())
                .selected(ctx.tool == Tool::ConnectionPoint)
                .tip(
                    "Connection Point tool: click a shape to add a connection \
                 point; click one to select it (Delete removes)",
                ),
            RibbonButton::new(Icon::Pen, "", Action::FreeformTool)
                .alt(crate::i18n::strs().msg_freeform())
                .selected(ctx.tool == Tool::Freeform)
                .tip(
                    "Freeform tool (#132): drag on the page to draw freehand \
                 geometry — the stroke becomes an editable shape (Esc finishes)",
                ),
        ]),
        // Visio's shape-drawing tools, compact icon tier (planned).
        grid(vec![
            vec![
                RibbonButton::new(Icon::TextTool, "", Action::Macros)
                    .alt(crate::i18n::strs().msg_text_tool())
                    .enabled(false)
                    .tip(crate::i18n::strs().msg_text_tool_planned_double_click_e()),
                RibbonButton::new(Icon::RectTool, "", Action::Macros)
                    .alt(crate::i18n::strs().msg_rectangle_tool())
                    .enabled(false)
                    .tip(crate::i18n::strs().msg_rectangle_tool_planned()),
            ],
            vec![
                RibbonButton::new(Icon::EllipseTool, "", Action::Macros)
                    .alt(crate::i18n::strs().msg_ellipse_tool())
                    .enabled(false)
                    .tip(crate::i18n::strs().msg_ellipse_tool_planned()),
                RibbonButton::new(Icon::LineTool, "", Action::Macros)
                    .alt(crate::i18n::strs().msg_line_tool())
                    .enabled(false)
                    .tip(crate::i18n::strs().msg_line_tool_planned()),
            ],
            vec![
                RibbonButton::new(Icon::Stamp, "", Action::Macros)
                    .alt(crate::i18n::strs().msg_stamp_tool())
                    .enabled(false)
                    .tip(crate::i18n::strs().msg_stamp_tool_planned()),
            ],
        ]),
    ]
}

fn shape_styles_cols(has_sel: bool) -> Vec<super::Col> {
    vec![
        big(
            RibbonButton::new(Icon::Theme, crate::i18n::strs().msg_quick_styles(), Action::Macros)
                .enabled(false)
                .tip(crate::i18n::strs().msg_style_gallery_planned()),
        ),
        stack(vec![
            RibbonButton::new(Icon::Bucket, crate::i18n::strs().msg_fill(), Action::FillMenu)
                .enabled(has_sel)
                .tip(crate::i18n::strs().msg_shape_fill_color()),
            RibbonButton::new(Icon::Pen, crate::i18n::strs().msg_line(), Action::LineMenu)
                .enabled(has_sel)
                .tip(crate::i18n::strs().msg_line_color_label()),
            RibbonButton::new(Icon::Effects, crate::i18n::strs().msg_effects(), Action::Macros)
                .enabled(false)
                .tip(crate::i18n::strs().msg_shape_effects_planned()),
        ]),
    ]
}

fn arrange_cols(ctx: &TabCtx, multi_sel: bool, tri_sel: bool) -> Vec<super::Col> {
    vec![
        stack(vec![
            RibbonButton::new(Icon::BringFront, "", Action::Macros)
                .alt(crate::i18n::strs().msg_bring_forward())
                .enabled(false)
                .tip(crate::i18n::strs().msg_bring_forward_planned()),
            RibbonButton::new(Icon::SendBack, "", Action::Macros)
                .alt(crate::i18n::strs().msg_send_backward())
                .enabled(false)
                .tip(crate::i18n::strs().msg_send_backward_planned()),
        ]),
        grid(vec![
            vec![
                RibbonButton::new(Icon::Group, "", Action::Group)
                    .alt(crate::i18n::strs().msg_group())
                    .enabled(multi_sel)
                    .tip(crate::i18n::strs().msg_group_the_selection_one_undo_ste()),
                RibbonButton::new(Icon::Group, "", Action::Ungroup)
                    .alt(crate::i18n::strs().msg_ungroup())
                    .enabled(ctx.sel_group)
                    .tip(crate::i18n::strs().msg_ungroup_the_selected_group_s()),
            ],
            vec![
                RibbonButton::new(Icon::Rotate, "", Action::RotateLeft)
                    .alt(crate::i18n::strs().msg_rotate_left())
                    .enabled(ctx.has_sel)
                    .tip(crate::i18n::strs().msg_rotate_the_selection_90_counter()),
                RibbonButton::new(Icon::Rotate, "", Action::RotateRight)
                    .alt(crate::i18n::strs().msg_rotate_right())
                    .enabled(ctx.has_sel)
                    .tip(crate::i18n::strs().msg_rotate_the_selection_90_clockwis()),
            ],
            vec![
                RibbonButton::new(Icon::SelectionPane, "", Action::Macros)
                    .alt(crate::i18n::strs().msg_selection_pane())
                    .enabled(false)
                    .tip(crate::i18n::strs().msg_selection_pane_planned_112_shell()),
                RibbonButton::new(Icon::Check, "", Action::AlignToPage)
                    .alt(crate::i18n::strs().msg_to_page())
                    .selected(ctx.align_to_page)
                    .tip(crate::i18n::strs().msg_align_against_the_page_instead_o()),
            ],
        ]),
        grid(vec![
            vec![
                RibbonButton::new(Icon::Align, "", Action::AlignLeft)
                    .alt(crate::i18n::strs().msg_align_left())
                    .enabled(multi_sel)
                    .tip(crate::i18n::strs().msg_align_left_edges_first_selected()),
                RibbonButton::new(Icon::AlignCenter, "", Action::AlignHCenter)
                    .alt(crate::i18n::strs().msg_center())
                    .enabled(multi_sel)
                    .tip(crate::i18n::strs().msg_align_vertical_centers()),
                RibbonButton::new(Icon::AlignRight, "", Action::AlignRight)
                    .alt(crate::i18n::strs().msg_align_right())
                    .enabled(multi_sel)
                    .tip(crate::i18n::strs().msg_align_right_edges()),
            ],
            vec![
                RibbonButton::new(Icon::AlignTop, "", Action::AlignTop)
                    .alt(crate::i18n::strs().msg_align_top())
                    .enabled(multi_sel)
                    .tip(crate::i18n::strs().msg_align_top_edges()),
                RibbonButton::new(Icon::AlignMiddle, "", Action::AlignVCenter)
                    .alt(crate::i18n::strs().msg_middle())
                    .enabled(multi_sel)
                    .tip(crate::i18n::strs().msg_align_horizontal_middles()),
                RibbonButton::new(Icon::AlignBottom, "", Action::AlignBottom)
                    .alt(crate::i18n::strs().msg_align_bottom())
                    .enabled(multi_sel)
                    .tip(crate::i18n::strs().msg_align_bottom_edges()),
            ],
            vec![
                RibbonButton::new(Icon::DistH, "", Action::DistributeH)
                    .alt(crate::i18n::strs().msg_dist_h())
                    .enabled(tri_sel)
                    .tip(crate::i18n::strs().msg_even_horizontal_spacing_3_shapes()),
                RibbonButton::new(Icon::DistV, "", Action::DistributeV)
                    .alt(crate::i18n::strs().msg_dist_v())
                    .enabled(tri_sel)
                    .tip(crate::i18n::strs().msg_even_vertical_spacing_3_shapes()),
            ],
        ]),
    ]
}
