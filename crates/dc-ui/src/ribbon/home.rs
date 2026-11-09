//! Home tab content (#252) — moved verbatim from `app.rs` (commit 1
//! scaffolding); the per-tab fidelity sweep reshapes this in its own
//! commit.

use super::{Action, Group, Icon, RibbonButton, TabCtx, big, grid, stack};
use crate::app::Tool;

pub(crate) fn groups(ctx: &TabCtx) -> Vec<Group> {
    let has_sel = ctx.has_sel;
    let multi_sel = ctx.multi_sel;
    let tri_sel = ctx.tri_sel;
    vec![
        Group::new(
            "Tools",
            vec![stack(vec![
                RibbonButton::new(Icon::Pointer, "Pointer Tool", Action::PointerTool)
                    .selected(ctx.tool == Tool::Pointer)
                    .tip("Select and drag (Esc from the Connector Tool)"),
                RibbonButton::new(Icon::Connector, "Connector", Action::ConnectorTool)
                    .selected(ctx.tool == Tool::Connector)
                    .tip("Draw a connector between shapes or points (Esc to finish)"),
                RibbonButton::new(Icon::ConnPoint, "Conn Point", Action::ConnPointTool)
                    .width(90.0)
                    .selected(ctx.tool == Tool::ConnectionPoint)
                    .tip(
                        "Connection Point tool: click a shape to add a connection \
                     point; click one to select it (Delete removes)",
                    ),
                RibbonButton::new(Icon::Pen, "Freeform", Action::FreeformTool)
                    .selected(ctx.tool == Tool::Freeform)
                    .tip(
                        "Freeform tool (#132): drag on the page to draw freehand \
                     geometry — the stroke becomes an editable shape (Esc finishes)",
                    ),
            ])],
        ),
        Group::new(
            "Clipboard",
            vec![
                big(
                    RibbonButton::new(Icon::Paste, "Paste", Action::Macros)
                        .enabled(false)
                        .split()
                        .tip("Clipboard — planned for a later milestone"),
                ),
                grid(vec![
                    vec![
                        RibbonButton::new(Icon::Cut, "Cut", Action::Macros)
                            .enabled(false)
                            .tip("Clipboard — planned"),
                        RibbonButton::new(Icon::Copy, "Copy", Action::Macros)
                            .enabled(false)
                            .tip("Clipboard — planned"),
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
                // Visio's Editing group rides beside Clipboard:
                // Select All + Select by Type (#110, URS-1160),
                // plus the Select menu's Lasso Select (#110
                // remainder: the freehand trail).
                grid(vec![
                    vec![
                        RibbonButton::new(Icon::Shapes, "Select All", Action::SelectAll)
                            .tip("Select every shape on the page (Ctrl+A)"),
                        RibbonButton::new(
                            Icon::Shapes,
                            "Select by Type…",
                            Action::SelectByType,
                        )
                        .selected(ctx.selby_open)
                        .tip("Select shapes by type, master, or layer"),
                    ],
                    vec![RibbonButton::new(Icon::Lasso, "Lasso Select", Action::LassoTool)
                        .selected(ctx.tool == Tool::Lasso)
                        .tip(
                            "Draw a freehand trail; shapes the trail touches or encloses \
                         are selected (Esc to finish)",
                        )],
                ]),
            ],
        ),
        Group::new(
            "Font",
            vec![grid(vec![
                vec![
                    RibbonButton::new(Icon::FontA, "Calibri", Action::Macros)
                        .combo()
                        .width(128.0)
                        .enabled(false)
                        .tip("Font family — planned"),
                    RibbonButton::new(Icon::FontA, "11", Action::Macros)
                        .combo()
                        .width(46.0)
                        .enabled(false)
                        .tip("Font size — planned"),
                    RibbonButton::new(Icon::Grow, "", Action::FontGrow)
                        .enabled(has_sel)
                        .tip("Increase font size"),
                    RibbonButton::new(Icon::Shrink, "", Action::FontShrink)
                        .enabled(has_sel)
                        .tip("Decrease font size"),
                    RibbonButton::new(Icon::ClearAll, "", Action::Macros)
                        .enabled(false)
                        .tip("Clear all formatting — planned"),
                ],
                vec![
                    RibbonButton::new(Icon::Bold, "", Action::Macros)
                        .enabled(false)
                        .tip("Bold — planned"),
                    RibbonButton::new(Icon::Italic, "", Action::Macros)
                        .enabled(false)
                        .tip("Italic — planned"),
                    RibbonButton::new(Icon::Underline, "", Action::Macros)
                        .enabled(false)
                        .tip("Underline — planned"),
                    RibbonButton::new(Icon::Strike, "", Action::Macros)
                        .enabled(false)
                        .tip("Strikethrough — planned"),
                    RibbonButton::new(Icon::Subscript, "", Action::Macros)
                        .enabled(false)
                        .tip("Subscript — planned"),
                    RibbonButton::new(Icon::Superscript, "", Action::Macros)
                        .enabled(false)
                        .tip("Superscript — planned"),
                    RibbonButton::new(Icon::TextColor, "", Action::Macros)
                        .enabled(false)
                        .tip("Text color — planned"),
                ],
            ])],
        )
        .with_launcher(),
        Group::new(
            "Paragraph",
            vec![grid(vec![
                vec![
                    RibbonButton::new(Icon::Align, "", Action::Macros)
                        .enabled(false)
                        .tip("Align left — planned"),
                    RibbonButton::new(Icon::AlignCenter, "", Action::Macros)
                        .enabled(false)
                        .tip("Align center — planned"),
                    RibbonButton::new(Icon::AlignRight, "", Action::Macros)
                        .enabled(false)
                        .tip("Align right — planned"),
                    RibbonButton::new(Icon::LineSpacing, "", Action::Macros)
                        .enabled(false)
                        .tip("Line spacing — planned"),
                ],
                vec![
                    RibbonButton::new(Icon::Bullets, "", Action::Macros)
                        .enabled(false)
                        .tip("Bullets — planned"),
                    RibbonButton::new(Icon::Numbering, "", Action::Macros)
                        .enabled(false)
                        .tip("Numbering — planned"),
                    RibbonButton::new(Icon::IndentLess, "", Action::Macros)
                        .enabled(false)
                        .tip("Decrease indent — planned"),
                    RibbonButton::new(Icon::IndentMore, "", Action::Macros)
                        .enabled(false)
                        .tip("Increase indent — planned"),
                ],
            ])],
        )
        .with_launcher(),
        Group::new(
            "Shape Styles",
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
            ],
        ),
        Group::new(
            "Arrange",
            vec![
                stack(vec![
                    RibbonButton::new(Icon::BringFront, "Bring Forward", Action::Macros)
                        .enabled(false)
                        .tip("Z-order — planned"),
                    RibbonButton::new(Icon::SendBack, "Send Backward", Action::Macros)
                        .enabled(false)
                        .tip("Z-order — planned"),
                    RibbonButton::new(Icon::Check, "To Page", Action::AlignToPage)
                        .selected(ctx.align_to_page)
                        .tip("Align against the page instead of the primary selection"),
                ]),
                grid(vec![
                    vec![
                        RibbonButton::new(Icon::Align, "Left", Action::AlignLeft)
                            .enabled(multi_sel)
                            .tip("Align left edges (first-selected or page)"),
                        RibbonButton::new(Icon::AlignCenter, "Center", Action::AlignHCenter)
                            .enabled(multi_sel)
                            .tip("Align vertical centers"),
                        RibbonButton::new(Icon::AlignRight, "Right", Action::AlignRight)
                            .enabled(multi_sel)
                            .tip("Align right edges"),
                    ],
                    vec![
                        RibbonButton::new(Icon::AlignTop, "Top", Action::AlignTop)
                            .enabled(multi_sel)
                            .tip("Align top edges"),
                        RibbonButton::new(Icon::AlignMiddle, "Middle", Action::AlignVCenter)
                            .enabled(multi_sel)
                            .tip("Align horizontal middles"),
                        RibbonButton::new(Icon::AlignBottom, "Bottom", Action::AlignBottom)
                            .enabled(multi_sel)
                            .tip("Align bottom edges"),
                    ],
                    vec![
                        RibbonButton::new(Icon::DistH, "Dist H", Action::DistributeH)
                            .enabled(tri_sel)
                            .tip("Even horizontal spacing (3+ shapes)"),
                        RibbonButton::new(Icon::DistV, "Dist V", Action::DistributeV)
                            .enabled(tri_sel)
                            .tip("Even vertical spacing (3+ shapes)"),
                    ],
                ]),
            ],
        ),
    ]
}
