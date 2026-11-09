//! Review tab content (#252 fidelity sweep). Visio group order:
//! **Proofing · Language · Comments · Markup · Reports**. Language
//! splits out of Proofing; Comments grows Visio's set (placeholders —
//! Review becomes the sole comments surface once Insert's duplicate
//! is gone); Markup and Reports are planned groups.

use super::{Action, Group, Icon, RibbonButton, TabCtx, big, grid, stack};

pub(crate) fn groups(_ctx: &TabCtx) -> Vec<Group> {
    vec![
        Group::new("Proofing", proofing_cols()),
        Group::new("Language", language_cols()),
        Group::new("Comments", comments_cols()),
        Group::new("Markup", markup_cols()),
        Group::new("Reports", reports_cols()),
    ]
}

fn proofing_cols() -> Vec<super::Col> {
    vec![
        big(
            RibbonButton::new(Icon::Spell, "Spelling", Action::Macros)
                .enabled(false)
                .tip("Spell check — planned"),
        ),
        grid(vec![
            vec![
                RibbonButton::new(Icon::FontA, "", Action::Macros)
                    .alt("Thesaurus")
                    .enabled(false)
                    .tip("Thesaurus — planned"),
                RibbonButton::new(Icon::Find, "", Action::Macros)
                    .alt("Word Count")
                    .enabled(false)
                    .tip("Word count — planned"),
            ],
            vec![
                RibbonButton::new(Icon::Check, "", Action::Macros)
                    .alt("AutoCorrect Options")
                    .enabled(false)
                    .tip("AutoCorrect options — planned"),
            ],
        ]),
    ]
}

fn language_cols() -> Vec<super::Col> {
    vec![stack(vec![
        RibbonButton::new(Icon::FontA, "Language", Action::Macros)
            .enabled(false)
            .tip("Language settings — planned"),
        RibbonButton::new(Icon::Shapes, "Translate", Action::Macros)
            .enabled(false)
            .tip("Translate — planned"),
        RibbonButton::new(Icon::Units, "Set Proofing Language", Action::Macros)
            .enabled(false)
            .tip("Set proofing language — planned"),
    ])]
}

fn comments_cols() -> Vec<super::Col> {
    vec![
        big(
            RibbonButton::new(Icon::Comment, "New Comment", Action::Macros)
                .enabled(false)
                .tip("Comments — planned"),
        ),
        grid(vec![
            vec![
                RibbonButton::new(Icon::Comment, "", Action::Macros)
                    .alt("Previous Comment")
                    .enabled(false)
                    .tip("Previous comment — planned"),
                RibbonButton::new(Icon::Comment, "", Action::Macros)
                    .alt("Next Comment")
                    .enabled(false)
                    .tip("Next comment — planned"),
            ],
            vec![
                RibbonButton::new(Icon::Delete, "", Action::Macros)
                    .alt("Delete Comment")
                    .enabled(false)
                    .tip("Delete comment — planned"),
                RibbonButton::new(Icon::Eye, "", Action::Macros)
                    .alt("Show/Hide Comments")
                    .enabled(false)
                    .tip("Show/hide comments — planned (View's toggle is wired today)"),
            ],
        ]),
    ]
}

fn markup_cols() -> Vec<super::Col> {
    vec![stack(vec![
        RibbonButton::new(Icon::Check, "Track Markup", Action::Macros)
            .enabled(false)
            .tip("Markup tracking — planned (no markup model yet)"),
        RibbonButton::new(Icon::Shapes, "Show Markup", Action::Macros)
            .enabled(false)
            .tip("Show markup — planned"),
        RibbonButton::new(Icon::Fit, "Reviewing Pane", Action::Macros)
            .enabled(false)
            .tip("Reviewing pane — planned"),
    ])]
}

fn reports_cols() -> Vec<super::Col> {
    vec![
        big(
            RibbonButton::new(Icon::DataGraphic, "Shape Report", Action::Macros)
                .enabled(false)
                .tip("Shape reports — planned"),
        ),
        stack(vec![RibbonButton::new(
            Icon::Field,
            "Manage Reports",
            Action::Macros,
        )
        .enabled(false)
        .tip("Report definitions — planned")]),
    ]
}
