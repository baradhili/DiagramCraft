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
        Group::new("Clipboard", clipboard_cols(ctx, has_sel)),
        Group::new("Font", vec![font_grid(has_sel)]).with_launcher(),
        Group::new("Paragraph", vec![paragraph_grid()]).with_launcher(),
        Group::new("Tools", tools_cols(ctx)),
        Group::new("Shape Styles", shape_styles_cols(has_sel)),
        Group::new("Arrange", arrange_cols(ctx, multi_sel, tri_sel)),
        // #252: Visio's Editing group — Find/Replace/Clear are
        // placeholders (no backend yet); Select stays beside
        // Clipboard until the Select dropdown lands.
        Group::new(
            "Editing",
            vec![grid(vec![
                vec![
                    RibbonButton::new(Icon::Find, "", Action::Macros)
                        .alt("Find")
                        .enabled(false)
                        .tip("Find — planned (later milestone)"),
                    RibbonButton::new(Icon::FindReplace, "", Action::Macros)
                        .alt("Replace")
                        .enabled(false)
                        .tip("Replace — planned (later milestone)"),
                ],
                vec![
                    RibbonButton::new(Icon::Eraser, "", Action::Macros)
                        .alt("Clear")
                        .enabled(false)
                        .tip("Clear — planned (later milestone)"),
                ],
            ])],
        ),
    ]
}

fn clipboard_cols(ctx: &TabCtx, has_sel: bool) -> Vec<super::Col> {
    vec![
        big(
            RibbonButton::new(Icon::Paste, "Paste", Action::Macros)
                .enabled(false)
                .split()
                .tip("Clipboard — planned for a later milestone"),
        ),
        grid(vec![
            vec![
                RibbonButton::new(Icon::Cut, "", Action::Macros)
                    .alt("Cut")
                    .enabled(false)
                    .tip("Cut — planned"),
                RibbonButton::new(Icon::Copy, "", Action::Macros)
                    .alt("Copy")
                    .enabled(false)
                    .tip("Copy — planned"),
            ],
            vec![RibbonButton::new(
                Icon::Painter,
                "Format Painter",
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
                    .alt("Select All")
                    .tip("Select every shape on the page (Ctrl+A)"),
                RibbonButton::new(Icon::Shapes, "", Action::SelectByType)
                    .alt("Select by Type…")
                    .selected(ctx.selby_open)
                    .tip("Select shapes by type, master, or layer"),
            ],
            vec![RibbonButton::new(Icon::Lasso, "", Action::LassoTool)
                .alt("Lasso Select")
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
            RibbonButton::new(Icon::FontA, "Calibri", Action::Macros)
                .combo()
                .width(96.0)
                .enabled(false)
                .tip("Font family — planned"),
            RibbonButton::new(Icon::FontA, "11", Action::Macros)
                .combo()
                .width(40.0)
                .enabled(false)
                .tip("Font size — planned"),
            RibbonButton::new(Icon::Grow, "", Action::FontGrow)
                .alt("Increase Font Size")
                .enabled(has_sel)
                .tip("Increase font size"),
            RibbonButton::new(Icon::Shrink, "", Action::FontShrink)
                .alt("Decrease Font Size")
                .enabled(has_sel)
                .tip("Decrease font size"),
        ],
        vec![
            RibbonButton::new(Icon::Bold, "", Action::Macros)
                .alt("Bold")
                .enabled(false)
                .tip("Bold — planned"),
            RibbonButton::new(Icon::Italic, "", Action::Macros)
                .alt("Italic")
                .enabled(false)
                .tip("Italic — planned"),
            RibbonButton::new(Icon::Underline, "", Action::Macros)
                .alt("Underline")
                .enabled(false)
                .tip("Underline — planned"),
            RibbonButton::new(Icon::Strike, "", Action::Macros)
                .alt("Strikethrough")
                .enabled(false)
                .tip("Strikethrough — planned"),
            RibbonButton::new(Icon::TextColor, "", Action::Macros)
                .alt("Text Color")
                .enabled(false)
                .tip("Text color — planned"),
        ],
    ])
}

fn paragraph_grid() -> super::Col {
    grid(vec![
        vec![
            RibbonButton::new(Icon::Align, "", Action::Macros)
                .alt("Align Left")
                .enabled(false)
                .tip("Align left — planned"),
            RibbonButton::new(Icon::AlignCenter, "", Action::Macros)
                .alt("Align Center")
                .enabled(false)
                .tip("Align center — planned"),
            RibbonButton::new(Icon::AlignRight, "", Action::Macros)
                .alt("Align Right")
                .enabled(false)
                .tip("Align right — planned"),
            RibbonButton::new(Icon::Justify, "", Action::Macros)
                .alt("Justify")
                .enabled(false)
                .tip("Justify — planned"),
        ],
        vec![
            RibbonButton::new(Icon::Bullets, "", Action::Macros)
                .alt("Bullets")
                .enabled(false)
                .tip("Bullets — planned"),
            RibbonButton::new(Icon::Numbering, "", Action::Macros)
                .alt("Numbering")
                .enabled(false)
                .tip("Numbering — planned"),
            RibbonButton::new(Icon::IndentLess, "", Action::Macros)
                .alt("Decrease Indent")
                .enabled(false)
                .tip("Decrease indent — planned"),
            RibbonButton::new(Icon::IndentMore, "", Action::Macros)
                .alt("Increase Indent")
                .enabled(false)
                .tip("Increase indent — planned"),
        ],
    ])
}

fn tools_cols(ctx: &TabCtx) -> Vec<super::Col> {
    vec![
        stack(vec![
            RibbonButton::new(Icon::Pointer, "", Action::PointerTool)
                .alt("Pointer Tool")
                .selected(ctx.tool == Tool::Pointer)
                .tip("Pointer tool — select and drag (Esc from the Connector Tool)"),
            RibbonButton::new(Icon::Connector, "", Action::ConnectorTool)
                .alt("Connector")
                .selected(ctx.tool == Tool::Connector)
                .tip("Connector tool — draw a connector between shapes or points (Esc to finish)"),
        ]),
        stack(vec![
            RibbonButton::new(Icon::ConnPoint, "", Action::ConnPointTool)
                .alt("Conn Point")
                .selected(ctx.tool == Tool::ConnectionPoint)
                .tip(
                    "Connection Point tool: click a shape to add a connection \
                 point; click one to select it (Delete removes)",
                ),
            RibbonButton::new(Icon::Pen, "", Action::FreeformTool)
                .alt("Freeform")
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
                    .alt("Text Tool")
                    .enabled(false)
                    .tip("Text tool — planned (double-click edits text today)"),
                RibbonButton::new(Icon::RectTool, "", Action::Macros)
                    .alt("Rectangle Tool")
                    .enabled(false)
                    .tip("Rectangle tool — planned"),
            ],
            vec![
                RibbonButton::new(Icon::EllipseTool, "", Action::Macros)
                    .alt("Ellipse Tool")
                    .enabled(false)
                    .tip("Ellipse tool — planned"),
                RibbonButton::new(Icon::LineTool, "", Action::Macros)
                    .alt("Line Tool")
                    .enabled(false)
                    .tip("Line tool — planned"),
            ],
            vec![
                RibbonButton::new(Icon::Stamp, "", Action::Macros)
                    .alt("Stamp Tool")
                    .enabled(false)
                    .tip("Stamp tool — planned"),
            ],
        ]),
    ]
}

fn shape_styles_cols(has_sel: bool) -> Vec<super::Col> {
    vec![
        big(
            RibbonButton::new(Icon::Theme, "Quick Styles", Action::Macros)
                .enabled(false)
                .tip("Style gallery — planned"),
        ),
        stack(vec![
            RibbonButton::new(Icon::Bucket, "Fill", Action::FillMenu)
                .enabled(has_sel)
                .tip("Shape fill color"),
            RibbonButton::new(Icon::Pen, "Line", Action::LineMenu)
                .enabled(has_sel)
                .tip("Line color"),
            RibbonButton::new(Icon::Effects, "Effects", Action::Macros)
                .enabled(false)
                .tip("Shape effects — planned"),
        ]),
    ]
}

fn arrange_cols(ctx: &TabCtx, multi_sel: bool, tri_sel: bool) -> Vec<super::Col> {
    vec![
        stack(vec![
            RibbonButton::new(Icon::BringFront, "", Action::Macros)
                .alt("Bring Forward")
                .enabled(false)
                .tip("Bring forward — planned"),
            RibbonButton::new(Icon::SendBack, "", Action::Macros)
                .alt("Send Backward")
                .enabled(false)
                .tip("Send backward — planned"),
        ]),
        grid(vec![
            vec![
                RibbonButton::new(Icon::Group, "", Action::Group)
                    .alt("Group")
                    .enabled(multi_sel)
                    .tip("Group the selection (one undo step; nests into an existing group)"),
                RibbonButton::new(Icon::Group, "", Action::Ungroup)
                    .alt("Ungroup")
                    .enabled(ctx.sel_group)
                    .tip("Ungroup the selected group(s)"),
            ],
            vec![
                RibbonButton::new(Icon::Rotate, "", Action::RotateLeft)
                    .alt("Rotate Left")
                    .enabled(ctx.has_sel)
                    .tip("Rotate the selection 90° counter-clockwise"),
                RibbonButton::new(Icon::Rotate, "", Action::RotateRight)
                    .alt("Rotate Right")
                    .enabled(ctx.has_sel)
                    .tip("Rotate the selection 90° clockwise"),
            ],
            vec![
                RibbonButton::new(Icon::SelectionPane, "", Action::Macros)
                    .alt("Selection Pane")
                    .enabled(false)
                    .tip("Selection pane — planned (#112 shell depth)"),
                RibbonButton::new(Icon::Check, "", Action::AlignToPage)
                    .alt("To Page")
                    .selected(ctx.align_to_page)
                    .tip("Align against the page instead of the primary selection"),
            ],
        ]),
        grid(vec![
            vec![
                RibbonButton::new(Icon::Align, "", Action::AlignLeft)
                    .alt("Left")
                    .enabled(multi_sel)
                    .tip("Align left edges (first-selected or page)"),
                RibbonButton::new(Icon::AlignCenter, "", Action::AlignHCenter)
                    .alt("Center")
                    .enabled(multi_sel)
                    .tip("Align vertical centers"),
                RibbonButton::new(Icon::AlignRight, "", Action::AlignRight)
                    .alt("Right")
                    .enabled(multi_sel)
                    .tip("Align right edges"),
            ],
            vec![
                RibbonButton::new(Icon::AlignTop, "", Action::AlignTop)
                    .alt("Top")
                    .enabled(multi_sel)
                    .tip("Align top edges"),
                RibbonButton::new(Icon::AlignMiddle, "", Action::AlignVCenter)
                    .alt("Middle")
                    .enabled(multi_sel)
                    .tip("Align horizontal middles"),
                RibbonButton::new(Icon::AlignBottom, "", Action::AlignBottom)
                    .alt("Bottom")
                    .enabled(multi_sel)
                    .tip("Align bottom edges"),
            ],
            vec![
                RibbonButton::new(Icon::DistH, "", Action::DistributeH)
                    .alt("Dist H")
                    .enabled(tri_sel)
                    .tip("Even horizontal spacing (3+ shapes)"),
                RibbonButton::new(Icon::DistV, "", Action::DistributeV)
                    .alt("Dist V")
                    .enabled(tri_sel)
                    .tip("Even vertical spacing (3+ shapes)"),
            ],
        ]),
    ]
}
