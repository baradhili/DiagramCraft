//! Review tab content (#252 fidelity sweep). Visio group order:
//! **Proofing · Language · Comments · Markup · Reports**. Language
//! splits out of Proofing; Comments grows Visio's set (placeholders —
//! Review becomes the sole comments surface once Insert's duplicate
//! is gone); Markup and Reports are planned groups.

use super::{Action, Group, Icon, RibbonButton, TabCtx, big, grid, stack};

pub(crate) fn groups(_ctx: &TabCtx) -> Vec<Group> {
    vec![
        Group::new(crate::i18n::strs().msg_proofing(), proofing_cols()),
        Group::new(crate::i18n::strs().msg_language(), language_cols()),
        Group::new(crate::i18n::strs().msg_comments(), comments_cols()),
        Group::new(crate::i18n::strs().msg_markup(), markup_cols()),
        Group::new(crate::i18n::strs().msg_reports(), reports_cols()),
    ]
}

fn proofing_cols() -> Vec<super::Col> {
    vec![
        big(
            RibbonButton::new(Icon::Spell, crate::i18n::strs().msg_spelling(), Action::Macros)
                .enabled(false)
                .tip(crate::i18n::strs().msg_spell_check_planned()),
        ),
        grid(vec![
            vec![
                RibbonButton::new(Icon::FontA, "", Action::Macros)
                    .alt(crate::i18n::strs().msg_thesaurus())
                    .enabled(false)
                    .tip(crate::i18n::strs().msg_thesaurus_planned()),
                RibbonButton::new(Icon::Find, "", Action::Macros)
                    .alt(crate::i18n::strs().msg_word_count())
                    .enabled(false)
                    .tip(crate::i18n::strs().msg_word_count_planned()),
            ],
            vec![
                RibbonButton::new(Icon::Check, "", Action::Macros)
                    .alt(crate::i18n::strs().msg_autocorrect_options())
                    .enabled(false)
                    .tip(crate::i18n::strs().msg_autocorrect_options_planned()),
            ],
        ]),
    ]
}

fn language_cols() -> Vec<super::Col> {
    vec![stack(vec![
        RibbonButton::new(Icon::FontA, crate::i18n::strs().msg_language(), Action::Macros)
            .enabled(false)
            .tip(crate::i18n::strs().msg_language_settings_planned()),
        RibbonButton::new(Icon::Shapes, crate::i18n::strs().msg_translate(), Action::Macros)
            .enabled(false)
            .tip(crate::i18n::strs().msg_translate_planned()),
        RibbonButton::new(Icon::Units, crate::i18n::strs().msg_set_proofing_language(), Action::Macros)
            .enabled(false)
            .tip(crate::i18n::strs().msg_set_proofing_language_planned()),
    ])]
}

fn comments_cols() -> Vec<super::Col> {
    vec![
        big(
            RibbonButton::new(Icon::Comment, crate::i18n::strs().msg_comment_new(), Action::Macros)
                .enabled(false)
                .tip(crate::i18n::strs().msg_comments_planned()),
        ),
        grid(vec![
            vec![
                RibbonButton::new(Icon::Comment, "", Action::Macros)
                    .alt(crate::i18n::strs().msg_previous_comment())
                    .enabled(false)
                    .tip(crate::i18n::strs().msg_previous_comment_planned()),
                RibbonButton::new(Icon::Comment, "", Action::Macros)
                    .alt(crate::i18n::strs().msg_next_comment())
                    .enabled(false)
                    .tip(crate::i18n::strs().msg_next_comment_planned()),
            ],
            vec![
                RibbonButton::new(Icon::Delete, "", Action::Macros)
                    .alt(crate::i18n::strs().msg_delete_comment())
                    .enabled(false)
                    .tip(crate::i18n::strs().msg_delete_comment_planned()),
                RibbonButton::new(Icon::Eye, "", Action::Macros)
                    .alt(crate::i18n::strs().msg_show_hide_comments())
                    .enabled(false)
                    .tip(crate::i18n::strs().msg_show_hide_comments_planned_view()),
            ],
        ]),
    ]
}

fn markup_cols() -> Vec<super::Col> {
    vec![stack(vec![
        RibbonButton::new(Icon::Check, crate::i18n::strs().msg_track_markup(), Action::Macros)
            .enabled(false)
            .tip(crate::i18n::strs().msg_markup_tracking_planned_no_marku()),
        RibbonButton::new(Icon::Shapes, crate::i18n::strs().msg_show_markup(), Action::Macros)
            .enabled(false)
            .tip(crate::i18n::strs().msg_show_markup_planned()),
        RibbonButton::new(Icon::Fit, crate::i18n::strs().msg_reviewing_pane(), Action::Macros)
            .enabled(false)
            .tip(crate::i18n::strs().msg_reviewing_pane_planned()),
    ])]
}

fn reports_cols() -> Vec<super::Col> {
    vec![
        big(
            RibbonButton::new(Icon::DataGraphic, crate::i18n::strs().msg_shape_report(), Action::Macros)
                .enabled(false)
                .tip(crate::i18n::strs().msg_shape_reports_planned()),
        ),
        stack(vec![RibbonButton::new(
            Icon::Field,
            crate::i18n::strs().msg_manage_reports(),
            Action::Macros,
        )
        .enabled(false)
        .tip(crate::i18n::strs().msg_report_definitions_planned())]),
    ]
}
