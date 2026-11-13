//! Review tab content (#252 fidelity sweep). Visio group order:
//! **Proofing · Language · Comments · Markup · Reports**. Language
//! splits out of Proofing; Comments grows Visio's set (placeholders —
//! Review becomes the sole comments surface once Insert's duplicate
//! is gone); Markup and Reports are planned groups.

use super::{Action, Group, Icon, RibbonButton, TabCtx, big, grid, stack};

pub(crate) fn groups(s: &'static crate::l10n::L10nLanguage, _ctx: &TabCtx) -> Vec<Group> {
    vec![
        Group::new(s.msg_proofing(), proofing_cols(s, )),
        Group::new(s.msg_language(), language_cols(s, )),
        Group::new(s.msg_comments(), comments_cols(s, )),
        Group::new(s.msg_markup(), markup_cols(s, )),
        Group::new(s.msg_reports(), reports_cols(s, )),
    ]
}

fn proofing_cols(s: &'static crate::l10n::L10nLanguage) -> Vec<super::Col> {
    vec![
        big(
            RibbonButton::new(Icon::Spell, s.msg_spelling(), Action::Macros)
                .enabled(false)
                .tip(s.msg_spell_check_planned()),
        ),
        grid(vec![
            vec![
                RibbonButton::new(Icon::FontA, "", Action::Macros)
                    .alt(s.msg_thesaurus())
                    .enabled(false)
                    .tip(s.msg_thesaurus_planned()),
                RibbonButton::new(Icon::Find, "", Action::Macros)
                    .alt(s.msg_word_count())
                    .enabled(false)
                    .tip(s.msg_word_count_planned()),
            ],
            vec![
                RibbonButton::new(Icon::Check, "", Action::Macros)
                    .alt(s.msg_autocorrect_options())
                    .enabled(false)
                    .tip(s.msg_autocorrect_options_planned()),
            ],
        ]),
    ]
}

fn language_cols(s: &'static crate::l10n::L10nLanguage) -> Vec<super::Col> {
    vec![stack(vec![
        RibbonButton::new(Icon::FontA, s.msg_language(), Action::Macros)
            .enabled(false)
            .tip(s.msg_language_settings_planned()),
        RibbonButton::new(Icon::Shapes, s.msg_translate(), Action::Macros)
            .enabled(false)
            .tip(s.msg_translate_planned()),
        RibbonButton::new(Icon::Units, s.msg_set_proofing_language(), Action::Macros)
            .enabled(false)
            .tip(s.msg_set_proofing_language_planned()),
    ])]
}

fn comments_cols(s: &'static crate::l10n::L10nLanguage) -> Vec<super::Col> {
    vec![
        big(
            RibbonButton::new(Icon::Comment, s.msg_comment_new(), Action::Macros)
                .enabled(false)
                .tip(s.msg_comments_planned()),
        ),
        grid(vec![
            vec![
                RibbonButton::new(Icon::Comment, "", Action::Macros)
                    .alt(s.msg_previous_comment())
                    .enabled(false)
                    .tip(s.msg_previous_comment_planned()),
                RibbonButton::new(Icon::Comment, "", Action::Macros)
                    .alt(s.msg_next_comment())
                    .enabled(false)
                    .tip(s.msg_next_comment_planned()),
            ],
            vec![
                RibbonButton::new(Icon::Delete, "", Action::Macros)
                    .alt(s.msg_delete_comment())
                    .enabled(false)
                    .tip(s.msg_delete_comment_planned()),
                RibbonButton::new(Icon::Eye, "", Action::Macros)
                    .alt(s.msg_show_hide_comments())
                    .enabled(false)
                    .tip(s.msg_show_hide_comments_planned_view()),
            ],
        ]),
    ]
}

fn markup_cols(s: &'static crate::l10n::L10nLanguage) -> Vec<super::Col> {
    vec![stack(vec![
        RibbonButton::new(Icon::Check, s.msg_track_markup(), Action::Macros)
            .enabled(false)
            .tip(s.msg_markup_tracking_planned_no_marku()),
        RibbonButton::new(Icon::Shapes, s.msg_show_markup(), Action::Macros)
            .enabled(false)
            .tip(s.msg_show_markup_planned()),
        RibbonButton::new(Icon::Fit, s.msg_reviewing_pane(), Action::Macros)
            .enabled(false)
            .tip(s.msg_reviewing_pane_planned()),
    ])]
}

fn reports_cols(s: &'static crate::l10n::L10nLanguage) -> Vec<super::Col> {
    vec![
        big(
            RibbonButton::new(Icon::DataGraphic, s.msg_shape_report(), Action::Macros)
                .enabled(false)
                .tip(s.msg_shape_reports_planned()),
        ),
        stack(vec![RibbonButton::new(
            Icon::Field,
            s.msg_manage_reports(),
            Action::Macros,
        )
        .enabled(false)
        .tip(s.msg_report_definitions_planned())]),
    ]
}
